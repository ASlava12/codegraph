//! Rationale comments, ORM table references, migration-directory
//! references, and MCP configuration facts.

use std::collections::BTreeMap;
use std::path::Path;

use codegraph_core::{Confidence, EdgeKind, NodeId, NodeKind};
use codegraph_parser::Language;

#[allow(unused_imports)]
use crate::*;

pub(crate) fn index_rationale_comments(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Option<Language>,
    source: &str,
) {
    for comment in rationale_comments(label, language, source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "rationale_comment".to_string());
        metadata.insert("source".to_string(), "source_comment".to_string());
        metadata.insert("rationale_kind".to_string(), comment.kind.to_string());
        metadata.insert("line".to_string(), comment.line.to_string());
        metadata.insert("text".to_string(), comment.text.clone());
        if let Some(language) = language {
            metadata.insert("language".to_string(), language.to_string());
        }
        let rationale_id = context.graph.add_node_with_metadata(
            NodeKind::Unknown,
            comment.label,
            Some(line_span(label, source, comment.line)),
            metadata,
        );
        context.graph.add_edge_with_metadata(
            file_id,
            rationale_id,
            EdgeKind::Contains,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "rationale_comment".to_string())]),
        );
    }
}

/// ORM table mappings in application code: each pattern names the table a
/// class/struct persists to across common frameworks.
pub(crate) fn index_orm_table_refs(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) {
    if is_sql_file(Path::new(label)) {
        return;
    }
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let table_and_pattern = orm_table_in_line(line);
        if let Some((table, pattern)) = table_and_pattern {
            context.pending_orm_table_refs.push(PendingOrmTableRef {
                file: file_id,
                table,
                pattern,
                line: line_number,
            });
        }
    }
}

pub(crate) fn orm_table_in_line(line: &str) -> Option<(String, &'static str)> {
    let checks: [(&str, &'static str); 9] = [
        ("__tablename__", "sqlalchemy_tablename"),
        ("db_table", "django_db_table"),
        ("@Entity(", "typeorm_entity"),
        ("tableName:", "sequelize_table_name"),
        ("@@map(", "prisma_map"),
        ("$table", "laravel_table"),
        ("diesel(table_name", "diesel_table_name"),
        ("ORM\\Table(name", "doctrine_table"),
        ("gorm:\"table:", "gorm_table_tag"),
    ];
    for (needle, pattern) in checks {
        let Some(position) = line.find(needle) else {
            continue;
        };
        // `$table` must be an assignment, not a local usage.
        if pattern == "laravel_table" && !line[position..].contains('=') {
            continue;
        }
        let rest = &line[position + needle.len()..];
        let table = if pattern == "gorm_table_tag" {
            rest.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        } else {
            first_quoted_value(rest)?
        };
        if !table.is_empty()
            && table
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        {
            return Some((table, pattern));
        }
    }
    // Go GORM convention: func (X) TableName() string { return "users" }
    if line.contains("TableName()")
        && line.contains("func ")
        && let Some(table) = first_quoted_value(line)
    {
        return Some((table, "gorm_table_name"));
    }
    None
}

/// Migrations-directory references: migration runner macros/calls in code
/// plus migration locations in database config files.
pub(crate) fn index_migration_dir_refs(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) {
    let file_name = label.rsplit('/').next().unwrap_or(label);
    let is_db_config = matches!(
        file_name,
        "alembic.ini" | "flyway.conf" | "phinx.php" | "knexfile.js" | "knexfile.ts"
    );
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let reference = if line.contains("migrate!(") {
            // sqlx::migrate!("./migrations") — default path when empty.
            Some(
                first_quoted_value_after(line, "migrate!(")
                    .unwrap_or_else(|| "migrations".to_string()),
            )
        } else if is_db_config && line.trim_start().starts_with("script_location") {
            line.split_once('=')
                .map(|(_, value)| value.trim().to_string())
        } else if is_db_config && line.contains("flyway.locations") {
            line.split_once('=')
                .map(|(_, value)| value.trim().trim_start_matches("filesystem:").to_string())
        } else if is_db_config && (line.contains("directory:") || line.contains("'directory'")) {
            first_quoted_value(line)
        } else {
            None
        };
        if let Some(dir) = reference {
            let normalized = normalize_path(dir.trim_start_matches("./"));
            if !normalized.is_empty() {
                context
                    .pending_migration_dir_refs
                    .push(PendingMigrationDirRef {
                        file: file_id,
                        dir: normalized,
                        source_kind: if is_db_config { "db_config" } else { "code" },
                        line: line_number,
                    });
            }
        }
    }
}

