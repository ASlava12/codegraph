//! Project-scoped agent installation: MCP registration and guidance snippets.
//!
//! `codegraph install-agent` writes idempotent artifacts into a repository so
//! coding assistants discover the graph before broad file reads: a
//! `.mcp.json` server entry plus marker-delimited guidance blocks in
//! `CLAUDE.md` and/or `AGENTS.md`. Marker blocks are replaced in place on
//! reruns; existing foreign `.mcp.json` entries are preserved.

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub const GUIDANCE_START: &str = "<!-- codegraph:start -->";
pub const GUIDANCE_END: &str = "<!-- codegraph:end -->";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AgentPlatform {
    /// Claude Code and compatible assistants (CLAUDE.md + .mcp.json).
    Claude,
    /// Codex-style assistants (AGENTS.md + .mcp.json).
    Codex,
    /// Generic agents (AGENTS.md + .mcp.json).
    Generic,
    /// All supported guidance files plus .mcp.json.
    All,
}

#[derive(Debug, Default, Serialize)]
pub struct InstallReport {
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub skipped: Vec<InstallSkip>,
}

#[derive(Debug, Serialize)]
pub struct InstallSkip {
    pub path: String,
    pub reason: String,
}

pub fn install_agent(root: &Path, platform: AgentPlatform, force: bool) -> Result<InstallReport> {
    let mut report = InstallReport::default();
    install_mcp_config(root, force, &mut report)?;
    let guidance_files: &[&str] = match platform {
        AgentPlatform::Claude => &["CLAUDE.md"],
        AgentPlatform::Codex | AgentPlatform::Generic => &["AGENTS.md"],
        AgentPlatform::All => &["CLAUDE.md", "AGENTS.md"],
    };
    for file_name in guidance_files {
        install_guidance(root, file_name, &mut report)?;
    }
    Ok(report)
}

fn install_mcp_config(root: &Path, force: bool, report: &mut InstallReport) -> Result<()> {
    let path = root.join(".mcp.json");
    let display = ".mcp.json".to_string();
    let server_entry = json!({
        "command": "codegraph",
        "args": ["mcp", "."],
    });

    let mut config: Value = if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("{} is not valid JSON", path.display()))?
    } else {
        json!({})
    };
    if !config.is_object() {
        report.skipped.push(InstallSkip {
            path: display,
            reason: "existing .mcp.json is not a JSON object".to_string(),
        });
        return Ok(());
    }
    let existed = path.exists();
    let servers = config
        .as_object_mut()
        .expect("checked object")
        .entry("mcpServers")
        .or_insert_with(|| json!({}));
    let Some(servers) = servers.as_object_mut() else {
        report.skipped.push(InstallSkip {
            path: display,
            reason: "existing mcpServers is not a JSON object".to_string(),
        });
        return Ok(());
    };

    match servers.get("codegraph") {
        Some(current) if *current == server_entry => {
            report.unchanged.push(display);
            return Ok(());
        }
        Some(_) if !force => {
            report.skipped.push(InstallSkip {
                path: display,
                reason: "codegraph entry already exists with different settings; rerun with --force to overwrite".to_string(),
            });
            return Ok(());
        }
        _ => {}
    }
    servers.insert("codegraph".to_string(), server_entry);

    let serialized = format!("{}\n", serde_json::to_string_pretty(&config)?);
    fs::write(&path, serialized).with_context(|| format!("failed to write {}", path.display()))?;
    if existed {
        report.updated.push(display);
    } else {
        report.created.push(display);
    }
    Ok(())
}

