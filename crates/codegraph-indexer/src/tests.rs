//! Unit tests for the indexer crate, driven by temporary project
//! fixtures on disk.

use super::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind};
use codegraph_parser::{Language, ParsedItemKind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

#[test]
fn scan_project_skips_default_ignored_directories() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("target")).unwrap();
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::create_dir_all(root.join("graphify-out")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("target").join("debug.log"), "noise\n").unwrap();
    fs::write(root.join(".codegraph").join("graph.json"), "{}\n").unwrap();
    fs::write(root.join("graphify-out").join("graph.json"), "{}\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

    assert!(labels.contains(&"src/main.rs"));
    assert!(!labels.contains(&"target"));
    assert!(!labels.contains(&"target/debug.log"));
    assert!(!labels.contains(&".codegraph"));
    assert!(!labels.contains(&".codegraph/graph.json"));
    assert!(!labels.contains(&"graphify-out"));
    assert!(!labels.contains(&"graphify-out/graph.json"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_paths_indexes_only_selected_files() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("src").join("other.rs"), "pub fn other() {}\n").unwrap();

    let paths = BTreeSet::from(["src/main.rs".to_string()]);
    let graph = scan_project_paths(&root, &IndexOptions::default(), &paths).unwrap();
    let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

    assert!(labels.contains(&"src"));
    assert!(labels.contains(&"src/main.rs"));
    assert!(labels.contains(&"main"));
    assert!(!labels.contains(&"src/other.rs"));
    assert!(!labels.contains(&"other"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn configured_index_options_loads_project_scan_config() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::write(
            root.join(".codegraph").join("config.toml"),
            "[scan]\nmax_file_size = 7\nextra_ignored_names = [\"generated\"]\nextra_ignored_globs = [\"fixtures/**\"]\ninclude_hidden = true\n",
        )
        .unwrap();

    let options = configured_index_options(&root, &IndexOptionOverrides::default()).unwrap();

    assert_eq!(options.max_file_size, 7);
    assert!(options.include_hidden);
    assert!(options.ignored_names.contains("generated"));
    assert!(options.ignored_names.contains("target"));
    assert!(options.ignored_globs.contains("fixtures/**"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn configured_index_options_rejects_non_string_ignored_entry() {
    // A non-string array entry must surface as a config error, not be silently
    // filtered out (the filter-before-collect bug swallowed the Err).
    let root = temp_project_root();
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::write(
        root.join(".codegraph").join("config.toml"),
        "[scan]\nignored_names = [\"target\", 42]\n",
    )
    .unwrap();

    let error = configured_index_options(&root, &IndexOptionOverrides::default())
        .expect_err("non-string entry is rejected");
    assert!(
        error.to_string().contains("entries must be strings"),
        "unexpected error: {error}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_skips_configured_ignored_globs() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("generated")).unwrap();
    fs::create_dir_all(root.join("src").join("domain")).unwrap();
    fs::write(
        root.join("src").join("generated").join("skip.rs"),
        "fn skip() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("domain").join("keep.rs"),
        "fn keep() {}\n",
    )
    .unwrap();

    let graph = scan_project(
        &root,
        &IndexOptions {
            ignored_globs: BTreeSet::from(["src/generated/**".to_string()]),
            ..IndexOptions::default()
        },
    )
    .unwrap();
    let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

    assert!(labels.contains(&"src/domain/keep.rs"));
    assert!(!labels.contains(&"src/generated"));
    assert!(!labels.contains(&"src/generated/skip.rs"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_coverage_reports_policy_and_size_skips() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("generated")).unwrap();
    fs::create_dir_all(root.join("src").join("domain")).unwrap();
    fs::create_dir_all(root.join("target")).unwrap();
    fs::write(
        root.join("src").join("domain").join("keep.rs"),
        "fn k(){}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("domain").join("huge.rs"),
        "fn huge_function_name() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("generated").join("skip.rs"),
        "fn skip() {}\n",
    )
    .unwrap();
    fs::write(root.join("target").join("skip.rs"), "fn target() {}\n").unwrap();

    let report = scan_coverage(
        &root,
        &IndexOptions {
            max_file_size: 12,
            ignored_globs: BTreeSet::from(["src/generated/**".to_string()]),
            ..IndexOptions::default()
        },
    )
    .unwrap();

    assert_eq!(report.indexed_files, 1);
    assert_eq!(report.skipped_large_files, 1);
    assert_eq!(report.skipped_ignored_name_entries, 1);
    assert_eq!(report.skipped_ignored_glob_entries, 1);
    assert_eq!(report.skipped_policy_entries, 2);
    assert_eq!(report.languages.get("rust"), Some(&1));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn configured_index_options_allows_cli_budget_override() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::write(
        root.join(".codegraph").join("config.toml"),
        "[scan]\nmax_file_size = 7\n",
    )
    .unwrap();

    let options = configured_index_options(
        &root,
        &IndexOptionOverrides {
            max_file_size: Some(42),
            ..IndexOptionOverrides::default()
        },
    )
    .unwrap();

    assert_eq!(options.max_file_size, 42);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_reports_large_source_files_as_skipped() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("huge.rs"), "fn main() {}\n").unwrap();
    fs::write(root.join("large.bin"), "not source but also large\n").unwrap();

    let graph = scan_project(
        &root,
        &IndexOptions {
            max_file_size: 4,
            ..IndexOptions::default()
        },
    )
    .unwrap();

    let skipped = graph
        .nodes
        .iter()
        .find(|node| node.label == "src/huge.rs")
        .expect("large source file should remain visible");
    assert_eq!(skipped.kind, NodeKind::File);
    assert_eq!(
        skipped.metadata.get("skipped").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        skipped.metadata.get("skipped_reason").map(String::as_str),
        Some("max_file_size")
    );
    assert_eq!(
        skipped
            .metadata
            .get("max_file_size_bytes")
            .map(String::as_str),
        Some("4")
    );
    assert!(
        graph.nodes.iter().all(|node| node.label != "large.bin"),
        "large non-source assets should not flood the graph"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_tree_sitter_symbols() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "use std::fs;\nstruct App;\nfn main() {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

    assert!(labels.contains(&"src/main.rs"));
    assert!(labels.contains(&"main"));
    assert!(labels.contains(&"App"));
    assert!(labels.contains(&"use std::fs;"));
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Entrypoint)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_deduplicates_unresolved_call_placeholders() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
            root.join("src").join("main.rs"),
            "fn main() {\n    custom_helper(1);\n    let text = format!(\"x\");\n    let value = Some(text);\n    drop(value);\n}\nfn other() {\n    custom_helper(2);\n    let more = format!(\"y\");\n    drop(more);\n}\n",
        )
        .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    let placeholders = |label: &str| -> Vec<_> {
        graph
            .nodes
            .iter()
            .filter(|node| {
                node.label == label
                    && node
                        .metadata
                        .get("item_kind")
                        .is_some_and(|value| value == "call")
            })
            .collect()
    };

    let helper_nodes = placeholders("custom_helper");
    assert_eq!(
        helper_nodes.len(),
        1,
        "two call sites must share one placeholder node"
    );
    assert_eq!(
        helper_nodes[0]
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("unresolved")
    );
    let helper_id = helper_nodes[0].id;
    let callers: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.target == helper_id && edge.kind == EdgeKind::Calls)
        .map(|edge| edge.source)
        .collect();
    assert_eq!(callers.len(), 2, "each caller keeps its own call edge");

    let format_nodes = placeholders("format");
    assert_eq!(format_nodes.len(), 1);
    assert_eq!(
        format_nodes[0]
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("builtin")
    );
    let some_nodes = placeholders("Some");
    assert_eq!(some_nodes.len(), 1);
    assert_eq!(
        some_nodes[0].metadata.get("resolution").map(String::as_str),
        Some("builtin")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_links_markdown_docs_to_code_nodes() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("docs").join("adr")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "mod config;\nfn main() { load_config(); }\nfn load_config() {}\n",
    )
    .unwrap();
    fs::write(root.join("src").join("config.rs"), "pub fn read() {}\n").unwrap();
    fs::write(
            root.join("docs").join("adr").join("0001-runtime.md"),
            "# ADR 0001: Runtime Flow\n\nThe startup path begins in [main](../../src/main.rs).\nIt keeps configuration work in `src/config.rs` and calls `load_config`.\n",
        )
        .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let doc = node_id(&graph, NodeKind::File, "docs/adr/0001-runtime.md");
    let section = node_id(
        &graph,
        NodeKind::Module,
        "docs/adr/0001-runtime.md#ADR 0001: Runtime Flow",
    );
    let main_file = node_id(&graph, NodeKind::File, "src/main.rs");
    let config_file = node_id(&graph, NodeKind::File, "src/config.rs");
    let load_config = function_id_in_file(&graph, "load_config", "src/main.rs");

    let doc_node = graph
        .nodes
        .iter()
        .find(|node| node.id == doc)
        .expect("missing doc node");
    assert_eq!(
        doc_node.metadata.get("language").map(String::as_str),
        Some("markdown")
    );
    assert_eq!(
        doc_node.metadata.get("document_kind").map(String::as_str),
        Some("adr")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == doc
            && edge.target == section
            && edge.kind == EdgeKind::Contains
            && edge.confidence == Confidence::Exact
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "document_section")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == section
            && edge.target == main_file
            && edge.kind == EdgeKind::References
            && edge.confidence == Confidence::Exact
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "markdown_link")
            && edge
                .metadata
                .get("resolved_path")
                .is_some_and(|value| value == "src/main.rs")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == section
            && edge.target == config_file
            && edge.kind == EdgeKind::References
            && edge.confidence == Confidence::Exact
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "markdown_code_path")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == section
            && edge.target == load_config
            && edge.kind == EdgeKind::References
            && edge.confidence == Confidence::Heuristic
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "markdown_symbol_reference")
    }));

    let coverage = scan_coverage(&root, &IndexOptions::default()).unwrap();
    assert_eq!(coverage.languages.get("markdown"), Some(&1));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_indexes_markdown_front_matter_wikilinks_and_backlinks() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {}\nfn helper() {}\n",
    )
    .unwrap();
    fs::write(
            root.join("docs").join("runtime.md"),
            "---\ntitle: Runtime Guide\nowner: platform-team\nstatus: approved\ntags: runtime, startup\n---\n\n# Runtime\n\nStartup lives in [main](../src/main.rs#L1-L2).\nSee also [[architecture|the architecture notes]].\n",
        )
        .unwrap();
    fs::write(
        root.join("docs").join("architecture.md"),
        "# Architecture\n\nEntry point: [main.rs](../src/main.rs).\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let runtime_doc = node_id(&graph, NodeKind::File, "docs/runtime.md");
    let architecture_doc = node_id(&graph, NodeKind::File, "docs/architecture.md");
    let main_file = node_id(&graph, NodeKind::File, "src/main.rs");

    let doc_node = graph
        .nodes
        .iter()
        .find(|node| node.id == runtime_doc)
        .expect("missing runtime doc node");
    assert_eq!(
        doc_node.metadata.get("doc_title").map(String::as_str),
        Some("Runtime Guide")
    );
    assert_eq!(
        doc_node.metadata.get("doc_owner").map(String::as_str),
        Some("platform-team")
    );
    assert_eq!(
        doc_node.metadata.get("doc_status").map(String::as_str),
        Some("approved")
    );
    assert_eq!(
        doc_node.metadata.get("doc_tags").map(String::as_str),
        Some("runtime, startup")
    );
    // Front matter keys must not leak as document sections.
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.label.contains("title: Runtime Guide"))
    );

    assert!(graph.edges.iter().any(|edge| {
        edge.target == main_file
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "markdown_link")
            && edge
                .metadata
                .get("line_ref")
                .is_some_and(|value| value == "L1-L2")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.target == architecture_doc
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "markdown_wikilink")
            && edge
                .metadata
                .get("text")
                .is_some_and(|value| value == "the architecture notes")
    }));

    let main_node = graph
        .nodes
        .iter()
        .find(|node| node.id == main_file)
        .expect("missing main file node");
    assert_eq!(
        main_node.metadata.get("doc_backlinks").map(String::as_str),
        Some("2")
    );
    let architecture_node = graph
        .nodes
        .iter()
        .find(|node| node.id == architecture_doc)
        .expect("missing architecture doc node");
    assert_eq!(
        architecture_node
            .metadata
            .get("doc_backlinks")
            .map(String::as_str),
        Some("1")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_indexes_sql_schema_facts() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("db").join("migrations")).unwrap();
    fs::write(
        root.join("db").join("migrations").join("001_schema.sql"),
        r#"
CREATE TABLE organizations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    org_id INTEGER NOT NULL REFERENCES organizations(id),
    email TEXT NOT NULL,
    CONSTRAINT users_org_fk FOREIGN KEY (org_id) REFERENCES organizations(id)
);

CREATE UNIQUE INDEX idx_users_email ON users (email);
CREATE VIEW active_users AS SELECT id, email FROM users;
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let sql_file = node_id(&graph, NodeKind::File, "db/migrations/001_schema.sql");
    let organizations = node_id(&graph, NodeKind::Type, "sql table:organizations");
    let users = node_id(&graph, NodeKind::Type, "sql table:users");
    let org_id = node_id(&graph, NodeKind::Config, "sql column:users.org_id");
    let email = node_id(&graph, NodeKind::Config, "sql column:users.email");
    let org_pk = node_id(&graph, NodeKind::Config, "sql column:organizations.id");
    let index = node_id(&graph, NodeKind::Config, "sql index:idx_users_email");
    let view = node_id(&graph, NodeKind::Type, "sql view:active_users");

    let sql_node = graph
        .nodes
        .iter()
        .find(|node| node.id == sql_file)
        .expect("missing SQL file");
    assert_eq!(
        sql_node.metadata.get("language").map(String::as_str),
        Some("sql")
    );
    assert_eq!(
        sql_node.metadata.get("item_kind").map(String::as_str),
        Some("sql_schema")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == sql_file
            && edge.target == users
            && edge.kind == EdgeKind::Contains
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "sql_table")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == users
            && edge.target == email
            && edge.kind == EdgeKind::Contains
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "sql_column")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == org_id
            && edge.target == org_pk
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "sql_foreign_key")
            && edge
                .metadata
                .get("target_table")
                .is_some_and(|value| value == "organizations")
            && edge
                .metadata
                .get("target_column")
                .is_some_and(|value| value == "id")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == index
            && edge.target == users
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "sql_index_table")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == sql_file
            && edge.target == view
            && edge.kind == EdgeKind::Contains
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "sql_view")
    }));
    assert!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == organizations)
            .and_then(|node| node.metadata.get("table_key"))
            .is_some_and(|value| value == "organizations")
    );

    let coverage = scan_coverage(&root, &IndexOptions::default()).unwrap();
    assert_eq!(coverage.languages.get("sql"), Some(&1));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sql_query_table_refs_require_statement_shaped_literals() {
    assert!(
        sql_query_table_refs(
            "Build a workflow from a selected node, Entry Flows, or a query, then open it here."
        )
        .is_empty()
    );
    assert!(sql_query_table_refs("Update Cache").is_empty());
    assert!(
        sql_query_table_refs("`x` imports `serde` from production-like code, but the package")
            .is_empty()
    );
    assert!(sql_query_table_refs("With flour from the mill").is_empty());

    assert_eq!(sql_query_table_refs("SELECT id FROM users").len(), 1);
    assert_eq!(
        sql_query_table_refs("UPDATE users SET email = ? WHERE id = ?").len(),
        1
    );
    assert_eq!(
        sql_query_table_refs("WITH active AS (SELECT id FROM users) SELECT * FROM active").len(),
        2
    );
    let delete_refs = sql_query_table_refs("DELETE FROM audit_log");
    assert!(
        delete_refs
            .iter()
            .any(|reference| reference.operation == "delete" && reference.table == "audit_log")
    );
    assert_eq!(
        sql_query_table_refs("INSERT INTO users (email) VALUES (?)").len(),
        1
    );
}

