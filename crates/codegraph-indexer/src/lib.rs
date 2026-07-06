use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeKind};
use codegraph_parser::{Language, ParsedItemKind, parse_source};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("failed to walk project tree at {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
}

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: u64,
    pub ignored_names: BTreeSet<String>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_ignored: false,
            max_file_size: 2 * 1024 * 1024,
            ignored_names: default_ignored_names(),
        }
    }
}

pub fn scan_project(
    root: impl AsRef<Path>,
    options: &IndexOptions,
) -> Result<CodeGraph, IndexError> {
    let root = root.as_ref();
    let root_label = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".");
    let mut graph = CodeGraph::new(root_label);

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, options))
    {
        let entry = entry.map_err(|source| IndexError::Walk {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path == root {
            continue;
        }

        let Ok(relative_path) = path.strip_prefix(root) else {
            continue;
        };
        let label = relative_path.to_string_lossy().replace('\\', "/");

        if entry.file_type().is_dir() {
            let id = graph.add_node(NodeKind::Directory, label);
            graph.add_edge(graph.root, id, EdgeKind::Contains, Confidence::Exact);
            continue;
        }

        if entry.file_type().is_file() && is_probably_source_file(path, options.max_file_size) {
            index_file(&mut graph, path, &label);
        }
    }

    Ok(graph)
}

fn index_file(graph: &mut CodeGraph, path: &Path, label: &str) {
    let language = Language::detect(path);
    let mut metadata = BTreeMap::new();

    if let Some(language) = language {
        metadata.insert("language".to_string(), language.to_string());
    }

    let parse_result = language.and_then(|language| match fs::read(path) {
        Ok(source) => Some((language, parse_source(label, &source, language))),
        Err(error) => {
            metadata.insert("read_error".to_string(), error.to_string());
            None
        }
    });

    let file_id = graph.add_node_with_metadata(NodeKind::File, label, None, metadata);
    graph.add_edge(graph.root, file_id, EdgeKind::Contains, Confidence::Exact);

    if let Some((language, parse_result)) = parse_result {
        match parse_result {
            Ok(parsed) => {
                if parsed.has_error_nodes {
                    add_file_metadata(graph, file_id, "syntax_errors", "true");
                }

                for item in parsed.items {
                    let node_kind = match item.kind {
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint => NodeKind::Function,
                        ParsedItemKind::Type => NodeKind::Type,
                        ParsedItemKind::Module => NodeKind::Module,
                        ParsedItemKind::Import => NodeKind::ExternalDependency,
                    };
                    let mut item_metadata = BTreeMap::new();
                    item_metadata.insert("language".to_string(), language.to_string());
                    item_metadata.insert("parser".to_string(), "tree-sitter".to_string());
                    item_metadata.insert(
                        "item_kind".to_string(),
                        parsed_item_kind_name(item.kind).to_string(),
                    );

                    let item_id = graph.add_node_with_metadata(
                        node_kind,
                        item.label,
                        Some(item.span),
                        item_metadata,
                    );
                    let edge_kind = match item.kind {
                        ParsedItemKind::Import => EdgeKind::Imports,
                        _ => EdgeKind::Contains,
                    };
                    graph.add_edge(file_id, item_id, edge_kind, Confidence::Syntactic);

                    if item.kind == ParsedItemKind::Entrypoint {
                        graph.add_edge(
                            graph.root,
                            item_id,
                            EdgeKind::Entrypoint,
                            Confidence::Syntactic,
                        );
                    }
                }
            }
            Err(error) => add_file_metadata(graph, file_id, "parse_error", error.to_string()),
        }
    }
}

fn add_file_metadata(
    graph: &mut CodeGraph,
    file_id: codegraph_core::NodeId,
    key: &str,
    value: impl Into<String>,
) {
    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == file_id) {
        node.metadata.insert(key.to_string(), value.into());
    }
}

fn parsed_item_kind_name(kind: ParsedItemKind) -> &'static str {
    match kind {
        ParsedItemKind::Function => "function",
        ParsedItemKind::Type => "type",
        ParsedItemKind::Module => "module",
        ParsedItemKind::Import => "import",
        ParsedItemKind::Entrypoint => "entrypoint",
    }
}

fn should_enter(entry: &DirEntry, options: &IndexOptions) -> bool {
    if !options.include_hidden && is_hidden(entry) {
        return false;
    }

    if !options.include_ignored && is_ignored_name(entry, &options.ignored_names) {
        return false;
    }

    true
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.') && name != ".")
}

fn is_ignored_name(entry: &DirEntry, ignored_names: &BTreeSet<String>) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| ignored_names.contains(name))
}

fn default_ignored_names() -> BTreeSet<String> {
    [
        ".git",
        ".hg",
        ".svn",
        "target",
        "node_modules",
        "dist",
        "build",
        ".next",
        ".turbo",
        ".venv",
        "__pycache__",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn is_probably_source_file(path: &Path, max_file_size: u64) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    metadata.len() <= max_file_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scan_project_skips_default_ignored_directories() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("target").join("debug.log"), "noise\n").unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

        assert!(labels.contains(&"src/main.rs"));
        assert!(!labels.contains(&"target"));
        assert!(!labels.contains(&"target/debug.log"));

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

    fn temp_project_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("codegraph-indexer-test-{nanos}"))
    }
}
