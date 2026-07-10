//! Git hooks: post-commit/post-checkout graph refresh with optional exports.
//!
//! `codegraph install-hooks` writes marker-delimited blocks into
//! `.git/hooks/post-commit` and `.git/hooks/post-checkout` (following
//! `gitdir:` redirects for worktrees), so every commit and checkout runs
//! `codegraph hook-run`: the same safe incremental refresh as watch mode,
//! plus optional export regeneration configured under `[hooks]` in
//! `.codegraph/config.toml`. Existing hook scripts are preserved — the
//! codegraph block is appended or replaced in place, and reruns are
//! idempotent. Hook output is appended to `.codegraph/hooks.log` by the
//! generated script so commits stay quiet.

use anyhow::{Context, Result, bail};
use codegraph_indexer::IndexOptions;
use codegraph_storage::{GraphCache, scan_project_cached};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const HOOKS_START: &str = "# <<< codegraph:start >>>";
pub const HOOKS_END: &str = "# <<< codegraph:end >>>";
pub const HOOK_KINDS: &[&str] = &["post-commit", "post-checkout"];

const KNOWN_EXPORT_FORMATS: &[&str] = &["json", "dot", "ndjson"];

#[derive(Debug, Default, Serialize)]
pub struct InstallHooksReport {
    pub hooks_dir: String,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookSettings {
    /// Export formats regenerated after each refresh (json, dot, ndjson).
    pub exports: Vec<String>,
    /// Directory for regenerated exports, relative to the project root.
    pub export_dir: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HookRunReport {
    pub kind: String,
    pub recorded_at_unix: u64,
    pub refreshed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored: Option<bool>,
    pub current_hash: String,
    pub exports: Vec<String>,
    pub duration_ms: u64,
}

/// Resolve the hooks directory, following a `gitdir:` redirect when `.git`
/// is a file (linked worktrees).
pub fn hooks_dir(root: &Path) -> Result<PathBuf> {
    let git_path = root.join(".git");
    if git_path.is_dir() {
        return Ok(git_path.join("hooks"));
    }
    if git_path.is_file() {
        let raw = fs::read_to_string(&git_path)
            .with_context(|| format!("failed to read {}", git_path.display()))?;
        let Some(gitdir) = raw
            .lines()
            .find_map(|line| line.trim().strip_prefix("gitdir:"))
        else {
            bail!("{} exists but has no gitdir: line", git_path.display());
        };
        let gitdir = gitdir.trim();
        let gitdir_path = if Path::new(gitdir).is_absolute() {
            PathBuf::from(gitdir)
        } else {
            root.join(gitdir)
        };
        return Ok(gitdir_path.join("hooks"));
    }
    bail!(
        "{} is not a git repository; run install-hooks from a repository root",
        root.display()
    );
}

fn hook_block(kind: &str) -> String {
    format!(
        "{HOOKS_START}\n\
         # Refresh the CodeGraph cache after {kind}; see .codegraph/hooks.log.\n\
         if command -v codegraph >/dev/null 2>&1; then\n\
         \x20\x20mkdir -p .codegraph\n\
         \x20\x20codegraph hook-run {kind} . >> .codegraph/hooks.log 2>&1 || true\n\
         fi\n\
         {HOOKS_END}"
    )
}

pub fn install_hooks(root: &Path) -> Result<InstallHooksReport> {
    let hooks_dir = hooks_dir(root)?;
    fs::create_dir_all(&hooks_dir)
        .with_context(|| format!("failed to create {}", hooks_dir.display()))?;
    let mut report = InstallHooksReport {
        hooks_dir: hooks_dir.display().to_string(),
        ..InstallHooksReport::default()
    };

    for kind in HOOK_KINDS {
        let path = hooks_dir.join(kind);
        let block = hook_block(kind);
        if !path.exists() {
            fs::write(&path, format!("#!/bin/sh\n{block}\n"))
                .with_context(|| format!("failed to write {}", path.display()))?;
            make_executable(&path)?;
            report.created.push((*kind).to_string());
            continue;
        }

        let existing = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let next = match (existing.find(HOOKS_START), existing.find(HOOKS_END)) {
            (Some(start), Some(end)) if end > start => {
                let mut next = String::new();
                next.push_str(&existing[..start]);
                next.push_str(&block);
                next.push_str(&existing[end + HOOKS_END.len()..]);
                next
            }
            _ => {
                let mut next = existing.clone();
                if !next.ends_with('\n') {
                    next.push('\n');
                }
                next.push('\n');
                next.push_str(&block);
                next.push('\n');
                next
            }
        };
        if next == existing {
            make_executable(&path)?;
            report.unchanged.push((*kind).to_string());
            continue;
        }
        fs::write(&path, next).with_context(|| format!("failed to write {}", path.display()))?;
        make_executable(&path)?;
        report.updated.push((*kind).to_string());
    }
    Ok(report)
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .with_context(|| format!("failed to stat {}", path.display()))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}

/// Read `[hooks]` from `.codegraph/config.toml`; absent file or section means
/// no exports.
pub fn load_settings(root: &Path) -> Result<HookSettings> {
    let mut settings = HookSettings::default();
    let path = root.join(".codegraph").join("config.toml");
    if !path.is_file() {
        return Ok(settings);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let table = raw
        .parse::<toml::Table>()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;
    let Some(section) = table.get("hooks") else {
        return Ok(settings);
    };
    let Some(section) = section.as_table() else {
        bail!("{}: [hooks] must be a table", path.display());
    };
    if let Some(value) = section.get("exports") {
        let Some(items) = value.as_array() else {
            bail!(
                "{}: hooks.exports must be an array of format names",
                path.display()
            );
        };
        let mut exports = Vec::new();
        for item in items {
            let Some(format) = item.as_str() else {
                bail!(
                    "{}: hooks.exports must be an array of format names",
                    path.display()
                );
            };
            if !KNOWN_EXPORT_FORMATS.contains(&format) {
                bail!(
                    "{}: hooks.exports format `{format}` is not supported; expected one of {}",
                    path.display(),
                    KNOWN_EXPORT_FORMATS.join(", ")
                );
            }
            if !exports.contains(&format.to_string()) {
                exports.push(format.to_string());
            }
        }
        settings.exports = exports;
    }
    if let Some(value) = section.get("export_dir") {
        let Some(dir) = value.as_str() else {
            bail!("{}: hooks.export_dir must be a string", path.display());
        };
        settings.export_dir = Some(dir.to_string());
    }
    Ok(settings)
}

/// The hook action: safe incremental refresh (with the same full-rescan
/// fallback as watch mode) plus optional export regeneration.
pub fn hook_run(
    cache: &GraphCache,
    root: &Path,
    options: &IndexOptions,
    kind: &str,
    limit: usize,
    recorded_at_unix: u64,
) -> Result<HookRunReport> {
    let started = Instant::now();
    let settings = load_settings(root)?;
    let tick = crate::watch::watch_tick(cache, root, options, None, limit, recorded_at_unix)?;

    let mut exports = Vec::new();
    if !settings.exports.is_empty() {
        // The cache is warm after the tick, so this loads the stored graph.
        let output = scan_project_cached(root, options, Some(cache))?;
        let export_dir = root.join(
            settings
                .export_dir
                .as_deref()
                .unwrap_or(".codegraph/exports"),
        );
        fs::create_dir_all(&export_dir)
            .with_context(|| format!("failed to create {}", export_dir.display()))?;
        for format in &settings.exports {
            let (file_name, contents) = match format.as_str() {
                "json" => (
                    "graph.json",
                    format!("{}\n", serde_json::to_string_pretty(&output.graph)?),
                ),
                "dot" => ("graph.dot", codegraph_analysis::export_dot(&output.graph)),
                "ndjson" => (
                    "graph.ndjson",
                    codegraph_analysis::export_ndjson(&output.graph)?,
                ),
                other => bail!("unsupported hook export format `{other}`"),
            };
            let path = export_dir.join(file_name);
            fs::write(&path, contents)
                .with_context(|| format!("failed to write {}", path.display()))?;
            exports.push(path.display().to_string());
        }
    }

    Ok(HookRunReport {
        kind: kind.to_string(),
        recorded_at_unix,
        refreshed: tick.refresh.is_some(),
        action: tick.refresh.as_ref().map(|refresh| refresh.action),
        stored: tick.refresh.as_ref().map(|refresh| refresh.stored),
        current_hash: tick.current_hash,
        exports,
        duration_ms: started.elapsed().as_millis() as u64,
    })
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
            "codegraph-hooks-{kind}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn git_root() -> PathBuf {
        let root = temp_dir("root");
        fs::create_dir_all(root.join(".git").join("hooks")).expect("git dirs");
        root
    }

    #[test]
    fn install_creates_idempotent_executable_hooks() {
        let root = git_root();
        let report = install_hooks(&root).expect("install");
        assert_eq!(report.created, vec!["post-commit", "post-checkout"]);

        for kind in HOOK_KINDS {
            let path = root.join(".git").join("hooks").join(kind);
            let script = fs::read_to_string(&path).expect("hook script");
            assert!(script.starts_with("#!/bin/sh"));
            assert!(script.contains(HOOKS_START));
            assert!(script.contains(&format!("codegraph hook-run {kind}")));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&path).unwrap().permissions().mode();
                assert_eq!(mode & 0o111, 0o111, "{kind} must be executable");
            }
        }

        let rerun = install_hooks(&root).expect("rerun");
        assert!(rerun.created.is_empty());
        assert!(rerun.updated.is_empty());
        assert_eq!(rerun.unchanged.len(), 2);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn install_preserves_existing_hooks_and_replaces_stale_blocks() {
        let root = git_root();
        let path = root.join(".git").join("hooks").join("post-commit");
        fs::write(&path, "#!/bin/sh\necho custom-step\n").unwrap();

        let report = install_hooks(&root).expect("install");
        assert!(report.updated.contains(&"post-commit".to_string()));
        let script = fs::read_to_string(&path).unwrap();
        assert!(script.contains("echo custom-step"));
        assert!(script.contains(HOOKS_START));

        // Simulate an outdated codegraph block: it is replaced in place.
        let stale = script.replace("codegraph hook-run post-commit", "codegraph old-command");
        fs::write(&path, stale).unwrap();
        let second = install_hooks(&root).expect("second install");
        assert!(second.updated.contains(&"post-commit".to_string()));
        let script = fs::read_to_string(&path).unwrap();
        assert!(script.contains("codegraph hook-run post-commit"));
        assert!(!script.contains("codegraph old-command"));
        assert_eq!(script.matches(HOOKS_START).count(), 1);
        assert!(script.contains("echo custom-step"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hooks_dir_follows_worktree_gitdir_redirect() {
        let root = temp_dir("worktree");
        let real_git = temp_dir("gitdir");
        fs::write(
            root.join(".git"),
            format!("gitdir: {}\n", real_git.display()),
        )
        .unwrap();
        let dir = hooks_dir(&root).expect("redirected hooks dir");
        assert_eq!(dir, real_git.join("hooks"));

        let plain = temp_dir("plain");
        let error = hooks_dir(&plain).expect_err("not a repository");
        assert!(error.to_string().contains("not a git repository"));
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(real_git).ok();
        fs::remove_dir_all(plain).ok();
    }

    #[test]
    fn hook_settings_parse_and_reject_unknown_formats() {
        let root = temp_dir("settings");
        assert_eq!(load_settings(&root).unwrap(), HookSettings::default());

        fs::create_dir_all(root.join(".codegraph")).unwrap();
        fs::write(
            root.join(".codegraph").join("config.toml"),
            "[hooks]\nexports = [\"dot\", \"json\", \"dot\"]\nexport_dir = \"exports\"\n",
        )
        .unwrap();
        let settings = load_settings(&root).unwrap();
        assert_eq!(settings.exports, vec!["dot", "json"], "duplicates dropped");
        assert_eq!(settings.export_dir.as_deref(), Some("exports"));

        fs::write(
            root.join(".codegraph").join("config.toml"),
            "[hooks]\nexports = [\"svg\"]\n",
        )
        .unwrap();
        let error = load_settings(&root).expect_err("unknown format");
        assert!(error.to_string().contains("svg"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hook_run_refreshes_cache_and_regenerates_exports() {
        let root = temp_dir("run-root");
        let cache_dir = temp_dir("run-cache");
        fs::create_dir_all(root.join(".codegraph")).unwrap();
        fs::write(
            root.join(".codegraph").join("config.toml"),
            "[hooks]\nexports = [\"dot\", \"json\"]\n",
        )
        .unwrap();
        fs::write(root.join("main.py"), "def worker():\n    return 1\n").unwrap();
        let cache = GraphCache::new(&cache_dir);
        let options = IndexOptions::default();

        let first = hook_run(&cache, &root, &options, "post-commit", 100, 1).expect("first run");
        assert_eq!(first.kind, "post-commit");
        assert!(first.refreshed, "cold cache should refresh");
        assert_eq!(first.stored, Some(true));
        assert_eq!(first.exports.len(), 2);
        let dot_path = root.join(".codegraph").join("exports").join("graph.dot");
        assert!(dot_path.is_file());
        assert!(fs::read_to_string(&dot_path).unwrap().contains("worker"));
        assert!(
            root.join(".codegraph")
                .join("exports")
                .join("graph.json")
                .is_file()
        );

        let second = hook_run(&cache, &root, &options, "post-checkout", 100, 2).expect("second");
        assert!(!second.refreshed, "warm cache needs no refresh");
        assert_eq!(second.current_hash, first.current_hash);
        assert_eq!(second.exports.len(), 2, "exports still regenerated");
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(cache_dir).ok();
    }
}