#[test]
fn rust_use_globs_and_item_imports_are_not_local_file_imports() {
    assert!(rust_local_import_target("crates/x/src/lib.rs", "use super::*;").is_none());
    assert!(
        rust_local_import_target(
            "crates/x/src/mcp.rs",
            "use crate::{ImpactRequest, QueryError};"
        )
        .is_none()
    );
    let module = rust_local_import_target("crates/x/src/main.rs", "use crate::mcp;")
        .expect("module import should stay resolvable");
    assert_eq!(module.target, "mcp");
    assert!(
        module
            .candidates
            .iter()
            .any(|candidate| candidate.ends_with("src/mcp.rs"))
    );
}

#[test]
fn rust_inline_test_sql_fixtures_are_marked_test_context() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
            root.join("src").join("lib.rs"),
            "pub fn run() {}\n\n#[cfg(test)]\nmod tests {\n    const QUERY: &str = \"SELECT id FROM missing_table\";\n}\n",
        )
        .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let query = graph
        .nodes
        .iter()
        .find(|node| node.label.starts_with("sql query:src/lib.rs:"))
        .expect("inline test SQL literal should still be indexed");
    assert_eq!(
        query.metadata.get("test_context").map(String::as_str),
        Some("true")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_links_source_sql_queries_to_schema_tables() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("db")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("db").join("schema.sql"),
        r#"
CREATE TABLE organizations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    org_id INTEGER NOT NULL REFERENCES organizations(id),
    email TEXT NOT NULL
);
"#,
    )
    .unwrap();
    fs::write(
        root.join("src").join("repo.py"),
        r#"def load_users(db):
    rows = db.execute("""
        SELECT users.id, organizations.name
        FROM users
        JOIN organizations ON organizations.id = users.org_id
        WHERE users.email = ?
    """)
    db.execute("INSERT INTO users (email, org_id) VALUES (?, ?)")
    db.execute("SELECT * FROM audit_log")
    return rows
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let load_users = function_id_in_file(&graph, "load_users", "src/repo.py");
    let select_query = node_id(&graph, NodeKind::Config, "sql query:src/repo.py:2");
    let insert_query = node_id(&graph, NodeKind::Config, "sql query:src/repo.py:8");
    let missing_query = node_id(&graph, NodeKind::Config, "sql query:src/repo.py:9");
    let users = node_id(&graph, NodeKind::Type, "sql table:users");
    let organizations = node_id(&graph, NodeKind::Type, "sql table:organizations");

    let select_node = graph
        .nodes
        .iter()
        .find(|node| node.id == select_query)
        .expect("missing select query");
    assert_eq!(
        select_node.metadata.get("item_kind").map(String::as_str),
        Some("app_sql_query")
    );
    assert_eq!(
        select_node.metadata.get("operation").map(String::as_str),
        Some("select")
    );
    assert!(
        select_node
            .metadata
            .get("tables")
            .is_some_and(|value| value.contains("users") && value.contains("organizations"))
    );

    for query in [select_query, insert_query, missing_query] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == load_users
                && edge.target == query
                && edge.kind == EdgeKind::References
                && edge.confidence == Confidence::Heuristic
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "app_sql_query")
        }));
    }
    assert!(graph.edges.iter().any(|edge| {
        edge.source == select_query
            && edge.target == users
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "app_sql_table_reference")
            && edge
                .metadata
                .get("operation")
                .is_some_and(|value| value == "select")
            && edge
                .metadata
                .get("role")
                .is_some_and(|value| value == "source")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == select_query
            && edge.target == organizations
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "app_sql_table_reference")
            && edge
                .metadata
                .get("role")
                .is_some_and(|value| value == "join")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == insert_query
            && edge.target == users
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "app_sql_table_reference")
            && edge
                .metadata
                .get("operation")
                .is_some_and(|value| value == "insert")
    }));
    let missing_node = graph
        .nodes
        .iter()
        .find(|node| node.id == missing_query)
        .expect("missing unresolved query");
    assert_eq!(
        missing_node.metadata.get("resolution").map(String::as_str),
        Some("unresolved")
    );
    assert_eq!(
        missing_node
            .metadata
            .get("unresolved_tables")
            .map(String::as_str),
        Some("audit_log")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_uses_persistent_parse_cache_records() {
    let root = temp_project_root();
    let cache_dir = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    let source_path = root.join("src").join("main.rs");
    fs::write(&source_path, "fn main() {}\n").unwrap();
    let options = IndexOptions::default().with_parse_cache_dir(cache_dir.clone());

    let graph = scan_project(&root, &options).unwrap();
    let stamp = file_stamp(&source_path).unwrap();
    let cached = load_cached_parse(&cache_dir, "src/main.rs", Language::Rust, stamp).unwrap();

    assert!(graph.nodes.iter().any(|node| node.label == "main"));
    assert!(
        cached
            .items
            .iter()
            .any(|item| item.label == "main" && item.kind == ParsedItemKind::Entrypoint)
    );

    fs::write(&source_path, "fn main() {}\nfn helper() {}\n").unwrap();
    let graph = scan_project(&root, &options).unwrap();

    assert!(
        load_cached_parse(&cache_dir, "src/main.rs", Language::Rust, stamp).is_none(),
        "stale parse cache records must not be reused after file changes"
    );
    assert!(graph.nodes.iter().any(|node| node.label == "helper"));

    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(cache_dir).unwrap();
}

#[test]
fn scan_project_resolves_local_import_files() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
            root.join("src").join("app.js"),
            "import { helper } from './util.js';\nimport missing from './missing.js';\nconst util = require('./util');\nconst express = require('express');\nhelper();\n",
        )
        .unwrap();
    fs::write(
        root.join("src").join("util.js"),
        "export function helper() {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let util_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "import { helper } from './util.js';")
        .expect("missing util import node");
    let missing_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "import missing from './missing.js';")
        .expect("missing unresolved import node");
    let require_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "require(\"./util\")")
        .expect("missing CommonJS local require node");
    let express_require = graph
        .nodes
        .iter()
        .find(|node| node.label == "require(\"express\")")
        .expect("missing CommonJS package require node");
    let util_file = node_id(&graph, NodeKind::File, "src/util.js");

    assert_eq!(
        util_import.metadata.get("import_scope").map(String::as_str),
        Some("local")
    );
    assert_eq!(
        util_import.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    assert_eq!(
        util_import
            .metadata
            .get("resolved_path")
            .map(String::as_str),
        Some("src/util.js")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == util_import.id
            && edge.target == util_file
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "local_import_file")
    }));
    assert_eq!(
        require_import
            .metadata
            .get("import_style")
            .map(String::as_str),
        Some("commonjs")
    );
    assert_eq!(
        require_import
            .metadata
            .get("resolved_path")
            .map(String::as_str),
        Some("src/util.js")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == require_import.id
            && edge.target == util_file
            && edge.kind == EdgeKind::References
    }));
    assert_eq!(
        express_require
            .metadata
            .get("import_style")
            .map(String::as_str),
        Some("commonjs")
    );
    assert!(!express_require.metadata.contains_key("import_scope"));
    assert_eq!(
        missing_import
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("unresolved")
    );
    assert!(
        missing_import
            .metadata
            .get("candidate_paths")
            .is_some_and(|value| value.contains("src/missing.js"))
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_indexes_mcp_configs_as_tool_server_facts() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::create_dir_all(root.join("tools")).unwrap();
    // Hidden root config: the conventional location.
    fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers":{
                "codegraph":{"command":"codegraph","args":["mcp","."]},
                "local-tools":{"command":"python","args":["./scripts/server.py"]},
                "team-graph":{"url":"https://graph.example.com/api/mcp"}
            }}"#,
    )
    .unwrap();
    // Visible config elsewhere in the tree.
    fs::write(
        root.join("tools").join("mcp_servers.json"),
        r#"{"servers":{"linter":{"command":"npx","args":["lint-mcp"]}}}"#,
    )
    .unwrap();
    fs::write(
        root.join("scripts").join("server.py"),
        "def main():\n    pass\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    // The hidden root config is indexed despite being a dotfile.
    let config_file = node_id(&graph, NodeKind::File, ".mcp.json");
    let config_node = graph
        .nodes
        .iter()
        .find(|node| node.id == config_file)
        .unwrap();
    assert_eq!(
        config_node.metadata.get("item_kind").map(String::as_str),
        Some("mcp_config")
    );

    let stdio_server = graph
        .nodes
        .iter()
        .find(|node| node.label == "mcp server:local-tools")
        .expect("stdio server node");
    assert_eq!(
        stdio_server.metadata.get("transport").map(String::as_str),
        Some("stdio")
    );
    assert_eq!(
        stdio_server.metadata.get("command").map(String::as_str),
        Some("python")
    );
    let http_server = graph
        .nodes
        .iter()
        .find(|node| node.label == "mcp server:team-graph")
        .expect("http server node");
    assert_eq!(
        http_server.metadata.get("transport").map(String::as_str),
        Some("http")
    );
    assert!(
        http_server
            .metadata
            .get("url")
            .is_some_and(|url| url.contains("graph.example.com"))
    );

    // Config file links each declared server.
    assert!(graph.edges.iter().any(|edge| {
        edge.source == config_file
            && edge.target == stdio_server.id
            && edge.metadata.get("relation").map(String::as_str) == Some("mcp_server")
    }));

    // Path-like args link the server to its scanned source.
    let script = node_id(&graph, NodeKind::File, "scripts/server.py");
    assert!(graph.edges.iter().any(|edge| {
        edge.source == stdio_server.id
            && edge.target == script
            && edge.metadata.get("relation").map(String::as_str) == Some("mcp_server_source")
    }));

    // Visible configs with a `servers` key are indexed too.
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.label == "mcp server:linter"),
        "mcp_servers.json server must be indexed"
    );
}

#[test]
fn scan_links_code_to_sql_through_orm_and_migrations() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("migrations")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("migrations").join("001_init.sql"),
        "CREATE TABLE users (id INTEGER PRIMARY KEY);\n",
    )
    .unwrap();
    fs::write(
        root.join("migrations").join("002_orders.sql"),
        "CREATE TABLE orders (id INTEGER PRIMARY KEY);\n",
    )
    .unwrap();
    // ORM mappings across frameworks.
    fs::write(
        root.join("src").join("models.py"),
        "class User(Base):\n    __tablename__ = \"users\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("user.entity.ts"),
        "@Entity(\"users\")\nexport class User {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("User.php"),
        "<?php\nclass User extends Model {\n    protected $table = 'users';\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("user.go"),
        "package main\n\nfunc (User) TableName() string { return \"users\" }\n",
    )
    .unwrap();
    // Migration runner in code plus database config.
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    let migrator = sqlx::migrate!(\"./migrations\");\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("alembic.ini"),
        "[alembic]\nscript_location = migrations\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    let users_table = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("sql_table")
                && node.label.contains("users")
        })
        .expect("users table node");

    // Every ORM mapping links its file to the users table.
    for (file, pattern) in [
        ("src/models.py", "sqlalchemy_tablename"),
        ("src/user.entity.ts", "typeorm_entity"),
        ("src/User.php", "laravel_table"),
        ("src/user.go", "gorm_table_name"),
    ] {
        let file_id = node_id(&graph, NodeKind::File, file);
        assert!(
            graph.edges.iter().any(|edge| {
                edge.source == file_id
                    && edge.target == users_table.id
                    && edge.metadata.get("relation").map(String::as_str)
                        == Some("orm_table_mapping")
                    && edge.metadata.get("pattern").map(String::as_str) == Some(pattern)
            }),
            "missing orm_table_mapping for {file} ({pattern})"
        );
    }

    // Migration runner and db config link to both migration files.
    for (file, source_kind) in [("src/main.rs", "code"), ("alembic.ini", "db_config")] {
        let file_id = node_id(&graph, NodeKind::File, file);
        let linked: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| {
                edge.source == file_id
                    && edge.metadata.get("relation").map(String::as_str) == Some("runs_migrations")
                    && edge.metadata.get("source").map(String::as_str) == Some(source_kind)
            })
            .collect();
        assert_eq!(
            linked.len(),
            2,
            "{file} must link both migrations: {linked:?}"
        );
    }
}

#[test]
fn scan_extracts_sql_joins_migration_order_and_schema_changes() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("migrations")).unwrap();
    fs::write(
            root.join("migrations").join("001_init.sql"),
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);\nCREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users(id));\n",
        )
        .unwrap();
    fs::write(
            root.join("migrations").join("002_report_view.sql"),
            "CREATE VIEW user_orders AS SELECT u.name, o.id FROM users u JOIN orders o ON o.user_id = u.id;\nALTER TABLE users ADD COLUMN email TEXT;\nDROP TABLE IF EXISTS legacy_events;\n",
        )
        .unwrap();
    fs::write(
        root.join("migrations").join("002_duplicate.sql"),
        "ALTER TABLE orders ADD COLUMN total INTEGER;\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    // JOIN semantics: users joined to orders with the ON condition.
    let users = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("sql_table")
                && node.label.contains("users")
        })
        .expect("users table node");
    let orders = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("sql_table")
                && node.label.contains("orders")
        })
        .expect("orders table node");
    let join_edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.source == users.id
                && edge.target == orders.id
                && edge.metadata.get("relation").map(String::as_str) == Some("sql_join")
        })
        .expect("sql_join edge");
    assert!(
        join_edge
            .metadata
            .get("condition")
            .is_some_and(|condition| condition.contains("user_id")),
        "join condition must be captured: {:?}",
        join_edge.metadata
    );

    // Migration ordering: 001 -> 002 chain and sequence metadata.
    let first = node_id(&graph, NodeKind::File, "migrations/001_init.sql");
    let migration_files: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.metadata.contains_key("migration_sequence"))
        .collect();
    assert_eq!(migration_files.len(), 3);
    assert!(graph.edges.iter().any(|edge| {
        edge.source == first
            && edge.metadata.get("relation").map(String::as_str) == Some("migration_order")
            && edge.metadata.get("from_sequence").map(String::as_str) == Some("001")
    }));

    // Duplicate sequence flagged on both 002 files.
    let duplicates: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.metadata.contains_key("duplicate_migration_sequence"))
        .collect();
    assert_eq!(duplicates.len(), 2, "both 002 files must be flagged");

    // ALTER on known table links; DROP on unknown table is recorded.
    let second = node_id(&graph, NodeKind::File, "migrations/002_report_view.sql");
    assert!(graph.edges.iter().any(|edge| {
        edge.source == second
            && edge.target == users.id
            && edge.metadata.get("relation").map(String::as_str) == Some("sql_schema_change")
            && edge.metadata.get("operation").map(String::as_str) == Some("alter")
    }));
    let second_node = graph.nodes.iter().find(|node| node.id == second).unwrap();
    assert!(
        second_node
            .metadata
            .get("unresolved_sql_alter_tables")
            .is_some_and(|tables| tables.contains("drop:legacy_events")),
        "unknown DROP target must be recorded: {:?}",
        second_node.metadata
    );
}