/// MCP configuration files declare the assistant tool servers a repository
/// uses. Each server becomes a dependency fact with transport, command, and
/// args; path-like commands/args are matched to scanned files afterwards.
pub(crate) fn index_mcp_config(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) {
    let file_name = label.rsplit('/').next().unwrap_or(label);
    if !matches!(
        file_name,
        ".mcp.json" | "mcp.json" | "mcp_servers.json" | "claude_desktop_config.json"
    ) {
        return;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return;
    };
    let servers = value
        .get("mcpServers")
        .or_else(|| value.get("servers"))
        .and_then(|servers| servers.as_object());
    let Some(servers) = servers else {
        return;
    };

    add_file_metadata(&mut context.graph, file_id, "item_kind", "mcp_config");
    for (name, spec) in servers {
        let command = spec.get("command").and_then(|value| value.as_str());
        let url = spec.get("url").and_then(|value| value.as_str());
        let args: Vec<String> = spec
            .get("args")
            .and_then(|value| value.as_array())
            .map(|args| {
                args.iter()
                    .filter_map(|arg| arg.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let mut metadata = BTreeMap::from([
            ("item_kind".to_string(), "mcp_server".to_string()),
            ("server".to_string(), name.clone()),
            ("source".to_string(), "mcp_config".to_string()),
            (
                "transport".to_string(),
                if url.is_some() { "http" } else { "stdio" }.to_string(),
            ),
        ]);
        if let Some(command) = command {
            metadata.insert("command".to_string(), command.to_string());
        }
        if let Some(url) = url {
            metadata.insert("url".to_string(), url.to_string());
        }
        if !args.is_empty() {
            metadata.insert("args".to_string(), args.join(" "));
        }
        let server_id = context.graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            format!("mcp server:{name}"),
            None,
            metadata,
        );
        add_edge_once_with_metadata(
            context,
            file_id,
            server_id,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([
                ("relation".to_string(), "mcp_server".to_string()),
                ("source".to_string(), "mcp_config".to_string()),
            ]),
        );

        for candidate in command.into_iter().map(ToString::to_string).chain(args) {
            let looks_like_path = candidate.contains('/')
                || [".py", ".js", ".ts", ".sh", ".rb"]
                    .iter()
                    .any(|extension| candidate.ends_with(extension));
            if looks_like_path && !candidate.contains("://") {
                context.pending_mcp_local_refs.push(PendingMcpLocalRef {
                    server: server_id,
                    candidate: normalize_path(candidate.trim_start_matches("./")),
                });
            }
        }
    }
}

pub(crate) fn is_sql_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sql"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RationaleComment {
    kind: &'static str,
    label: String,
    text: String,
    line: u32,
}

pub(crate) fn rationale_comments(
    label: &str,
    language: Option<Language>,
    source: &str,
) -> Vec<RationaleComment> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            rationale_comment_text(language, line)
                .and_then(|text| rationale_marker(text))
                .map(|(kind, text)| {
                    let text = normalize_rationale_text(text);
                    RationaleComment {
                        kind,
                        label: format!("{}: {}", kind.to_ascii_uppercase(), rationale_label(&text)),
                        text,
                        line: index as u32 + 1,
                    }
                })
        })
        .filter(|comment| !comment.text.is_empty() && comment.text != label)
        .collect()
}

pub(crate) fn rationale_comment_text(language: Option<Language>, line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    match language {
        Some(Language::Python | Language::Bash | Language::Ruby | Language::Elixir) => {
            trimmed.strip_prefix('#')
        }
        Some(Language::Lua | Language::Haskell) => trimmed.strip_prefix("--"),
        Some(Language::Julia | Language::Erlang | Language::Nix | Language::R) => trimmed
            .strip_prefix('%')
            .or_else(|| trimmed.strip_prefix('#')),
        Some(Language::OCaml) => trimmed
            .strip_prefix("(*")
            .map(|text| text.trim_end_matches("*)").trim()),
        Some(
            Language::Rust
            | Language::JavaScript
            | Language::TypeScript
            | Language::Tsx
            | Language::Go
            | Language::C
            | Language::Cpp
            | Language::Dart
            | Language::Java
            | Language::CSharp
            | Language::Kotlin
            | Language::Swift
            | Language::Scala
            | Language::Zig,
        ) => trimmed.strip_prefix("//").or_else(|| {
            trimmed
                .strip_prefix("/*")
                .map(|text| text.trim_end_matches("*/").trim())
        }),
        Some(Language::Php) => trimmed
            .strip_prefix("//")
            .or_else(|| trimmed.strip_prefix('#'))
            .or_else(|| {
                trimmed
                    .strip_prefix("/*")
                    .map(|text| text.trim_end_matches("*/").trim())
            }),
        None => trimmed
            .strip_prefix("//")
            .or_else(|| trimmed.strip_prefix('#'))
            .or_else(|| {
                trimmed
                    .strip_prefix("/*")
                    .map(|text| text.trim_end_matches("*/").trim())
            }),
    }
}

pub(crate) fn rationale_marker(text: &str) -> Option<(&'static str, &str)> {
    let trimmed = text.trim_start();
    let normalized = trimmed.to_ascii_uppercase();
    for marker in [
        "SECURITY", "FIXME", "TODO", "WHY", "NOTE", "HACK", "BUG", "XXX",
    ] {
        if normalized == marker {
            return Some((rationale_kind(marker), ""));
        }
        for separator in [":", "-", " "] {
            let prefix = format!("{marker}{separator}");
            if normalized.starts_with(&prefix) {
                return Some((rationale_kind(marker), trimmed[prefix.len()..].trim()));
            }
        }
    }
    None
}

pub(crate) fn rationale_kind(marker: &str) -> &'static str {
    match marker {
        "SECURITY" => "security",
        "FIXME" => "fixme",
        "TODO" => "todo",
        "WHY" => "why",
        "NOTE" => "note",
        "HACK" => "hack",
        "BUG" => "bug",
        "XXX" => "xxx",
        _ => "note",
    }
}

pub(crate) fn normalize_rationale_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn rationale_label(text: &str) -> String {
    let mut label = text.chars().take(96).collect::<String>();
    if text.chars().count() > 96 {
        label.push_str("...");
    }
    label
}
