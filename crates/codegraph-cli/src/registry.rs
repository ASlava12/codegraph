//! Global graph registry: query several local repositories as one set.
//!
//! The registry is a machine-managed JSON file (default
//! `<cache-dir>/registry.json`) mapping short project names to canonical
//! repository roots. `registry-query` runs one graph query expression —
//! including `path from:.. to:..` — against every registered (or selected)
//! repository through the shared persistent cache and returns per-project
//! results; a repository that fails to scan reports its error inline instead
//! of failing the whole run, so one broken checkout never hides answers from
//! the others.

use anyhow::{Context, Result, bail};
use codegraph_analysis::{compact_query_result, query_graph};
use codegraph_indexer::{IndexOptionOverrides, configured_index_options};
use codegraph_storage::{GraphCache, default_cache_dir, scan_project_cached};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const REGISTRY_FILE: &str = "registry.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub projects: Vec<RegistryProject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryProject {
    pub name: String,
    pub root: String,
}

#[derive(Debug, Serialize)]
pub struct RegistryListEntry {
    pub name: String,
    pub root: String,
    pub exists: bool,
}

#[derive(Debug, Serialize)]
pub struct RegistryListReport {
    pub registry_path: String,
    pub total: usize,
    pub projects: Vec<RegistryListEntry>,
}

#[derive(Debug, Serialize)]
pub struct RegistryQueryEntry {
    pub project: String,
    pub root: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegistryQueryReport {
    pub expression: String,
    pub registry_path: String,
    pub total_projects: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<RegistryQueryEntry>,
}

pub fn default_registry_path() -> PathBuf {
    default_cache_dir().join(REGISTRY_FILE)
}

pub fn load(path: &Path) -> Result<Registry> {
    if !path.exists() {
        return Ok(Registry::default());
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let registry: Registry = serde_json::from_str(&raw)
        .with_context(|| format!("{} is not a valid registry file", path.display()))?;
    Ok(registry)
}

fn save(path: &Path, registry: &Registry) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut registry = registry.clone();
    registry.projects.sort_by(|a, b| a.name.cmp(&b.name));
    let raw = format!("{}\n", serde_json::to_string_pretty(&registry)?);
    fs::write(path, raw).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// Register a repository under a short name (default: its directory name).
/// Re-adding the same root under the same name is a no-op.
pub fn add(registry_path: &Path, root: &Path, name: Option<String>) -> Result<RegistryProject> {
    let root = root
        .canonicalize()
        .with_context(|| format!("{} does not exist", root.display()))?;
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }
    let name = match name {
        Some(name) if !name.trim().is_empty() => name.trim().to_string(),
        _ => root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty())
            .with_context(|| {
                format!("cannot derive a name from {}; pass --name", root.display())
            })?,
    };

    let mut registry = load(registry_path)?;
    let root_text = root.display().to_string();
    if let Some(existing) = registry
        .projects
        .iter()
        .find(|project| project.name == name)
    {
        if existing.root == root_text {
            return Ok(existing.clone());
        }
        bail!(
            "name `{name}` is already registered for {}; pass --name to pick another",
            existing.root
        );
    }
    let project = RegistryProject {
        name,
        root: root_text,
    };
    registry.projects.push(project.clone());
    save(registry_path, &registry)?;
    Ok(project)
}

/// Remove a project by name; returns the removed entry.
pub fn remove(registry_path: &Path, name: &str) -> Result<RegistryProject> {
    let mut registry = load(registry_path)?;
    let index = registry
        .projects
        .iter()
        .position(|project| project.name == name)
        .with_context(|| format!("project `{name}` is not registered"))?;
    let removed = registry.projects.remove(index);
    save(registry_path, &registry)?;
    Ok(removed)
}

pub fn list(registry_path: &Path) -> Result<RegistryListReport> {
    let registry = load(registry_path)?;
    let projects: Vec<RegistryListEntry> = registry
        .projects
        .iter()
        .map(|project| RegistryListEntry {
            name: project.name.clone(),
            root: project.root.clone(),
            exists: Path::new(&project.root).is_dir(),
        })
        .collect();
    Ok(RegistryListReport {
        registry_path: registry_path.display().to_string(),
        total: projects.len(),
        projects,
    })
}