#[test]
fn scan_matches_platform_channels_to_native_handlers() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::create_dir_all(root.join("android/app/src/main/kotlin/com/example")).unwrap();
    fs::create_dir_all(root.join("ios/Runner")).unwrap();
    fs::write(root.join("pubspec.yaml"), "name: demo\n").unwrap();
    fs::write(
            root.join("lib").join("main.dart"),
            "const channel = MethodChannel('com.example/native');\nconst events = EventChannel('com.example/events');\nconst lonely = MethodChannel('com.example/unhandled');\nvoid main() {}\n",
        )
        .unwrap();
    fs::write(
            root.join("android/app/src/main/kotlin/com/example/MainActivity.kt"),
            "class MainActivity : FlutterActivity() {\n  override fun configureFlutterEngine(engine: FlutterEngine) {\n    MethodChannel(engine.dartExecutor.binaryMessenger, \"com.example/native\").setMethodCallHandler { call, result -> }\n    EventChannel(engine.dartExecutor.binaryMessenger, \"com.example/events\").setStreamHandler(null)\n  }\n}\n",
        )
        .unwrap();
    fs::write(
            root.join("ios/Runner").join("AppDelegate.swift"),
            "let channel = FlutterMethodChannel(name: \"com.example/native\", binaryMessenger: controller.binaryMessenger)\n",
        )
        .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    let method_channel = graph
        .nodes
        .iter()
        .find(|node| node.label == "flutter method channel:com.example/native")
        .expect("method channel node");
    assert_eq!(
        method_channel
            .metadata
            .get("native_handler_android")
            .map(String::as_str),
        Some("android/app/src/main/kotlin/com/example/MainActivity.kt")
    );
    assert_eq!(
        method_channel
            .metadata
            .get("native_handler_ios")
            .map(String::as_str),
        Some("ios/Runner/AppDelegate.swift")
    );

    let event_channel = graph
        .nodes
        .iter()
        .find(|node| node.label == "flutter event channel:com.example/events")
        .expect("event channel node");
    assert!(
        event_channel
            .metadata
            .contains_key("native_handler_android")
    );
    assert!(!event_channel.metadata.contains_key("native_handler_ios"));

    let kotlin_file = node_id(
        &graph,
        NodeKind::File,
        "android/app/src/main/kotlin/com/example/MainActivity.kt",
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == kotlin_file
            && edge.target == method_channel.id
            && edge.kind == EdgeKind::References
            && edge.metadata.get("relation").map(String::as_str) == Some("platform_channel_handler")
            && edge.metadata.get("platform").map(String::as_str) == Some("android")
    }));

    let lonely = graph
        .nodes
        .iter()
        .find(|node| node.label == "flutter method channel:com.example/unhandled")
        .expect("unhandled channel node");
    assert!(
        !lonely
            .metadata
            .keys()
            .any(|key| key.starts_with("native_handler_")),
        "unmatched channel must stay unmarked"
    );
}

#[test]
fn scan_resolves_dart_package_config_and_generated_files() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("lib")).unwrap();
    fs::create_dir_all(root.join("app").join(".dart_tool")).unwrap();
    fs::create_dir_all(root.join("packages").join("shared").join("lib")).unwrap();
    fs::write(root.join("app").join("pubspec.yaml"), "name: app\n").unwrap();
    fs::write(
            root.join("app").join(".dart_tool").join("package_config.json"),
            r#"{"configVersion":2,"packages":[
                {"name":"shared","rootUri":"../../packages/shared","packageUri":"lib/"},
                {"name":"outside","rootUri":"../../../elsewhere","packageUri":"lib/"},
                {"name":"cached","rootUri":"file:///pub-cache/hosted/pub.dev/cached-1.0.0","packageUri":"lib/"}
            ]}"#,
        )
        .unwrap();
    fs::write(
            root.join("app").join("lib").join("main.dart"),
            "import 'package:shared/util.dart';\nimport 'package:outside/thing.dart';\npart 'main.g.dart';\nvoid main() {}\n",
        )
        .unwrap();
    fs::write(
        root.join("app").join("lib").join("main.g.dart"),
        "part of 'main.dart';\nvoid generatedHelper() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("lib").join("orphan.freezed.dart"),
        "void orphanGenerated() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("packages")
            .join("shared")
            .join("lib")
            .join("util.dart"),
        "void util() {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    // package: import resolved through package_config.json (shared has
    // no pubspec.yaml in the workspace, so only the config can map it).
    let util_file = node_id(&graph, NodeKind::File, "packages/shared/lib/util.dart");
    let import = graph
        .nodes
        .iter()
        .find(|node| node.label == "import 'package:shared/util.dart';")
        .expect("package_config import node");
    assert_eq!(
        import.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == import.id && edge.target == util_file && edge.kind == EdgeKind::References
    }));

    // Escaping and absolute rootUris never resolve into the workspace.
    let outside = graph
        .nodes
        .iter()
        .find(|node| node.label == "import 'package:outside/thing.dart';")
        .expect("outside import node");
    assert_ne!(
        outside.metadata.get("resolution").map(String::as_str),
        Some("resolved"),
        "rootUri escaping the scan root must stay unresolved"
    );

    // Generated-file conventions: tagged, and linked to the source that
    // generates them when it exists next to them.
    let generated = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "app/lib/main.g.dart")
        .expect("generated file node");
    assert_eq!(
        generated.metadata.get("generated").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        generated.metadata.get("generated_from").map(String::as_str),
        Some("app/lib/main.dart")
    );
    let orphan = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "app/lib/orphan.freezed.dart")
        .expect("orphan generated file node");
    assert_eq!(
        orphan.metadata.get("generated").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        orphan.metadata.get("generated_from"),
        None,
        "no source sibling means no generated_from link"
    );
}

#[test]
fn dart_root_uri_resolution_guards_workspace_escapes() {
    assert_eq!(
        resolve_dart_root_uri("app/.dart_tool", "../../packages/shared"),
        Some(Some("packages/shared".to_string()))
    );
    assert_eq!(resolve_dart_root_uri(".dart_tool", ".."), Some(None));
    assert_eq!(resolve_dart_root_uri("app/.dart_tool", "../../.."), None);
    assert_eq!(
        resolve_dart_root_uri(".dart_tool", "file:///abs/path"),
        None
    );
    assert_eq!(
        resolve_dart_root_uri(".dart_tool", "https://example.com/pkg"),
        None
    );
}

