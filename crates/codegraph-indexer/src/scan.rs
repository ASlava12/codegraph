//! Project walking and per-file indexing: the scan entrypoints, scan
//! coverage reporting, and the file-level fact dispatcher.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind, SourceSpan};
use codegraph_parser::{Language, ParsedItemKind, adapter_for_language, adapter_for_path};
use globset::GlobSet;
use walkdir::WalkDir;

#[allow(unused_imports)]
use crate::*;

/// Cooperative cancellation for a running scan. Cloning shares the flag, so a
/// caller can hand one clone to the scan and keep another to cancel it; an
/// unset token (`ScanCancellation::none`) never cancels.
#[derive(Debug, Clone, Default)]
pub struct ScanCancellation {
    flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl ScanCancellation {
    pub fn new() -> Self {
        Self {
            flag: Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
                false,
            ))),
        }
    }

    pub fn none() -> Self {
        Self { flag: None }
    }

    pub fn cancel(&self) {
        if let Some(flag) = &self.flag {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    pub fn is_canceled(&self) -> bool {
        self.flag
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn check(&self) -> Result<(), IndexError> {
        if self.is_canceled() {
            return Err(IndexError::Canceled);
        }
        Ok(())
    }
}

pub fn scan_project(
    root: impl AsRef<Path>,
    options: &IndexOptions,
) -> Result<CodeGraph, IndexError> {
    scan_project_with_scope(root.as_ref(), options, None, &ScanCancellation::none())
}

/// Like [`scan_project`], but aborts with [`IndexError::Canceled`] once the
/// token is tripped, so a server can stop a scan it no longer needs instead of
/// letting it run to completion.
pub fn scan_project_cancelable(
    root: impl AsRef<Path>,
    options: &IndexOptions,
    cancel: &ScanCancellation,
) -> Result<CodeGraph, IndexError> {
    scan_project_with_scope(root.as_ref(), options, None, cancel)
}

pub fn scan_project_paths(
    root: impl AsRef<Path>,
    options: &IndexOptions,
    paths: &BTreeSet<String>,
) -> Result<CodeGraph, IndexError> {
    let scope = ScanScope::new(paths);
    scan_project_with_scope(
        root.as_ref(),
        options,
        Some(&scope),
        &ScanCancellation::none(),
    )
}

/// Whether a `Module` declaration in this language reopens a single shared
/// entity (so every declaring file should point at one node) rather than
/// declaring a distinct module per site (Rust `mod`, Elixir `defmodule`).
pub(crate) fn namespace_declaration_is_reopenable(language: Language) -> bool {
    matches!(language, Language::CSharp | Language::Php | Language::Ruby)
}

pub(crate) fn scan_project_with_scope(
    root: &Path,
    options: &IndexOptions,
    scope: Option<&ScanScope>,
    cancel: &ScanCancellation,
) -> Result<CodeGraph, IndexError> {
    let ignored_globs = compile_ignored_globs(&options.ignored_globs)?;
    // `scan .` named the repository "." and gave its node a stable id
    // derived from that, so the same project answered to two identities
    // depending on how its path was written. The directory has a name;
    // ask the filesystem for it.
    let canonical_root = root.canonicalize().ok();
    let root_label = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name != "." && *name != "..")
        .or_else(|| {
            canonical_root
                .as_deref()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
        })
        .unwrap_or(".");
    let cargo_workspace_dependencies = cargo_workspace_dependencies(root);
    let go_modules = go_module_roots(root, options, &ignored_globs);
    let npm_packages = npm_package_roots(root, options, &ignored_globs);
    let path_aliases = typescript_path_aliases(root, options, &ignored_globs);
    let dart_packages = dart_package_roots(root, options, &ignored_globs);
    let c_include_dirs = c_include_dirs(root, options, &ignored_globs);
    let julia_exports = julia_exported_names(root, options, &ignored_globs);
    let r_exports = r_exported_names(root, options, &ignored_globs);
    let custom_rules = custom_rules(root);
    let annotations = graph_annotations(root);
    let mut context = IndexContext {
        graph: CodeGraph::new(root_label),
        edge_keys: BTreeSet::new(),
        edge_keys_synced: 0,
        function_symbols: BTreeMap::new(),
        namespace_nodes: BTreeMap::new(),
        build_products: build_product_globs(root),
        pending_namespace_imports: Vec::new(),
        effect_entities: BTreeMap::new(),
        file_import_qualifiers: BTreeMap::new(),
        file_imported_names: BTreeMap::new(),
        file_wildcard_imports: BTreeSet::new(),
        type_symbols: BTreeMap::new(),
        file_nodes: BTreeMap::new(),
        directory_nodes: BTreeMap::new(),
        external_dependencies: BTreeMap::new(),
        unresolved_call_placeholders: BTreeMap::new(),
        cargo_workspace_dependencies,
        go_modules,
        npm_packages,
        path_aliases,
        own_package_ids: BTreeSet::new(),
        dart_packages,
        c_include_dirs,
        julia_exports,
        r_exports,
        custom_rules,
        annotations,
        pending_calls: Vec::new(),
        pending_type_references: Vec::new(),
        pending_local_imports: Vec::new(),
        pending_entrypoint_targets: Vec::new(),
        pending_route_handlers: Vec::new(),
        pending_file_routes: Vec::new(),
        string_constants: BTreeMap::new(),
        pending_computed_environment_reads: Vec::new(),
        pending_compose_config_targets: Vec::new(),
        pending_compose_volume_targets: Vec::new(),
        kubernetes_configs: BTreeMap::new(),
        kubernetes_services: BTreeMap::new(),
        pending_kubernetes_config_refs: Vec::new(),
        pending_kubernetes_service_refs: Vec::new(),
        pending_github_actions_local_actions: Vec::new(),
        pending_document_path_refs: Vec::new(),
        pending_document_symbol_refs: Vec::new(),
        sql_tables: BTreeMap::new(),
        sql_columns: BTreeMap::new(),
        pending_sql_foreign_keys: Vec::new(),
        pending_sql_query_table_refs: Vec::new(),
        pending_native_channel_handlers: Vec::new(),
        pending_sql_joins: Vec::new(),
        pending_sql_alter_refs: Vec::new(),
        sql_migrations: Vec::new(),
        pending_orm_table_refs: Vec::new(),
        pending_migration_dir_refs: Vec::new(),
        sql_migration_dirs: BTreeMap::new(),
        pending_mcp_local_refs: Vec::new(),
        parsed_ahead: BTreeMap::new(),
    };

    // The walk is read in two passes: what it found, and then what each
    // file holds. Reading a file into facts is the whole cost of a first
    // scan — 80% of it on terraform — and it is the one part that needs
    // nothing from the graph being built, so it happens on every core while
    // the graph itself is still assembled in one order.
    let mut walked: Vec<WalkedEntry> = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, &ignored_globs))
    {
        let entry = entry.map_err(|source| IndexError::Walk {
            path: root.to_path_buf(),
            source,
        })?;
        // Cooperative cancellation: checked per entry so a canceled scan stops
        // promptly instead of running to completion for a result nobody wants.
        cancel.check()?;
        let path = entry.path();

        if path == root {
            continue;
        }

        let Ok(relative_path) = path.strip_prefix(root) else {
            continue;
        };
        let label = relative_path.to_string_lossy().replace('\\', "/");

        if entry.file_type().is_dir() {
            if !scope.is_none_or(|scope| scope.includes_directory(&label)) {
                continue;
            }
            walked.push(WalkedEntry::Directory { label });
            continue;
        }

        if entry.file_type().is_file() {
            if !scope.is_none_or(|scope| scope.includes_file(&label)) {
                continue;
            }
            let oversized = entry
                .metadata()
                .ok()
                .map(|metadata| metadata.len())
                .filter(|bytes| *bytes > options.max_file_size);
            walked.push(WalkedEntry::File {
                path: path.to_path_buf(),
                label,
                oversized,
            });
        }
    }

    let mut unparsed = walked
        .iter()
        .filter_map(|entry| match entry {
            WalkedEntry::File {
                path,
                label,
                oversized: None,
            } => Some((path.clone(), label.clone())),
            _ => None,
        })
        .collect::<VecDeque<_>>();

    let indexed = std::thread::scope(|scope| -> Result<(), IndexError> {
        let mut reading_ahead: Option<
            std::thread::ScopedJoinHandle<'_, Vec<(String, ParsedSource)>>,
        > = None;
        for entry in &walked {
            cancel.check()?;
            match entry {
                WalkedEntry::Directory { label } => {
                    let id = context.graph.add_node(NodeKind::Directory, label);
                    context.directory_nodes.insert(label.to_string(), id);
                    context.graph.add_edge(
                        context.graph.root,
                        id,
                        EdgeKind::Contains,
                        Confidence::Exact,
                    );
                }
                WalkedEntry::File {
                    path,
                    label,
                    oversized: Some(bytes),
                } => {
                    if is_index_relevant_file(path) {
                        index_skipped_file(&mut context, path, label, *bytes, options);
                    }
                }
                WalkedEntry::File { path, label, .. } => {
                    if context.parsed_ahead.is_empty() {
                        // The round after this one is read while this one is
                        // assembled into the graph, so the cores are not idle
                        // for the part of a scan that has to happen in order.
                        let round = match reading_ahead.take() {
                            Some(handle) => handle.join().unwrap_or_default(),
                            None => parse_round(&next_round(&mut unparsed), options),
                        };
                        context.parsed_ahead.extend(round);
                        let upcoming = next_round(&mut unparsed);
                        if !upcoming.is_empty() {
                            reading_ahead =
                                Some(scope.spawn(move || parse_round(&upcoming, options)));
                        }
                    }
                    index_file(&mut context, path, label, options);
                }
            }
        }
        Ok(())
    });
    indexed?;

    // `.mcp.json` is a dotfile the walk skips by default, yet it declares
    // the repository's MCP tool servers. Probe the conventional root
    // location explicitly.
    if !options.include_hidden && !context.file_nodes.contains_key(".mcp.json") {
        let hidden_mcp = root.join(".mcp.json");
        let within_budget = hidden_mcp
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() <= options.max_file_size);
        if within_budget && scope.is_none_or(|scope| scope.includes_file(".mcp.json")) {
            index_file(&mut context, &hidden_mcp, ".mcp.json", options);
        }
    }

    cancel.check()?;
    // What the project calls itself, so nothing downstream mistakes its own
    // package for an outside dependency.
    if !context.own_package_ids.is_empty() {
        let own = context
            .own_package_ids
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let root_id = context.graph.root;
        add_node_metadata(&mut context.graph, root_id, "own_package_ids", own);
    }
    resolve_pending_calls(&mut context);
    resolve_pending_type_references(&mut context);
    resolve_pending_local_imports(&mut context);
    resolve_pending_namespace_imports(&mut context);
    resolve_pending_entrypoint_targets(&mut context);
    resolve_pending_route_handlers(&mut context);
    resolve_pending_file_routes(&mut context);
    resolve_pending_computed_environment_reads(&mut context);
    link_imports_to_package_hubs(&mut context);
    resolve_pending_compose_config_targets(&mut context);
    resolve_pending_compose_volume_targets(&mut context);
    resolve_pending_kubernetes_config_refs(&mut context);
    resolve_pending_kubernetes_service_refs(&mut context);
    resolve_pending_github_actions_local_actions(&mut context);
    resolve_pending_document_path_refs(&mut context);
    resolve_pending_document_symbol_refs(&mut context);
    annotate_document_backlinks(&mut context);
    resolve_pending_sql_foreign_keys(&mut context);
    resolve_pending_sql_query_table_refs(&mut context);
    resolve_pending_native_channel_handlers(&mut context);
    resolve_pending_sql_joins(&mut context);
    resolve_pending_sql_alter_refs(&mut context);
    resolve_sql_migration_order(&mut context);
    resolve_pending_orm_table_refs(&mut context);
    resolve_pending_migration_dir_refs(&mut context);
    resolve_pending_mcp_local_refs(&mut context);
    apply_graph_annotations(&mut context);
    apply_custom_rules(&mut context);
    annotate_stable_node_ids(&mut context.graph);

    Ok(context.graph)
}

