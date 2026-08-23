//! Obsidian vault / Markdown wiki export.
//!
//! `codegraph export-wiki` writes a folder of interlinked Markdown notes that
//! opens directly as an Obsidian vault (or any Markdown wiki): a `Home` index
//! plus bounded pages for communities, entrypoints, hotspots, config flows,
//! and risky insights, cross-linked with `[[wikilinks]]`. Content is derived
//! deterministically from the existing graph reports, every section states
//! its truncation explicitly, and regeneration overwrites the same files so
//! the vault can live in version control next to the code.

use anyhow::{Context, Result};
use codegraph_analysis::{InsightSeverity, communities, entrypoints, hotspots, insights};
use codegraph_core::{CodeGraph, NodeKind};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const COMMUNITY_LIMIT: usize = 50;
const HOTSPOT_LIMIT: usize = 50;
const ENTRYPOINT_LIMIT: usize = 200;
const CONFIG_LIMIT: usize = 200;
const RISK_LIMIT: usize = 300;
const READERS_PER_CONFIG: usize = 8;

#[derive(Debug, Serialize)]
pub struct WikiExportReport {
    pub output_dir: String,
    pub files: Vec<String>,
    pub communities: usize,
    pub entrypoints: usize,
    pub hotspots: usize,
    pub config_flows: usize,
    pub risks: usize,
}

fn severity_label(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Info => "info",
        InsightSeverity::Warning => "warning",
        InsightSeverity::Error => "error",
    }
}

/// Escape characters that break Markdown emphasis/links inside labels.
fn md_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '|' => escaped.push_str("\\|"),
            '[' => escaped.push_str("\\["),
            ']' => escaped.push_str("\\]"),
            _ => escaped.push(c),
        }
    }
    escaped
}

fn node_location(node: &codegraph_core::Node) -> Option<String> {
    node.span
        .as_ref()
        .map(|span| format!("{}:{}", span.path, span.start_line))
        .or_else(|| node.metadata.get("path").cloned())
}

