//! Unit tests for the indexer crate, driven by temporary project
//! fixtures on disk.

use super::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

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
    // Nothing in the project is called `custom_helper`, so the placeholder
    // says the call leaves it rather than that the resolver failed.
    assert_eq!(
        helper_nodes[0]
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("external")
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
fn a_table_created_inside_a_do_block_is_still_a_table() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("migrations")).unwrap();
    // kong writes eight of its tables this way, so a migration can run
    // twice without failing.
    fs::write(
        root.join("migrations").join("001_vaults.sql"),
        r#"DO $$
BEGIN
  IF (SELECT to_regclass('vaults_tags_idx')) IS NULL THEN
    CREATE TABLE IF NOT EXISTS "vaults_beta" (
      "id"     UUID PRIMARY KEY,
      "prefix" TEXT UNIQUE
    );

    CREATE INDEX IF NOT EXISTS "vaults_beta_tags_idx" ON "vaults_beta" ("prefix");
  END IF;
END$$;

INSERT INTO audit (statement) VALUES ('CREATE TABLE never_ran (id INT)');
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let tables: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.label.starts_with("sql table:"))
        .map(|node| node.label.as_str())
        .collect();

    assert!(tables.contains(&"sql table:vaults_beta"), "{tables:?}");
    // A verb inside a string literal is text, not a statement.
    assert!(!tables.contains(&"sql table:never_ran"), "{tables:?}");
    // And the table cites the line it is written on, not the `DO`.
    let table = graph
        .nodes
        .iter()
        .find(|node| node.label == "sql table:vaults_beta")
        .and_then(|node| node.span.as_ref())
        .expect("the table carries a span");
    assert_eq!(table.start_line, 4, "{table:?}");

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
fn sql_query_table_refs_skip_prose_that_opens_with_a_statement_keyword() {
    // A docstring signature: the table came from "columns from `df`" prose.
    assert!(
        sql_query_table_refs(
            "select(df::AbstractDataFrame, args...)\n\nCreate a data frame with columns from `df`."
        )
        .is_empty()
    );
    // A CLI help string.
    assert!(
        sql_query_table_refs(
            "Select context to install from. By default, install files from all contexts."
        )
        .is_empty()
    );
    // Test names that list the operations they exercise.
    assert!(sql_query_table_refs("insert, update, insert_or_update and delete").is_empty());
    assert!(sql_query_table_refs("replace because cannot update (delete first)").is_empty());
    // A panic message: SQL aliases are never qualified names.
    assert!(sql_query_table_refs("Delete from uninitialized collections.Map").is_empty());
    // FROM inside a call separates arguments instead of naming a row source.
    assert!(sql_query_table_refs("SELECT EXTRACT(EPOCH FROM CURRENT_TIMESTAMP) AS now").is_empty());

    // Real statements keep their tables.
    assert_eq!(
        sql_query_table_refs("SELECT path FROM searchIndex").len(),
        1
    );
    assert_eq!(
        sql_query_table_refs(
            "INSERT INTO plans (name) VALUES ($1) ON CONFLICT DO UPDATE SET x = 1"
        )
        .len(),
        1
    );
    assert_eq!(
        sql_query_table_refs("SELECT id FROM sessions s WHERE s.expires < now()").len(),
        1
    );
    assert_eq!(
        sql_query_table_refs("SELECT id FROM users WHERE id IN (SELECT user_id FROM admins)").len(),
        2
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
    let options = IndexOptions::default().with_parse_cache_dir(cache_dir.to_path_buf());

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
fn julia_and_r_packages_export_from_one_place() {
    // Both write the package's exports away from the files that define
    // the functions: a Julia `export` list in the module file, and R's
    // NAMESPACE beside the package.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("R")).unwrap();
    fs::write(
        root.join("src").join("Demo.jl"),
        "module Demo\n\nexport shared,\n       also_shared\n\ninclude(\"impl.jl\")\n\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("impl.jl"),
        "function shared(x)\n    return x\nend\n\nfunction hidden(x)\n    return x\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("NAMESPACE"),
        "export(mutate)\nS3method(\"[\",tbl)\n",
    )
    .unwrap();
    fs::write(
        root.join("R").join("verbs.R"),
        "mutate <- function(.data, ...) {\n  .data\n}\n\ncheck_names <- function(x) {\n  x\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let visibility_of = |label: &str| -> String {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("visibility").cloned())
            .unwrap_or_else(|| format!("no `{label}`"))
    };
    assert_eq!(visibility_of("shared"), "public");
    assert_eq!(visibility_of("hidden"), "private");
    assert_eq!(visibility_of("mutate"), "public");
    assert_eq!(visibility_of("check_names"), "private");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_rust_use_may_name_a_module_or_an_item() {
    // `use crate::parse_cli_node_id;` names a function and `mod cli; use
    // cli::*;` a module of the file's own. Rust writes all of it the same
    // way, and only what the project holds tells them apart -- the scan of
    // this very repository reported the function as a missing file and the
    // module as a crate nobody had declared.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("helpers.rs"),
        "pub fn parse_node_id(value: &str) -> usize {\n    value.len()\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "mod helpers;\n\nuse helpers::*;\nuse crate::parse_node_id;\n\nfn main() {\n    parse_node_id(\"n1\");\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let unresolved: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "import")
                && node
                    .metadata
                    .get("resolution")
                    .is_some_and(|resolution| resolution == "unresolved")
        })
        .map(|node| node.label.as_str())
        .collect();
    assert!(unresolved.is_empty(), "{unresolved:?}");

    // `use helpers::*` names the module declared above it, and the module
    // node the insight checks for is right there in the same file.
    assert!(
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Module
                && node.label == "helpers"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path == "src/main.rs")
        }),
        "the file's own module declaration is indexed"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn documentation_does_not_name_a_test_helper() {
    // Documentation describes what a project offers, not how it tests
    // itself: 11 of nlohmann/json's 14 prose mentions named a helper
    // inside its test suite. A document written among the tests may mean
    // one.
    let root = temp_project_root();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests").join("helpers.py"),
        "def build_fixture(name):\n    return name\n",
    )
    .unwrap();
    fs::write(
        root.join("README.md"),
        "# Demo\n\nCall `build_fixture` to get started.\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("README.md"),
        "# Tests\n\n`build_fixture` makes one.\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let mentions: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.metadata
                .get("resolution")
                .is_some_and(|resolution| resolution == "document_symbol")
        })
        .filter_map(|edge| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == edge.source)
                .map(|node| node.label.as_str())
        })
        .collect();

    assert!(
        mentions.iter().all(|label| !label.starts_with("README.md")),
        "the project's own README does not name a test helper: {mentions:?}"
    );
    assert!(
        mentions
            .iter()
            .any(|label| label.contains("tests/README.md")),
        "a document among the tests may: {mentions:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_header_that_declares_a_namespace_is_cpp() {
    // `.h` is C's extension and C++'s alike, and the extension is all the
    // path can say. Redis vendors `fast_float.h`, which is C++: read as C
    // it gave 1152 parse errors and 56 functions, and as C++ 150 and 178.
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("vendored.h"),
        "namespace fast_float {\n\ntemplate <typename T>\nT parse(const char* input) {\n  return T();\n}\n\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("plain.h"),
        "#ifndef PLAIN_H\n#define PLAIN_H\n\nint add(int a, int b);\n\n#endif\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let language_of = |path: &str| -> String {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == path)
            .and_then(|node| node.metadata.get("language").cloned())
            .unwrap_or_else(|| format!("no `{path}`"))
    };
    assert_eq!(language_of("vendored.h"), "cpp");
    assert_eq!(language_of("plain.h"), "c");
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Function && node.label == "parse"),
        "the template is read as the function it is"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_static_c_function_answers_only_its_own_translation_unit() {
    // `static` belongs to the file that compiles it -- unless it sits in a
    // header, which every file that includes it compiles for itself. 2681
    // of redis's calls landed on a `static` in a file that never sees
    // them, and every one that remains is in a header.
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("helpers.h"),
        "static int shared_helper(int value) { return value; }\n",
    )
    .unwrap();
    fs::write(
        root.join("other.c"),
        "static int hidden_helper(int value) { return value; }\n",
    )
    .unwrap();
    fs::write(
        root.join("main.c"),
        "#include \"helpers.h\"\n\nint main(void) {\n    return shared_helper(1) + hidden_helper(2);\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let node_in = |label: &str, path: &str| {
        graph.nodes.iter().find(|node| {
            node.kind == NodeKind::Function
                && node.label == label
                && node.span.as_ref().is_some_and(|span| span.path == path)
        })
    };
    let main = node_in("main", "main.c").expect("main is indexed");
    let shared = node_in("shared_helper", "helpers.h").expect("the header helper is indexed");
    let hidden = node_in("hidden_helper", "other.c").expect("the other unit's helper is indexed");
    let called: Vec<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.source == main.id)
        .map(|edge| edge.target)
        .collect();

    assert!(
        called.contains(&shared.id),
        "a header is compiled into whoever includes it"
    );
    assert!(
        !called.contains(&hidden.id),
        "another translation unit cannot name it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_ocaml_module_offers_what_its_interface_states() {
    // `filename.mli` lists what `filename.ml` offers, and the parser never
    // sees the file beside the one it is reading. A module with no
    // interface offers all of itself.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("filename.ml"),
        "let concat a b = a ^ b\n\nlet helper x = x\n\nlet ( >>= ) x f = f x\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("filename.mli"),
        "val concat : string -> string -> string\n\nval ( >>= ) : 'a -> ('a -> 'b) -> 'b\n",
    )
    .unwrap();
    fs::write(root.join("src").join("bare.ml"), "let anything x = x\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let visibility_of = |label: &str| -> String {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("visibility").cloned())
            .unwrap_or_else(|| format!("no `{label}`"))
    };
    assert_eq!(visibility_of("concat"), "public");
    assert_eq!(visibility_of("helper"), "private");
    // An operator is written the same way in both files, brackets and all.
    assert_eq!(visibility_of("( >>= )"), "public");
    assert_eq!(visibility_of("anything"), "public");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_module_keeps_what_it_does_not_export() {
    // `e.tag.toLowerCase()` in one vue package was answered by a `const
    // toLowerCase` in another. A module keeps what it does not export, so
    // no other file can be calling it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("helpers.ts"),
        "export const shared = (value: string) => value\n\nconst hidden = (value: string) => value\n\nexport const use = (value: string) => hidden(value)\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.ts"),
        "import { shared } from './helpers'\n\nexport function run(tag: string) {\n  shared(tag)\n  return tag.hidden()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let node_at = |label: &str, line: u32| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == label
                    && node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.start_line == line)
            })
            .unwrap_or_else(|| panic!("no `{label}` on line {line}"))
    };
    let shared = node_at("shared", 1);
    let hidden = node_at("hidden", 3);
    let run = node_at("run", 3);
    assert_eq!(
        shared.metadata.get("visibility").map(String::as_str),
        Some("public"),
        "`export const` exports it, however many wrappers stand between"
    );
    assert_eq!(
        hidden.metadata.get("visibility").map(String::as_str),
        Some("private")
    );

    let called_from_run: Vec<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.source == run.id)
        .map(|edge| edge.target)
        .collect();
    assert!(
        called_from_run.contains(&shared.id),
        "the export is reached"
    );
    assert!(
        !called_from_run.contains(&hidden.id),
        "another file cannot name what the module keeps"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_call_to_something_out_of_scope_stays_unresolved() {
    // `db.close()` in flask's tutorial example was answered by a `close`
    // defined inside a test function. Where no candidate is visible from
    // the caller, keeping them all because nothing better was found is how
    // a resolver invents a dependency.
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.py"),
        r#"def use_database(db):
    db.close()


def test_streaming():
    class Recorder:
        def close(self):
            return 1

    return Recorder()
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let nested = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "close"
                && node
                    .metadata
                    .get("enclosing_function")
                    .is_some_and(|enclosing| enclosing == "test_streaming")
        })
        .expect("the nested close is indexed");
    let caller = graph
        .nodes
        .iter()
        .find(|node| node.label == "use_database")
        .expect("the caller is indexed");

    assert!(
        !graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls && edge.source == caller.id && edge.target == nested.id
        }),
        "a definition nested in another one is not visible outside it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_python_route_belongs_to_the_framework_the_file_imports() {
    // `@app.get("/")` is the same line in Flask 2 and in FastAPI. Reading
    // it as FastAPI filed 45 of flask's own routes under the wrong
    // framework; what the file imports tells them apart.
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("api.py"),
        "from fastapi import FastAPI\n\napp = FastAPI()\n\n\n@app.get(\"/items\")\ndef items():\n    return []\n",
    )
    .unwrap();
    fs::write(
        root.join("web.py"),
        "from flask import Flask\n\napp = Flask(__name__)\n\n\n@app.get(\"/health\")\ndef health():\n    return \"ok\"\n",
    )
    .unwrap();
    fs::write(
        root.join("hooks.py"),
        "from .app import app\n\n\n@app.post(\"/hook\")\ndef hook():\n    return \"\"\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let framework_of = |path: &str| -> String {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.metadata
                    .get("item_kind")
                    .is_some_and(|kind| kind == "framework_route")
                    && node
                        .metadata
                        .get("target")
                        .is_some_and(|target| target == path)
            })
            .and_then(|node| node.metadata.get("framework").cloned())
            .unwrap_or_else(|| format!("no route in {path}"))
    };
    assert_eq!(framework_of("api.py"), "fastapi");
    assert_eq!(framework_of("web.py"), "flask");
    // A file that names neither is not filed under either.
    assert_eq!(framework_of("hooks.py"), "python-route");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_handler_written_in_place_has_no_name_to_find() {
    // `mux.HandleFunc("/x", func(w http.ResponseWriter, ...))` puts the
    // handler right there. Reading `func(w http.ResponseWriter` as its
    // name sent terraform looking for 32 functions that were never named.
    assert_eq!(
        handler_after_first_comma("mux.HandleFunc(\"/api\", handleManifest)"),
        Some("handleManifest".to_string())
    );
    assert_eq!(
        handler_after_first_comma("router.get('/users', UserController::index)"),
        Some("UserController::index".to_string())
    );
    for line in [
        "mux.HandleFunc(\"/api\", func(w http.ResponseWriter, r *http.Request) {",
        "app.post('/upload', multer({ dest: 'x' }).single('f'), save)",
        "app.get('/users', (req, res) => res.send(1))",
        "app.get('/users', function (req, res) {})",
        "router.get(\"/users\", |request| async { })",
    ] {
        assert_eq!(handler_after_first_comma(line), None, "{line}");
    }
}

#[test]
fn python_extras_are_not_a_version() {
    // `celery[redis]==5.2.7` asks for celery with its redis extra. Reading
    // the extras as the version pinned celery to `[redis]`.
    assert_eq!(
        package_name_and_version_from_requirement("celery[redis]==5.2.7"),
        Some(("celery".to_string(), Some("==5.2.7".to_string())))
    );
    assert_eq!(
        package_name_and_version_from_requirement("celery[redis]"),
        Some(("celery".to_string(), None))
    );
    assert_eq!(
        package_name_and_version_from_requirement("flask>=3.1.0"),
        Some(("flask".to_string(), Some(">=3.1.0".to_string())))
    );
}

#[test]
fn sql_literals_carry_the_line_they_were_written_on() {
    // The line used to be counted from the start of the file for every
    // literal, which cost a long file its length squared. It is carried
    // along with the cursor now, and has to agree with what it was.
    let source = "package main\n\nfunc run() {\n\tconst greeting = \"hello\"\n\tq := \"SELECT id FROM users\"\n\t// a comment\n\tother := `INSERT INTO audit_log VALUES (1)`\n}\n";
    let literals = source_sql_literals(source);
    let found: Vec<(u32, &str)> = literals
        .iter()
        .map(|literal| (literal.line, literal.value.as_str()))
        .collect();
    assert_eq!(
        found,
        vec![
            (4, "hello"),
            (5, "SELECT id FROM users"),
            (7, "INSERT INTO audit_log VALUES (1)"),
        ]
    );

    // A file with no SQL verb in it holds no query, so its literals are
    // never pulled out.
    assert!(source_sql_literals("let greeting = \"hello\"\n").is_empty());
}

#[test]
fn a_computed_require_names_no_module() {
    // `require(resolve(`package.json`))` asks for whatever that call
    // returns. Reaching past it for the first quoted string inside
    // reported vue as importing a package called `package.json`.
    assert_eq!(
        commonjs_require_call("const pkg = require('./package.json')"),
        Some("require(\"./package.json\")".to_string())
    );
    assert_eq!(
        commonjs_require_call("const pkg = require(resolve(`package.json`))"),
        None
    );
    assert_eq!(commonjs_require_call("const x = require(name)"), None);
}

#[test]
fn a_csharp_using_finds_the_namespace_the_project_declares() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("Polly.Core").join("Telemetry")).unwrap();
    fs::create_dir_all(root.join("src").join("Polly.Core").join("Retry")).unwrap();
    fs::write(
        root.join("src")
            .join("Polly.Core")
            .join("Telemetry")
            .join("TelemetryEvent.cs"),
        "namespace Polly.Telemetry;\n\npublic class TelemetryEvent\n{\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Polly.Core").join("Retry").join("RetryStrategy.cs"),
        "using System.Diagnostics;\nusing Polly.Telemetry;\nusing static Polly.Telemetry.TelemetryEvent;\n\nnamespace Polly.Retry;\n\npublic class RetryStrategy\n{\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.label == label)
            .and_then(|node| node.metadata.get("resolution").cloned())
    };
    // The project declares `Polly.Telemetry`, so the using names something.
    assert_eq!(
        resolution("using Polly.Telemetry;").as_deref(),
        Some("resolved")
    );
    // `System.Diagnostics` is the framework's, and `using static` names a
    // type rather than the namespace.
    assert_eq!(resolution("using System.Diagnostics;"), None);
    assert_eq!(
        resolution("using static Polly.Telemetry.TelemetryEvent;"),
        None
    );

    let namespace = graph
        .nodes
        .iter()
        .find(|node| node.label == "Polly.Telemetry")
        .expect("the namespace is one node");
    assert!(
        graph.edges.iter().any(|edge| edge.target == namespace.id
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "namespace_import")),
        "the using reaches the namespace it names"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_import_python_erases_is_not_a_runtime_dependency() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // requests writes `_types.py` this way: the interpreter never runs the
    // imports under `if TYPE_CHECKING:`, so they cannot close a cycle.
    // flask imports typing under an alias and writes `if t.TYPE_CHECKING:`,
    // so what the module is called cannot be part of the test.
    fs::write(
        root.join("src").join("_types.py"),
        "import typing as t\n\nif t.TYPE_CHECKING:\n    from .auth import AuthBase\n\n\ndef describe(value):\n    return value\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("auth.py"),
        "from ._types import describe\n\n\nclass AuthBase:\n    pass\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let type_only: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("type_only")
                .is_some_and(|value| value == "true")
        })
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(
        type_only,
        vec!["from .auth import AuthBase"],
        "{type_only:?}"
    );
    // The import that does run is not marked.
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.label == "from ._types import describe"
                && !node.metadata.contains_key("type_only"))
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_elixir_attribute_is_not_a_call_and_neither_is_invoking_a_value() {
    // `@moduledoc false` and `@spec change(..) :: t` are module
    // attributes, and the grammar reads what follows the `@` as a call:
    // ecto filed 356 calls to things named `doc`, `type` and `spec`.
    // `fun.(new, current)` invokes whatever the variable holds, and the
    // label it produced -- `fun.` -- names nothing; ecto writes 82.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("mix.exs"),
        "defmodule App.MixProject do\n  use Mix.Project\n\n  defp deps do\n    [{:jason, \"~> 1.0\"}]\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("relation.ex"),
        "defmodule App.Relation do\n  @moduledoc false\n\n  @spec change(map, map) :: map\n  def change(new, current) do\n    apply_change(new, current)\n  end\n\n  defp single_change(new, current, fun) do\n    fun.(new, current)\n  end\n\n  defp apply_change(new, _current), do: new\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let called: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls)
        .filter_map(|edge| edge.metadata.get("call_label").map(String::as_str))
        .collect();
    assert!(
        called.contains(&"apply_change"),
        "a call the module makes is still a call, got {called:?}"
    );
    for attribute in ["moduledoc", "spec"] {
        assert!(
            !called.contains(&attribute),
            "`@{attribute}` is a declaration, got {called:?}"
        );
    }
    assert!(
        !called.iter().any(|label| label.ends_with('.')),
        "invoking a value names nothing to call, got {called:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_javascript_spec_is_the_callbacks_it_is_written_in() {
    // `describe('x', () => { it('y', () => { service.load() }) })` puts
    // every call a test makes inside an anonymous function, which is a
    // callback everywhere else and the test itself here: koel's 498 spec
    // files made 1456 calls between them.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("service.ts"),
        "export function load() {\n  return 1\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("service.spec.ts"),
        "import { load } from './service'\n\ndescribe('service', () => {\n  it('loads', () => {\n    expect(load()).toBe(1)\n  })\n})\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let spec = graph
        .nodes
        .iter()
        .find(|node| node.label == "src/service.spec.ts")
        .expect("the spec is indexed")
        .id;
    let called: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.source == spec)
        .filter_map(|edge| edge.metadata.get("call_label").map(String::as_str))
        .collect();
    assert!(
        called.contains(&"load"),
        "the call a test makes belongs to the file that runs it, got {called:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_spec_is_the_blocks_it_is_written_in() {
    // `describe .. do it .. do expect(described_class.new).to be end end`
    // is what a spec file runs, and a Ruby block is a callback that runs
    // when something invokes it -- so mastodon's 1312 spec files had 3163
    // calls between them and "which tests cover this" had almost nothing
    // to answer with.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app/services")).unwrap();
    fs::create_dir_all(root.join("spec/services")).unwrap();
    fs::write(
        root.join("app/services/suspend_service.rb"),
        "class SuspendService\n  def call(account)\n    account\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("spec/services/suspend_service_spec.rb"),
        "require 'rails_helper'\n\nRSpec.describe SuspendService do\n  it 'suspends' do\n    expect(subject.call(nil)).to be_nil\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let spec = graph
        .nodes
        .iter()
        .find(|node| node.label == "spec/services/suspend_service_spec.rb")
        .expect("the spec is indexed")
        .id;
    let called: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.source == spec)
        .filter_map(|edge| edge.metadata.get("call_label").map(String::as_str))
        .collect();
    assert!(
        called.contains(&"call"),
        "the call a test makes belongs to the file that runs it, got {called:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn prose_before_a_colon_is_not_a_make_target() {
    // requests writes `$(error The '$(SPHINXBUILD)' command was not
    // found. .. https://www.sphinx-doc.org/)` in its docs Makefile, and
    // the URL's colon turned the sentence in front of it into make
    // targets called `The`, `command` and `was`. Every word before a
    // rule's colon is one of its targets, so a word that is not a target
    // means the line is not a rule.
    let targets = makefile_targets(
        "html dirhtml: deps\n\techo build\n\n$(error The '$(SPHINXBUILD)' command was not found. See https://www.sphinx-doc.org/)\n\nclean:\n\trm -rf build\n",
    );
    let names: Vec<&str> = targets.iter().map(|target| target.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["html", "dirhtml", "clean"],
        "a rule names every word before its colon"
    );
}

#[test]
fn a_python_class_is_reached_by_what_it_inherits_and_annotates() {
    // django-oscar declares 1697 classes and 14% of them had anything
    // pointing at them: a Django project states its structure through
    // inheritance -- `class Basket(AbstractBasket)` -- and nothing read
    // it, nor the annotations beside it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/oscar/apps/basket")).unwrap();
    fs::write(
        root.join("setup.py"),
        "from setuptools import setup\n\nsetup(name='oscar')\n",
    )
    .unwrap();
    fs::write(
        root.join("src/oscar/apps/basket/abstract_models.py"),
        "from django.db import models\n\n\nclass AbstractBasket(models.Model):\n    def add(self, product):\n        return product\n",
    )
    .unwrap();
    fs::write(
        root.join("src/oscar/apps/basket/models.py"),
        "from oscar.apps.basket.abstract_models import AbstractBasket\n\n\nclass Basket(AbstractBasket):\n    pass\n",
    )
    .unwrap();
    fs::write(
        root.join("src/oscar/apps/basket/views.py"),
        "from oscar.apps.basket.models import Basket\n\n\ndef summary(basket: Basket) -> str:\n    return str(basket)\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reached = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.target && node.label == label)
        })
    };
    assert!(reached("AbstractBasket"), "a class states what it inherits");
    assert!(
        reached("Basket"),
        "and an annotation states the class a value has"
    );
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| { edge.metadata.get("type_label").map(String::as_str) == Some("str") }),
        "a name the language provides is not a class the project declares"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn haskell_and_julia_reach_the_types_their_signatures_name() {
    // shellcheck writes `runChecker :: Parameters -> Checker ->
    // [TokenComment]` and nothing pointed at any of those types;
    // DataFrames.jl writes `df::AbstractDataFrame` and `struct DataFrame
    // <: AbstractDataFrame` and the same held.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("ShellCheck.cabal"),
        "name: ShellCheck\nversion: 0.10.0\n\nlibrary\n    build-depends:\n      base\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Lib.hs"),
        "module Lib where\n\ndata Parameters = Parameters { shellType :: Int }\n\nrunChecker :: Parameters -> Int\nrunChecker p = shellType p\n",
    )
    .unwrap();
    fs::write(
        root.join("Project.toml"),
        "name = \"DataFrames\"\n\n[deps]\nCompat = \"34da2185\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("abstract.jl"),
        "abstract type AbstractDataFrame end\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("frame.jl"),
        "struct DataFrame\n    columns::Vector\nend\n\nfunction nrow(df::AbstractDataFrame)\n    length(df.columns)\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reached = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.target && node.label == label)
        })
    };
    assert!(
        reached("Parameters"),
        "a Haskell signature names the types the function works with"
    );
    assert!(
        reached("AbstractDataFrame"),
        "and a Julia annotation names the type a value has"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn swift_and_erlang_reach_what_they_name() {
    // Alamofire's `Session` -- the type its whole API is written around --
    // had nothing pointing at it and four declarations, because every
    // `extension Session { .. }` read as another one. cowboy's modules
    // were reached by nothing at all, and a `cowboy_req:reply(..)` names
    // the module on the left of the colon.
    let root = temp_project_root();
    fs::create_dir_all(root.join("Sources")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Sources").join("Session.swift"),
        "public class Session {\n    public let identifier: String\n\n    init(identifier: String) {\n        self.identifier = identifier\n    }\n}\n\nextension Session {\n    func reset() {}\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("Sources").join("Request.swift"),
        "public class Request {\n    let session: Session\n\n    init(session: Session) {\n        self.session = session\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("cowboy_req.erl"),
        "-module(cowboy_req).\n-export([reply/2]).\n\nreply(Status, Req) ->\n    {Status, Req}.\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("cowboy_handler.erl"),
        "-module(cowboy_handler).\n-export([execute/2]).\n\nexecute(Req, State) ->\n    cowboy_req:reply(200, Req),\n    {ok, State}.\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let named = |label: &str| -> Vec<&str> {
        graph
            .nodes
            .iter()
            .filter(|node| {
                matches!(node.kind, NodeKind::Type | NodeKind::Module) && node.label == label
            })
            .filter_map(|node| node.span.as_ref().map(|span| span.path.as_str()))
            .collect()
    };
    assert_eq!(
        named("Session"),
        vec!["Sources/Session.swift"],
        "an extension adds to a type declared elsewhere and declares none"
    );
    let reached = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.target && node.label == label)
        })
    };
    assert!(
        reached("Session"),
        "a property's type names the class it holds"
    );
    assert!(
        reached("cowboy_req"),
        "and `cowboy_req:reply(..)` names the module it calls into"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_c_struct_declares_a_type_only_where_it_has_a_body() {
    // `struct client { .. }` declares a type and `struct client *c` names
    // one, and reading both as declarations gave redis 183 nodes for
    // `redisCommand` and 3635 types for its 1492 names -- so no reference
    // could choose a target and `impact robj` answered with nothing.
    // `typedef struct client { .. } client;` is one declaration written
    // twice, and the name a program uses is the typedef's.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("server.h"),
        "#ifndef SERVER_H\n#define SERVER_H\n\ntypedef struct client {\n    int fd;\n} client;\n\nstruct command {\n    char *name;\n};\n\n#endif\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("networking.c"),
        "#include \"server.h\"\n\nvoid readQuery(struct client *c, struct command *cmd) {\n    c->fd = 0;\n    (void)cmd;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let nodes_named = |label: &str| -> Vec<&str> {
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Type && node.label == label)
            .filter_map(|node| node.span.as_ref().map(|span| span.path.as_str()))
            .collect()
    };
    assert_eq!(
        nodes_named("client"),
        vec!["src/server.h"],
        "the typedef declares it once, and the use in another file names it"
    );
    assert_eq!(nodes_named("command"), vec!["src/server.h"]);
    let references_into = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.target && node.label == label)
        })
    };
    assert!(
        references_into("client") && references_into("command"),
        "a parameter's type names the struct it is given"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_elixir_module_is_reached_by_the_alias_that_names_it() {
    // ecto declares 390 modules and nothing pointed at any of them, so
    // "what breaks if I change `Ecto.Changeset`" answered with nothing.
    // An Elixir program names a module by its alias -- `alias
    // Ecto.Changeset`, `use Ecto.Schema` -- and through the dot of a
    // qualified call.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib/ecto")).unwrap();
    fs::write(
        root.join("mix.exs"),
        "defmodule App.MixProject do\n  use Mix.Project\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("lib/ecto/changeset.ex"),
        "defmodule Ecto.Changeset do\n  defstruct [:data, :changes]\n\n  def change(data), do: data\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("lib/ecto/repo.ex"),
        "defmodule Ecto.Repo do\n  alias Ecto.Changeset\n\n  def insert(struct) do\n    Changeset.change(struct)\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let changeset: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.label == "Ecto.Changeset")
        .collect();
    assert_eq!(
        changeset.len(),
        1,
        "a `defstruct` states the shape of the module it sits in, and the \
         module already stands for it"
    );
    assert!(
        graph.edges.iter().any(|edge| {
            edge.target == changeset[0].id
                && edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
        }),
        "and the module another file aliases is reached"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn kotlin_types_are_reached_by_the_declarations_that_name_them() {
    // okio declares 358 types and four references pointed into them, so
    // "what breaks if I change `Buffer`" -- the type its whole API is
    // written around -- answered with nothing. A source set is a
    // directory, and okio declares `Buffer` once per platform it builds
    // for, so a name written in one means that directory's.
    let root = temp_project_root();
    fs::create_dir_all(root.join("okio/src/commonMain/kotlin/okio")).unwrap();
    fs::create_dir_all(root.join("okio/src/jvmMain/kotlin/okio")).unwrap();
    fs::write(
        root.join("okio/src/commonMain/kotlin/okio/Buffer.kt"),
        "package okio\n\nclass Buffer : Sink {\n  fun clear() {}\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("okio/src/commonMain/kotlin/okio/Sink.kt"),
        "package okio\n\ninterface Sink {\n  fun write(source: Buffer, count: Long)\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("okio/src/jvmMain/kotlin/okio/Buffer.kt"),
        "package okio\n\nclass Buffer {\n  fun readUtf8(): String = \"\"\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let buffer_at = |path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "Buffer" && node.span.as_ref().is_some_and(|span| span.path == path)
            })
            .unwrap_or_else(|| panic!("Buffer is declared in {path}"))
            .id
    };
    let referenced = |target: NodeId| {
        graph.edges.iter().any(|edge| {
            edge.target == target
                && edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
        })
    };
    assert!(
        referenced(buffer_at("okio/src/commonMain/kotlin/okio/Buffer.kt")),
        "a parameter's type names the class it is given"
    );
    assert!(
        !referenced(buffer_at("okio/src/jvmMain/kotlin/okio/Buffer.kt")),
        "and the platform beside it declares a Buffer of its own"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_rails_callback_calls_the_method_its_class_declares() {
    // `before_action :set_account` is Rails invoking a method of the
    // controller that wrote it, and mastodon names 342 methods that way.
    // Every one read as a method nobody calls, and the name alone chooses
    // none of them: eleven controllers declare `set_account`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app/controllers/admin")).unwrap();
    fs::write(
        root.join("Gemfile"),
        "source 'https://rubygems.org'\n\ngem 'rails'\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/accounts_controller.rb"),
        "class AccountsController < ApplicationController\n  before_action :set_account, only: [:show]\n\n  def show\n    render json: @account\n  end\n\n  private\n\n  def set_account\n    @account = Account.find(params[:id])\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/accounts_controller.rb"),
        "class Admin::AccountsController < ApplicationController\n  def set_account\n    @account = nil\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("set_account")
        })
        .expect("the registration calls the method");
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("the target is a node");
    assert_eq!(
        target.metadata.get("owner_type").map(String::as_str),
        Some("AccountsController"),
        "the class the registration is written in is the one Rails calls"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_rust_test_is_run_by_the_harness_and_not_by_the_project() {
    // `#[test] fn a_call_edge_says_what_settled_it` is called by nobody,
    // and that is how a test works. A Rust crate keeps its tests beside
    // its code in `#[cfg(test)] mod tests`, so the path says nothing: 684
    // of this repository's own 1018 orphan functions were its tests.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("lib.rs"),
        "pub fn used() -> u32 {\n    1\n}\n\npub fn never_called() -> u32 {\n    2\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    fn helper() -> u32 {\n        used()\n    }\n\n    #[test]\n    fn it_works() {\n        assert_eq!(helper(), 1);\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let marked = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("invoked_by").cloned())
    };
    assert_eq!(
        marked("it_works").as_deref(),
        Some("test_runner"),
        "the attribute says the harness runs it"
    );
    assert_eq!(
        marked("helper").as_deref(),
        Some("test_runner"),
        "and a helper inside `#[cfg(test)] mod tests` is test code too"
    );
    assert_eq!(
        marked("never_called"),
        None,
        "while the crate's own function is the project's"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_bare_go_call_means_its_own_package() {
    // Go resolves an unqualified name inside its own package, and a
    // package is a directory: gqlgen declares `is_bin_in_path` in several
    // and every call to it was ambiguous.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal/code")).unwrap();
    fs::create_dir_all(root.join("internal/tool")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal/code/paths.go"),
        "package code\n\nfunc isBinInPath() bool {\n\treturn true\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal/code/run.go"),
        "package code\n\nfunc Run() bool {\n\treturn isBinInPath()\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal/tool/paths.go"),
        "package tool\n\nfunc isBinInPath() bool {\n\treturn false\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("isBinInPath")
        })
        .expect("the call is recorded");
    assert_eq!(
        call.metadata.get("resolution").map(String::as_str),
        Some("resolved"),
        "a package's own declaration is what an unqualified name means"
    );
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("the target is a node");
    assert_eq!(
        target.span.as_ref().map(|span| span.path.as_str()),
        Some("internal/code/paths.go"),
        "and the package next door declares a different function"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_go_package_is_a_directory_and_a_type_written_in_one_is_its_own() {
    // terraform declares `Backend` in seventeen packages, one per remote
    // state backend, so every reference to it was ambiguous and `impact
    // Backend` answered with nothing. A Go package is a directory, and a
    // name written inside one means what that directory declares.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal/backend/azure")).unwrap();
    fs::create_dir_all(root.join("internal/backend/gcs")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal/backend/azure/backend.go"),
        "package azure\n\ntype Backend struct {\n\tName string\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal/backend/azure/state.go"),
        "package azure\n\nfunc Load(b *Backend) string {\n\treturn b.Name\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal/backend/gcs/backend.go"),
        "package gcs\n\ntype Backend struct {\n\tBucket string\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let azure = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Backend"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path == "internal/backend/azure/backend.go")
        })
        .expect("the azure backend is declared")
        .id;
    let gcs = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Backend"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path == "internal/backend/gcs/backend.go")
        })
        .expect("the gcs backend is declared")
        .id;
    let references = |target: NodeId| {
        graph.edges.iter().any(|edge| {
            edge.target == target
                && edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
        })
    };
    assert!(
        references(azure),
        "the package's own file names the type its package declares"
    );
    assert!(
        !references(gcs),
        "and a package next door declares a different type of the same name"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ruby_classes_are_reached_by_the_constants_that_name_them() {
    // mastodon declares 2083 classes and modules and nothing pointed at
    // any of them: "what breaks if I change `Account`" answered with
    // nothing at all. A Ruby program names a class by its constant --
    // `Account.find(id)`, `class X < ApplicationRecord`, `include
    // Payloadable` -- and a name written on its own means the class that
    // answers to exactly that name, not the stub a migration or a
    // maintenance task declares under a module of its own.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app/models")).unwrap();
    fs::create_dir_all(root.join("app/services")).unwrap();
    fs::create_dir_all(root.join("db/migrate")).unwrap();
    fs::create_dir_all(root.join("lib/tasks")).unwrap();
    fs::write(
        root.join("app/models/account.rb"),
        "class Account < ApplicationRecord\n  include Payloadable\n\n  def suspend!\n    update!(suspended: true)\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/models/application_record.rb"),
        "class ApplicationRecord < ActiveRecord::Base\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/models/payloadable.rb"),
        "module Payloadable\n  def payload\n    {}\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/services/suspend_service.rb"),
        "class SuspendService\n  def call(id)\n    account = Account.where(id: id).first\n    account.suspend!\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("db/migrate/20240101000000_backfill_accounts.rb"),
        "class BackfillAccounts < ActiveRecord::Migration[8.0]\n  class Account < ApplicationRecord\n  end\n\n  def up\n    Account.find_each { |account| account.touch }\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("lib/tasks/maintenance.rb"),
        "module Maintenance\n  class Account < ApplicationRecord\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let node_at = |label: &str, path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                matches!(node.kind, NodeKind::Type | NodeKind::Module)
                    && node.label == label
                    && node.span.as_ref().is_some_and(|span| span.path == path)
            })
            .unwrap_or_else(|| panic!("{label} is declared in {path}"))
            .id
    };
    let model = node_at("Account", "app/models/account.rb");
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|node| node.label == "Maintenance::Account")
            .count(),
        1,
        "a class inside a module states the constant path it answers to"
    );
    let references_into = |target: NodeId| -> Vec<String> {
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.target == target
                    && edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
            })
            .filter_map(|edge| {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == edge.source)
                    .map(|node| node.label.clone())
            })
            .collect()
    };
    assert!(
        references_into(model).contains(&"call".to_string()),
        "the service that writes `Account.where` reaches the model, got {:?}",
        references_into(model)
    );
    assert!(
        !references_into(model).is_empty(),
        "and the migration's own stub does not take the reference"
    );
    assert!(
        !references_into(node_at("Payloadable", "app/models/payloadable.rb")).is_empty(),
        "`include Payloadable` names the module it mixes in"
    );
    assert!(
        !references_into(node_at(
            "ApplicationRecord",
            "app/models/application_record.rb"
        ))
        .is_empty(),
        "and `< ApplicationRecord` names the class it inherits"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn busted_and_munit_hand_a_spec_its_cases() {
    // kong writes 1011 spec files whose `describe`, `it`, `lazy_setup`
    // and `assert.same` come from busted, and cats 255 whose `test`,
    // `checkAll` and `forAll` come from munit and ScalaCheck.
    let root = temp_project_root();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::create_dir_all(root.join("src/test/scala")).unwrap();
    fs::write(
        root.join("kong-1.0-0.rockspec"),
        "package = \"kong\"\ndependencies = {\n  \"penlight == 1.14.0\",\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("spec").join("router_spec.lua"),
        "describe(\"router\", function()\n  lazy_setup(function()\n    helpers.start_kong()\n  end)\n\n  it(\"routes\", function()\n    assert.same(1, 1)\n  end)\nend)\n",
    )
    .unwrap();
    fs::write(
        root.join("build.sbt"),
        "libraryDependencies += \"org.typelevel\" %%% \"cats-core\" % catsVersion\n",
    )
    .unwrap();
    fs::write(
        root.join("src/test/scala").join("MonadSuite.scala"),
        "class MonadSuite extends munit.FunSuite {\n  test(\"laws\") {\n    assertEquals(1, 1)\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in [
        "describe",
        "it",
        "lazy_setup",
        "assert.same",
        "test",
        "assertEquals",
    ] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is the runner's"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rspec_hands_a_ruby_spec_its_cases_and_its_matchers() {
    // 7366 of mastodon's 29668 unresolved ruby calls are RSpec's own:
    // `it`, `let`, `expect`, `eq`, `allow`. Reporting them as unresolved
    // reads as a resolver that failed rather than a gem that provides
    // them, which is the same thing busted and munit already say.
    let root = temp_project_root();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("Gemfile"),
        "source 'https://rubygems.org'\ngem 'rspec-rails'\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("account.rb"),
        "class Account\n  def suspend\n    true\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("spec").join("account_spec.rb"),
        "describe Account do\n  let(:account) { Account.new }\n\n  before do\n    allow(account).to receive(:suspend)\n  end\n\n  it 'suspends' do\n    expect(account.suspend).to eq(true)\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in [
        "describe", "let", "before", "allow", "receive", "it", "expect", "eq",
    ] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is the runner's"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_language_names_its_own_vocabulary() {
    // Nix says outright what is the evaluator's -- everything under
    // `builtins.` and a handful of globals -- and home-manager writes 798
    // of them. package:test hands a Dart suite its cases, 470 of them in
    // the `http` package. Neither is a function a project failed to ship.
    let root = temp_project_root();
    fs::create_dir_all(root.join("test")).unwrap();
    fs::write(
        root.join("default.nix"),
        "{ lib }:\n{\n  mkEntry = name: builtins.toFile name (toString 1);\n  helper = value: map (x: x) value;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("pubspec.yaml"),
        "name: sample\ndev_dependencies:\n  test: ^1.0.0\n",
    )
    .unwrap();
    fs::write(
        root.join("test").join("client_test.dart"),
        "void main() {\n  group('client', () {\n    setUp(() {});\n    test('sends', () {\n      expect(1, equals(1));\n    });\n  });\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in [
        "builtins.toFile",
        "toString",
        "map",
        "group",
        "setUp",
        "test",
        "expect",
        "equals",
    ] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is the language's or the runner's"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_default_import_is_a_name_calls_are_written_through() {
    // `import path from 'path'` is how a file reaches `path.join`, and
    // only `import * as path` was recorded as a qualifier. axios writes 22
    // of the first kind for `path` alone, and a local default import says
    // as much: `zlib.gzip` had been answered by a `const gzip` the same
    // test file declares.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"sample\",\"version\":\"1.0.0\"}",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("server.js"),
        "import path from 'path';\n\nexport function resolveFile(root, name) {\n  return path.join(root, name);\n}\n",
    )
    .unwrap();
    // A project's own tests import it by the name it publishes, which is
    // not an outside dependency however the walk ordered the manifest.
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("lib").join("index.js"),
        "export function create(config) {\n  return config;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("smoke.test.js"),
        "import sample from 'sample';\n\nexport function run() {\n  return sample.create({});\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("path.join").as_deref(),
        Some("external"),
        "the file says where `path` comes from"
    );
    assert_eq!(
        resolution("sample.create").as_deref(),
        Some("resolved"),
        "a project does not import itself from outside"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn xctest_hands_a_swift_suite_its_assertions() {
    // 2559 of Alamofire's 4317 unresolved swift calls are XCTest's own --
    // 59% of them -- led by XCTAssertEqual, expectation, fulfill and
    // waitForExpectations. None is a method the project wrote.
    let root = temp_project_root();
    fs::create_dir_all(root.join("Sources")).unwrap();
    fs::create_dir_all(root.join("Tests")).unwrap();
    fs::write(
        root.join("Package.swift"),
        "// swift-tools-version:5.9\nimport PackageDescription\n",
    )
    .unwrap();
    fs::write(
        root.join("Sources").join("Session.swift"),
        "public func makeSession() -> Int {\n  return 1\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("Tests").join("SessionTests.swift"),
        "final class SessionTests: XCTestCase {\n  func testMakes() {\n    let done = expectation(description: \"done\")\n    XCTAssertEqual(makeSession(), 1)\n    done.fulfill()\n    waitForExpectations(timeout: 1)\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in [
        "expectation",
        "XCTAssertEqual",
        "fulfill",
        "waitForExpectations",
    ] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is XCTest's"
        );
    }
    // The suite still reaches what the project wrote.
    assert_eq!(resolution("makeSession").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn kotlin_test_hands_a_suite_its_assertions() {
    // 1791 of okio's 3985 unresolved kotlin calls are kotlin.test's and
    // AssertJ's -- 45% -- led by assertEquals 876, assertTrue 216 and
    // assertThat 171. A suite still reaches what the project wrote.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/main/kotlin")).unwrap();
    fs::create_dir_all(root.join("src/test/kotlin")).unwrap();
    fs::write(
        root.join("build.gradle.kts"),
        "dependencies {\n  testImplementation(kotlin(\"test\"))\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/kotlin").join("Buffer.kt"),
        "package okio\n\nfun readSize(): Int {\n  return 1\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/test/kotlin").join("BufferTest.kt"),
        "package okio\n\nclass BufferTest {\n  fun testReads() {\n    assertEquals(1, readSize())\n    assertTrue(true)\n    assertThat(readSize()).isEqualTo(1)\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in ["assertEquals", "assertTrue", "assertThat", "isEqualTo"] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is the test framework's"
        );
    }
    assert_eq!(resolution("readSize").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_python_test_case_gets_its_assertions_from_unittest() {
    // django-oscar writes self.assertEqual 841 times and self.assertTrue
    // 314, 1717 calls in all, and every one comes from the TestCase it
    // extends. A file that writes its checks with the `assert` statement,
    // as flask and requests do, has nothing here to find.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(root.join("requirements.txt"), "django\n").unwrap();
    fs::write(
        root.join("src").join("basket.py"),
        "def total(lines):\n    return len(lines)\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("test_basket.py"),
        "from unittest import TestCase\nfrom src.basket import total\n\n\nclass BasketTests(TestCase):\n    def test_total(self):\n        self.assertEqual(total([]), 0)\n        self.assertTrue(True)\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in ["self.assertEqual", "self.assertTrue"] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} comes from the TestCase"
        );
    }
    assert_eq!(resolution("total").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn shouldly_and_xunit_hand_a_csharp_suite_its_assertions() {
    // 5762 of Polly's 8552 unresolved csharp calls are the test
    // framework's -- 67% -- led by Should.Throw and the ShouldBe that
    // reads it. A project that declares its own shim keeps it: Newtonsoft
    // writes XUnitAssert and its 2197 Assert.AreEqual calls still reach
    // it, because a call that resolves never asks this list.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    fs::write(
        root.join("Polly.sln"),
        "Microsoft Visual Studio Solution File\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Breaker.cs"),
        "namespace Polly;\n\npublic class Breaker\n{\n    public int Attempt() => 1;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("test").join("BreakerTests.cs"),
        "namespace Polly.Tests;\n\npublic class BreakerTests\n{\n    public void Opens()\n    {\n        var breaker = new Breaker();\n        breaker.Attempt().ShouldBe(1);\n        Should.Throw<Exception>(() => breaker.Attempt());\n        Assert.Equal(1, breaker.Attempt());\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in ["Should.Throw", "Assert.Equal"] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is the framework's"
        );
    }
    // The suite still reaches what the project wrote.
    assert_eq!(resolution("breaker.Attempt").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ruby_provides_its_own_methods() {
    // 4434 of mastodon's 22033 unresolved ruby calls are Ruby's own --
    // `new` 672, `to_s` 350, `map` 347, `each` 298 -- and none is a method
    // the project wrote. A call through a constant the project never
    // declares is still a gem's: that rule is asked first, and letting
    // these names past it handed 107 gem methods to same-named definitions
    // of the project's own.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(root.join("Gemfile"), "source 'https://rubygems.org'\n").unwrap();
    fs::write(
        root.join("app").join("account.rb"),
        "class Account\n  def display_name\n    handle.to_s\n  end\n\n  def handle\n    'x'\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("to_s").as_deref(),
        Some("builtin"),
        "to_s is Ruby's"
    );
    // What the project writes is still what a call to it means.
    assert_eq!(resolution("handle").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_matcher_is_part_of_the_runner_that_hands_it_over() {
    // `expect` was on the list and the matchers that read it were not,
    // which is most of what a suite writes: 3271 across core, koel, zod
    // and openzeppelin. chai reads through `to`, which is how openzeppelin
    // writes `to.be.revertedWithCustomError`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"sample\",\"version\":\"1.0.0\"}",
    )
    .unwrap();
    fs::write(
        root.join("src").join("sum.js"),
        "export function sum(a, b) {\n  return a + b;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("test").join("sum.test.js"),
        "import { sum } from '../src/sum.js';\n\ndescribe('sum', () => {\n  it('adds', () => {\n    expect(sum(1, 2)).toBe(3);\n    expect(sum(1, 2)).toEqual(3);\n    expect(() => sum()).toThrow();\n  });\n});\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in ["expect", "toBe", "toEqual", "toThrow"] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is the runner's"
        );
    }
    // The suite still reaches what the project wrote.
    assert_eq!(resolution("sum").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_factory_and_a_mock_builder_belong_to_the_framework() {
    // koel writes `Song::factory` 829 times and `createOne` 526; monolog
    // writes PHPUnit's `getMock`, `onlyMethods` and `method`. 2030 calls
    // in all, and a label that names the class it goes through is read
    // from its end.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("composer.json"),
        "{\"name\":\"koel/koel\",\"require-dev\":{\"phpunit/phpunit\":\"^10\"}}",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Song.php"),
        "<?php\n\nclass Song\n{\n    public function title(): string\n    {\n        return 'x';\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("SongTest.php"),
        "<?php\n\nclass SongTest extends TestCase\n{\n    public function testTitle(): void\n    {\n        $song = Song::factory()->createOne();\n        $this->getJson('/songs');\n        $mock = $this->getMock(Song::class);\n        $mock->method('title');\n        $song->title();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for provided in ["Song::factory", "createOne", "getJson", "getMock", "method"] {
        assert_eq!(
            resolution(provided).as_deref(),
            Some("builtin"),
            "{provided} is the framework's"
        );
    }
    // What the project writes is still what a call to it means.
    assert_eq!(resolution("title").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_php_use_says_whose_class_a_bare_name_means() {
    // guzzle writes `use GuzzleHttp\Psr7\Request;` and then `new
    // Request(..)` 612 times: the class is psr7's, and without the binding
    // a bare name could only be matched against everything the project
    // declares. Two things it must not do, both measured: a name the
    // project declares is the project's whatever the import list says --
    // `use GuzzleHttp\Client;` names guzzle's own src/Client.php, and 425
    // `new Client(..)` calls stopped reaching its constructor -- and `use
    // function` binds a name PSR-4 cannot place, which took 825 of koel's
    // own test helpers away from tests/Helpers.php.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("composer.json"),
        "{\"name\":\"acme/client\",\"require\":{\"guzzlehttp/psr7\":\"^2\"}}",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Client.php"),
        "<?php\n\nnamespace Acme;\n\nclass Client\n{\n    public function __construct()\n    {\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("Helpers.php"),
        "<?php\n\nnamespace Tests;\n\nfunction make_client(): void\n{\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("ClientTest.php"),
        "<?php\n\nuse Acme\\Client;\nuse GuzzleHttp\\Psr7\\Request;\n\nuse function Tests\\make_client;\n\nclass ClientTest\n{\n    public function testIt(): void\n    {\n        $client = new Client();\n        $request = new Request('GET', '/');\n        make_client();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("Request").as_deref(),
        Some("external"),
        "the file says Request is psr7's"
    );
    assert_eq!(
        resolution("Client").as_deref(),
        Some("constructor"),
        "a class the project declares is the project's"
    );
    assert_eq!(
        resolution("make_client").as_deref(),
        Some("resolved"),
        "PSR-4 places classes, not functions"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_module_names_the_file_that_holds_it_even_with_nothing_to_narrow() {
    // OCaml names a module after its file, and the rule was being used
    // only to choose between candidates. A call with no candidate at all
    // never reached it, so dune's own stdune answered none of its 683
    // `List.map`: 689 findings of that kind went away, and `Pp.textf`
    // reached the vendored pp it names 516 times.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n").unwrap();
    fs::write(
        root.join("lib").join("list.ml"),
        "let map f xs = List.rev (List.rev_map f xs)\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("run.ml"),
        "let all xs = List.map (fun x -> x) xs\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let edge = graph.edges.iter().find(|edge| {
        edge.kind == EdgeKind::Calls
            && edge.metadata.get("call_label").map(String::as_str) == Some("List.map")
    });
    assert_eq!(
        edge.and_then(|edge| edge.metadata.get("resolution").cloned())
            .as_deref(),
        Some("resolved"),
        "the module names the file that holds it"
    );
    // A project that declares its own `list.ml` means that one, not the
    // standard library's: 112 of dune's calls read as OCaml's `List` and
    // are its own stdune.
    let target = edge
        .and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .and_then(|node| node.span.as_ref())
        .map(|span| span.path.clone());
    assert_eq!(target.as_deref(), Some("lib/list.ml"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_ruby_class_that_descends_from_outside_answers_with_its_base() {
    // `where`, `present?`, `redirect_to` and `permit` are ActiveRecord's
    // and ActionController's, and none of them is among what the project
    // declares. mastodon writes 403 such calls from inside a class whose
    // ancestry leaves it, and calling them unresolved says a resolver
    // failed where a gem provides them.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app/models")).unwrap();
    fs::write(
        root.join("Gemfile"),
        "source 'https://rubygems.org'\ngem 'rails'\n",
    )
    .unwrap();
    fs::write(
        root.join("app/models").join("application_record.rb"),
        "class ApplicationRecord < ActiveRecord::Base\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/models").join("account.rb"),
        "class Account < ApplicationRecord\n  def suspended\n    where(suspended: true)\n  end\n\n  def own_helper\n    suspended\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("where").as_deref(),
        Some("external"),
        "the base is outside the project and so is its method"
    );
    // What the class writes itself is still what a call to it means.
    assert_eq!(resolution("suspended").as_deref(), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_php_test_case_gets_its_assertions_from_the_class_it_extends() {
    // `$this->assertSame(..)` is PHPUnit's, reached through the class the
    // test extends, and `$mock->shouldReceive(..)` is Mockery's: guzzle
    // writes 1800 such calls and koel a thousand more. A project that
    // declares an assertion helper of its own keeps its callers, which is
    // why the runner is asked last.
    let root = temp_project_root();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("composer.json"),
        "{\n  \"name\": \"acme/app\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("Helper.php"),
        "<?php\n\nnamespace Tests;\n\nclass Helper\n{\n    public function assertSongMatches($song): void\n    {\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("SongTest.php"),
        "<?php\n\nnamespace Tests;\n\nclass SongTest extends TestCase\n{\n    public function testItWorks(): void\n    {\n        $this->assertSame(1, 1);\n        $this->assertSongMatches(null);\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("assertSame").as_deref(),
        Some("builtin"),
        "the runner hands the test case its assertions"
    );
    assert_eq!(
        resolution("assertSongMatches").as_deref(),
        Some("resolved"),
        "and a helper the project writes keeps its caller"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn solidity_states_its_own_primitives_and_a_test_gets_its_cheatcodes() {
    // `require` and `keccak256` are the language's, `abi.encode` is how a
    // contract encodes what it sends, and `assertEq` and `vm.` come from
    // the Foundry base contract a test inherits. 887 of openzeppelin's
    // 3012 unresolved Solidity calls were one of those.
    let root = temp_project_root();
    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    fs::write(
        root.join("contracts").join("Vault.sol"),
        "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\n\ncontract Vault {\n    function store(uint256 amount) public {\n        require(amount > 0, \"empty\");\n        bytes32 key = keccak256(abi.encode(amount));\n        emit Stored(key);\n    }\n\n    event Stored(bytes32 key);\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("test").join("Vault.t.sol"),
        "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\n\ncontract VaultTest {\n    function testStore() public {\n        vm.assume(true);\n        assertEq(uint256(1), uint256(1));\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for builtin in ["require", "keccak256", "abi.encode"] {
        assert_eq!(
            resolution(builtin).as_deref(),
            Some("builtin"),
            "{builtin} is the language's own"
        );
    }
    for cheatcode in ["assertEq", "vm.assume"] {
        assert_eq!(
            resolution(cheatcode).as_deref(),
            Some("builtin"),
            "{cheatcode} is what a Foundry test inherits"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_table_is_a_sql_entity_wherever_it_is_declared() {
    // mastodon writes its schema in Ruby migrations and some of its
    // indexes in raw SQL, and each table took the language of the file
    // that declared it: an index and the table it belongs to then looked
    // like a link across languages, which is what `surprising-links`
    // ranked above every real one.
    let root = temp_project_root();
    fs::create_dir_all(root.join("db")).unwrap();
    fs::write(
        root.join("db").join("schema.rb"),
        "ActiveRecord::Schema[8.0].define(version: 2024_01_01_000000) do\n  create_table \"accounts\", force: :cascade do |t|\n    t.string \"username\"\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let table = graph
        .nodes
        .iter()
        .find(|node| node.label == "sql table:accounts")
        .expect("the migration declares the table");
    assert_eq!(
        table.metadata.get("language").map(String::as_str),
        Some("sql"),
        "a table is a SQL entity"
    );
    assert_eq!(
        table.metadata.get("declared_in").map(String::as_str),
        Some("ruby"),
        "and the language that declares it is a fact of its own"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_language_module_answers_only_where_the_project_declares_none() {
    // 1144 of dune's unresolved calls named an OCaml module the language
    // ships -- `Printf.sprintf`, `Unix.getenv`, `Filename.concat`. But
    // dune's own `stdune` library declares `String`, `List` and `Array`
    // of its own, and 19,897 of its qualified calls resolve into the
    // project that way: the language answers only where nothing else did.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n(name app)\n").unwrap();
    fs::write(
        root.join("src").join("string.ml"),
        "let capitalize value = value\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.ml"),
        "let run value =\n  let text = String.capitalize value in\n  Printf.sprintf \"%s\" text\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("String.capitalize").as_deref(),
        Some("resolved"),
        "a project that declares the module means its own"
    );
    assert_eq!(
        resolution("Printf.sprintf").as_deref(),
        Some("builtin"),
        "and the language answers for the module it ships"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn zigs_standard_library_is_the_languages_own() {
    // zls reaches the standard library through the constant its files
    // bind with `@import("std")`, and 775 of its 2955 unresolved calls
    // were `std.` -- while 174 more resolved into zls itself, so
    // `std.debug.print` claimed the project's own `print` as its target.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("build.zig.zon"), ".{\n    .name = .app,\n}\n").unwrap();
    fs::write(
        root.join("src").join("main.zig"),
        "const std = @import(\"std\");\n\npub fn print(value: []const u8) void {\n    _ = value;\n}\n\npub fn main() void {\n    std.debug.print(\"hello\", .{});\n    print(\"hello\");\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("std.debug.print").as_deref(),
        Some("builtin"),
        "the standard library answers for its own"
    );
    assert_eq!(
        resolution("print").as_deref(),
        Some("resolved"),
        "and the project's own function keeps its caller"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn asking_a_request_for_a_parameter_is_not_a_require() {
    // `params.require(:source)` is how a Rails controller reads a
    // parameter, and mastodon writes fifteen of them: each filed an import
    // of something called `params.require(:source)`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app").join("posts_controller.rb"),
        "require 'json'\n\nclass PostsController < ApplicationController\n  def create\n    params.require(:post).permit(:title)\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let imports: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("import")
                && node.metadata.get("language").map(String::as_str) == Some("ruby")
        })
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(
        imports,
        vec!["require 'json'"],
        "`require` is Kernel's, and a bare call is the only way to reach it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_flake_states_the_flakes_it_is_built_from() {
    // home-manager was the last project in the corpus whose dependencies
    // came from nowhere: a flake states them as `inputs`, flat or in a
    // block, and both forms sit in that one repository.
    let flat = nix_flake_dependencies(
        "{\n  description = \"Home Manager for Nix\";\n\n  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixpkgs-unstable\";\n\n  outputs = { self, nixpkgs, ... }: { };\n}\n",
    );
    assert_eq!(
        flat.iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        vec!["nixpkgs"]
    );

    let block = nix_flake_dependencies(
        "{\n  inputs = {\n    nixpkgs.url = \"github:NixOS/nixpkgs\";\n    scss-reset = {\n      url = \"github:andreymatin/scss-reset/1.4.2\";\n      inputs.nixpkgs.follows = \"nixpkgs\";\n    };\n  };\n\n  outputs = { self, nixpkgs, scss-reset }: { };\n}\n",
    );
    assert_eq!(
        block
            .iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        vec!["nixpkgs", "scss-reset"],
        "an input names itself once, however many times the file follows it"
    );
}

#[test]
fn every_other_ecosystem_states_what_it_needs_in_its_own_way() {
    // cowboy declared nothing at all, and ecto, kong, shellcheck,
    // DataFrames.jl, dplyr, cats and zls declared only the GitHub Actions
    // their workflows use.
    let mix = mix_dependencies(
        "  defp deps do\n    [\n      {:telemetry, \"~> 1.0\"},\n      {:jason, \"~> 1.0\", optional: true},\n      {:ex_doc, \"~> 0.38\", only: :docs}\n    ]\n  end\n",
    );
    let elixir = |name: &str| {
        mix.iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
    };
    assert_eq!(
        elixir("telemetry"),
        Some(("runtime", Some("~> 1.0".to_string())))
    );
    assert_eq!(
        elixir("ex_doc"),
        Some(("dev", Some("~> 0.38".to_string()))),
        "`only: :docs` says what the dependency is for"
    );

    let rebar = rebar_dependencies(
        "{deps, [\n{cowlib,\".*\",{git,\"https://github.com/ninenines/cowlib\",{tag,\"2.19.0\"}}},{ranch,\".*\",{git,\"https://github.com/ninenines/ranch\",{tag,\"1.8.1\"}}}\n]}.\n{erl_opts, [debug_info]}.\n",
    );
    assert_eq!(
        rebar
            .iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        vec!["cowlib", "ranch"],
        "a dependency tuple holds tuples of its own, and only the outermost names one"
    );

    let rockspec = rockspec_dependencies(
        "dependencies = {\n  \"lua >= 5.1\",\n  \"lua-resty-http == 0.17.2\",\n  \"penlight == 1.14.0\",\n}\n",
    );
    assert_eq!(
        rockspec
            .iter()
            .find(|dependency| dependency.name == "lua-resty-http")
            .and_then(|dependency| dependency.version.clone()),
        Some("== 0.17.2".to_string())
    );
    assert!(
        !rockspec.iter().any(|dependency| dependency.name == "lua"),
        "the language a rock runs on is not a rock"
    );

    let cabal = cabal_dependencies(
        "library\n    build-depends:\n      aeson >= 1.4.0 && < 2.3,\n      base >= 4.8.0.0 && < 5\n    ghc-options: -Wall\n\ntest-suite check\n    build-depends:\n      QuickCheck,\n      base\n",
    );
    let haskell = |name: &str| {
        cabal
            .iter()
            .filter(|dependency| dependency.name == name)
            .map(|dependency| dependency.kind.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(haskell("aeson"), vec!["runtime"]);
    assert_eq!(
        haskell("QuickCheck"),
        vec!["dev"],
        "a `test-suite` stanza states what the tests need"
    );
    assert_eq!(
        haskell("base"),
        vec!["runtime", "dev"],
        "and a stanza states what it needs even when another stanza needs it too"
    );

    let julia = julia_project_dependencies(
        "[deps]\nCompat = \"34da2185\"\nDataAPI = \"9a962f9c\"\n\n[extras]\nTest = \"8dfed614\"\n",
    );
    assert_eq!(
        julia
            .iter()
            .find(|dependency| dependency.name == "Test")
            .map(|dependency| dependency.kind.as_str()),
        Some("dev")
    );

    let description = r_description_dependencies(
        "Depends:\n    R (>= 4.1.0)\nImports:\n    cli (>= 3.6.2),\n    generics,\nSuggests:\n    covr,\n",
    );
    let r = |name: &str| {
        description
            .iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
    };
    assert_eq!(r("cli"), Some(("runtime", Some(">= 3.6.2".to_string()))));
    assert_eq!(r("generics"), Some(("runtime", None)));
    assert_eq!(r("covr"), Some(("dev", None)));
    assert!(
        r("R").is_none(),
        "the language a package runs on is not a package"
    );

    let sbt = sbt_dependencies(
        "  libraryDependencies ++= Seq(\n    \"org.typelevel\" %%% \"discipline-core\" % disciplineVersion,\n    \"org.scalameta\" %%% \"munit\" % munitVersion % Test\n  )\n",
    );
    let scala = |name: &str| {
        sbt.iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| dependency.kind.as_str())
    };
    assert_eq!(scala("org.typelevel:discipline-core"), Some("runtime"));
    assert_eq!(scala("org.scalameta:munit"), Some("dev"));

    let zon = zig_zon_dependencies(
        ".{\n    .name = .zls,\n    .dependencies = .{\n        .known_folders = .{\n            .url = \"https://example.com/known-folders.tar.gz\",\n        },\n        .diffz = .{\n            .url = \"https://example.com/diffz.tar.gz\",\n        },\n    },\n}\n",
    );
    assert_eq!(
        zon.iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        vec!["known_folders", "diffz"],
        "a dependency is a field of `.dependencies`, and its url is not one"
    );
}

#[test]
fn a_dotnet_project_states_the_packages_it_references() {
    // eShopOnWeb and Newtonsoft.Json declared nothing at all: a `.csproj`
    // was read by nobody, so 49 and 13 packages were invisible.
    let app = nuget_dependencies(
        std::path::Path::new("src/Web/Web.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk.Web\">\n  <ItemGroup>\n    <PackageReference Include=\"Azure.Identity\" Version=\"1.10.4\" />\n    <PackageReference Include=\"Ardalis.Specification\" />\n    <PackageReference Include=\"Microsoft.SourceLink.GitHub\" Version=\"$(SourceLinkVersion)\" PrivateAssets=\"All\" />\n  </ItemGroup>\n</Project>\n",
    );
    let declared = |name: &str| {
        app.iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
    };
    assert_eq!(
        declared("Azure.Identity"),
        Some(("runtime", Some("1.10.4".to_string())))
    );
    assert_eq!(
        declared("Ardalis.Specification"),
        Some(("runtime", None)),
        "a repository that manages versions centrally states none here"
    );
    assert_eq!(
        declared("Microsoft.SourceLink.GitHub"),
        Some(("dev", None)),
        "`PrivateAssets=\"All\"` builds the project and ships with nothing, \
         and a property is not a version the file states"
    );

    let tests = nuget_dependencies(
        std::path::Path::new("tests/UnitTests/UnitTests.csproj"),
        "<Project>\n  <ItemGroup>\n    <PackageReference Include=\"xunit\" Version=\"2.4.2\" />\n  </ItemGroup>\n</Project>\n",
    );
    assert_eq!(
        tests
            .iter()
            .find(|dependency| dependency.name == "xunit")
            .map(|dependency| dependency.kind.as_str()),
        Some("dev"),
        "a test project's packages are what its tests need"
    );
}

#[test]
fn maven_and_gradle_state_what_a_jvm_project_needs() {
    // gson declares 19 dependencies across four `pom.xml` files, petclinic
    // 30 in a Gradle build and retrofit 50 in a version catalog, and none
    // of them was read: a JVM project's dependencies came from nowhere.
    let maven = maven_dependencies(
        "<project>\n  <dependencyManagement>\n    <dependencies>\n      <dependency>\n        <groupId>com.example</groupId>\n        <artifactId>pinned</artifactId>\n        <version>1.0</version>\n      </dependency>\n    </dependencies>\n  </dependencyManagement>\n  <dependencies>\n    <dependency>\n      <groupId>com.google.code.gson</groupId>\n      <artifactId>gson</artifactId>\n      <version>${project.version}</version>\n    </dependency>\n    <dependency>\n      <groupId>junit</groupId>\n      <artifactId>junit</artifactId>\n      <version>4.13.2</version>\n      <scope>test</scope>\n    </dependency>\n  </dependencies>\n</project>\n",
    );
    let declared = |name: &str| {
        maven
            .iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
    };
    assert_eq!(
        declared("com.google.code.gson:gson"),
        Some(("runtime", None)),
        "a property is not a version the file states"
    );
    assert_eq!(
        declared("junit:junit"),
        Some(("dev", Some("4.13.2".to_string()))),
        "`<scope>test</scope>` says what the dependency is for"
    );
    assert!(
        declared("com.example:pinned").is_none(),
        "a `<dependencyManagement>` block pins a version for whoever \
         declares the dependency, and declares none of its own"
    );

    let gradle = gradle_dependencies(
        "dependencies {\n  implementation 'org.springframework.boot:spring-boot-starter-cache'\n  implementation(\"com.squareup.okio:okio:3.9.0\")\n  testImplementation 'org.junit.jupiter:junit-jupiter'\n  api libs.okhttp.client\n  implementation project(':shared')\n}\n",
    );
    let built = |name: &str| {
        gradle
            .iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
    };
    assert_eq!(
        built("org.springframework.boot:spring-boot-starter-cache"),
        Some(("runtime", None))
    );
    assert_eq!(
        built("com.squareup.okio:okio"),
        Some(("runtime", Some("3.9.0".to_string())))
    );
    assert_eq!(
        built("org.junit.jupiter:junit-jupiter"),
        Some(("dev", None))
    );
    assert_eq!(
        gradle.len(),
        3,
        "a catalog reference names an entry the catalog declares, and \
         `project(':shared')` is the repository's own module"
    );

    let catalog = gradle_version_catalog_dependencies(
        "[versions]\nokhttp = \"5.5.0\"\n\n[libraries]\nandroidPlugin = \"com.android.tools.build:gradle:9.3.1\"\nokhttp-client = { module = \"com.squareup.okhttp3:okhttp\", version.ref = \"okhttp\" }\nbnd = { module = \"biz.aQute.bnd:biz.aQute.bnd.gradle\", version = \"7.4.0\" }\n",
    );
    let listed = |name: &str| {
        catalog
            .iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| dependency.version.clone())
    };
    assert_eq!(
        listed("com.android.tools.build:gradle"),
        Some(Some("9.3.1".to_string()))
    );
    assert_eq!(
        listed("com.squareup.okhttp3:okhttp"),
        Some(Some("5.5.0".to_string())),
        "`version.ref` names an entry of the `[versions]` table"
    );
    assert_eq!(
        listed("biz.aQute.bnd:biz.aQute.bnd.gradle"),
        Some(Some("7.4.0".to_string()))
    );
}

#[test]
fn a_gemfile_and_a_gemspec_state_what_a_ruby_project_needs() {
    // Nothing read a Ruby manifest at all, so mastodon declared 154 gems
    // and the graph knew none of them: "which packages does it depend on"
    // answered with the GitHub Actions its workflows use.
    let dependencies = gemfile_dependencies(
        "# frozen_string_literal: true\n\nsource 'https://rubygems.org'\nruby '>= 3.3.0'\n\ngem 'rails', '~> 8.1.0'\ngem 'bootsnap', require: false\ngem 'aws-sdk-s3', '~> 1.123', require: false\n\ngroup :development, :test do\n  gem 'rspec-rails'\nend\n\ngroup :opentelemetry do\n  gem 'opentelemetry-sdk', '~> 1.4'\nend\n\ngem 'brakeman', group: :development\n",
    );
    let declared = |name: &str| {
        dependencies
            .iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
    };
    assert_eq!(
        declared("rails"),
        Some(("runtime", Some("~> 8.1.0".to_string()))),
        "a gem states the version it wants second"
    );
    assert_eq!(
        declared("bootsnap"),
        Some(("runtime", None)),
        "`require: false` is not a version"
    );
    assert_eq!(
        declared("aws-sdk-s3"),
        Some(("runtime", Some("~> 1.123".to_string())))
    );
    assert_eq!(
        declared("rspec-rails"),
        Some(("dev", None)),
        "a gem inside `group :development, :test` is not what the program runs on"
    );
    assert_eq!(
        declared("brakeman"),
        Some(("dev", None)),
        "and a group stated on the line says the same"
    );
    assert_eq!(
        declared("opentelemetry-sdk"),
        Some(("runtime", Some("~> 1.4".to_string()))),
        "while a group of its own is a choice about how the program runs"
    );
    assert!(
        declared("ruby").is_none() && declared("https://rubygems.org").is_none(),
        "the language and the source are not gems"
    );

    let gemspec = gemspec_dependencies(
        "Gem::Specification.new do |s|\n  s.add_dependency 'rack', '>= 3.0.0', '< 4'\n  s.add_dependency 'rack-protection', version\n  s.add_development_dependency 'rake'\nend\n",
    );
    let spec_declared = |name: &str| {
        gemspec
            .iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
    };
    assert_eq!(
        spec_declared("rack"),
        Some(("runtime", Some(">= 3.0.0".to_string())))
    );
    assert_eq!(
        spec_declared("rack-protection"),
        Some(("runtime", None)),
        "a version held in a variable is not one the file states"
    );
    assert_eq!(spec_declared("rake"), Some(("dev", None)));
}

#[test]
fn a_suggested_package_is_named_without_a_version() {
    // composer's `suggest` maps a package to a sentence about why to install
    // it. monolog suggests a dozen, one per optional handler.
    let dependencies = composer_dependencies(
        r#"{
            "require": {"php": ">=8.1", "psr/log": "^3"},
            "require-dev": {"predis/predis": "^1.1"},
            "suggest": {
                "predis/predis": "Allow sending log messages to a Redis server",
                "aws/aws-sdk-php": "Allow sending log messages to AWS services"
            }
        }"#,
    );
    let scopes = |name: &str| {
        dependencies
            .iter()
            .filter(|dependency| dependency.name == name)
            .map(|dependency| (dependency.kind.as_str(), dependency.version.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        scopes("aws/aws-sdk-php"),
        vec![("optional", None)],
        "a suggestion names a package, not a version"
    );
    assert_eq!(
        scopes("predis/predis"),
        vec![("dev", Some("^1.1".to_string())), ("optional", None)]
    );
    assert_eq!(scopes("psr/log"), vec![("runtime", Some("^3".to_string()))]);
}

#[test]
fn a_rust_module_path_is_read_inside_its_own_crate() {
    // `crate::` names the crate the file belongs to, and a workspace has one
    // per member: serde_derive's `use crate::internals::ast` is
    // `serde_derive/src/internals/mod.rs`, not a sibling crate's file.
    let target = possible_local_import_target(
        Language::Rust,
        "serde_derive/src/de/enum_.rs",
        "use crate::internals::ast::Variant;",
        &[],
        &[],
        &[],
    )
    .expect("a crate-relative module is a possible local import");
    assert_eq!(target.target, "internals");
    assert!(
        target
            .candidates
            .contains(&"serde_derive/src/internals/mod.rs".to_string()),
        "{:?}",
        target.candidates
    );
    // The crate root is tried before the file's own directory, so a module
    // that exists in both is read as the crate's.
    let position = |needle: &str| {
        target
            .candidates
            .iter()
            .position(|candidate| candidate == needle)
    };
    assert!(
        position("serde_derive/src/internals/mod.rs")
            < position("serde_derive/src/de/internals.rs"),
        "{:?}",
        target.candidates
    );

    // A crate laid out without `src/` is found by walking up: ripgrep keeps
    // `crates/core/flags/mod.rs` next to `crates/core/main.rs`.
    let ripgrep = possible_local_import_target(
        Language::Rust,
        "crates/core/flags/complete/bash.rs",
        "use crate::flags::defs::FLAGS;",
        &[],
        &[],
        &[],
    )
    .expect("a module path away from src/ is still a candidate");
    assert!(
        ripgrep
            .candidates
            .contains(&"crates/core/flags/mod.rs".to_string()),
        "{:?}",
        ripgrep.candidates
    );

    // And a module a compiling crate names is never a missing file: it may
    // be written inline or re-exported, so a miss stays quiet.
    assert!(
        local_import_target(
            Language::Rust,
            "serde/src/private/ser.rs",
            "use crate::ser::Impossible;",
            &[],
            &[],
            &[],
        )
        .is_none()
    );
}

#[test]
fn a_setting_read_is_not_a_route() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    // `app.get('json escape')` reads a setting; express reads eleven of
    // them in `lib/response.js` alone, and each looked like a route.
    fs::write(
        root.join("lib").join("response.js"),
        "const app = require('./app')\nfunction send(res) {\n  const escape = app.get('json escape')\n  return escape\n}\napp.get('/users', function listUsers(req, res) { res.end() })\nmodule.exports = send\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "framework_route")
        })
        .filter_map(|node| node.metadata.get("path").cloned())
        .collect();
    assert_eq!(routes, vec!["/users".to_string()], "{routes:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_schema_written_in_lua_is_a_schema() {
    // Kong keeps every table in a migration's long string, in capitals.
    let table = parse_sql_create_table(
        "CREATE TABLE IF NOT EXISTS \"plugins\" (\n  id UUID PRIMARY KEY,\n  name TEXT\n)",
    )
    .expect("a guarded create still names its table");
    assert_eq!(table.name, "plugins");

    let literals = source_sql_literals(
        "return {\n  postgres = {\n    up = [[\n      CREATE TABLE IF NOT EXISTS acls (id UUID);\n    ]]\n  }\n}\n",
    );
    assert_eq!(literals.len(), 1, "{literals:?}");
    assert!(literals[0].value.contains("CREATE TABLE"));
    assert_eq!(literals[0].line, 3, "the literal starts where Lua opens it");

    let root = temp_project_root();
    fs::create_dir_all(root.join("migrations")).unwrap();
    fs::write(
        root.join("migrations").join("001_init.lua"),
        "return {\n  postgres = {\n    up = [[\n      CREATE TABLE IF NOT EXISTS plugins (\n        id UUID PRIMARY KEY,\n        name TEXT\n      );\n    ]]\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("dao.lua"),
        "local function select_plugins(db)\n  return db:query(\"SELECT id, name FROM plugins\")\nend\n\nreturn select_plugins\n",
    )
    .unwrap();

    // A comment describing SQL is not a schema, and not a query either.
    fs::write(
        root.join("docs.lua"),
        "-- `DROP TABLE [IF EXISTS] <table>` removes a table.\n-- CREATE TABLE notes (id INT);\nreturn {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    assert_graph_invariants(&graph);
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.label == "sql table:notes"),
        "a commented-out create declares nothing"
    );
    let table = graph
        .nodes
        .iter()
        .find(|node| node.label == "sql table:plugins")
        .expect("the Lua migration declares the table");
    assert_eq!(
        table.span.as_ref().map(|span| span.path.as_str()),
        Some("migrations/001_init.lua")
    );
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.label == "sql column:plugins.name"),
        "its columns come with it"
    );
    let query = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "app_sql_query")
        })
        .expect("the query is indexed");
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.source == query.id && edge.target == table.id),
        "the query reaches the table the migration created"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_file_does_not_import_itself() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("flask")).unwrap();
    // `import typing as t` inside `typing.py` names the standard library,
    // and a fixture named `flask.py` importing flask names the package.
    fs::write(
        root.join("src").join("flask").join("typing.py"),
        "import typing as t


def ensure(value):
    return t.cast(str, value)
",
    )
    .unwrap();
    fs::write(
        root.join("src").join("flask").join("__init__.py"),
        "from .typing import ensure
",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let self_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::References)
        .filter_map(|edge| {
            let source = graph.nodes.iter().find(|node| node.id == edge.source)?;
            let target = graph.nodes.iter().find(|node| node.id == edge.target)?;
            let path = source.span.as_ref()?.path.as_str();
            (path == target.label).then(|| format!("{path} -> {}", target.label))
        })
        .collect();
    assert!(
        self_edges.is_empty(),
        "no import resolves onto the file it is written in: {self_edges:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn pyproject_dependency_groups_are_declared_dependencies() {
    // PEP 735 groups, which uv writes and pip installs with `--group`.
    // flask keeps `cryptography` and `python-dotenv` only here.
    let dependencies = pyproject_dependencies(
        r#"[project]
name = "flask"
dependencies = ["click>=8.1.3"]

[project.optional-dependencies]
dotenv = ["python-dotenv"]

[dependency-groups]
dev = ["ruff", {include-group = "tests"}]
typing = ["cryptography", "mypy"]
"#,
    );
    let named = |name: &str| {
        dependencies
            .iter()
            .find(|dependency| dependency.name == name)
            .map(|dependency| dependency.kind.as_str())
    };
    assert_eq!(named("click"), Some("runtime"));
    assert_eq!(named("python-dotenv"), Some("optional"));
    assert_eq!(named("cryptography"), Some("dev"));
    assert_eq!(named("ruff"), Some("dev"));
    assert_eq!(named("mypy"), Some("dev"));
    // `{include-group = "tests"}` names a group rather than a package.
    assert_eq!(named("tests"), None);
}

#[test]
fn a_quoted_include_of_a_system_header_is_not_a_missing_file() {
    // redis writes four of its libc includes with quotes, which searches
    // next to the file first and the system path second.
    for header in ["stdio.h", "limits.h", "ctype.h", "sys/socket.h"] {
        let include = format!("#include \"{header}\"");
        assert!(
            local_import_target(Language::C, "src/mstr.c", &include, &[], &[], &[]).is_none(),
            "{header} is the toolchain's"
        );
        let possible =
            possible_local_import_target(Language::C, "src/mstr.c", &include, &[], &[], &[])
                .unwrap_or_else(|| panic!("{header} still resolves against a project copy"));
        assert_eq!(possible.target, header);
        assert!(possible.candidates.contains(&format!("src/{header}")));
    }

    // A project header keeps its own resolution, misses included.
    let project = local_import_target(
        Language::C,
        "src/mstr.c",
        "#include \"release.h\"",
        &[],
        &[],
        &[],
    )
    .expect("a project header is a local import");
    assert_eq!(project.target, "release.h");
}

#[test]
fn a_command_names_the_path_the_shell_would_run() {
    let path_of = |label: &str, command: &str| {
        normalized_command_path_candidate(label, command).map(|candidate| candidate.path)
    };
    // `(cd ..; ./runtest)` runs the script one directory up from the
    // Makefile that says so, not next to it.
    assert_eq!(
        path_of("src/Makefile", "(cd ..; ./runtest)"),
        Some("runtest".to_string())
    );
    assert_eq!(
        path_of("src/Makefile", "./redis-server test all"),
        Some("src/redis-server".to_string())
    );

    // A make variable, a Go package walk, text to read, and where the
    // output goes: none of them is a file in the project.
    for command in [
        "./$(REDIS_BENCHMARK_NAME)",
        "go generate ./...",
        "echo \"Please specify AE_DIR (e.g. <redis repository>/src)\"",
        "(cd hiredis && $(MAKE) clean) > /dev/null || true",
        // A tap and a formula, a project and its task, a build alias.
        "brew install alamofire/alamofire/firewalk",
        "sbt docs/tlSite",
        "dune build @doc/runtest --auto-promote",
    ] {
        assert_eq!(path_of("deps/hiredis/Makefile", command), None, "{command}");
    }

    // A command that deletes or copies to a path still names it, with a note
    // that the path is written rather than read.
    let removed = normalized_command_path_candidate("Makefile", "rm -f include/hedley.hpp")
        .expect("rm names the file it deletes");
    assert_eq!(removed.path, "include/hedley.hpp");
    assert!(removed.written);

    let copied = normalized_command_path_candidate("Makefile", "cp -r doc/index.html public/out")
        .expect("cp names what it copies");
    assert_eq!(copied.path, "doc/index.html");
    assert!(!copied.written);

    // Every command in a chain is read with its own program, so the script
    // after a package install is still a path.
    let chained = normalized_command_path_candidate(
        "Makefile",
        "brew install alamofire/firewalk && ./scripts/check.sh",
    )
    .expect("the second command names a script");
    assert_eq!(chained.path, "scripts/check.sh");
    assert!(!chained.written);
}

#[test]
fn a_typescript_import_finds_the_file_typescript_would() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("template")).unwrap();
    fs::write(
        root.join("src").join("app.ts"),
        "import { expectType } from './utils'\nimport { isVSlot } from './vFor.spec'\nimport readme from './template/index.html?raw'\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("utils.d.ts"),
        "export declare function expectType(): void\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("vFor.spec.ts"),
        "export const isVSlot = true\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("template").join("index.html"),
        "<!-- -->\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolved = |written: &str| -> String {
        graph
            .nodes
            .iter()
            .find(|node| node.label.contains(written))
            .and_then(|node| node.metadata.get("resolved_path").cloned())
            .unwrap_or_else(|| format!("`{written}` did not resolve"))
    };

    // A declaration file is what `./utils` names when it is the only
    // `utils` around, `vFor.spec` wears a dot without being a file
    // extension, and `?raw` tells a bundler how to load the file rather
    // than which file to load.
    assert_eq!(resolved("'./utils'"), "src/utils.d.ts");
    assert_eq!(resolved("'./vFor.spec'"), "src/vFor.spec.ts");
    assert_eq!(
        resolved("'./template/index.html?raw'"),
        "src/template/index.html"
    );

    fs::remove_dir_all(root).unwrap();
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

    let environment_default = |label: &str, expected: &str| {
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Environment
                && node.label == label
                && graph.edges.iter().any(|edge| {
                    edge.target == node.id
                        && edge.kind == EdgeKind::ReadsEnvironment
                        && edge
                            .metadata
                            .get("default_value")
                            .is_some_and(|value| value == expected)
                })
        })
    };
    assert!(environment_default("PORT", "8080"));
    assert!(environment_default("API_URL", "http://localhost"));
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

    // `flutter:` with `sdk: flutter` written under it declares a
    // dependency whose source, not whose version, is on the next line.
    // Requiring a value on the same line dropped every dependency that
    // comes from an SDK, a path, or a git remote, and then the imports of
    // those packages read as undeclared.
    let flutter = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "flutter"
                && node
                    .metadata
                    .get("item_kind")
                    .is_some_and(|kind| kind == "dependency")
        })
        .expect("flutter is declared, from the SDK");
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::DependsOn
            && edge.target == flutter.id
            && edge
                .metadata
                .get("dependency_kind")
                .is_some_and(|value| value == "runtime")
            && !edge.metadata.contains_key("dependency_version")
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
fn base_r_is_builtin_but_a_dependency_is_not() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("R")).unwrap();
    fs::write(
        root.join("R").join("summarise.R"),
        r#"summarise <- function(.data, ...) {
  cols <- names(.data)
  if (is.null(cols)) {
    abort("no columns")
  }
  UseMethod("summarise")
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution_of = |call_label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge
                        .metadata
                        .get("call_label")
                        .is_some_and(|label| label == call_label)
            })
            .and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
            .and_then(|node| node.metadata.get("resolution").cloned())
            .unwrap_or_else(|| panic!("no call edge for {call_label}"))
    };

    // base is attached in every R session.
    assert_eq!(resolution_of("names"), "builtin");
    assert_eq!(resolution_of("UseMethod"), "builtin");
    // `abort` reads as just as fundamental in modern R, but it comes from
    // rlang — a dependency, and calling it builtin would be a lie. The
    // project declares nothing by that name, which is what `external` says.
    assert_eq!(resolution_of("abort"), "external");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn elixir_kernel_calls_are_builtin() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib").join("schema.ex"),
        r#"defmodule Schema do
  def cast(value) do
    if is_list(value) do
      length(value)
    else
      normalize(value)
    end
  end

  def normalize(value), do: value
end
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution_of = |call_label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge
                        .metadata
                        .get("call_label")
                        .is_some_and(|label| label == call_label)
            })
            .and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
            .map(|node| (node.kind.clone(), node.metadata.get("resolution").cloned()))
            .unwrap_or_else(|| panic!("no call edge for {call_label}"))
    };

    // Kernel is imported into every module: `is_list` and `length` are the
    // language, not something this project failed to declare.
    assert_eq!(
        resolution_of("is_list").1.as_deref(),
        Some("builtin"),
        "got {:?}",
        resolution_of("is_list")
    );
    assert_eq!(resolution_of("length").1.as_deref(), Some("builtin"));
    // ...but what the module declares is still the module's.
    assert_eq!(resolution_of("normalize").0, NodeKind::Function);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn julia_base_calls_are_builtin_unless_the_project_declares_them() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("frame.jl"),
        r#"function nrow(df)
    return 1
end

function describe(df)
    n = nrow(df)
    return length(df) + n
end
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let target_of = |call_label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge
                        .metadata
                        .get("call_label")
                        .is_some_and(|label| label == call_label)
            })
            .and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
            .unwrap_or_else(|| panic!("no call edge for {call_label}"))
    };

    // `length` is in scope in every Julia file and declared nowhere the scan
    // can see; calling that unresolved reads as a resolver that failed.
    assert_eq!(
        target_of("length")
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("builtin")
    );
    // ...but a name the project declares is still the project's.
    assert_eq!(target_of("nrow").kind, NodeKind::Function);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_python_module_qualifier_says_where_a_call_goes() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app").join("views.py"),
        "def render():\n    return 1\n",
    )
    .unwrap();
    // A local helper with the same name as the external one being called.
    fs::write(
        root.join("app").join("helpers.py"),
        "def echo(message):\n    return message\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("cli.py"),
        r#"import click

from . import views


def run():
    click.echo("hello")
    return views.render()
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let target_of = |call_label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge
                        .metadata
                        .get("call_label")
                        .is_some_and(|label| label == call_label)
            })
            .and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
            .unwrap_or_else(|| panic!("no call edge for {call_label}"))
    };

    // `click` is not in this project, so the call leaves it — matching the
    // name against a local `echo` would be a false link, not a resolution.
    let echo = target_of("click.echo");
    assert_eq!(
        echo.metadata.get("resolution").map(String::as_str),
        Some("external")
    );

    // `from . import views` names a module, and a Python module is a file.
    let render = target_of("views.render");
    assert_eq!(render.kind, NodeKind::Function);
    assert_eq!(
        render.span.as_ref().map(|span| span.path.as_str()),
        Some("app/views.py")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_unqualified_go_call_stays_in_its_own_package() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("alpha")).unwrap();
    fs::create_dir_all(root.join("beta")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.23\n").unwrap();
    // The same helper name in two packages. Written without a qualifier, the
    // call can only mean the one next door — Go has no other reading.
    fs::write(
        root.join("alpha").join("helper.go"),
        "package alpha\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        root.join("beta").join("helper.go"),
        "package beta\n\nfunc helper() int { return 2 }\n",
    )
    .unwrap();
    fs::write(
        root.join("alpha").join("run.go"),
        "package alpha\n\nfunc run() int { return helper() }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge
                    .metadata
                    .get("call_label")
                    .is_some_and(|label| label == "helper")
        })
        .expect("missing call edge");
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("target node");

    assert_eq!(target.kind, NodeKind::Function);
    assert_eq!(
        target.span.as_ref().map(|span| span.path.as_str()),
        Some("alpha/helper.go"),
        "an unqualified call cannot reach another package"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_qualified_receiver_type_picks_the_package_it_names() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("tfdiags")).unwrap();
    fs::create_dir_all(root.join("addrs")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.23\n").unwrap();
    // The same type name with the same method in two packages: neither the
    // label nor the owner's name alone can choose between them.
    fs::write(
        root.join("tfdiags").join("diagnostics.go"),
        "package tfdiags\n\ntype Diagnostics struct{}\n\nfunc (d Diagnostics) Append() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("addrs").join("addrs.go"),
        "package addrs\n\ntype Diagnostics struct{}\n\nfunc (d Diagnostics) Append() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("run.go"),
        r#"package app

import "example.com/app/tfdiags"

func run() {
    var diags tfdiags.Diagnostics
    diags.Append()
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge
                    .metadata
                    .get("call_label")
                    .is_some_and(|label| label == "diags.Append")
        })
        .expect("missing call edge");
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("target node");

    // `var diags tfdiags.Diagnostics` states both halves: the type picks the
    // method, the package picks which `Diagnostics`.
    assert_eq!(target.kind, NodeKind::Function);
    assert_eq!(
        target.span.as_ref().map(|span| span.path.as_str()),
        Some("tfdiags/diagnostics.go")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_receiver_from_outside_the_repository_is_an_external_call() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.23\n").unwrap();
    fs::write(
        root.join("worker.go"),
        r#"package app

import (
    "context"
    "testing"
)

type worker struct{}

func (w *worker) work() {}

func Fatalf(format string) {}

func TestWorker(t *testing.T) {
    t.Fatalf("boom")
}

func run(ctx context.Context) {
    ctx := &worker{}
    ctx.work()
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let target_of = |call_label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge
                        .metadata
                        .get("call_label")
                        .is_some_and(|label| label == call_label)
            })
            .and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
            .unwrap_or_else(|| panic!("no call edge for {call_label}"))
    };

    // `t` is a `*testing.T`: whatever `Fatalf` means, it is not this file's
    // own `Fatalf`, and calling it unresolved suggests a resolver that failed
    // rather than a dependency that left.
    let fatalf = target_of("t.Fatalf");
    assert_eq!(
        fatalf.metadata.get("resolution").map(String::as_str),
        Some("external"),
        "got {:?}",
        fatalf.label
    );

    // ...but a name the body re-declares is no longer what the signature said.
    let work = target_of("ctx.work");
    assert_eq!(work.kind, NodeKind::Function);
    assert_eq!(work.label, "work");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_declared_receiver_type_picks_the_method_it_owns() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.23\n").unwrap();
    fs::write(
        root.join("store.go"),
        r#"package app

type Reader struct{}

func (r *Reader) Load() string { return "read" }

type Writer struct{}

func (w *Writer) Load() string { return "write" }

func run(w *Writer) string {
    return w.Load()
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let node_at = |line: u32| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.start_line == line)
            })
            .unwrap_or_else(|| panic!("no function starting on line {line}"))
    };
    let reader_load = node_at(5);
    let writer_load = node_at(9);
    let run = node_at(11);
    assert_eq!(
        writer_load.metadata.get("owner_type").map(String::as_str),
        Some("Writer")
    );

    // Both methods are called `Load`; only the parameter's declared type says
    // which one `w.Load()` means.
    let called: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.source == run.id && edge.kind == EdgeKind::Calls)
        .map(|edge| edge.target)
        .collect();
    assert!(called.contains(&writer_load.id), "w is a *Writer");
    assert!(!called.contains(&reader_load.id), "w is not a *Reader");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_value_built_in_place_carries_the_type_it_was_built_from() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.23\n").unwrap();
    fs::write(
        root.join("build.go"),
        r#"package app

type Action struct{ Name string }

func (a Action) Describe() string { return a.Name }

type Resource struct{ Name string }

func (r *Resource) Describe() string { return r.Name }

func run() {
    action := Action{Name: "x"}
    res := &Resource{Name: "y"}
    counts := map[string]int{}
    _ = counts
    println(action.Describe())
    println(res.Describe())
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let node_at = |line: u32| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.start_line == line)
            })
            .unwrap_or_else(|| panic!("no function starting on line {line}"))
    };
    let action_describe = node_at(5);
    let resource_describe = node_at(9);
    let run = node_at(11);

    // `action := Action{...}` writes the type at the assignment, and
    // `&Resource{...}` is a pointer to one, which carries the same methods.
    // `map[string]int{}` states a shape rather than a name and has no
    // methods to confuse them with.
    let called: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.source == run.id && edge.kind == EdgeKind::Calls)
        .map(|edge| edge.target)
        .collect();
    assert!(called.contains(&action_describe.id), "action is an Action");
    assert!(called.contains(&resource_describe.id), "res is a *Resource");
    assert_eq!(
        graph
            .edges
            .iter()
            .filter(|edge| edge.source == run.id
                && edge.kind == EdgeKind::Calls
                && edge
                    .metadata
                    .get("call_label")
                    .is_some_and(|label| label.ends_with("Describe")))
            .count(),
        2,
        "each call goes to one method, not to both"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_nested_helper_is_only_visible_inside_its_own_function() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.py"),
        r#"def outer_a():
    def helper():
        return 1

    return helper()


def outer_b():
    def helper():
        return 2

    return helper()
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let function_at = |line: u32| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.start_line == line)
            })
            .unwrap_or_else(|| panic!("no function starting on line {line}"))
    };
    let outer_a = function_at(1);
    let helper_a = function_at(2);
    let helper_b = function_at(9);
    assert_eq!(
        helper_a
            .metadata
            .get("enclosing_function")
            .map(String::as_str),
        Some("outer_a")
    );

    // Both helpers share a label, so matching by name alone made the call
    // ambiguous. Lexical scope settles it: `outer_a` can only mean its own.
    let called: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.source == outer_a.id && edge.kind == EdgeKind::Calls)
        .map(|edge| edge.target)
        .collect();
    assert!(
        called.contains(&helper_a.id),
        "outer_a should call its own helper"
    );
    assert!(
        !called.contains(&helper_b.id),
        "outer_a cannot see outer_b's helper"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn call_edges_carry_the_call_site() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.py"),
        r#"def helper():
    return 1


def main():
    return helper()
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let main_id = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "main")
        .expect("missing main")
        .id;
    let call = graph
        .edges
        .iter()
        .find(|edge| edge.source == main_id && edge.kind == EdgeKind::Calls)
        .expect("missing call edge");

    // The call is on the last line, not where `main` is declared: a semantic
    // pass asking "what is defined here?" has to ask at the call.
    assert_eq!(call.metadata.get("line").map(String::as_str), Some("6"));
    assert!(
        call.metadata.contains_key("column"),
        "call edges carry a column too: {:?}",
        call.metadata
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn go_qualified_calls_resolve_through_the_import_list() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("states")).unwrap();
    fs::create_dir_all(root.join("internal").join("plans")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.23\n").unwrap();
    fs::write(
        root.join("internal").join("states").join("state.go"),
        "package states\n\nfunc NewState() int { return 1 }\n",
    )
    .unwrap();
    // A same-named function in another package: matching by name alone cannot
    // tell the two apart.
    fs::write(
        root.join("internal").join("plans").join("plan.go"),
        "package plans\n\nfunc NewState() int { return 2 }\n",
    )
    .unwrap();
    fs::write(
        root.join("main.go"),
        "package main\n\nimport (\n    \"strings\"\n    \"example.com/app/internal/states\"\n)\n\nfunc main() {\n    _ = states.NewState()\n    _ = strings.Contains(\"a\", \"b\")\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let main_id = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && node.label == "main"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path == "main.go")
        })
        .expect("missing main")
        .id;
    let called = graph
        .edges
        .iter()
        .filter(|edge| edge.source == main_id && edge.kind == EdgeKind::Calls)
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .collect::<Vec<_>>();

    // The import says which package `states` is, so the call lands in that
    // package and not in `internal/plans`.
    let new_state = called
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "NewState")
        .expect("states.NewState should resolve to a function");
    assert_eq!(
        new_state.span.as_ref().map(|span| span.path.as_str()),
        Some("internal/states/state.go")
    );

    // `strings` is not in the repository, so the call is external rather than
    // an unresolved or ambiguous in-repo name.
    let contains = called
        .iter()
        .find(|node| node.label == "strings.Contains")
        .expect("missing strings.Contains target");
    assert_eq!(
        contains.metadata.get("resolution").map(String::as_str),
        Some("external")
    );
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
            // Both sit in one file, so the syntax settles it rather than a
            // name match across the repository.
            && edge.confidence == Confidence::Syntactic
            && edge.metadata.get("resolution_basis").map(String::as_str) == Some("same_file")
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
fn a_configurations_declarations_refer_to_each_other() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("env").join("prod")).unwrap();
    fs::write(
        root.join("env").join("prod").join("main.tf"),
        "variable \"region\" {\n  type = string\n\n  validation {\n    condition = length(var.region) > 0\n  }\n}\n\nresource \"aws_instance\" \"web\" {\n  region = var.region\n}\n\noutput \"id\" {\n  value = aws_instance.web.id\n}\n",
    )
    .unwrap();
    // A second module declaring the same name: a reference means what its
    // own directory declares.
    fs::create_dir_all(root.join("env").join("dev")).unwrap();
    fs::write(
        root.join("env").join("dev").join("main.tf"),
        "variable \"region\" {\n  type = string\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let label = |id: NodeId| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    };
    let references = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::References
                && edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
        })
        .map(|edge| (label(edge.source), label(edge.target)))
        .collect::<Vec<_>>();

    assert!(references.contains(&("aws_instance.web".to_string(), "var.region".to_string())));
    assert!(references.contains(&("output.id".to_string(), "aws_instance.web".to_string())));
    assert!(
        !references.iter().any(|(source, target)| source == target),
        "a variable's own validation reads the variable it states a rule for"
    );
    let declaring_file = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.label == label && node.kind == NodeKind::Type)
            .and_then(|node| node.span.as_ref())
            .map(|span| span.path.clone())
            .unwrap_or_default()
    };
    assert!(declaring_file("aws_instance.web").starts_with("env/prod/"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reading_files_ahead_of_the_walk_changes_nothing_it_finds() {
    // Files are read into facts on every core before the graph is
    // assembled; more files than one round holds proves the rounds line up
    // with the walk.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    for index in 0..1200 {
        fs::write(
            root.join("src").join(format!("module{index}.rs")),
            format!("pub fn helper{index}() {{ other{index}(); }}\n\nfn other{index}() {{}}\n"),
        )
        .unwrap();
    }
    // A header whose contents, not its extension, say which language it is:
    // the round that reads it must decide the same way the walk would.
    fs::write(
        root.join("src").join("shape.h"),
        "namespace shapes {\nclass Circle { public: double area(); };\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    // Reading runs one round ahead of the walk, so a project of more than
    // one round proves the rounds line up with it.
    let again = scan_project(&root, &IndexOptions::default()).unwrap();
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| (format!("{:?}", node.kind), node.label.clone()))
            .collect::<Vec<_>>(),
        again
            .nodes
            .iter()
            .map(|node| (format!("{:?}", node.kind), node.label.clone()))
            .collect::<Vec<_>>(),
        "the same project reads the same way twice"
    );
    let functions = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function)
        .count();
    assert!(functions >= 600, "every file was read: {functions}");
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.label == "src/shape.h")
            .and_then(|node| node.metadata.get("language"))
            .map(String::as_str),
        Some("cpp")
    );
    let calls = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls)
        .count();
    assert!(calls >= 300, "and its calls were resolved: {calls}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_typescript_import_names_the_compiled_file_and_finds_the_source() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // TypeScript requires the compiled name in an ESM specifier, and the
    // file on disk is the source: zod writes 61 imports this way.
    fs::write(
        root.join("src").join("index.ts"),
        "import { snapshot } from \"./snapshot.js\";\n\nexport const run = () => snapshot();\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("snapshot.ts"),
        "export function snapshot() { return 1; }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let import = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::ExternalDependency && node.label.contains("./snapshot.js")
        })
        .expect("the import is recorded");
    assert_eq!(
        import.metadata.get("resolved_path").map(String::as_str),
        Some("src/snapshot.ts")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_path_built_as_the_program_runs_is_not_one_file() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("commands")).unwrap();
    fs::write(root.join("src").join("commands").join("get.json"), "{}\n").unwrap();
    fs::write(
        root.join("linter.js"),
        "const schema = require('./src/commands/' + name);\nconst fs = require('node:fs');\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let imports = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::ExternalDependency)
        .map(|node| node.label.as_str())
        .collect::<Vec<_>>();
    assert!(imports.iter().any(|label| label.contains("node:fs")));
    // redis writes `require('../src/commands/' + command_schema)`, and the
    // literal in front of the `+` is a prefix rather than a file.
    assert!(
        !imports.iter().any(|label| label.contains("src/commands")),
        "{imports:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_require_written_in_a_comment_is_not_an_import() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib").join("application.js"),
        "/**\n * Register a view engine:\n *\n *     app.engine('ejs', require('ejs').__express);\n */\nconst http = require('node:http');\n// require('debug') is what we used to do\nmodule.exports = http;\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let imported = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::ExternalDependency)
        .map(|node| node.label.as_str())
        .collect::<Vec<_>>();
    assert!(
        imported.iter().any(|label| label.contains("node:http")),
        "{imported:?}"
    );
    // Express documents its view engines in a comment above the method.
    assert!(
        !imported.iter().any(|label| label.contains("ejs")),
        "{imported:?}"
    );
    assert!(
        !imported.iter().any(|label| label.contains("debug")),
        "{imported:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_import_a_project_handles_the_absence_of_is_optional() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("compat.py"),
        // The two shapes a project writes: the import in the `try`, and the
        // one in the `else` that runs when the `try` succeeded.
        "try:\n    import simplejson as json\nexcept ImportError:\n    import json\n\ntry:\n    from urllib3.contrib import pyopenssl\nexcept ImportError:\n    pyopenssl = None\nelse:\n    import cryptography\n\nimport requests\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let optional = |needle: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::ExternalDependency && node.label.contains(needle))
            .map(|node| node.metadata.get("optional").map(String::as_str) == Some("true"))
    };
    assert_eq!(optional("simplejson"), Some(true));
    assert_eq!(optional("cryptography"), Some(true));
    // An import nothing guards is what the program needs to run.
    assert_eq!(optional("import requests"), Some(false));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_renamed_cargo_dependency_is_declared_under_both_names() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"searcher\"\nversion = \"0.1.0\"\n\n[dependencies]\nmemmap = { package = \"memmap2\", version = \"0.9.0\" }\n",
    )
    .unwrap();
    fs::write(root.join("src").join("lib.rs"), "use memmap::Mmap;\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let declared = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::ExternalDependency)
        .filter(|node| node.metadata.get("ecosystem").map(String::as_str) == Some("cargo"))
        .map(|node| node.label.as_str())
        .collect::<Vec<_>>();
    // The key is the name the code writes; the registry name travels beside
    // it.
    assert!(declared.contains(&"memmap"), "{declared:?}");
    assert!(declared.contains(&"memmap2"), "{declared:?}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_entrypoint_points_at_the_line_that_declares_it() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        "{\n  \"name\": \"demo\",\n  \"scripts\": {\n    \"start\": \"node server.js\",\n    \"lint:fix\": \"eslint --fix .\"\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("run.sh"),
        "#!/usr/bin/env bash\nset -euo pipefail\necho running\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let span_of = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Entrypoint && node.label == label)
            .and_then(|node| node.span.clone())
    };

    let start = span_of("npm script:start").expect("the npm script is an entrypoint");
    assert_eq!(start.path, "package.json");
    assert_eq!(start.start_line, 4, "the line that declares it");
    // A name with a colon in it is still one name.
    let lint = span_of("npm script:lint:fix").expect("the second script too");
    assert_eq!(lint.start_line, 5);
    // A script is a program because of its first line.
    let script = span_of("script:run.sh").expect("the shebang makes it a program");
    assert_eq!(script.path, "run.sh");
    assert_eq!(script.start_line, 1);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_objective_c_header_is_read_as_the_language_it_states() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // `.h` is C's extension, C++'s and Objective-C's alike; only what the
    // file states says which it is.
    fs::write(
        root.join("src").join("Manager.h"),
        "#import <Foundation/Foundation.h>\n\n@interface Manager : NSObject\n- (void)startWithURL:(NSURL *)url;\n@end\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Manager.m"),
        "#import \"Manager.h\"\n\n@implementation Manager\n- (void)startWithURL:(NSURL *)url {\n  [self startWithURL:url];\n}\n@end\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.label == "src/Manager.h")
            .and_then(|node| node.metadata.get("language"))
            .map(String::as_str),
        Some("objc")
    );
    // A selector is one name, and the header and the implementation state
    // the same one.
    assert!(
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Function)
            .filter(|node| node.label == "startWithURL:")
            .count()
            >= 2,
        "the header declares it and the implementation defines it"
    );
    assert!(
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Function
                && node.label == "startWithURL:"
                && node.metadata.get("owner_type").map(String::as_str) == Some("Manager")
        }),
        "and it belongs to the class that states it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_contract_reaches_the_contract_it_inherits() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::write(
        root.join("contracts").join("Ownable.sol"),
        "pragma solidity ^0.8.20;\n\ncontract Ownable {\n    address public owner;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("contracts").join("Token.sol"),
        "pragma solidity ^0.8.20;\n\nimport {Ownable} from \"./Ownable.sol\";\n\ncontract Token is Ownable {\n    function transfer(address to) external returns (bool) { return true; }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let label = |id: NodeId| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    };
    let references = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::References
                && edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
        })
        .map(|edge| (label(edge.source), label(edge.target)))
        .collect::<Vec<_>>();
    assert!(
        references.contains(&("Token".to_string(), "Ownable".to_string())),
        "what a contract inherits is what it is made of: {references:?}"
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::ExternalDependency && node.label == "./Ownable.sol"
            })
            .and_then(|node| node.metadata.get("resolved_path"))
            .map(String::as_str),
        Some("contracts/Ownable.sol")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_proto_import_of_the_compilers_own_types_is_a_dependency() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("api")).unwrap();
    fs::write(
        root.join("api").join("user.proto"),
        "syntax = \"proto3\";\n\nimport \"google/protobuf/timestamp.proto\";\nimport \"api/common.proto\";\n\nmessage User {\n  Address address = 1;\n}\n\nmessage Address { string city = 1; }\n",
    )
    .unwrap();
    fs::write(
        root.join("api").join("common.proto"),
        "syntax = \"proto3\";\n\nmessage Empty {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    // The file and the import that names it share a label; the import is
    // the one that says what it reached for.
    let import_scope = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::ExternalDependency && node.label == label)
            .and_then(|node| node.metadata.get("import_scope"))
            .cloned()
    };
    assert_eq!(
        import_scope("google/protobuf/timestamp.proto"),
        None,
        "the compiler's own types are a dependency, not a file to look for"
    );
    assert_eq!(
        import_scope("api/common.proto").as_deref(),
        Some("local"),
        "a path this repository holds is a file of it"
    );

    let references = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::References
                && edge.metadata.get("relation").map(String::as_str) == Some("type_reference")
        })
        .filter_map(|edge| {
            let source = graph.nodes.iter().find(|node| node.id == edge.source)?;
            let target = graph.nodes.iter().find(|node| node.id == edge.target)?;
            Some((source.label.clone(), target.label.clone()))
        })
        .collect::<Vec<_>>();
    assert!(
        references.contains(&("User".to_string(), "Address".to_string())),
        "a message carries the messages its fields state: {references:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_configuration_declares_the_providers_it_requires() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("main.tf"),
        "terraform {\n  required_providers {\n    happycloud = {\n      source  = \"example.com/awesomecorp/happycloud\"\n      version = \"1.0.0\"\n    }\n\n    aws = {\n      source = \"hashicorp/aws\"\n    }\n\n    null = \"~> 2.0\"\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let version_of = |id: NodeId| {
        graph
            .edges
            .iter()
            .find(|edge| edge.target == id && edge.kind == EdgeKind::DependsOn)
            .and_then(|edge| edge.metadata.get("dependency_version").cloned())
    };
    let declared = graph
        .nodes
        .iter()
        .filter(|node| node.metadata.get("ecosystem").map(String::as_str) == Some("terraform"))
        .map(|node| (node.label.clone(), version_of(node.id)))
        .collect::<Vec<_>>();

    // A provider is named the way its source names it, because that is what
    // another configuration would write.
    assert!(
        declared.contains(&(
            "example.com/awesomecorp/happycloud".to_string(),
            Some("1.0.0".to_string())
        )),
        "{declared:?}"
    );
    assert!(
        declared.iter().any(|(label, _)| label == "hashicorp/aws"),
        "{declared:?}"
    );
    // The older form states the version instead of a block, and the key is
    // the name.
    assert!(
        declared.iter().any(|(label, _)| label == "null"),
        "{declared:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_terraform_module_reaches_the_configuration_it_names() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("modules").join("vpc")).unwrap();
    fs::write(
        root.join("main.tf"),
        "module \"vpc\" {\n  source = \"./modules/vpc\"\n}\n\nresource \"aws_instance\" \"web\" {\n  subnet_id = module.vpc.id\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("modules").join("vpc").join("main.tf"),
        "resource \"aws_vpc\" \"this\" {\n  cidr_block = \"10.0.0.0/16\"\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Type && node.label == "aws_instance.web"),
        "a resource is a thing the configuration declares"
    );
    let module = graph
        .nodes
        .iter()
        .find(|node| node.label == "./modules/vpc")
        .expect("the module's source");
    let target = graph
        .edges
        .iter()
        .filter(|edge| edge.source == module.id)
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .map(|node| node.label.as_str())
        .collect::<Vec<_>>();
    assert!(
        target.contains(&"modules/vpc/main.tf"),
        "a local module source is a directory of this repository, not an outside dependency: {target:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_dotnet_platform_answers_its_own_calls() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // Polly declares `Outcome.FromResult` and calls `Task.FromResult` 112
    // times; the platform's static is not the project's method.
    fs::write(
        root.join("src").join("outcome.cs"),
        "namespace Demo {\n    public class Outcome {\n        public static Outcome FromResult(int value) { return null; }\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("runner.cs"),
        "namespace Demo {\n    public class Runner {\n        public object Run() { return Task.FromResult(1); }\n        public object Own() { return Outcome.FromResult(1); }\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let declared = node_id(&graph, NodeKind::Function, "FromResult");
    let reaches = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.target == declared
                && edge.metadata.get("call_label").map(String::as_str) == Some(label)
        })
    };
    assert!(reaches("Outcome.FromResult"));
    assert!(!reaches("Task.FromResult"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rubys_kernel_calls_belong_to_the_language() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib").join("app.rb"),
        "def render(value)\n  raise ArgumentError if value.nil?\n  present(value)\nend\n\ndef present(value)\n  value\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution"))
            .map(String::as_str)
    };
    assert_eq!(resolution("raise"), Some("builtin"));
    assert_eq!(resolution("nil?"), Some("builtin"));
    // The project's own method still answers its own call.
    assert_eq!(resolution("present"), Some("resolved"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_php_call_to_the_root_namespace_is_still_the_language() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("util.php"),
        "<?php\nfunction total(array $rows) { return \\count($rows); }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("\\count")
        })
        .and_then(|edge| edge.metadata.get("resolution"))
        .map(String::as_str);
    assert_eq!(resolution, Some("builtin"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_protected_method_answers_a_subclass_in_another_file() {
    // monolog declares `protected function getRecord` in its test base
    // class; 332 calls from the test files that extend it read as calls to
    // nothing while `protected` was recorded as `private`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("base.php"),
        "<?php\nclass TestCase {\n    protected function getRecord() { return 1; }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("logger_test.php"),
        "<?php\nclass LoggerTest extends TestCase {\n    public function testWrites() { return $this->getRecord(); }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let declared = node_id(&graph, NodeKind::Function, "getRecord");
    assert_eq!(
        graph
            .nodes
            .iter()
            .find(|node| node.id == declared)
            .and_then(|node| node.metadata.get("visibility"))
            .map(String::as_str),
        Some("protected")
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == declared),
        "a subclass in another file is exactly who `protected` is for"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_csharp_call_on_an_instance_is_not_the_projects_static_method() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // Newtonsoft declares `JsonConvert.ToString`, called 720 times, and its
    // `value.ToString()` calls are the one every object inherits.
    fs::write(
        root.join("src").join("convert.cs"),
        "namespace Demo {\n    public class JsonConvert {\n        public static string ToString(int value) { return \"\"; }\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("writer.cs"),
        "namespace Demo {\n    public class Writer {\n        public string Render(object value) { return JsonConvert.ToString(1) + value.ToString(); }\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let declared = node_id(&graph, NodeKind::Function, "ToString");
    let reaches = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.target == declared
                && edge.metadata.get("call_label").map(String::as_str) == Some(label)
        })
    };
    assert!(
        reaches("JsonConvert.ToString"),
        "a static call on the project's own type is the project's method"
    );
    assert!(
        !reaches("value.ToString"),
        "an instance's `ToString` belongs to a type the syntax cannot name"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_minified_file_is_recorded_but_not_read_for_facts() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    let packed = format!(
        "!function(e){{var t={{}};function n(r){{if(t[r])return t[r].exports;{}}}}}(window);",
        "var a=e[r],b=a.length,c=b?a[0]:null;if(c){return c.call(this,a,b)}".repeat(40)
    );
    fs::write(root.join("bundle.min.js"), &packed).unwrap();
    fs::write(
        root.join("app.js"),
        format!(
            "// {}\nfunction start() {{ return 1; }}\n",
            "a long note about why this module exists ".repeat(80)
        ),
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let minified = graph
        .nodes
        .iter()
        .find(|node| node.label == "bundle.min.js")
        .expect("the file is still part of the project");
    assert_eq!(
        minified.metadata.get("skipped_reason").map(String::as_str),
        Some("minified")
    );
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Function && node.label == "n"),
        "a minifier's names are not the project's"
    );
    // A long comment is not minification: this file is read as usual.
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Function && node.label == "start"),
        "a file with one long comment line is still source"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_private_rust_function_answers_only_its_own_module() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() { helper(); }\n",
    )
    .unwrap();
    fs::write(root.join("src").join("other.rs"), "fn helper() {}\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let helper = node_id(&graph, NodeKind::Function, "helper");
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == helper),
        "a private `fn helper` is not visible from a sibling file"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_rust_method_every_type_has_is_not_the_projects_own() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("query.rs"),
        "pub struct Query;\n\nimpl Query {\n    pub fn parse(text: &str) -> Query { Query }\n}\n\npub fn limit(text: &str) -> usize {\n    text.parse::<usize>().unwrap_or(0)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let parse = node_id(&graph, NodeKind::Function, "parse");
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == parse),
        "`text.parse::<usize>()` is `str::parse`, not `Query::parse`"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_marks_ambiguous_call_edges() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() { parse(); }\n").unwrap();
    // Both are `pub`: a private `fn parse` in a sibling file is not something
    // `main.rs` could be calling, so only public ones make the call ambiguous.
    fs::write(root.join("src").join("left.rs"), "pub fn parse() {}\n").unwrap();
    fs::write(root.join("src").join("right.rs"), "pub fn parse() {}\n").unwrap();

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
fn a_type_nested_in_another_is_not_what_a_bare_name_elsewhere_means() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/main/scala")).unwrap();
    // `Right` here is `Ior.Right`, which is not scala's `Either.Right`.
    fs::write(
        root.join("src/main/scala").join("Ior.scala"),
        "object Ior {\n  final case class Right[+B](b: B)\n}\n\nobject IorOps {\n  def wrap[B](b: B) = Right(b)\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/scala").join("EitherOps.scala"),
        "object EitherOps {\n  def wrap[B](b: B) = Right(b)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let nested = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Type && node.label == "Right")
        .expect("the nested case class is a type");
    assert_eq!(
        nested.metadata.get("owner_type").map(String::as_str),
        Some("Ior"),
        "a type written inside another one records its owner"
    );

    let sources = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == nested.id
                && edge.kind == EdgeKind::References
                && edge.metadata.get("relation").map(String::as_str)
                    == Some("constructor_reference")
        })
        .filter_map(|edge| edge.metadata.get("file").cloned())
        .collect::<Vec<_>>();
    assert!(
        sources.iter().any(|file| file.ends_with("Ior.scala")),
        "the file that declares it still reaches it: {sources:?}"
    );
    assert!(
        !sources.iter().any(|file| file.ends_with("EitherOps.scala")),
        "a bare `Right` in another file is not `Ior.Right`: {sources:?}"
    );

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

/// What every scanned graph has to hold, whatever the languages in it.
///
/// Driving these over the 23-project corpus found no violation - no dangling
/// edge, no repeated or missing stable id, no span pointing at a file the
/// graph does not have, and no two identical edges - and the self-loops are
/// all recursive calls. Asserting them here keeps it that way: an indexer
/// pass that forgets `add_edge_once` or invents a span shows up as a failing
/// test rather than as a number nobody recomputes.
fn assert_graph_invariants(graph: &codegraph_core::CodeGraph) {
    let ids: BTreeSet<NodeId> = graph.nodes.iter().map(|node| node.id).collect();
    let files: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .map(|node| node.label.as_str())
        .collect();
    for edge in &graph.edges {
        assert!(
            ids.contains(&edge.source) && ids.contains(&edge.target),
            "edge {:?} names a node the graph does not hold",
            edge.kind
        );
        // Only a call can start and end at the same node: a function that
        // calls itself. Anything else is a pass linking a node to itself.
        assert!(
            edge.source != edge.target || edge.kind == EdgeKind::Calls,
            "{:?} is a self edge",
            edge.kind
        );
    }
    let mut stable_ids = BTreeSet::new();
    for node in &graph.nodes {
        let stable_id = node
            .metadata
            .get("stable_id")
            .unwrap_or_else(|| panic!("`{}` has no stable id", node.label));
        assert!(
            stable_ids.insert(stable_id.as_str()),
            "two nodes share the stable id {stable_id}"
        );
        if let Some(span) = node.span.as_ref()
            && span.path != "."
        {
            assert!(
                files.contains(span.path.as_str()),
                "`{}` is placed in `{}`, which is not a file in the graph",
                node.label,
                span.path
            );
        }
    }
    let mut seen = BTreeSet::new();
    for edge in &graph.edges {
        let key = (
            edge.source,
            edge.target,
            edge.kind,
            format!("{:?}", edge.metadata),
        );
        assert!(
            seen.insert(key),
            "two identical {:?} edges between the same nodes",
            edge.kind
        );
    }
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
    assert_graph_invariants(&graph);
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
fn a_closure_assigned_to_an_outer_name_is_callable_everywhere() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // Vue's shape: a module-level binding filled in by one function and
    // called by another.
    fs::write(
        root.join("src").join("component.ts"),
        r#"let installWithProxy: (i: number) => void

export function registerRuntimeCompiler(compile: any) {
  installWithProxy = i => {
    compile(i)
  }
}

export function finishComponentSetup(instance: number) {
  if (installWithProxy) {
    installWithProxy(instance)
  }
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let installed = node_id(&graph, NodeKind::Function, "installWithProxy");
    let setup = node_id(&graph, NodeKind::Function, "finishComponentSetup");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls && edge.source == setup && edge.target == installed
        }),
        "the call from finishComponentSetup was left unresolved"
    );
}

#[test]
fn a_lua_call_names_the_file_its_module_lives_in() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("kong").join("pdk")).unwrap();
    fs::write(
        root.join("kong").join("globalpatches.lua"),
        "local function patch(options)\n  if options.cli then\n    ngx.exit = function() end\n  end\nend\n\nreturn patch\n",
    )
    .unwrap();
    fs::write(
        root.join("kong").join("pdk").join("response.lua"),
        "local _M = {}\n\nfunction _M.exit(status)\n  return status\nend\n\nreturn _M\n",
    )
    .unwrap();
    fs::write(
        root.join("kong").join("handler.lua"),
        "local kong = kong\n\nlocal function access()\n  return kong.response.exit(200)\nend\n\nreturn access\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let patched = node_id(&graph, NodeKind::Function, "ngx.exit");
    let access = node_id(&graph, NodeKind::Function, "access");
    // kong patches `ngx.exit`, but `kong.response.exit(...)` is a different
    // function; only a call written `ngx.exit(...)` means the patch.
    assert!(
        !graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls && edge.source == access && edge.target == patched
        }),
        "the call was answered by the patched global"
    );
    // The module the call names is the file that holds it.
    let exit = node_id(&graph, NodeKind::Function, "_M.exit");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls && edge.source == access && edge.target == exit
        }),
        "the call did not reach the module the file holds"
    );
}

#[test]
fn a_builtin_namespace_call_is_not_a_project_function() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("lib").join("axios.js"),
        r#"function createInstance(defaultConfig) {
  const instance = {};
  instance.create = function create(instanceConfig) {
    return createInstance(instanceConfig);
  };
  return instance;
}

module.exports = createInstance;
"#,
    )
    .unwrap();
    fs::write(
        root.join("lib").join("mergeConfig.js"),
        r#"function mergeConfig(config1, config2) {
  const config = Object.create(null);
  config.merged = true;
  return config;
}

module.exports = mergeConfig;
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let create = node_id(&graph, NodeKind::Function, "instance.create");
    let merge = node_id(&graph, NodeKind::Function, "mergeConfig");
    assert!(
        !graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls && edge.source == merge && edge.target == create
        }),
        "`Object.create` was answered by the project's `instance.create`"
    );
}

#[test]
fn a_call_belongs_to_the_method_that_makes_it() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    // Go writes String() once per type, so one file holds several of them.
    fs::write(
        root.join("main.go"),
        r#"package main

type A struct{}

func (a A) String() string {
	return helperA()
}

type B struct{}

func (b B) String() string {
	return helperB()
}

func helperA() string { return "a" }

func helperB() string { return "b" }
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let caller_span = |callee: &str| {
        let target = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == callee)
            .unwrap_or_else(|| panic!("missing {callee}"));
        let edge = graph
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::Calls && edge.target == target.id)
            .unwrap_or_else(|| panic!("nothing calls {callee}"));
        graph
            .nodes
            .iter()
            .find(|node| node.id == edge.source)
            .and_then(|node| node.span.clone())
            .expect("caller has no span")
    };
    // The call to helperB is on line 12, inside the second String().
    let second = caller_span("helperB");
    assert!(
        second.start_line <= 12 && 12 <= second.end_line,
        "the call went to the String() at {}-{}",
        second.start_line,
        second.end_line
    );
    // ...and the call to helperA is on line 6, inside the first.
    let first = caller_span("helperA");
    assert!(
        first.start_line <= 6 && 6 <= first.end_line,
        "the call went to the String() at {}-{}",
        first.start_line,
        first.end_line
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_sinatra_route_is_an_entrypoint() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.rb"),
        "require 'sinatra'\n\nget '/hello' do\n  'hi'\nend\n\npost '/messages' do\n  201\nend\n\nget params['name']\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("entrypoint_kind")
                .is_some_and(|kind| kind == "route")
        })
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(
        routes,
        vec!["route GET /hello", "route POST /messages"],
        "a call whose argument is not a path is not a route"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_php_use_reaches_the_class_file() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("Exception")).unwrap();
    fs::write(
        root.join("src").join("Client.php"),
        "<?php\nnamespace GuzzleHttp;\n\nuse GuzzleHttp\\Exception\\BadResponseException;\nuse Psr\\Http\\Message\\UriInterface;\n\nclass Client {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Exception").join("BadResponseException.php"),
        "<?php\nnamespace GuzzleHttp\\Exception;\n\nclass BadResponseException extends \\RuntimeException {}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let file = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::File && node.label == "src/Exception/BadResponseException.php"
        })
        .expect("missing class file");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == file.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        }),
        "PSR-4 maps the namespace onto the directory that holds the class"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_jvm_import_and_a_zig_import_reach_their_file() {
    let root = temp_project_root();
    let java_dir = root.join("gson/src/main/java/com/google/gson");
    fs::create_dir_all(&java_dir).unwrap();
    fs::write(
        java_dir.join("Gson.java"),
        "package com.google.gson;\n\nimport com.google.gson.FormattingStyle;\nimport java.util.Objects;\n\npublic final class Gson {}\n",
    )
    .unwrap();
    fs::write(
        java_dir.join("FormattingStyle.java"),
        "package com.google.gson;\n\npublic final class FormattingStyle {}\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.zig"),
        "const std = @import(\"std\");\nconst ast = @import(\"ast.zig\");\n\npub fn main() void {\n    _ = ast;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("ast.zig"),
        "pub const Node = struct {};\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |path: &str| {
        let file = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == path)
            .unwrap_or_else(|| panic!("missing {path}"));
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == file.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        })
    };
    assert!(
        reaches("gson/src/main/java/com/google/gson/FormattingStyle.java"),
        "the package path names the file, whatever the source root"
    );
    assert!(reaches("src/ast.zig"), "@import(\"ast.zig\")");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_julia_include_and_a_ruby_require_relative_reach_their_file() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("other")).unwrap();
    fs::write(
        root.join("src").join("DataFrames.jl"),
        "module DataFrames\ninclude(\"other/utils.jl\")\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("other").join("utils.jl"),
        "function helper(x)\n    x\nend\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("lib").join("sinatra")).unwrap();
    fs::write(
        root.join("lib").join("sinatra.rb"),
        "require_relative 'sinatra/logger'\nrequire 'json'\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("sinatra").join("logger.rb"),
        "module Sinatra\n  class Logger\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |path: &str| {
        let file = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == path)
            .unwrap_or_else(|| panic!("missing {path}"));
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == file.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        })
    };
    assert!(reaches("src/other/utils.jl"), "include(\"other/utils.jl\")");
    assert!(reaches("lib/sinatra/logger.rb"), "require_relative");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_go_call_through_a_package_qualifier_reads_past_the_type() {
    // `protoimpl.X.MessageStateOf(m)` names a package, a variable inside it
    // and a method. The owner is the last segment before the method -- `X` --
    // and looking that up in the import list found nothing, so 3234 of
    // terraform's generated protobuf calls were reported unresolved instead
    // of as calls into the package the file states it imports.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("state")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("state").join("state.go"),
        "package state\n\ntype Store struct{}\n\nfunc (s Store) MessageStateOf(m int) int {\n\treturn m\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("main.go"),
        "package main\n\nimport (\n\t\"google.golang.org/protobuf/runtime/protoimpl\"\n)\n\nfunc main() {\n\tprotoimpl.X.MessageStateOf(1)\n\t_ = string(1)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let own = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "MessageStateOf"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with("state.go"))
        })
        .expect("the project's own method");
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == own.id),
        "a call into an imported package is not answered by a method sharing its name"
    );
    let external = graph.nodes.iter().any(|node| {
        node.kind == NodeKind::ExternalDependency
            && node.label == "protoimpl.X.MessageStateOf"
            && node.metadata.get("resolution").map(String::as_str) == Some("external")
    });
    assert!(external, "the call is recorded as leaving the project");
    // `string(1)` converts a value; the language declares the name.
    let conversion = graph.nodes.iter().any(|node| {
        node.kind == NodeKind::ExternalDependency
            && node.label == "string"
            && node.metadata.get("resolution").map(String::as_str) == Some("builtin")
    });
    assert!(
        conversion,
        "a predeclared type used as a conversion is the language's, not a missing function"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_program_without_an_extension_states_its_language_in_its_first_line() {
    // A file with no extension is read, and its shebang is the only thing
    // that says what it is. Four interpreters were missing: mastodon keeps
    // thirteen ruby programs in `bin/`, and kong's `bin/kong` -- the
    // gateway's whole CLI -- runs under OpenResty's `resty`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(
        root.join("bin").join("kong"),
        "#!/usr/bin/env resty\n\nlocal cli = require \"kong.cmd\"\n\nlocal function run()\n  return cli.run()\nend\n\nrun()\n",
    )
    .unwrap();
    fs::write(
        root.join("bin").join("rake"),
        "#!/usr/bin/env ruby\n\ndef run\n  puts 'rake'\nend\n\nrun\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let language_of = |path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node.span.as_ref().is_some_and(|span| span.path == path)
            })
            .and_then(|node| node.metadata.get("language").cloned())
    };
    assert_eq!(language_of("bin/kong").as_deref(), Some("lua"));
    assert_eq!(language_of("bin/rake").as_deref(), Some("ruby"));
    assert!(
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Entrypoint
                && node.label == "script:bin/kong"
                && node.metadata.get("interpreter").map(String::as_str) == Some("resty")
        }),
        "and the entrypoint names what runs it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_cabal_file_states_the_programs_it_builds() {
    // Haskell states its programs in the package's `.cabal` file:
    // `executable shellcheck` with `main-is: shellcheck.hs`. Without
    // reading it, shellcheck's entrypoints were its shell scripts and CI
    // jobs, and the coverage finding said so.
    let root = temp_project_root();
    fs::create_dir_all(root.join("test")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("shellcheck.hs"),
        "main :: IO ()\nmain = return ()\n",
    )
    .unwrap();
    fs::write(
        root.join("test").join("run.hs"),
        "main :: IO ()\nmain = return ()\n",
    )
    .unwrap();
    fs::write(root.join("src").join("Lib.hs"), "module Lib where\n").unwrap();
    fs::write(
        root.join("ShellCheck.cabal"),
        "name: ShellCheck\nversion: 0.1\n\nlibrary\n    hs-source-dirs: src\n    exposed-modules: Lib\n\nexecutable shellcheck\n    build-depends:\n      base\n    main-is: shellcheck.hs\n\ntest-suite spec\n    type: exitcode-stdio-1.0\n    hs-source-dirs: test\n    main-is: run.hs\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let entrypoint = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Entrypoint && node.label == label)
    };
    let program = entrypoint("cabal executable:shellcheck").expect("the program");
    assert_eq!(
        program.metadata.get("target").map(String::as_str),
        Some("shellcheck.hs")
    );
    assert!(
        graph.edges.iter().any(|edge| {
            edge.source == program.id
                && edge.kind == EdgeKind::References
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.kind == NodeKind::File
                        && node.label == "shellcheck.hs"
                })
        }),
        "and it reaches the module it names"
    );
    // A test suite states a program too, under the source directory the
    // stanza gives it.
    let suite = entrypoint("cabal test:spec").expect("the test program");
    assert_eq!(
        suite.metadata.get("target").map(String::as_str),
        Some("test/run.hs")
    );
    assert!(
        entrypoint("cabal executable:library").is_none(),
        "a library declares no program"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_dune_file_states_the_programs_it_builds() {
    // OCaml states what it builds in `dune` files, one per directory:
    // `(executable (name main))` in `bin/dune` is `bin/main.ml`. Without
    // reading them the dune repository showed eighteen entrypoints for a
    // build system that declares three hundred.
    let root = temp_project_root();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("bench")).unwrap();
    fs::write(
        root.join("bin").join("dune"),
        "(include_subdirs qualified)\n\n(executable\n (name main)\n (public_name dune)\n (libraries stdune))\n",
    )
    .unwrap();
    fs::write(
        root.join("bin").join("main.ml"),
        "let () = print_endline \"hi\"\n",
    )
    .unwrap();
    fs::write(
        root.join("bench").join("dune"),
        "; a comment (executable (name ignored))\n(executables\n (names bench gen_synthetic)\n (libraries unix))\n",
    )
    .unwrap();
    fs::write(root.join("bench").join("bench.ml"), "let () = ()\n").unwrap();
    fs::write(root.join("bench").join("gen_synthetic.ml"), "let () = ()\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let entrypoints: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Entrypoint
                && node.metadata.get("ecosystem").map(String::as_str) == Some("dune")
        })
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(
        entrypoints,
        BTreeSet::from([
            "dune executable:main",
            "dune executable:bench",
            "dune executable:gen_synthetic",
        ]),
        "a comment states nothing, and `names` states each program"
    );
    let main = graph
        .nodes
        .iter()
        .find(|node| node.label == "dune executable:main")
        .expect("the program");
    let reaches_file = graph.edges.iter().any(|edge| {
        edge.source == main.id
            && edge.kind == EdgeKind::References
            && graph.nodes.iter().any(|node| {
                node.id == edge.target && node.kind == NodeKind::File && node.label == "bin/main.ml"
            })
    });
    assert!(reaches_file, "and it reaches the file beside the dune file");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_c_file_is_c_whatever_its_variables_are_called() {
    // `.h` is C's extension, C++'s and Objective-C's alike, so a header is
    // sniffed for what it declares. `.c` says C outright, and sniffing it
    // too read `class = getClientType(c)` -- an assignment to a variable
    // named `class` -- as a C++ class declaration: redis parsed
    // networking.c as C++, which put its `addReplyError` in a different
    // language than the 132 C calls to it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("server.h"),
        "void addReplyError(void *c, const char *err);\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("networking.c"),
        "#include \"server.h\"\n\nint getClientType(void *c) {\n    return 0;\n}\n\nvoid addReplyError(void *c, const char *err) {\n    int class = getClientType(c);\n    (void)class;\n    (void)err;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("db.c"),
        "#include \"server.h\"\n\nvoid lookupKey(void *c) {\n    addReplyError(c, \"boom\");\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let handler = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && node.label == "addReplyError"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with("networking.c"))
        })
        .expect("the definition");
    assert_eq!(
        handler.metadata.get("language").map(String::as_str),
        Some("c"),
        "a .c file is C"
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == handler.id),
        "so a call from another .c file reaches it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_java_static_import_says_whose_method_a_bare_call_means() {
    // `import static com.google.common.truth.Truth.assertThat` makes a bare
    // `assertThat` Truth's, and retrofit declares an `assertThat` of its
    // own in a test helper: 663 calls read as the helper's. The static
    // import is the only thing that tells them apart -- and when it names
    // the project's own class, the file it points at is the class, not a
    // file named after the member.
    let root = temp_project_root();
    let src = root.join("src").join("main").join("java").join("app");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("TestingUtils.java"),
        "package app;\n\npublic final class TestingUtils {\n  public static String buildRequest(String path) {\n    return path;\n  }\n\n  public static String assertThat(String value) {\n    return value;\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        src.join("RequestTest.java"),
        "package app;\n\nimport static app.TestingUtils.buildRequest;\nimport static com.google.common.truth.Truth.assertThat;\n\npublic final class RequestTest {\n  public void run() {\n    assertThat(buildRequest(\"/x\"));\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some(label)
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.label == label
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path.ends_with("TestingUtils.java"))
                })
        })
    };
    assert!(
        reaches("buildRequest"),
        "a static import of the project's own class reaches the class's file"
    );
    assert!(
        !reaches("assertThat"),
        "and a static import from a package rules the project's own method out"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_module_calls_what_it_imports_or_declares() {
    // `const h = originalH` binds a name the file never imports, and
    // matching by name alone sent the call into the module that declares
    // the original. A module shares nothing ambiently: what a file calls by
    // a bare name it either declares or imports -- including through a
    // `require`, which binds a name just as an import statement does.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("lib").join("utils.js"),
        "function compileETag (value) {\n  return value\n}\n\nfunction render (value) {\n  return value\n}\n\nmodule.exports = { compileETag, render }\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("app.js"),
        "var compileETag = require('./utils').compileETag\n\nfunction start (options) {\n  var render = options.render\n\n  compileETag('x')\n  render('y')\n}\n\nmodule.exports = start\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.label == label
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path.ends_with("utils.js"))
                })
        })
    };
    assert!(
        reaches("compileETag"),
        "a require binds the name the file then calls"
    );
    assert!(
        !reaches("render"),
        "a name the body binds is not the module's export of the same name"
    );
    // And the edge says why it found nothing: the module cannot reach a
    // name it never imports, which is not a resolver that failed.
    assert_eq!(
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some("render")
            })
            .and_then(|edge| edge.metadata.get("unresolved_reason"))
            .map(String::as_str),
        Some("not_imported")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn typescript_types_are_reached_by_the_annotations_that_name_them() {
    // vue's `ComponentInternalInstance` is the interface its whole runtime
    // is written against, and nothing pointed at it: a type is named by
    // annotations, generic arguments and heritage clauses, none of which
    // were read. `impact` on it answered with nothing.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("types.ts"),
        "export interface Instance {\n  uid: number\n}\n\nexport interface Renderer {\n  render(): void\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("runtime.ts"),
        "import type { Instance, Renderer } from './types'\n\nexport class Runtime implements Renderer {\n  render(): void {}\n}\n\nexport function mount(instance: Instance): Array<Instance> {\n  return [instance]\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let references = |label: &str| {
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.kind == EdgeKind::References
                    && edge
                        .metadata
                        .get("relation")
                        .is_some_and(|relation| relation == "type_reference")
                    && graph
                        .nodes
                        .iter()
                        .any(|node| node.id == edge.target && node.label == label)
            })
            .count()
    };
    assert!(
        references("Instance") >= 1,
        "a parameter's annotation names the type it is given"
    );
    assert!(
        references("Renderer") >= 1,
        "and a class states the interface it implements"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn java_and_rust_types_are_reached_by_the_declarations_that_name_them() {
    // The same holds wherever a type is written down: a Java field's type
    // and a Rust `impl` block name the type as plainly as a call does. Two
    // thirds of gson's classes and six sevenths of ripgrep's types had
    // nothing pointing at them.
    let root = temp_project_root();
    let java = root.join("src").join("main").join("java").join("app");
    fs::create_dir_all(&java).unwrap();
    fs::create_dir_all(root.join("rust").join("src")).unwrap();
    fs::write(
        java.join("Reader.java"),
        "package app;\n\npublic final class Reader {\n}\n",
    )
    .unwrap();
    fs::write(
        java.join("Parser.java"),
        "package app;\n\npublic final class Parser {\n  private final Reader reader;\n\n  Parser(Reader reader) {\n    this.reader = reader;\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("rust").join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("rust").join("src").join("lib.rs"),
        "pub struct Matcher {\n    pub pattern: String,\n}\n\nimpl Matcher {\n    pub fn new(pattern: String) -> Matcher {\n        Matcher { pattern }\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let references = |label: &str| {
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.kind == EdgeKind::References
                    && edge
                        .metadata
                        .get("relation")
                        .is_some_and(|relation| relation == "type_reference")
                    && graph
                        .nodes
                        .iter()
                        .any(|node| node.id == edge.target && node.label == label)
            })
            .count()
    };
    assert!(
        references("Reader") >= 1,
        "a java field and constructor parameter name the class"
    );
    assert!(
        references("Matcher") >= 1,
        "and a rust `impl` block names the type it is written for"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn go_and_csharp_types_are_reached_by_the_declarations_that_name_them() {
    // gin's `Context` is the type its whole framework is written against
    // and had 27 references for 208 types; C# writes its types as plain
    // identifiers, so what a declaration states is in its `type` field and
    // the classes it derives from are its base list.
    let root = temp_project_root();
    fs::create_dir_all(root.join("go")).unwrap();
    fs::create_dir_all(root.join("cs")).unwrap();
    fs::write(
        root.join("go").join("go.mod"),
        "module example.com/app\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        root.join("go").join("context.go"),
        "package app\n\ntype Context struct {\n\tPath string\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("go").join("router.go"),
        "package app\n\nfunc Handle(c *Context) string {\n\treturn c.Path\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("cs").join("Context.cs"),
        "namespace App;\n\npublic class ResilienceContext\n{\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("cs").join("Pipeline.cs"),
        "namespace App;\n\npublic class Pipeline\n{\n    private ResilienceContext _context;\n\n    public void Run(ResilienceContext context) => _context = context;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let references = |label: &str| {
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.kind == EdgeKind::References
                    && edge
                        .metadata
                        .get("relation")
                        .is_some_and(|relation| relation == "type_reference")
                    && graph
                        .nodes
                        .iter()
                        .any(|node| node.id == edge.target && node.label == label)
            })
            .count()
    };
    assert!(
        references("Context") >= 1,
        "a go parameter names the struct it takes"
    );
    assert!(
        references("ResilienceContext") >= 1,
        "and a C# field and parameter name the class they hold"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_type_parameter_is_not_a_type_the_project_declares() {
    // Every generic declaration writes `T`, `A`, `K`, `V`, and no project
    // means its own type by them: reading them as references pointed 10756
    // of cats' 13896 at whatever happened to be called `A`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("types.ts"),
        "export interface A {\n  id: number\n}\n\nexport interface Item {\n  id: number\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("box.ts"),
        "import type { Item } from './types'\n\nexport function unwrap<A>(values: A[], item: Item): A {\n  return values[0]\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let references = |label: &str| {
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.kind == EdgeKind::References
                    && edge
                        .metadata
                        .get("relation")
                        .is_some_and(|relation| relation == "type_reference")
                    && graph
                        .nodes
                        .iter()
                        .any(|node| node.id == edge.target && node.label == label)
            })
            .count()
    };
    assert_eq!(
        references("A"),
        0,
        "the interface named `A` is not what `<A>` means"
    );
    assert!(references("Item") >= 1, "a real type is still reached");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn building_a_class_runs_its_constructor() {
    // `new SongService($repository)` reached the class and stopped there,
    // so koel's 378 `__construct` methods had no caller between them --
    // and a constructor is where a framework hands a class what it needs.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("composer.json"),
        "{\n  \"name\": \"acme/app\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("SongService.php"),
        "<?php\n\nnamespace App;\n\nclass SongService\n{\n    public function __construct(private string $name)\n    {\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Controller.php"),
        "<?php\n\nnamespace App;\n\nclass Controller\n{\n    public function index(): SongService\n    {\n        return new SongService('x');\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let constructor = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && node.label == "__construct"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with("SongService.php"))
        })
        .expect("the constructor");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.target == constructor.id
                && edge.metadata.get("resolution").map(String::as_str) == Some("constructor")
        }),
        "building the class calls what it declares as its constructor"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn php_classes_are_reached_by_the_types_that_name_them() {
    // Laravel builds a service from its constructor's type hints rather
    // than with `new`, and a serializer states the interface it implements.
    // Neither was read, so koel had two references pointing into its 1319
    // classes and "what breaks if I change SongService" answered with
    // nothing at all.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("composer.json"),
        "{\n  \"name\": \"acme/app\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("SongService.php"),
        "<?php\n\nnamespace App;\n\nclass SongService\n{\n    public function update(): bool\n    {\n        return true;\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Serializer.php"),
        "<?php\n\nnamespace App;\n\ninterface Serializer\n{\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("routes.php"),
        "<?php\n\nnamespace App;\n\nRoute::apiResource('songs', SongController::class);\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("SongController.php"),
        "<?php\n\nnamespace App;\n\nclass SongController implements Serializer\n{\n    public function __construct(private SongService $songs)\n    {\n    }\n\n    public function update(): bool\n    {\n        return $this->songs->update();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "type_reference")
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.target && node.label == label)
        })
    };
    assert!(
        reaches("SongService"),
        "a constructor's type hint names the class it is given"
    );
    assert!(
        reaches("Serializer"),
        "and a class states the interface it implements"
    );
    assert!(
        reaches("SongController"),
        "and `SongController::class` names it without building it -- which is \
         how a Laravel route, a container binding and a config file all name \
         a class, 111 times in koel's routes alone"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_php_static_call_names_the_class_it_goes_through() {
    // `File::hash($path)` is Laravel's facade, and koel declares a `hash`
    // of its own in an authenticator: the label kept only the method, so
    // eight call sites reached a method they never name. The class the call
    // is written through is the evidence, and one the project declares
    // settles which method is meant instead.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("Models")).unwrap();
    fs::create_dir_all(root.join("app").join("Auth")).unwrap();
    fs::write(
        root.join("composer.json"),
        "{\n  \"name\": \"acme/app\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Auth").join("Authenticator.php"),
        "<?php\n\nnamespace App\\Auth;\n\nclass Authenticator\n{\n    public function hash(string $key): string\n    {\n        return $key;\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Models").join("Song.php"),
        "<?php\n\nnamespace App\\Models;\n\nclass Song\n{\n    public static function query(): string\n    {\n        return 'songs';\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Reader.php"),
        "<?php\n\nnamespace App;\n\nuse App\\Models\\Song;\nuse Illuminate\\Support\\Facades\\File;\n\nclass Reader\n{\n    public function read(string $path): string\n    {\n        Song::query();\n\n        return File::hash($path);\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |label: &str, path: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.label == label
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path.ends_with(path))
                })
        })
    };
    assert!(
        reaches("query", "Song.php"),
        "the class the project declares settles which method is meant"
    );
    assert!(
        !reaches("hash", "Authenticator.php"),
        "and a facade the project does not declare is not one of its methods"
    );
    assert!(
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::ExternalDependency
                && node.label == "File::hash"
                && node.metadata.get("resolution").map(String::as_str) == Some("external")
        }),
        "the facade call is recorded as leaving the project"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_makefile_is_read_as_make_and_not_as_shell() {
    // A Makefile's recipes are shell; the file around them is not. Reading
    // the whole file as shell made terraform's Makefile call `protobuf:`,
    // `.PHONY:` and `CURDIR`, and reported a syntax error on every Makefile
    // in the corpus. The targets and what they run are the makefile
    // detector's to state.
    let root = temp_project_root();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts").join("check.sh"),
        "#!/bin/sh\necho ok\n",
    )
    .unwrap();
    fs::write(
        root.join("Makefile"),
        ".DEFAULT_GOAL := help\n\nCURDIR_ARG := $(if $(CURDIR),--dir $(CURDIR),)\n\ncheck:\n\t\"$(CURDIR)/scripts/check.sh\"\n\n.PHONY: check\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let makefile_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.span
                .as_ref()
                .is_some_and(|span| span.path == "Makefile")
        })
        .collect();
    assert!(
        makefile_nodes.iter().any(|node| {
            node.kind == NodeKind::Entrypoint
                && node.metadata.get("item_kind").map(String::as_str) == Some("makefile_target")
        }),
        "the target is still read"
    );
    for node in &makefile_nodes {
        assert_ne!(
            node.kind,
            NodeKind::ExternalDependency,
            "make syntax is not a shell command: {:?}",
            node.label
        );
    }
    let file = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "Makefile")
        .expect("the Makefile is still scanned");
    assert!(
        !file.metadata.contains_key("syntax_errors"),
        "and no grammar is asked to read it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_dockerfile_command_runs_from_the_build_context() {
    // A Dockerfile's command runs inside the image, on the paths `COPY` put
    // there from the build context. Mastodon keeps `streaming/Dockerfile`
    // and runs `node ./streaming/index.js` from `WORKDIR /opt/mastodon`,
    // and reading that beside the Dockerfile looked for
    // `streaming/streaming/index.js`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("streaming")).unwrap();
    fs::write(
        root.join("streaming").join("index.js"),
        "console.log('streaming');\n",
    )
    .unwrap();
    fs::write(
        root.join("streaming").join("Dockerfile"),
        "FROM node:20\nWORKDIR /opt/app\nCOPY . /opt/app\nCMD [ \"node\", \"./streaming/index.js\" ]\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let entrypoint = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Entrypoint
                && node.metadata.get("item_kind").map(String::as_str)
                    == Some("dockerfile_entrypoint")
        })
        .expect("the docker command entrypoint");
    let index = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "streaming/index.js")
        .expect("the file it runs");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.source == entrypoint.id
                && edge.target == index.id
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|resolution| resolution == "docker_command_path")
        }),
        "the command reaches the file the build context holds"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_route_reaches_the_action_its_controllers_parent_declares() {
    // Eleven of mastodon's settings pages declare no action of their own:
    // `class BrandingController < Admin::SettingsController` inherits
    // `show` and `update`, and a route that reaches nothing is where a
    // flow stops. `with_options only: [:index] do` hands its options to
    // every resource inside it, and reading them without it claimed 42
    // routes mastodon does not serve.
    let root = temp_project_root();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("app/controllers/admin")).unwrap();
    fs::write(
        root.join("config/routes.rb"),
        "Rails.application.routes.draw do\n  namespace :admin do\n    resource :branding, only: [:show, :update]\n\n    with_options only: [:index] do\n      resources :links\n    end\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/settings_controller.rb"),
        "class Admin::SettingsController < ApplicationController\n  def show\n    render :show\n  end\n\n  def update\n    redirect_to admin_root_path\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/brandings_controller.rb"),
        "class Admin::BrandingsController < Admin::SettingsController\n  private\n\n  def after_update_redirect_path\n    admin_root_path\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/links_controller.rb"),
        "class Admin::LinksController < ApplicationController\n  def index\n    render :index\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let branding = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Type && node.label == "Admin::BrandingsController")
        .expect("the controller is indexed");
    assert_eq!(
        branding.metadata.get("extends").map(String::as_str),
        Some("Admin::SettingsController"),
        "a class states what it inherits from"
    );

    let route = graph
        .nodes
        .iter()
        .find(|node| node.label == "route GET /admin/branding")
        .expect("the page is served")
        .id;
    let handler = graph
        .edges
        .iter()
        .find(|edge| {
            edge.source == route
                && edge.metadata.get("relation").map(String::as_str) == Some("entrypoint_function")
        })
        .map(|edge| edge.target)
        .and_then(|id| graph.nodes.iter().find(|node| node.id == id))
        .expect("the route reaches the action serving it");
    assert_eq!(
        handler.metadata.get("owner_type").map(String::as_str),
        Some("Admin::SettingsController"),
        "which its parent declares"
    );

    let links: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.label.starts_with("route ") && node.label.contains("/admin/links"))
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(
        links,
        vec!["route GET /admin/links"],
        "`with_options only: [:index]` states what the resources inside it declare"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rails_says_which_actions_a_resource_declares_and_which_controller_serves_them() {
    // Four things a Rails router states that the graph read wrong on
    // mastodon: `only: []` declares none of the seven, a singular
    // `resource :setup` is served by `SetupsController`, `module:` puts
    // the controller one module deeper without moving the path, and `get
    // :export` inside a resource block is that resource's action. A route
    // written inside a `concern` block is a template served wherever
    // `concerns:` names it, not where it is written.
    let root = temp_project_root();
    fs::create_dir_all(root.join("config/routes")).unwrap();
    fs::create_dir_all(root.join("app/controllers/admin/email_subscriptions")).unwrap();
    fs::create_dir_all(root.join("app/controllers/admin/terms_of_service")).unwrap();
    fs::write(
        root.join("config/routes.rb"),
        "Rails.application.routes.draw do\n  namespace :admin do\n    concern :approvable do\n      collection do\n        post :approve\n      end\n    end\n\n    resources :users, only: [] do\n      member do\n        get :download\n      end\n    end\n\n    resources :export_domain_allows, only: [:new] do\n      collection do\n        get :export\n      end\n    end\n\n    namespace :email_subscriptions do\n      resource :setup, only: [:show, :create]\n    end\n\n    resources :terms_of_service, only: [:index] do\n      resource :preview, only: [:show], module: :terms_of_service\n    end\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/users_controller.rb"),
        "class Admin::UsersController < ApplicationController\n  def download\n    head :ok\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/export_domain_allows_controller.rb"),
        "class Admin::ExportDomainAllowsController < ApplicationController\n  def export\n    head :ok\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/email_subscriptions/setups_controller.rb"),
        "class Admin::EmailSubscriptions::SetupsController < ApplicationController\n  def show\n    head :ok\n  end\n\n  def create\n    head :ok\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/admin/terms_of_service/previews_controller.rb"),
        "class Admin::TermsOfService::PreviewsController < ApplicationController\n  def show\n    head :ok\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Entrypoint && node.label.starts_with("route "))
        .collect();
    let labels: Vec<&str> = routes.iter().map(|node| node.label.as_str()).collect();
    assert!(
        !labels.contains(&"route GET /admin/users"),
        "`only: []` declares none of the seven, got {labels:?}"
    );
    assert!(
        !labels.iter().any(|label| label.ends_with("/admin/approve")),
        "a route inside a concern is served where `concerns:` names it, got {labels:?}"
    );
    let qualifier = |label: &str| -> Option<String> {
        routes
            .iter()
            .find(|node| node.label == label)
            .and_then(|node| node.metadata.get("handler_qualifier").cloned())
    };
    assert_eq!(
        qualifier("route GET /admin/users/:id/download").as_deref(),
        Some("Admin::UsersController"),
        "an action written inside a resource block is that resource's"
    );
    assert_eq!(
        qualifier("route GET /admin/export_domain_allows/export").as_deref(),
        Some("Admin::ExportDomainAllowsController"),
        "and a collection block does not change which controller serves it"
    );
    assert_eq!(
        qualifier("route POST /admin/email_subscriptions/setup").as_deref(),
        Some("Admin::EmailSubscriptions::SetupsController"),
        "a singular resource is served by the controller named for the set"
    );
    assert_eq!(
        qualifier("route GET /admin/terms_of_service/:terms_of_service_id/preview").as_deref(),
        Some("Admin::TermsOfService::PreviewsController"),
        "`module:` moves the controller without moving the path"
    );

    let handled = |label: &str| -> Option<String> {
        let route = routes.iter().find(|node| node.label == label)?.id;
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.source == route
                    && edge.metadata.get("relation").map(String::as_str)
                        == Some("entrypoint_function")
            })
            .and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
            .map(|node| node.label.clone())
    };
    assert_eq!(
        handled("route GET /admin/export_domain_allows/export").as_deref(),
        Some("export"),
        "and the action it names is reached"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_environment_read_named_by_a_constant_says_which_variable_it_reads() {
    // `os.Getenv(envLogFile)` is how a Go program reads `TF_LOG_PATH`,
    // and the constant is declared in whichever file declares it. 45 of
    // terraform's 62 computed reads name one, and each read as a hole in
    // the environment map. A loop variable names nothing to look up and
    // stays a hole, which is the honest answer.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("names.go"),
        "package internal\n\nconst (\n\tenvLogFile = \"TF_LOG_PATH\"\n\tenvLog     = \"TF_LOG\"\n)\n",
    )
    .unwrap();
    fs::write(
        root.join("main.go"),
        "package main\n\nimport \"os\"\n\nfunc read(keys []string) (string, string, string) {\n\tvalue := \"\"\n\tfor _, key := range keys {\n\t\tvalue = os.Getenv(key)\n\t}\n\treturn os.Getenv(envLogFile), os.Getenv(\"TF_INPUT\"), value\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let read = |label: &str| {
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Environment
                && node.label == label
                && graph
                    .edges
                    .iter()
                    .any(|edge| edge.kind == EdgeKind::ReadsEnvironment && edge.target == node.id)
        })
    };
    assert!(
        read("TF_LOG_PATH"),
        "the constant says which variable the read names"
    );
    assert!(read("TF_INPUT"), "and a literal still names its own");
    assert!(
        read("<computed name>"),
        "while a key the loop builds names nothing to look up"
    );
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::ReadsEnvironment
                && edge.metadata.get("resolution").map(String::as_str) == Some("named_constant")
        }),
        "and the read says what settled its name"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_csharp_file_of_top_level_statements_is_where_the_program_starts() {
    // .NET lets one file per project write statements outside any
    // declaration, and the compiler wraps them in `Program.Main`.
    // eShopOnWeb starts all three of its programs that way, and with no
    // `Main` to find, nothing said where any of them begins.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("Program.cs"),
        "using System;\n\nvar builder = WebApplication.CreateBuilder(args);\nvar app = builder.Build();\napp.Run();\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Catalog.cs"),
        "namespace Shop;\n\npublic class Catalog\n{\n    public int Count()\n    {\n        return 0;\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let program = graph
        .nodes
        .iter()
        .find(|node| node.metadata.get("entrypoint_kind").map(String::as_str) == Some("program"))
        .expect("the file of statements is the program");
    assert_eq!(
        program.label, "Program",
        "which is what the compiler calls it"
    );
    assert_eq!(
        program.span.as_ref().map(|span| span.path.as_str()),
        Some("src/Program.cs")
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| { edge.kind == EdgeKind::Entrypoint && edge.target == program.id }),
        "and the repository starts there"
    );
    // The program is every statement outside a declaration, so the calls
    // those statements make are the program's: with only the first
    // statement in its span, eShopOnWeb's three programs reached nothing.
    let called: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.source == program.id)
        .filter_map(|edge| edge.metadata.get("call_label").map(String::as_str))
        .collect();
    assert!(
        called.contains(&"builder.Build") && called.contains(&"app.Run"),
        "the program makes the calls its statements write, got {called:?}"
    );
    assert!(
        !graph.nodes.iter().any(|node| node.label == "Program"
            && node
                .span
                .as_ref()
                .is_some_and(|span| span.path == "src/Catalog.cs")),
        "a file that only declares types starts nothing"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_minimal_api_declares_a_route_and_a_razor_page_serves_where_it_sits() {
    // `app.MapGet("api/catalog-items", ..)` writes the verb into the
    // method name, so the `.get(` every other route call ends in never
    // appeared and eShopOnWeb's minimal API endpoints were missing from a
    // graph that found its attribute routes. A Razor Page states its URL
    // by sitting under `Pages/`, and the `.cshtml.cs` beside it says
    // which methods it serves.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/Web/Pages/Basket")).unwrap();
    fs::create_dir_all(root.join("src/Web/Areas/Identity/Pages/Account")).unwrap();
    fs::create_dir_all(root.join("src/Blazor/Pages")).unwrap();
    fs::write(
        root.join("src/Web/Endpoints.cs"),
        "public static class Endpoints\n{\n    public static void AddRoutes(IEndpointRouteBuilder app)\n    {\n        app.MapGet(\"api/catalog-items\",\n            async (IRepository repository) => await ListAsync(repository));\n        app.MapDelete(\"/api/catalog-items/{id}\", DeleteAsync);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Web/Pages/Basket/Index.cshtml"),
        "\u{feff}@page \"{handler?}\"\n@model IndexModel\n<h1>Basket</h1>\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Web/Pages/Basket/Index.cshtml.cs"),
        "public class IndexModel : PageModel\n{\n    public async Task OnGet()\n    {\n        Load();\n    }\n\n    public async Task OnPost(CatalogItemViewModel product)\n    {\n        Add(product);\n    }\n\n    public async Task OnPostUpdate(IEnumerable<BasketItemViewModel> items)\n    {\n        Update(items);\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Web/Areas/Identity/Pages/Account/Login.cshtml"),
        "@page\n@model LoginModel\n<h1>Login</h1>\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Blazor/Pages/List.razor"),
        "@page \"/admin\"\n<h1>Admin</h1>\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Entrypoint && node.label.starts_with("route "))
        .map(|node| node.label.as_str())
        .collect();
    for expected in [
        "route GET /api/catalog-items",
        "route DELETE /api/catalog-items/{id}",
        "route GET /Basket/{handler?}",
        "route POST /Basket/{handler?}",
        "route GET /Identity/Account/Login",
        "route GET /admin",
    ] {
        assert!(
            routes.contains(&expected),
            "{expected} is a route the project serves, got {routes:?}"
        );
    }
    assert_eq!(
        routes
            .iter()
            .filter(|label| **label == "route POST /Basket/{handler?}")
            .count(),
        1,
        "two handlers for one method are one route the page serves, got {routes:?}"
    );

    let post = graph
        .nodes
        .iter()
        .find(|node| node.label == "route POST /Basket/{handler?}")
        .expect("the page serves POST")
        .id;
    let handler = graph
        .edges
        .iter()
        .find(|edge| {
            edge.source == post
                && edge.metadata.get("relation").map(String::as_str) == Some("entrypoint_function")
        })
        .map(|edge| edge.target)
        .and_then(|id| graph.nodes.iter().find(|node| node.id == id))
        .expect("the code behind handles it");
    assert_eq!(
        handler.label, "OnPost",
        "the handler is the method named for the verb"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_dynamic_import_loads_a_file_rather_than_calling_a_function_named_import() {
    // `import('./Home.vue')` is how a router loads a page on demand, and
    // it is the only edge that reaches one. koel filed 168 of them as
    // calls to a function named `import` and reached none of the pages.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("package.json"),
        "{\n  \"name\": \"app\",\n  \"dependencies\": { \"lodash\": \"^4\" }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("routes.js"),
        "export const routes = [\n  { path: '/home', component: () => import('./Home.js') },\n]\n\nexport async function heavy(name) {\n  await import('lodash')\n  return import(`./pages/${name}.js`)\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Home.js"),
        "export function Home() {\n  return 'home'\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    assert!(
        !graph.edges.iter().any(|edge| edge.kind == EdgeKind::Calls
            && edge.metadata.get("call_label").map(String::as_str) == Some("import")),
        "loading a module is not a call to a function named `import`"
    );
    let home = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "src/Home.js")
        .expect("the lazily loaded page is indexed")
        .id;
    assert!(
        graph.edges.iter().any(|edge| {
            edge.target == home
                && edge.metadata.get("relation").map(String::as_str) == Some("local_import_file")
        }),
        "the dynamic import reaches the file it loads"
    );
    let imported: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.label.starts_with("import(\""))
        .map(|node| node.label.clone())
        .collect();
    assert!(
        imported.contains(&"import(\"lodash\")".to_string()),
        "a package loaded on demand is still that package, got {imported:?}"
    );
    assert_eq!(
        imported.len(),
        2,
        "a path the program builds at runtime names nothing to resolve, got {imported:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_compiler_macro_and_a_test_runners_globals_are_provided_rather_than_missing() {
    // `defineProps` is expanded by the compiler that reads a `<script
    // setup>` block, and `describe` is handed to a test file by its
    // runner. Neither is imported, and reading them as calls the resolver
    // failed on buried the 1027 failures somebody can act on under 365
    // that nobody can.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("Title.vue"),
        "<script setup lang=\"ts\">\nconst props = defineProps<{ title: string }>()\n</script>\n\n<template>\n  <h1>{{ props.title }}</h1>\n</template>\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("title.test.ts"),
        "describe('Title', () => {\n  it('renders', () => {\n    expect(1).toBe(1)\n  })\n})\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("helper.ts"),
        "export function defineProps() {\n  return 1\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| -> Option<String> {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("defineProps").as_deref(),
        Some("builtin"),
        "a compiler macro is provided to the block the compiler reads"
    );
    assert_eq!(
        resolution("describe").as_deref(),
        Some("builtin"),
        "a runner hands its test file `describe`"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_contract_is_not_a_function_its_methods_are_local_to() {
    // A solidity method carries `enclosing_function: <its contract>`, which
    // is how a reference written inside the contract knows whose it is. The
    // rule that a definition nested in another is visible only inside it
    // then read that as a hiding scope and dropped every inherited call:
    // openzeppelin declares 3477 methods that way and 2150 of its calls
    // reached none of them. A definition's own type does not hide it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::write(
        root.join("contracts").join("Base.sol"),
        "pragma solidity ^0.8.0;\n\nabstract contract Base {\n    function plainHelper() internal pure returns (uint256) {\n        return 1;\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("contracts").join("Child.sol"),
        "pragma solidity ^0.8.0;\n\nimport {Base} from \"./Base.sol\";\n\ncontract Child is Base {\n    function run() public pure returns (uint256) {\n        return plainHelper();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let helper = graph
        .nodes
        .iter()
        .find(|node| node.label == "plainHelper" && node.kind == NodeKind::Function)
        .expect("the base declares it");
    assert_eq!(
        helper.metadata.get("owner_type").map(String::as_str),
        Some("Base"),
        "the contract that declares it is its owner"
    );
    assert_eq!(
        helper
            .metadata
            .get("enclosing_function")
            .map(String::as_str),
        Some("Base"),
        "the contract it is written in is still recorded, which is what a \
         reference inside it is attributed to"
    );
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.target == helper.id
                && edge.metadata.get("resolution").map(String::as_str) == Some("resolved")
        }),
        "so the inherited call reaches it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_scala_type_alias_is_a_type_the_project_declares() {
    // `type NonEmptyMap[K, +A] = NonEmptyMapImpl.Type[K, A]` declares a
    // type as much as a class does. cats writes 106 alias names and the
    // graph had 34 of them, so asking what depends on `NonEmptyMap` found
    // nothing -- and every rule that asks whether the project declares a
    // name read the alias as someone else's.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("data.scala"),
        "package demo\n\nobject Impl {\n  type Inner = String\n}\n\ntype Alias = Impl.Inner\n\nclass Holder\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let types: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Type)
        .map(|node| node.label.as_str())
        .collect();
    for declared in ["Alias", "Inner", "Holder", "Impl"] {
        assert!(
            types.contains(&declared),
            "the project declares {declared}: {types:?}"
        );
    }
}

#[test]
fn an_ocaml_module_call_is_not_answered_by_a_same_named_local() {
    // `Process.run` is that module's function, whatever this file happens
    // to call `run`. Letting the same-file name answer said 2366 of dune's
    // calls belong to the definition that contains them, which `doctor`
    // reports as a definition calling itself -- 1027 times on dune, 822 on
    // cats. A file that declares the module inside itself is the exception,
    // and opam's `opamStd.ml` is why: it writes `module List = struct .. end`
    // and then calls `List.map`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("caller.ml"),
        "let run () = 1\n\nlet start () = Process.run ()\n",
    )
    .unwrap();
    fs::write(root.join("src").join("process.ml"), "let run () = 2\n").unwrap();
    fs::write(
        root.join("src").join("nested.ml"),
        "module Local = struct\n  let step () = 3\nend\n\nlet step () = 4\n\nlet go () = Local.step ()\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let target_of = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == edge.target)
                    .and_then(|node| node.span.as_ref())
                    .map(|span| span.path.clone())
            })
    };
    assert_ne!(
        target_of("Process.run").as_deref(),
        Some("src/caller.ml"),
        "the module names whose `run` it is, and this file is not that module"
    );
    assert_eq!(
        target_of("Local.step").as_deref(),
        Some("src/nested.ml"),
        "while a module the file declares itself is answered where it is declared"
    );
}

#[test]
fn an_erlang_call_names_the_module_that_answers_it() {
    // `gun:open(..)` says which module answers, and a module this project
    // has no file for is a dependency's or OTP's -- OTP is answered before
    // this, so what is left is a dependency. cowboy writes 1764 such calls
    // to `gun`, `ranch`, `cow_hpack` and `quicer`, every one of them
    // reported as a resolver failure. A capitalised head is a variable
    // holding a module name and says nothing about where it points.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("listener.erl"),
        "-module(listener).\n-export([start/0]).\n\nstart() ->\n    ok = gun:open(\"host\", 80),\n    helper:assist(),\n    lists:map(fun(X) -> X end, []),\n    Transport:send(<<>>).\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("helper.erl"),
        "-module(helper).\n-export([assist/0]).\n\nassist() ->\n    ok.\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution_of = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution_of("gun:open").as_deref(),
        Some("external"),
        "the project has no `gun` module"
    );
    assert_eq!(
        resolution_of("helper:assist").as_deref(),
        Some("resolved"),
        "while it does have `helper`"
    );
    assert_ne!(
        resolution_of("Transport:send").as_deref(),
        Some("external"),
        "a module held in a variable says nothing about where it points"
    );
}

#[test]
fn testthat_hands_an_r_suite_its_assertions() {
    // testthat is R's harness and its vocabulary is closed: every assertion
    // is an `expect_*` and the blocks around them are named outright. dplyr
    // writes 418 of them and every one sits under `tests/`. `pkg::fn` names
    // the package that answers it, which is not this one -- 189 more.
    let root = temp_project_root();
    fs::create_dir_all(root.join("R")).unwrap();
    fs::create_dir_all(root.join("tests").join("testthat")).unwrap();
    fs::write(root.join("DESCRIPTION"), "Package: demo\nVersion: 0.1.0\n").unwrap();
    fs::write(
        root.join("R").join("select.R"),
        "select_columns <- function(data) {\n  lifecycle::signal_stage(\"experimental\", \"select_columns()\")\n  data\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("testthat").join("test-select.R"),
        "test_that(\"it keeps the data\", {\n  expect_equal(select_columns(1), 1)\n  expect_snapshot(select_columns(2))\n})\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution_of = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    for harness in ["test_that", "expect_equal", "expect_snapshot"] {
        assert_eq!(
            resolution_of(harness).as_deref(),
            Some("builtin"),
            "the harness provides {harness}"
        );
    }
    assert_eq!(
        resolution_of("lifecycle::signal_stage").as_deref(),
        Some("external"),
        "the source names the package that answers it"
    );
    assert_eq!(
        resolution_of("select_columns").as_deref(),
        Some("resolved"),
        "while the package's own function is the package's"
    );
}

#[test]
fn a_nix_file_takes_lib_and_pkgs_from_whoever_imports_it() {
    // A nix file is a function: `{ config, lib, pkgs, ... }:` says what its
    // caller supplies, so `lib.optionalAttrs` names something inside a
    // value this file was handed and nothing in the project can be looked
    // up for it. home-manager writes 1834 such calls, 1505 through `lib`
    // and 311 through `pkgs`. That is what a call through a bound value
    // means everywhere else, and the resolution stays unresolved because
    // saying `external` would claim to know which package answers.
    let root = temp_project_root();
    fs::create_dir_all(root.join("modules")).unwrap();
    fs::write(
        root.join("modules").join("program.nix"),
        "{ lib, pkgs ? import <nixpkgs> { }, ... }:\n\nlet\n  render = name: lib.optionalString true name;\n  wrap = name: pkgs.writeText name \"body\";\n  stray = name: missing.thing name;\nin {\n  inherit render wrap stray;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reason_of = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .map(|edge| {
                (
                    edge.metadata.get("resolution").cloned(),
                    edge.metadata.get("unresolved_reason").cloned(),
                )
            })
    };
    assert_eq!(
        reason_of("lib.optionalString"),
        Some((
            Some("unresolved".to_string()),
            Some("local_value".to_string())
        )),
        "the caller supplies `lib`"
    );
    assert_eq!(
        reason_of("pkgs.writeText"),
        Some((
            Some("unresolved".to_string()),
            Some("local_value".to_string())
        )),
        "and `pkgs`, whose default does not end the parameter list"
    );
    assert_eq!(
        reason_of("missing.thing").map(|(_, reason)| reason),
        Some(None),
        "while a name the file never took is still a name nothing answers"
    );
}

#[test]
fn a_shell_command_is_the_environments_when_it_is_not_the_scripts() {
    // A shell has three places a command can come from: a function the
    // script declares, the shell itself, and PATH. There is no fourth, so
    // `unresolved` says a resolver failed where nothing was there to find
    // -- and not one of redis's 424 or shellcheck's 58 unresolved shell
    // calls named a function either project declares.
    let root = temp_project_root();
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("scripts").join("release.sh"),
        "#!/usr/bin/env bash\n\nannounce() {\n    echo \"$1\"\n}\n\nmain() {\n    announce start\n    pwd\n    git status\n}\n\nmain\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution_of = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution_of("announce").as_deref(),
        Some("resolved"),
        "the script declares it"
    );
    assert_eq!(
        resolution_of("pwd").as_deref(),
        Some("builtin"),
        "the shell provides it"
    );
    assert_eq!(
        resolution_of("git").as_deref(),
        Some("external"),
        "and PATH provides the rest"
    );
    assert!(
        graph.edges.iter().all(|edge| {
            edge.metadata.get("language").map(String::as_str) != Some("bash")
                || edge.metadata.get("resolution").map(String::as_str) != Some("unresolved")
        }),
        "nothing shell-shaped is left unresolved"
    );
}

#[test]
fn a_quickcheck_property_is_run_by_its_module() {
    // `$(forAllProperties)` collects every top-level `prop_*` through
    // Template Haskell, so the harness runs it and no edge records that --
    // the same thing `#[test]` does. shellcheck writes 2252 of them and
    // read as functions nobody calls they made 2756 orphan findings where
    // 579 belonged. The file has to say it collects them: a `prop_` prefix
    // in a module that does not is a name, not a property.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("Checks.hs"),
        "module Checks where\n\nimport Test.QuickCheck.All (forAllProperties)\n\nprop_addsUp :: Bool\nprop_addsUp = True\n\nhelper :: Bool\nhelper = False\n\nrunTests = $( [| $(forAllProperties) |] )\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Plain.hs"),
        "module Plain where\n\nprop_notCollected :: Bool\nprop_notCollected = True\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let invoked_by = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("invoked_by").cloned())
    };
    assert_eq!(
        invoked_by("prop_addsUp").as_deref(),
        Some("test_runner"),
        "the module collects it"
    );
    assert_eq!(
        invoked_by("helper"),
        None,
        "while an ordinary definition beside it is the program's"
    );
    assert_eq!(
        invoked_by("prop_notCollected"),
        None,
        "and a module that collects nothing runs nothing"
    );
}

#[test]
fn an_unsettled_call_marks_the_definitions_it_may_mean() {
    // The placeholder was the only record of an ambiguity, so none of the
    // definitions it might mean had an incoming edge and each read as a
    // function nobody calls. The narrowing that produced the candidates is
    // gone by the time an insight runs, so the resolver says which they
    // were: terraform's 3658 unsettled calls have 77707 candidates against
    // 333455 declarations that merely share their names.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("left.rs"),
        "pub struct Left;\n\nimpl Left {\n    pub fn eqv(&self) -> bool {\n        true\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("right.rs"),
        "pub struct Right;\n\nimpl Right {\n    pub fn eqv(&self) -> bool {\n        false\n    }\n}\n\npub fn only_here() -> bool {\n    true\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "mod left;\nmod right;\n\nfn main() {\n    let value = pick();\n    let _ = value.eqv();\n}\n\nfn pick() -> left::Left {\n    left::Left\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let marked = |path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node.label == "eqv"
                    && node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.path.ends_with(path))
            })
            .map(|node| node.metadata.get("may_be_called_by").cloned())
    };
    assert_eq!(
        marked("left.rs"),
        Some(Some("unsettled_call".to_string())),
        "the call could mean this one"
    );
    assert_eq!(
        marked("right.rs"),
        Some(Some("unsettled_call".to_string())),
        "and this one"
    );
    let unmarked = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "only_here")
        .expect("only_here");
    assert_eq!(
        unmarked.metadata.get("may_be_called_by"),
        None,
        "a name nothing reaches for is not marked"
    );
}

#[test]
fn a_file_that_says_a_generator_wrote_it_is_not_the_programs_own() {
    // 219 of gqlgen's 865 go files carry a generator's banner and hold
    // 14363 of its 18653 functions; 168 of them sit where no path rule
    // looks, because `generated.go` is written beside the resolvers a
    // person wrote. The banner is the only thing that travels between
    // languages, and the generator writes it down itself.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    println!(\"hi\");\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("bindings.rs"),
        "// Code generated by bindgen. DO NOT EDIT.\n\npub fn bound() -> u32 {\n    1\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("hand_written.rs"),
        "// A note that mentions a generator without being one.\n\npub fn written() -> u32 {\n    2\n}\n",
    )
    .unwrap();
    // ffigen writes seven `// ignore_for_file:` lines before its banner,
    // which a six-line window missed by two.
    fs::write(
        root.join("src").join("late_banner.rs"),
        "// ignore_for_file: a\n// ignore_for_file: b\n// ignore_for_file: c\n// ignore_for_file: d\n// ignore_for_file: e\n// ignore_for_file: f\n// ignore_for_file: g\n// AUTO GENERATED FILE, DO NOT EDIT.\n\npub fn bound_late() -> u32 {\n    3\n}\n",
    )
    .unwrap();
    // A generator names itself at the head of the line without shouting:
    // oscar ships 95 migrations that say only this, in its own source tree.
    fs::write(
        root.join("src").join("migration.rs"),
        "// Generated by Django 1.10.6 on 2017-03-30 14:35\n\npub fn applied() -> u32 {\n    4\n}\n",
    )
    .unwrap();
    // The same words inside a sentence are prose: dplyr documents "a list
    // of columns generated by [vars()]".
    fs::write(
        root.join("src").join("documented.rs"),
        "// A list of columns generated by vars(), which this function reads.\n\npub fn documented() -> u32 {\n    5\n}\n",
    )
    .unwrap();
    // kong returns a `DO NOT EDIT` header as a lua long string: the banner
    // is data the file emits, not a statement about the file.
    fs::write(
        root.join("src").join("template.rs"),
        "pub fn template() -> &'static str {\n    \"# DO NOT EDIT THIS FILE\"\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let written_by = |path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == path)
            .and_then(|node| node.metadata.get("written_by").cloned())
    };
    assert_eq!(
        written_by("src/bindings.rs").as_deref(),
        Some("generator"),
        "the banner in its first lines says so"
    );
    assert_eq!(
        written_by("src/hand_written.rs"),
        None,
        "and a file that merely mentions one is a person's"
    );
    assert_eq!(written_by("src/main.rs"), None);
    assert_eq!(
        written_by("src/late_banner.rs").as_deref(),
        Some("generator"),
        "the banner is in the opening comment however long that is"
    );
    assert_eq!(
        written_by("src/template.rs"),
        None,
        "and a banner the file emits as data is not a banner about the file"
    );
    assert_eq!(
        written_by("src/migration.rs").as_deref(),
        Some("generator"),
        "a generator that names itself at the head of the line said so"
    );
    assert_eq!(
        written_by("src/documented.rs"),
        None,
        "while the same words inside a sentence are prose"
    );
}

#[test]
fn a_call_is_test_code_when_its_own_file_is() {
    // "The program does not call its own tests" asked the CALLER node for
    // its file, and a call written at a lua module's top level has no
    // caller with a span. Those calls read as program code wherever they
    // were written, so the rule refused them every helper their own suite
    // declares: kong writes 287 `helpers.get_db_utils` in `spec/` and not
    // one reached `spec/internal/db.lua`.
    let root = temp_project_root();
    fs::create_dir_all(root.join("spec/internal")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("spec/internal").join("db.lua"),
        "local M = {}\n\nfunction M.get_db_utils()\n  return 1\nend\n\nreturn M\n",
    )
    .unwrap();
    fs::write(
        root.join("spec").join("some_spec.lua"),
        "local db = require \"spec.internal.db\"\n\ndb.get_db_utils()\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("app.lua"),
        "local db = require \"spec.internal.db\"\n\ndb.get_db_utils()\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let helper = graph
        .nodes
        .iter()
        .find(|node| node.label.ends_with("get_db_utils") && node.kind == NodeKind::Function)
        .expect("the suite declares it");
    let callers: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.target == helper.id)
        .filter_map(|edge| edge.metadata.get("file").map(String::as_str))
        .collect();
    assert!(
        callers.iter().any(|file| file.starts_with("spec/")),
        "a call written in the suite reaches its own helper: {callers:?}"
    );
    assert!(
        !callers.iter().any(|file| file.starts_with("src/")),
        "and the program still does not call its tests: {callers:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_module_that_includes_another_answers_for_what_it_re_exports() {
    // `include M` makes M's definitions the includer's own, and dune builds
    // whole modules that way: `src/fiber/src/fiber.ml` is 38 lines of
    // `include Core` and module aliases, so `Fiber.return` names something
    // core.ml declares. 207 of dune's files include another module and 345
    // calls to `Fiber.return` alone reached nothing.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("core.ml"), "let answer x = x\n").unwrap();
    fs::write(root.join("src").join("fiber.ml"), "include Core\n").unwrap();
    fs::write(
        root.join("src").join("user.ml"),
        "let run () = Fiber.answer 1\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let module_node = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Module
                && node.label == "Fiber"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with("fiber.ml"))
        })
        .expect("the file is a module");
    assert_eq!(
        module_node.metadata.get("extends").map(String::as_str),
        Some("Core"),
        "what it includes it re-exports"
    );
    let answer = graph
        .nodes
        .iter()
        .find(|node| node.label == "answer" && node.kind == NodeKind::Function)
        .expect("core declares it");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.target == answer.id
                && edge.metadata.get("call_label").map(String::as_str) == Some("Fiber.answer")
        }),
        "so a call through the includer reaches it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_super_call_means_the_parent_never_the_caller() {
    // `super.x` written inside `x` is the parent's implementation. Answering
    // with the caller's own closes a call edge from a definition to itself,
    // and openzeppelin wrote 174 of those.
    let root = temp_project_root();
    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::write(
        root.join("contracts").join("Base.sol"),
        "pragma solidity ^0.8.0;\n\nabstract contract Base {\n    function update() internal virtual {\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("contracts").join("Child.sol"),
        "pragma solidity ^0.8.0;\n\nimport {Base} from \"./Base.sol\";\n\ncontract Child is Base {\n    function update() internal virtual override {\n        super.update();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let of = |contract: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node.label == "update"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(contract)
            })
            .map(|node| node.id)
            .unwrap_or_else(|| panic!("{contract} declares update"))
    };
    let parent = of("Base");
    let child = of("Child");
    let supers: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("super.update")
        })
        .collect();
    assert!(!supers.is_empty(), "the call is in the graph");
    assert!(
        supers.iter().all(|edge| edge.target != child),
        "and never answers with the caller's own"
    );
    assert!(
        supers.iter().any(|edge| edge.target == parent),
        "it answers with the one it inherits"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_contract_states_every_base_it_names() {
    // Solidity composes rather than descends: `abstract contract
    // PaymasterSigner is AbstractSigner, EIP712, Paymaster` reaches
    // `EIP712._hashTypedDataV4` through its second base. Nothing recorded
    // any of them, so openzeppelin's 354 contracts stated no parent at all.
    let root = temp_project_root();
    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::write(
        root.join("contracts").join("Both.sol"),
        "pragma solidity ^0.8.0;\n\nabstract contract Base {\n    function plainHelper() internal pure returns (uint256) {\n        return 1;\n    }\n}\n\nabstract contract Mixin {}\n\ncontract Child is Base, Mixin {\n    function run() public pure returns (uint256) {\n        return plainHelper();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let child = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Type && node.label == "Child")
        .expect("the contract is a type");
    assert_eq!(
        child.metadata.get("extends").map(String::as_str),
        Some("Base,Mixin"),
        "both bases, in the order the source names them"
    );
    let base = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Type && node.label == "Base")
        .expect("the base is a type");
    assert_eq!(
        base.metadata.get("extends"),
        None,
        "a contract that names none states none"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_line_on_an_edge_says_which_file_it_is_in() {
    // `component-contract` answered `bot= -> actor_type` with `line: 224`
    // and no file, and 82,320 of mastodon's 82,816 edges that carry a line
    // carried it that way. A line without a file is not a place.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "mod helper;\n\nfn main() {\n    helper::run();\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("helper.rs"),
        "pub fn run() {\n    missing_helper();\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let placed = graph
        .edges
        .iter()
        .filter(|edge| edge.metadata.contains_key("line"))
        .collect::<Vec<_>>();
    assert!(
        !placed.is_empty(),
        "the fixture makes edges that carry a line"
    );
    for edge in &placed {
        let file = edge
            .metadata
            .get("file")
            .unwrap_or_else(|| panic!("an edge with a line says which file: {:?}", edge.metadata));
        assert!(
            file.ends_with(".rs"),
            "the file is the call site's, not a label: {file}"
        );
    }
    let run = graph
        .edges
        .iter()
        .find(|edge| edge.metadata.get("call_label").map(String::as_str) == Some("helper::run"))
        .expect("main calls helper::run");
    assert_eq!(
        run.metadata.get("file").map(String::as_str),
        Some("src/main.rs"),
        "and it is the file the call is written in, not the one it reaches"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_slash_in_a_javascript_call_is_a_regular_expression_not_a_name() {
    // `/^\s*$/.test(value)` left `/^\s*$/.test` as a call target, and
    // mastodon reported `+\.json$/.exec` among the calls nothing resolved.
    // A shell script naming the program it runs is a call, which is why the
    // rule asks the language first.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("check.js"),
        "export function blank(value) {\n  return /^\\s*$/.test(value) && trimmed(value);\n}\n\nexport function trimmed(value) {\n  return value;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("build.sh"),
        "#!/bin/sh\n./configure\n/usr/bin/env node src/check.js\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call_labels = graph
        .edges
        .iter()
        .filter_map(|edge| edge.metadata.get("call_label").cloned())
        .collect::<Vec<_>>();
    assert!(
        !call_labels.iter().any(|label| label.contains('/')
            && graph.edges.iter().any(|edge| {
                edge.metadata.get("call_label") == Some(label)
                    && edge.metadata.get("language").map(String::as_str) == Some("javascript")
            })),
        "a javascript name holds no slash: {call_labels:?}"
    );
    assert!(
        call_labels.iter().any(|label| label == "trimmed"),
        "the real call beside it is still read: {call_labels:?}"
    );
    assert!(
        call_labels.iter().any(|label| label == "./configure"),
        "a shell script names the program it runs: {call_labels:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_function_an_object_holds_is_a_value_the_program_indexes() {
    // mastodon writes its modals as `{ 'ACCOUNT_NOTE': () => import(..) }`
    // and picks one by a key it computes, and lint-staged writes
    // `{ '**/*.ts?(x)': () => 'yarn tsc' }`. Neither key is a name anybody
    // writes a call to, so "has no incoming call edge" was said about 214 of
    // mastodon's functions and about a glob.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("modals.js"),
        "const MODALS = {\n  'ACCOUNT_NOTE': () => loadNote(),\n};\n\nexport function open(kind) {\n  return MODALS[kind]();\n}\n\nexport function spelledOut() {\n  return 1;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let held = graph
        .nodes
        .iter()
        .find(|node| node.label == "ACCOUNT_NOTE" && node.kind == NodeKind::Function)
        .expect("the function the object holds is in the graph");
    assert_eq!(
        held.metadata.get("definition_form").map(String::as_str),
        Some("value"),
        "a function an object holds is a value"
    );
    let spelled = graph
        .nodes
        .iter()
        .find(|node| node.label == "spelledOut" && node.kind == NodeKind::Function)
        .expect("a function the file spells out");
    assert_eq!(
        spelled.metadata.get("definition_form"),
        None,
        "a function the file spells out is not one"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_value_a_factory_builds_is_a_declaration_other_files_call() {
    // `export const onMounted = createHook(MOUNTED)` is how vue declares
    // most of its public API, and `const buttonVariants = cva(..)` how a
    // component library declares its variants. Neither was in the graph, so
    // 523 of vue's calls resolved to nothing. A local `const rows =
    // getRows()` inside a function is a variable, not a declaration.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("hooks.ts"),
        "import { createHook } from './factory'\n\nexport const onMounted = createHook('mounted')\n\nexport function useIt() {\n  const rows = createHook('rows')\n  return rows\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("factory.ts"),
        "export function createHook(name: string) {\n  return () => name\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("app.ts"),
        "import { onMounted } from './hooks'\n\nexport function start() {\n  return onMounted()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let value = graph
        .nodes
        .iter()
        .find(|node| node.label == "onMounted" && node.kind == NodeKind::Function)
        .expect("the value the factory builds is a declaration");
    assert_eq!(
        value.metadata.get("definition_form").map(String::as_str),
        Some("value"),
        "and it says it is a value rather than a function spelled out"
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == value.id),
        "a file that imports it and calls it reaches it"
    );
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.label == "rows" && node.kind == NodeKind::Function),
        "a local variable inside a function is not a declaration"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_component_is_used_where_jsx_renders_it_and_ts_and_tsx_are_one_project() {
    // `<TailwindIndicator />` is how a JSX runtime calls a component, and
    // it was not read at all: taxonomy's layout renders eleven components
    // and reached none. And a TypeScript project with React components is
    // written in two languages -- `.ts` and `.tsx` -- so every import from
    // a module into a component crossed a line the resolver would not: 32
    // of taxonomy's 494 calls resolved.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::create_dir_all(root.join("components")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("lib").join("utils.ts"),
        "export function cn(...inputs: string[]) {\n  return inputs.join(' ')\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("components").join("indicator.tsx"),
        "export function TailwindIndicator() {\n  return null\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("components").join("layout.tsx"),
        "import { cn } from \"../lib/utils\"\nimport { TailwindIndicator } from \"./indicator\"\n\nexport function Layout() {\n  return <div className={cn(\"a\")}><TailwindIndicator /><span /></div>\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |label: &str, file: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.label == label
                        && node.kind == NodeKind::Function
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path.ends_with(file))
                })
        })
    };
    assert!(
        reaches("TailwindIndicator", "indicator.tsx"),
        "rendering a component is using it"
    );
    assert!(
        reaches("cn", "utils.ts"),
        "and a `.tsx` file reaches the `.ts` module it imports"
    );
    // A lower-case tag is the platform's, not a component this project
    // declares.
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.label == "span" || node.label == "div"),
        "an html tag is not a component"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_next_js_project_declares_its_routes_by_where_its_files_sit() {
    // Next.js, Nuxt and SvelteKit name a URL by the path of the file that
    // serves it, so a project written that way had no entrypoints at all --
    // no routes, and nothing for a workflow or a journey to start from.
    // The manifest is what says the project is written that way: `app/` is
    // a PHP directory as often as a Next.js one.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("api").join("users")).unwrap();
    fs::create_dir_all(
        root.join("app")
            .join("(marketing)")
            .join("blog")
            .join("[slug]"),
    )
    .unwrap();
    fs::create_dir_all(root.join("pages").join("api")).unwrap();
    fs::write(
        root.join("package.json"),
        "{\n  \"name\": \"shop\",\n  \"dependencies\": { \"next\": \"^15.0.0\" }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("api").join("users").join("route.ts"),
        "export async function GET(request: Request) {\n  return null\n}\n\nexport async function POST(request: Request) {\n  return null\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app")
            .join("(marketing)")
            .join("blog")
            .join("[slug]")
            .join("page.tsx"),
        "export default function Post() {\n  return null\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("pages").join("api").join("legacy.ts"),
        "export default function handler(req: any, res: any) {\n  return null\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Entrypoint)
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(
        routes,
        BTreeSet::from([
            "route GET /api/users",
            "route POST /api/users",
            // A `(marketing)` segment groups files without naming a URL.
            "route GET /blog/:slug",
            // A pages API handler serves whatever method it is sent.
            "route ANY /api/legacy",
        ]),
        "the layout states the routes"
    );
    // The exported verb is the handler the route reaches.
    let get = graph
        .nodes
        .iter()
        .find(|node| node.label == "route GET /api/users")
        .expect("the GET route");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.source == get.id
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|resolution| resolution == "framework_route_handler")
        }),
        "and it reaches the function that serves it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_layout_is_an_entrypoint_and_base_url_names_a_directory() {
    // A Next.js layout wraps every route beneath it and has no URL of its
    // own, so the eleven components taxonomy's layout renders were reached
    // by nothing. And `baseUrl` makes every directory under it importable
    // by name: `import { User } from "types"` is the `types/` directory
    // beside the tsconfig, which eleven of taxonomy's files write.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("types")).unwrap();
    fs::write(
        root.join("package.json"),
        "{\n  \"name\": \"shop\",\n  \"dependencies\": { \"next\": \"^15.0.0\" }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("tsconfig.json"),
        "{\n  \"compilerOptions\": { \"baseUrl\": \".\" }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("types").join("index.ts"),
        "export function formatUser(name: string) {\n  return name\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("layout.tsx"),
        "import { formatUser } from \"types\"\n\nexport default function Layout() {\n  return formatUser(\"x\")\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    assert!(
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Entrypoint
                && node.metadata.get("entrypoint_kind").map(String::as_str)
                    == Some("framework_entry")
        }),
        "the layout is an entrypoint of its own"
    );
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.label == "formatUser"
                        && node.kind == NodeKind::Function
                })
        }),
        "and `types` names the directory the tsconfig's baseUrl points at"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_directory_named_app_is_not_a_route_unless_the_project_says_so() {
    // koel keeps its PHP in `app/`, and a `route.ts` shape means nothing
    // there. The manifest is the evidence.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("api").join("users")).unwrap();
    fs::write(
        root.join("package.json"),
        "{\n  \"name\": \"tool\",\n  \"dependencies\": { \"express\": \"^4.0.0\" }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("api").join("users").join("route.ts"),
        "export function GET() {\n  return null\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Entrypoint && node.label.starts_with("route ")),
        "nothing says this project routes by file path"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_spec_that_calls_a_route_is_not_a_route() {
    // `post '/accounts', params: { id: 1 }` in a request spec calls a route;
    // it does not declare one. Sinatra declares a route with the block that
    // serves it, and reading a brace anywhere on the line as that block made
    // 148 of mastodon's specs read as routes the program serves.
    let root = temp_project_root();
    fs::create_dir_all(root.join("spec")).unwrap();
    fs::write(
        root.join("app.rb"),
        "require 'sinatra'\n\nget '/health' do\n  'ok'\nend\n\nget('/ready') {\n  'ok'\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("spec").join("app_spec.rb"),
        "describe 'the app' do\n  it 'answers' do\n    get '/health', params: { id: 1 }\n    post :batch, params: {\n      id: 1,\n    }\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Entrypoint
                && node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
        })
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(
        routes,
        ["route GET /health", "route GET /ready"],
        "only the declarations that carry a block are routes"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rails_reads_a_collection_block_and_a_namespaced_controller() {
    // A `collection do` block holds the set's routes, which have no id, and
    // `to: 'auth/registrations#new'` names a class inside a module. Mastodon
    // declares both `Auth::RegistrationsController` and
    // `Admin::Fasp::RegistrationsController`, so the last path segment alone
    // chose neither and the handler read as missing.
    let root = temp_project_root();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("app").join("controllers").join("auth")).unwrap();
    fs::write(
        root.join("config").join("routes.rb"),
        "Rails.application.routes.draw do\n  resources :requests, only: [:index] do\n    collection do\n      post :accept, to: 'requests#accept_bulk'\n    end\n\n    member do\n      post :dismiss\n    end\n  end\n\n  get '/invite/:code', to: 'auth/registrations#new'\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app")
            .join("controllers")
            .join("auth")
            .join("registrations_controller.rb"),
        "class Auth::RegistrationsController < Devise::RegistrationsController\n  def new\n    super\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let route = |label: &str| {
        graph.nodes.iter().find(|node| {
            node.kind == NodeKind::Entrypoint && node.label == format!("route {label}")
        })
    };
    assert!(
        route("POST /requests/accept").is_some(),
        "a collection route has no id of its own: {:?}",
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Entrypoint)
            .map(|node| node.label.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        route("POST /requests/:id/dismiss").is_some(),
        "a member route keeps an id, and Rails names it `:id` --          `:request_id` is what a resource nested inside gets"
    );
    let invite = route("GET /invite/:code").expect("the invite route");
    assert_eq!(
        invite.metadata.get("handler_qualifier").map(String::as_str),
        Some("Auth::RegistrationsController"),
        "the controller path states the modules the class sits in"
    );
    let handler = graph
        .nodes
        .iter()
        .find(|node| node.label == "new" && node.kind == NodeKind::Function)
        .expect("the handler");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.source == invite.id
                && edge.target == handler.id
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|resolution| resolution == "framework_route_handler")
        }),
        "and the route reaches it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_python_call_on_a_value_is_not_a_project_method_of_the_same_name() {
    // `key.split(",")` is a string's and `kwargs.setdefault` a dict's,
    // while django-oscar declares a `split` template filter and flask a
    // `setdefault`. `self` is the one receiver whose methods are the
    // class's own, and the mapping protocol stays out of the list because a
    // project that mimics a dict declares all of it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app").join("filters.py"),
        "def split(value, separator=','):\n    return value\n\n\ndef slugify(value):\n    return value\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("views.py"),
        "from .filters import split, slugify\n\n\nclass View:\n    def render(self, key):\n        parts = key.split(',')\n        return slugify(split(parts, ','))\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let calls = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some(label)
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path.ends_with("filters.py"))
                })
        })
    };
    assert!(!calls("key.split"), "`key.split(',')` is the string's own");
    assert!(
        calls("split") && calls("slugify"),
        "the imported filters still resolve"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_js_call_on_a_value_is_not_a_project_function_of_the_same_name() {
    // `str.trim()` is a string's, `Buffer.concat` node's, `args.map` an
    // array's -- and axios declares a `trim`, vue a `map` and zod a
    // `startsWith`, so matching on the tail of the call gave each of them
    // callers it never had. `this.trim()` is the class's own.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\n  \"name\": \"app\"\n}\n").unwrap();
    fs::write(
        root.join("src").join("utils.js"),
        "export function trim(value) {\n  return value\n}\n\nexport function shout(value) {\n  return value\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("app.js"),
        "import { trim, shout } from './utils'\n\nexport function run(str) {\n  const clean = str.trim()\n  return shout(trim(clean))\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let calls_into_utils = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some(label)
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path.ends_with("utils.js"))
                })
        })
    };
    assert!(
        !calls_into_utils("str.trim"),
        "`str.trim()` is the string's own"
    );
    assert!(
        calls_into_utils("trim") && calls_into_utils("shout"),
        "the imported helpers still resolve"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_call_into_otp_is_the_platforms() {
    // cowboy calls `gen_tcp:recv` 283 times and `lists:keyfind` 238, ecto
    // `Enum.reverse` 79: a module the platform ships is not a dependency
    // this repository failed to hold, and reporting those as unresolved
    // reads as a resolver that failed.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("server.erl"),
        "-module(server).\n-export([start/0]).\n\nstart() ->\n    Sorted = lists:sort([2, 1]),\n    own_helper(Sorted).\n\nown_helper(X) -> X.\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == label
                    && node.metadata.get("item_kind").map(String::as_str) == Some("call")
            })
            .and_then(|node| node.metadata.get("resolution").cloned())
    };
    assert_eq!(
        resolution("lists:sort").as_deref(),
        Some("builtin"),
        "OTP's `lists` is the platform's"
    );
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.target && node.label == "own_helper")
        }),
        "and the module's own function still resolves"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_ruby_call_on_a_value_is_not_a_project_method_every_value_has() {
    // `params.each`, `@queue.empty?`, `formats.include?`: ruby writes the
    // receiver and the label keeps only the method, so a project method
    // named after one every collection has answered calls on values it
    // never saw. mastodon's `Trends::History#each` had 268 callers, and
    // its connection pool's own `@queue.size` was answered by the `size`
    // it declares two lines above.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app").join("history.rb"),
        "class History\n  def each(&block)\n    @values.each(&block)\n  end\n\n  def refresh\n    @values = []\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("filter.rb"),
        "class Filter\n  def run(params, history)\n    params.each { |key, value| puts key }\n    history.refresh\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && graph.nodes.iter().any(|node| {
                    node.id == edge.target
                        && node.label == label
                        && node
                            .span
                            .as_ref()
                            .is_some_and(|span| span.path.ends_with("history.rb"))
                })
        })
    };
    assert!(
        !reaches("each"),
        "`params.each` is a hash's, not this project's"
    );
    assert!(
        reaches("refresh"),
        "and a method the core library does not have still resolves"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_ruby_call_through_a_gems_constant_is_not_this_projects_method() {
    // A ruby call's label keeps only the method name, so
    // `Addressable::URI.parse(href).normalize` and the project's own
    // `HashtagNormalizer#normalize` looked like the same call. The constant
    // the call is written through is the evidence: one the project never
    // declares belongs to a gem.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("lib")).unwrap();
    fs::write(
        root.join("app").join("lib").join("hashtag_normalizer.rb"),
        "class HashtagNormalizer\n  def normalize(tag)\n    tag\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("lib").join("parser.rb"),
        "class Parser\n  def run(href)\n    Addressable::URI.parse(href).normalize\n  end\n\n  def own(tag)\n    HashtagNormalizer.new.normalize(tag)\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let normalize = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "normalize"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with("hashtag_normalizer.rb"))
        })
        .expect("the project's own normalize");
    let callers: Vec<u32> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.target == normalize.id)
        .filter_map(|edge| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == edge.source)
                .and_then(|node| node.span.as_ref())
                .map(|span| span.start_line)
        })
        .collect();
    assert_eq!(
        callers.len(),
        1,
        "only the call written through the project's own constant reaches it: {callers:?}"
    );
    // The gem call is not a resolver failure -- it left the project.
    let external = graph.nodes.iter().any(|node| {
        node.kind == NodeKind::ExternalDependency
            && node.label == "normalize"
            && node.metadata.get("resolution").map(String::as_str) == Some("external")
    });
    assert!(external, "the gem call is recorded as leaving the project");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_elixir_alias_reaches_the_module_file() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib").join("ecto")).unwrap();
    fs::write(
        root.join("lib").join("ecto").join("repo.ex"),
        "defmodule Ecto.Repo do\n  alias Ecto.Query\n  import Config\n\n  def all(_), do: []\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("lib").join("ecto").join("query.ex"),
        "defmodule Ecto.Query do\n  def from(_), do: %{}\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let query = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "lib/ecto/query.ex")
        .expect("missing module file");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == query.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        }),
        "`alias Ecto.Query` must reach lib/ecto/query.ex"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_lua_require_reaches_the_module_it_names() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("kong").join("tools")).unwrap();
    fs::write(
        root.join("kong").join("handler.lua"),
        "local utils = require \"kong.tools.utils\"\nlocal cjson = require \"cjson\"\nreturn { utils = utils, cjson = cjson }\n",
    )
    .unwrap();
    fs::write(
        root.join("kong").join("tools").join("utils.lua"),
        "local M = {}\nfunction M.trim(s) return s end\nreturn M\n",
    )
    .unwrap();
    // The rock the project requires, whose name a file here happens to share.
    fs::write(
        root.join("kong").join("tools").join("cjson.lua"),
        "local M = {}\nreturn M\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let file = |path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == path)
            .unwrap_or_else(|| panic!("missing {path}"))
            .id
    };
    let reaches = |target: NodeId| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == target
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        })
    };
    assert!(reaches(file("kong/tools/utils.lua")), "kong.tools.utils");
    // `require "cjson"` names a rock, not the file that happens to share
    // the name.
    assert!(!reaches(file("kong/tools/cjson.lua")), "cjson is a rock");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_ocaml_open_reaches_the_module_file() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.ml"),
        "open Stdune\n\nlet run () = Stdune.Path.root\n",
    )
    .unwrap();
    fs::write(root.join("src").join("stdune.ml"), "let version = \"1\"\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let stdune = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "src/stdune.ml")
        .expect("missing module file");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == stdune.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        }),
        "`open Stdune` must reach stdune.ml"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_haskell_import_reaches_the_module_it_names() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("ShellCheck")).unwrap();
    fs::write(
        root.join("src").join("ShellCheck").join("Checker.hs"),
        "module ShellCheck.Checker where\nimport ShellCheck.AST\nimport qualified Data.List\n\ncheck :: Int -> Int\ncheck x = x\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("ShellCheck").join("AST.hs"),
        "module ShellCheck.AST where\n\nid' :: Int -> Int\nid' x = x\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let ast = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "src/ShellCheck/AST.hs")
        .expect("missing module file");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == ast.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        }),
        "the import must reach the file its module name names"
    );
    // `import qualified Data.List` names a library, not a file this
    // project failed to ship.
    assert!(
        !graph.nodes.iter().any(|node| {
            node.metadata
                .get("resolution")
                .is_some_and(|value| value == "unresolved")
                && node.label.contains("Data.List")
        }),
        "a library import must not be reported as a missing local file"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_nix_import_reaches_the_file_it_names() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("modules").join("shell")).unwrap();
    fs::write(
        root.join("default.nix"),
        "{ pkgs ? import <nixpkgs> {} }:\nlet\n  shell = import ./modules/shell;\n  helper = import ./modules/helper.nix { inherit pkgs; };\nin\n  { inherit shell helper; }\n",
    )
    .unwrap();
    fs::write(
        root.join("modules").join("shell").join("default.nix"),
        "{ }:\n{ value = 1; }\n",
    )
    .unwrap();
    fs::write(
        root.join("modules").join("helper.nix"),
        "{ pkgs }:\n{ value = 2; }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reaches = |target: &str| {
        let file = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == target)
            .unwrap_or_else(|| panic!("missing {target}"));
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == file.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        })
    };
    // A directory means its `default.nix`, and a file means itself.
    assert!(
        reaches("modules/shell/default.nix"),
        "import ./modules/shell"
    );
    assert!(reaches("modules/helper.nix"), "import ./modules/helper.nix");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_erlang_include_reaches_its_header() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("include")).unwrap();
    fs::write(
        root.join("src").join("demo.erl"),
        "-module(demo).\n-include(\"demo.hrl\").\n-export([run/0]).\nrun() -> ok.\n",
    )
    .unwrap();
    fs::write(
        root.join("include").join("demo.hrl"),
        "-define(TAG, demo).\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let header = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "include/demo.hrl")
        .expect("missing header");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::References
                && edge.target == header.id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "local_import_file")
        }),
        "the include must reach the header beside the module"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_stable_id_survives_an_edit_above_it() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    let ids = |graph: &CodeGraph| {
        graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Function)
            .filter_map(|node| {
                node.metadata
                    .get("stable_id")
                    .map(|id| (node.label.clone(), id.clone()))
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };

    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    helper();\n}\n\nfn helper() -> u32 {\n    1\n}\n",
    )
    .unwrap();
    let before = ids(&scan_project(&root, &IndexOptions::default()).unwrap());

    // A function added at the top moves every line below it.
    fs::write(
        root.join("src").join("main.rs"),
        "fn added() {}\n\nfn main() {\n    helper();\n}\n\nfn helper() -> u32 {\n    1\n}\n",
    )
    .unwrap();
    let after = ids(&scan_project(&root, &IndexOptions::default()).unwrap());

    for (label, id) in &before {
        assert_eq!(
            after.get(label),
            Some(id),
            "`{label}` changed identity because something above it moved"
        );
    }
    assert!(after.contains_key("added"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_repository_is_named_by_its_directory() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("temp root has a name")
        .to_string();

    let absolute = scan_project(&root, &IndexOptions::default()).unwrap();
    // The same project reached through a path that ends in `.` -- what
    // `codegraph scan .` hands the indexer -- is the same project. (The
    // cwd is process-wide; a test must not move it out from under the
    // others.)
    let dotted = scan_project(root.join("."), &IndexOptions::default()).unwrap();

    let repository = |graph: &CodeGraph| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Repository)
            .expect("no repository node")
            .clone()
    };
    assert_eq!(repository(&absolute).label, name);
    assert_eq!(repository(&dotted).label, name);
    assert_eq!(
        repository(&absolute).metadata.get("stable_id"),
        repository(&dotted).metadata.get("stable_id"),
        "one project, one identity"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn a_fact_belongs_to_the_definition_that_holds_it() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    // One name, three definitions -- the shape flask writes for a typed
    // function. The `raise` is written inside the last one.
    fs::write(
        root.join("cli.py"),
        r#"import typing as t


@t.overload
def locate_app(module_name: str, app_name: str) -> t.Any: ...


@t.overload
def locate_app(module_name: str, app_name: None) -> None: ...


def locate_app(module_name, app_name):
    if not app_name:
        raise RuntimeError("no application found")
    return app_name
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let error_source = graph
        .edges
        .iter()
        .find(|edge| edge.kind == EdgeKind::MayError)
        .map(|edge| edge.source)
        .expect("no may_error edge");
    let holder = graph
        .nodes
        .iter()
        .find(|node| node.id == error_source)
        .expect("missing source node");
    let span = holder.span.as_ref().expect("source has no span");
    assert_eq!(holder.label, "locate_app");
    assert!(
        span.start_line <= 13 && 15 <= span.end_line,
        "the raise on line 15 is outside {}-{}, so it went to a stub",
        span.start_line,
        span.end_line
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
fn a_marker_inside_a_string_is_a_fixture() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    // The shape this repository's own tests are written in: a fixture
    // carrying comment markers inside a raw string.
    fs::write(
        root.join("lib.rs"),
        "// HACK: this one is a note about this file\nfn write_fixture() -> &'static str {\n    r#\"fn sample() {\n    // HACK: this one is a sample\n}\n\"#\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let markers: Vec<u32> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "rationale_comment")
        })
        .filter_map(|node| node.span.as_ref().map(|span| span.start_line))
        .collect();
    assert_eq!(markers, vec![1], "the sample inside the string was counted");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_marker_is_shouted_or_punctuated_but_never_prose() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("hook.go"),
        r#"package hook

func run() {
    // We don't expect any other actions in here, so anything else is a
    // bug in the caller but we'll ignore it in order to be robust.
    // Hack: we borrow the sensitive-value treatment from the decoder.
    // TODO(alice): drop this once the decoder lands.
    // HACK we shell out because the library has no batch mode
    // note that the caller already holds the lock
    // — an em dash keeps the marker off a character boundary
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("SECURITY.md"),
        "# Security Policy\n\nReport issues to the maintainers.\n<!-- FIXME: link the advisory page -->\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let mut found: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|value| value == "rationale_comment")
        })
        .map(|node| node.label.as_str())
        .collect();
    found.sort_unstable();

    assert_eq!(
        found,
        vec![
            "FIXME: link the advisory page",
            "HACK: we borrow the sensitive-value treatment from the decoder.",
            "HACK: we shell out because the library has no batch mode",
            "TODO: drop this once the decoder lands.",
        ],
        "prose that opens with a marker word, and a markdown heading, are not markers"
    );

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
    // One variable is one node no matter how many files read it; the per-read
    // fallback rides on the reading edge.
    let ports = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Environment && node.label == "PORT")
        .collect::<Vec<_>>();
    assert_eq!(ports.len(), 1, "PORT should be one shared node");
    let port = ports[0];
    assert_eq!(
        port.metadata.get("declaration_scope").map(String::as_str),
        Some("shared")
    );

    let defaults = graph
        .edges
        .iter()
        .filter(|edge| edge.target == port.id && edge.kind == EdgeKind::ReadsEnvironment)
        .filter_map(|edge| edge.metadata.get("default_value").map(String::as_str))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        defaults,
        BTreeSet::from([
            "3000", "5000", "7000", "8000", "8080", "9090", "9091", "9092"
        ])
    );
    // Each read still names its own language and file.
    let languages = graph
        .edges
        .iter()
        .filter(|edge| edge.target == port.id && edge.kind == EdgeKind::ReadsEnvironment)
        .filter_map(|edge| edge.metadata.get("language").map(String::as_str))
        .collect::<BTreeSet<_>>();
    assert!(languages.contains("python"), "languages: {languages:?}");
    assert!(languages.contains("rust"), "languages: {languages:?}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_project_keeps_every_read_site_of_one_environment_key() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("settings.py"),
        r#"import os


def configure():
    required = os.environ["PORT"]
    fallback = os.getenv("PORT", "8080")
    return required, fallback
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let ports = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Environment && node.label == "PORT")
        .collect::<Vec<_>>();
    assert_eq!(ports.len(), 1, "PORT should be one shared node");

    // Both reads happen inside the same function, so they share a source and a
    // target; keeping one edge per read site is what lets the analysis see
    // that the key is read once as required and once with a fallback.
    let reads = graph
        .edges
        .iter()
        .filter(|edge| edge.target == ports[0].id && edge.kind == EdgeKind::ReadsEnvironment)
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 2, "expected both read sites, got {reads:?}");
    assert_eq!(
        reads
            .iter()
            .map(|edge| edge.source)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([reads[0].source]),
        "both reads come from the same function"
    );
    assert_eq!(
        reads
            .iter()
            .filter_map(|edge| edge.metadata.get("default_value").map(String::as_str))
            .collect::<Vec<_>>(),
        vec!["8080"],
        "only the fallback read carries a default"
    );
    assert_eq!(
        reads
            .iter()
            .filter_map(|edge| edge.metadata.get("line"))
            .collect::<BTreeSet<_>>()
            .len(),
        2,
        "read sites are distinguished by line"
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
                .is_some_and(|value| value == ">=0.24")
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
fn an_entrypoint_cites_the_line_its_own_section_declares_it_on() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{
  "name": "demo",
  "devDependencies": {
    "eslint": "^8.56.0"
  },
  "scripts": {
    "eslint": "eslint src",
    "build": "gulp"
  }
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("CMakeLists.txt"),
        "project(demo)\n\nADD_EXECUTABLE(demo-test test.c)\n\nADD_TEST(NAME demo-test\n  COMMAND demo-test)\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let line_of = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == label
                    && node.metadata.get("item_kind").map(String::as_str)
                        == Some("manifest_entrypoint")
            })
            .and_then(|node| node.span.as_ref())
            .map(|span| span.start_line)
    };

    // The name is written twice: once as a dev dependency and once as the
    // script. Only the script's own section declares the script.
    assert_eq!(line_of("npm script:eslint"), Some(7));
    assert_eq!(line_of("npm script:build"), Some(8));
    // And the command that builds the executable is where it is declared,
    // not the later command that names it again.
    assert_eq!(line_of("cmake executable:demo-test"), Some(3));

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
    // `IMAGE := demo` is a variable; there is nothing to run.
    assert!(
        !graph
            .nodes
            .iter()
            .any(|node| node.label == "make target:IMAGE"),
        "a variable assignment was read as a target"
    );
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

/// What a declaring site says about an environment variable: the facts
/// live on the edge from the job or service that sets it, because the
/// variable itself is one node the whole project shares.
fn environment_fact(graph: &CodeGraph, environment: NodeId, key: &str) -> Option<String> {
    graph
        .edges
        .iter()
        .find(|edge| {
            edge.target == environment
                && edge.kind == EdgeKind::ReadsEnvironment
                && edge.metadata.contains_key(key)
        })
        .and_then(|edge| edge.metadata.get(key).cloned())
}

#[test]
fn a_declared_block_spans_the_lines_it_declares() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("docker-compose.yml"),
        r#"services:
  web:
    image: demo/web
    ports:
      - "8080:80"
  db:
    image: postgres:16
"#,
    )
    .unwrap();
    fs::write(
        root.join(".gitlab-ci.yml"),
        r#"build:
  stage: build
  script:
    - make build
test:
  stage: test
  script:
    - make test
"#,
    )
    .unwrap();
    fs::write(
        root.join("deploy.yaml"),
        r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
spec:
  template:
    spec:
      containers:
        - name: web
          image: demo/web
---
apiVersion: v1
kind: Service
metadata:
  name: web
spec:
  ports:
    - port: 80
"#,
    )
    .unwrap();
    fs::write(
        root.join("schema.sql"),
        "CREATE TABLE user (
  id INTEGER,
  name TEXT
);
",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let span_of = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.label == label)
            .and_then(|node| node.span.clone())
            .map(|span| (span.start_line, span.end_line))
            .unwrap_or_else(|| panic!("missing {label}"))
    };
    assert_eq!(span_of("compose service:web"), (2, 5));
    assert_eq!(span_of("compose service:db"), (6, 7));
    assert_eq!(span_of("gitlab job:build"), (1, 4));
    assert_eq!(span_of("gitlab job:test"), (5, 8));
    assert_eq!(span_of("k8s deployment:default/web"), (1, 10));
    assert_eq!(span_of("k8s service:default/web"), (12, 18));
    assert_eq!(span_of("sql table:user"), (1, 4));

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
    // What a service assigns to a variable is a fact of the assignment,
    // so it rides on the edge rather than on the shared variable node.
    assert_eq!(
        environment_fact(&graph, app_env, "value_present"),
        Some("true".to_string())
    );
    assert_eq!(
        environment_fact(&graph, worker_token, "value_source"),
        Some("host".to_string())
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
fn a_workflow_job_spans_the_steps_it_runs() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".github").join("workflows")).unwrap();
    fs::write(
        root.join(".github").join("workflows").join("ci.yml"),
        r#"name: CI
on: [push]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: ./build.sh
      - run: ./test.sh
  deploy:
    runs-on: ubuntu-latest
    steps:
      - run: ./deploy.sh
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let span_of = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Entrypoint && node.label == label)
            .and_then(|node| node.span.clone())
            .unwrap_or_else(|| panic!("missing {label}"))
    };
    // The job is written on line 5 and its last step on line 9.
    let build = span_of("github workflow:CI/build");
    assert_eq!((build.start_line, build.end_line), (5, 9));
    let deploy = span_of("github workflow:CI/deploy");
    assert_eq!((deploy.start_line, deploy.end_line), (10, 13));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_manifest_that_does_not_parse_says_so() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("fixtures")).unwrap();
    // A missing brace, and a table header that never closes.
    fs::write(
        root.join("package.json"),
        "{\"name\": \"demo\", \"dependencies\": {",
    )
    .unwrap();
    fs::write(root.join("Cargo.toml"), "[package\nname = \"demo\"\n").unwrap();
    // A whole manifest is fine.
    fs::write(
        root.join("composer.json"),
        "{\"name\": \"demo/app\", \"require\": {\"psr/log\": \"^3.0\"}}",
    )
    .unwrap();
    // A deliberately broken fixture is a note rather than a warning.
    fs::write(root.join("fixtures").join("package.json"), "{oops").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reasons: Vec<_> = graph
        .nodes
        .iter()
        .filter_map(|node| {
            node.metadata
                .get("manifest_parse_error")
                .map(|reason| (node.label.as_str(), reason.as_str()))
        })
        .collect();
    let labels: Vec<_> = reasons.iter().map(|(label, _)| *label).collect();
    assert!(labels.contains(&"package.json"), "{reasons:?}");
    assert!(labels.contains(&"Cargo.toml"), "{reasons:?}");
    assert!(!labels.contains(&"composer.json"), "{reasons:?}");
    assert!(
        reasons
            .iter()
            .all(|(_, reason)| !reason.contains('\n') && reason.len() <= 200),
        "the reason is one short line: {reasons:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_workflow_step_runs_where_the_workflow_says() {
    let root = temp_project_root();
    fs::create_dir_all(root.join(".github").join("workflows")).unwrap();
    fs::create_dir_all(root.join("pkgs").join("http").join("test")).unwrap();
    fs::write(
        root.join("pkgs")
            .join("http")
            .join("test")
            .join("client_test.dart"),
        "void main() {}\n",
    )
    .unwrap();
    fs::write(
        root.join(".github").join("workflows").join("dart.yml"),
        r#"name: Dart CI
on: [push]

jobs:
  unit_test:
    runs-on: ubuntu-latest
    steps:
      - name: run the tests
        run: dart test test/client_test.dart
        working-directory: pkgs/http
  generate:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: pkgs/http
    steps:
      - name: check the generated file
        run: git diff --exit-code test/client_test.dart
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    assert_graph_invariants(&graph);
    let steps: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "github_actions_run_step")
        })
        .filter_map(|node| node.metadata.get("command_path").map(String::as_str))
        .collect();
    assert_eq!(
        steps,
        vec![
            "pkgs/http/test/client_test.dart",
            "pkgs/http/test/client_test.dart"
        ],
        "a step's own working-directory and the job's defaults both move the path"
    );
    let test_file = graph
        .nodes
        .iter()
        .find(|node| node.label == "pkgs/http/test/client_test.dart")
        .expect("the test file is scanned");
    assert_eq!(
        graph
            .edges
            .iter()
            .filter(|edge| edge.target == test_file.id
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "github_actions_run_command_path"))
            .count(),
        2,
        "both jobs reach the file they run"
    );
    fs::remove_dir_all(&root).ok();
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
        environment_fact(&graph, global_token, "value_kind"),
        Some("secret_reference".to_string())
    );
    assert_eq!(
        environment_fact(&graph, build_mode, "value_kind"),
        Some("literal".to_string())
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
    let test_mode = node_id(&graph, NodeKind::Environment, "TEST_MODE");
    assert_eq!(
        environment_fact(&graph, test_mode, "value_kind"),
        Some("literal".to_string())
    );
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
fn a_rails_resource_declares_what_it_says_it_declares() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("app").join("controllers")).unwrap();
    fs::create_dir_all(root.join("db")).unwrap();
    // mastodon writes 64 resources with `only:`, and reading them as the
    // whole set of seven invented routes it does not serve.
    fs::write(
        root.join("config").join("routes.rb"),
        "Rails.application.routes.draw do\n  concern :actor do\n    resource :outbox, only: [:show]\n  end\n\n  resources :accounts, path: 'users', only: [:show] do\n    resources :statuses, only: [:show]\n  end\n  resources :followers, only: [:index], controller: :follower_accounts\n  get '/about', to: 'about#show'\n  get :verify_credentials, to: 'credentials#show'\n\n  namespace :api do\n    namespace :v2 do\n      get '/search', to: 'search#index'\n    end\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app")
            .join("controllers")
            .join("accounts_controller.rb"),
        "class AccountsController\n  def show\n    render :show\n  end\nend\n\nmodule Settings\n  class AccountsController\n    def show\n      render :settings\n    end\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("db").join("schema.rb"),
        "ActiveRecord::Schema.define(version: 1) do\n  create_table \"accounts\", force: :cascade do |t|\n    t.string \"username\"\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: Vec<(String, String)> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
        })
        .map(|node| {
            (
                node.metadata.get("method").cloned().unwrap_or_default(),
                node.metadata.get("path").cloned().unwrap_or_default(),
            )
        })
        .collect();

    // `path: 'users'` renames the segment and `only: [:show]` is one route.
    assert!(
        routes.contains(&("GET".to_string(), "/users/:id".to_string())),
        "{routes:?}"
    );
    assert!(
        !routes
            .iter()
            .any(|(method, path)| method == "DELETE" && path == "/users/:id"),
        "{routes:?}"
    );
    assert!(
        routes.contains(&("GET".to_string(), "/followers".to_string())),
        "{routes:?}"
    );

    // A namespace states a module as well as a path segment, so the
    // route names `Api::V2::SearchController` rather than every
    // `SearchController` in the project.
    let search = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
                && node.metadata.get("path").map(String::as_str) == Some("/api/v2/search")
        })
        .expect("the namespaced route is in the graph");
    assert_eq!(
        search.metadata.get("handler_qualifier").map(String::as_str),
        Some("Api::V2::SearchController")
    );

    // A symbol names the path as often as a string does, and the `to:`
    // target is not the path.
    assert!(
        routes.contains(&("GET".to_string(), "/verify_credentials".to_string())),
        "{routes:?}"
    );
    // A nested resource lives under its parent's member path, and a
    // concern states routes to be mounted elsewhere.
    assert!(
        routes.contains(&(
            "GET".to_string(),
            "/users/:account_id/statuses/:id".to_string()
        )),
        "{routes:?}"
    );
    assert!(
        !routes.iter().any(|(_, path)| path == "/outbox"),
        "{routes:?}"
    );

    // The controller the route names settles which `show` it means, and
    // the name as written wins over one that merely ends the same way.
    let show = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && node.label == "show"
                && node.metadata.get("owner_type").map(String::as_str) == Some("AccountsController")
        })
        .expect("the controller action is in the graph");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.target == show.id
                && edge.metadata.get("resolution").map(String::as_str)
                    == Some("framework_route_handler")
        }),
        "the route reaches the action it names"
    );

    // And `db/schema.rb` states the tables.
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.label == "sql table:accounts"),
        "the schema declares the table"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_asciidoc_document_states_its_sections() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("contracts")).unwrap();
    fs::write(root.join("remappings.txt"), "@openzeppelin/=contracts/\n").unwrap();
    // openzeppelin writes one of these beside every contract directory.
    fs::write(
        root.join("contracts").join("README.adoc"),
        "= Access Control\n\nNOTE: read the guide.\n\n== Core\n\nUse `remappings.txt` to point at the contracts.\n\n=== Extensions\n\nSee xref:governance.adoc[the governance guide].\n",
    )
    .unwrap();
    fs::write(
        root.join("contracts").join("governance.adoc"),
        "= Governance\n\nHow to set up on-chain governance.\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let sections: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("document_section")
        })
        .map(|node| node.label.as_str())
        .collect();

    assert!(
        sections.contains(&"contracts/README.adoc#Access Control"),
        "{sections:?}"
    );
    assert!(
        sections.contains(&"contracts/README.adoc#Core"),
        "{sections:?}"
    );
    assert!(
        sections.contains(&"contracts/README.adoc#Extensions"),
        "{sections:?}"
    );

    // A cross-reference reaches the document it names, and a literal
    // reaches the file.
    let relations: Vec<&str> = graph
        .edges
        .iter()
        .filter_map(|edge| edge.metadata.get("relation").map(String::as_str))
        .filter(|relation| relation.starts_with("asciidoc"))
        .collect();
    assert!(relations.contains(&"asciidoc_xref"), "{relations:?}");
    assert!(
        relations.contains(&"asciidoc_literal_path"),
        "{relations:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_generate_directive_names_the_program_that_writes_the_code() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("graph")).unwrap();
    fs::create_dir_all(root.join("testdata")).unwrap();
    // gqlgen writes 58 of these and terraform 42.
    fs::write(
        root.join("testdata").join("gqlgen.go"),
        "package main\n\nfunc main() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("graph").join("resolver.go"),
        "//go:generate go run ../testdata/gqlgen.go\n\npackage graph\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let directive = graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("generate_directive")
        })
        .expect("the directive is in the graph");
    assert_eq!(
        directive
            .span
            .as_ref()
            .map(|span| (span.path.as_str(), span.start_line)),
        Some(("graph/resolver.go", 1))
    );

    // And it reaches the program it names.
    let target = graph
        .nodes
        .iter()
        .find(|node| node.label == "testdata/gqlgen.go")
        .expect("the generator is in the graph");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.source == directive.id
                && edge.target == target.id
                && edge.metadata.get("resolution").map(String::as_str)
                    == Some("go_generate_command_path")
        }),
        "the directive reaches the program that writes the code"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_single_file_component_states_its_program_in_a_script_block() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("player.ts"),
        "export const play = () => true;\n",
    )
    .unwrap();
    // koel writes 337 components like this one.
    fs::write(
        root.join("src").join("App.vue"),
        r#"<template>
  <button @click="start">Play</button>
</template>

<script setup lang="ts">
import { play } from './player'

const start = () => {
  return play()
}
</script>

<style scoped>
button { color: red; }
</style>
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let start = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "start")
        .expect("the component's function is in the graph");
    // Every line outside the script is blanked, so a fact keeps the line
    // of the file that holds it.
    assert_eq!(
        start.span.as_ref().map(|span| span.start_line),
        Some(8),
        "{:?}",
        start.span
    );
    assert_eq!(
        start.metadata.get("language").map(String::as_str),
        Some("typescript")
    );
    // And what the component imports resolves like any other import.
    assert!(
        graph.nodes.iter().any(|node| {
            node.label.contains("import { play }")
                && node.metadata.get("resolved_path").map(String::as_str) == Some("src/player.ts")
        }),
        "the component's import reaches the file it names"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_dialog_asking_a_question_is_not_a_sql_statement() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // koel asks this in a confirmation dialog, and the graph read it as a
    // statement against a table called `the`.
    fs::write(
        root.join("src").join("menu.ts"),
        "export const remove = async () => {\n  return confirm('Delete selected playable(s) from the filesystem? This action is NOT reversible!')\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let queries: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.label.starts_with("sql query:"))
        .map(|node| node.label.as_str())
        .collect();
    assert!(queries.is_empty(), "{queries:?}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_compiled_requirements_file_is_a_lock_not_a_constraint() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("examples").join("celery")).unwrap();
    // flask's example is pip-compile output: it pins the released flask
    // and everything that release wanted, which is one installation rather
    // than what this project asks for.
    fs::write(
        root.join("examples").join("celery").join("requirements.txt"),
        "#\n# This file is autogenerated by pip-compile with Python 3.11\n#\nblinker==1.6.2\n    # via flask\nflask==2.3.2\n",
    )
    .unwrap();
    fs::write(
        root.join("requirements.txt"),
        "# what the project asks for\nblinker>=1.9.0\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let kinds: Vec<(String, String)> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::DependsOn)
        .filter_map(|edge| {
            let manifest = graph.nodes.iter().find(|node| node.id == edge.source)?;
            let package = graph.nodes.iter().find(|node| node.id == edge.target)?;
            (package.label == "blinker").then(|| {
                (
                    manifest.label.clone(),
                    edge.metadata
                        .get("dependency_version_kind")
                        .cloned()
                        .unwrap_or_default(),
                )
            })
        })
        .collect();

    assert!(
        kinds.contains(&(
            "examples/celery/requirements.txt".to_string(),
            "locked".to_string()
        )),
        "{kinds:?}"
    );
    assert!(
        kinds.contains(&("requirements.txt".to_string(), "constraint".to_string())),
        "{kinds:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_php_include_written_from_the_root_reaches_the_file() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("Providers")).unwrap();
    fs::create_dir_all(root.join("routes")).unwrap();
    // Laravel's `base_path()` names a path from the project root, and the
    // provider that writes it sits three directories down.
    fs::write(
        root.join("routes").join("channels.php"),
        "<?php\n\nBroadcast::channel('user.{id}', static fn () => true);\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Providers").join("BroadcastServiceProvider.php"),
        "<?php\n\nclass BroadcastServiceProvider\n{\n    public function boot(): void\n    {\n        require base_path('routes/channels.php');\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let include = graph
        .nodes
        .iter()
        .find(|node| node.label.contains("channels.php") && node.label.contains("require"))
        .expect("the include is in the graph");
    assert_eq!(
        include.metadata.get("resolved_path").map(String::as_str),
        Some("routes/channels.php"),
        "{:?}",
        include.metadata
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_lockfile_says_which_package_autoloads_a_namespace() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    // koel imports `Illuminate\..` and declares `laravel/framework`; no
    // rule about names could connect the two, and the lockfile states it.
    fs::write(
        root.join("composer.json"),
        r#"{
  "name": "koel/koel",
  "require": {
    "laravel/framework": "^11.0"
  }
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("composer.lock"),
        r#"{
  "packages": [
    {
      "name": "laravel/framework",
      "version": "v11.0.0",
      "autoload": {
        "psr-4": {
          "Illuminate\\": "src/Illuminate/"
        }
      }
    }
  ],
  "packages-dev": []
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("app").join("Event.php"),
        r#"<?php

use Illuminate\Broadcasting\Channel;
use Spatie\Permission\Models\Role;

class Event
{
    public function channel(): Channel
    {
        return new Channel('events');
    }
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let framework = graph
        .nodes
        .iter()
        .find(|node| node.label == "laravel/framework")
        .expect("the package is in the graph");
    assert_eq!(
        framework
            .metadata
            .get("autoloaded_namespaces")
            .map(String::as_str),
        Some("Illuminate\\")
    );

    // What the analysis reads: the framework states its namespace on the
    // node, and nothing states one for `Spatie\Permission\..`.
    let namespaces: Vec<&str> = graph
        .nodes
        .iter()
        .filter_map(|node| node.metadata.get("autoloaded_namespaces"))
        .map(String::as_str)
        .collect();
    assert_eq!(namespaces, vec!["Illuminate\\"], "{namespaces:?}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_import_written_through_a_path_alias_reaches_the_file() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("resources").join("js").join("stores")).unwrap();
    // koel writes `@/utils` and states what `@` is in the tsconfig beside
    // its sources; every bundler reads the same file.
    fs::write(
        root.join("resources").join("tsconfig.json"),
        r#"{
  // The alias every component writes.
  "compilerOptions": {
    "paths": {
      "@/*": ["./js/*"],
    },
  },
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("resources").join("js").join("utils.ts"),
        "export const noop = () => {};
",
    )
    .unwrap();
    fs::write(
        root.join("resources")
            .join("js")
            .join("stores")
            .join("userStore.ts"),
        "export const userStore = {};
",
    )
    .unwrap();
    fs::write(
        root.join("resources").join("js").join("app.ts"),
        "import { noop } from '@/utils';
import { userStore } from '@/stores/userStore';

export const start = () => {
  noop();
  return userStore;
};
",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolved: Vec<Option<&str>> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("import")
                && node.label.contains("@/")
        })
        .map(|node| node.metadata.get("resolved_path").map(String::as_str))
        .collect();

    assert_eq!(resolved.len(), 2, "{resolved:?}");
    assert!(
        resolved.contains(&Some("resources/js/utils.ts")),
        "{resolved:?}"
    );
    assert!(
        resolved.contains(&Some("resources/js/stores/userStore.ts")),
        "{resolved:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn laravel_states_its_routes_in_a_file_of_route_calls() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("routes")).unwrap();
    fs::create_dir_all(root.join("app").join("Http").join("Controllers")).unwrap();
    fs::write(
        root.join("routes").join("api.php"),
        r#"<?php

use App\Http\Controllers\AlbumController;
use App\Http\Controllers\PingController;

Route::prefix('api')
    ->middleware('auth')
    ->group(static function (): void {
        Route::get('ping', PingController::class);
        Route::put('albums/{album}/rename', [AlbumController::class, 'rename']);
        Route::apiResource('albums', AlbumController::class)
            ->except('destroy');
    });
"#,
    )
    .unwrap();
    fs::write(
        root.join("app")
            .join("Http")
            .join("Controllers")
            .join("AlbumController.php"),
        r#"<?php

namespace App\Http\Controllers;

class AlbumController
{
    public function index()
    {
        return [];
    }

    public function rename()
    {
        return [];
    }
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("app")
            .join("Http")
            .join("Controllers")
            .join("PingController.php"),
        r#"<?php

namespace App\Http\Controllers;

class PingController
{
    public function __invoke()
    {
        return 'pong';
    }
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes: Vec<(String, String, Option<String>)> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
        })
        .map(|node| {
            (
                node.metadata.get("method").cloned().unwrap_or_default(),
                node.metadata.get("path").cloned().unwrap_or_default(),
                node.metadata.get("handler").cloned(),
            )
        })
        .collect();

    // The group hands its prefix to everything it holds.
    assert!(
        routes.contains(&(
            "GET".to_string(),
            "/api/ping".to_string(),
            Some("__invoke".to_string())
        )),
        "{routes:?}"
    );
    assert!(
        routes.contains(&(
            "PUT".to_string(),
            "/api/albums/{album}/rename".to_string(),
            Some("rename".to_string())
        )),
        "{routes:?}"
    );
    // `apiResource` declares the set, and `->except('destroy')` takes one
    // of them back.
    assert!(
        routes.contains(&(
            "GET".to_string(),
            "/api/albums".to_string(),
            Some("index".to_string())
        )),
        "{routes:?}"
    );
    assert!(
        !routes
            .iter()
            .any(|(method, path, _)| method == "DELETE" && path == "/api/albums/{album}"),
        "{routes:?}"
    );

    // And the controller the route names settles which method it means.
    let renamed = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "rename")
        .expect("the controller method is in the graph");
    assert!(
        graph.edges.iter().any(|edge| {
            edge.target == renamed.id
                && edge.metadata.get("resolution").map(String::as_str)
                    == Some("framework_route_handler")
        }),
        "the route reaches the method it names"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_route_handler_the_file_imports_belongs_to_the_package() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    // django-oscar's sandbox mounts Django's own sitemap views, and the
    // project was reported as failing to declare `index` and `sitemap`.
    fs::write(
        root.join("urls.py"),
        "from django.contrib.sitemaps import views\nfrom django.urls import path\n\nfrom . import shop\n\nurlpatterns = [\n    path('sitemap.xml', views.index),\n    path('catalogue/', shop.catalogue),\n]\n",
    )
    .unwrap();
    fs::write(
        root.join("shop.py"),
        "def catalogue(request):\n    return request\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let scope_of = |path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
                    && node.metadata.get("path").map(String::as_str) == Some(path)
            })
            .map(|node| {
                (
                    node.metadata.get("handler").cloned(),
                    node.metadata.get("handler_scope").cloned(),
                )
            })
    };

    assert_eq!(
        scope_of("/sitemap.xml"),
        Some((Some("index".to_string()), Some("external".to_string())))
    );
    // A handler the project does declare says nothing of the kind.
    assert_eq!(
        scope_of("/catalogue/"),
        Some((Some("catalogue".to_string()), None))
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_call_into_the_global_namespace_is_not_a_member() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // spdlog calls the POSIX `::open` from `os::fopen_s`, and the graph
    // answered with its own `file_helper::open`.
    fs::write(
        root.join("src").join("file_helper.h"),
        "#pragma once\n\nclass file_helper {\npublic:\n    void open(const char *name);\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("os.cpp"),
        "#include \"file_helper.h\"\n#include <fcntl.h>\n\nvoid file_helper::open(const char *name) {\n    (void)name;\n}\n\nint write_to(const char *name) {\n    return ::open(name, 0);\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let member = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "open")
        .expect("the member definition is named after the method");
    assert_eq!(
        member.metadata.get("owner_type").map(String::as_str),
        Some("file_helper")
    );
    // And the global call does not reach it.
    assert!(
        !graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.target == member.id
                && edge.metadata.get("call_label").map(String::as_str) == Some("::open")
        }),
        "a class member cannot answer a call into the global namespace"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_include_reaches_the_directory_the_build_puts_on_its_path() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("windows").join("runner")).unwrap();
    fs::create_dir_all(root.join("windows").join("flutter")).unwrap();
    // CMake puts `windows/` on the include path, so the runner's include
    // of `flutter/generated_plugin_registrant.h` reaches the header one
    // directory out from the file that includes it.
    fs::write(
        root.join("windows")
            .join("flutter")
            .join("generated_plugin_registrant.h"),
        "#pragma once\nvoid RegisterPlugins(void);\n",
    )
    .unwrap();
    fs::write(
        root.join("windows").join("runner").join("flutter_window.cpp"),
        "#include \"flutter/generated_plugin_registrant.h\"\n\nint main(void) {\n  RegisterPlugins();\n  return 0;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let include = graph
        .nodes
        .iter()
        .find(|node| {
            node.label.contains("generated_plugin_registrant.h")
                && node.label.starts_with("#include")
        })
        .expect("the include is in the graph");
    assert_eq!(
        include.metadata.get("resolved_path").map(String::as_str),
        Some("windows/flutter/generated_plugin_registrant.h"),
        "{:?}",
        include.metadata
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_import_of_a_file_the_build_writes_is_not_a_dead_link() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // redis lists `src/release.h` in its own `.gitignore` because a script
    // writes it before every build.
    fs::write(
        root.join(".gitignore"),
        "# build products\nsrc/release.h\n*.o\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("version.c"),
        "#include \"release.h\"\n#include \"missing.h\"\n\nint version(void) {\n  return 1;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let built = graph
        .nodes
        .iter()
        .find(|node| node.label.contains("release.h"))
        .expect("the include is in the graph");
    assert_eq!(
        built
            .metadata
            .get("target_is_a_build_product")
            .map(String::as_str),
        Some("true"),
        "{:?}",
        built.metadata
    );
    // A header nothing generates says nothing of the kind.
    let missing = graph
        .nodes
        .iter()
        .find(|node| node.label.contains("missing.h"))
        .expect("the second include is in the graph");
    assert!(
        !missing.metadata.contains_key("target_is_a_build_product"),
        "{:?}",
        missing.metadata
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_type_only_import_closes_no_cycle() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    // vue and zod both write rings like this: the value flows one way and
    // only the type comes back.
    fs::write(
        root.join("src").join("ast.ts"),
        "import type { PropsExpression } from './transform';\n\nexport interface Node {\n  props: PropsExpression;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("transform.ts"),
        "import { Node } from './ast';\n\nexport type PropsExpression = string;\n\nexport function transform(node: Node) {\n  return node;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let import = graph
        .nodes
        .iter()
        .find(|node| node.label.starts_with("import type { PropsExpression }"))
        .expect("the type import is in the graph");
    assert_eq!(
        import.metadata.get("type_only").map(String::as_str),
        Some("true"),
        "{:?}",
        import.metadata
    );

    // And the edge it resolves to carries the same, which is what keeps
    // it out of cycle detection.
    let erased = graph.edges.iter().any(|edge| {
        edge.source == import.id
            && edge.metadata.get("type_only").map(String::as_str) == Some("true")
    });
    assert!(erased, "the resolved reference is erased too");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_import_written_for_the_test_build_says_so() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("testutil.rs"),
        "pub fn matcher() -> u8 {\n    1\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("lib.rs"),
        "mod testutil;\n\npub fn search() -> u8 {\n    2\n}\n\n#[cfg(test)]\nmod tests {\n    use crate::testutil::matcher;\n\n    #[test]\n    fn works() {\n        assert_eq!(matcher(), 1);\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    // ripgrep resolves `use crate::testutil::..` through the module path
    // rather than a relative one, and the marker used to be set only for
    // the imports the scan placed straight away.
    let import = graph
        .nodes
        .iter()
        .find(|node| node.label.contains("use crate::testutil::matcher"))
        .expect("the test module's import is in the graph");
    assert_eq!(
        import.metadata.get("test_context").map(String::as_str),
        Some("true"),
        "{:?}",
        import.metadata
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_solidity_function_says_who_may_call_it() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Token.sol"),
        r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Token {
    function transfer(address to, uint256 value) public returns (bool) {
        return _update(to, value);
    }

    function balanceOf(address holder) external view returns (uint256) {
        return 0;
    }

    function _update(address to, uint256 value) internal returns (bool) {
        return true;
    }

    function _seed() private pure returns (uint256) {
        return 1;
    }
}
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let visibility_of = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("visibility").cloned())
    };

    // The ABI is what `public` and `external` put outside.
    assert_eq!(visibility_of("transfer"), Some("public".to_string()));
    assert_eq!(visibility_of("balanceOf"), Some("public".to_string()));
    // `internal` reaches derived contracts, the way `protected` does.
    assert_eq!(visibility_of("_update"), Some("protected".to_string()));
    assert_eq!(visibility_of("_seed"), Some("private".to_string()));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn objective_c_calls_the_frameworks_by_name() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Client.m"),
        r#"#import <Foundation/Foundation.h>

@implementation AFClient

- (void)start {
    NSURL *url = [NSURL URLWithString:@"https://example.com"];
    dispatch_async(dispatch_get_main_queue(), ^{
        NSLog(@"%@", NSStringFromClass([self class]));
    });
    [self send:url];
}

- (void)send:(NSURL *)url {
}

@end
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution_of = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && graph
                        .nodes
                        .iter()
                        .any(|node| node.id == edge.target && node.label == label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };

    // A message to a Foundation class, a free function of one of the
    // frameworks, and what every object answers.
    assert_eq!(resolution_of("URLWithString:"), Some("builtin".to_string()));
    assert_eq!(resolution_of("dispatch_async"), Some("builtin".to_string()));
    assert_eq!(resolution_of("class"), Some("builtin".to_string()));
    // The project's own method still wins.
    assert_eq!(resolution_of("send:"), Some("resolved".to_string()));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_restructured_text_document_states_its_sections() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("setup.py"), "from setuptools import setup\n").unwrap();
    fs::write(
        root.join("docs").join("guide.rst"),
        "=========\nOverview\n=========\n\nSome prose.\n\nInstalling\n----------\n\nRun ``setup.py`` to install.\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let sections = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("document_section")
        })
        .map(|node| node.label.clone())
        .collect::<Vec<_>>();

    // A section is a line with a rule under it, and a rule above as well is
    // the same section rather than another.
    assert!(
        sections.contains(&"docs/guide.rst#Overview".to_string()),
        "{sections:?}"
    );
    assert!(
        sections.contains(&"docs/guide.rst#Installing".to_string()),
        "{sections:?}"
    );
    assert_eq!(sections.len(), 2, "{sections:?}");

    // A path in double backticks is a mention of the file it names.
    assert!(
        graph.edges.iter().any(|edge| {
            edge.metadata.get("relation").map(String::as_str) == Some("rst_literal_path")
        }),
        "the guide mentions setup.py"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_notebook_holds_the_program_someone_wrote_in_cells() {
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    // Jupyter writes one source line per line of JSON, which is what lets
    // a fact point at the line of the notebook that holds it.
    fs::write(
        root.join("analysis.ipynb"),
        "{\n \"cells\": [\n  {\n   \"cell_type\": \"markdown\",\n   \"source\": [\n    \"# Analysis\\n\"\n   ]\n  },\n  {\n   \"cell_type\": \"code\",\n   \"source\": [\n    \"import os\\n\",\n    \"\\n\",\n    \"DATA = os.environ[\\\"DATA_PATH\\\"]\\n\"\n   ]\n  },\n  {\n   \"cell_type\": \"code\",\n   \"source\": [\n    \"def load_frame(path):\\n\",\n    \"    return path\\n\"\n   ]\n  }\n ],\n \"nbformat\": 4\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let node = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.label == label)
            .unwrap_or_else(|| {
                panic!(
                    "missing {label}: {:?}",
                    graph
                        .nodes
                        .iter()
                        .map(|node| node.label.as_str())
                        .collect::<Vec<_>>()
                )
            })
    };

    // The markdown cell is prose, and the code cells are the program.
    let function = node("load_frame");
    assert_eq!(function.kind, NodeKind::Function);
    assert_eq!(
        function.span.as_ref().map(|span| span.start_line),
        Some(20),
        "a fact points at the line of the notebook that holds it"
    );
    assert_eq!(node("DATA_PATH").kind, NodeKind::Environment);
    assert!(
        graph
            .nodes
            .iter()
            .any(|node| node.label.contains("import os")),
        "what a notebook imports is what its program imports"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rails_states_its_routes_in_a_file_of_its_own() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::write(
        root.join("config").join("routes.rb"),
        "Rails.application.routes.draw do\n  root to: \"home#index\"\n  get \"/health\", to: \"health#show\"\n  resources :users\n  namespace :admin do\n    get \"dashboard\", to: \"dashboard#index\"\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
        })
        .map(|node| node.label.clone())
        .collect::<Vec<_>>();

    // `resources :users` is seven routes, and Rails writes both PATCH and
    // PUT for the update.
    for expected in [
        "route GET /users",
        "route POST /users",
        "route GET /users/new",
        "route GET /users/:id",
        "route GET /users/:id/edit",
        "route PATCH /users/:id",
        "route PUT /users/:id",
        "route DELETE /users/:id",
    ] {
        assert!(routes.contains(&expected.to_string()), "{routes:?}");
    }
    // `root to:` is the one route with no path written, and a namespace
    // puts everything inside it under its own.
    assert!(routes.contains(&"route GET /".to_string()), "{routes:?}");
    assert!(
        routes.contains(&"route GET /admin/dashboard".to_string()),
        "{routes:?}"
    );
    // The action a route points at is what serves it.
    assert!(
        graph.nodes.iter().any(|node| {
            node.label == "route GET /health"
                && node.metadata.get("handler").map(String::as_str) == Some("show")
        }),
        "{routes:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn aspnet_fills_in_the_route_template_it_writes() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("Controllers")).unwrap();
    fs::write(
        root.join("Controllers").join("OrderController.cs"),
        "using Microsoft.AspNetCore.Mvc;\n\nnamespace Web.Controllers;\n\n[Authorize]\n[Route(\"[controller]/[action]\")]\npublic class OrderController : Controller\n{\n    [HttpGet]\n    public async Task<IActionResult> MyOrders()\n    {\n        return View();\n    }\n\n    [HttpGet(\"{orderId}\")]\n    public async Task<IActionResult> Detail(int orderId)\n    {\n        return View();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
        })
        .map(|node| node.label.clone())
        .collect::<Vec<_>>();

    // `[controller]` is the class's own name without the suffix ASP.NET
    // strips, and `[action]` is the method's: without filling them in, both
    // actions serve one written path.
    assert!(
        routes.contains(&"route GET /Order/MyOrders".to_string()),
        "{routes:?}"
    );
    assert!(
        routes.contains(&"route GET /Order/Detail/{orderId}".to_string()),
        "{routes:?}"
    );
    assert_eq!(routes.len(), 2, "{routes:?}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_properties_file_states_settings_and_reads_them() {
    let root = temp_project_root();
    fs::create_dir_all(
        root.join("src")
            .join("main")
            .join("resources")
            .join("messages"),
    )
    .unwrap();
    fs::write(
        root.join("src").join("main").join("resources").join("application.properties"),
        "# database\ndatabase=h2\nspring.sql.init.schema-locations=classpath*:db/${database}/schema.sql\nspring.datasource.url=${MYSQL_URL:jdbc:mysql://localhost/petclinic}\n",
    )
    .unwrap();
    // A resource bundle holds a program's words rather than its settings.
    fs::write(
        root.join("src")
            .join("main")
            .join("resources")
            .join("messages")
            .join("messages_de.properties"),
        "welcome=Willkommen\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let label_of = |id: NodeId| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.label.clone())
            .unwrap_or_default()
    };
    let declared = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Defines
                && edge.metadata.get("source").map(String::as_str) == Some("properties")
        })
        .map(|edge| label_of(edge.target))
        .collect::<Vec<_>>();
    assert!(declared.contains(&"database".to_string()), "{declared:?}");
    assert!(
        declared.contains(&"spring.datasource.url".to_string()),
        "{declared:?}"
    );
    assert!(!declared.contains(&"welcome".to_string()), "{declared:?}");

    // `${database}` in a value is this file reading another setting, and a
    // default after the colon is not part of the name.
    let read = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::ReadsConfig
                && edge.metadata.get("source").map(String::as_str) == Some("properties")
        })
        .map(|edge| label_of(edge.target))
        .collect::<Vec<_>>();
    assert!(read.contains(&"database".to_string()), "{read:?}");
    assert!(read.contains(&"MYSQL_URL".to_string()), "{read:?}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn spring_states_its_routes_above_the_method_that_serves_them() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("OwnerController.java"),
        "package clinic;\n\nimport org.springframework.web.bind.annotation.GetMapping;\n\n@Controller\n@RequestMapping(\"/owners/{ownerId}\")\nclass OwnerController {\n\n\t@GetMapping(\"/pets/new\")\n\tpublic String initCreationForm(Owner owner) {\n\t\treturn \"pets/createOrUpdatePetForm\";\n\t}\n\n\t@PostMapping(\"/pets/new\")\n\tpublic String processCreationForm(Pet pet) {\n\t\treturn \"redirect:/owners/{ownerId}\";\n\t}\n\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("VetController.java"),
        "package clinic;\n\nimport org.springframework.web.bind.annotation.GetMapping;\n\n@Controller\nclass VetController {\n\n\t@GetMapping({ \"/vets\" })\n\tpublic String showVetList(int page) {\n\t\treturn \"vets/vetList\";\n\t}\n\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
        })
        .map(|node| {
            (
                node.label.clone(),
                node.metadata.get("handler").cloned().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();

    // The class states where its methods live, and the method its own path.
    assert!(
        routes.contains(&(
            "route GET /owners/{ownerId}/pets/new".to_string(),
            "initCreationForm".to_string()
        )),
        "{routes:?}"
    );
    assert!(
        routes.contains(&(
            "route POST /owners/{ownerId}/pets/new".to_string(),
            "processCreationForm".to_string()
        )),
        "{routes:?}"
    );
    // A mapping can state its path as a list of one.
    assert!(
        routes.contains(&("route GET /vets".to_string(), "showVetList".to_string())),
        "{routes:?}"
    );
    // The class annotation is where its methods live, not a route of its own.
    assert_eq!(routes.len(), 3, "{routes:?}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn django_states_its_routes_in_a_urlconf() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("shop")).unwrap();
    fs::write(
        root.join("shop").join("urls.py"),
        "from django.urls import path, re_path, include\nfrom . import views\n\nurlpatterns = [\n    path(\"health/\", views.HealthView.as_view(), name=\"health\"),\n    re_path(\n        r\"^orders/(?P<pk>\\d+)/$\",\n        views.OrderView.as_view(),\n        name=\"order\",\n    ),\n    path(\"basket/\", include(\"shop.basket.urls\")),\n]\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let routes = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata.get("item_kind").map(String::as_str) == Some("framework_route")
        })
        .map(|node| {
            (
                node.label.clone(),
                node.metadata.get("framework").cloned().unwrap_or_default(),
                node.metadata.get("handler").cloned(),
            )
        })
        .collect::<Vec<_>>();

    assert!(
        routes.iter().any(|(label, framework, handler)| {
            label == "route ROUTE /health/"
                && framework == "django"
                && handler.as_deref() == Some("HealthView")
        }),
        "{routes:?}"
    );
    // A `re_path` writes its pattern on a line of its own, and the pattern
    // is a regular expression rather than a path.
    assert!(
        routes.iter().any(|(label, _, handler)| {
            label.contains("^orders/(?P<pk>") && handler.as_deref() == Some("OrderView")
        }),
        "{routes:?}"
    );
    // `include(..)` names another URLconf, not a view.
    assert!(
        routes
            .iter()
            .any(|(label, _, handler)| label.contains("/basket/") && handler.is_none()),
        "{routes:?}"
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

/// A test's project directory, removed when the test ends.
///
/// Twenty-five tests built one and never removed it, and a test that panics
/// never reaches its own cleanup line either; one run leaked 31 directories
/// and 21437 of them had collected in the system temp directory. A root that
/// cleans up after itself cannot be forgotten by the next test written.
struct TempProjectRoot {
    path: PathBuf,
}

impl std::ops::Deref for TempProjectRoot {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for TempProjectRoot {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempProjectRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn temp_project_root() -> TempProjectRoot {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    TempProjectRoot {
        path: std::env::temp_dir().join(format!(
            "codegraph-indexer-test-{}-{nanos}-{id}",
            std::process::id()
        )),
    }
}

#[test]
fn a_test_root_takes_itself_away() {
    // 21437 directories had collected in the system temp directory because 25
    // tests built a root and never removed it, and a test that panics never
    // reaches its own cleanup line. The root removes itself instead, on the
    // way out of the test either way.
    let path = {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        assert!(root.exists());
        root.to_path_buf()
    };
    assert!(
        !path.exists(),
        "the root is gone once the test lets go of it"
    );

    let panicked = std::panic::catch_unwind(|| {
        let root = temp_project_root();
        fs::create_dir_all(&root).unwrap();
        panic!("{}", root.display());
    });
    let message = panicked.expect_err("the closure panics");
    let path = PathBuf::from(
        message
            .downcast_ref::<String>()
            .expect("the panic carries the path"),
    );
    assert!(!path.exists(), "a panicking test leaves no root behind");
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

#[test]
fn reopened_namespaces_share_one_node_but_distinct_modules_do_not() {
    // C#/PHP/Ruby namespace declarations reopen one entity: every declaring
    // file must point at the same node instead of minting a look-alike (a
    // real C# repository had 335 nodes for one namespace). Rust `mod` blocks
    // are distinct modules per site and must stay separate.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("a.cs"),
        "namespace Shared.Core { class A { } }\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("b.cs"),
        "namespace Shared.Core { class B { } }\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("one.rs"),
        "mod helpers { pub fn one() {} }\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("two.rs"),
        "mod helpers { pub fn two() {} }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let namespaces: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Module && node.label == "Shared.Core")
        .collect();
    assert_eq!(
        namespaces.len(),
        1,
        "both C# files share one namespace node: {namespaces:?}"
    );
    let namespace_id = namespaces[0].id;
    // A file contains what its own lines hold, so the file the shared node's
    // span points at contains it and the other declares it: saying both
    // contain it put one file's span inside another, 1020 times in koel.
    let holds = graph
        .edges
        .iter()
        .filter(|edge| edge.target == namespace_id && edge.kind == EdgeKind::Contains)
        .count();
    assert_eq!(holds, 1, "one file's lines hold the declaration");
    let declares = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == namespace_id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "declares_namespace")
        })
        .count();
    assert_eq!(declares, 1, "and the file that reopens it declares it");

    let rust_modules = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Module && node.label == "helpers")
        .count();
    assert_eq!(rust_modules, 2, "Rust mod blocks stay distinct per file");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_canceled_scan_aborts_instead_of_finishing() {
    // Cancellation must reach the scanner: previously "cancel" only relabeled
    // the job while the scan ran to completion, holding its concurrency slot.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    for index in 0..8 {
        fs::write(
            root.join("src").join(format!("m{index}.rs")),
            format!("fn f{index}() {{}}\n"),
        )
        .unwrap();
    }

    let cancel = ScanCancellation::new();
    cancel.cancel();
    let error = scan_project_cancelable(&root, &IndexOptions::default(), &cancel)
        .expect_err("a tripped token aborts the scan");
    assert!(
        matches!(error, IndexError::Canceled),
        "unexpected error: {error}"
    );

    // An untouched token scans normally.
    let graph = scan_project_cancelable(&root, &IndexOptions::default(), &ScanCancellation::none())
        .expect("uncanceled scan succeeds");
    assert!(graph.nodes.len() > 8);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn qualified_calls_resolve_to_the_named_types_method() {
    // `Alpha::make` used to match every bare `make` declaration equally, so an
    // ambiguous set was reported instead of one edge. The method's owner_type
    // disambiguates it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("alpha.rs"),
        "pub struct Alpha;\nimpl Alpha {\n    pub fn make() -> Alpha { Alpha }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("beta.rs"),
        "pub struct Beta;\nimpl Beta {\n    pub fn make() -> Beta { Beta }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() { let _ = Alpha::make(); }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let alpha_make = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && node.label == "make"
                && node.metadata.get("owner_type").map(String::as_str) == Some("Alpha")
        })
        .expect("Alpha::make is recorded with its owner type");
    let beta_make = graph
        .nodes
        .iter()
        .find(|node| {
            node.kind == NodeKind::Function
                && node.label == "make"
                && node.metadata.get("owner_type").map(String::as_str) == Some("Beta")
        })
        .expect("Beta::make is recorded with its owner type");

    let calls: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls && edge.target == alpha_make.id)
        .collect();
    assert!(
        !calls.is_empty(),
        "the qualified call resolves to Alpha's method"
    );
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == beta_make.id),
        "and not to the same-named method on Beta"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn module_level_calls_belong_to_the_file_but_callback_bodies_do_not() {
    // Registration calls, initialisers and `if __name__ == "__main__"` run
    // when the file loads, yet they used to be dropped for having no
    // enclosing definition. A call inside an unnamed callback is different:
    // it runs when something invokes the callback, so it stays out.
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("main.py"),
        "def start():\n    return 1\n\n\nif __name__ == \"__main__\":\n    start()\n",
    )
    .unwrap();
    fs::write(
        root.join("app.js"),
        "function helper() { return 1; }\nfunction run(cb) { return cb(); }\nconst value = helper();\nrun(() => { helper(); });\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let file_id = |name: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::File && node.label == name)
            .unwrap_or_else(|| panic!("{name} is indexed"))
            .id
    };
    let called_labels = |source: NodeId| -> Vec<String> {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Calls && edge.source == source)
            .filter_map(|edge| edge.metadata.get("call_label").cloned())
            .collect()
    };

    let python = called_labels(file_id("main.py"));
    assert!(
        python.contains(&"start".to_string()),
        "the guarded entry call is attributed to the file, got {python:?}"
    );

    let js = called_labels(file_id("app.js"));
    assert!(
        js.contains(&"run".to_string()),
        "a load-time call is attributed to the file, got {js:?}"
    );
    // `const value = helper()` declares a value the module exports, and the
    // call that builds it belongs to that declaration -- which the file
    // contains, so the chain still runs from the file.
    let value_calls = called_labels(function_id_in_file(&graph, "value", "app.js"));
    assert!(
        value_calls.contains(&"helper".to_string()),
        "the call that builds a value belongs to it, got {value_calls:?}"
    );

    // `helper` inside the arrow reaches the graph only through the
    // module-level `const value = helper()`, so exactly one edge exists and
    // the callback adds nothing of its own.
    let run_calls: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("cb")
        })
        .collect();
    assert_eq!(
        run_calls.len(),
        1,
        "the call inside `run` belongs to `run` itself"
    );
    let run_id = function_id_in_file(&graph, "run", "app.js");
    assert_eq!(
        run_calls[0].source, run_id,
        "a named function keeps its own calls"
    );
}

#[test]
fn from_imported_names_say_where_a_bare_call_comes_from() {
    // `OrderedDict()` carries no qualifier for the import map to match, so
    // a standard-library call looked like a resolver failure. The names a
    // `from module import ...` binds answer it: outside the repository the
    // call is external, inside it the module narrows the candidates, and a
    // definition the file makes itself still wins.
    let root = temp_project_root();
    fs::create_dir_all(root.join("pkg")).unwrap();
    fs::write(root.join("pkg").join("__init__.py"), "").unwrap();
    fs::write(
        root.join("pkg").join("helpers.py"),
        "def build():\n    return 1\n",
    )
    .unwrap();
    fs::write(
        root.join("pkg").join("other.py"),
        "def build():\n    return 2\n",
    )
    .unwrap();
    fs::write(
        root.join("pkg").join("app.py"),
        "from collections import OrderedDict\nfrom .helpers import build\n\n\ndef run():\n    store = OrderedDict()\n    return build(), store\n",
    )
    .unwrap();
    fs::write(
        root.join("pkg").join("shadow.py"),
        "from .other import build\n\n\ndef build():\n    return 3\n\n\ndef run():\n    return build()\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = |path: &str, label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
                    && graph
                        .nodes
                        .iter()
                        .find(|node| node.id == edge.source)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| span.path.ends_with(path))
            })
            .unwrap_or_else(|| panic!("{path} calls {label}"))
    };

    assert_eq!(
        call("app.py", "OrderedDict").metadata.get("resolution"),
        Some(&"external".to_string()),
        "a name imported from outside the repository is not a resolver miss"
    );

    let build_from_app = call("app.py", "build");
    assert_eq!(
        build_from_app.metadata.get("resolution"),
        Some(&"resolved".to_string()),
        "the import names which of the two `build`s is meant"
    );
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == build_from_app.target)
        .expect("the call has a target");
    assert!(
        target
            .span
            .as_ref()
            .is_some_and(|span| span.path.ends_with("helpers.py")),
        "the imported module decides, got {:?}",
        target.span
    );

    let shadowed = call("shadow.py", "build");
    let shadow_target = graph
        .nodes
        .iter()
        .find(|node| node.id == shadowed.target)
        .expect("the shadowed call has a target");
    assert!(
        shadow_target
            .span
            .as_ref()
            .is_some_and(|span| span.path.ends_with("shadow.py")),
        "the file's own definition wins over the import, got {:?}",
        shadow_target.span
    );
}

#[test]
fn javascript_imports_name_which_module_a_call_means() {
    // `import { build } from './lib/helpers.js'` says exactly which of two
    // same-named exports is meant, `import * as helpers` qualifies the
    // rest, and a package the repository does not contain is external —
    // while a workspace package it does contain is not.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("lib")).unwrap();
    fs::create_dir_all(root.join("packages").join("shared")).unwrap();
    fs::write(
        root.join("packages").join("shared").join("package.json"),
        "{\n  \"name\": \"@acme/shared\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("packages").join("shared").join("index.js"),
        "export function extend(target) { return target; }\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("lib").join("helpers.js"),
        "export function build() { return 1; }\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("lib").join("other.js"),
        "export function build() { return 2; }\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("main.js"),
        "import { build } from './lib/helpers.js';\nimport * as other from './lib/other.js';\nimport { extend } from '@acme/shared';\nimport { createServer } from 'node:http';\n\nexport function run() {\n  return build() + other.build() + extend({}) + createServer();\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .unwrap_or_else(|| panic!("the call to {label} is recorded"))
    };
    let target_path = |label: &str| {
        let edge = call(label);
        graph
            .nodes
            .iter()
            .find(|node| node.id == edge.target)
            .and_then(|node| node.span.as_ref())
            .map(|span| span.path.clone())
            .unwrap_or_default()
    };

    assert!(
        target_path("build").ends_with("helpers.js"),
        "the named import picks helpers.js, got {}",
        target_path("build")
    );
    assert!(
        target_path("other.build").ends_with("other.js"),
        "the namespace import picks other.js, got {}",
        target_path("other.build")
    );
    assert!(
        target_path("extend").ends_with("index.js"),
        "a workspace package stays inside the repository, got {}",
        target_path("extend")
    );
    assert_eq!(
        call("createServer").metadata.get("resolution"),
        Some(&"external".to_string()),
        "a package the repository does not contain is external"
    );
}

#[test]
fn a_projects_own_definition_wins_over_the_builtin_list() {
    // Naming `type` as a Lua builtin must not hide a project that defines
    // its own. Builtins are only consulted once matching by name has found
    // nothing, so the local definition still takes the call.
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("mod.lua"),
        "local function type(value)\n    return value\n end\n\nlocal function run()\n    return type(1) .. tostring(2)\nend\n\nreturn run\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution"))
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(
        resolution("type"),
        "resolved",
        "the file's own `type` takes the call"
    );
    assert_eq!(
        resolution("tostring"),
        "builtin",
        "a name only the language provides is not a resolver miss"
    );
}

#[test]
fn a_call_naming_several_types_records_the_candidates() {
    // One type of that name gives a constructor reference. Several used to
    // give nothing at all, so the call read as "found nothing" when in
    // fact more than one declaration answered to the name.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("alpha.scala"),
        "package alpha\n\ncase class Session(id: String)\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("beta.scala"),
        "package beta\n\ncase class Session(token: String)\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("only.scala"),
        "package only\n\ncase class Ticket(id: String)\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.scala"),
        "package main\n\nobject Main {\n  def run(): Unit = {\n    val a = Session(\"x\")\n    val b = Ticket(\"y\")\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let placeholder = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::ExternalDependency && node.label == "Session")
        .expect("the ambiguous call is recorded");
    assert_eq!(
        placeholder.metadata.get("resolution"),
        Some(&"ambiguous".to_string())
    );
    assert_eq!(
        placeholder.metadata.get("candidate_kind"),
        Some(&"type".to_string()),
        "the candidates are types being constructed"
    );
    assert_eq!(
        placeholder.metadata.get("candidate_count"),
        Some(&"2".to_string())
    );

    // The unambiguous one still becomes a direct reference to its type,
    // carrying where it was written — the file beside the line, as a call
    // edge carries them.
    let reference = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::References
                && edge.metadata.get("relation").map(String::as_str)
                    == Some("constructor_reference")
                && edge.metadata.get("type_label").map(String::as_str) == Some("Ticket")
        })
        .expect("a single matching type still gives a constructor reference");
    assert!(
        reference.metadata.contains_key("file") && reference.metadata.contains_key("line"),
        "the reference must say where it was written: {:?}",
        reference.metadata
    );
}

#[test]
fn parse_facts_from_another_build_are_not_reused() {
    // A file's stamp does not move when the extraction rules do, so a
    // record written by an earlier build would otherwise be served as
    // though this build had produced it — the scan would answer with facts
    // it no longer extracts.
    let root = temp_project_root();
    let cache_dir = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    let source_path = root.join("src").join("main.rs");
    fs::write(&source_path, "fn main() {}\n").unwrap();
    let options = IndexOptions::default().with_parse_cache_dir(cache_dir.to_path_buf());

    scan_project(&root, &options).unwrap();
    let stamp = file_stamp(&source_path).unwrap();
    assert!(
        load_cached_parse(&cache_dir, "src/main.rs", Language::Rust, stamp).is_some(),
        "this build reuses its own records"
    );

    // Rewrite the stored record as though an earlier build had written it.
    let mut rewritten = 0;
    for entry in fs::read_dir(&cache_dir).unwrap().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let Some(replaced) = replace_build_identity(&text) else {
            continue;
        };
        fs::write(&path, replaced).unwrap();
        rewritten += 1;
    }
    assert_eq!(rewritten, 1, "exactly one parse record was written");

    assert!(
        load_cached_parse(&cache_dir, "src/main.rs", Language::Rust, stamp).is_none(),
        "facts from another build are not this build's facts"
    );

    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(cache_dir).unwrap();
}

/// Swap the `build_identity` of a stored JSON record for a different one,
/// leaving everything else untouched.
fn replace_build_identity(text: &str) -> Option<String> {
    let key = "\"build_identity\":\"";
    let start = text.find(key)? + key.len();
    let end = start + text[start..].find('"')?;
    Some(format!("{}0.0.0-other{}", &text[..start], &text[end..]))
}

#[test]
fn a_route_handler_is_not_every_function_of_that_name() {
    // A decorator sits above the function it registers, so the handler is
    // in the route's own file. When it is not — a route written inside a
    // docstring, say — linking to every same-named function invented the
    // links wholesale: one `@app.route` in flask claimed about 140
    // different `index` functions as its handler.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app")).unwrap();
    fs::write(
        root.join("app").join("main.py"),
        "from flask import Flask\n\napp = Flask(__name__)\n\n\n@app.route(\"/here\")\ndef index():\n    return \"here\"\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("other.py"),
        "def index():\n    return \"other\"\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("third.py"),
        "def index():\n    return \"third\"\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("docs.py"),
        "def helper():\n    \"\"\"Example:\n\n    @app.route(\"/example\")\n    def index():\n        return \"x\"\n    \"\"\"\n    return 1\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let handler_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.metadata.get("resolution").map(String::as_str) == Some("framework_route_handler")
        })
        .collect();

    // The route in main.py finds its own file's `index` and nothing else.
    assert_eq!(
        handler_edges.len(),
        1,
        "one route resolves to one handler, got {:?}",
        handler_edges
            .iter()
            .map(|edge| edge.target)
            .collect::<Vec<_>>()
    );
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == handler_edges[0].target)
        .expect("the handler exists");
    assert!(
        target
            .span
            .as_ref()
            .is_some_and(|span| span.path.ends_with("main.py")),
        "the handler is the one beside the route, got {:?}",
        target.span
    );
}

#[test]
fn a_route_handler_is_written_in_the_routes_own_language() {
    // codegraph's own `GET /api/scan` names a Rust function, and the
    // `scan` in its JavaScript bundle answered to the name too — two
    // candidates where the language leaves one.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
    let app = Router::new().route("/api/scan", get(scan));
}
"#,
    )
    .unwrap();
    fs::write(
        root.join("src").join("handlers.rs"),
        "pub async fn scan() -> String {\n    String::new()\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("app.js"),
        "export function scan() {\n  return 1;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let handlers: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.metadata.get("resolution").map(String::as_str) == Some("framework_route_handler")
        })
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .collect();
    assert_eq!(handlers.len(), 1, "{handlers:?}");
    assert_eq!(
        handlers[0].metadata.get("language").map(String::as_str),
        Some("rust"),
        "the JavaScript function of the same name is not a candidate"
    );
}

#[test]
fn a_document_mention_links_only_when_the_name_leaves_no_choice() {
    // Prose carries no scope. A README saying `render` says nothing about
    // which of vue core's 699 functions of that name it means, and linking
    // to all of them claimed the document referenced every one — 76000
    // invented edges across the corpora, 7% of the whole graph.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("alpha.rs"),
        "pub fn render() {}\npub fn unique_helper() {}\n",
    )
    .unwrap();
    fs::write(root.join("src").join("beta.rs"), "pub fn render() {}\n").unwrap();
    fs::write(
        root.join("README.md"),
        "# Guide\n\nCall `render` to draw, and `unique_helper` for the rest.\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let mentions: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.metadata.get("resolution").map(String::as_str) == Some("document_symbol")
        })
        .filter_map(|edge| edge.metadata.get("symbol").map(String::as_str))
        .collect();
    assert_eq!(
        mentions,
        vec!["unique_helper"],
        "only the name with one definition is linked"
    );
}

#[test]
fn a_route_written_in_a_docstring_is_not_a_route() {
    // The route detectors read text, so they cannot tell a route from an
    // example of one. flask documents `@app.route("/")` inside docstrings,
    // and those seven lines became served entrypoints — one of them
    // claiming about 140 functions as its handler, and every file holding
    // one looking reachable from an entrypoint.
    let root = temp_project_root();
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("app.py"),
        "from flask import Flask\n\napp = Flask(__name__)\n\n\ndef documented():\n    \"\"\"Register a handler like this:\n\n    @app.route(\"/example\")\n    def index():\n        return \"x\"\n    \"\"\"\n    return 1\n\n\n@app.route(\"/real\")\ndef real_handler():\n    return \"ok\"\n",
    )
    .unwrap();
    fs::write(
        root.join("server.js"),
        "const app = express();\n\n// app.get(\"/disabled\", handleDisabled);\napp.get(\"/live\", handleLive);\n\nfunction handleLive() { return 1; }\nfunction handleDisabled() { return 2; }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let mut routes: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.metadata.get("entrypoint_kind").map(String::as_str) == Some("route"))
        .filter_map(|node| node.metadata.get("path").map(String::as_str))
        .collect();
    routes.sort_unstable();
    assert_eq!(
        routes,
        vec!["/live", "/real"],
        "only the routes the program actually serves"
    );
}

#[test]
fn code_outside_the_tests_does_not_call_into_them() {
    // flask has exactly one function named `close` — a helper in
    // tests/test_helpers.py — so `builder.close()` in src/flask/app.py
    // resolved to it with full confidence. A unique name is not evidence
    // when the only definition wearing it belongs to the tests, and 1143
    // such links existed across the corpora.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("src").join("app.py"),
        "def run(builder):\n    return builder.close()\n",
    )
    .unwrap();
    fs::write(
        root.join("tests").join("test_helpers.py"),
        "class Wrapper:\n    def close(self):\n        return 1\n\n\ndef exercise(builder):\n    return builder.close()\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call_from = |path: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some("builder.close")
                    && graph
                        .nodes
                        .iter()
                        .find(|node| node.id == edge.source)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| span.path.ends_with(path))
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
            .unwrap_or_default()
    };
    assert_eq!(
        call_from("app.py"),
        "unresolved",
        "the only `close` in the project is a test helper, so the call has no target here"
    );
    assert_eq!(
        call_from("test_helpers.py"),
        "resolved",
        "a test may of course call its own helper"
    );
}

#[test]
fn a_rust_use_says_whose_name_the_call_is_written_through() {
    // `BTreeMap::new` was matched against the 8 functions this repository
    // calls `new` and kept as one bounded ambiguity: 395 of its 464
    // ambiguous calls were a standard library or dependency type, and
    // `PathBuf::from` had picked up a project `from` outright. A rust `use`
    // says which crate the name comes from, and rust's own scoping makes
    // that answer safe -- a file cannot both import `BTreeMap` and declare
    // one. A sibling crate of the same workspace is this project, not a
    // dependency, and must keep resolving by name: ripgrep writes its
    // imports as `use {grep_matcher::LineTerminator, ..};`, where the brace
    // comes first and every part carries its own crate, and reading one
    // root for the whole statement took 7 of its own calls with it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("src")).unwrap();
    fs::create_dir_all(root.join("engine").join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"app\", \"engine\"]\n",
    )
    .unwrap();
    fs::write(
        root.join("engine").join("Cargo.toml"),
        "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("engine").join("src").join("lib.rs"),
        "pub struct Engine;\n\nimpl Engine {\n    pub fn new() -> Self {\n        Engine\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = { path = \"../engine\" }\nwalkdir = \"2\"\n",
    )
    .unwrap();
    fs::write(
        root.join("app").join("src").join("main.rs"),
        "use {engine::Engine, walkdir::WalkDir};\nuse std::collections::BTreeMap;\nuse std::env;\n\npub struct Config;\n\nimpl Config {\n    pub fn build() -> Self {\n        Config\n    }\n}\n\nfn main() {\n    let _: BTreeMap<u8, u8> = BTreeMap::new();\n    let _ = WalkDir::new(\".\");\n    let _ = Engine::new();\n    let _ = Config::build();\n    let _ = env::temp_dir();\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution_of = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };

    assert_eq!(
        resolution_of("BTreeMap::new").as_deref(),
        Some("builtin"),
        "the standard library provides it, and this crate's own `new` is not a candidate"
    );
    assert_eq!(
        resolution_of("WalkDir::new").as_deref(),
        Some("external"),
        "a dependency provides it"
    );
    assert_eq!(
        resolution_of("Engine::new").as_deref(),
        Some("resolved"),
        "a sibling crate of the same workspace is this project"
    );
    assert_eq!(
        resolution_of("Config::build").as_deref(),
        Some("resolved"),
        "and a type declared here keeps resolving to what it declares"
    );
    // `use std::env;` binds a module, and `env::temp_dir` is written
    // through it. Reading only capitalised names left 1194 such calls
    // unresolved on this repository and matched one to a project function
    // of that name inside the very function that calls it, which `doctor`
    // reported as a definition calling itself.
    assert_eq!(
        resolution_of("env::temp_dir").as_deref(),
        Some("builtin"),
        "a lowercase module is imported the same way a type is"
    );
}

#[test]
fn a_call_edge_says_what_settled_it() {
    // Every call edge claimed `heuristic` confidence, so a link the syntax
    // settles read exactly like a name matched across the repository —
    // 411374 edges carrying one constant instead of a fact.
    let root = temp_project_root();
    fs::create_dir_all(root.join("pkg")).unwrap();
    fs::write(root.join("pkg").join("__init__.py"), "").unwrap();
    fs::write(
        root.join("pkg").join("helpers.py"),
        "def build():\n    return 1\n",
    )
    .unwrap();
    fs::write(
        root.join("pkg").join("other.py"),
        "def build():\n    return 2\n\n\ndef lonely():\n    return 3\n",
    )
    .unwrap();
    fs::write(
        root.join("pkg").join("app.py"),
        // The star import is what lets `lonely` be resolved by name at all:
        // a module reaches nothing it does not import, and `import *` is
        // the one form whose bindings cannot be listed.
        "from .helpers import build\nfrom .other import *\n\n\ndef near():\n    return 4\n\n\ndef run():\n    return build() + near() + lonely()\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
                    && graph
                        .nodes
                        .iter()
                        .find(|node| node.id == edge.source)
                        .and_then(|node| node.span.as_ref())
                        .is_some_and(|span| span.path.ends_with("app.py"))
            })
            .unwrap_or_else(|| panic!("the call to {label} is recorded"))
    };

    let by_import = call("build");
    assert_eq!(
        by_import
            .metadata
            .get("resolution_basis")
            .map(String::as_str),
        Some("import"),
        "the import named the module this one comes from"
    );
    assert_eq!(by_import.confidence, Confidence::Syntactic);

    let by_file = call("near");
    assert_eq!(
        by_file.metadata.get("resolution_basis").map(String::as_str),
        Some("same_file")
    );
    assert_eq!(by_file.confidence, Confidence::Syntactic);

    // Nothing in `app.py` says where `lonely` lives; only its name matched.
    let by_name = call("lonely");
    assert_eq!(
        by_name.metadata.get("resolution_basis").map(String::as_str),
        Some("name")
    );
    assert_eq!(by_name.confidence, Confidence::Heuristic);
}

#[test]
fn overloads_of_one_method_are_not_a_choice() {
    // `JsonConvert.SerializeObject` has six signatures, and a caller means
    // the method rather than one of them — 4419 calls across the corpora
    // read as ambiguous when every candidate was the same method of the
    // same type. But a type name is not unique: terraform declares
    // `Diagnostics.HasErrors` in two packages, two different types, and Go
    // has no overloads at all.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src").join("alpha")).unwrap();
    fs::create_dir_all(root.join("src").join("beta")).unwrap();
    fs::write(
        root.join("src").join("alpha").join("writer.cs"),
        "namespace Alpha {\n    public class Writer {\n        public void Write(string value) { }\n        public void Write(int value) { }\n    }\n}\n",
    )
    .unwrap();
    // A different file, and a receiver whose type nothing states, so
    // nothing but the overload rule can settle it.
    fs::write(
        root.join("src").join("alpha").join("user.cs"),
        "namespace Alpha {\n    public class User {\n        public void Run() { var writer = Fetch(); writer.Write(\"x\"); }\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("alpha").join("diagnostics.go"),
        "package alpha\n\ntype Diagnostics struct{}\n\nfunc (d Diagnostics) HasErrors() bool { return false }\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("alpha").join("check.go"),
        "package alpha\n\nfunc Check(d Diagnostics) bool { return d.HasErrors() }\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("beta").join("other.go"),
        "package beta\n\ntype Diagnostics struct{}\n\nfunc (d Diagnostics) HasErrors() bool { return true }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .map(|edge| {
                (
                    edge.metadata.get("resolution").cloned().unwrap_or_default(),
                    edge.metadata
                        .get("resolution_basis")
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default()
    };

    let (write_resolution, write_basis) = resolution("writer.Write");
    assert_eq!(write_resolution, "resolved");
    assert_eq!(write_basis, "overload");
    let written: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("writer.Write")
        })
        .collect();
    // One call site is one call: an edge per signature multiplied every
    // count downstream by the number of overloads. The edge says how many
    // signatures share the name instead.
    assert_eq!(written.len(), 1);
    assert_eq!(
        written[0]
            .metadata
            .get("overload_count")
            .map(String::as_str),
        Some("2")
    );

    // Two packages, two types of one name: not overloads.
    assert_eq!(resolution("d.HasErrors").0, "ambiguous");
}

#[test]
fn an_ocaml_module_call_finds_the_file_that_is_that_module() {
    // OCaml names a module after its file: `Json.assoc` is `assoc` in
    // json.ml. Matching by name alone left 11214 of dune's calls choosing
    // between every `assoc` in the project.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("json.ml"),
        "let assoc pairs = pairs\nlet string value = value\n",
    )
    .unwrap();
    fs::write(root.join("src").join("table.ml"), "let assoc key = key\n").unwrap();
    fs::write(
        root.join("src").join("main.ml"),
        "let run pairs = Json.assoc pairs\nlet nested pairs = Stdune.Json.assoc pairs\nlet other key = Missing.assoc key\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .unwrap_or_else(|| panic!("the call to {label} is recorded"))
    };
    let target_path = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.id == call(label).target)
            .and_then(|node| node.span.as_ref())
            .map(|span| span.path.clone())
            .unwrap_or_default()
    };

    assert!(
        target_path("Json.assoc").ends_with("json.ml"),
        "{}",
        target_path("Json.assoc")
    );
    // The module's own files answer for it, and they are what narrows the
    // call now -- `owner_type` said the receiver's type chose, which was
    // true and less specific: a module answers for its file and for what
    // that file includes, and for nothing else.
    assert_eq!(
        call("Json.assoc")
            .metadata
            .get("resolution_basis")
            .map(String::as_str),
        Some("module_file")
    );
    // A path-qualified module is still that module.
    assert!(target_path("Stdune.Json.assoc").ends_with("json.ml"));
    // A module the project does not define settles nothing -- and saying
    // `ambiguous` claimed several of this project's definitions answer to
    // the name, when not one of them is `Missing`'s. dune wrote 890 such
    // calls and 237 were answered outright by an unrelated file.
    assert_eq!(
        call("Missing.assoc")
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("unresolved")
    );
}

#[test]
fn a_call_through_a_bound_value_is_not_a_resolver_failure() {
    // `runningCtx, done := context.WithCancel(…)` then `defer done()`:
    // there is no definition named `done` to find, and terraform has 1483
    // such calls filed as though the resolver had missed something.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("run.go"),
        "package run\n\nfunc Work() {\n\t_, done := context.WithCancel(context.Background())\n\tdefer done()\n\tmissing()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .unwrap_or_else(|| panic!("the call to {label} is recorded"))
    };

    assert_eq!(
        call("done")
            .metadata
            .get("unresolved_reason")
            .map(String::as_str),
        Some("local_value"),
        "the body binds `done` to a value"
    );
    // A name nothing in the file binds is still a resolver miss.
    assert_eq!(
        call("missing").metadata.get("unresolved_reason"),
        None,
        "nothing binds `missing`, so it stays a plain miss"
    );
}

#[test]
fn an_import_of_the_projects_own_package_is_not_an_outside_dependency() {
    // Vue's `@vue/runtime-test` is one of its own packages and flask's
    // tutorial imports `flaskr.db` from examples/tutorial/flaskr/db.py.
    // Neither is a dependency to declare, yet both were recorded as
    // imports of something outside the repository.
    let root = temp_project_root();
    fs::create_dir_all(root.join("packages").join("runtime-test").join("src")).unwrap();
    fs::create_dir_all(root.join("packages").join("core").join("src")).unwrap();
    fs::create_dir_all(root.join("examples").join("tutorial").join("flaskr")).unwrap();
    fs::write(
        root.join("packages")
            .join("runtime-test")
            .join("package.json"),
        "{\n  \"name\": \"@vue/runtime-test\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("packages")
            .join("runtime-test")
            .join("src")
            .join("index.ts"),
        "export function nodeOps() {}\n",
    )
    .unwrap();
    fs::write(
        root.join("packages").join("core").join("package.json"),
        "{\n  \"name\": \"@vue/core\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("packages")
            .join("core")
            .join("src")
            .join("app.ts"),
        "import { nodeOps } from '@vue/runtime-test';\nimport { readFile } from 'node:fs';\n\nexport function run() {\n  return nodeOps() + readFile('x');\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("examples")
            .join("tutorial")
            .join("flaskr")
            .join("db.py"),
        "def get_db():\n    return 1\n",
    )
    .unwrap();
    fs::write(
        root.join("examples")
            .join("tutorial")
            .join("flaskr")
            .join("blog.py"),
        "from flaskr.db import get_db\n\n\ndef index():\n    return get_db()\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let scope_of = |needle: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.metadata.get("item_kind").map(String::as_str) == Some("import")
                    && node.label.contains(needle)
            })
            .and_then(|node| node.metadata.get("import_scope").cloned())
            .unwrap_or_default()
    };

    assert_eq!(
        scope_of("@vue/runtime-test"),
        "workspace",
        "a package the repository defines is inside it"
    );
    assert_eq!(
        scope_of("flaskr.db"),
        "local",
        "the tutorial's own module is inside the repository too"
    );
    assert_eq!(
        scope_of("node:fs"),
        "",
        "a runtime module is not a package of this repository"
    );
}

#[test]
fn an_include_resolves_along_the_projects_own_header_tree() {
    // redis compiles with `-Ideps/jemalloc/include` from a Makefile, and
    // include directories were only read from CMake and
    // compile_commands.json, so 911 of its includes had nothing to resolve
    // against. The header as written is a candidate too, matched when
    // exactly one file in the repository ends with it.
    let root = temp_project_root();
    fs::create_dir_all(root.join("deps").join("lib").join("include").join("pkg")).unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("deps")
            .join("lib")
            .join("include")
            .join("pkg")
            .join("util.h"),
        "int helper(void);\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.c"),
        "#include \"pkg/util.h\"\n#include \"missing/nowhere.h\"\n\nint main(void) { return helper(); }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let import = |needle: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.metadata.get("item_kind").map(String::as_str) == Some("import")
                    && node.label.contains(needle)
            })
            .unwrap_or_else(|| panic!("the include of {needle} is recorded"))
    };
    assert_eq!(
        import("pkg/util.h")
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("resolved")
    );
    assert!(
        import("pkg/util.h")
            .metadata
            .get("resolved_path")
            .is_some_and(|path| path.ends_with("deps/lib/include/pkg/util.h")),
        "{:?}",
        import("pkg/util.h").metadata.get("resolved_path")
    );
    assert_eq!(
        import("missing/nowhere.h")
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("unresolved"),
        "a header the repository does not contain stays unresolved"
    );
}

#[test]
fn a_lua_call_is_answered_by_the_module_its_file_requires() {
    // `local pl_path = require "pl.path"` is the only place a Lua file says
    // what `pl_path.exists(...)` means, and matching on the tail alone gave
    // kong's `kong/tools/queue.lua` every one of them.
    let root = temp_project_root();
    fs::create_dir_all(root.join("myproj/tools")).unwrap();
    fs::write(
        root.join("init.lua"),
        r#"
local pl_path = require "pl.path"
local tbl = require("myproj.tools.table")

local function run()
  local ok = pl_path.exists("/tmp")
  return tbl.concat({ok})
end

return { run = run }
"#,
    )
    .unwrap();
    fs::write(
        root.join("myproj/tools/table.lua"),
        "local _M = {}\nfunction _M.concat(t)\n  return t\nend\nreturn _M\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call_edge = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .unwrap_or_else(|| panic!("no call edge for {label}"))
    };

    // `pl.path` is a package outside the repository, so nothing here can be
    // what `pl_path.exists` means -- the tail alone used to hand it to any
    // project function named `exists`.
    let external = call_edge("pl_path.exists");
    assert_eq!(
        external.metadata.get("resolution").map(String::as_str),
        Some("external")
    );

    // `tbl` names a module the repository holds, so the call goes there.
    let local = call_edge("tbl.concat");
    assert_eq!(
        local.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == local.target)
        .expect("target node");
    assert_eq!(
        target.span.as_ref().map(|span| span.path.as_str()),
        Some("myproj/tools/table.lua")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_bare_scala_call_stays_with_the_object_it_has() {
    // `f(...)` inside a method is a function the body was handed, not the
    // `def f` some other file wrote on a class of its own. cats declares
    // one on `FlatMapped` in FreeT.scala and 833 calls across the
    // repository read as that one method.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/main/scala")).unwrap();
    fs::write(root.join("build.sbt"), "name := \"app\"\n").unwrap();
    fs::write(
        root.join("src/main/scala/Free.scala"),
        "package app\n\nfinal case class FlatMapped(f0: Int => Int) {\n  def f: Int => Int = f0\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/scala/Chain.scala"),
        "package app\n\nobject Chain {\n  def run(f: Int => Int, value: Int): Int = f(value)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("f")
        })
        .expect("the call is recorded");
    assert_ne!(
        call.metadata.get("resolution").map(String::as_str),
        Some("resolved"),
        "a method of FlatMapped is not reachable from Chain without naming one"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_java_call_is_answered_by_the_class_its_file_imports() {
    // `Arrays.asList(..)` keeps only `asList` in its label, because Java
    // writes the receiver in a field of its own. gson declares an `asList`
    // that answered 77 of the standard library's.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/main/java/app")).unwrap();
    fs::write(root.join("pom.xml"), "<project></project>\n").unwrap();
    fs::write(
        root.join("src/main/java/app/Lists.java"),
        "package app;\n\npublic final class Lists {\n  public static String asList(String value) {\n    return value;\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/java/app/Caller.java"),
        "package app;\n\nimport java.util.Arrays;\n\npublic final class Caller {\n  public Object run() {\n    return Arrays.asList(\"a\", \"b\");\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("asList")
        })
        .expect("the call is recorded");
    assert_eq!(
        call.metadata.get("resolution").map(String::as_str),
        Some("external"),
        "`java.util.Arrays` is not this project's `Lists`"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_chained_call_says_it_went_through_a_value() {
    // `args.into_iter().map(..)` reaches the graph as `map`, because the
    // receiver is not part of what is called -- and a name that lost its
    // receiver then looks exactly like one written bare. ripgrep declares
    // `Match::map` and it collected 101 iterator `map`s.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub struct Match(pub u32);\n\nimpl Match {\n    pub fn map<F: FnOnce(u32) -> u32>(self, f: F) -> Match {\n        Match(f(self.0))\n    }\n}\n\npub fn shout(words: Vec<String>) -> Vec<String> {\n    words.into_iter().map(|word| word.to_uppercase()).collect()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("map")
        })
        .expect("the call is recorded");
    assert_ne!(
        call.metadata.get("resolution").map(String::as_str),
        Some("resolved"),
        "an iterator's `map` is the standard library's, not this crate's"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_nix_let_binding_stays_in_its_own_file() {
    // A nix file's `let` bindings are its own, and the language has no
    // global namespace to reach another file's through. home-manager binds
    // `map` in modules/lib/dag.nix and it answered 132 calls to the
    // primop; `lib` is what the evaluator hands the module, not a name any
    // file here declares.
    let root = temp_project_root();
    fs::create_dir_all(root.join("modules/lib")).unwrap();
    fs::write(root.join("flake.nix"), "{ outputs = { self }: { }; }\n").unwrap();
    fs::write(
        root.join("modules/lib/dag.nix"),
        "{ lib }:\nrec {\n  map = f: xs: builtins.map f xs;\n  optionalString = c: s: if c then s else \"\";\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("modules/other.nix"),
        "{ lib, config }:\nlet\n  names = map (x: x) [ 1 2 ];\n  text = lib.optionalString true \"on\";\nin { inherit names text; }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolved_to_dag = |label: &str| {
        graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some(label)
                && edge.metadata.get("resolution").map(String::as_str) == Some("resolved")
        })
    };
    assert!(
        !resolved_to_dag("map"),
        "a bare `map` is the primop, not the binding another file made"
    );
    assert!(
        !resolved_to_dag("lib.optionalString"),
        "`lib` is nixpkgs', not a name this project declares"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_bare_ocaml_call_needs_an_open_to_leave_its_file() {
    // Nobody in dune opens `Predicate_lang`, yet the `not` it declares
    // answered 436 calls to the standard library's. A module a file does
    // open is a different matter: `open Decoder` is what makes a bare
    // `located` mean that module's.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n").unwrap();
    fs::write(
        root.join("src/predicate_lang.ml"),
        "let not t = t\n\nlet union ts = ts\n",
    )
    .unwrap();
    fs::write(
        root.join("src/decoder.ml"),
        "let located t = t\n\nlet enter t = t\n",
    )
    .unwrap();
    fs::write(
        root.join("src/reader.ml"),
        "open Decoder\n\nlet run flag t = if not flag then located t else enter t\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolution = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .and_then(|edge| edge.metadata.get("resolution").cloned())
    };
    assert_ne!(
        resolution("not").as_deref(),
        Some("resolved"),
        "no file opens Predicate_lang, so its `not` is out of reach"
    );
    assert_eq!(
        resolution("located").as_deref(),
        Some("resolved"),
        "`open Decoder` is what puts `located` within reach"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_elixir_function_belongs_to_the_module_that_declares_it() {
    // A module is a `defmodule` call rather than a block the grammar names,
    // so the walk that finds a class or an impl block never saw one: ecto
    // declares 3029 functions and not one knew its module. Two modules that
    // write the same name are then one name with two answers, and 5000 of
    // ecto's calls were reported ambiguous.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib/app")).unwrap();
    fs::write(
        root.join("mix.exs"),
        "defmodule App.MixProject do\n  use Mix.Project\n\n  def project, do: [app: :app]\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("lib/app/builder.ex"),
        "defmodule App.Builder do\n  def escape(value), do: value\n\n  def escape(value, _opts), do: value\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("lib/app/planner.ex"),
        "defmodule App.Planner do\n  def escape(value), do: value\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owners: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function && node.label == "escape")
        .filter_map(|node| node.metadata.get("owner_type").map(String::as_str))
        .collect();
    assert_eq!(
        owners,
        vec!["App.Builder", "App.Builder", "App.Planner"],
        "each clause belongs to the module that declares it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_cpp_method_written_inside_its_class_knows_whose_it_is() {
    // A method defined outside its class names the owner in the declarator
    // -- `void file_helper::open(..)` -- but one written inside the class
    // body has only the class around it to say so. nlohmann and spdlog
    // write nearly every method that way, and 96% of their functions knew
    // no owner at all.
    let root = temp_project_root();
    fs::create_dir_all(root.join("include")).unwrap();
    fs::write(root.join("CMakeLists.txt"), "project(app)\n").unwrap();
    fs::write(
        root.join("include/reader.hpp"),
        "#pragma once\n\nclass reader {\npublic:\n  int parse() { return 1; }\n};\n\nstruct writer {\n  int parse() { return 2; }\n};\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owners: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function && node.label == "parse")
        .filter_map(|node| node.metadata.get("owner_type").map(String::as_str))
        .collect();
    assert_eq!(
        owners,
        vec!["reader", "writer"],
        "each method belongs to the class body it is written in"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_function_knows_the_module_its_file_declares() {
    // Erlang states the module once at the top of the file and OCaml names
    // one after the file itself, so neither encloses anything a walk up the
    // tree can find: cowboy's 3924 functions and dune's 14636 belonged to
    // nobody, and every name two files shared was a choice the graph could
    // not make.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("rebar.config"), "{erl_opts, []}.\n").unwrap();
    fs::write(
        root.join("src/cowboy_req.erl"),
        "-module(cowboy_req).\n-export([reply/1]).\n\nreply(Req) -> Req.\n",
    )
    .unwrap();
    fs::write(
        root.join("src/path.ml"),
        "let build parts = parts\n\nmodule Local = struct\n  let build parts = parts\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owner = |label: &str, path: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == NodeKind::Function
                    && node.label == label
                    && node.span.as_ref().is_some_and(|span| span.path == path)
            })
            .and_then(|node| node.metadata.get("owner_type").cloned())
    };
    assert_eq!(
        owner("reply", "src/cowboy_req.erl").as_deref(),
        Some("cowboy_req")
    );
    // The file is the module, whether a binding sits at its top level or
    // inside a `module ... = struct` written in it.
    let builds: Vec<String> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function && node.label == "build")
        .filter_map(|node| node.metadata.get("owner_type").cloned())
        .collect();
    assert_eq!(builds, vec!["Path".to_string(), "Path".to_string()]);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_zig_function_belongs_to_the_container_that_holds_it() {
    // A zig type is a constant bound to a container, and a zig file is a
    // container too: `analysis.zig` is what `const analysis = @import(..)`
    // binds. zls declares 1215 functions and not one knew whose it was.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("build.zig"),
        "pub fn build(b: *Builder) void {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/analysis.zig"),
        "pub fn getPositionContext() u32 {\n    return 1;\n}\n\npub const Server = struct {\n    pub fn init() u32 {\n        return 2;\n    }\n};\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owner = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("owner_type").cloned())
    };
    assert_eq!(owner("getPositionContext").as_deref(), Some("analysis"));
    assert_eq!(owner("init").as_deref(), Some("Server"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_two_arms_of_one_macro_are_one_macro() {
    // A header defines `JSON_THROW` twice, once per side of an `#ifdef`,
    // and a caller means the macro rather than one of the two arms.
    // nlohmann keeps three copies of its header and 290 calls reported a
    // choice between six definitions of the same name.
    let root = temp_project_root();
    fs::create_dir_all(root.join("include")).unwrap();
    fs::write(root.join("CMakeLists.txt"), "project(app)\n").unwrap();
    fs::write(
        root.join("include/macro_scope.hpp"),
        "#pragma once\n\n#if defined(APP_NOEXCEPTION)\n    #define APP_THROW(exception) std::abort()\n#else\n    #define APP_THROW(exception) throw exception\n#endif\n",
    )
    .unwrap();
    fs::write(
        root.join("include/reader.hpp"),
        "#pragma once\n#include \"macro_scope.hpp\"\n\nint read(int value) {\n  if (value < 0) { APP_THROW(1); }\n  return value;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("APP_THROW")
        })
        .expect("the call is recorded");
    assert_ne!(
        call.metadata.get("resolution").map(String::as_str),
        Some("ambiguous"),
        "both definitions are the same macro under different conditions"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_call_through_a_type_parameter_names_no_definition() {
    // `F.map(fa)(f)` goes through a value whose type is a type parameter,
    // so nothing the project declares can be named by it. cats writes 178
    // of those and each was a choice between every `map` in the repository.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/main/scala")).unwrap();
    fs::write(root.join("build.sbt"), "name := \"app\"\n").unwrap();
    fs::write(
        root.join("src/main/scala/Chain.scala"),
        "package app\n\nfinal class Chain {\n  def map(f: Int => Int): Chain = this\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/scala/Ops.scala"),
        "package app\n\nobject Ops {\n  def run[F[_]](F: Functor[F], fa: F[Int]): F[Int] = F.map(fa)(x => x)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("F.map")
        })
        .expect("the call is recorded");
    assert_eq!(
        call.metadata.get("resolution").map(String::as_str),
        Some("external"),
        "a type parameter names no type the project declares, so the call leaves it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_expect_declaration_and_its_actual_are_one_declaration() {
    // `expect class Buffer` in commonMain and `actual class Buffer` in
    // jvmMain are one class written twice, and a source set is a directory
    // of its own -- so what tells two halves of one declaration apart is
    // exactly what the overload test asks them to share. okio spreads 768
    // calls over pairs like that.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/commonMain/kotlin/app")).unwrap();
    fs::create_dir_all(root.join("src/jvmMain/kotlin/app")).unwrap();
    fs::write(
        root.join("build.gradle.kts"),
        "plugins { kotlin(\"multiplatform\") }\n",
    )
    .unwrap();
    fs::write(
        root.join("src/commonMain/kotlin/app/Buffer.kt"),
        "package app\n\nexpect class Buffer() {\n  fun writeUtf8(text: String): Buffer\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/jvmMain/kotlin/app/Buffer.kt"),
        "package app\n\nactual class Buffer {\n  actual fun writeUtf8(text: String): Buffer {\n    return this\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/commonMain/kotlin/app/Writer.kt"),
        "package app\n\nfun write(buffer: Buffer) {\n  buffer.writeUtf8(\"hi\")\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("writeUtf8")
        })
        .expect("the call is recorded");
    assert_eq!(
        call.metadata.get("resolution").map(String::as_str),
        Some("resolved"),
        "both halves are the same method of the same class"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_csharp_using_alias_names_the_type_a_call_goes_through() {
    // `using Assert = Newtonsoft.Json.Tests.XUnitAssert;` renames a type,
    // and every call written through the alias means the type it stands
    // for. Newtonsoft's tests write 2199 `Assert.AreEqual` and each was a
    // choice between three `AreEqual` the project declares.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("App.sln"),
        "Microsoft Visual Studio Solution File\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Asserts.cs"),
        "namespace App\n{\n    public class XUnitAssert\n    {\n        public static void AreEqual(object a, object b) { }\n    }\n\n    public class StringAssert\n    {\n        public static void AreEqual(string a, string b) { }\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Tests.cs"),
        "using Assert = App.XUnitAssert;\n\nnamespace App\n{\n    public class Tests\n    {\n        public void Check()\n        {\n            Assert.AreEqual(1, 1);\n        }\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("Assert.AreEqual")
        })
        .expect("the call is recorded");
    assert_eq!(
        call.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("the target is a node");
    assert_eq!(
        target.metadata.get("owner_type").map(String::as_str),
        Some("XUnitAssert"),
        "the alias names which of the two `AreEqual` is meant"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_java_receiver_states_which_method_a_call_means() {
    // `Gson gson = new Gson();` says which `fromJson` the call means, and
    // gson declares fourteen of them. Java states the type of everything it
    // binds, and none of it had ever been read.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/main/java/app")).unwrap();
    fs::write(root.join("pom.xml"), "<project></project>\n").unwrap();
    fs::write(
        root.join("src/main/java/app/Gson.java"),
        "package app;\n\npublic final class Gson {\n  public String fromJson(String json) {\n    return json;\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/java/app/TypeAdapter.java"),
        "package app;\n\npublic final class TypeAdapter {\n  public String fromJson(String json) {\n    return json;\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/java/app/Caller.java"),
        "package app;\n\npublic final class Caller {\n  public String run() {\n    Gson gson = new Gson();\n    return gson.fromJson(\"{}\");\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("fromJson")
        })
        .expect("the call is recorded");
    assert_eq!(
        call.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("the target is a node");
    assert_eq!(
        target.metadata.get("owner_type").map(String::as_str),
        Some("Gson"),
        "the declaration of the receiver says whose method is meant"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_kotlin_receiver_states_which_method_a_call_means() {
    // `sink.writeUtf8(..)` reaches the graph as `writeUtf8`, because Kotlin
    // writes the callee as one navigation expression and the label keeps
    // its last segment. okio declares three `writeUtf8` and 503 calls chose
    // between them; what the call was written through is the only thing
    // that says which.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/main/kotlin/app")).unwrap();
    fs::write(
        root.join("build.gradle.kts"),
        "plugins { kotlin(\"jvm\") }\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/kotlin/app/Types.kt"),
        "package app\n\nclass Buffer {\n  fun writeUtf8(text: String): Buffer = this\n}\n\nclass BufferedSink {\n  fun writeUtf8(text: String): BufferedSink = this\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main/kotlin/app/Writer.kt"),
        "package app\n\nfun write(sink: BufferedSink) {\n  sink.writeUtf8(\"hi\")\n}\n\nfun local() {\n  val buffer: Buffer = Buffer()\n  buffer.writeUtf8(\"hi\")\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owners: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("writeUtf8")
        })
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .filter_map(|node| node.metadata.get("owner_type").map(String::as_str))
        .collect();
    assert_eq!(
        owners,
        vec!["BufferedSink", "Buffer"],
        "the parameter and the binding each say whose method is meant"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_swift_receiver_states_which_method_a_call_means() {
    // `session.request(..)` reaches the graph as `request`, and Alamofire
    // declares one on `Session` and one on `Manager`. A Swift parameter
    // always states its type, and `let manager = Manager()` names what it
    // builds.
    let root = temp_project_root();
    fs::create_dir_all(root.join("Sources/App")).unwrap();
    fs::write(
        root.join("Package.swift"),
        "// swift-tools-version:5.5\nimport PackageDescription\nlet package = Package(name: \"App\")\n",
    )
    .unwrap();
    fs::write(
        root.join("Sources/App/Types.swift"),
        "public class Session {\n    public func request(_ url: String) -> String { return url }\n}\n\npublic class Manager {\n    public func request(_ url: String) -> String { return url }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("Sources/App/Caller.swift"),
        "public func run(session: Session) -> String {\n    return session.request(\"u\")\n}\n\npublic func runLocal() -> String {\n    let manager = Manager()\n    return manager.request(\"u\")\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owners: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("request")
        })
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .filter_map(|node| node.metadata.get("owner_type").map(String::as_str))
        .collect();
    assert_eq!(owners, vec!["Session", "Manager"]);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_call_through_a_class_is_not_a_method_of_another() {
    // `Account.new` is not the `new` action twenty-three of mastodon's
    // controllers declare. Ruby keeps only the method in the label and
    // states the constant beside it, so the owner has to be asked for
    // rather than read off the label -- and when the project declares that
    // class and none of the candidates is its, the call means the class.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app/controllers")).unwrap();
    fs::create_dir_all(root.join("app/models")).unwrap();
    fs::write(
        root.join("Gemfile"),
        "source 'https://rubygems.org'\n\ngem 'rails'\n",
    )
    .unwrap();
    fs::write(
        root.join("app/models/account.rb"),
        "class Account\n  def initialize(name)\n    @name = name\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/accounts_controller.rb"),
        "class AccountsController\n  def new\n    @account = Account.new('a')\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/sessions_controller.rb"),
        "class SessionsController\n  def new\n    nil\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("new")
        })
        .expect("the call is recorded");
    assert_ne!(
        call.metadata.get("resolution").map(String::as_str),
        Some("ambiguous"),
        "neither controller's `new` is what `Account.new` means"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_php_receiver_states_which_method_a_call_means() {
    // `$handler->handle($record)` reaches the graph as `handle`, and
    // monolog declares nine of them. PHP states a parameter's type in the
    // signature and a property's in the class, and `$handler = new
    // StreamHandler()` names what it builds.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("composer.json"), "{\"name\": \"app/app\"}\n").unwrap();
    fs::write(
        root.join("src/StreamHandler.php"),
        "<?php\n\nclass StreamHandler\n{\n    public function handle(array $record): bool\n    {\n        return true;\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/NullHandler.php"),
        "<?php\n\nclass NullHandler\n{\n    public function handle(array $record): bool\n    {\n        return false;\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Logger.php"),
        "<?php\n\nclass Logger\n{\n    public function write(StreamHandler $handler, array $record): bool\n    {\n        return $handler->handle($record);\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("handle")
        })
        .expect("the call is recorded");
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("the target is a node");
    assert_eq!(
        target.metadata.get("owner_type").map(String::as_str),
        Some("StreamHandler"),
        "the signature says which handler is meant"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_julia_function_belongs_to_the_module_that_included_its_file() {
    // A julia file is not a module: DataFrames writes `module DataFrames`
    // once and `include`s the rest, so only 98 of its 1387 functions sat
    // inside the block that names them all. Every name two of its files
    // shared was then a choice the graph could not make, though multiple
    // dispatch means they are one function.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/other")).unwrap();
    fs::write(
        root.join("Project.toml"),
        "name = \"Frames\"\nuuid = \"a93c6f00-0000-0000-0000-000000000000\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Frames.jl"),
        "module Frames\n\ninclude(\"other/iteration.jl\")\ninclude(\"other/metadata.jl\")\n\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("src/other/iteration.jl"),
        "function nrow(df)\n    return 1\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("src/other/metadata.jl"),
        "function nrow(df, cols)\n    return 2\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owners: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function && node.label == "nrow")
        .filter_map(|node| node.metadata.get("owner_type").map(String::as_str))
        .collect();
    assert_eq!(
        owners,
        vec!["Frames", "Frames"],
        "an included file's functions belong to the module that included it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_go_method_comes_from_a_package_the_file_imports() {
    // terraform declares `Diagnostics.HasErrors` in `internal/policy` and
    // in `internal/tfdiags`, and every file that calls it imports exactly
    // one of the two -- a method on a type from a package the file never
    // imports cannot be the one meant.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal/tfdiags")).unwrap();
    fs::create_dir_all(root.join("internal/policy")).unwrap();
    fs::create_dir_all(root.join("internal/addrs")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal/tfdiags/diagnostics.go"),
        "package tfdiags\n\ntype Diagnostics []string\n\nfunc (d Diagnostics) HasErrors() bool {\n\treturn len(d) > 0\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal/policy/diagnostics.go"),
        "package policy\n\ntype Diagnostics []string\n\nfunc (d Diagnostics) HasErrors() bool {\n\treturn len(d) > 0\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal/addrs/checkable.go"),
        "package addrs\n\nimport (\n\t\"example.com/app/internal/tfdiags\"\n)\n\nfunc Check() bool {\n\tvar diags tfdiags.Diagnostics\n\treturn diags.HasErrors()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("diags.HasErrors")
        })
        .expect("the call is recorded");
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("the target is a node");
    assert_eq!(
        target.span.as_ref().map(|span| span.path.as_str()),
        Some("internal/tfdiags/diagnostics.go"),
        "the package the file imports is the one the method comes from"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_module_the_project_declares_answers_only_with_its_own() {
    // dune's `List.map` is the standard library's: stdune's list.ml
    // declares plenty but not `map`, and matching on the name alone
    // offered fifty-nine other modules' `map` instead. A nested path is
    // read from its head -- `Path.Build.append_source` sits in path.ml --
    // and a definition the caller's own file writes is reachable whatever
    // module path the call spells. The question only arises where the name
    // is shared: one definition and one name is not a choice.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n").unwrap();
    fs::write(
        root.join("src/list.ml"),
        "let filter_map f t = (f, t)\n\nlet rev t = t\n",
    )
    .unwrap();
    fs::write(
        root.join("src/other.ml"),
        "let map f t = (f, t)\n\nlet append_source a b = (a, b)\n",
    )
    .unwrap();
    fs::write(root.join("src/seq.ml"), "let map f t = (f, t)\n").unwrap();
    fs::write(
        root.join("src/path.ml"),
        "let append_source a b = (a, b)\n\nlet relative a b = (a, b)\n",
    )
    .unwrap();
    fs::write(
        root.join("src/user.ml"),
        "let run xs = List.map (fun x -> x) xs\n\nlet build a b = Path.Build.append_source a b\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = |label: &str| {
        graph
            .edges
            .iter()
            .find(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.metadata.get("call_label").map(String::as_str) == Some(label)
            })
            .unwrap_or_else(|| panic!("the call to {label} is recorded"))
    };
    assert_ne!(
        call("List.map")
            .metadata
            .get("resolution")
            .map(String::as_str),
        Some("resolved"),
        "the project's `List` has no `map`, so other.ml's is not it"
    );
    let nested = call("Path.Build.append_source");
    assert_eq!(
        nested.metadata.get("resolution").map(String::as_str),
        Some("resolved")
    );
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == nested.target)
        .expect("the target is a node");
    assert_eq!(
        target.span.as_ref().map(|span| span.path.as_str()),
        Some("src/path.ml"),
        "the head of the module path says where the definition lives"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_c_file_reaches_a_declaration_through_the_headers_it_includes() {
    // nlohmann keeps its sources under `include/` and an amalgamated copy
    // under `single_include/`, so every macro is declared twice and only
    // the include says which copy a caller means. It writes its own
    // includes in angle brackets -- `#include <nlohmann/detail/x.hpp>` --
    // because the build puts `include/` on the compiler's path, and none
    // of them reached another header at all.
    let root = temp_project_root();
    fs::create_dir_all(root.join("include/app/detail")).unwrap();
    fs::create_dir_all(root.join("single_include/app")).unwrap();
    fs::write(root.join("CMakeLists.txt"), "project(app)\n").unwrap();
    fs::write(
        root.join("include/app/detail/macro_scope.hpp"),
        "#pragma once\n#define APP_THROW(e) throw e\n",
    )
    .unwrap();
    fs::write(
        root.join("include/app/json.hpp"),
        "#pragma once\n#include <app/detail/macro_scope.hpp>\n\nint parse(int value) {\n  if (value < 0) { APP_THROW(1); }\n  return value;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("single_include/app/json.hpp"),
        "#pragma once\n#define APP_THROW(e) throw e\n\nint parse_amalgamated(int value) {\n  return value;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("APP_THROW")
        })
        .expect("the call is recorded");
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == call.target)
        .expect("the target is a node");
    assert_eq!(
        target.span.as_ref().map(|span| span.path.as_str()),
        Some("include/app/detail/macro_scope.hpp"),
        "the header the file includes is the one it reaches"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_constructor_of_another_type_in_the_same_file_is_not_the_answer() {
    // `SearcherBuilder::new()` names the type outright, so the three `new`
    // that ripgrep's JSON printer declares for types of its own are not it,
    // however near they sit. The escape that keeps a definition in the
    // caller's own file reachable is for a module the graph could not name
    // -- OCaml's and julia's -- not for a language where every definition
    // carries the type it belongs to.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub mod printer;\npub mod searcher;\n",
    )
    .unwrap();
    fs::write(
        root.join("src/searcher.rs"),
        // `SearcherBuilder` is a type the project declares and whose `new`
        // the graph never sees -- ripgrep derives it. The name is known;
        // the constructor is not.
        "pub struct Searcher;\n\nimpl Searcher {\n    pub fn run(&self) -> u32 {\n        1\n    }\n}\n\npub struct SearcherBuilder;\n\nimpl SearcherBuilder {\n    pub fn build(&self) -> Searcher {\n        Searcher\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/printer.rs"),
        "pub struct JsonSink;\n\nimpl JsonSink {\n    pub fn new() -> JsonSink {\n        JsonSink\n    }\n}\n\npub struct JsonBuilder;\n\nimpl JsonBuilder {\n    pub fn new() -> JsonBuilder {\n        JsonBuilder\n    }\n}\n\npub fn build() -> u32 {\n    let searcher = SearcherBuilder::new();\n    searcher.run()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let call = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str)
                    == Some("SearcherBuilder::new")
        })
        .expect("the call is recorded");
    assert_ne!(
        call.metadata.get("resolution").map(String::as_str),
        Some("ambiguous"),
        "neither `JsonSink::new` nor `JsonBuilder::new` is what the call names"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_ocaml_file_is_a_module_and_says_so() {
    // A file is a module, and that is how every call written through it
    // spells the name: `build` in path.ml is `Path.build`. Only `module X =
    // struct` was read, and that yielded no label either, so dune had no
    // OCaml module node at all and `Path` answered as something in
    // dune-rpc.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n").unwrap();
    fs::write(
        root.join("src/path.ml"),
        "let build parts = parts\n\nmodule Local = struct\n  let relative a b = (a, b)\nend\n",
    )
    .unwrap();
    fs::write(root.join("src/path.mli"), "val build : 'a -> 'a\n").unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let modules: Vec<(&str, &str)> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Module)
        .filter_map(|node| {
            node.span
                .as_ref()
                .map(|span| (node.label.as_str(), span.path.as_str()))
        })
        .collect();
    assert!(
        modules.contains(&("Path", "src/path.ml")),
        "the file is a module named after it: {modules:?}"
    );
    assert!(
        modules.contains(&("Local", "src/path.ml")),
        "a module written inside the file is one too: {modules:?}"
    );
    // The interface beside it is the same module, not a second one.
    assert_eq!(
        modules.iter().filter(|(label, _)| *label == "Path").count(),
        1,
        "{modules:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_haskell_file_states_the_module_its_definitions_belong_to() {
    // `module ShellCheck.Analytics where` states the name every import and
    // every qualified call writes, and nothing recorded it: asking about
    // the module found nothing, and none of shellcheck's 5985 functions
    // knew which module it was in.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src/ShellCheck")).unwrap();
    fs::write(
        root.join("shellcheck.cabal"),
        "name: shellcheck\nversion: 0.1.0\n",
    )
    .unwrap();
    fs::write(
        root.join("src/ShellCheck/Analytics.hs"),
        "module ShellCheck.Analytics where\n\ndata Severity = Warning | Error\n\nchecker :: Int -> Int\nchecker n = n + 1\n",
    )
    .unwrap();
    fs::write(
        root.join("src/ShellCheck/Analyzer.hs"),
        "module ShellCheck.Analyzer where\n\nimport ShellCheck.Analytics\n\nrun :: Int -> Int\nrun n = checker n\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let module = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Module && node.label == "ShellCheck.Analytics")
        .expect("the header declares the module");
    assert_eq!(
        module.span.as_ref().map(|span| span.path.as_str()),
        Some("src/ShellCheck/Analytics.hs")
    );
    // The module is the file, so what the file declares is what it holds.
    assert_eq!(
        module.metadata.get("module_scope").map(String::as_str),
        Some("file")
    );
    let checker = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "checker")
        .expect("the function is recorded");
    assert_eq!(
        checker.metadata.get("owner_type").map(String::as_str),
        Some("ShellCheck.Analytics"),
        "a definition belongs to the module its file declares"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_file_carries_the_name_its_importers_call_it_by() {
    // A lua file is a module and so is a python one, and neither states
    // its own name: `require "kong.tools.table"` does. Without reading the
    // importers the file could be asked about by path and by nothing else.
    let root = temp_project_root();
    fs::create_dir_all(root.join("kong/tools")).unwrap();
    fs::write(root.join("kong-3.0.rockspec"), "package = \"kong\"\n").unwrap();
    fs::write(
        root.join("kong/tools/table.lua"),
        "local _M = {}\n\nfunction _M.concat(t)\n  return t\nend\n\nreturn _M\n",
    )
    .unwrap();
    fs::write(
        root.join("kong/init.lua"),
        "local tbl = require \"kong.tools.table\"\n\nlocal function run()\n  return tbl.concat({1})\nend\n\nreturn { run = run }\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let file = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "kong/tools/table.lua")
        .expect("the file is scanned");
    assert_eq!(
        file.metadata.get("module_name").map(String::as_str),
        Some("kong.tools.table"),
        "the require says what the file is called"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_files_own_name_is_not_a_module_name() {
    // Zig writes `@import("Server.zig")`, which calls the file by the name
    // its label already carries. Recording that as the module name says
    // nothing new; a dotted module path does.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("build.zig"),
        "pub fn build(b: *Builder) void {}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Server.zig"),
        "pub fn run() u32 {\n    return 1;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/main.zig"),
        "const server = @import(\"Server.zig\");\n\npub fn main() void {\n    _ = server.run();\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let file = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label == "src/Server.zig")
        .expect("the file is scanned");
    assert_eq!(
        file.metadata.get("module_name"),
        None,
        "the import names the file by its own filename"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn ruby_calls_a_method_by_writing_its_name() {
    // `filtered_statuses` calls `default_statuses` and `hashtag_scope` with
    // no parentheses and no receiver, which is how Ruby is written -- and
    // the syntax gives nothing to tell such a call from a variable. 1248 of
    // mastodon's 1495 private methods with no caller are named this way in
    // the file that declares them. Only a name the same class declares
    // counts, and a name the body binds is a variable whatever the class
    // also declares.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app/controllers")).unwrap();
    fs::write(
        root.join("Gemfile"),
        "source 'https://rubygems.org'\n\ngem 'rails'\n",
    )
    .unwrap();
    fs::write(
        root.join("app/controllers/accounts_controller.rb"),
        "class AccountsController\n  def index\n    filtered_statuses\n  end\n\n  private\n\n  def filtered_statuses\n    default_statuses.tap do |statuses|\n      statuses.merge!(hashtag_scope)\n    end\n  end\n\n  def default_statuses\n    []\n  end\n\n  def hashtag_scope\n    {}\n  end\n\n  def shadowed\n    hashtag_scope = 1\n    hashtag_scope\n  end\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let callers_of = |label: &str| {
        let target = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .unwrap_or_else(|| panic!("{label} is declared"));
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Calls && edge.target == target.id)
            .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.source))
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>()
    };
    assert!(
        callers_of("filtered_statuses").contains(&"index"),
        "a bare name in a body is a call to the method the class declares"
    );
    assert!(
        callers_of("default_statuses").contains(&"filtered_statuses"),
        "the receiver of another call is a call too"
    );
    assert!(
        callers_of("hashtag_scope").contains(&"filtered_statuses"),
        "an argument written as a bare name is a call"
    );
    assert!(
        !callers_of("hashtag_scope").contains(&"shadowed"),
        "a name the body assigns is a variable, whatever the class declares"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_c_file_hands_a_function_over_by_name() {
    // `iter->_next_fp = all_values_iter_next` stores a function and
    // `aeCreateFileEvent(.., redisAeReadEvent, ..)` passes one, and both
    // make it run when the time comes -- neither is a call the syntax
    // records. 2543 of redis's 4619 functions with no caller are named
    // this way in the file that declares them.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Makefile"), "all:\n\tcc src/ae.c\n").unwrap();
    fs::write(
        root.join("src/ae.c"),
        "#include <stddef.h>\n\ntypedef struct iter { int (*next_fp)(void); } iter;\n\nstatic int all_values_iter_next(void) { return 1; }\n\nstatic int read_event(void) { return 2; }\n\nvoid create_event(int fd, int (*proc)(void), void *data);\n\nvoid setup(iter *it) {\n    it->next_fp = all_values_iter_next;\n    create_event(1, read_event, NULL);\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let callers_of = |label: &str| {
        let target = graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .unwrap_or_else(|| panic!("{label} is declared"));
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Calls && edge.target == target.id)
            .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.source))
            .map(|node| node.label.clone())
            .collect::<Vec<_>>()
    };
    assert!(
        callers_of("all_values_iter_next").contains(&"setup".to_string()),
        "a function stored in a field is handed over by the code that stores it"
    );
    assert!(
        callers_of("read_event").contains(&"setup".to_string()),
        "a function passed as an argument is handed over too"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_dart_getter_is_read_rather_than_called() {
    // `bool get isEmpty => length == 0` is written as a method and read as
    // a field, so no call edge can ever point at one and "nothing calls
    // it" says nothing. 608 of the http package's 3238 functions with no
    // caller are accessors.
    let root = temp_project_root();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("pubspec.yaml"),
        "name: app\nenvironment:\n  sdk: \">=3.0.0 <4.0.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("lib/box.dart"),
        "class Box {\n  int _length = 0;\n\n  bool get isEmpty => _length == 0;\n\n  set length(int value) => _length = value;\n\n  int size() => _length;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let form = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("definition_form").cloned())
    };
    assert_eq!(form("isEmpty").as_deref(), Some("accessor"));
    assert_eq!(form("length").as_deref(), Some("accessor"));
    assert_eq!(
        form("size"),
        None,
        "a method is still a method, whatever sits beside it"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_property_is_read_rather_than_called() {
    // `@property def description` is reached by writing `obj.description`
    // and `get inSFCRoot()` by writing `parser.inSFCRoot`, so no call edge
    // can ever point at either. 232 of django-oscar's 3312 functions with
    // no caller are properties.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("pyproject.toml"), "[project]\nname = \"app\"\n").unwrap();
    fs::write(
        root.join("src/models.py"),
        "class Address:\n    @property\n    def description(self):\n        return self.city\n\n    @description.setter\n    def description(self, value):\n        self._description = value\n\n    def save(self):\n        return None\n",
    )
    .unwrap();
    fs::write(root.join("package.json"), "{\"name\": \"app\"}\n").unwrap();
    fs::write(
        root.join("src/parser.ts"),
        "export class Parser {\n  private root = true;\n\n  public get inSFCRoot(): boolean {\n    return this.root;\n  }\n\n  set prop(value: boolean) {\n    this.root = value;\n  }\n\n  parse(): boolean {\n    return this.root;\n  }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let form = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .and_then(|node| node.metadata.get("definition_form").cloned())
    };
    assert_eq!(form("description").as_deref(), Some("accessor"));
    assert_eq!(form("inSFCRoot").as_deref(), Some("accessor"));
    assert_eq!(form("prop").as_deref(), Some("accessor"));
    assert_eq!(form("save"), None, "a method is still a method");
    assert_eq!(form("parse"), None, "a method is still a method");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_definition_is_linked_to_the_one_that_holds_it() {
    // flask's `route` returns a `decorator` that calls `add_url_rule`, and
    // asking for the way from `route` to `add_url_rule` found no path at
    // all: the nesting was recorded as metadata and no edge. Every
    // decorator, factory and callback-returning function was a dead end.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("pyproject.toml"), "[project]\nname = \"app\"\n").unwrap();
    fs::write(
        root.join("src/scaffold.py"),
        "class Scaffold:\n    def route(self, rule):\n        def decorator(f):\n            self.add_url_rule(rule, f)\n            return f\n\n        return decorator\n\n    def add_url_rule(self, rule, f):\n        return None\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let node = |label: &str| {
        graph
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Function && node.label == label)
            .unwrap_or_else(|| panic!("{label} is declared"))
    };
    let holds = graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::References
            && edge.metadata.get("relation").map(String::as_str) == Some("encloses")
            && edge.source == node("route").id
            && edge.target == node("decorator").id
    });
    assert!(holds, "the function that writes a closure reaches it");
    let calls = graph.edges.iter().any(|edge| {
        edge.kind == EdgeKind::Calls
            && edge.source == node("decorator").id
            && edge.target == node("add_url_rule").id
    });
    assert!(calls, "and the closure reaches what it calls");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_jsx_file_is_javascript_and_is_read() {
    // `.jsx` was not among the extensions any adapter claimed, so the file
    // was walked and never parsed: mastodon's hundred components held
    // nothing at all. The javascript grammar reads JSX -- the same
    // component saved as `.js` parses without a syntax error -- so a
    // `.jsx` file is javascript and says so.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("package.json"), "{\"name\": \"app\"}\n").unwrap();
    fs::write(
        root.join("src/Button.jsx"),
        "export function Button({ label }) {\n  return <button onClick={label}>{label}</button>;\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let button = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Function && node.label == "Button")
        .expect("the component is read");
    assert_eq!(
        button.metadata.get("language").map(String::as_str),
        Some("javascript"),
        "a .jsx file is javascript, not a dialect of its own"
    );
    assert_eq!(
        button.span.as_ref().map(|span| span.path.as_str()),
        Some("src/Button.jsx")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scan_coverage_says_what_it_passed_over() {
    // "5757 files were not indexed" says nothing a reader can act on.
    // mastodon's are 4276 `.svg` -- the assets, rightly left alone -- and
    // 310 `.haml`, a language this scan does not read, and only the
    // breakdown tells one from the other.
    let root = temp_project_root();
    fs::create_dir_all(root.join("app/views")).unwrap();
    fs::create_dir_all(root.join("public")).unwrap();
    fs::write(root.join("Gemfile"), "source 'https://rubygems.org'\n").unwrap();
    fs::write(root.join("app/views/show.haml"), "%h1 Title\n").unwrap();
    fs::write(root.join("app/views/index.haml"), "%h1 Index\n").unwrap();
    fs::write(root.join("public/logo.svg"), "<svg></svg>\n").unwrap();
    fs::write(root.join("app/account.rb"), "class Account\nend\n").unwrap();

    let report = scan_coverage(&root, &IndexOptions::default()).unwrap();

    assert_eq!(report.non_index_extensions.get("haml"), Some(&2));
    assert_eq!(report.non_index_extensions.get("svg"), Some(&1));
    assert_eq!(
        report.non_index_extensions.get("rb"),
        None,
        "a file the scan reads is not among the ones it passed over"
    );
    assert_eq!(
        report.non_index_files,
        report.non_index_extensions.values().sum::<usize>(),
        "every file passed over is accounted for by an extension"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_call_through_a_field_is_not_the_callers_own_method() {
    // `s.state.Module()` reaches the method through a field, so the name
    // belongs to the field's type, not to the caller's. terraform answered
    // 360 such calls with a method of the calling file: `s.mu.Lock` with the
    // caller's own `Lock`, `s.state.Module` with `SyncState.Module` where the
    // field is a `*State`.
    let root = temp_project_root();
    fs::create_dir_all(&*root).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("state.go"),
        "package app\n\ntype State struct{}\n\nfunc (s *State) Module(addr string) string {\n\treturn addr\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("sync.go"),
        "package app\n\nimport \"sync\"\n\ntype SyncState struct {\n\tstate *State\n\tmu    sync.Mutex\n}\n\nfunc (s *SyncState) Module(addr string) string {\n\treturn s.state.Module(addr)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let own = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Module"
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with("sync.go"))
        })
        .expect("the file's own Module");
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == own.id),
        "a call through a field is not answered by the caller's own method"
    );
}

#[test]
fn a_csharp_receiver_states_which_read_a_call_means() {
    // Newtonsoft.Json declares `Read` on a dozen classes, and 3093 calls
    // through a named receiver chose between all of them. C# states the type
    // of a parameter always and of a local most of the time, and that names
    // the owner outright.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("Readers.cs"),
        "namespace App;\n\npublic class JsonReader\n{\n    public bool Read() { return true; }\n}\n\npublic class BsonReader\n{\n    public bool Read() { return false; }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Loader.cs"),
        "namespace App;\n\npublic class Loader\n{\n    public bool Load(JsonReader reader)\n    {\n        return reader.Read();\n    }\n\n    public bool LoadBson()\n    {\n        var other = new BsonReader();\n        return other.Read();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let read_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "Read"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.Read"))
            .id
    };
    for (caller, owner) in [("Load", "JsonReader"), ("LoadBson", "BsonReader")] {
        let source = graph
            .nodes
            .iter()
            .find(|node| node.label == caller)
            .unwrap_or_else(|| panic!("{caller}"));
        let target = read_of(owner);
        assert!(
            graph.edges.iter().any(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge.source == source.id
                    && edge.target == target
                    && edge.metadata.get("resolution_basis").map(String::as_str)
                        == Some("receiver_type")
            }),
            "{caller} calls {owner}.Read, and the receiver's declared type says so"
        );
    }
}

#[test]
fn a_go_binding_takes_the_type_its_call_hands_back() {
    // `mgr := b.StateMgr()` says nothing about `mgr` on its own line, and
    // 4494 of terraform's unplaceable receivers are bound that way. A Go
    // signature states what it hands back, and a bare call means the
    // caller's own package, so the two join.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("states")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("states").join("kinds.go"),
        "package states\n\ntype Alpha struct{}\n\nfunc (a *Alpha) Run() string { return \"a\" }\n\ntype Beta struct{}\n\nfunc (b *Beta) Run() string { return \"b\" }\n\nfunc newAlpha() *Alpha {\n\treturn &Alpha{}\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal").join("states").join("use.go"),
        "package states\n\nfunc Use() string {\n\ta := newAlpha()\n\treturn a.Run()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let alpha_run = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Run"
                && node.metadata.get("owner_type").map(String::as_str) == Some("Alpha")
        })
        .expect("Alpha.Run");
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("a.Run")
        })
        .expect("the call through the bound name");
    assert_eq!(edge.target, alpha_run.id, "the call is Alpha's, not Beta's");
    assert_eq!(
        edge.metadata.get("resolution_basis").map(String::as_str),
        Some("receiver_type")
    );
}

#[test]
fn a_go_binding_reads_the_package_that_hands_it_back() {
    // `parser := configs.NewParser(fs)` types `parser` in another package's
    // signature. The file's own import says which directory that is, and
    // 1181 of terraform's unplaceable receivers are bound that way.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("configs")).unwrap();
    fs::create_dir_all(root.join("internal").join("command")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("configs").join("parser.go"),
        "package configs\n\ntype Parser struct{}\n\nfunc (p *Parser) LoadConfigDir(path string) string {\n\treturn path\n}\n\nfunc NewParser() *Parser {\n\treturn &Parser{}\n}\n",
    )
    .unwrap();
    // A namesake in a third package, so the name alone settles nothing.
    fs::write(
        root.join("internal").join("command").join("meta.go"),
        "package command\n\ntype Loader struct{}\n\nfunc (l *Loader) LoadConfigDir(path string) string {\n\treturn path\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal").join("command").join("load.go"),
        "package command\n\nimport (\n\t\"example.com/app/internal/configs\"\n)\n\ntype Meta struct{}\n\nfunc (m *Meta) Load() string {\n\tparser := configs.NewParser()\n\treturn parser.LoadConfigDir(\".\")\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let owned = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "LoadConfigDir"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.LoadConfigDir"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str)
                    == Some("parser.LoadConfigDir")
        })
        .expect("the call through the bound name");
    assert_eq!(
        edge.target,
        owned("Parser"),
        "the binding takes the type the package hands back, not the caller's own method"
    );
    assert_ne!(edge.target, owned("Loader"));
}

#[test]
fn a_stated_receiver_outranks_the_file_the_call_sits_in() {
    // `JsonSerializer.Populate` builds a `JsonSerializerInternalReader` and
    // calls `serializerReader.Populate`, which the file answered with its
    // own method: 201 of Newtonsoft.Json's 546 self-calls were a receiver
    // whose type the line above states.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("InternalReader.cs"),
        "namespace App;\n\npublic class InternalReader\n{\n    public void Populate(string target) { }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Serializer.cs"),
        "namespace App;\n\npublic class Serializer\n{\n    public void Populate(string target)\n    {\n        InternalReader reader = new InternalReader();\n        reader.Populate(target);\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let populate_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "Populate"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.Populate"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("reader.Populate")
        })
        .expect("the call through the built receiver");
    assert_eq!(
        edge.target,
        populate_of("InternalReader"),
        "the receiver's stated type answers, not the file the call sits in"
    );
    assert_ne!(edge.target, populate_of("Serializer"));
}

#[test]
fn a_scala_parameter_states_which_eqv_a_call_means() {
    // cats writes `implicit ev: Eq[A]` and then `ev.eqv(x, y)`, and the name
    // `eqv` belongs to dozens of its instances: 630 of the 2166 calls it
    // left ambiguous go through a receiver whose stated type names a type
    // the project declares.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("Eq.scala"),
        "package cats\n\ntrait Eq[A] {\n  def eqv(x: A, y: A): Boolean\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Instances.scala"),
        "package cats\n\ntrait ArrayInstances {\n  def eqv(x: Int, y: Int): Boolean = x == y\n\n  def check[A](ev: Eq[A], x: A, y: A): Boolean =\n    ev.eqv(x, y)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let eqv_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "eqv"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.eqv"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("ev.eqv")
        })
        .expect("the call through the stated parameter");
    assert_eq!(
        edge.target,
        eqv_of("Eq"),
        "the parameter's stated type answers, not the trait the call sits in"
    );
    assert_ne!(edge.target, eqv_of("ArrayInstances"));
}

#[test]
fn a_go_chain_is_a_field_whatever_the_case_of_its_name() {
    // gqlgen's generated stubs hold a func in a field named after the
    // interface -- `r.QueryResolver.Users(ctx)` -- and the file answered
    // with the very method the call sits in. A Go package is lowercase and
    // a type is not written in front of a call, so two dots are a field.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("stub")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("stub").join("stub.go"),
        "package stub\n\ntype Stub struct {\n\tQueryResolver struct {\n\t\tUsers func() string\n\t}\n}\n\ntype stubQuery struct{ *Stub }\n\nfunc (r *stubQuery) Users() string {\n\treturn r.QueryResolver.Users()\n}\n",
    )
    .unwrap();
    // A second definition of the name, so the name alone settles nothing.
    fs::create_dir_all(root.join("internal").join("real")).unwrap();
    fs::write(
        root.join("internal").join("real").join("resolver.go"),
        "package real\n\ntype Resolver struct{}\n\nfunc (r *Resolver) Users() string {\n\treturn \"\"\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let users_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "Users"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.Users"))
            .id
    };
    let own = users_of("stubQuery");
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == own),
        "a call through a field is not the method it is written in"
    );
}

#[test]
fn a_go_interface_states_the_methods_it_declares() {
    // `type Backend interface { Configure(..) }` declares `Configure` as
    // surely as an implementation does, and a call through a field of that
    // type means the contract rather than any one implementer. gqlgen states
    // 1206 methods that way and terraform 510, none of which the graph held.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("backend")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("backend").join("backend.go"),
        "package backend\n\ntype Backend interface {\n\tConfigure(path string) error\n}\n\ntype Local struct{}\n\nfunc (l *Local) Configure(path string) error {\n\treturn nil\n}\n\ntype Init struct {\n\tBackend Backend\n}\n\nfunc (i *Init) Start(path string) error {\n\treturn i.Backend.Configure(path)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let stated = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Configure"
                && node.metadata.get("owner_type").map(String::as_str) == Some("Backend")
        })
        .expect("the method the interface states");
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str)
                    == Some("i.Backend.Configure")
        })
        .expect("the call through the interface-typed field");
    assert_eq!(
        edge.target, stated.id,
        "a call through a field of an interface type means the contract"
    );
}

#[test]
fn a_receiver_reaches_the_method_its_type_inherits() {
    // Polly writes `bulkhead.Execute(..)` where `BulkheadPolicy` states no
    // `Execute` and `Policy` does: 545 of its calls name a type the project
    // declares and a method that type's base holds.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("Policy.cs"),
        "namespace App;\n\npublic class Policy\n{\n    public void Execute(string action) { }\n}\n\npublic class BulkheadPolicy : Policy\n{\n}\n",
    )
    .unwrap();
    // A namesake elsewhere, so the name alone settles nothing.
    fs::write(
        root.join("src").join("Runner.cs"),
        "namespace App;\n\npublic class Runner\n{\n    public void Execute(string action) { }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Specs.cs"),
        "namespace App;\n\npublic class Specs\n{\n    public void Run()\n    {\n        BulkheadPolicy bulkhead = new BulkheadPolicy();\n        bulkhead.Execute(\"x\");\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let inherited = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Execute"
                && node.metadata.get("owner_type").map(String::as_str) == Some("Policy")
        })
        .expect("Policy.Execute");
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("bulkhead.Execute")
        })
        .expect("the call through the derived receiver");
    assert_eq!(
        edge.target, inherited.id,
        "the method the receiver's type inherits is still reached through it"
    );
}

#[test]
fn a_csharp_field_states_the_type_its_method_belongs_to() {
    // `private readonly BsonBinaryWriter _writer;` is declared in the class
    // rather than in the method that uses it, so `_writer.Flush()` was
    // answered by the writer it sits in -- a self-call. Polly reaches for a
    // field that way in 591 of the calls it could not place.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("BsonBinaryWriter.cs"),
        "namespace App;\n\npublic class BsonBinaryWriter\n{\n    public void Flush() { }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("BsonWriter.cs"),
        "namespace App;\n\npublic class BsonWriter\n{\n    private readonly BsonBinaryWriter _writer;\n\n    public void Flush()\n    {\n        _writer.Flush();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let flush_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "Flush"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.Flush"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("_writer.Flush")
        })
        .expect("the call through the field");
    assert_eq!(
        edge.target,
        flush_of("BsonBinaryWriter"),
        "the field states which Flush the call means"
    );
    assert_ne!(edge.target, flush_of("BsonWriter"));
}

#[test]
fn a_kotlin_property_states_the_type_its_method_belongs_to() {
    // okio writes `val sink: BufferedSink` in the class and `sink.flush()` in
    // the tests, and the call was answered by the test class's own `flush`:
    // 2108 of the calls it could not place reach for a property that way.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("BufferedSink.kt"),
        "package okio\n\nclass BufferedSink {\n    fun flush() {\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("SinkTest.kt"),
        "package okio\n\nclass SinkTest {\n    val sink: BufferedSink = BufferedSink()\n\n    fun flush() {\n    }\n\n    fun writes() {\n        sink.flush()\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let flush_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "flush"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.flush"))
            .id
    };
    let writes = graph
        .nodes
        .iter()
        .find(|node| node.label == "writes")
        .expect("the calling method");
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.source == writes.id
                && edge.metadata.get("call_label").map(String::as_str) == Some("flush")
        })
        .expect("the call through the property");
    assert_eq!(
        edge.target,
        flush_of("BufferedSink"),
        "the property states which flush the call means"
    );
    assert_ne!(edge.target, flush_of("SinkTest"));
}

#[test]
fn a_php_property_states_the_type_its_method_belongs_to() {
    // monolog writes `$this->handler->handle($record)` inside
    // `HandlerWrapper::handle`, which the graph read as the wrapper calling
    // itself. The class states the property's type, and that names the
    // interface the call means.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("HandlerInterface.php"),
        "<?php\n\nnamespace Monolog;\n\ninterface HandlerInterface\n{\n    public function handle(array $record): bool;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("HandlerWrapper.php"),
        "<?php\n\nnamespace Monolog;\n\nclass HandlerWrapper implements HandlerInterface\n{\n    protected HandlerInterface $handler;\n\n    public function handle(array $record): bool\n    {\n        return $this->handler->handle($record);\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let handle_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "handle"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}::handle"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("handle")
        })
        .expect("the call through the property");
    assert_eq!(
        edge.target,
        handle_of("HandlerInterface"),
        "the property states which handle the call means"
    );
    assert_ne!(edge.target, handle_of("HandlerWrapper"));
}

#[test]
fn a_go_struct_states_the_types_it_embeds() {
    // `type ApplyCommand struct { Meta }` is how Go says a method of `Meta`
    // is also its own, and terraform's commands are written that way: 250 of
    // its types embed another and none of them said so, so `c.Operation`
    // stood ambiguous between every `Operation` in the repository.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("command")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("command").join("meta.go"),
        "package command\n\ntype Meta struct{}\n\nfunc (m *Meta) Operation() string {\n\treturn \"meta\"\n}\n",
    )
    .unwrap();
    // A namesake elsewhere, so the name alone settles nothing.
    fs::write(
        root.join("internal").join("command").join("other.go"),
        "package command\n\ntype Other struct{}\n\nfunc (o *Other) Operation() string {\n\treturn \"other\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal").join("command").join("apply.go"),
        "package command\n\ntype ApplyCommand struct {\n\tMeta\n}\n\nfunc (c *ApplyCommand) Run() string {\n\treturn c.Operation()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let operation_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "Operation"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.Operation"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("c.Operation")
        })
        .expect("the call through the embedding receiver");
    assert_eq!(
        edge.target,
        operation_of("Meta"),
        "the type a struct embeds answers for its methods"
    );
    assert_ne!(edge.target, operation_of("Other"));
}

#[test]
fn a_scala_implicit_named_after_its_type_is_a_value() {
    // `class MapAdditiveMonoid[K, V](implicit V: AdditiveSemigroup[V])` names
    // the instance after the type parameter it carries, and `V.plus(x, y)`
    // was read as a call through a type parameter -- which names nothing --
    // and then answered by the class the call sits in. cats writes 450 of
    // them, and the class states the type in its own parameter list.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("semigroup.scala"),
        "package algebra\n\ntrait AdditiveSemigroup[A] {\n  def plus(x: A, y: A): A\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("map.scala"),
        "package algebra\n\nclass MapAdditiveMonoid[K, V](implicit V: AdditiveSemigroup[V]) {\n  def plus(xs: Int, ys: Int): Int = V.plus(xs, ys)\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let plus_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "plus"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.plus"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("V.plus")
        })
        .expect("the call through the implicit");
    assert_eq!(
        edge.target,
        plus_of("AdditiveSemigroup"),
        "the class parameter states that V is a value of that type"
    );
    assert_ne!(edge.target, plus_of("MapAdditiveMonoid"));
}

#[test]
fn a_csharp_extension_is_reached_through_the_type_it_extends() {
    // `static bool IsValueType(this Type type)` belongs to the static class
    // that declares it and is reached through `Type`, which that class never
    // names. Newtonsoft.Json declares 272 such methods and Polly 137.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("TypeExtensions.cs"),
        "namespace App;\n\npublic static class TypeExtensions\n{\n    public static bool IsValueType(this Reflected type) { return true; }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Reflected.cs"),
        "namespace App;\n\npublic class Reflected\n{\n}\n",
    )
    .unwrap();
    // A namesake, so the name alone settles nothing.
    fs::write(
        root.join("src").join("Other.cs"),
        "namespace App;\n\npublic class Other\n{\n    public bool IsValueType() { return false; }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("Caller.cs"),
        "namespace App;\n\npublic class Caller\n{\n    public bool Check(Reflected type)\n    {\n        return type.IsValueType();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let declared_by = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "IsValueType"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.IsValueType"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("type.IsValueType")
        })
        .expect("the call through the extended type");
    assert_eq!(
        edge.target,
        declared_by("TypeExtensions"),
        "the receiver's type reaches the extension declared for it"
    );
    assert_ne!(edge.target, declared_by("Other"));
}

#[test]
fn a_kotlin_extension_is_reached_through_its_receiver() {
    // `fun Buffer.asAscii(): String` writes the type in front of the name,
    // and okio declares 408 functions that way. A method the type itself
    // declares still wins over one declared for it, which is what Kotlin
    // does -- reading them as one choice cost okio 79 answers.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("buffer.kt"),
        "package okio\n\nclass Buffer {\n    fun size(): Int = 0\n}\n\nfun Buffer.asAscii(): String = \"x\"\n",
    )
    .unwrap();
    // A namesake declared as a method, so the name alone settles nothing.
    fs::write(
        root.join("src").join("other.kt"),
        "package okio\n\nclass Other {\n    fun asAscii(): String = \"\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("user.kt"),
        "package okio\n\nclass User {\n    fun run(b: Buffer): String = b.asAscii()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let extension = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "asAscii"
                && node.metadata.get("reached_through").map(String::as_str) == Some("Buffer")
        })
        .expect("the extension states the type it is reached through");
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("asAscii")
        })
        .expect("the call through the receiver");
    assert_eq!(
        edge.target, extension.id,
        "the receiver's type reaches the extension declared for it"
    );
}

#[test]
fn a_csharp_base_call_means_the_class_it_extends() {
    // `base.Close()` inside `Close()` is the parent's implementation and
    // never this one; the rule knew `super` and not the word C# uses, so
    // 130 of Newtonsoft.Json's calls were a definition calling itself.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("JsonReader.cs"),
        "namespace App;\n\npublic class JsonReader\n{\n    public virtual void Close() { }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("BsonReader.cs"),
        "namespace App;\n\npublic class BsonReader : JsonReader\n{\n    public override void Close()\n    {\n        base.Close();\n    }\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let close_of = |owner: &str| {
        graph
            .nodes
            .iter()
            .find(|node| {
                node.label == "Close"
                    && node.metadata.get("owner_type").map(String::as_str) == Some(owner)
            })
            .unwrap_or_else(|| panic!("{owner}.Close"))
            .id
    };
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("base.Close")
        })
        .expect("the call through base");
    assert_eq!(
        edge.target,
        close_of("JsonReader"),
        "base names the class the caller extends"
    );
    assert_ne!(edge.target, close_of("BsonReader"));
}

#[test]
fn a_binding_from_a_foreign_package_leaves_the_project() {
    // `f, err := os.Open(path)` binds `f` to what a package outside the
    // repository hands back, so `f.Close()` is not the repository's own
    // `Close`. terraform writes 881 calls on such bindings, and they were
    // answering with whatever shared the name -- `f.Close` with a
    // `SyncState`, `info.IsDir` with a snapshot's file info.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("states")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("states").join("sync.go"),
        "package states\n\ntype SyncState struct{}\n\nfunc (s *SyncState) Close() error {\n\treturn nil\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal").join("states").join("read.go"),
        "package states\n\nimport (\n\t\"os\"\n)\n\nfunc Read(path string) error {\n\tf, err := os.Open(path)\n\tif err != nil {\n\t\treturn err\n\t}\n\treturn f.Close()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let own = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Close"
                && node.metadata.get("owner_type").map(String::as_str) == Some("SyncState")
        })
        .expect("the project's own Close");
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == own.id),
        "a call on what a foreign package handed back is not the project's own method"
    );
    let leaves = graph.nodes.iter().any(|node| {
        node.kind == NodeKind::ExternalDependency
            && node.label == "f.Close"
            && node.metadata.get("resolution").map(String::as_str) == Some("external")
    });
    assert!(leaves, "the call is recorded as leaving the project");
}

#[test]
fn an_ocaml_constructor_is_a_declaration_a_call_can_reach() {
    // `type sexp = Atom of string` declares `Atom`, and every `Atom s` in
    // the project names that declaration -- dune applies its constructors
    // 4314 times and the graph held none of them. A polymorphic variant is
    // structural, declared nowhere, so `` `Payload v `` is not a call.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n").unwrap();
    fs::write(
        root.join("src").join("conv.ml"),
        "type sexp = Atom of string\n\nlet quote s = String.trim s\n\nlet encode s =\n  let value = Atom (quote s) in\n  let tagged = `Payload value in\n  (value, tagged)\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let declared = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Atom"
                && node.metadata.get("definition_form").map(String::as_str) == Some("constructor")
        })
        .expect("the type states the constructor");
    assert_eq!(
        declared.metadata.get("owner_type").map(String::as_str),
        Some("sexp"),
        "the constructor belongs to the type that declares it"
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == declared.id),
        "the application reaches the declaration"
    );
    let call_labels: Vec<&str> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls)
        .filter_map(|edge| edge.metadata.get("call_label").map(String::as_str))
        .collect();
    assert!(
        !call_labels.iter().any(|label| label.starts_with('`')),
        "a polymorphic variant is declared nowhere and is not a call: {call_labels:?}"
    );
    assert!(
        call_labels.contains(&"quote"),
        "the function the file declares is still called: {call_labels:?}"
    );
}

#[test]
fn a_type_that_embeds_a_foreign_one_reaches_its_methods() {
    // `type Provider struct { sync.Mutex }` gives `p.Lock()` the mutex's
    // method, and the project declares no `Lock` on `Provider` -- so the
    // call left the type and was answered by whichever `Lock` shared the
    // name. terraform writes 232 of them.
    let root = temp_project_root();
    fs::create_dir_all(root.join("internal").join("states")).unwrap();
    fs::write(root.join("go.mod"), "module example.com/app\n\ngo 1.22\n").unwrap();
    fs::write(
        root.join("internal").join("states").join("sync.go"),
        "package states\n\ntype SyncState struct{}\n\nfunc (s *SyncState) Lock() {\n}\n\ntype OtherState struct{}\n\nfunc (o *OtherState) Lock() {\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("internal").join("states").join("provider.go"),
        "package states\n\nimport (\n\t\"sync\"\n)\n\ntype Provider struct {\n\tsync.Mutex\n}\n\nfunc (p *Provider) Configure() {\n\tp.Lock()\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let own = graph
        .nodes
        .iter()
        .find(|node| {
            node.label == "Lock"
                && node.metadata.get("owner_type").map(String::as_str) == Some("SyncState")
        })
        .expect("the project's own Lock");
    assert!(
        !graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls && edge.target == own.id),
        "a method reached through an embedded foreign type is not the project's"
    );
    let leaves = graph.nodes.iter().any(|node| {
        node.kind == NodeKind::ExternalDependency
            && node.label == "p.Lock"
            && node.metadata.get("resolution").map(String::as_str) == Some("external")
    });
    assert!(leaves, "the call is recorded as leaving the project");
}

#[test]
fn a_pattern_named_inside_a_string_is_not_a_call() {
    // A detector's own source is source too: `if lower.contains("dotenv(")`
    // names the pattern rather than calling it, and this repository read
    // itself as loading a `.env` file for exactly that line.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"app\"\n").unwrap();
    fs::write(
        root.join("src").join("detector.rs"),
        "pub fn detect(line: &str) -> bool {\n    let lower = line.to_ascii_lowercase();\n    lower.contains(\"dotenv\") && lower.contains(\"dotenv(\")\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    dotenv::dotenv().ok();\n}\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let reads = |path: &str| {
        graph.nodes.iter().any(|node| {
            node.kind == NodeKind::Config
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with(path))
        })
    };
    assert!(reads("main.rs"), "the call is still read as one");
    assert!(
        !reads("detector.rs"),
        "a pattern named inside a string is not a call"
    );
}

#[test]
fn a_lua_module_hands_out_what_another_declares() {
    // `spec/helpers.lua` binds `local cmd = reload_module("spec.internal.cmd")`
    // and returns `start_kong = cmd.start_kong`: the callers write this
    // module's name and the definition is the other's. 689 of kong's calls
    // stood between the spec files that declare the same names locally.
    let root = temp_project_root();
    fs::create_dir_all(root.join("spec").join("internal")).unwrap();
    fs::write(
        root.join("spec").join("internal").join("cmd.lua"),
        "local M = {}\n\nfunction M.start_kong(conf)\n  return conf\nend\n\nreturn M\n",
    )
    .unwrap();
    fs::write(
        root.join("spec").join("helpers.lua"),
        "local cmd = require(\"spec.internal.cmd\")\n\nreturn {\n  start_kong = cmd.start_kong,\n}\n",
    )
    .unwrap();
    // A local of the same name, so the name alone settles nothing.
    fs::write(
        root.join("spec").join("other_spec.lua"),
        "local function start_kong(conf)\n  return conf\nend\n\nreturn start_kong\n",
    )
    .unwrap();
    fs::write(
        root.join("spec").join("use_spec.lua"),
        "local helpers = require(\"spec.helpers\")\n\nlocal function run()\n  return helpers.start_kong({})\nend\n\nreturn run\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let declared = graph
        .nodes
        .iter()
        .find(|node| {
            node.label.ends_with("start_kong")
                && node
                    .span
                    .as_ref()
                    .is_some_and(|span| span.path.ends_with("internal/cmd.lua"))
        })
        .expect("the module that declares it");
    let edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.kind == EdgeKind::Calls
                && edge.metadata.get("call_label").map(String::as_str) == Some("helpers.start_kong")
        })
        .expect("the call through the module that hands it out");
    assert_eq!(
        edge.target, declared.id,
        "the call reaches the module the name is handed out from"
    );
    assert_eq!(
        edge.metadata.get("resolution_basis").map(String::as_str),
        Some("module_re_export")
    );
}

#[test]
fn an_ocaml_open_of_a_name_the_file_binds_is_not_another_module() {
    // `module Make (Sexp : Sexp) = struct .. open Sexp` opens the functor's
    // own parameter. dune's vendored csexp does exactly that, and the graph
    // answered with stdune's `sexp.ml` -- a dependency cycle between the two
    // that does not exist.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n").unwrap();
    fs::write(
        root.join("src").join("sexp.ml"),
        "type t = Atom of string\n\nlet to_string t =\n  match t with Atom s -> s\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("csexp.ml"),
        "module type Sexp = sig\n  type t\nend\n\nmodule Make (Sexp : Sexp) = struct\n  open Sexp\n\n  let hold (x : t) = x\nend\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let sexp_file = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label.ends_with("src/sexp.ml"))
        .expect("the other file");
    let linked = graph.edges.iter().any(|edge| {
        edge.target == sexp_file.id
            && graph.nodes.iter().any(|node| {
                node.id == edge.source
                    && node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.path.ends_with("csexp.ml"))
            })
    });
    assert!(
        !linked,
        "opening a name the file binds itself does not reach another file"
    );
}

#[test]
fn an_ocaml_file_cannot_import_the_one_that_includes_it() {
    // stdune's `string.ml` includes `String_split`, and `string_split.ml`
    // opens `String`. The language forbids a cycle between modules, so that
    // `String` is the standard library's -- but the graph answered with the
    // file that includes it and reported the pair as a dependency cycle.
    let root = temp_project_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("dune-project"), "(lang dune 3.0)\n").unwrap();
    fs::write(
        root.join("src").join("string.ml"),
        "include String_split\n\nlet length s = String.length s\n",
    )
    .unwrap();
    fs::write(
        root.join("src").join("string_split.ml"),
        "open String\n\nlet split_on s = String.split_on_char ',' s\n",
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let string_file = graph
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::File && node.label.ends_with("src/string.ml"))
        .expect("the including file");
    let back_edge = graph.edges.iter().any(|edge| {
        edge.target == string_file.id
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "local_import_file")
            && graph.nodes.iter().any(|node| {
                node.id == edge.source
                    && node
                        .span
                        .as_ref()
                        .is_some_and(|span| span.path.ends_with("string_split.ml"))
            })
    });
    assert!(
        !back_edge,
        "the file that is included does not import the one that includes it"
    );
}

#[test]
fn a_member_of_a_value_is_not_a_static_call() {
    use crate::resolve::receiver_call_is_universal;

    // A static call names the type first, whatever case the member after
    // it carries.
    assert!(receiver_call_is_universal("csharp", "a1.Length.CompareTo"));
    assert!(receiver_call_is_universal("csharp", "value.ToString"));
    assert!(receiver_call_is_universal(
        "csharp",
        "args.Outcome.Exception.GetType"
    ));
    // `JsonConvert.ToString` is Newtonsoft's own, called 720 times.
    assert!(!receiver_call_is_universal(
        "csharp",
        "JsonConvert.ToString"
    ));
    assert!(!receiver_call_is_universal(
        "csharp",
        "Newtonsoft.Json.JsonConvert.ToString"
    ));
    // A name the project declares is still its own.
    assert!(!receiver_call_is_universal("csharp", "reader.Read"));
}

#[test]
fn a_word_the_spec_runner_provides_is_not_the_projects_own() {
    let root = temp_project_root();
    fs::create_dir_all(root.join("app").join("models")).unwrap();
    fs::create_dir_all(root.join("spec").join("models")).unwrap();
    // mastodon declares a `context` of its own in `app/models/export.rb`,
    // and 716 of its spec calls read as a choice between that method and
    // the word RSpec writes the spec in.
    fs::write(
        root.join("app").join("models").join("export.rb"),
        "class Export\n  def context\n    @context\n  end\nend\n",
    )
    .unwrap();
    fs::write(
        root.join("spec").join("models").join("export_spec.rb"),
        r#"require 'rails_helper'

def change(model, field)
  [model, field]
end

def uses_the_suites_own_change
  change(Export, :count)
end

RSpec.describe Export do
  context 'when exporting' do
    it 'names the account' do
      Export.new.context
    end
  end
end
"#,
    )
    .unwrap();

    let graph = scan_project(&root, &IndexOptions::default()).unwrap();
    let resolutions_of = |call_label: &str| {
        graph
            .edges
            .iter()
            .filter(|edge| {
                edge.kind == EdgeKind::Calls
                    && edge
                        .metadata
                        .get("call_label")
                        .is_some_and(|label| label == call_label)
            })
            .filter_map(|edge| edge.metadata.get("resolution").cloned())
            .collect::<Vec<_>>()
    };

    // RSpec hands the spec `context` and `it`, whatever the project
    // declares under those names -- and a `context` reached through an
    // object is still the object's.
    assert!(
        resolutions_of("context").contains(&"builtin".to_string()),
        "got {:?}",
        resolutions_of("context")
    );
    assert!(resolutions_of("context").contains(&"resolved".to_string()));
    assert_eq!(resolutions_of("it"), vec!["builtin".to_string()]);
    // A suite does declare helpers of its own, and one of those is what
    // the spec means: koel writes `assertMatchesAgainstRules` 21 times and
    // declares it under `tests/`.
    assert_eq!(resolutions_of("change"), vec!["resolved".to_string()]);

    fs::remove_dir_all(root).unwrap();
}