#[test]
fn scan_project_indexes_dart_flutter_pubspec_and_imports() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib").join("src")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    fs::write(
            root.join("pubspec.yaml"),
            "name: demo_app\nversion: 0.1.0\ndependencies:\n  flutter:\n    sdk: flutter\n  http: ^1.2.0\ndev_dependencies:\n  test: any\nflutter:\n  assets:\n    - assets/config/app.json\n    - assets/images/\n",
        )
        .unwrap();
    fs::write(
            root.join("lib").join("main.dart"),
            "import 'package:flutter/material.dart';\nimport 'package:demo_app/src/app.dart';\nimport 'src/local.dart';\npart 'src/main_part.dart';\n\nconst channel = MethodChannel('com.example.demo/native');\nclass Shell {}\nvoid main() {\n  const port = String.fromEnvironment('PORT', defaultValue: '8080');\n  final api = Platform.environment['API_URL'] ?? 'http://localhost';\n  final config = rootBundle.loadString('assets/config/app.json');\n  final logo = Image.asset('assets/images/logo.png');\n  runApp(App());\n  throw StateError('broken');\n}\n",
        )
        .unwrap();
    fs::write(
        root.join("lib").join("src").join("app.dart"),
        "class App {}\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("src").join("local.dart"),
        "void localHelper() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("src").join("main_part.dart"),
        "part of '../main.dart';\nvoid partHelper() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("bin").join("tool.dart"),
        "void main() { print('tool'); }\n",
    )
    .unwrap();
    fs::write(
        root.join("test").join("widget_test.dart"),
        "void main() { print('test'); }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    let main_file = node_id(&graph, NodeKind::File, "lib/main.dart");
    let app_file = node_id(&graph, NodeKind::File, "lib/src/app.dart");
    let local_file = node_id(&graph, NodeKind::File, "lib/src/local.dart");
    let part_file = node_id(&graph, NodeKind::File, "lib/src/main_part.dart");
    let main_fn = function_id_in_file(&graph, "main", "lib/main.dart");
    let tool_main = function_id_in_file(&graph, "main", "bin/tool.dart");
    let test_main = function_id_in_file(&graph, "main", "test/widget_test.dart");

    assert!(graph.edges.iter().any(|edge| {
        edge.source == main_file && edge.target == main_fn && edge.kind == EdgeKind::Contains
    }));

    for (label, target) in [
        ("import 'package:demo_app/src/app.dart';", app_file),
        ("import 'src/local.dart';", local_file),
        ("part 'src/main_part.dart';", part_file),
    ] {
        let import = graph
            .nodes
            .iter()
            .find(|node| node.label == label)
            .unwrap_or_else(|| panic!("missing Dart import node `{label}`"));
        assert_eq!(
            import.metadata.get("import_scope").map(String::as_str),
            Some("local")
        );
        assert_eq!(
            import.metadata.get("resolution").map(String::as_str),
            Some("resolved")
        );
        assert!(graph.edges.iter().any(|edge| {
            edge.source == import.id
                && edge.target == target
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "local_import_file")
        }));
    }

    assert!(graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Environment
            && node.label == "PORT"
            && node
                .metadata
                .get("default_value")
                .is_some_and(|value| value == "8080")
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Environment
            && node.label == "API_URL"
            && node
                .metadata
                .get("default_value")
                .is_some_and(|value| value == "http://localhost")
    }));
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Config && node.label == "assets/config/app.json")
    );
    let declared_asset = node_id(
        &graph,
        NodeKind::Config,
        "flutter asset:assets/config/app.json",
    );
    let declared_asset_dir = node_id(&graph, NodeKind::Config, "flutter asset:assets/images/");
    let asset_read = node_id(
        &graph,
        NodeKind::Config,
        "flutter asset read:assets/config/app.json",
    );
    let image_asset_read = node_id(
        &graph,
        NodeKind::Config,
        "flutter asset read:assets/images/logo.png",
    );
    for asset in [declared_asset, declared_asset_dir] {
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == asset)
            .expect("missing declared Flutter asset node");
        assert_eq!(
            node.metadata.get("item_kind").map(String::as_str),
            Some("flutter_asset")
        );
        assert_eq!(
            node.metadata.get("source").map(String::as_str),
            Some("pubspec")
        );
    }
    for asset_read in [asset_read, image_asset_read] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == main_file
                && edge.target == asset_read
                && edge.kind == EdgeKind::ReadsConfig
                && edge
                    .metadata
                    .get("config_kind")
                    .is_some_and(|value| value == "flutter_asset_read")
        }));
    }
    let channel = node_id(
        &graph,
        NodeKind::ExternalDependency,
        "flutter method channel:com.example.demo/native",
    );
    let channel_node = graph
        .nodes
        .iter()
        .find(|node| node.id == channel)
        .expect("missing platform channel node");
    assert_eq!(
        channel_node.metadata.get("item_kind").map(String::as_str),
        Some("platform_channel")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == main_file
            && edge.target == channel
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "platform_channel")
    }));
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.source == main_fn && edge.kind == EdgeKind::MayError)
    );

    let flutter_entry = node_id(&graph, NodeKind::Entrypoint, "flutter app:demo_app");
    let tool_entry = node_id(&graph, NodeKind::Entrypoint, "dart bin:tool");
    let test_entry = node_id(&graph, NodeKind::Entrypoint, "dart test:widget_test.dart");
    assert!(has_entrypoint_reference(
        &graph,
        flutter_entry,
        main_fn,
        "entrypoint_function",
        Confidence::Syntactic
    ));
    assert!(has_entrypoint_reference(
        &graph,
        tool_entry,
        tool_main,
        "entrypoint_function",
        Confidence::Syntactic
    ));
    assert!(has_entrypoint_reference(
        &graph,
        test_entry,
        test_main,
        "entrypoint_function",
        Confidence::Syntactic
    ));

    let http_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "dart:http")
        })
        .expect("missing pubspec http dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == http_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "^1.2.0")
    }));
    let test_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "dart:test")
        })
        .expect("missing pubspec test dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == test_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "dev")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "any")
    }));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_resolves_cmake_include_directories() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("include").join("app")).unwrap();
    fs::write(
            root.join("CMakeLists.txt"),
            "cmake_minimum_required(VERSION 3.20)\nproject(demo C)\nadd_executable(demo src/main.c)\ntarget_include_directories(demo PRIVATE ${PROJECT_SOURCE_DIR}/include)\n",
        )
        .unwrap();
    fs::write(
        root.join("src").join("main.c"),
        "#include \"app/config.h\"\nint main() { return APP_VALUE; }\n",
    )
    .unwrap();
    fs::write(
        root.join("include").join("app").join("config.h"),
        "#define APP_VALUE 0\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let include = graph
        .nodes
        .iter()
        .find(|node| node.label == "#include \"app/config.h\"")
        .expect("missing C include node");
    let header = node_id(&graph, NodeKind::File, "include/app/config.h");

    assert_eq!(
        include.metadata.get("import_scope").map(String::as_str),
        Some("local")
    );
    assert_eq!(
        include.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    assert_eq!(
        include.metadata.get("resolved_path").map(String::as_str),
        Some("include/app/config.h")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == include.id
            && edge.target == header
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "local_import_file")
    }));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_resolves_compile_commands_include_directories() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("include").join("app")).unwrap();
    fs::create_dir_all(root.join("extras").join("detail")).unwrap();
    fs::write(
            root.join("src").join("main.cpp"),
            "#include \"app/settings.hpp\"\n#include \"detail/log.hpp\"\nint main() { return SETTING; }\n",
        )
        .unwrap();
    fs::write(
        root.join("include").join("app").join("settings.hpp"),
        "#define SETTING 0\n",
    )
    .unwrap();
    fs::write(
        root.join("extras").join("detail").join("log.hpp"),
        "#pragma once\n",
    )
    .unwrap();
    let commands = serde_json::json!([
        {
            "directory": root.to_string_lossy(),
            "file": "src/main.cpp",
            "arguments": ["clang++", "-I", "include", "-c", "src/main.cpp"]
        },
        {
            "directory": root.to_string_lossy(),
            "file": "src/main.cpp",
            "command": "clang++ -Iextras -c src/main.cpp"
        }
    ]);
    fs::write(
        root.join("compile_commands.json"),
        serde_json::to_string_pretty(&commands).unwrap(),
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let settings_include = graph
        .nodes
        .iter()
        .find(|node| node.label == "#include \"app/settings.hpp\"")
        .expect("missing settings include node");
    let log_include = graph
        .nodes
        .iter()
        .find(|node| node.label == "#include \"detail/log.hpp\"")
        .expect("missing log include node");
    let settings_header = node_id(&graph, NodeKind::File, "include/app/settings.hpp");
    let log_header = node_id(&graph, NodeKind::File, "extras/detail/log.hpp");

    assert_eq!(
        settings_include
            .metadata
            .get("resolved_path")
            .map(String::as_str),
        Some("include/app/settings.hpp")
    );
    assert_eq!(
        log_include
            .metadata
            .get("resolved_path")
            .map(String::as_str),
        Some("extras/detail/log.hpp")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == settings_include.id
            && edge.target == settings_header
            && edge.kind == EdgeKind::References
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == log_include.id
            && edge.target == log_header
            && edge.kind == EdgeKind::References
    }));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_resolves_python_absolute_local_imports() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("services")).unwrap();
    fs::write(
        root.join("app").join("main.py"),
        "import utils\nfrom app.services import auth\nfrom requests import Session\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("utils.py"),
        "def helper():\n    pass\n",
    )
    .unwrap();
    fs::write(root.join("app").join("__init__.py"), "").unwrap();
    fs::write(root.join("app").join("services").join("__init__.py"), "").unwrap();
    fs::write(
        root.join("app").join("services").join("auth.py"),
        "def login():\n    pass\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let utils_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "import utils")
        .expect("missing utils import node");
    let auth_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "from app.services import auth")
        .expect("missing auth import node");
    let requests_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "from requests import Session")
        .expect("missing requests import node");
    let utils_file = node_id(&graph, NodeKind::File, "app/utils.py");
    let auth_file = node_id(&graph, NodeKind::File, "app/services/auth.py");

    assert_eq!(
        utils_import
            .metadata
            .get("import_scope")
            .map(String::as_str),
        Some("local")
    );
    assert_eq!(
        utils_import
            .metadata
            .get("resolved_path")
            .map(String::as_str),
        Some("app/utils.py")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == utils_import.id
            && edge.target == utils_file
            && edge.kind == EdgeKind::References
    }));
    assert_eq!(
        auth_import.metadata.get("import_scope").map(String::as_str),
        Some("local")
    );
    assert_eq!(
        auth_import
            .metadata
            .get("resolved_path")
            .map(String::as_str),
        Some("app/services/auth.py")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == auth_import.id
            && edge.target == auth_file
            && edge.kind == EdgeKind::References
    }));
    assert!(!requests_import.metadata.contains_key("import_scope"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_resolves_go_module_local_imports() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("auth")).unwrap();
    fs::write(
        root.join("go.mod"),
        "module github.com/acme/demo\n\ngo 1.23\n\nrequire github.com/gin-gonic/gin v1.10.0\n",
    )
    .unwrap();
    fs::write(
            root.join("main.go"),
            "package main\n\nimport (\n    \"fmt\"\n    \"github.com/acme/demo/internal/auth\"\n    \"github.com/gin-gonic/gin\"\n)\n\nfunc main() { fmt.Println(auth.Name); _ = gin.Mode() }\n",
        )
        .unwrap();
    fs::write(
        root.join("internal").join("auth").join("service.go"),
        "package auth\n\nconst Name = \"auth\"\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let local_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "\"github.com/acme/demo/internal/auth\"")
        .expect("missing Go module local import node");
    let external_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "\"github.com/gin-gonic/gin\"")
        .expect("missing Go external import node");
    let stdlib_import = graph
        .nodes
        .iter()
        .find(|node| node.label == "\"fmt\"")
        .expect("missing Go stdlib import node");
    let auth_file = node_id(&graph, NodeKind::File, "internal/auth/service.go");

    assert_eq!(
        local_import
            .metadata
            .get("import_scope")
            .map(String::as_str),
        Some("local")
    );
    assert_eq!(
        local_import.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    assert_eq!(
        local_import
            .metadata
            .get("resolved_path")
            .map(String::as_str),
        Some("internal/auth/service.go")
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == local_import.id
            && edge.target == auth_file
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "local_import_file")
    }));
    assert!(!external_import.metadata.contains_key("import_scope"));
    assert!(!stdlib_import.metadata.contains_key("import_scope"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_approximate_call_edges() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() { helper(); }\nfn helper() {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let main_id = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "main")
        .map(|node| node.id)
        .unwrap();
    let helper_id = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "helper")
        .map(|node| node.id)
        .unwrap();

    assert!(graph.edges.iter().any(|edge| {
        edge.source == main_id
            && edge.target == helper_id
            && edge.kind == EdgeKind::Calls
            && edge.confidence == Confidence::Heuristic
            && edge.metadata.get("call_label").map(String::as_str) == Some("helper")
            && edge.metadata.get("resolution").map(String::as_str) == Some("resolved")
    }));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_indexes_control_flow_facts() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "async fn worker() { if ready() { for item in items() { item.await; } return; } }\n",
    )
    .unwrap();
    fs::write(
            root.join("lib").join("main.dart"),
            "void worker() async { if (ready) { for (final item in items) { await item; } return; } }\n",
        )
        .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let rust_worker = function_id_in_file(&graph, "worker", "src/main.rs");
    let dart_worker = function_id_in_file(&graph, "worker", "lib/main.dart");

    for (language, source_id, expected) in [
        (
            "rust",
            rust_worker,
            ["branch: if", "loop: for", "async: await", "return: return"],
        ),
        (
            "dart",
            dart_worker,
            ["branch: if", "loop: for", "async: await", "return: return"],
        ),
    ] {
        for label in expected {
            let fact = graph
                .nodes
                .iter()
                .find(|node| {
                    node.kind == NodeKind::ControlFlow
                        && node.label == label
                        && node.metadata.get("language").map(String::as_str) == Some(language)
                })
                .unwrap_or_else(|| panic!("missing {language} control-flow fact {label}"));
            assert!(matches!(
                fact.metadata.get("item_kind").map(String::as_str),
                Some("branch" | "loop" | "async" | "return")
            ));
            assert_eq!(
                fact.metadata.get("parent").map(String::as_str),
                Some("worker")
            );
            assert!(fact.metadata.contains_key("control_kind"));
            assert!(graph.edges.iter().any(|edge| {
                edge.source == source_id
                    && edge.target == fact.id
                    && edge.kind == EdgeKind::References
                    && edge.confidence == Confidence::Heuristic
            }));
        }
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_marks_ambiguous_call_edges() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() { parse(); }\n").unwrap();
    fs::write(root.join("src").join("left.rs"), "fn parse() {}\n").unwrap();
    fs::write(root.join("src").join("right.rs"), "fn parse() {}\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let main_id = node_id(&graph, NodeKind::Function, "main");
    let ambiguous_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.source == main_id && edge.kind == EdgeKind::Calls)
        .filter(|edge| edge.metadata.get("call_label").map(String::as_str) == Some("parse"))
        .collect::<Vec<_>>();

    assert_eq!(ambiguous_edges.len(), 1);
    assert_eq!(
        ambiguous_edges[0]
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("ambiguous")
    );
    let placeholder = graph
        .nodes
        .iter()
        .find(|node| node.id == ambiguous_edges[0].target)
        .expect("bounded ambiguity placeholder");
    assert_eq!(placeholder.kind, NodeKind::ExternalDependency);
    assert_eq!(
        placeholder
            .metadata
            .get("candidate_count")
            .map(String::as_str),
        Some("2")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn call_resolution_never_crosses_languages() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() { shared(); }\nfn shared() {}\n",
    )
    .unwrap();
    fs::write(root.join("lib").join("main.dart"), "void shared() {}\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let rust_main = function_id_in_file(&graph, "main", "src/main.rs");
    let rust_shared = function_id_in_file(&graph, "shared", "src/main.rs");
    let calls = graph
        .edges
        .iter()
        .filter(|edge| edge.source == rust_main && edge.kind == EdgeKind::Calls)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].target, rust_shared);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dart_type_references_link_consumers_without_fanout() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib").join("service.dart"),
        "class GroupService {}\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("consumer.dart"),
        "GroupService? current;\nGroupService make() => GroupService();\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let service = node_id(&graph, NodeKind::Type, "GroupService");
    let consumer = function_id_in_file(&graph, "make", "lib/consumer.dart");
    let type_edges = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == service
                && edge.kind == EdgeKind::References
                && matches!(
                    edge.metadata.get("relation").map(String::as_str),
                    Some("type_reference" | "constructor_reference")
                )
        })
        .collect::<Vec<_>>();
    assert!(type_edges.iter().any(|edge| edge.source == consumer));
    assert!(type_edges.len() <= 2, "one edge per consumer source");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stable_ids_survive_unrelated_file_additions() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let before = scan_project(&root, &IndexOptions::default()).unwrap();
    let stable_before = before
        .nodes
        .iter()
        .find(|node| node.label == "main")
        .and_then(|node| node.metadata.get("stable_id"))
        .cloned()
        .expect("stable id");

    fs::write(root.join("src").join("added.rs"), "fn added() {}\n").unwrap();
    let after = scan_project(&root, &IndexOptions::default()).unwrap();
    let stable_after = after
        .nodes
        .iter()
        .find(|node| node.label == "main")
        .and_then(|node| node.metadata.get("stable_id"))
        .expect("stable id after rescan");
    assert_eq!(stable_before, stable_after.as_str());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_indexes_mixed_language_repository_as_one_graph() {
    let root = temp_project_root();
    for dir in [
        "rust/src",
        "py",
        "web",
        "go/cmd/server",
        "go/internal/app",
        "native",
        "cpp",
        "public",
        "scripts",
    ] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(
            root.join("rust").join("src").join("main.rs"),
            "mod config;\nuse crate::config::load_config;\nfn main() { let _ = std::env::var(\"DATABASE_URL\"); load_config(); }\n",
        )
        .unwrap();
    fs::write(
        root.join("rust").join("src").join("config.rs"),
        "pub fn load_config() {}\n",
    )
    .unwrap();
    fs::write(
            root.join("py").join("app.py"),
            "import os\nfrom helpers import py_helper\ndef main():\n    os.getenv(\"PY_TOKEN\")\n    py_helper()\n",
        )
        .unwrap();
    fs::write(
        root.join("py").join("helpers.py"),
        "def py_helper():\n    return True\n",
    )
    .unwrap();
    fs::write(
            root.join("web").join("app.js"),
            "import { start } from './lib.js';\nfunction main() { start(); return process.env.API_URL; }\n",
        )
        .unwrap();
    fs::write(
        root.join("web").join("lib.js"),
        "export function start() { return true; }\n",
    )
    .unwrap();
    fs::write(root.join("go").join("go.mod"), "module example.com/mixed\n").unwrap();
    fs::write(
            root.join("go").join("cmd").join("server").join("main.go"),
            "package main\nimport (\n  \"fmt\"\n  \"example.com/mixed/internal/app\"\n)\nfunc main() { fmt.Println(app.Name()) }\n",
        )
        .unwrap();
    fs::write(
        root.join("go").join("internal").join("app").join("app.go"),
        "package app\nfunc Name() string { return \"mixed\" }\n",
    )
    .unwrap();
    fs::write(
            root.join("native").join("main.c"),
            "#include \"native.h\"\n#include <stdlib.h>\nint main(void) { getenv(\"C_TOKEN\"); return native_value(); }\n",
        )
        .unwrap();
    fs::write(
        root.join("native").join("native.h"),
        "int native_value(void);\n",
    )
    .unwrap();
    fs::write(
        root.join("cpp").join("service.cpp"),
        "#include \"service.hpp\"\nint main() { return service_value(); }\n",
    )
    .unwrap();
    fs::write(
        root.join("cpp").join("service.hpp"),
        "int service_value();\n",
    )
    .unwrap();
    fs::write(
        root.join("public").join("index.php"),
        "<?php\nrequire 'lib.php';\nfunction main() { getenv('PHP_TOKEN'); app_boot(); }\n",
    )
    .unwrap();
    fs::write(
        root.join("public").join("lib.php"),
        "<?php\nfunction app_boot() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts").join("deploy.sh"),
        "source ./env.sh\nmain() { echo \"$DEPLOY_ENV\"; }\n",
    )
    .unwrap();
    fs::write(root.join("scripts").join("env.sh"), "DEPLOY_ENV=prod\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let labels: BTreeSet<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();
    for expected in [
        "rust/src/main.rs",
        "py/app.py",
        "web/app.js",
        "go/cmd/server/main.go",
        "native/main.c",
        "cpp/service.cpp",
        "public/index.php",
        "scripts/deploy.sh",
    ] {
        assert!(labels.contains(expected), "missing file node `{expected}`");
    }

    let languages: BTreeSet<_> = graph
        .nodes
        .iter()
        .filter_map(|node| node.metadata.get("language").map(String::as_str))
        .collect();
    for expected in [
        "rust",
        "python",
        "javascript",
        "go",
        "c",
        "cpp",
        "php",
        "bash",
    ] {
        assert!(
            languages.contains(expected),
            "missing language facet `{expected}` in {languages:?}"
        );
    }

    for expected in [
        "py/helpers.py",
        "web/lib.js",
        "go/internal/app/app.go",
        "native/native.h",
        "cpp/service.hpp",
        "public/lib.php",
        "scripts/env.sh",
    ] {
        assert!(
            has_resolved_local_import(&graph, expected),
            "missing resolved local import to `{expected}`"
        );
    }

    for expected in [
        "DATABASE_URL",
        "PY_TOKEN",
        "API_URL",
        "C_TOKEN",
        "PHP_TOKEN",
    ] {
        assert!(
            graph.nodes.iter().any(|node| matches!(
                node.kind,
                NodeKind::Config | NodeKind::Environment
            ) && node.label == expected),
            "missing config/environment fact `{expected}`"
        );
    }

    let entrypoint_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Entrypoint)
        .count();
    assert!(
        entrypoint_edges >= 6,
        "expected several mixed-language entrypoints, found {entrypoint_edges}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_environment_config_and_error_edges() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
                let _ = std::env::var("DATABASE_URL");
                let _ = std::fs::read_to_string("config/app.toml");
                panic!("broken");
            }
            "#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    assert!(
        graph
            .nodes
            .iter()
            .any(|node| { node.kind == NodeKind::Environment && node.label == "DATABASE_URL" })
    );
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Config && node.label == "config/app.toml")
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::ReadsEnvironment)
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::ReadsConfig)
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::MayError)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_rationale_comment_nodes() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("main.py"),
        r#"# WHY: keep startup simple until plugins stabilize
def main():
    # FIXME: handle retry backoff
    return True
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    let why = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|value| value == "rationale_comment")
                && node
                    .metadata
                    .get("rationale_kind")
                    .is_some_and(|value| value == "why")
        })
        .expect("WHY comment should be indexed");
    assert_eq!(
        why.label,
        "WHY: keep startup simple until plugins stabilize"
    );
    assert_eq!(why.span.as_ref().map(|span| span.start_line), Some(1));
    assert_eq!(
        why.metadata.get("language").map(String::as_str),
        Some("python")
    );
    let fixme = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|value| value == "rationale_comment")
                && node
                    .metadata
                    .get("rationale_kind")
                    .is_some_and(|value| value == "fixme")
        })
        .expect("FIXME comment should be indexed");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::Contains
            && edge.target == fixme.id
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "rationale_comment")
    }));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_preserves_environment_default_values() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.py"),
        r#"import os
PORT = os.getenv("PORT", "8000")
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "7000".to_string());
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("server.js"),
        r#"const port = process.env.PORT || "3000";
