//! Project walking and per-file indexing: the scan entrypoints, scan
//! coverage reporting, and the file-level fact dispatcher.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeKind};
use codegraph_parser::{Language, ParsedItemKind, adapter_for_language, adapter_for_path};
use walkdir::WalkDir;

#[allow(unused_imports)]
use crate::*;

pub fn scan_project(
    root: impl AsRef<Path>,
    options: &IndexOptions,
) -> Result<CodeGraph, IndexError> {
    scan_project_with_scope(root.as_ref(), options, None)
}

pub fn scan_project_paths(
    root: impl AsRef<Path>,
    options: &IndexOptions,
    paths: &BTreeSet<String>,
) -> Result<CodeGraph, IndexError> {
    let scope = ScanScope::new(paths);
    scan_project_with_scope(root.as_ref(), options, Some(&scope))
}

pub(crate) fn scan_project_with_scope(
    root: &Path,
    options: &IndexOptions,
    scope: Option<&ScanScope>,
) -> Result<CodeGraph, IndexError> {
    let ignored_globs = compile_ignored_globs(&options.ignored_globs)?;
    let root_label = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".");
    let cargo_workspace_dependencies = cargo_workspace_dependencies(root);
    let go_modules = go_module_roots(root, options, &ignored_globs);
    let dart_packages = dart_package_roots(root, options, &ignored_globs);
    let c_include_dirs = c_include_dirs(root, options, &ignored_globs);
    let custom_rules = custom_rules(root);
    let annotations = graph_annotations(root);
    let mut context = IndexContext {
        graph: CodeGraph::new(root_label),
        edge_keys: BTreeSet::new(),
        edge_keys_synced: 0,
        function_symbols: BTreeMap::new(),
        type_symbols: BTreeMap::new(),
        file_nodes: BTreeMap::new(),
        directory_nodes: BTreeMap::new(),
        external_dependencies: BTreeMap::new(),
        unresolved_call_placeholders: BTreeMap::new(),
        cargo_workspace_dependencies,
        go_modules,
        dart_packages,
        c_include_dirs,
        custom_rules,
        annotations,
        pending_calls: Vec::new(),
        pending_type_references: Vec::new(),
        pending_local_imports: Vec::new(),
        pending_entrypoint_targets: Vec::new(),
        pending_route_handlers: Vec::new(),
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
    };

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, &ignored_globs))
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
            if !scope.is_none_or(|scope| scope.includes_directory(&label)) {
                continue;
            }
            let id = context.graph.add_node(NodeKind::Directory, &label);
            context.directory_nodes.insert(label.to_string(), id);
            context.graph.add_edge(
                context.graph.root,
                id,
                EdgeKind::Contains,
                Confidence::Exact,
            );
            continue;
        }

        if entry.file_type().is_file() {
            if !scope.is_none_or(|scope| scope.includes_file(&label)) {
                continue;
            }
            match entry.metadata() {
                Ok(metadata) if metadata.len() > options.max_file_size => {
                    if is_index_relevant_file(path) {
                        index_skipped_file(&mut context, path, &label, metadata.len(), options);
                    }
                }
                _ => index_file(&mut context, path, &label, options),
            }
        }
    }

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

    resolve_pending_calls(&mut context);
    resolve_pending_type_references(&mut context);
    resolve_pending_local_imports(&mut context);
    resolve_pending_entrypoint_targets(&mut context);
    resolve_pending_route_handlers(&mut context);
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

    let mut entries = WalkDir::new(root).into_iter();
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

        if let Some(exclusion) = entry_exclusion(&entry, root, options, &ignored_globs) {
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
    if let Some(adapter) = adapter_for_path(path) {
        metadata.insert("language".to_string(), adapter.language().to_string());
    } else if is_markdown_document(path) {
        metadata.insert("language".to_string(), "markdown".to_string());
        metadata.insert("item_kind".to_string(), "document".to_string());
        metadata.insert("document_kind".to_string(), document_kind(path, label));
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

pub(crate) fn index_file(
    context: &mut IndexContext,
    path: &Path,
    label: &str,
    options: &IndexOptions,
) {
    let mut metadata = BTreeMap::new();
    let source_bytes = fs::read(path)
        .map_err(|error| {
            metadata.insert("read_error".to_string(), error.to_string());
        })
        .ok();
    let adapter = adapter_for_path(path);
    let language = adapter.map(|adapter| adapter.language()).or_else(|| {
        source_bytes
            .as_deref()
            .and_then(|source| std::str::from_utf8(source).ok())
            .and_then(shebang_language)
    });

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

    let parse_result = source_bytes.as_ref().and_then(|source| {
        let adapter = adapter.or_else(|| language.and_then(adapter_for_language))?;
        Some((
            adapter.language(),
            parse_source_cached(options, path, label, source, adapter),
        ))
    });

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

    let source_text = fs::read_to_string(path).ok();
    let mut script_entrypoint = None;
    if let Some(source) = source_text.as_deref() {
        index_rationale_comments(context, file_id, label, language, source);
        script_entrypoint = index_script_entrypoint(context, file_id, label, source);
        index_manifest_facts(context, file_id, path, label, source);
        index_markdown_document(context, file_id, path, label, source);
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
    }

    if let Some((language, parse_result)) = parse_result {
        match parse_result {
            Ok(parsed) => {
                if parsed.has_error_nodes {
                    add_file_metadata(&mut context.graph, file_id, "syntax_errors", "true");
                }

                let mut local_functions = BTreeMap::new();
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
                    item_metadata.insert("language".to_string(), language.to_string());
                    item_metadata.insert("parser".to_string(), "tree-sitter".to_string());
                    item_metadata.insert(
                        "item_kind".to_string(),
                        parsed_item_kind_name(item.kind).to_string(),
                    );
                    let local_import = if item.kind == ParsedItemKind::Import {
                        local_import_target(
                            language,
                            label,
                            &item.label,
                            &context.c_include_dirs,
                            &context.dart_packages,
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
                            )
                        } else {
                            None
                        };
                    if let Some(local_import) = local_import.as_ref() {
                        item_metadata.insert("import_scope".to_string(), "local".to_string());
                        item_metadata
                            .insert("import_target".to_string(), local_import.target.clone());
                        item_metadata.insert("resolution".to_string(), "pending".to_string());
                        if test_cutoff.is_some_and(|cutoff| item.span.start_line >= cutoff) {
                            item_metadata.insert("test_context".to_string(), "true".to_string());
                        }
                    }

                    let item_id = context.graph.add_node_with_metadata(
                        node_kind,
                        item.label.clone(),
                        Some(item.span.clone()),
                        item_metadata,
                    );
                    let edge_kind = match item.kind {
                        ParsedItemKind::Import => EdgeKind::Imports,
                        _ => EdgeKind::Contains,
                    };
                    context
                        .graph
                        .add_edge(file_id, item_id, edge_kind, Confidence::Syntactic);
                    if let Some(local_import) = local_import {
                        context.pending_local_imports.push(PendingLocalImport {
                            import_node: item_id,
                            target: local_import.target,
                            candidates: local_import.candidates,
                            mark_unresolved: true,
                        });
                    } else if let Some(possible_local_import) = possible_local_import {
                        context.pending_local_imports.push(PendingLocalImport {
                            import_node: item_id,
                            target: possible_local_import.target,
                            candidates: possible_local_import.candidates,
                            mark_unresolved: false,
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
                    }
                    if item.kind == ParsedItemKind::Type {
                        register_function_symbol(&mut context.type_symbols, &item.label, item_id);
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

                if let Some(source) = source_text.as_deref() {
                    index_framework_routes(
                        context,
                        file_id,
                        label,
                        language,
                        source,
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
                }

                for item in parsed.items.iter().filter(|item| is_effect_item(item.kind)) {
                    let source_id = item
                        .parent
                        .as_deref()
                        .and_then(|parent| resolve_local_function(&local_functions, parent))
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
                    item_metadata.insert("parser".to_string(), "tree-sitter".to_string());
                    item_metadata.insert(
                        "item_kind".to_string(),
                        parsed_item_kind_name(item.kind).to_string(),
                    );
                    if let Some(parent) = item.parent.as_deref() {
                        item_metadata.insert("parent".to_string(), parent.to_string());
                    }

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
                }

                for item in parsed
                    .items
                    .iter()
                    .filter(|item| item.kind == ParsedItemKind::Call)
                {
                    let Some(parent) = item.parent.as_deref() else {
                        continue;
                    };
                    let Some(caller) = resolve_local_function(&local_functions, parent) else {
                        continue;
                    };
                    context.pending_calls.push(PendingCall {
                        caller,
                        label: item.label.clone(),
                        span: item.span.clone(),
                        language: language.to_string(),
                    });
                }

                for reference in &parsed.type_references {
                    let source = reference
                        .parent
                        .as_deref()
                        .and_then(|parent| resolve_local_function(&local_functions, parent))
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