fn install_guidance(root: &Path, file_name: &str, report: &mut InstallReport) -> Result<()> {
    let path = root.join(file_name);
    let block = guidance_block();
    if !path.exists() {
        fs::write(&path, format!("{block}\n"))
            .with_context(|| format!("failed to write {}", path.display()))?;
        report.created.push(file_name.to_string());
        return Ok(());
    }

    let existing =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let next = match (existing.find(GUIDANCE_START), existing.find(GUIDANCE_END)) {
        (Some(start), Some(end)) if end > start => {
            let mut next = String::new();
            next.push_str(&existing[..start]);
            next.push_str(&block);
            next.push_str(&existing[end + GUIDANCE_END.len()..]);
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
        report.unchanged.push(file_name.to_string());
        return Ok(());
    }
    fs::write(&path, next).with_context(|| format!("failed to write {}", path.display()))?;
    report.updated.push(file_name.to_string());
    Ok(())
}

fn guidance_block() -> String {
    format!(
        "{GUIDANCE_START}\n\
## CodeGraph\n\
\n\
This repository is indexed by CodeGraph: a typed code knowledge graph with\n\
confidence and provenance on every fact. Query the graph before broad file\n\
reads or grep sweeps — it answers structural questions in one bounded call.\n\
\n\
- Ask in natural language: `codegraph ask \"Where is DATABASE_URL read?\" .`\n\
- Query slices: `codegraph query 'nodes kind:function label:main' .`\n\
- Follow an execution flow: `codegraph journey --from <entrypoint> --to <target> .`\n\
- Assess a change before making it: `codegraph impact <target> .`\n\
- Get full refactor context in one call: `codegraph refactor-context <target> .`\n\
- Project overview with risks: `codegraph report . --format markdown`\n\
\n\
Over MCP, the `codegraph` server (see `.mcp.json`) exposes `query_graph`,\n\
`get_node_card`, `get_neighbors`, `shortest_path`, `workflow`, `insights`,\n\
`impact`, and `report` tools with the same graph answers.\n\
{GUIDANCE_END}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .subsec_nanos();
        let counter = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "codegraph-install-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    #[test]
    fn fresh_install_creates_all_artifacts() {
        let root = temp_root();
        let report = install_agent(&root, AgentPlatform::All, false).expect("install");

        assert_eq!(report.created.len(), 3);
        assert!(report.skipped.is_empty());
        let mcp: Value =
            serde_json::from_str(&fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["codegraph"]["command"], "codegraph");
        let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
        assert!(claude.contains(GUIDANCE_START));
        assert!(claude.contains("codegraph refactor-context"));
        assert!(root.join("AGENTS.md").exists());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn rerun_is_idempotent_and_updates_marker_blocks() {
        let root = temp_root();
        install_agent(&root, AgentPlatform::Claude, false).expect("first install");

        // Simulate an outdated guidance block plus user content around it.
        let stale = format!(
            "# My project\n\n{GUIDANCE_START}\nold guidance\n{GUIDANCE_END}\n\n## Notes\nkeep me\n"
        );
        fs::write(root.join("CLAUDE.md"), stale).unwrap();

        let report = install_agent(&root, AgentPlatform::Claude, false).expect("second install");
        assert!(report.updated.contains(&"CLAUDE.md".to_string()));
        assert!(report.unchanged.contains(&".mcp.json".to_string()));
        let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
        assert!(claude.starts_with("# My project"));
        assert!(claude.contains("keep me"));
        assert!(!claude.contains("old guidance"));
        assert_eq!(claude.matches(GUIDANCE_START).count(), 1);

        let third = install_agent(&root, AgentPlatform::Claude, false).expect("third install");
        assert!(third.created.is_empty());
        assert!(third.updated.is_empty());
        assert_eq!(third.unchanged.len(), 2);
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn existing_mcp_servers_are_preserved_and_conflicts_need_force() {
        let root = temp_root();
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"other":{"command":"other-tool"},"codegraph":{"command":"legacy"}}}"#,
        )
        .unwrap();

        let report = install_agent(&root, AgentPlatform::Generic, false).expect("install");
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].reason.contains("--force"));
        let untouched: Value =
            serde_json::from_str(&fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(untouched["mcpServers"]["codegraph"]["command"], "legacy");

        let forced = install_agent(&root, AgentPlatform::Generic, true).expect("forced install");
        assert!(forced.updated.contains(&".mcp.json".to_string()));
        let merged: Value =
            serde_json::from_str(&fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(merged["mcpServers"]["other"]["command"], "other-tool");
        assert_eq!(merged["mcpServers"]["codegraph"]["command"], "codegraph");
        fs::remove_dir_all(root).ok();
    }
}