"#,
    )
    .unwrap();
    fs::write(
        root.join("main.go"),
        r#"package main

import (
    "cmp"
    "os"
)

func main() {
    port := cmp.Or(os.Getenv("PORT"), "9090")
    _ = port
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("main.c"),
        r#"#include <stdlib.h>
int main(void) {
    const char *port = getenv("PORT") ?: "9091";
    return port ? 0 : 1;
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("main.cpp"),
        r#"#include <cstdlib>
int main() {
    auto port = std::getenv("PORT") ?: "9092";
    return port ? 0 : 1;
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("index.php"),
        r#"<?php
$port = getenv('PORT') ?: '8080';
"#,
    )
    .unwrap();
    fs::write(
        root.join("entrypoint.sh"),
        r#"#!/usr/bin/env bash
PORT="${PORT:-5000}"
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let defaults = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Environment && node.label == "PORT")
        .filter_map(|node| node.metadata.get("default_value").map(String::as_str))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        defaults,
        BTreeSet::from([
            "3000", "5000", "7000", "8000", "8080", "9090", "9091", "9092"
        ])
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_manifest_dependency_edges() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"

[workspace]
members = ["crates/app"]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
local-util = { path = "crates/local-util" }

[dependencies]
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
anyhow = "1"
"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("crates").join("app")).unwrap();
    fs::write(
        root.join("crates").join("app").join("Cargo.toml"),
        r#"[package]
name = "app"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
local-util = { workspace = true }
"#,
    )
    .unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
  "dependencies": { "react": "^19.0.0" },
  "devDependencies": { "react": "^19.0.0", "vite": "^7.0.0" }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("package-lock.json"),
        r#"{
  "name": "demo",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "dependencies": {
        "react": "^19.0.0",
        "lodash": "^4.17.21"
      },
      "devDependencies": {
        "vitest": "^3.2.0"
      },
      "optionalDependencies": {
        "fsevents": "^2.3.3"
      }
    },
    "node_modules/react": { "version": "19.0.0" },
    "node_modules/lodash": { "version": "4.17.21" },
    "node_modules/vitest": { "version": "3.2.1" },
    "node_modules/fsevents": { "version": "2.3.3", "optional": true }
  }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("pnpm-lock.yaml"),
        r#"lockfileVersion: '9.0'

importers:
  .:
    dependencies:
      solid-js:
        specifier: ^1.8.0
        version: 1.8.19
    devDependencies:
      '@types/node':
        specifier: ^22.0.0
        version: 22.13.1
  packages/app:
    peerDependencies:
      magic-string:
        specifier: ^0.30.0
        version: 0.30.17(supports-color@9.4.0)
"#,
    )
    .unwrap();
    fs::write(
            root.join("go.mod"),
            "module example.com/demo\n\nrequire github.com/gin-gonic/gin v1.10.0\nrequire golang.org/x/sys v0.30.0 // indirect\n",
        )
        .unwrap();
    fs::write(root.join("requirements.txt"), "fastapi==0.115.0\n").unwrap();
    fs::write(
        root.join("pyproject.toml"),
        r#"[project]
dependencies = ["pydantic>=2"]

[tool.poetry.group.dev.dependencies]
black = "^24.8"

[tool.poetry.group.test.dependencies]
pytest-asyncio = { version = "^0.24" }

[tool.poetry.group.docs.dependencies]
sphinx = "^8.0"
"#,
    )
    .unwrap();
    fs::write(
        root.join("Pipfile"),
        r#"[packages]
Flask = ">=3"
python-dotenv = "*"

[dev-packages]
pytest-cov = { version = ">=5" }
"#,
    )
    .unwrap();
    fs::write(
        root.join("setup.py"),
        r#"from setuptools import setup

setup(
    name="legacy-demo",
    install_requires=[
        "requests>=2.31",
        "uvicorn[standard]>=0.24",
    ],
    setup_requires=["wheel>=0.42"],
    tests_require=["pytest>=8"],
    extras_require={
        "dev": ["ruff==0.6.0"],
        "docs": ["mkdocs>=1.6"],
    },
)
"#,
    )
    .unwrap();
    fs::write(
        root.join("setup.cfg"),
        r#"[metadata]
name = legacy-cfg-demo

[options]
install_requires =
    httpx>=0.27
setup_requires =
    cython>=3
tests_require =
    hypothesis>=6

[options.extras_require]
cli =
    rich>=13
"#,
    )
    .unwrap();
    fs::write(
        root.join("composer.json"),
        r#"{
  "require": {
    "php": ">=8.2",
    "monolog/monolog": "^3.0"
  }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("composer.lock"),
        r#"{
  "packages": [
    { "name": "monolog/monolog", "version": "3.8.1" },
    { "name": "symfony/console", "version": "v7.2.1" }
  ],
  "packages-dev": [
    { "name": "phpunit/phpunit", "version": "11.5.0" }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        root.join("vcpkg.json"),
        r#"{
  "name": "demo",
  "version-string": "0.1.0",
  "dependencies": [
    "fmt",
    { "name": "zlib", "features": ["minizip"] }
  ],
  "overrides": [
    { "name": "fmt", "version": "10.2.1" },
    { "name": "zlib", "version>=": "1.3.1" }
  ]
}"#,
    )
    .unwrap();
    fs::write(
        root.join("conanfile.txt"),
        r#"[requires]
spdlog/1.13.0
openssl/[>=3 <4]

[tool_requires]
cmake/3.29.0

[test_requires]
gtest/1.14.0
"#,
    )
    .unwrap();
    fs::write(
        root.join("CMakeLists.txt"),
        r#"cmake_minimum_required(VERSION 3.20)
project(demo CXX)
find_package(OpenSSL 3 REQUIRED)
find_package(Boost 1.83 REQUIRED COMPONENTS filesystem)
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let dependency_labels: BTreeSet<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "dependency")
        })
        .map(|node| node.label.as_str())
        .collect();

    for expected in [
        "serde",
        "local-util",
        "anyhow",
        "react",
        "vite",
        "lodash",
        "vitest",
        "fsevents",
        "solid-js",
        "@types/node",
        "magic-string",
        "github.com/gin-gonic/gin",
        "golang.org/x/sys",
        "fastapi",
        "pydantic",
        "black",
        "pytest-asyncio",
        "sphinx",
        "flask",
        "python-dotenv",
        "pytest-cov",
        "requests",
        "uvicorn",
        "wheel",
        "pytest",
        "ruff",
        "mkdocs",
        "httpx",
        "cython",
        "hypothesis",
        "rich",
        "monolog/monolog",
        "symfony/console",
        "phpunit/phpunit",
        "fmt",
        "zlib",
        "spdlog",
        "openssl",
        "boost",
        "cmake",
        "gtest",
    ] {
        assert!(dependency_labels.contains(expected), "missing {expected}");
    }
    assert!(!dependency_labels.contains("php"));
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn && edge.confidence == Confidence::Exact
        })
    );
    let serde_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "cargo:serde")
        })
        .collect();
    assert_eq!(serde_nodes.len(), 1);
    let serde_incoming_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::DependsOn && edge.target == serde_nodes[0].id)
        .collect();
    assert_eq!(serde_incoming_edges.len(), 2);
    assert!(serde_incoming_edges.iter().all(|edge| {
        edge.metadata
            .get("dependency_kind")
            .is_some_and(|value| value == "runtime")
    }));
    assert!(serde_incoming_edges.iter().all(|edge| {
        edge.metadata
            .get("dependency_version")
            .is_some_and(|value| value == "1")
    }));
    let local_util = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "cargo:local-util")
        })
        .expect("missing local-util dependency");
    let local_util_edge = graph
        .edges
        .iter()
        .find(|edge| edge.kind == EdgeKind::DependsOn && edge.target == local_util.id)
        .expect("missing local-util dependency edge");
    assert!(!local_util_edge.metadata.contains_key("dependency_version"));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "^19.0.0")
            && edge
                .metadata
                .get("dependency_version_kind")
                .is_some_and(|value| value == "constraint")
    }));
    let react = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:react")
        })
        .expect("missing react dependency");
    let react_kinds: BTreeSet<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::DependsOn && edge.target == react.id)
        .filter_map(|edge| edge.metadata.get("dependency_kind").map(String::as_str))
        .collect();
    assert_eq!(react_kinds, BTreeSet::from(["dev", "runtime"]));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == react.id
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "19.0.0")
            && edge
                .metadata
                .get("dependency_version_kind")
                .is_some_and(|value| value == "locked")
    }));
    let lodash_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:lodash")
        })
        .expect("missing package-lock lodash dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == lodash_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "4.17.21")
    }));
    let vitest_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:vitest")
        })
        .expect("missing package-lock vitest dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == vitest_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "dev")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "3.2.1")
    }));
    let fsevents_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:fsevents")
        })
        .expect("missing package-lock fsevents dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == fsevents_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "optional")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "2.3.3")
    }));
    let solid_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:solid-js")
        })
        .expect("missing pnpm solid-js dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == solid_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "1.8.19")
    }));
    let types_node_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:@types/node")
        })
        .expect("missing pnpm @types/node dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == types_node_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "dev")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "22.13.1")
    }));
    let magic_string_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:magic-string")
        })
        .expect("missing pnpm magic-string dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == magic_string_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "peer")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "0.30.17")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "v1.10.0")
    }));
    let go_indirect_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "go:golang.org/x/sys")
        })
        .expect("missing go indirect dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == go_indirect_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "indirect")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "v0.30.0")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == ">=2")
    }));
    let black_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:black")
        })
        .expect("missing Poetry group black dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == black_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "dev")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "^24.8")
    }));
    let pytest_asyncio_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:pytest-asyncio")
        })
        .expect("missing Poetry group pytest-asyncio dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == pytest_asyncio_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "test")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "^0.24")
    }));
    let sphinx_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:sphinx")
        })
        .expect("missing Poetry group sphinx dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == sphinx_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "optional")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "^8.0")
    }));
    let flask_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:flask")
        })
        .expect("missing Pipfile flask dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == flask_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == ">=3")
    }));
    let python_dotenv_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:python-dotenv")
        })
        .expect("missing Pipfile python-dotenv dependency");
    let python_dotenv_edge = graph
        .edges
        .iter()
        .find(|edge| edge.kind == EdgeKind::DependsOn && edge.target == python_dotenv_dep.id)
        .expect("missing Pipfile python-dotenv dependency edge");
    assert!(
        !python_dotenv_edge
            .metadata
            .contains_key("dependency_version")
    );
    let pytest_cov_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:pytest-cov")
        })
        .expect("missing Pipfile pytest-cov dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == pytest_cov_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "dev")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == ">=5")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == ">=2.31")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == ">=0.27")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "[standard]>=0.24")
    }));
    let wheel_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:wheel")
        })
        .expect("missing setup.py wheel dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == wheel_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "build")
    }));
    let cython_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:cython")
        })
        .expect("missing setup.cfg cython dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == cython_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "build")
    }));
    let pytest_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:pytest")
        })
        .expect("missing setup.py pytest dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == pytest_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "test")
    }));
    let hypothesis_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:hypothesis")
        })
        .expect("missing setup.cfg hypothesis dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == hypothesis_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "test")
    }));
    let ruff_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:ruff")
        })
        .expect("missing setup.py ruff dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == ruff_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "optional")
    }));
    let rich_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "python:rich")
        })
        .expect("missing setup.cfg rich dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == rich_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "optional")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "^3.0")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "3.8.1")
            && edge
                .metadata
                .get("dependency_version_kind")
                .is_some_and(|value| value == "locked")
    }));
    let symfony_console_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "composer:symfony/console")
        })
        .expect("missing composer.lock symfony/console dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == symfony_console_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "v7.2.1")
            && edge
                .metadata
                .get("dependency_version_kind")
                .is_some_and(|value| value == "locked")
    }));
    let phpunit_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "composer:phpunit/phpunit")
        })
        .expect("missing composer.lock phpunit dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == phpunit_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "dev")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "11.5.0")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "10.2.1")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == ">=1.3.1")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "1.13.0")
    }));
    let cmake_openssl_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "cmake:openssl")
        })
        .expect("missing CMake OpenSSL dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == cmake_openssl_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "3")
    }));
    let cmake_boost_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "cmake:boost")
        })
        .expect("missing CMake Boost dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == cmake_boost_dep.id
            && edge
                .metadata
                .get("dependency_version")
                .is_some_and(|value| value == "1.83")
    }));
    let cmake_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "conan:cmake")
        })
        .expect("missing conan cmake dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == cmake_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "build")
    }));
    let gtest_dep = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "conan:gtest")
        })
        .expect("missing conan gtest dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == gtest_dep.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "test")
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.metadata
            .get("ecosystem")
            .is_some_and(|value| value == "cargo")
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.metadata
            .get("ecosystem")
            .is_some_and(|value| value == "npm")
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.metadata
            .get("ecosystem")
            .is_some_and(|value| value == "vcpkg")
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.metadata
            .get("ecosystem")
            .is_some_and(|value| value == "conan")
    }));
    assert!(graph.nodes.iter().any(|node| {
        node.metadata
            .get("ecosystem")
            .is_some_and(|value| value == "cmake")
    }));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_manifest_entrypoint_edges() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("src").join("bin")).unwrap();
    fs::create_dir_all(root.join("codegraph")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("cmd").join("server")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() { helper(); }\nfn helper() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("bin").join("worker.rs"),
        "fn main() {}\n",
    )
    .unwrap();
    fs::write(root.join("src").join("index.js"), "console.log('start');\n").unwrap();
    fs::write(
        root.join("src").join("main.c"),
        "int main(void) { return 0; }\n",
    )
    .unwrap();
    fs::write(root.join("main.go"), "package main\nfunc main() {}\n").unwrap();
    fs::write(
        root.join("cmd").join("server").join("main.go"),
        "package main\nfunc main() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("codegraph").join("cli.py"),
        "def main():\n    pass\n",
    )
    .unwrap();
    fs::write(
        root.join("bin").join("codegraph"),
        "#!/usr/bin/env php\n<?php\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"

[[bin]]
name = "worker"
path = "src/bin/worker.rs"
"#,
    )
    .unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
  "scripts": {
    "start": "node src/index.js",
    "test": "vitest"
  }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("pyproject.toml"),
        r#"[project.scripts]
cg = "codegraph.cli:main"
"#,
    )
    .unwrap();
    fs::write(
        root.join("setup.py"),
        r#"from setuptools import setup

setup(
    name="legacy-demo",
    entry_points={
        "console_scripts": [
            "legacy = codegraph.cli:main",
        ],
    },
)
"#,
    )
    .unwrap();
    fs::write(
        root.join("setup.cfg"),
        r#"[metadata]
name = legacy-cfg-demo

[options.entry_points]
console_scripts =
    cfglegacy = codegraph.cli:main
"#,
    )
    .unwrap();
    fs::write(
        root.join("composer.json"),
        r#"{
  "bin": ["bin/codegraph"],
  "scripts": {
    "analyse": "phpstan analyse"
  }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("CMakeLists.txt"),
        r#"# comment add_executable(nope src/nope.c)
cmake_minimum_required(VERSION 3.20)
project(demo_c)
add_executable(demo_c
  src/main.c
  src/extra.c
)
add_executable(alias_target ALIAS demo_c)
add_executable(imported_tool IMPORTED)
"#,
    )
    .unwrap();
    fs::write(root.join("go.mod"), "module example.com/demo\n\ngo 1.23\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let entrypoints: BTreeSet<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Entrypoint)
        .map(|node| node.label.as_str())
        .collect();

    for expected in [
        "cargo bin:demo",
        "cargo binary:worker",
        "npm script:start",
        "npm script:test",
        "python console_script:cg",
        "python console_script:legacy",
        "python console_script:cfglegacy",
        "composer bin:bin/codegraph",
        "composer script:analyse",
        "cmake executable:demo_c",
        "go module:example.com/demo",
        "go command:server",
    ] {
        assert!(entrypoints.contains(expected), "missing {expected}");
    }
    assert!(!entrypoints.contains("cmake executable:alias_target"));
    assert!(!entrypoints.contains("cmake executable:imported_tool"));
    assert!(!entrypoints.contains("cmake executable:nope"));
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Entrypoint && edge.confidence == Confidence::Exact
        })
    );
    assert!(graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Entrypoint
            && node.label == "npm script:start"
            && node
                .metadata
                .get("target")
                .is_some_and(|value| value == "node src/index.js")
    }));
    let cargo_entrypoint = node_id(&graph, NodeKind::Entrypoint, "cargo bin:demo");
    let cargo_file = node_id(&graph, NodeKind::File, "src/main.rs");
    let cargo_main = function_id_in_file(&graph, "main", "src/main.rs");
    let npm_entrypoint = node_id(&graph, NodeKind::Entrypoint, "npm script:start");
    let npm_file = node_id(&graph, NodeKind::File, "src/index.js");
    let python_entrypoint = node_id(&graph, NodeKind::Entrypoint, "python console_script:cg");
    let setup_py_entrypoint = node_id(&graph, NodeKind::Entrypoint, "python console_script:legacy");
    let setup_cfg_entrypoint = node_id(
        &graph,
        NodeKind::Entrypoint,
        "python console_script:cfglegacy",
    );
    let python_main = function_id_in_file(&graph, "main", "codegraph/cli.py");
    let composer_entrypoint = node_id(&graph, NodeKind::Entrypoint, "composer bin:bin/codegraph");
    let composer_file = node_id(&graph, NodeKind::File, "bin/codegraph");
    let cmake_entrypoint = node_id(&graph, NodeKind::Entrypoint, "cmake executable:demo_c");
    let cmake_file = node_id(&graph, NodeKind::File, "src/main.c");
    let cmake_main = function_id_in_file(&graph, "main", "src/main.c");
    let go_module_entrypoint = node_id(&graph, NodeKind::Entrypoint, "go module:example.com/demo");
    let go_module_file = node_id(&graph, NodeKind::File, "main.go");
    let go_module_main = function_id_in_file(&graph, "main", "main.go");
    let go_command_entrypoint = node_id(&graph, NodeKind::Entrypoint, "go command:server");
    let go_command_file = node_id(&graph, NodeKind::File, "cmd/server/main.go");
    let go_command_main = function_id_in_file(&graph, "main", "cmd/server/main.go");

    assert!(has_entrypoint_reference(
        &graph,
        cargo_entrypoint,
        cargo_file,
        "entrypoint_file",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        cargo_entrypoint,
        cargo_main,
        "entrypoint_function",
        Confidence::Syntactic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        npm_entrypoint,
        npm_file,
        "entrypoint_file",
        Confidence::Heuristic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        python_entrypoint,
        python_main,
        "entrypoint_function",
        Confidence::Heuristic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        setup_py_entrypoint,
        python_main,
        "entrypoint_function",
        Confidence::Heuristic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        setup_cfg_entrypoint,
        python_main,
        "entrypoint_function",
        Confidence::Heuristic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        composer_entrypoint,
        composer_file,
        "entrypoint_file",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        cmake_entrypoint,
        cmake_file,
        "entrypoint_file",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        cmake_entrypoint,
        cmake_main,
        "entrypoint_function",
        Confidence::Syntactic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        go_module_entrypoint,
        go_module_file,
        "entrypoint_file",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        go_module_entrypoint,
        go_module_main,
        "entrypoint_function",
        Confidence::Syntactic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        go_command_entrypoint,
        go_command_file,
        "entrypoint_file",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        go_command_entrypoint,
        go_command_main,
        "entrypoint_function",
        Confidence::Syntactic,
    ));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_makefile_target_entrypoints() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("Makefile"),
        r#".PHONY: build test deploy
