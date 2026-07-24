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

pub fn install_agent(
    root: &Path,
    platform: AgentPlatform,
    force: bool,
    hooks: bool,
) -> Result<InstallReport> {
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
    if hooks {
        install_hook_snippets(root, platform, force, &mut report)?;
    }
    Ok(report)
}

/// Write assistant hook configuration snippets under `.codegraph/hooks/`.
/// These are copy-paste materials, not an active hook runtime: each file
/// documents where to install it and stays a gentle, non-blocking nudge
/// toward `codegraph ask/query/path/explain` before grep-heavy workflows.
fn install_hook_snippets(
    root: &Path,
    platform: AgentPlatform,
    force: bool,
    report: &mut InstallReport,
) -> Result<()> {
    let hooks_dir = root.join(".codegraph").join("hooks");
    fs::create_dir_all(&hooks_dir)
        .with_context(|| format!("failed to create {}", hooks_dir.display()))?;

    let mut snippets: Vec<(&str, String)> = vec![
        ("README.md", hook_readme()),
        ("pre-search-nudge.sh", generic_hook_script()),
    ];
    if matches!(platform, AgentPlatform::Claude | AgentPlatform::All) {
        snippets.push(("claude-settings-snippet.json", claude_hook_snippet()));
    }

    for (name, content) in snippets {
        let path = hooks_dir.join(name);
        let display = format!(".codegraph/hooks/{name}");
        match fs::read_to_string(&path) {
            Ok(existing) if existing == content => report.unchanged.push(display),
            Ok(_) if !force => report.skipped.push(InstallSkip {
                path: display,
                reason: "existing file differs; re-run with --force to overwrite".to_string(),
            }),
            Ok(_) => {
                fs::write(&path, &content)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                report.updated.push(display);
            }
            Err(_) => {
                fs::write(&path, &content)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                report.created.push(display);
            }
        }
    }
    Ok(())
}

fn claude_hook_snippet() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Grep|Glob",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "echo 'CodeGraph tip: this repository has a code graph — try `codegraph ask \"...\" .`, `codegraph query ...`, or the codegraph MCP tools before broad file searches (see CLAUDE.md).' >&2"
                        }
                    ]
                }
            ]
        }
    }))
    .expect("static hook snippet serializes")
        + "\n"
}

fn generic_hook_script() -> String {
    "#!/bin/sh\n\
# CodeGraph pre-search nudge for assistants with command-hook support.\n\
# Prints a reminder to standard error and never blocks the original tool.\n\
echo \"CodeGraph tip: query the code graph first — codegraph ask \\\"question\\\" . | codegraph query '...' . | codegraph explain-edge --edge-index N .\" >&2\n\
exit 0\n"
        .to_string()
}

fn hook_readme() -> String {
    "# CodeGraph assistant hook snippets\n\
\n\
Generated by `codegraph install-agent --hooks`. These are configuration\n\
snippets to copy into your assistant's hook settings — CodeGraph does not\n\
run any hook itself. Both variants only print a reminder and never block.\n\
\n\
## Claude Code\n\
\n\
Merge `claude-settings-snippet.json` into `.claude/settings.json`. The\n\
`PreToolUse` hook prints a nudge whenever the agent reaches for Grep/Glob,\n\
suggesting `codegraph ask/query` and the MCP tools first. For a strict\n\
variant that blocks the search and returns the tip to the agent, append\n\
`; exit 2` to the command — opt in deliberately, it changes agent behavior.\n\
\n\
## Other assistants\n\
\n\
`pre-search-nudge.sh` is a portable pre-command hook: wire it into any\n\
runner that can execute a shell command before file-search tools.\n"
        .to_string()
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
`impact`, `report`, `ask`, `source_search`, `refactor_context`, and the\n\
`memory_save`/`memory_list`/`memory_reflect` investigation-memory tools with\n\
the same graph answers.\n\
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
    fn hooks_flag_writes_snippets_idempotently() {
        let root = temp_root();
        let report = install_agent(&root, AgentPlatform::All, false, true).expect("install");
        for name in [
            ".codegraph/hooks/README.md",
            ".codegraph/hooks/pre-search-nudge.sh",
            ".codegraph/hooks/claude-settings-snippet.json",
        ] {
            assert!(
                report.created.iter().any(|entry| entry == name),
                "expected {name} to be created: {report:?}"
            );
        }

        let snippet = std::fs::read_to_string(
            root.join(".codegraph")
                .join("hooks")
                .join("claude-settings-snippet.json"),
        )
        .expect("snippet readable");
        let parsed: serde_json::Value =
            serde_json::from_str(&snippet).expect("snippet is valid JSON");
        assert_eq!(
            parsed["hooks"]["PreToolUse"][0]["matcher"],
            serde_json::json!("Grep|Glob")
        );
        assert!(
            parsed["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
                .as_str()
                .expect("command string")
                .contains("codegraph ask"),
        );

        let second = install_agent(&root, AgentPlatform::All, false, true).expect("re-install");
        assert!(
            second
                .unchanged
                .iter()
                .any(|entry| entry == ".codegraph/hooks/claude-settings-snippet.json"),
            "second run should leave snippets unchanged: {second:?}"
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fresh_install_creates_all_artifacts() {
        let root = temp_root();
        let report = install_agent(&root, AgentPlatform::All, false, false).expect("install");

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
        install_agent(&root, AgentPlatform::Claude, false, false).expect("first install");

        // Simulate an outdated guidance block plus user content around it.
        let stale = format!(
            "# My project\n\n{GUIDANCE_START}\nold guidance\n{GUIDANCE_END}\n\n## Notes\nkeep me\n"
        );
        fs::write(root.join("CLAUDE.md"), stale).unwrap();

        let report =
            install_agent(&root, AgentPlatform::Claude, false, false).expect("second install");
        assert!(report.updated.contains(&"CLAUDE.md".to_string()));
        assert!(report.unchanged.contains(&".mcp.json".to_string()));
        let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
        assert!(claude.starts_with("# My project"));
        assert!(claude.contains("keep me"));
        assert!(!claude.contains("old guidance"));
        assert_eq!(claude.matches(GUIDANCE_START).count(), 1);

        let third =
            install_agent(&root, AgentPlatform::Claude, false, false).expect("third install");
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

        let report = install_agent(&root, AgentPlatform::Generic, false, false).expect("install");
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].reason.contains("--force"));
        let untouched: Value =
            serde_json::from_str(&fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(untouched["mcpServers"]["codegraph"]["command"], "legacy");

        let forced =
            install_agent(&root, AgentPlatform::Generic, true, false).expect("forced install");
        assert!(forced.updated.contains(&".mcp.json".to_string()));
        let merged: Value =
            serde_json::from_str(&fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(merged["mcpServers"]["other"]["command"], "other-tool");
        assert_eq!(merged["mcpServers"]["codegraph"]["command"], "codegraph");
        fs::remove_dir_all(root).ok();
    }
}