pub fn export_wiki(graph: &CodeGraph, output_dir: &Path, label: &str) -> Result<WikiExportReport> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    let community_report = communities(graph, COMMUNITY_LIMIT);
    let entrypoint_nodes = entrypoints(graph);
    let hotspot_report = hotspots(graph, HOTSPOT_LIMIT);
    let insight_report = insights(graph);

    // Communities page.
    let mut communities_md = String::from(
        "[[Home]]\n\n# Communities\n\nSubsystems detected from graph connectivity, with size and coupling counts.\n\n",
    );
    for community in &community_report.communities {
        communities_md.push_str(&format!(
            "## {}\n\n- Nodes: {} ({} files, {} entrypoints)\n- Edges: {} internal, {} incoming / {} outgoing external\n",
            md_escape(&community.label),
            community.node_count,
            community.files,
            community.entrypoints,
            community.internal_edges,
            community.incoming_external_edges,
            community.outgoing_external_edges,
        ));
        if !community.languages.is_empty() {
            let languages: Vec<String> = community
                .languages
                .iter()
                .map(|(language, count)| format!("{language} ({count})"))
                .collect();
            communities_md.push_str(&format!("- Languages: {}\n", languages.join(", ")));
        }
        if !community.sample_nodes.is_empty() {
            communities_md.push_str("- Sample nodes:\n");
            for node in community.sample_nodes.iter().take(6) {
                communities_md.push_str(&format!("  - `{}`\n", md_escape(&node.label)));
            }
        }
        communities_md.push('\n');
    }
    if community_report.truncated {
        communities_md.push_str(&format!(
            "> Truncated: showing {} of {} communities.\n",
            community_report.communities.len(),
            community_report.total_communities
        ));
    }

    // Entrypoints page.
    let mut entrypoints_md = String::from(
        "[[Home]]\n\n# Entrypoints\n\nWhere execution starts: binaries, routes, CI jobs, containers.\n\n",
    );
    // `entrypoints` already ranks programs above CI jobs, so group by first
    // appearance instead of alphabetically — sorting by kind name put
    // `entrypoint` (a Dockerfile) and `make_target` ahead of the program.
    let mut entrypoints_by_kind: Vec<(String, Vec<String>)> = Vec::new();
    for node in entrypoint_nodes.iter().take(ENTRYPOINT_LIMIT) {
        let kind = node
            .metadata
            .get("entrypoint_kind")
            .cloned()
            .unwrap_or_else(|| "other".to_string());
        let mut line = format!("`{}`", md_escape(&node.label));
        if let Some(location) = node_location(node) {
            line.push_str(&format!(" — {}", md_escape(&location)));
        }
        match entrypoints_by_kind
            .iter_mut()
            .find(|(existing, _)| existing == &kind)
        {
            Some((_, lines)) => lines.push(line),
            None => entrypoints_by_kind.push((kind, vec![line])),
        }
    }
    for (kind, lines) in &entrypoints_by_kind {
        entrypoints_md.push_str(&format!("## {kind}\n\n"));
        for line in lines {
            entrypoints_md.push_str(&format!("- {line}\n"));
        }
        entrypoints_md.push('\n');
    }
    if entrypoint_nodes.len() > ENTRYPOINT_LIMIT {
        entrypoints_md.push_str(&format!(
            "> Truncated: showing {ENTRYPOINT_LIMIT} of {} entrypoints.\n",
            entrypoint_nodes.len()
        ));
    }

    // Hotspots page.
    let mut hotspots_md = String::from(
        "[[Home]]\n\n# Hotspots\n\nHigh-degree nodes; architectural hubs deserve care before refactors, utility hubs are usually noise.\n\n| Node | Hub kind | Score | In | Out |\n| --- | --- | ---: | ---: | ---: |\n",
    );
    for hotspot in &hotspot_report.hotspots {
        hotspots_md.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            md_escape(&hotspot.node.label),
            hotspot.hub_kind,
            hotspot.score,
            hotspot.incoming,
            hotspot.outgoing
        ));
    }
    if hotspot_report.truncated {
        hotspots_md.push_str(&format!(
            "\n> Truncated: showing {} of {} candidates.\n",
            hotspot_report.hotspots.len(),
            hotspot_report.total_candidates
        ));
    }

    // Config flows page.
    let mut node_labels: BTreeMap<u64, &codegraph_core::Node> = BTreeMap::new();
    for node in &graph.nodes {
        node_labels.insert(node.id.0, node);
    }
    let mut configs_md = String::from(
        "[[Home]]\n\n# Config Flows\n\nConfiguration and environment reads with the code that touches them.\n\n",
    );
    let config_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| is_configuration_node(node))
        .take(CONFIG_LIMIT)
        .collect();
    for node in &config_nodes {
        let kind = match node.kind {
            NodeKind::Environment => "environment",
            _ => "config",
        };
        configs_md.push_str(&format!("## `{}` ({kind})\n\n", md_escape(&node.label)));
        if let Some(location) = node_location(node) {
            configs_md.push_str(&format!("- Location: {}\n", md_escape(&location)));
        }
        let mut seen_readers = std::collections::BTreeSet::new();
        let mut readers = Vec::new();
        for edge in &graph.edges {
            let other = if edge.target == node.id {
                edge.source
            } else if edge.source == node.id {
                edge.target
            } else {
                continue;
            };
            // A reader connected through several edges must list once; stop
            // scanning as soon as the cap is reached.
            if let Some(reader) = node_labels.get(&other.0)
                && !matches!(reader.kind, NodeKind::Repository | NodeKind::Directory)
                && seen_readers.insert(other)
            {
                readers.push(format!("`{}`", md_escape(&reader.label)));
                if readers.len() >= READERS_PER_CONFIG {
                    break;
                }
            }
        }
        if !readers.is_empty() {
            configs_md.push_str(&format!("- Touched by: {}\n", readers.join(", ")));
        }
        configs_md.push('\n');
    }
    let total_config_nodes = graph
        .nodes
        .iter()
        .filter(|node| is_configuration_node(node))
        .count();
    if total_config_nodes > config_nodes.len() {
        configs_md.push_str(&format!(
            "> Truncated: showing {} of {total_config_nodes} config/environment reads.\n",
            config_nodes.len()
        ));
    }
    if config_nodes.is_empty() {
        configs_md.push_str("No config or environment reads were detected.\n");
    }

    // Risks page: warning and error insights.
    let mut risks_md = String::from(
        "[[Home]]\n\n# Risks\n\nWarning- and error-severity findings from graph analysis.\n\n",
    );
    let risky: Vec<_> = insight_report
        .insights
        .iter()
        .filter(|insight| !matches!(insight.severity, InsightSeverity::Info))
        .take(RISK_LIMIT)
        .collect();
    let mut risks_by_kind: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for insight in &risky {
        risks_by_kind
            .entry(insight.kind.as_str())
            .or_default()
            .push(format!(
                "**{}**: {}",
                severity_label(insight.severity),
                md_escape(&insight.message)
            ));
    }
    for (kind, lines) in &risks_by_kind {
        risks_md.push_str(&format!("## {kind}\n\n"));
        for line in lines {
            risks_md.push_str(&format!("- {line}\n"));
        }
        risks_md.push('\n');
    }
    if risky.is_empty() {
        risks_md.push_str("No warning or error findings.\n");
    }
    let total_risky = insight_report
        .insights
        .iter()
        .filter(|insight| !matches!(insight.severity, InsightSeverity::Info))
        .count();
    if total_risky > risky.len() {
        risks_md.push_str(&format!(
            "> Truncated: showing {} of {total_risky} findings.\n",
            risky.len()
        ));
    }

    // Home page.
    let home_md = format!(
        "# {label}\n\nCodeGraph knowledge wiki. Regenerate with `codegraph export-wiki`.\n\n\
         - [[Communities]] — {} subsystems across {} nodes\n\
         - [[Entrypoints]] — {} startup surfaces\n\
         - [[Hotspots]] — {} high-degree nodes ({} architectural hubs)\n\
         - [[Config Flows]] — {total_config_nodes} config/environment reads\n\
         - [[Risks]] — {total_risky} warning/error findings\n",
        community_report.total_communities,
        community_report.total_nodes,
        entrypoint_nodes.len(),
        hotspot_report.total_candidates,
        hotspot_report.total_architectural_hubs,
    );

    let pages = [
        ("Home.md", home_md),
        ("Communities.md", communities_md),
        ("Entrypoints.md", entrypoints_md),
        ("Hotspots.md", hotspots_md),
        ("Config Flows.md", configs_md),
        ("Risks.md", risks_md),
    ];
    let mut files = Vec::new();
    for (name, contents) in &pages {
        let path = output_dir.join(name);
        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
        files.push(name.to_string());
    }

    Ok(WikiExportReport {
        output_dir: output_dir.display().to_string(),
        files,
        communities: community_report.communities.len(),
        entrypoints: entrypoint_nodes.len().min(ENTRYPOINT_LIMIT),
        hotspots: hotspot_report.hotspots.len(),
        config_flows: config_nodes.len(),
        risks: risky.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_indexer::{IndexOptions, scan_project};
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
            "codegraph-wiki-{kind}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn the_entrypoints_page_leads_with_programs() {
        let root = temp_dir("order");
        fs::create_dir_all(root.join(".github/workflows")).unwrap();
        fs::write(
            root.join(".github/workflows/ci.yml"),
            "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo test\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).expect("scan");
        let out = temp_dir("vault");
        export_wiki(&graph, &out, "demo").expect("export");
        let page = fs::read_to_string(out.join("Entrypoints.md")).expect("page");

        let program = page.find("## program").expect("program section");
        // A CI job is where execution starts too, but never the first thing a
        // reader wants to see.
        if let Some(workflow) = page.find("## workflow_job") {
            assert!(program < workflow, "programs come before CI jobs:\n{page}");
        }

        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(out).ok();
    }

    #[test]
    fn the_config_page_holds_configuration() {
        let root = temp_dir("configs");
        fs::create_dir_all(root.join(".github/workflows")).unwrap();
        // Eleven CI steps on this repository pushed every environment
        // variable off the page.
        let mut workflow = String::from(
            "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n",
        );
        for index in 0..12 {
            workflow.push_str(&format!("      - run: echo step {index}\n"));
        }
        fs::write(root.join(".github/workflows/ci.yml"), workflow).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() {\n    let _ = std::env::var(\"DATABASE_URL\");\n}\n",
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).expect("scan");
        let out = temp_dir("vault");
        export_wiki(&graph, &out, "demo").expect("export");
        let page = fs::read_to_string(out.join("Config Flows.md")).expect("page");

        assert!(
            page.contains("`DATABASE_URL` (environment)"),
            "the variable the code reads is missing:\n{page}"
        );
        assert!(
            !page.contains("github run:"),
            "a CI step is not configuration:\n{page}"
        );

        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(out).ok();
    }

    #[test]
    fn exports_interlinked_vault_from_scanned_project() {
        let root = temp_dir("root");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            r#"fn main() {
    // FIXME: handle errors instead of unwrapping
    let url = std::env::var("DATABASE_URL").unwrap();
    connect(&url);
    missing_helper();
}

fn connect(_url: &str) {}
"#,
        )
        .unwrap();
        let graph = scan_project(&root, &IndexOptions::default()).expect("scan");
        let vault = temp_dir("vault");

        let report = export_wiki(&graph, &vault, "demo").expect("wiki export");
        assert_eq!(report.files.len(), 6);
        for file in [
            "Home.md",
            "Communities.md",
            "Entrypoints.md",
            "Hotspots.md",
            "Config Flows.md",
            "Risks.md",
        ] {
            assert!(vault.join(file).is_file(), "missing {file}");
        }

        let home = fs::read_to_string(vault.join("Home.md")).unwrap();
        assert!(home.contains("# demo"));
        for link in [
            "[[Communities]]",
            "[[Entrypoints]]",
            "[[Hotspots]]",
            "[[Config Flows]]",
            "[[Risks]]",
        ] {
            assert!(home.contains(link), "Home must link {link}");
        }

        let entrypoints_page = fs::read_to_string(vault.join("Entrypoints.md")).unwrap();
        assert!(entrypoints_page.starts_with("[[Home]]"), "back-link");
        assert!(entrypoints_page.contains("main"));

        let configs_page = fs::read_to_string(vault.join("Config Flows.md")).unwrap();
        assert!(
            configs_page.contains("DATABASE_URL"),
            "environment read must be listed"
        );
        assert!(
            configs_page.contains("Touched by:"),
            "readers must be linked"
        );

        let risks_page = fs::read_to_string(vault.join("Risks.md")).unwrap();
        assert!(
            risks_page.contains("**warning**") || risks_page.contains("**error**"),
            "unresolved call / FIXME findings must appear: {risks_page}"
        );

        // Regeneration overwrites in place and stays deterministic.
        let second = export_wiki(&graph, &vault, "demo").expect("second export");
        assert_eq!(second.files.len(), 6);
        let home_again = fs::read_to_string(vault.join("Home.md")).unwrap();
        assert_eq!(home, home_again, "regeneration must be byte-stable");

        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(vault).ok();
    }

    #[test]
    fn markdown_labels_are_escaped() {
        assert_eq!(md_escape("a|b [c]"), "a\\|b \\[c\\]");
    }
}

/// Whether this node is configuration a reader came to the page for. A
/// workflow's `run:` step and a SQL statement are `config` nodes too, and
/// on this repository the eleven CI steps pushed the environment
/// variables off the page.
fn is_configuration_node(node: &codegraph_core::Node) -> bool {
    if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
        return false;
    }
    !matches!(
        node.metadata.get("item_kind").map(String::as_str),
        Some("github_actions_run_step" | "gitlab_ci_script" | "app_sql_query" | "sql_index")
    )
}
