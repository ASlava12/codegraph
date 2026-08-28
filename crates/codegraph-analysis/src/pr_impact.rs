//! PR impact dashboard: map changed files onto the graph before merging.
//!
//! `codegraph pr-impact` takes the files a change touches (from `git diff`
//! or explicit `--file` arguments) and reports what the change actually
//! affects: which graph communities it lands in, which shared hotspots it
//! touches or feeds, the blast radius of reverse dependents with affected
//! entrypoints/tests/routes, and warning/error findings inside the changed
//! code. CI and review state strings are recorded verbatim so pipelines can
//! stamp their context into the `codegraph.pr_impact.v1` artifact.

use crate::{
    InsightSeverity, ProjectAreas, QueryError, communities, entrypoints, hotspots, insights,
};
use codegraph_core::{
    CodeGraph, EdgeKind, NodeId, NodeKind, is_test_like_source_path, names_tests,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

pub const PR_IMPACT_SCHEMA: &str = "codegraph.pr_impact.v1";
const COMMUNITY_LIMIT: usize = 50;
const HOTSPOT_LIMIT: usize = 50;
const RISK_LIMIT: usize = 100;
const SAMPLE_ENTRYPOINT_LIMIT: usize = 10;
const BLAST_DEPTH: usize = 6;

#[derive(Debug, Default, Clone)]
pub struct PrImpactContext {
    pub base: Option<String>,
    pub branch: Option<String>,
    pub ci_state: Option<String>,
    pub review_state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChangedFileImpact {
    pub path: String,
    pub in_graph: bool,
    pub symbols: usize,
    pub community: String,
}

#[derive(Debug, Serialize)]
pub struct TouchedCommunity {
    pub id: String,
    pub label: String,
    pub changed_files: usize,
    pub community_nodes: usize,
}

#[derive(Debug, Serialize)]
pub struct AffectedHotspot {
    pub label: String,
    pub hub_kind: String,
    pub score: usize,
    /// `contains_changes` when the hotspot itself changed;
    /// `depends_on_changes` when it directly uses changed code.
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
pub struct PrBlastRadius {
    pub dependents: usize,
    /// The dependents that are the program rather than its suite. This is
    /// what the risk score counts: a change reaching more tests is better
    /// covered, not more dangerous.
    pub program_dependents: usize,
    pub affected_entrypoints: usize,
    pub affected_tests: usize,
    pub affected_routes: usize,
    pub sample_entrypoints: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PrRisk {
    pub severity: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PrImpactReport {
    pub schema: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_state: Option<String>,
    pub total_changed_files: usize,
    pub matched_files: usize,
    pub changed_files: Vec<ChangedFileImpact>,
    pub touched_communities: Vec<TouchedCommunity>,
    pub affected_hotspots: Vec<AffectedHotspot>,
    pub blast: PrBlastRadius,
    pub risk_score: usize,
    pub risks: Vec<PrRisk>,
    pub risks_truncated: bool,
}

fn severity_label(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Info => "info",
        InsightSeverity::Warning => "warning",
        InsightSeverity::Error => "error",
    }
}

fn normalize(path: &str) -> String {
    path.trim().trim_start_matches("./").replace('\\', "/")
}

/// Changed files from `git diff --name-only <base>`, shared by the CLI
/// command and the HTTP endpoint.
pub fn git_changed_files(root: &Path, base: &str) -> Result<Vec<String>, QueryError> {
    // `base` is passed as a positional argument to git. A value starting with
    // `-` would be parsed as an option (e.g. `--output=<file>` writes an
    // arbitrary file), so reject it before spawning git rather than relying on
    // git's own argument handling.
    if base.starts_with('-') {
        return Err(QueryError::new(format!(
            "invalid base revision {base:?}: must not start with '-'"
        )));
    }
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg(base)
        .arg("--")
        .current_dir(root)
        .output()
        .map_err(|error| QueryError::new(format!("failed to run git diff: {error}")))?;
    if !output.status.success() {
        return Err(QueryError::new(format!(
            "git diff --name-only {base} failed: {}; pass explicit changed files instead",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect())
}

pub fn git_current_branch(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

pub fn pr_impact(
    graph: &CodeGraph,
    changed_paths: &[String],
    context: PrImpactContext,
) -> PrImpactReport {
    let changed_paths: Vec<String> = changed_paths
        .iter()
        .map(|path| normalize(path))
        .filter(|path| !path.is_empty())
        .collect();

    // Changed file nodes and the symbols they contain.
    let mut changed_nodes: BTreeSet<NodeId> = BTreeSet::new();
    let mut changed_files = Vec::new();
    let mut community_counts: BTreeMap<String, usize> = BTreeMap::new();
    let project = ProjectAreas::from_graph(graph);
    for path in &changed_paths {
        let file_node = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == *path);
        let community = project.community_id_for_path(path);
        *community_counts.entry(community.clone()).or_insert(0) += 1;
        match file_node {
            Some(file) => {
                changed_nodes.insert(file.id);
                let mut symbols = 0usize;
                for edge in &graph.edges {
                    if edge.kind == EdgeKind::Contains && edge.source == file.id {
                        changed_nodes.insert(edge.target);
                        symbols += 1;
                    }
                }
                changed_files.push(ChangedFileImpact {
                    path: path.clone(),
                    in_graph: true,
                    symbols,
                    community,
                });
            }
            None => changed_files.push(ChangedFileImpact {
                path: path.clone(),
                in_graph: false,
                symbols: 0,
                community,
            }),
        }
    }

    // Touched communities joined with the community report for size/labels.
    let community_report = communities(graph, COMMUNITY_LIMIT);
    let touched_communities: Vec<TouchedCommunity> = community_counts
        .iter()
        .map(|(id, changed)| {
            let known = community_report
                .communities
                .iter()
                .find(|community| &community.id == id);
            TouchedCommunity {
                id: id.clone(),
                label: known
                    .map(|community| community.label.clone())
                    .unwrap_or_else(|| id.trim_start_matches("area:").to_string()),
                changed_files: *changed,
                community_nodes: known.map(|community| community.node_count).unwrap_or(0),
            }
        })
        .collect();

    // Blast radius: reverse BFS over dependency edges (Contains excluded so
    // parent directories do not count as dependents).
    let mut reverse: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.kind != EdgeKind::Contains {
            reverse.entry(edge.target).or_default().push(edge.source);
        }
    }
    let mut dependents: BTreeSet<NodeId> = BTreeSet::new();
    let mut queue: VecDeque<(NodeId, usize)> =
        changed_nodes.iter().map(|id| (*id, 0usize)).collect();
    let mut visited: BTreeSet<NodeId> = changed_nodes.clone();
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= BLAST_DEPTH {
            continue;
        }
        if let Some(sources) = reverse.get(&node) {
            for source in sources {
                if visited.insert(*source) {
                    dependents.insert(*source);
                    queue.push_back((*source, depth + 1));
                }
            }
        }
    }

    let nodes_by_id: BTreeMap<NodeId, &codegraph_core::Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let entrypoint_ids: BTreeSet<NodeId> = entrypoints(graph).iter().map(|node| node.id).collect();
    let affected: BTreeSet<NodeId> = dependents.union(&changed_nodes).copied().collect();
    let mut affected_entrypoints = 0usize;
    let mut affected_tests = 0usize;
    let mut suite_dependents = 0usize;
    let mut affected_routes = 0usize;
    let mut sample_entrypoints = Vec::new();
    for id in &affected {
        let Some(node) = nodes_by_id.get(id) else {
            continue;
        };
        if entrypoint_ids.contains(id) {
            affected_entrypoints += 1;
            if sample_entrypoints.len() < SAMPLE_ENTRYPOINT_LIMIT {
                sample_entrypoints.push(node.label.clone());
            }
            if node.metadata.get("entrypoint_kind").map(String::as_str) == Some("route") {
                affected_routes += 1;
            }
        }
        // A file or a directory carries no span -- its label is its path.
        let path = node
            .span
            .as_ref()
            .map(|span| span.path.as_str())
            .unwrap_or(match node.kind {
                NodeKind::File | NodeKind::Directory => node.label.as_str(),
                _ => "",
            });
        // Asking whether "test" appears anywhere counted `latest_admin_action_log`
        // and terraform's own `moduletest` package as suite code, and never once
        // counted mastodon's 2741 `spec/` nodes or koel's `.spec.ts` files, because
        // no convention spells its suites with that substring.
        if is_test_like_source_path(path) || names_tests(&node.label) {
            affected_tests += 1;
            if dependents.contains(id) {
                suite_dependents += 1;
            }
        }
    }

    // Hotspots that changed or directly use changed code.
    let hotspot_report = hotspots(graph, HOTSPOT_LIMIT);
    let mut affected_hotspots = Vec::new();
    for hotspot in &hotspot_report.hotspots {
        let reason = if changed_nodes.contains(&hotspot.node.id) {
            Some("contains_changes")
        } else if graph.edges.iter().any(|edge| {
            edge.kind != EdgeKind::Contains
                && edge.source == hotspot.node.id
                && changed_nodes.contains(&edge.target)
        }) {
            Some("depends_on_changes")
        } else {
            None
        };
        if let Some(reason) = reason {
            affected_hotspots.push(AffectedHotspot {
                label: hotspot.node.label.clone(),
                hub_kind: hotspot.hub_kind.clone(),
                score: hotspot.score,
                reason,
            });
        }
    }

    // Warning/error findings inside the changed code. Findings often anchor
    // on the fact a change touches (an unresolved call target, a config key)
    // rather than the changed symbol itself, so the risk scope includes the
    // direct forward neighbors of changed nodes.
    let mut risk_scope = changed_nodes.clone();
    for edge in &graph.edges {
        if changed_nodes.contains(&edge.source) {
            risk_scope.insert(edge.target);
        }
    }
    let insight_report = insights(graph);
    let mut risks = Vec::new();
    let mut truncated = false;
    let mut errors = 0usize;
    let mut warnings = 0usize;
    for insight in &insight_report.insights {
        if matches!(insight.severity, InsightSeverity::Info) {
            continue;
        }
        if !insight.nodes.iter().any(|id| risk_scope.contains(id)) {
            continue;
        }
        match insight.severity {
            InsightSeverity::Error => errors += 1,
            InsightSeverity::Warning => warnings += 1,
            InsightSeverity::Info => {}
        }
        if risks.len() < RISK_LIMIT {
            risks.push(PrRisk {
                severity: severity_label(insight.severity).to_string(),
                kind: insight.kind.clone(),
                message: insight.message.clone(),
            });
        } else {
            truncated = true;
        }
    }

    let matched_files = changed_files.iter().filter(|file| file.in_graph).count();
    // A suite in the blast radius was counted twice -- once inside
    // `dependents`, once again as `affected_tests` -- so flask, whose reach is
    // 78% its own tests, drew more than half its merge-gate risk from being
    // well covered. Risk is how much program the change can reach; the suite
    // it also reaches is the answer to what to run, not to how dangerous this
    // is.
    let program_dependents = dependents.len().saturating_sub(suite_dependents);
    let risk_score = program_dependents + 5 * affected_entrypoints + 5 * errors + 2 * warnings;

    let mut touched_communities = touched_communities;
    touched_communities.sort_by(|a, b| b.changed_files.cmp(&a.changed_files).then(a.id.cmp(&b.id)));

    PrImpactReport {
        schema: PR_IMPACT_SCHEMA.to_string(),
        base: context.base,
        branch: context.branch,
        ci_state: context.ci_state,
        review_state: context.review_state,
        total_changed_files: changed_paths.len(),
        matched_files,
        changed_files,
        touched_communities,
        affected_hotspots,
        blast: PrBlastRadius {
            dependents: dependents.len(),
            program_dependents,
            affected_entrypoints,
            affected_tests,
            affected_routes,
            sample_entrypoints,
        },
        risk_score,
        risks,
        risks_truncated: truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_indexer::{IndexOptions, scan_project};
    use std::fs;
    use std::path::PathBuf;
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
            "codegraph-pr-impact-{kind}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn git_changed_files_rejects_option_like_base() {
        // A `-`-prefixed base would be parsed by git as an option (e.g.
        // `--output=<file>` writes an arbitrary file); it must be rejected
        // before git is ever spawned.
        let root = temp_dir("inject");
        let error = git_changed_files(&root, "--output=/tmp/should-not-be-written")
            .expect_err("option-like base is rejected");
        assert!(
            error.to_string().contains("must not start with '-'"),
            "unexpected error: {error}"
        );
        assert!(
            !std::path::Path::new("/tmp/should-not-be-written").exists(),
            "git must not have run"
        );
    }

    #[test]
    fn the_blast_radius_counts_a_spec_suite_and_not_a_name_holding_test() {
        // Two conventions the old substring rule got backwards: a `.spec.js`
        // suite spells none of its paths with "test", and `latest` spells it
        // without being one.
        let suite = temp_dir("suite");
        fs::create_dir_all(suite.join("src")).unwrap();
        fs::write(
            suite.join("src").join("util.js"),
            "export function helper() {\n  return 1;\n}\n",
        )
        .unwrap();
        fs::write(
            suite.join("src").join("util.spec.js"),
            "import { helper } from './util.js';\n\nit('works', () => {\n  helper();\n});\n",
        )
        .unwrap();
        let graph = scan_project(&suite, &IndexOptions::default()).expect("scan");
        let report = pr_impact(
            &graph,
            &["src/util.js".to_string()],
            PrImpactContext::default(),
        );
        assert!(
            report.blast.affected_tests >= 1,
            "a .spec.js suite is the suite: {:?}",
            report.blast
        );
        // The suite is in the blast radius and is not scored for it.
        assert!(
            report.blast.program_dependents < report.blast.dependents,
            "the spec file is a dependent that is not program: {:?}",
            report.blast
        );
        let weigh = |severity: &str, weight: usize| {
            report
                .risks
                .iter()
                .filter(|risk| risk.severity == severity)
                .count()
                * weight
        };
        assert_eq!(
            report.risk_score,
            report.blast.program_dependents
                + 5 * report.blast.affected_entrypoints
                + weigh("error", 5)
                + weigh("warning", 2),
            "risk counts the program the change reaches, not its coverage"
        );

        let program = temp_dir("program");
        fs::create_dir_all(program.join("src")).unwrap();
        fs::write(
            program.join("src").join("util.js"),
            "export function helper() {\n  return 1;\n}\n",
        )
        .unwrap();
        fs::write(
            program.join("src").join("latest.js"),
            "import { helper } from './util.js';\n\nexport function latestManifest() {\n  return helper();\n}\n",
        )
        .unwrap();
        let graph = scan_project(&program, &IndexOptions::default()).expect("scan");
        let report = pr_impact(
            &graph,
            &["src/util.js".to_string()],
            PrImpactContext::default(),
        );
        assert_eq!(
            report.blast.affected_tests, 0,
            "`latest` and `latestManifest` name no test: {:?}",
            report.blast
        );
    }

    #[test]
    fn dashboard_maps_changed_files_to_communities_blast_and_risks() {
        let root = temp_dir("root");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() {\n    helper();\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("src").join("util.rs"),
            "// FIXME: helper still calls a missing function\npub fn helper() {\n    missing_helper();\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("tests").join("util_test.rs"),
            "#[test]\nfn test_helper() {\n    helper();\n}\n",
        )
        .unwrap();
        let graph = scan_project(&root, &IndexOptions::default()).expect("scan");

        let report = pr_impact(
            &graph,
            &["src/util.rs".to_string(), "./docs/unrelated.md".to_string()],
            PrImpactContext {
                base: Some("origin/main".to_string()),
                branch: Some("feature/util".to_string()),
                ci_state: Some("pending".to_string()),
                review_state: None,
            },
        );

        assert_eq!(report.schema, PR_IMPACT_SCHEMA);
        assert_eq!(report.total_changed_files, 2);
        assert_eq!(report.matched_files, 1);
        assert_eq!(report.base.as_deref(), Some("origin/main"));
        assert_eq!(report.ci_state.as_deref(), Some("pending"));

        let matched = report
            .changed_files
            .iter()
            .find(|file| file.path == "src/util.rs")
            .expect("matched file");
        assert!(matched.in_graph);
        assert!(matched.symbols >= 1, "helper symbol should be counted");
        assert_eq!(matched.community, "area:src");
        let unmatched = report
            .changed_files
            .iter()
            .find(|file| file.path == "docs/unrelated.md")
            .expect("unmatched file normalized and listed");
        assert!(!unmatched.in_graph);

        let src_community = report
            .touched_communities
            .iter()
            .find(|community| community.id == "area:src")
            .expect("src community touched");
        assert_eq!(src_community.changed_files, 1);
        assert!(src_community.community_nodes > 0);

        assert!(
            report.blast.dependents >= 2,
            "main and the test depend on helper: {:?}",
            report.blast
        );
        assert!(
            report.blast.affected_entrypoints >= 1,
            "main entrypoint must be affected"
        );
        assert!(
            report.blast.affected_tests >= 1,
            "test caller must be counted"
        );
        assert!(!report.blast.sample_entrypoints.is_empty());

        assert!(
            report
                .risks
                .iter()
                .any(|risk| risk.severity == "warning" || risk.severity == "error"),
            "unresolved call inside changed file must surface: {:?}",
            report.risks
        );
        assert!(report.risk_score > 0);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hotspots_report_contains_or_depends_reasons() {
        let root = temp_dir("hub");
        fs::create_dir_all(root.join("src")).unwrap();
        // hub is called by many; hub itself calls changed code.
        let mut main_body =
            String::from("fn main() {\n    hub();\n}\n\npub fn hub() {\n    changed_fn();\n}\n");
        for i in 0..6 {
            main_body.push_str(&format!("\nfn caller_{i}() {{\n    hub();\n}}\n"));
        }
        fs::write(root.join("src").join("main.rs"), main_body).unwrap();
        fs::write(
            root.join("src").join("changed.rs"),
            "pub fn changed_fn() {}\n",
        )
        .unwrap();
        let graph = scan_project(&root, &IndexOptions::default()).expect("scan");

        let report = pr_impact(
            &graph,
            &["src/changed.rs".to_string()],
            PrImpactContext::default(),
        );
        let depends = report
            .affected_hotspots
            .iter()
            .find(|hotspot| hotspot.label == "hub");
        if let Some(depends) = depends {
            assert_eq!(depends.reason, "depends_on_changes");
        }
        let contains = pr_impact(
            &graph,
            &["src/main.rs".to_string()],
            PrImpactContext::default(),
        );
        assert!(
            contains
                .affected_hotspots
                .iter()
                .any(|hotspot| hotspot.reason == "contains_changes"),
            "changed hub file must flag contains_changes: {:?}",
            contains.affected_hotspots
        );
        fs::remove_dir_all(root).ok();
    }
}