#[derive(Debug)]
pub(crate) struct ScanScope {
    files: BTreeSet<String>,
    directories: BTreeSet<String>,
}

impl ScanScope {
    fn new(paths: &BTreeSet<String>) -> Self {
        let mut files = BTreeSet::new();
        let mut directories = BTreeSet::new();
        for path in paths {
            let normalized = normalize_scan_scope_path(path);
            if normalized.is_empty() {
                continue;
            }
            files.insert(normalized.clone());
            let mut prefix = String::new();
            for segment in normalized.split('/').take(normalized.matches('/').count()) {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(segment);
                directories.insert(prefix.clone());
            }
        }
        Self { files, directories }
    }

    fn includes_file(&self, path: &str) -> bool {
        self.files.contains(path)
    }

    fn includes_directory(&self, path: &str) -> bool {
        self.directories.contains(path)
    }
}

pub(crate) fn normalize_scan_scope_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }
    normalized
}

pub fn scan_coverage(
    root: impl AsRef<Path>,
    options: &IndexOptions,
) -> Result<ScanCoverageReport, IndexError> {
    let root = root.as_ref();
    let ignored_globs = compile_ignored_globs(&options.ignored_globs)?;
    let mut report = ScanCoverageReport {
        root: root.display().to_string(),
        include_hidden: options.include_hidden,
        include_ignored: options.include_ignored,
        max_file_size: options.max_file_size,
        ignored_names: options.ignored_names.iter().cloned().collect(),
        ignored_globs: options.ignored_globs.iter().cloned().collect(),
        directories_seen: 0,
        files_seen: 0,
        indexed_files: 0,
        skipped_large_files: 0,
        skipped_policy_entries: 0,
        skipped_hidden_entries: 0,
        skipped_ignored_name_entries: 0,
        skipped_ignored_glob_entries: 0,
        non_index_files: 0,
        seen_bytes: 0,
        indexed_bytes: 0,
        skipped_large_bytes: 0,
        languages: BTreeMap::new(),
    };

    let mut entries = WalkDir::new(root).sort_by_file_name().into_iter();
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|source| IndexError::Walk {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path == root {
            continue;
        }

        if entry.file_type().is_dir() {
            report.directories_seen += 1;
        } else if entry.file_type().is_file() {
            report.files_seen += 1;
        }

        // Mirror should_enter exactly: CI infrastructure under hidden dirs
        // (.github/workflows, .gitlab-ci.yml, ...) is indexed even without
        // include_hidden, so the coverage report must not count it as skipped.
        let exclusion = if !options.include_hidden && is_ci_infrastructure_path(path, root) {
            entry_exclusion_without_hidden(&entry, root, options, &ignored_globs)
        } else {
            entry_exclusion(&entry, root, options, &ignored_globs)
        };
        if let Some(exclusion) = exclusion {
            report.skipped_policy_entries += 1;
            match exclusion {
                EntryExclusion::Hidden => report.skipped_hidden_entries += 1,
                EntryExclusion::IgnoredName => report.skipped_ignored_name_entries += 1,
                EntryExclusion::IgnoredGlob => report.skipped_ignored_glob_entries += 1,
            }
            if entry.file_type().is_dir() {
                entries.skip_current_dir();
            }
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let bytes = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        report.seen_bytes += bytes;
        if !is_index_relevant_file(path) {
            report.non_index_files += 1;
            continue;
        }

        if bytes > options.max_file_size {
            report.skipped_large_files += 1;
            report.skipped_large_bytes += bytes;
            continue;
        }

        report.indexed_files += 1;
        report.indexed_bytes += bytes;
        if let Some(adapter) = adapter_for_path(path) {
            *report
                .languages
                .entry(adapter.language().to_string())
                .or_default() += 1;
        } else if is_markdown_document(path) {
            *report.languages.entry("markdown".to_string()).or_default() += 1;
        } else if is_sql_file(path) {
            *report.languages.entry("sql".to_string()).or_default() += 1;
        }
    }

    Ok(report)
}

pub(crate) fn index_skipped_file(
    context: &mut IndexContext,
    path: &Path,
    label: &str,
    bytes: u64,
    options: &IndexOptions,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("skipped".to_string(), "true".to_string());
    metadata.insert("skipped_reason".to_string(), "max_file_size".to_string());
    metadata.insert("file_size_bytes".to_string(), bytes.to_string());
    metadata.insert(
        "max_file_size_bytes".to_string(),
        options.max_file_size.to_string(),
    );
    add_skipped_file(context, path, label, metadata);
}

/// A file the scan records but does not read into facts: it still belongs to
/// the project, and saying so is more useful than leaving a hole where a file
/// was.
fn add_skipped_file(
    context: &mut IndexContext,
    path: &Path,
    label: &str,
    mut metadata: BTreeMap<String, String>,
) {
    if let Some(adapter) = adapter_for_path(path) {
        metadata.insert("language".to_string(), adapter.language().to_string());
    } else if is_markdown_document(path) {
        metadata.insert("language".to_string(), "markdown".to_string());
        metadata.insert("item_kind".to_string(), "document".to_string());
        metadata.insert("document_kind".to_string(), document_kind(path, label));
    } else if is_rst_document(path) {
        metadata.insert("language".to_string(), "rst".to_string());
        metadata.insert("item_kind".to_string(), "document".to_string());
        metadata.insert("document_kind".to_string(), document_kind(path, label));
    } else if is_notebook_path(path) {
        // A notebook holds a program, and the file is too large to read
        // here, so say what it is without guessing which language.
        metadata.insert("item_kind".to_string(), "notebook".to_string());
    } else if is_sql_file(path) {
        metadata.insert("language".to_string(), "sql".to_string());
        metadata.insert("item_kind".to_string(), "sql_schema".to_string());
    }

    let file_id = context
        .graph
        .add_node_with_metadata(NodeKind::File, label, None, metadata);
    context.file_nodes.insert(label.to_string(), file_id);
    context.graph.add_edge(
        context.graph.root,
        file_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
}

/// What one pass of the walk found, so the next pass can read files into
/// facts on every core before the graph is assembled in one order.
enum WalkedEntry {
    Directory {
        label: String,
    },
    File {
        path: PathBuf,
        label: String,
        /// The size of a file too large to read, which the graph records
        /// without reading.
        oversized: Option<u64>,
    },
}

/// How many files one round of reading holds at once. Bounded so a
/// repository of any size costs the same memory here.
const PARSE_AHEAD_FILES: usize = 1024;

/// Read the next files into facts, on as many threads as the machine has.
/// This is the one step of a scan that needs nothing from the graph.
/// The next files to read, taken off the queue.
fn next_round(unparsed: &mut VecDeque<(PathBuf, String)>) -> Vec<(PathBuf, String)> {
    unparsed
        .drain(..unparsed.len().min(PARSE_AHEAD_FILES))
        .collect::<Vec<_>>()
}

fn parse_round(round: &[(PathBuf, String)], options: &IndexOptions) -> Vec<(String, ParsedSource)> {
    if round.is_empty() {
        return Vec::new();
    }
    let threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .clamp(1, 16)
        .min(round.len());
    if threads < 2 {
        return round
            .iter()
            .filter_map(|(path, label)| preparse_file(path, label, options))
            .collect();
    }
    // One file can cost more than a hundred others — redis's `fast_float.h`
    // against a page of Lua — so a thread takes the next file when it is
    // free rather than a fixed share of them up front.
    let next = AtomicUsize::new(0);
    let mut parsed_round = Vec::new();
    std::thread::scope(|scope| {
        let handles = (0..threads)
            .map(|_| {
                let next = &next;
                scope.spawn(move || {
                    let mut parsed = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((path, label)) = round.get(index) else {
                            break;
                        };
                        parsed.extend(preparse_file(path, label, options));
                    }
                    parsed
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            if let Ok(parsed) = handle.join() {
                parsed_round.extend(parsed);
            }
        }
    });
    parsed_round
}

/// One file read into facts, exactly as [`index_file`] would read it.
fn preparse_file(
    path: &Path,
    label: &str,
    options: &IndexOptions,
) -> Option<(String, ParsedSource)> {
    let stamp = file_stamp(path);
    let source = fs::read(path).ok()?;
    if minified_line_length(&source).is_some() {
        return None;
    }
    let adapter = parsing_adapter(path, &source)?;
    Some((
        label.to_string(),
        (
            adapter.language(),
            parse_source_cached(options, stamp, label, &source, adapter),
        ),
    ))
}

/// The adapter that reads a file: its extension states one, a `.h` that
/// declares C++ means the other, and a script says so on its first line.
pub(crate) fn parsing_adapter(
    path: &Path,
    source: &[u8],
) -> Option<&'static dyn codegraph_parser::LanguageAdapter> {
    if let Some(adapter) = adapter_for_path(path) {
        // `.h` is C's extension, C++'s and Objective-C's alike, and the
        // extension is all the path can say. A header that declares a
        // namespace, a template, a class or an access section is C++:
        // parsing 21 such headers in the corpus as C++ produced fewer
        // errors every time, and redis's `fast_float.h` went from 1152 to
        // 150. One that declares an `@interface` is Objective-C, and that
        // is where a framework states the whole of what it offers.
        // Only a header's extension is ambiguous. `redis/src/networking.c`
        // writes `class = getClientType(c)` -- an assignment to a variable
        // named `class` -- and reading that as a C++ class declaration
        // parsed the file as C++, which put its `addReplyError` in another
        // language than the 132 C calls to it.
        if adapter.language() == Language::C
            && path_extension_is_ambiguous(path)
            && let Ok(text) = std::str::from_utf8(source)
        {
            if declares_objc(text)
                && let Some(objc) = adapter_for_language(Language::ObjectiveC)
            {
                return Some(objc);
            }
            if declares_cpp(text)
                && let Some(cpp) = adapter_for_language(Language::Cpp)
            {
                return Some(cpp);
            }
        }
        return Some(adapter);
    }
    std::str::from_utf8(source)
        .ok()
        .and_then(shebang_language)
        .and_then(adapter_for_language)
}

pub(crate) fn index_file(
    context: &mut IndexContext,
    path: &Path,
    label: &str,
    options: &IndexOptions,
) {
    let mut metadata = BTreeMap::new();
    // Stamp before reading: if the file changes between stat and read, the
    // cache holds newer content under an older stamp and the next scan
    // misses and reparses (self-healing). Stamping after the read could pin
    // stale content under the new stamp forever.
    let pre_read_stamp = file_stamp(path);
    let source_bytes = fs::read(path)
        .map_err(|error| {
            metadata.insert("read_error".to_string(), error.to_string());
        })
        .ok();
    // Minified code is a build product: its names are the minifier's, not
    // the project's. Alamofire's `docs/js/jquery.min.js` alone produced 2029
    // facts named after jQuery's internals, more than a third of everything
    // that project's own source declares.
    if let Some(source) = source_bytes.as_deref()
        && let Some(longest_line) = minified_line_length(source)
    {
        metadata.insert("skipped".to_string(), "true".to_string());
        metadata.insert("skipped_reason".to_string(), "minified".to_string());
        metadata.insert("longest_line_bytes".to_string(), longest_line.to_string());
        add_skipped_file(context, path, label, metadata);
        return;
    }

    let adapter = source_bytes
        .as_deref()
        .and_then(|source| parsing_adapter(path, source))
        .or_else(|| adapter_for_path(path));
    let language = adapter.map(|adapter| adapter.language());

    if let Some(language) = language {
        metadata.insert("language".to_string(), language.to_string());
    } else if is_markdown_document(path) {
        metadata.insert("language".to_string(), "markdown".to_string());
        metadata.insert("item_kind".to_string(), "document".to_string());
        metadata.insert("document_kind".to_string(), document_kind(path, label));
    } else if is_sql_file(path) {
        metadata.insert("language".to_string(), "sql".to_string());
        metadata.insert("item_kind".to_string(), "sql_schema".to_string());
    }

    if language.is_some_and(|language| language == Language::Dart)
        && let Some(generated_from) = dart_generated_source_name(label)
    {
        metadata.insert("generated".to_string(), "true".to_string());
        let sibling_exists = path
            .parent()
            .map(|parent| parent.join(generated_from.rsplit('/').next().unwrap_or(&generated_from)))
            .is_some_and(|sibling| sibling.is_file());
        if sibling_exists {
            metadata.insert("generated_from".to_string(), generated_from);
        }
    }

    // `foo.mli` states what `foo.ml` offers; a module with no interface
    // file offers everything, which is why this is `None` rather than an
    // empty set.
    let ocaml_interface_names = (language == Some(Language::OCaml))
        .then(|| ocaml_interface_names(path))
        .flatten();

    // Read ahead of the walk on every core; only a file the round did not
    // cover is read here.
    let parse_result = context.parsed_ahead.remove(label).or_else(|| {
        source_bytes.as_ref().and_then(|source| {
            // A notebook is JSON holding the program someone wrote in
            // cells, and the facts in it are the program's.
            if label.ends_with(".ipynb") {
                return parse_notebook(label, source).map(|parsed| (Language::Python, Ok(parsed)));
            }
            // A `.vue` or `.svelte` file holds a template, a script and a
            // style together; the script is the program, and every other
            // line is blanked so a fact keeps the line that holds it.
            if is_single_file_component(path) {
                return parse_single_file_component(label, source);
            }
            let adapter = adapter?;
            Some((
                adapter.language(),
                parse_source_cached(options, pre_read_stamp, label, source, adapter),
            ))
        })
    });

    let file_id = context
        .graph
        .add_node_with_metadata(NodeKind::File, label, None, metadata);
    context.file_nodes.insert(label.to_string(), file_id);
    // Next.js, Nuxt and SvelteKit declare a route by where the file sits.
    // Whether the project is written that way is stated in its manifest,
    // which the walk may not have reached yet.
    let declared_route = source_bytes
        .as_deref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|source| razor_page_route(label, source));
    if declared_route.is_some() || file_based_route(label).is_some() {
        context.pending_file_routes.push(PendingFileRoute {
            file: file_id,
            label: label.to_string(),
            declared: declared_route,
        });
    }
    context.graph.add_edge(
        context.graph.root,
        file_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );

    let source_text = fs::read_to_string(path).ok();
    let mut script_entrypoint = None;
    if let Some(source) = source_text.as_deref() {
        let string_lines = parse_result
            .as_ref()
            .and_then(|(_, parsed)| parsed.as_ref().ok())
            .map(|parsed| parsed.string_line_ranges.as_slice())
            .unwrap_or_default();
        index_rationale_comments(context, file_id, label, language, source, string_lines);
        script_entrypoint = index_script_entrypoint(context, file_id, label, source);
        index_manifest_facts(context, file_id, path, label, source, language);
        index_markdown_document(context, file_id, path, label, source);
        index_rst_document(context, file_id, path, label, source);
        index_asciidoc_document(context, file_id, path, label, source);
        index_plain_text_document(context, file_id, path, label, source);
        index_sql_schema(context, file_id, path, label, source);
        index_framework_configs(context, file_id, label, language, source);
        if language == Some(Language::Dart) {
            index_dart_platform_channels(context, file_id, label, source);
        }
        index_native_platform_channels(context, file_id, label, source);
        index_orm_table_refs(context, file_id, label, source);
        index_migration_dir_refs(context, file_id, label, source);
        index_mcp_config(context, file_id, label, source);
        if let Some(language) = language {
            index_commonjs_require_imports(context, file_id, label, language, source);
        }
        index_notebook_run_imports(context, file_id, path, label, source);
    }

    if let Some((language, parse_result)) = parse_result {
        match parse_result {
            Ok(parsed) => {
                if parsed.has_error_nodes {
                    add_file_metadata(&mut context.graph, file_id, "syntax_errors", "true");
                    if let Some(line) = parsed.first_error_line {
                        add_file_metadata(
                            &mut context.graph,
                            file_id,
                            "syntax_error_line",
                            line.to_string(),
                        );
                    }
                }

                let mut local_functions = BTreeMap::new();
                // Several definitions in one file can answer to one name:
                // flask writes `locate_app` three times, twice as a
                // `@t.overload` stub with no body. Keeping every span lets
                // a fact go to the definition that contains it.
                let mut local_function_spans: Vec<(String, NodeId, SourceSpan)> = Vec::new();
                let test_cutoff = source_text
                    .as_deref()
                    .and_then(|source| rust_test_module_cutoff(language, source));
                for item in parsed.items.iter().filter(|item| is_symbol_item(item.kind)) {
                    let node_kind = match item.kind {
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint => NodeKind::Function,
                        ParsedItemKind::Type => NodeKind::Type,
                        ParsedItemKind::Module => NodeKind::Module,
                        ParsedItemKind::Import => NodeKind::ExternalDependency,
                        ParsedItemKind::Call
                        | ParsedItemKind::EnvironmentRead
                        | ParsedItemKind::ConfigRead
                        | ParsedItemKind::Error
                        | ParsedItemKind::Branch
                        | ParsedItemKind::Loop
                        | ParsedItemKind::Async
                        | ParsedItemKind::Return => {
                            unreachable!("non-symbol facts are processed separately")
                        }
                    };
                    let mut item_metadata = BTreeMap::new();
                    item_metadata.extend(item.metadata.clone());
                    // `if TYPE_CHECKING:` holds imports a type checker reads
                    // and the interpreter never runs, so what they name is
                    // not a dependency at run time: requests writes its
                    // `_types.py` that way and reads as a cycle otherwise.
                    if item.kind == ParsedItemKind::Import
                        && language == Language::Python
                        && source_text.as_deref().is_some_and(|source| {
                            line_is_type_checking_only(source, item.span.start_line)
                        })
                    {
                        item_metadata.insert("type_only".to_string(), "true".to_string());
                    }
                    // TypeScript writes the same thing in the import
                    // itself. `import type { PropsExpression } from
                    // './transforms/transformElement'` is erased before
                    // anything runs, and six of vue's ten cycles were
                    // closed by one of these.
                    if item.kind == ParsedItemKind::Import
                        && matches!(
                            language,
                            Language::TypeScript | Language::Tsx | Language::JavaScript
                        )
                        && item.label.trim_start().starts_with("import type ")
                    {
                        item_metadata.insert("type_only".to_string(), "true".to_string());
                    }
                    // `try: import cryptography / except ImportError:` says
                    // the program runs without the package, which is what an
                    // optional dependency is.
                    if item.kind == ParsedItemKind::Import
                        && language == Language::Python
                        && source_text.as_deref().is_some_and(|source| {
                            line_is_a_guarded_import(source, item.span.start_line)
                        })
                    {
                        item_metadata.insert("optional".to_string(), "true".to_string());
                    }
                    // A Dart `part` and the file it belongs to are one
                    // library written across two files, the way a header and
                    // its source are: `frame_reader.dart` says `part of
                    // 'frames.dart'` and `frames.dart` says `part
                    // 'frame_reader.dart'`, and that is not two files
                    // depending on each other.
                    if item.kind == ParsedItemKind::Import
                        && language == Language::Dart
                        && (item.label.starts_with("part ") || item.label.starts_with("part of"))
                    {
                        item_metadata.insert("import_form".to_string(), "part".to_string());
                    }
                    item_metadata.insert("language".to_string(), language.to_string());
                    item_metadata.insert("parser".to_string(), "tree-sitter".to_string());
                    item_metadata.insert(
                        "item_kind".to_string(),
                        parsed_item_kind_name(item.kind).to_string(),
                    );
                    // Julia and R write their package's exports in one
                    // place, away from the files that define the
                    // functions: an `export` statement in the module file,
                    // and the NAMESPACE beside the R package.
                    if matches!(
                        item.kind,
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint
                    ) {
                        let exports = match language {
                            Language::Julia => Some(&context.julia_exports),
                            Language::R => Some(&context.r_exports),
                            _ => None,
                        };
                        if let Some(exports) = exports.filter(|exports| !exports.is_empty()) {
                            item_metadata.insert(
                                "visibility".to_string(),
                                if exports.contains(&item.label) {
                                    "public".to_string()
                                } else {
                                    "private".to_string()
                                },
                            );
                        }
                    }
                    // What an OCaml module lets out is written in the file
                    // beside it, which the parser never sees: `foo.mli`
                    // lists what `foo.ml` offers, and a module with no
                    // interface offers everything.
                    if language == Language::OCaml
                        && matches!(
                            item.kind,
                            ParsedItemKind::Function | ParsedItemKind::Entrypoint
                        )
                    {
                        let visibility = match ocaml_interface_names.as_ref() {
                            // A module with no interface file offers all of
                            // itself.
                            None => "public",
                            Some(interface) if interface.contains(&item.label) => "public",
                            Some(_) => "private",
                        };
                        item_metadata.insert("visibility".to_string(), visibility.to_string());
                    }
                    // A `main` the parser recognised is an entrypoint like any
                    // other, and every other kind says what it is. Without this
                    // it was the only nameless one: the wiki filed programs
                    // under "other" and no surface could filter for them.
                    if item.kind == ParsedItemKind::Entrypoint {
                        item_metadata.insert("entrypoint_kind".to_string(), "program".to_string());
                        item_metadata.insert("source".to_string(), "code".to_string());
                    }
                    let local_import = if item.kind == ParsedItemKind::Import {
                        local_import_target(
                            language,
                            label,
                            &item.label,
                            &context.c_include_dirs,
                            &context.dart_packages,
                            &context.path_aliases,
                        )
                    } else {
                        None
                    };
                    let possible_local_import =
                        if local_import.is_none() && item.kind == ParsedItemKind::Import {
                            possible_local_import_target(
                                language,
                                label,
                                &item.label,
                                &context.go_modules,
                                &context.dart_packages,
                                &context.npm_packages,
                            )
                        } else {
                            None
                        };
                    // Remember what each qualifier binds, so `states.NewState`
                    // can be resolved inside the package the file imports
                    // instead of against every `NewState` in the project.
                    let js_bindings = matches!(
                        language,
                        Language::JavaScript | Language::TypeScript | Language::Tsx
                    )
                    .then(|| js_import_bindings(&item.label));
                    let import_qualifier = match language {
                        Language::Go => go_import_qualifier(&item.label),
                        Language::Python => python_import_qualifier(&item.label),
                        // A Lua require is an expression, so the name it
                        // binds is written to its left rather than in the
                        // call: the parser records it.
                        Language::Lua => item.metadata.get("binds").cloned(),
                        _ => js_bindings
                            .as_ref()
                            .and_then(|bindings| bindings.qualifier.clone()),
                    };
                    if item.kind == ParsedItemKind::Import {
                        let package = local_import
                            .as_ref()
                            .or(possible_local_import.as_ref())
                            .map(|target| target.candidates.clone())
                            .filter(|candidates| !candidates.is_empty())
                            .map_or(ImportedPackage::External, ImportedPackage::Local);
                        if let Some(qualifier) = import_qualifier {
                            context
                                .file_import_qualifiers
                                .entry(label.to_string())
                                .or_default()
                                .insert(qualifier, package.clone());
                        }
                        // `from x import *` binds every name the other
                        // module defines, so this file's import list can
                        // never say what a bare call means.
                        if item.label.trim_end().ends_with("import *") {
                            context.file_wildcard_imports.insert(label.to_string());
                        }
                        let bound_names = match language {
                            Language::Python => python_imported_names(&item.label),
                            // `import static ..Truth.assertThat` binds a
                            // bare name the same way python's `from x
                            // import y` does.
                            Language::Java => java_static_imported_names(&item.label),
                            _ => js_bindings
                                .map(|bindings| bindings.names)
                                .unwrap_or_default(),
                        };
                        for name in bound_names {
                            context
                                .file_imported_names
                                .entry(label.to_string())
                                .or_default()
                                .insert(name, package.clone());
                        }
                    }

                    // An import the scan resolved inside the repository is
                    // not an outside dependency, however it was found: a
                    // workspace package like `@vue/runtime-test` lives in
                    // packages/ and reaches here through the package map
                    // rather than through a relative path.
                    if let Some(local_import) = local_import.as_ref() {
                        item_metadata.insert("import_scope".to_string(), "local".to_string());
                        item_metadata
                            .insert("import_target".to_string(), local_import.target.clone());
                        item_metadata.insert("resolution".to_string(), "pending".to_string());
                    }
                    // An import below `#[cfg(test)]` is written for the
                    // test build, whether or not the scan could tell where
                    // it points yet: ripgrep's `use crate::testutil::..`
                    // is resolved later, through the module path.
                    if item.kind == ParsedItemKind::Import
                        && test_cutoff.is_some_and(|cutoff| item.span.start_line >= cutoff)
                    {
                        item_metadata.insert("test_context".to_string(), "true".to_string());
                    }

                    // A namespace declaration names one entity that many files
                    // reopen (C# `namespace`, PHP `namespace`, Ruby `module`).
                    // Reuse one canonical node instead of emitting a duplicate
                    // per declaring file, which split the namespace's edges
                    // across hundreds of look-alike nodes.
                    let namespace_key = (item.kind == ParsedItemKind::Module
                        && namespace_declaration_is_reopenable(language))
                    .then(|| (language.name(), item.label.clone()));
                    let existing_namespace = namespace_key
                        .as_ref()
                        .and_then(|key| context.namespace_nodes.get(key).copied());
                    let item_id = match existing_namespace {
                        Some(existing) => existing,
                        None => {
                            if namespace_key.is_some() {
                                item_metadata
                                    .insert("declaration_scope".to_string(), "shared".to_string());
                            }
                            let id = context.graph.add_node_with_metadata(
                                node_kind,
                                item.label.clone(),
                                Some(item.span.clone()),
                                item_metadata,
                            );
                            if let Some(key) = namespace_key {
                                context.namespace_nodes.insert(key, id);
                            }
                            id
                        }
                    };
                    let edge_kind = match item.kind {
                        ParsedItemKind::Import => EdgeKind::Imports,
                        _ => EdgeKind::Contains,
                    };
                    // A file holds the code its own lines hold. The second
                    // file to reopen a namespace shares the first one's node,
                    // and saying it *contains* that node put another file's
                    // span inside it: koel's 1020 `contains` edges and
                    // mastodon's 122 crossed files that way. What this file
                    // does is declare the namespace again.
                    if existing_namespace.is_some() && edge_kind == EdgeKind::Contains {
                        let mut metadata = BTreeMap::new();
                        metadata.insert("relation".to_string(), "declares_namespace".to_string());
                        add_edge_once_with_metadata(
                            context,
                            file_id,
                            item_id,
                            EdgeKind::References,
                            Confidence::Syntactic,
                            metadata,
                        );
                    } else {
                        context
                            .graph
                            .add_edge(file_id, item_id, edge_kind, Confidence::Syntactic);
                    }
                    if let Some(local_import) = local_import {
                        context.pending_local_imports.push(PendingLocalImport {
                            import_node: item_id,
                            target: local_import.target,
                            candidates: local_import.candidates,
                            mark_unresolved: true,
                            allow_suffix_fallback: language != Language::Rust,
                        });
                    } else if let Some(namespace) = (item.kind == ParsedItemKind::Import)
                        .then(|| csharp_namespace_import(language, &item.label))
                        .flatten()
                    {
                        // `using Polly.Telemetry;` names a namespace, and the
                        // project declares 92 of them; the declaration may be
                        // in a file this walk has not reached yet.
                        context
                            .pending_namespace_imports
                            .push(PendingNamespaceImport {
                                import_node: item_id,
                                language: language.name(),
                                namespace,
                            });
                    } else if let Some(possible_local_import) = possible_local_import {
                        context.pending_local_imports.push(PendingLocalImport {
                            import_node: item_id,
                            target: possible_local_import.target,
                            candidates: possible_local_import.candidates,
                            mark_unresolved: false,
                            allow_suffix_fallback: language != Language::Rust,
                        });
                    }

                    if item.kind == ParsedItemKind::Entrypoint {
                        context.graph.add_edge(
                            context.graph.root,
                            item_id,
                            EdgeKind::Entrypoint,
                            Confidence::Syntactic,
                        );
                    }

                    if matches!(
                        item.kind,
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint
                    ) {
                        register_function_symbol(
                            &mut context.function_symbols,
                            &item.label,
                            item_id,
                        );
                        register_local_function(&mut local_functions, &item.label, item_id);
                        local_function_spans.push((item.label.clone(), item_id, item.span.clone()));
                    }
                    // A Ruby module is a constant other files name: `include
                    // Payloadable` and `Mastodon::CLI::Maintenance::Account`
                    // reach one the same way a class is reached.
                    // A Ruby module and an Elixir one are constants other
                    // files name: `include Payloadable` and `alias
                    // Ecto.Changeset` reach one the way a class is reached.
                    if item.kind == ParsedItemKind::Type
                        || (item.kind == ParsedItemKind::Module
                            && matches!(
                                language,
                                Language::Ruby | Language::Elixir | Language::Erlang
                            ))
                    {
                        register_function_symbol(&mut context.type_symbols, &item.label, item_id);
                    }
                    // A configuration's declarations are what its facts sit
                    // inside and what its expressions refer to, the way
                    // functions and types are in a programming language.
                    if matches!(
                        language,
                        Language::Hcl | Language::Proto | Language::GraphQl | Language::Solidity
                    ) && matches!(item.kind, ParsedItemKind::Type | ParsedItemKind::Module)
                    {
                        register_local_function(&mut local_functions, &item.label, item_id);
                        local_function_spans.push((item.label.clone(), item_id, item.span.clone()));
                        if item.kind == ParsedItemKind::Module {
                            register_function_symbol(
                                &mut context.type_symbols,
                                &item.label,
                                item_id,
                            );
                        }
                    }
                }

                if let Some(entrypoint_id) = script_entrypoint
                    && let Some(main_id) = resolve_local_function(&local_functions, "main")
                {
                    add_entrypoint_reference(
                        context,
                        entrypoint_id,
                        main_id,
                        "entrypoint_function",
                        "shebang_main",
                        Confidence::Syntactic,
                        Some("main"),
                    );
                }

                // What the file binds a name to, for the environment reads
                // that name a constant rather than a variable.
                for (name, value) in &parsed.string_constants {
                    match context.string_constants.get(name) {
                        Some(Some(existing)) if existing != value => {
                            context.string_constants.insert(name.clone(), None);
                        }
                        Some(_) => {}
                        None => {
                            context
                                .string_constants
                                .insert(name.clone(), Some(value.clone()));
                        }
                    }
                }

                if let Some(source) = source_text.as_deref() {
                    index_framework_routes(
                        context,
                        file_id,
                        label,
                        language,
                        source,
                        &parsed,
                        &local_functions,
                    );
                    index_inline_sql_queries(
                        context,
                        file_id,
                        label,
                        language,
                        source,
                        &parsed,
                        &local_functions,
                    );
                    index_embedded_sql_schema(context, file_id, path, label, source, &parsed);
                }

                // One edge per read site, so two reads of the same key in one
                // function stay two facts: `add_edge_once` would keep only the
                // first, hiding a key that is read once with a fallback and
                // once without it.
                let mut effect_read_sites: BTreeSet<(NodeId, NodeId, EdgeKind, u32)> =
                    BTreeSet::new();
                for item in parsed.items.iter().filter(|item| is_effect_item(item.kind)) {
                    let source_id = item
                        .parent
                        .as_deref()
                        .and_then(|parent| {
                            enclosing_local_function(&local_function_spans, parent, &item.span)
                                .or_else(|| resolve_local_function(&local_functions, parent))
                        })
                        .unwrap_or(file_id);
                    let node_kind = match item.kind {
                        ParsedItemKind::EnvironmentRead => NodeKind::Environment,
                        ParsedItemKind::ConfigRead => NodeKind::Config,
                        ParsedItemKind::Error
                        | ParsedItemKind::Branch
                        | ParsedItemKind::Loop
                        | ParsedItemKind::Async
                        | ParsedItemKind::Return => NodeKind::ControlFlow,
                        _ => unreachable!("only effect facts are processed here"),
                    };
                    let edge_kind = match item.kind {
                        ParsedItemKind::EnvironmentRead => EdgeKind::ReadsEnvironment,
                        ParsedItemKind::ConfigRead => EdgeKind::ReadsConfig,
                        ParsedItemKind::Error => EdgeKind::MayError,
                        ParsedItemKind::Branch
                        | ParsedItemKind::Loop
                        | ParsedItemKind::Async
                        | ParsedItemKind::Return => EdgeKind::References,
                        _ => unreachable!("only effect facts are processed here"),
                    };
                    let mut item_metadata = BTreeMap::new();
                    item_metadata.extend(item.metadata.clone());
                    item_metadata.insert("language".to_string(), language.to_string());
                    item_metadata.insert(
                        "item_kind".to_string(),
                        parsed_item_kind_name(item.kind).to_string(),
                    );
                    if let Some(parent) = item.parent.as_deref() {
                        item_metadata.insert("parent".to_string(), parent.to_string());
                    }

                    // An environment variable or config key names one entity
                    // the whole project shares, so every read points at the
                    // same node. Emitting a node per read split a key's
                    // readers across look-alike duplicates (kong: 1211 nodes
                    // for 371 keys), and the per-read facts — default value,
                    // language, position — belong to the read, not the key:
                    // they travel on the edge instead.
                    let entity_kind = match item.kind {
                        ParsedItemKind::EnvironmentRead => Some("environment"),
                        ParsedItemKind::ConfigRead => Some("config"),
                        _ => None,
                    };

                    let Some(entity_kind) = entity_kind else {
                        item_metadata.insert("parser".to_string(), "tree-sitter".to_string());
                        let item_id = context.graph.add_node_with_metadata(
                            node_kind,
                            item.label.clone(),
                            Some(item.span.clone()),
                            item_metadata,
                        );
                        add_edge_once(
                            context,
                            source_id,
                            item_id,
                            edge_kind,
                            Confidence::Heuristic,
                        );
                        continue;
                    };

                    // A read whose key is a name waits for the file that
                    // binds it: `os.Getenv(envLogFile)` is `TF_LOG_PATH`
                    // wherever terraform declares that constant, and 45 of
                    // its 62 computed reads name one.
                    if entity_kind == "environment"
                        && item.label == codegraph_core::COMPUTED_ENVIRONMENT_KEY
                        && let Some(expression) = item.metadata.get("key_expression")
                    {
                        item_metadata.insert("file".to_string(), label.to_string());
                        item_metadata.insert("line".to_string(), item.span.start_line.to_string());
                        context.pending_computed_environment_reads.push(
                            PendingComputedEnvironmentRead {
                                source: source_id,
                                span: item.span.clone(),
                                metadata: item_metadata,
                                key_expression: expression.clone(),
                            },
                        );
                        continue;
                    }

                    let item_id = shared_effect_entity(
                        context,
                        entity_kind,
                        node_kind,
                        &item.label,
                        item.span.clone(),
                        BTreeMap::from([
                            ("parser".to_string(), "tree-sitter".to_string()),
                            (
                                "item_kind".to_string(),
                                parsed_item_kind_name(item.kind).to_string(),
                            ),
                        ]),
                    );
                    let line = item.span.start_line;
                    if !effect_read_sites.insert((source_id, item_id, edge_kind, line)) {
                        continue;
                    }
                    item_metadata.insert("file".to_string(), label.to_string());
                    item_metadata.insert("line".to_string(), line.to_string());
                    context.graph.add_edge_with_metadata(
                        source_id,
                        item_id,
                        edge_kind,
                        Confidence::Heuristic,
                        item_metadata,
                    );
                }

                for item in parsed
                    .items
                    .iter()
                    .filter(|item| item.kind == ParsedItemKind::Call)
                {
                    // A call at module level has no enclosing definition;
                    // the file itself is what runs it -- unless a
                    // definition's span covers it without the walk naming
                    // it, which is what a C# file of top-level statements
                    // is: the program is every statement outside a
                    // declaration, and its calls are the program's.
                    let caller = item
                        .parent
                        .as_deref()
                        .and_then(|parent| {
                            enclosing_local_function(&local_function_spans, parent, &item.span)
                                .or_else(|| resolve_local_function(&local_functions, parent))
                        })
                        .or_else(|| covering_local_definition(&local_function_spans, &item.span))
                        .unwrap_or(file_id);
                    context.pending_calls.push(PendingCall {
                        caller,
                        label: item.label.clone(),
                        span: item.span.clone(),
                        language: language.to_string(),
                        receiver_type: item.metadata.get("receiver_type").cloned(),
                        receiver: item.metadata.get("receiver").cloned(),
                        callee_is_value: item.metadata.get("callee_form").map(String::as_str)
                            == Some("value"),
                        receiver_is_a_value: item.metadata.get("receiver_form").map(String::as_str)
                            == Some("value"),
                    });
                }

                for reference in &parsed.type_references {
                    let source = reference
                        .parent
                        .as_deref()
                        .and_then(|parent| {
                            enclosing_local_function(&local_function_spans, parent, &reference.span)
                                .or_else(|| resolve_local_function(&local_functions, parent))
                        })
                        .unwrap_or(file_id);
                    context.pending_type_references.push(PendingTypeReference {
                        source,
                        label: reference.label.clone(),
                        language: language.to_string(),
                        span: reference.span.clone(),
                    });
                }
            }
            Err(error) => add_file_metadata(
                &mut context.graph,
                file_id,
                "parse_error",
                error.to_string(),
            ),
        }
    }
}

/// The names an OCaml interface file states, when there is one beside the
/// module: `val concat : string -> string -> string` in `filename.mli`
/// says `concat` is what `filename.ml` offers. A module with no interface
/// offers everything, and answers `None`.
fn ocaml_interface_names(path: &Path) -> Option<BTreeSet<String>> {
    let interface = path.with_extension("mli");
    let text = fs::read_to_string(interface).ok()?;
    // `include Io_intf.S` brings in names written in another file, which
    // this cannot enumerate. 125 of dune's 931 interfaces do it, and
    // guessing that what it cannot see is private would call `read_file`
    // hidden when the interface plainly offers it.
    if text
        .lines()
        .any(|line| line.trim_start().starts_with("include "))
    {
        return None;
    }
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("val ") else {
            continue;
        };
        // An operator is declared as `val (>>=) : ...` or `val ( let+ ) :
        // ...` and defined the same way, parentheses, spaces and all, so
        // the name runs to the closing bracket rather than to the first
        // space.
        let rest = rest.trim_start();
        let name = match rest.strip_prefix('(') {
            Some(after) => after
                .split_once(')')
                .map(|(inner, _)| format!("({inner})"))
                .unwrap_or_default(),
            None => rest
                .split([':', ' ', '\t'])
                .next()
                .unwrap_or_default()
                .trim()
                .to_string(),
        };
        if !name.is_empty() {
            names.insert(name);
        }
    }
    Some(names)
}

/// The names a Julia package exports. `export` opens a list that runs
/// until a line does not end in a comma, and the list sits in the module
/// file while the functions live in the files it includes.
fn julia_exported_names(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("jl") {
            continue;
        }
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let mut collecting = false;
        for line in text.lines() {
            let trimmed = line.trim();
            let body = if collecting {
                trimmed
            } else if let Some(rest) = trimmed.strip_prefix("export ") {
                rest
            } else {
                continue;
            };
            for name in body.split(',') {
                let name = name.trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
            collecting = body.ends_with(',');
        }
    }
    names
}

/// The names an R package's NAMESPACE lists: `export(mutate)` and the
/// generics behind `S3method("[", grouped_df)`.
fn r_exported_names(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) != Some("NAMESPACE") {
            continue;
        }
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed
                .strip_prefix("export(")
                .or_else(|| trimmed.strip_prefix("exportMethods("))
            else {
                continue;
            };
            let Some((inside, _)) = rest.split_once(')') else {
                continue;
            };
            for name in inside.split(',') {
                let name = name.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

/// Whether a header written for either language is C++. C has no
/// namespaces, no templates, no classes and no access sections, so a line
/// opening one settles it.
/// Whether a header is Objective-C's. `.h` is C's extension, C++'s and
/// Objective-C's alike, and only what the file states says which: no other
/// language writes `@interface`, `@protocol` or `@property`.
fn declares_objc(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("@interface")
            || trimmed.starts_with("@protocol")
            || trimmed.starts_with("@property")
            || trimmed.starts_with("@implementation")
    })
}