IMAGE := demo

build test: ## grouped task targets
	cargo test --workspace

deploy:
	@./scripts/deploy.sh --prod

generated/output.txt:
	echo generated > generated/output.txt

%.o: %.c
	$(CC) -c $<
"#,
    )
    .unwrap();
    fs::write(
        root.join("scripts").join("deploy.sh"),
        "#!/usr/bin/env bash\nmain() { echo deploy; }\nmain \"$@\"\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let makefile = node_id(&graph, NodeKind::File, "Makefile");
    let deploy_script = node_id(&graph, NodeKind::File, "scripts/deploy.sh");
    let build_entrypoint = node_id(&graph, NodeKind::Entrypoint, "make target:build");
    let test_entrypoint = node_id(&graph, NodeKind::Entrypoint, "make target:test");
    let deploy_entrypoint = node_id(&graph, NodeKind::Entrypoint, "make target:deploy");

    for entrypoint in [build_entrypoint, test_entrypoint, deploy_entrypoint] {
        assert!(has_entrypoint_reference(
            &graph,
            entrypoint,
            makefile,
            "entrypoint_file",
            Confidence::Exact,
        ));
    }
    assert!(has_entrypoint_reference(
        &graph,
        deploy_entrypoint,
        deploy_script,
        "entrypoint_file",
        Confidence::Heuristic,
    ));

    let deploy = graph
        .nodes
        .iter()
        .find(|node| node.id == deploy_entrypoint)
        .expect("missing deploy make target");
    assert_eq!(
        deploy.metadata.get("item_kind").map(String::as_str),
        Some("makefile_target")
    );
    assert_eq!(
        deploy.metadata.get("source").map(String::as_str),
        Some("makefile")
    );
    assert_eq!(
        deploy.metadata.get("command").map(String::as_str),
        Some("./scripts/deploy.sh --prod")
    );
    assert_eq!(
        deploy.metadata.get("command_path").map(String::as_str),
        Some("scripts/deploy.sh")
    );
    assert!(!graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Entrypoint && node.label == "make target:generated/output.txt"
    }));
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| { node.kind == NodeKind::Entrypoint && node.label == "make target:%.o" })
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_dockerfile_entrypoints() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("docker")).unwrap();
    fs::write(
            root.join("Dockerfile"),
            "FROM debian:stable-slim\nENTRYPOINT [\"/bin/sh\", \"-c\", \"./docker/start.sh --serve\"]\nCMD ./docker/migrate.sh\n",
        )
        .unwrap();
    fs::write(
        root.join("docker").join("start.sh"),
        "#!/usr/bin/env bash\necho start\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let dockerfile = node_id(&graph, NodeKind::File, "Dockerfile");
    let start_script = node_id(&graph, NodeKind::File, "docker/start.sh");
    let entrypoint = node_id(
        &graph,
        NodeKind::Entrypoint,
        "docker entrypoint:/bin/sh -c ./docker/start.sh --serve",
    );
    let cmd = node_id(
        &graph,
        NodeKind::Entrypoint,
        "docker cmd:./docker/migrate.sh",
    );

    assert!(has_entrypoint_reference(
        &graph,
        entrypoint,
        dockerfile,
        "entrypoint_file",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        entrypoint,
        start_script,
        "entrypoint_file",
        Confidence::Heuristic,
    ));

    let entrypoint_node = graph
        .nodes
        .iter()
        .find(|node| node.id == entrypoint)
        .expect("missing Dockerfile entrypoint");
    assert_eq!(
        entrypoint_node
            .metadata
            .get("item_kind")
            .map(String::as_str),
        Some("dockerfile_entrypoint")
    );
    assert_eq!(
        entrypoint_node
            .metadata
            .get("command_path")
            .map(String::as_str),
        Some("docker/start.sh")
    );
    let cmd_node = graph
        .nodes
        .iter()
        .find(|node| node.id == cmd)
        .expect("missing Dockerfile CMD");
    assert_eq!(
        cmd_node.metadata.get("command_path").map(String::as_str),
        Some("docker/migrate.sh")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_compose_service_entrypoints() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("data")).unwrap();
    fs::write(
        root.join("docker-compose.yml"),
        r#"services:
  web:
    build:
      context: .
      dockerfile: Dockerfile
    command: ["./scripts/start.sh", "--serve"]
    env_file:
      - config/web.env
    environment:
      APP_ENV: production
      DATABASE_URL:
    ports:
      - "8080:80"
    volumes:
      - ./config:/app/config:ro
      - worker.env:/app/worker.env
    depends_on:
      - db
  worker:
    image: demo/worker
    entrypoint: ./scripts/worker.sh
    env_file: [worker.env]
    environment: [WORKER_TOKEN, QUEUE=critical]
    ports:
      - target: 9000
        published: "19000"
        protocol: udp
    volumes:
      - type: bind
        source: ./scripts
        target: /app/scripts
        read_only: true
    depends_on:
      db:
        condition: service_healthy
  db:
    image: postgres:16
    volumes:
      - db-data:/var/lib/postgresql/data
"#,
    )
    .unwrap();
    fs::write(root.join("Dockerfile"), "FROM debian:stable-slim\n").unwrap();
    fs::write(
        root.join("config").join("web.env"),
        "DATABASE_URL=postgres\n",
    )
    .unwrap();
    fs::write(root.join("worker.env"), "QUEUE=critical\n").unwrap();
    fs::write(
        root.join("scripts").join("start.sh"),
        "#!/usr/bin/env bash\necho start\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts").join("worker.sh"),
        "#!/usr/bin/env bash\necho worker\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let compose = node_id(&graph, NodeKind::File, "docker-compose.yml");
    let dockerfile = node_id(&graph, NodeKind::File, "Dockerfile");
    let config_dir = node_id(&graph, NodeKind::Directory, "config");
    let scripts_dir = node_id(&graph, NodeKind::Directory, "scripts");
    let web_env_file = node_id(&graph, NodeKind::File, "config/web.env");
    let worker_env_file = node_id(&graph, NodeKind::File, "worker.env");
    let start_script = node_id(&graph, NodeKind::File, "scripts/start.sh");
    let worker_script = node_id(&graph, NodeKind::File, "scripts/worker.sh");
    let web = node_id(&graph, NodeKind::Entrypoint, "compose service:web");
    let worker = node_id(&graph, NodeKind::Entrypoint, "compose service:worker");
    let db = node_id(&graph, NodeKind::Entrypoint, "compose service:db");
    let app_env = node_id(&graph, NodeKind::Environment, "APP_ENV");
    let database_url = node_id(&graph, NodeKind::Environment, "DATABASE_URL");
    let worker_token = node_id(&graph, NodeKind::Environment, "WORKER_TOKEN");
    let queue = node_id(&graph, NodeKind::Environment, "QUEUE");
    let web_env_config = node_id(&graph, NodeKind::Config, "compose env file:config/web.env");
    let worker_env_config = node_id(&graph, NodeKind::Config, "compose env file:worker.env");
    let web_port = node_id(&graph, NodeKind::Config, "compose port:8080->80/tcp");
    let worker_port = node_id(&graph, NodeKind::Config, "compose port:19000->9000/udp");
    let web_config_volume = node_id(
        &graph,
        NodeKind::Config,
        "compose volume:./config->/app/config",
    );
    let web_file_volume = node_id(
        &graph,
        NodeKind::Config,
        "compose volume:worker.env->/app/worker.env",
    );
    let worker_scripts_volume = node_id(
        &graph,
        NodeKind::Config,
        "compose volume:./scripts->/app/scripts",
    );
    let db_named_volume = node_id(
        &graph,
        NodeKind::Config,
        "compose volume:db-data->/var/lib/postgresql/data",
    );

    for service in [web, worker, db] {
        assert!(has_entrypoint_reference(
            &graph,
            service,
            compose,
            "entrypoint_file",
            Confidence::Exact,
        ));
    }
    assert!(has_entrypoint_reference(
        &graph,
        web,
        start_script,
        "entrypoint_file",
        Confidence::Heuristic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        worker,
        worker_script,
        "entrypoint_file",
        Confidence::Heuristic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        web,
        dockerfile,
        "entrypoint_file",
        Confidence::Exact,
    ));
    for (service, environment) in [
        (web, app_env),
        (web, database_url),
        (worker, worker_token),
        (worker, queue),
    ] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == service
                && edge.target == environment
                && edge.kind == EdgeKind::ReadsEnvironment
                && edge.confidence == Confidence::Exact
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "compose_environment")
        }));
    }
    for (config, file) in [
        (web_env_config, web_env_file),
        (worker_env_config, worker_env_file),
    ] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == config
                && edge.target == file
                && edge.kind == EdgeKind::References
                && edge.confidence == Confidence::Exact
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "compose_env_file_path")
        }));
    }
    assert!(graph.edges.iter().any(|edge| {
        edge.source == web
            && edge.target == web_env_config
            && edge.kind == EdgeKind::ReadsConfig
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "compose_env_file")
    }));
    for (service, port) in [(web, web_port), (worker, worker_port)] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == service
                && edge.target == port
                && edge.kind == EdgeKind::References
                && edge.confidence == Confidence::Exact
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "compose_port")
        }));
    }
    for (service, volume) in [
        (web, web_config_volume),
        (web, web_file_volume),
        (worker, worker_scripts_volume),
        (db, db_named_volume),
    ] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == service
                && edge.target == volume
                && edge.kind == EdgeKind::References
                && edge.confidence == Confidence::Exact
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "compose_volume")
        }));
    }
    for (volume, target) in [
        (web_config_volume, config_dir),
        (web_file_volume, worker_env_file),
        (worker_scripts_volume, scripts_dir),
    ] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == volume
                && edge.target == target
                && edge.kind == EdgeKind::References
                && edge.confidence == Confidence::Exact
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "compose_volume_source_path")
        }));
    }
    assert!(graph.edges.iter().any(|edge| {
        edge.source == web
            && edge.target == db
            && edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "compose_service_depends_on")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == worker
            && edge.target == db
            && edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "compose_service_depends_on")
    }));

    let web_node = graph
        .nodes
        .iter()
        .find(|node| node.id == web)
        .expect("missing compose web service");
    assert_eq!(
        web_node.metadata.get("item_kind").map(String::as_str),
        Some("compose_service")
    );
    assert_eq!(
        web_node.metadata.get("command_path").map(String::as_str),
        Some("scripts/start.sh")
    );
    assert_eq!(
        web_node.metadata.get("dockerfile").map(String::as_str),
        Some("Dockerfile")
    );
    assert_eq!(
        web_node
            .metadata
            .get("environment_count")
            .map(String::as_str),
        Some("2")
    );
    assert_eq!(
        web_node.metadata.get("env_file_count").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        web_node.metadata.get("port_count").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        web_node.metadata.get("volume_count").map(String::as_str),
        Some("2")
    );
    let web_port_node = graph
        .nodes
        .iter()
        .find(|node| node.id == web_port)
        .expect("missing web compose port");
    assert_eq!(
        web_port_node
            .metadata
            .get("published_port")
            .map(String::as_str),
        Some("8080")
    );
    assert_eq!(
        web_port_node
            .metadata
            .get("target_port")
            .map(String::as_str),
        Some("80")
    );
    assert_eq!(
        web_port_node.metadata.get("protocol").map(String::as_str),
        Some("tcp")
    );
    let web_config_volume_node = graph
        .nodes
        .iter()
        .find(|node| node.id == web_config_volume)
        .expect("missing web config compose volume");
    assert_eq!(
        web_config_volume_node
            .metadata
            .get("local_source_path")
            .map(String::as_str),
        Some("config")
    );
    assert_eq!(
        web_config_volume_node
            .metadata
            .get("read_only")
            .map(String::as_str),
        Some("true")
    );
    assert!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == db_named_volume)
            .and_then(|node| node.metadata.get("local_source_path"))
            .is_none()
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == app_env)
            .and_then(|node| node.metadata.get("value_present"))
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == worker_token)
            .and_then(|node| node.metadata.get("value_source"))
            .map(String::as_str),
        Some("host")
    );
    let db_node = graph
        .nodes
        .iter()
        .find(|node| node.id == db)
        .expect("missing compose db service");
    assert!(!db_node.metadata.contains_key("dockerfile"));
    assert!(!has_entrypoint_reference(
        &graph,
        db,
        dockerfile,
        "entrypoint_file",
        Confidence::Exact,
    ));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_github_actions_workflow_entrypoints() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".github").join("workflows")).unwrap();
    fs::create_dir_all(root.join(".github").join("actions").join("setup")).unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join(".github").join("workflows").join("ci.yml"),
        r#"name: CI
on: [push]
env:
  GLOBAL_TOKEN: ${{ secrets.GLOBAL_TOKEN }}
  RUNNER_FLAG:

jobs:
  build:
    name: Build and test
    runs-on: ubuntu-latest
    env:
      BUILD_MODE: ci
      OPTIONAL_FLAG:
    steps:
      - uses: actions/checkout@v4
      - name: Setup
        uses: ./.github/actions/setup
      - run: ./scripts/test.sh --ci
  deploy:
    needs: [build]
    steps:
      - uses: ./.github/actions/missing
