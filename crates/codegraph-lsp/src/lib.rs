use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspDiscoveryReport {
    pub servers: Vec<LspServerStatus>,
    pub total_servers: usize,
    pub available_servers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspServerStatus {
    pub id: &'static str,
    pub languages: &'static [&'static str],
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub capabilities: &'static [&'static str],
    pub installed: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LspServerSpec {
    id: &'static str,
    languages: &'static [&'static str],
    command: &'static str,
    args: &'static [&'static str],
    capabilities: &'static [&'static str],
}

const LSP_SERVER_SPECS: &[LspServerSpec] = &[
    LspServerSpec {
        id: "rust-analyzer",
        languages: &["rust"],
        command: "rust-analyzer",
        args: &[],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "gopls",
        languages: &["go"],
        command: "gopls",
        args: &[],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "typescript-language-server",
        languages: &["javascript", "typescript", "tsx"],
        command: "typescript-language-server",
        args: &["--stdio"],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "pyright-langserver",
        languages: &["python"],
        command: "pyright-langserver",
        args: &["--stdio"],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "clangd",
        languages: &["c", "cpp"],
        command: "clangd",
        args: &[],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "intelephense",
        languages: &["php"],
        command: "intelephense",
        args: &["--stdio"],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "bash-language-server",
        languages: &["bash"],
        command: "bash-language-server",
        args: &["start"],
        capabilities: &["document_symbols", "diagnostics"],
    },
];

pub fn discover_lsp_servers() -> LspDiscoveryReport {
    let servers: Vec<_> = LSP_SERVER_SPECS
        .iter()
        .map(|spec| {
            let path = find_executable(spec.command);
            LspServerStatus {
                id: spec.id,
                languages: spec.languages,
                command: spec.command,
                args: spec.args,
                capabilities: spec.capabilities,
                installed: path.is_some(),
                path: path.map(|path| path.display().to_string()),
            }
        })
        .collect();
    let available_servers = servers.iter().filter(|server| server.installed).count();

    LspDiscoveryReport {
        total_servers: servers.len(),
        available_servers,
        servers,
    }
}

pub fn server_specs() -> &'static [&'static str] {
    const IDS: &[&str] = &[
        "rust-analyzer",
        "gopls",
        "typescript-language-server",
        "pyright-langserver",
        "clangd",
        "intelephense",
        "bash-language-server",
    ];
    IDS
}

fn find_executable(command: &str) -> Option<PathBuf> {
    find_executable_with_path(command, env::var_os("PATH").as_deref())
}

fn find_executable_with_path(command: &str, path_var: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    for dir in env::split_paths(path_var) {
        for candidate in executable_candidates(&dir, command) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_candidates(dir: &Path, command: &str) -> Vec<PathBuf> {
    if cfg!(windows) {
        let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        pathext
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| dir.join(format!("{command}{extension}")))
            .chain(std::iter::once(dir.join(command)))
            .collect()
    } else {
        vec![dir.join(command)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn discovery_report_lists_target_language_servers() {
        let report = discover_lsp_servers();
        let ids: Vec<_> = report.servers.iter().map(|server| server.id).collect();

        assert_eq!(report.total_servers, 7);
        assert!(ids.contains(&"rust-analyzer"));
        assert!(ids.contains(&"gopls"));
        assert!(ids.contains(&"typescript-language-server"));
        assert!(ids.contains(&"pyright-langserver"));
        assert!(ids.contains(&"clangd"));
        assert!(ids.contains(&"intelephense"));
        assert!(ids.contains(&"bash-language-server"));
        assert!(report.available_servers <= report.total_servers);
    }

    #[test]
    fn executable_lookup_uses_path_entries() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("rust-analyzer"), "").unwrap();
        let path_var = env::join_paths([dir.as_path()]).unwrap();

        let found = find_executable_with_path("rust-analyzer", Some(path_var.as_os_str()));

        assert_eq!(found, Some(dir.join("rust-analyzer")));
        fs::remove_dir_all(dir).unwrap();
    }

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!(
            "codegraph-lsp-test-{}-{nanos}-{id}",
            std::process::id()
        ))
    }
}