fn declares_cpp(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        // A declaration, not an assignment: C has no `class` keyword, so a
        // line that opens with one and then assigns is a variable named
        // `class` in a header shared with C code.
        let declares = |keyword: &str| {
            trimmed
                .strip_prefix(keyword)
                .is_some_and(|rest| !rest.trim_start().starts_with('='))
        };
        trimmed.starts_with("namespace ")
            || trimmed.starts_with("template <")
            || trimmed.starts_with("template<")
            || declares("class ")
            || trimmed.starts_with("public:")
            || trimmed.starts_with("private:")
    })
}

/// Whether a path's extension leaves the language open. `.h` is C's, C++'s
/// and Objective-C's alike; `.c` is C's.
fn path_extension_is_ambiguous(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("h") | Some("inc")
    )
}

/// The definition of `name` in this file whose body holds `span`. Several
/// can share a name -- a Python `@t.overload` stub and the implementation
/// under it -- and the fact belongs to the one it was written inside.
/// The definition whose span covers a fact the walk could not name. A C#
/// file of top-level statements declares no function around them, and the
/// program the compiler generates from them is exactly that span: without
/// this, eShopOnWeb's three programs reached nothing at all.
fn covering_local_definition(
    definitions: &[(String, NodeId, SourceSpan)],
    span: &SourceSpan,
) -> Option<NodeId> {
    definitions
        .iter()
        .filter(|(_, _, definition)| {
            definition.path == span.path
                && definition.start_line <= span.start_line
                && span.start_line <= definition.end_line
        })
        .min_by_key(|(_, _, definition)| definition.end_line - definition.start_line)
        .map(|(_, id, _)| *id)
}

fn enclosing_local_function(
    definitions: &[(String, NodeId, SourceSpan)],
    name: &str,
    span: &SourceSpan,
) -> Option<NodeId> {
    definitions
        .iter()
        .filter(|(label, _, definition)| {
            label == name
                && definition.path == span.path
                && definition.start_line <= span.start_line
                && span.start_line <= definition.end_line
        })
        // The innermost definition wins, as a nested helper does over the
        // function that holds it.
        .min_by_key(|(_, _, definition)| definition.end_line - definition.start_line)
        .map(|(_, id, _)| *id)
}