"#,
    )
    .unwrap();
    fs::write(
        root.join(".github")
            .join("actions")
            .join("setup")
            .join("action.yml"),
        "name: setup\nruns:\n  using: composite\n  steps: []\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts").join("test.sh"),
        "#!/usr/bin/env bash\necho test\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let workflow_file = node_id(&graph, NodeKind::File, ".github/workflows/ci.yml");
    let setup_action_dir = node_id(&graph, NodeKind::Directory, ".github/actions/setup");
    let test_script = node_id(&graph, NodeKind::File, "scripts/test.sh");
    let build = node_id(&graph, NodeKind::Entrypoint, "github workflow:CI/build");
    let deploy = node_id(&graph, NodeKind::Entrypoint, "github workflow:CI/deploy");
    let checkout = node_id(
        &graph,
        NodeKind::ExternalDependency,
        "github action:actions/checkout",
    );
    let setup_action = node_id(
        &graph,
        NodeKind::Config,
        "github action:.github/actions/setup",
    );
    let missing_action = node_id(
        &graph,
        NodeKind::Config,
        "github action:.github/actions/missing",
    );
    let run_step = node_id(&graph, NodeKind::Config, "github run:CI/build/18");
    let global_token = node_id(&graph, NodeKind::Environment, "GLOBAL_TOKEN");
    let build_mode = node_id(&graph, NodeKind::Environment, "BUILD_MODE");

    for job in [build, deploy] {
        assert!(has_entrypoint_reference(
            &graph,
            job,
            workflow_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
    }
    assert!(has_entrypoint_reference(
        &graph,
        build,
        test_script,
        "entrypoint_file",
        Confidence::Heuristic,
    ));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == deploy
            && edge.target == build
            && edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "github_actions_needs")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == build
            && edge.target == checkout
            && edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "github_actions_uses")
            && edge
                .metadata
                .get("version")
                .is_some_and(|value| value == "v4")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == setup_action
            && edge.target == setup_action_dir
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "github_actions_local_action_path")
    }));
    assert!(!graph.edges.iter().any(|edge| {
        edge.source == missing_action
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "github_actions_local_action_path")
    }));

    let build_node = graph
        .nodes
        .iter()
        .find(|node| node.id == build)
        .expect("missing build workflow job");
    assert_eq!(
        build_node.metadata.get("item_kind").map(String::as_str),
        Some("github_actions_job")
    );
    assert_eq!(
        build_node.metadata.get("step_count").map(String::as_str),
        Some("3")
    );
    assert_eq!(
        build_node.metadata.get("uses_count").map(String::as_str),
        Some("2")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == deploy)
            .and_then(|node| node.metadata.get("needs"))
            .map(String::as_str),
        Some("build")
    );
    assert_eq!(
        build_node
            .metadata
            .get("environment_count")
            .map(String::as_str),
        Some("4")
    );
    for environment in [global_token, build_mode] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == build
                && edge.target == environment
                && edge.kind == EdgeKind::ReadsEnvironment
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "ci_environment")
        }));
    }
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == global_token)
            .and_then(|node| node.metadata.get("value_kind"))
            .map(String::as_str),
        Some("secret_reference")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == build_mode)
            .and_then(|node| node.metadata.get("value_kind"))
            .map(String::as_str),
        Some("literal")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == run_step)
            .and_then(|node| node.metadata.get("command_path"))
            .map(String::as_str),
        Some("scripts/test.sh")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_gitlab_ci_job_entrypoints() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join(".gitlab-ci.yml"),
        r#"stages:
  - build
  - test
  - deploy

variables:
  GLOBAL_URL: https://example.test
  EMPTY_VAR:

.base:
  script:
    - echo template

build:
  stage: build
  image: rust:1.78
  script:
    - ./scripts/build.sh
    - cargo test

test:
  stage: test
  needs: [build]
  dependencies:
    - build
  variables:
    TEST_MODE: ci
    SECRET_TOKEN:
  script: ["./scripts/test.sh", "cargo clippy"]

deploy:
  stage: deploy
  needs:
    - job: test
  script:
    - ./scripts/missing.sh
"#,
    )
    .unwrap();
    fs::write(
        root.join("scripts").join("build.sh"),
        "#!/usr/bin/env bash\necho build\n",
    )
    .unwrap();
    fs::write(
        root.join("scripts").join("test.sh"),
        "#!/usr/bin/env bash\necho test\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let gitlab_file = node_id(&graph, NodeKind::File, ".gitlab-ci.yml");
    let build_script = node_id(&graph, NodeKind::File, "scripts/build.sh");
    let test_script_file = node_id(&graph, NodeKind::File, "scripts/test.sh");
    let build = node_id(&graph, NodeKind::Entrypoint, "gitlab job:build");
    let test = node_id(&graph, NodeKind::Entrypoint, "gitlab job:test");
    let deploy = node_id(&graph, NodeKind::Entrypoint, "gitlab job:deploy");
    let build_script_fact = node_id(&graph, NodeKind::Config, "gitlab script:build/18#1");
    let test_script_fact = node_id(&graph, NodeKind::Config, "gitlab script:test/29#1");
    let deploy_script_fact = node_id(&graph, NodeKind::Config, "gitlab script:deploy/36#1");

    for job in [build, test, deploy] {
        assert!(has_entrypoint_reference(
            &graph,
            job,
            gitlab_file,
            "entrypoint_file",
            Confidence::Exact,
        ));
    }
    assert!(has_entrypoint_reference(
        &graph,
        build,
        build_script,
        "entrypoint_file",
        Confidence::Heuristic,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        test,
        test_script_file,
        "entrypoint_file",
        Confidence::Heuristic,
    ));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == test
            && edge.target == build
            && edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "gitlab_ci_needs")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == deploy
            && edge.target == test
            && edge.kind == EdgeKind::DependsOn
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "gitlab_ci_needs")
    }));
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| { node.kind == NodeKind::Entrypoint && node.label == "gitlab job:.base" })
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == build)
            .and_then(|node| node.metadata.get("image"))
            .map(String::as_str),
        Some("rust:1.78")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == test)
            .and_then(|node| node.metadata.get("needs"))
            .map(String::as_str),
        Some("build")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == test)
            .and_then(|node| node.metadata.get("environment_count"))
            .map(String::as_str),
        Some("4")
    );
    for environment_label in ["GLOBAL_URL", "TEST_MODE"] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == test
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.kind == NodeKind::Environment
                        && node.label == environment_label
                })
                && edge.kind == EdgeKind::ReadsEnvironment
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "ci_environment")
        }));
    }
    assert!(graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Environment
            && node.label == "TEST_MODE"
            && node
                .metadata
                .get("value_kind")
                .is_some_and(|value| value == "literal")
    }));
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == test)
            .and_then(|node| node.metadata.get("dependencies"))
            .map(String::as_str),
        Some("build")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == build_script_fact)
            .and_then(|node| node.metadata.get("command_path"))
            .map(String::as_str),
        Some("scripts/build.sh")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == test_script_fact)
            .and_then(|node| node.metadata.get("command_path"))
            .map(String::as_str),
        Some("scripts/test.sh")
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == deploy_script_fact)
            .and_then(|node| node.metadata.get("command_path"))
            .map(String::as_str),
        Some("scripts/missing.sh")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_kubernetes_runtime_config_refs() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("k8s")).unwrap();
    fs::write(
        root.join("k8s").join("app.yaml"),
        r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: app-config
  namespace: prod
data:
  APP_ENV: production
---
apiVersion: v1
kind: Secret
metadata:
  name: app-secret
  namespace: prod
stringData:
  token: demo
---
apiVersion: v1
kind: Service
metadata:
  name: web
  namespace: prod
spec:
  selector:
    app: web
    tier: frontend
  ports:
    - name: http
      port: 80
      targetPort: 8080
      protocol: TCP
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: web
  namespace: prod
spec:
  rules:
    - host: example.test
      http:
        paths:
          - path: /api
            pathType: Prefix
            backend:
              service:
                name: web
                port:
                  number: 80
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  namespace: prod
  labels:
    app.kubernetes.io/name: web
spec:
  selector:
    matchLabels:
      app: web
      tier: frontend
  template:
    metadata:
      labels:
        app: web
        tier: frontend
    spec:
      containers:
        - name: web
          image: demo/web
          envFrom:
            - configMapRef:
                name: app-config
            - secretRef: { name: app-secret }
          env:
            - name: API_TOKEN
              valueFrom:
                secretKeyRef:
                  name: app-secret
                  key: token
            - name: MISSING
              valueFrom:
                configMapKeyRef:
                  name: missing-config
                  key: value
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let manifest = node_id(&graph, NodeKind::File, "k8s/app.yaml");
    let deployment = node_id(&graph, NodeKind::Entrypoint, "k8s deployment:prod/web");
    let configmap = node_id(&graph, NodeKind::Config, "k8s configmap:prod/app-config");
    let secret = node_id(&graph, NodeKind::Config, "k8s secret:prod/app-secret");
    let service = node_id(&graph, NodeKind::Config, "k8s service:prod/web");
    let ingress = node_id(&graph, NodeKind::Entrypoint, "k8s ingress:prod/web");
    let service_ref = node_id(&graph, NodeKind::Config, "k8s service ref:prod/web");
    let service_port = node_id(
        &graph,
        NodeKind::Config,
        "k8s service port:prod/web:80->8080/TCP",
    );
    let app_config_ref = node_id(
        &graph,
        NodeKind::Config,
        "k8s config ref:configmap prod/app-config",
    );
    let app_secret_ref = node_id(
        &graph,
        NodeKind::Config,
        "k8s config ref:secret prod/app-secret",
    );
    let missing_config_ref = node_id(
        &graph,
        NodeKind::Config,
        "k8s config ref:configmap prod/missing-config",
    );

    assert!(graph.edges.iter().any(|edge| {
        edge.source == graph.root
            && edge.target == deployment
            && edge.kind == EdgeKind::Entrypoint
            && edge.confidence == Confidence::Exact
    }));
    for node in [deployment, configmap, secret, service, ingress] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == manifest
                && edge.target == node
                && edge.kind == EdgeKind::Contains
                && edge.confidence == Confidence::Exact
        }));
    }
    assert!(graph.edges.iter().any(|edge| {
        edge.source == deployment
            && edge.target == manifest
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "kubernetes_manifest")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == graph.root
            && edge.target == ingress
            && edge.kind == EdgeKind::Entrypoint
            && edge.confidence == Confidence::Exact
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == ingress
            && edge.target == service_ref
            && edge.kind == EdgeKind::References
            && edge.confidence == Confidence::Exact
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "kubernetes_ingress_backend")
            && edge
                .metadata
                .get("path")
                .is_some_and(|value| value == "/api")
            && edge
                .metadata
                .get("host")
                .is_some_and(|value| value == "example.test")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == service_ref
            && edge.target == service
            && edge.kind == EdgeKind::References
            && edge.confidence == Confidence::Exact
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "kubernetes_service_ref")
    }));
    for config_ref in [app_config_ref, app_secret_ref, missing_config_ref] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == deployment
                && edge.target == config_ref
                && edge.kind == EdgeKind::ReadsConfig
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "kubernetes_config_ref")
        }));
    }
    for (config_ref, config) in [(app_config_ref, configmap), (app_secret_ref, secret)] {
        assert!(graph.edges.iter().any(|edge| {
            edge.source == config_ref
                && edge.target == config
                && edge.kind == EdgeKind::References
                && edge.confidence == Confidence::Exact
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "kubernetes_config_ref")
        }));
    }
    assert!(!graph.edges.iter().any(|edge| {
        edge.source == missing_config_ref
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "kubernetes_config_ref")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == service
            && edge.target == service_port
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "kubernetes_service_port")
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == service
            && edge.target == deployment
            && edge.kind == EdgeKind::References
            && edge.confidence == Confidence::Exact
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "kubernetes_service_selector")
            && edge
                .metadata
                .get("selector")
                .is_some_and(|value| value == "app=web,tier=frontend")
    }));
    let deployment_node = graph
        .nodes
        .iter()
        .find(|node| node.id == deployment)
        .expect("missing Kubernetes deployment");
    assert_eq!(
        deployment_node
            .metadata
            .get("config_ref_count")
            .map(String::as_str),
        Some("4")
    );
    assert_eq!(
        deployment_node
            .metadata
            .get("container_count")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        deployment_node
            .metadata
            .get("pod_labels")
            .map(String::as_str),
        Some("app=web,tier=frontend")
    );
    let service_node = graph
        .nodes
        .iter()
        .find(|node| node.id == service)
        .expect("missing Kubernetes service");
    assert_eq!(
        service_node.metadata.get("selector").map(String::as_str),
        Some("app=web,tier=frontend")
    );
    let ingress_node = graph
        .nodes
        .iter()
        .find(|node| node.id == ingress)
        .expect("missing Kubernetes ingress");
    assert_eq!(
        ingress_node
            .metadata
            .get("backend_count")
            .map(String::as_str),
        Some("1")
    );
    let service_ref_node = graph
        .nodes
        .iter()
        .find(|node| node.id == service_ref)
        .expect("missing Kubernetes service ref");
    assert_eq!(
        service_ref_node
            .metadata
            .get("service_port")
            .map(String::as_str),
        Some("80")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_shebang_script_entrypoints() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(
        root.join("bin").join("deploy"),
        "#!/usr/bin/env bash\nmain() { echo deploy; }\nmain \"$@\"\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let script_entrypoint = node_id(&graph, NodeKind::Entrypoint, "script:bin/deploy");
    let script_file = node_id(&graph, NodeKind::File, "bin/deploy");
    let script_main = function_id_in_file(&graph, "main", "bin/deploy");
    let entrypoint = graph
        .nodes
        .iter()
        .find(|node| node.id == script_entrypoint)
        .expect("missing script entrypoint");

    assert_eq!(
        entrypoint.metadata.get("item_kind").map(String::as_str),
        Some("script_entrypoint")
    );
    assert_eq!(
        entrypoint.metadata.get("interpreter").map(String::as_str),
        Some("bash")
    );
    assert!(has_entrypoint_reference(
        &graph,
        script_entrypoint,
        script_file,
        "entrypoint_file",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        script_entrypoint,
        script_main,
        "entrypoint_function",
        Confidence::Syntactic,
    ));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn plain_text_documents_join_the_graph_with_provenance() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        root.join("NOTES.txt"),
        "Operational notes.\nThe entrypoint lives in src/main.rs and the design in docs/design.md.\n",
    )
    .unwrap();
    fs::write(root.join("docs").join("design.md"), "# Design\n").unwrap();
    fs::write(
        root.join("docs").join("report.pdf.md"),
        "# Generated transcript\nSee src/main.rs.\n",
    )
    .unwrap();
    // Manifest-convention txt files must stay manifests, not documents.
    fs::write(root.join("requirements.txt"), "fastapi==0.100\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    let notes = graph
        .nodes
        .iter()
        .find(|node| node.label == "NOTES.txt")
        .expect("NOTES.txt file node");
    assert_eq!(
        notes.metadata.get("item_kind").map(String::as_str),
        Some("document")
    );
    assert_eq!(
        notes.metadata.get("document_kind").map(String::as_str),
        Some("plain_text")
    );
    assert_eq!(
        notes.metadata.get("line_count").map(String::as_str),
        Some("2")
    );

    let main_file = node_id(&graph, NodeKind::File, "src/main.rs");
    let design = node_id(&graph, NodeKind::File, "docs/design.md");
    for target in [main_file, design] {
        assert!(
            graph.edges.iter().any(|edge| {
                edge.source == notes.id
                    && edge.target == target
                    && edge.metadata.get("relation").map(String::as_str) == Some("document_path")
            }),
            "NOTES.txt should reference scanned file {target}"
        );
    }

    let sidecar = graph
        .nodes
        .iter()
        .find(|node| node.label == "docs/report.pdf.md")
        .expect("sidecar file node");
    assert_eq!(
        sidecar.metadata.get("generated").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        sidecar.metadata.get("sidecar_of").map(String::as_str),
        Some("docs/report.pdf")
    );

    let requirements = graph
        .nodes
        .iter()
        .find(|node| node.label == "requirements.txt")
        .expect("requirements file node");
    assert_ne!(
        requirements
            .metadata
            .get("document_kind")
            .map(String::as_str),
        Some("plain_text"),
        "manifest txt files must not become documents"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_imports_link_to_manifest_package_hubs() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde_json = \"1\"\n",
    )
    .unwrap();
    fs::write(root.join("src").join("lib.rs"), "use serde_json::Value;\n").unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"demo\",\"dependencies\":{\"express\":\"^4\"}}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("app.js"),
        "import express from 'express';\n",
    )
    .unwrap();
    fs::write(root.join("requirements.txt"), "FastAPI==0.100\n").unwrap();
    fs::write(root.join("src").join("main.py"), "import fastapi\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();

    for (package_id, import_needle) in [
        ("cargo:serde_json", "use serde_json"),
        ("npm:express", "import express"),
        ("python:fastapi", "import fastapi"),
    ] {
        let hub = graph
            .nodes
            .iter()
            .find(|node| {
                node.metadata.get("package_id").map(String::as_str) == Some(package_id)
                    && node.metadata.get("item_kind").map(String::as_str) == Some("dependency")
            })
            .unwrap_or_else(|| panic!("missing package hub {package_id}"));
        let import = graph
            .nodes
            .iter()
            .find(|node| {
                node.label.contains(import_needle)
                    && node.metadata.get("item_kind").map(String::as_str) == Some("import")
            })
            .unwrap_or_else(|| panic!("missing import fact for {import_needle}"));
        assert_eq!(
            import.metadata.get("package_id").map(String::as_str),
            Some(package_id),
            "import {import_needle} should carry the hub package id"
        );
        assert!(
            graph.edges.iter().any(|edge| {
                edge.source == import.id
                    && edge.target == hub.id
                    && edge.kind == EdgeKind::DependsOn
                    && edge.metadata.get("relation").map(String::as_str) == Some("package_import")
            }),
            "import {import_needle} should link to hub {package_id}"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cross_module_route_handlers_resolve_through_function_registry() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "use axum::{routing::get, Router};\nmod handlers;\nfn app() -> Router {\n    Router::new().route(\"/status\", get(status_handler))\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("handlers.rs"),
        "pub async fn status_handler() {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let entrypoint = node_id(&graph, NodeKind::Entrypoint, "route GET /status");
    let handler = function_id_in_file(&graph, "status_handler", "src/handlers.rs");
    assert!(
        has_entrypoint_reference(
            &graph,
            entrypoint,
            handler,
            "entrypoint_function",
            Confidence::Heuristic,
        ),
        "route handler in another module should resolve through the global function registry"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_framework_route_entrypoints() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
            root.join("api.py"),
            "from fastapi import FastAPI\napp = FastAPI()\n\n@app.get(\"/health\")\ndef health():\n    return {}\n",
        )
        .unwrap();
    fs::write(
            root.join("server.js"),
            "const express = require('express');\nconst app = express();\nfunction listUsers() {}\napp.post('/users', listUsers);\n",
        )
        .unwrap();
    fs::write(
            root.join("main.go"),
            "package main\nimport \"net/http\"\nfunc health(w http.ResponseWriter, r *http.Request) {}\nfunc main() { http.HandleFunc(\"/ready\", health) }\n",
        )
        .unwrap();
    fs::write(
            root.join("router.rs"),
            "use axum::{routing::{get, post}, Router};\nasync fn status() {}\nasync fn create_account() {}\nfn app() -> Router {\n    let literal = \".route(\";\n    Router::new()\n        .route(\n            \"/status\",\n            get(status),\n        )\n        .route(\"/accounts\", post(create_account))\n}\n",
        )
        .unwrap();
    fs::write(
        root.join("Controller.php"),
        "<?php\n#[Route('/admin', methods: ['GET'])]\nfunction admin() {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let expected = [
        ("route GET /health", "health", "api.py", "fastapi"),
        ("route POST /users", "listUsers", "server.js", "express"),
        ("route ROUTE /ready", "health", "main.go", "net/http"),
        ("route GET /status", "status", "router.rs", "axum"),
        (
            "route POST /accounts",
            "create_account",
            "router.rs",
            "axum",
        ),
        (
            "route GET /admin",
            "admin",
            "Controller.php",
            "php-attribute",
        ),
    ];

    for (route_label, handler, path, framework) in expected {
        let entrypoint = node_id(&graph, NodeKind::Entrypoint, route_label);
        let file = node_id(&graph, NodeKind::File, path);
        let handler_id = function_id_in_file(&graph, handler, path);
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == entrypoint)
            .expect("missing route entrypoint");

        assert_eq!(
            node.metadata.get("item_kind").map(String::as_str),
            Some("framework_route")
        );
        assert_eq!(
            node.metadata.get("framework").map(String::as_str),
            Some(framework)
        );
        assert!(
            node.span
                .as_ref()
                .is_some_and(|span| span.path == path && span.start_line >= 1),
            "missing route source span for {route_label}"
        );
        assert!(has_entrypoint_reference(
            &graph,
            entrypoint,
            file,
            "entrypoint_file",
            Confidence::Syntactic,
        ));
        assert!(has_entrypoint_reference(
            &graph,
            entrypoint,
            handler_id,
            "entrypoint_function",
            Confidence::Syntactic,
        ));
    }
    assert!(
        !graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Entrypoint && node.label == "route ROUTE .route("
        }),
        "string literal route marker should not become a route entrypoint"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_adds_framework_config_conventions() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::write(root.join("app").join("settings.py"), "SECRET_KEY = 'dev'\n").unwrap();
    fs::write(
            root.join("app.py"),
            "from pydantic_settings import BaseSettings, SettingsConfigDict\napp.config.from_pyfile('settings.toml')\nclass Settings(BaseSettings):\n    model_config = SettingsConfigDict(env_file='.env.local')\n",
        )
        .unwrap();
    fs::write(
            root.join("server.js"),
            "const dotenv = require('dotenv');\ndotenv.config({ path: '.env.test' });\napp.set('view engine', 'pug');\n",
        )
        .unwrap();
    fs::write(root.join("next.config.ts"), "export default {};\n").unwrap();
    fs::write(
            root.join("main.go"),
            "package main\nfunc main() { viper.SetConfigName(\"service\"); viper.AddConfigPath(\"/etc/demo\"); godotenv.Load(\".env.go\") }\n",
        )
        .unwrap();
    fs::write(
        root.join("config").join("app.php"),
        "<?php\nreturn ['name' => config('app.name')];\n",
    )
    .unwrap();
    fs::write(root.join("deploy.sh"), "source config/runtime.env\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let expected = [
        (
            "django settings:app/settings.py",
            "django",
            "settings_module",
        ),
        ("flask config:settings.toml", "flask", "config_file"),
        ("pydantic settings:Settings", "pydantic", "settings_class"),
        ("pydantic env file:.env.local", "pydantic", "env_file"),
        ("dotenv config:.env.test", "dotenv", "env_file"),
        ("express setting:view engine", "express", "setting"),
        ("nextjs config:next.config.ts", "nextjs", "config_file"),
        ("viper config:service", "viper", "config_name"),
        ("viper config path:/etc/demo", "viper", "config_path"),
        ("godotenv config:.env.go", "godotenv", "env_file"),
        ("laravel config:config/app.php", "laravel", "config_file"),
        ("laravel config key:app.name", "laravel", "config_key"),
        ("shell config:config/runtime.env", "shell", "source_file"),
    ];

    for (label, framework, config_kind) in expected {
        let config_id = node_id(&graph, NodeKind::Config, label);
        let node = graph
            .nodes
            .iter()
            .find(|node| node.id == config_id)
            .expect("missing framework config node");
        assert_eq!(
            node.metadata.get("item_kind").map(String::as_str),
            Some("framework_config")
        );
        assert_eq!(
            node.metadata.get("framework").map(String::as_str),
            Some(framework)
        );
        assert_eq!(
            node.metadata.get("config_kind").map(String::as_str),
            Some(config_kind)
        );
        assert!(
            node.span
                .as_ref()
                .is_some_and(|span| !span.path.is_empty() && span.start_line >= 1),
            "missing framework config source span for {label}"
        );
        assert!(graph.edges.iter().any(|edge| {
            edge.target == config_id
                && edge.kind == EdgeKind::ReadsConfig
                && edge.confidence == Confidence::Syntactic
                && edge
                    .metadata
                    .get("source")
                    .is_some_and(|value| value == "framework")
        }));
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_applies_user_graph_annotations() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join(".codegraph").join("annotations.toml"),
        r#"[[annotations.node]]
id = "payments-files"
kind = "file"
label = "payments"

[annotations.node.set]
domain = "payments"
owner = "team-payments"
critical = true

[[annotations.node]]
id = "rust-functions"
kind = "function"
language = "rust"

[annotations.node.set]
runtime = "native"
"#,
    )
    .unwrap();
    fs::write(
        root.join("src").join("payments.rs"),
        "fn charge_card() {}\nfn refund() {}\n",
    )
    .unwrap();
    fs::write(root.join("src").join("users.rs"), "fn list_users() {}\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let payments_file = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "src/payments.rs")
        .expect("missing payments file");
    assert_eq!(
        payments_file
            .metadata
            .get("annotation.domain")
            .map(String::as_str),
        Some("payments")
    );
    assert_eq!(
        payments_file
            .metadata
            .get("annotation.owner")
            .map(String::as_str),
        Some("team-payments")
    );
    assert_eq!(
        payments_file
            .metadata
            .get("annotation.critical")
            .map(String::as_str),
        Some("true")
    );
    assert!(
        payments_file
            .metadata
            .get("annotation_ids")
            .is_some_and(|value| value.contains("payments-files"))
    );

    let users_file = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "src/users.rs")
        .expect("missing users file");
    assert!(!users_file.metadata.contains_key("annotation.domain"));

    let charge_card = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "charge_card")
        .expect("missing charge_card function");
    assert_eq!(
        charge_card
            .metadata
            .get("annotation.runtime")
            .map(String::as_str),
        Some("native")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_applies_custom_rules() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::write(
        root.join(".codegraph").join("rules.toml"),
        r#"[[rules.forbidden_dependency]]
id = "no-left-pad"
ecosystem = "npm"
package = "left-pad"
severity = "error"
message = "left-pad is not allowed"

[[rules.required_config]]
id = "needs-database-url"
target = "DATABASE_URL"

[[rules.required_config]]
id = "needs-payments-token"
target = "PAYMENTS_TOKEN"
severity = "warning"
"#,
    )
    .unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
  "dependencies": {
    "left-pad": "1.3.0"
  }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("app.py"),
        "import os\nDATABASE_URL = os.environ.get('DATABASE_URL')\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let violations: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "custom_rule_violation")
        })
        .collect();

    assert_eq!(violations.len(), 2);
    assert!(violations.iter().any(|node| {
        node.metadata
            .get("rule_id")
            .is_some_and(|value| value == "no-left-pad")
            && node
                .metadata
                .get("severity")
                .is_some_and(|value| value == "error")
            && node
                .metadata
                .get("message")
                .is_some_and(|value| value == "left-pad is not allowed")
    }));
    assert!(violations.iter().any(|node| {
        node.metadata
            .get("rule_id")
            .is_some_and(|value| value == "needs-payments-token")
    }));
    assert!(!violations.iter().any(|node| {
        node.metadata
            .get("rule_id")
            .is_some_and(|value| value == "needs-database-url")
    }));

    let forbidden = violations
        .iter()
        .find(|node| {
            node.metadata
                .get("rule_id")
                .is_some_and(|value| value == "no-left-pad")
        })
        .expect("missing forbidden dependency violation");
    let dependency = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("package_id")
                .is_some_and(|value| value == "npm:left-pad")
        })
        .expect("missing left-pad dependency");
    assert!(graph.edges.iter().any(|edge| {
        edge.source == forbidden.id
            && edge.target == dependency.id
            && edge.kind == EdgeKind::References
            && edge.confidence == Confidence::Exact
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "custom_rule_target")
    }));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_applies_forbidden_edge_custom_rules() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join(".codegraph").join("annotations.toml"),
        r#"[[annotations.node]]