/// Run one query expression across registered repositories. `projects`
/// narrows the run to the named subset; unknown names are an error so typos
/// do not silently query nothing.
pub fn run_query(
    registry_path: &Path,
    cache: Option<&GraphCache>,
    expression: &str,
    projects: &[String],
    compact: bool,
) -> Result<RegistryQueryReport> {
    let registry = load(registry_path)?;
    if registry.projects.is_empty() {
        bail!("no repositories are registered; add one with `codegraph registry-add <path>` first");
    }
    for requested in projects {
        if !registry
            .projects
            .iter()
            .any(|project| &project.name == requested)
        {
            bail!(
                "project `{requested}` is not registered; known projects: {}",
                registry
                    .projects
                    .iter()
                    .map(|project| project.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    let selected: Vec<&RegistryProject> = registry
        .projects
        .iter()
        .filter(|project| projects.is_empty() || projects.contains(&project.name))
        .collect();

    let mut results = Vec::new();
    for project in selected {
        let root = PathBuf::from(&project.root);
        let outcome = query_project(&root, cache, expression, compact);
        results.push(match outcome {
            Ok(result) => RegistryQueryEntry {
                project: project.name.clone(),
                root: project.root.clone(),
                ok: true,
                result: Some(result),
                error: None,
            },
            Err(error) => RegistryQueryEntry {
                project: project.name.clone(),
                root: project.root.clone(),
                ok: false,
                result: None,
                error: Some(format!("{error:#}")),
            },
        });
    }

    Ok(RegistryQueryReport {
        expression: expression.to_string(),
        registry_path: registry_path.display().to_string(),
        total_projects: results.len(),
        succeeded: results.iter().filter(|entry| entry.ok).count(),
        failed: results.iter().filter(|entry| !entry.ok).count(),
        results,
    })
}

fn query_project(
    root: &Path,
    cache: Option<&GraphCache>,
    expression: &str,
    compact: bool,
) -> Result<serde_json::Value> {
    let options = configured_index_options(root, &IndexOptionOverrides::default())?;
    let output = scan_project_cached(root, &options, cache)?;
    let result = query_graph(&output.graph, expression)?;
    let result = if compact {
        compact_query_result(result)
    } else {
        result
    };
    Ok(serde_json::to_value(result)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(kind: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .subsec_nanos();
        let counter = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "codegraph-registry-{kind}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn add_list_remove_roundtrip_is_idempotent() {
        let base = temp_dir("base");
        let registry_path = base.join("registry.json");
        let alpha = temp_dir("alpha");
        let beta = temp_dir("beta");

        let first = add(&registry_path, &alpha, None).expect("add alpha");
        assert_eq!(
            first.name,
            alpha.file_name().unwrap().to_string_lossy().as_ref()
        );
        let again = add(&registry_path, &alpha, None).expect("re-add alpha");
        assert_eq!(again, first, "re-adding the same root is a no-op");
        add(&registry_path, &beta, Some("beta-repo".to_string())).expect("add beta");

        let report = list(&registry_path).expect("list");
        assert_eq!(report.total, 2);
        assert!(report.projects.iter().all(|entry| entry.exists));

        let conflicting = temp_dir("conflict");
        let error =
            add(&registry_path, &conflicting, Some(first.name.clone())).expect_err("name clash");
        assert!(error.to_string().contains("already registered"));

        let removed = remove(&registry_path, "beta-repo").expect("remove");
        assert_eq!(removed.name, "beta-repo");
        assert_eq!(list(&registry_path).unwrap().total, 1);
        let missing = remove(&registry_path, "beta-repo").expect_err("already removed");
        assert!(missing.to_string().contains("not registered"));

        for dir in [base, alpha, beta, conflicting] {
            fs::remove_dir_all(dir).ok();
        }
    }

    #[test]
    fn add_rejects_missing_paths_and_loads_tolerate_absent_file() {
        let base = temp_dir("missing");
        let registry_path = base.join("registry.json");
        assert_eq!(load(&registry_path).unwrap().projects.len(), 0);
        let error = add(&registry_path, &base.join("ghost"), None).expect_err("missing root");
        assert!(error.to_string().contains("does not exist"));
        fs::remove_dir_all(base).ok();
    }

    #[test]
    fn cross_project_query_returns_per_project_results_and_errors() {
        let base = temp_dir("query-base");
        let registry_path = base.join("registry.json");
        let cache_dir = temp_dir("query-cache");
        let cache = GraphCache::new(&cache_dir);

        let alpha = temp_dir("query-alpha");
        fs::write(
            alpha.join("main.py"),
            "def alpha_entry():\n    alpha_helper()\n\ndef alpha_helper():\n    return 1\n",
        )
        .unwrap();
        let beta = temp_dir("query-beta");
        fs::write(
            beta.join("main.rs"),
            "fn beta_entry() { beta_helper(); }\n\nfn beta_helper() {}\n",
        )
        .unwrap();

        add(&registry_path, &alpha, Some("alpha".to_string())).unwrap();
        add(&registry_path, &beta, Some("beta".to_string())).unwrap();

        let report = run_query(
            &registry_path,
            Some(&cache),
            "nodes kind:function",
            &[],
            false,
        )
        .expect("cross-project query");
        assert_eq!(report.total_projects, 2);
        assert_eq!(report.succeeded, 2);
        let alpha_result = report
            .results
            .iter()
            .find(|entry| entry.project == "alpha")
            .expect("alpha entry");
        assert!(alpha_result.ok);
        assert_eq!(alpha_result.result.as_ref().unwrap()["total_nodes"], 2);

        // Path queries work per project too.
        let path_report = run_query(
            &registry_path,
            Some(&cache),
            "path from:beta_entry to:beta_helper",
            &["beta".to_string()],
            false,
        )
        .expect("path query");
        assert_eq!(path_report.total_projects, 1);
        assert!(path_report.results[0].ok);

        // A broken checkout reports inline instead of failing the whole run.
        fs::remove_dir_all(&alpha).unwrap();
        let degraded = run_query(
            &registry_path,
            Some(&cache),
            "nodes kind:function",
            &[],
            false,
        )
        .expect("degraded query");
        assert_eq!(degraded.succeeded, 1);
        assert_eq!(degraded.failed, 1);
        let broken = degraded
            .results
            .iter()
            .find(|entry| entry.project == "alpha")
            .unwrap();
        assert!(!broken.ok);
        assert!(broken.error.is_some());

        // Unknown project filters fail loudly.
        let unknown = run_query(
            &registry_path,
            Some(&cache),
            "nodes",
            &["ghost".to_string()],
            false,
        )
        .expect_err("unknown project");
        assert!(unknown.to_string().contains("known projects"));

        for dir in [base, cache_dir, beta] {
            fs::remove_dir_all(dir).ok();
        }
    }
}