id = "ui-render"
kind = "function"
label = "render"

[annotations.node.set]
layer = "ui"

[[annotations.node]]
id = "db-query"
kind = "function"
label = "query_user"

[annotations.node.set]
layer = "database"
"#,
    )
    .unwrap();
    fs::write(
        root.join(".codegraph").join("rules.toml"),
        r#"[[rules.forbidden_edge]]
id = "ui-cannot-call-db"
edge_kind = "calls"
severity = "error"
message = "UI layer must not call database layer directly"

[rules.forbidden_edge.source_metadata]
"annotation.layer" = "ui"

[rules.forbidden_edge.target_metadata]
"annotation.layer" = "database"
"#,
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn render() { query_user(); }\nfn query_user() {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let render = node_id(&graph, NodeKind::Function, "render");
    let query_user = node_id(&graph, NodeKind::Function, "query_user");
    let call_edge_index = graph
        .edges
        .iter()
        .position(|edge| {
            edge.source == render && edge.target == query_user && edge.kind == EdgeKind::Calls
        })
        .expect("missing render -> query_user call edge");
    let violation = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("rule_id")
                .is_some_and(|value| value == "ui-cannot-call-db")
        })
        .expect("missing forbidden edge violation");

    assert_eq!(
        violation.metadata.get("rule_kind").map(String::as_str),
        Some("forbidden_edge")
    );
    assert_eq!(
        violation.metadata.get("violated_edge_index").cloned(),
        Some(call_edge_index.to_string())
    );
    assert!(has_entrypoint_reference(
        &graph,
        violation.id,
        render,
        "custom_rule_target",
        Confidence::Exact,
    ));
    assert!(has_entrypoint_reference(
        &graph,
        violation.id,
        query_user,
        "custom_rule_target",
        Confidence::Exact,
    ));

    fs::remove_dir_all(root).unwrap();
}

fn node_id(graph: &CodeGraph, kind: NodeKind, label: &str) -> NodeId {
    graph
        .nodes
        .iter()
        .find(|node| node.kind == kind && node.label == label)
        .map(|node| node.id)
        .unwrap_or_else(|| panic!("missing {kind:?} node `{label}`"))
}

fn function_id_in_file(graph: &CodeGraph, label: &str, path: &str) -> NodeId {
    graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && node.label == label
                && node.span.as_ref().is_some_and(|span| span.path == path)
        })
        .map(|node| node.id)
        .unwrap_or_else(|| panic!("missing function `{label}` in `{path}`"))
}

fn has_resolved_local_import(graph: &CodeGraph, resolved_path: &str) -> bool {
    graph.nodes.iter().any(|node| {
        node.kind == NodeKind::ExternalDependency
            && node
                .metadata
                .get("item_kind")
                .is_some_and(|value| value == "import")
            && node
                .metadata
                .get("import_scope")
                .is_some_and(|value| value == "local")
            && node
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "resolved")
            && node
                .metadata
                .get("resolved_path")
                .is_some_and(|value| value == resolved_path)
    })
}

fn has_entrypoint_reference(
    graph: &CodeGraph,
    source: NodeId,
    target: NodeId,
    relation: &str,
    confidence: Confidence,
) -> bool {
    graph.edges.iter().any(|edge| {
        edge.source == source
            && edge.target == target
            && edge.kind == EdgeKind::References
            && edge.confidence == confidence
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == relation)
    })
}

fn temp_project_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "codegraph-indexer-test-{}-{nanos}-{id}",
        std::process::id()
    ))
}

#[test]
fn scan_is_deterministic_across_runs() {
    // The walk is explicitly sorted (sort_by_file_name); two scans of the same
    // tree must produce byte-identical graphs — node ids are positional, so an
    // unsorted readdir order would silently reshuffle them.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("b.rs"), "fn beta() {}\n").unwrap();
    fs::write(root.join("src").join("a.rs"), "fn alpha() { beta(); }\n").unwrap();
    fs::write(root.join("main.rs"), "fn main() { alpha(); }\n").unwrap();

    let options = IndexOptions::default();
    let first = scan_project(&root, &options).unwrap();
    let second = scan_project(&root, &options).unwrap();

    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}
