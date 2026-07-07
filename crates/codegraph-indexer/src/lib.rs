use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind, SourceSpan};
use codegraph_parser::{
    Language, LanguageAdapter, ParsedFile, ParsedItemKind, adapter_for_language, adapter_for_path,
};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

pub const DEFAULT_MAX_FILE_SIZE: u64 = 2 * 1024 * 1024;
const PARSE_CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("failed to walk project tree at {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("failed to read project config at {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse project config at {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid project config at {path}: {message}")]
    ConfigInvalid { path: PathBuf, message: String },
    #[error("invalid scan options: {message}")]
    InvalidOptions { message: String },
}

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: u64,
    pub ignored_names: BTreeSet<String>,
    pub ignored_globs: BTreeSet<String>,
    pub parse_cache_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct IndexOptionOverrides {
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScanCoverageReport {
    pub root: String,
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: u64,
    pub ignored_names: Vec<String>,
    pub ignored_globs: Vec<String>,
    pub directories_seen: usize,
    pub files_seen: usize,
    pub indexed_files: usize,
    pub skipped_large_files: usize,
    pub skipped_policy_entries: usize,
    pub skipped_hidden_entries: usize,
    pub skipped_ignored_name_entries: usize,
    pub skipped_ignored_glob_entries: usize,
    pub non_index_files: usize,
    pub seen_bytes: u64,
    pub indexed_bytes: u64,
    pub skipped_large_bytes: u64,
    pub languages: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryExclusion {
    Hidden,
    IgnoredName,
    IgnoredGlob,
}

struct IndexContext {
    graph: CodeGraph,
    function_symbols: BTreeMap<String, Vec<NodeId>>,
    file_nodes: BTreeMap<String, NodeId>,
    external_dependencies: BTreeMap<String, NodeId>,
    cargo_workspace_dependencies: BTreeMap<String, Option<String>>,
    go_modules: Vec<GoModuleRoot>,
    c_include_dirs: Vec<String>,
    custom_rules: CustomRules,
    annotations: GraphAnnotations,
    pending_calls: Vec<PendingCall>,
    pending_local_imports: Vec<PendingLocalImport>,
    pending_entrypoint_targets: Vec<PendingEntrypointTarget>,
}

struct PendingCall {
    caller: NodeId,
    label: String,
    span: SourceSpan,
    language: String,
}

struct PendingLocalImport {
    import_node: NodeId,
    target: String,
    candidates: Vec<String>,
    mark_unresolved: bool,
}

struct PendingEntrypointTarget {
    entrypoint: NodeId,
    manifest_label: String,
    target: String,
    ecosystem: String,
    entrypoint_kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct FileStamp {
    len: u64,
    modified_ns: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ParseCacheRecord {
    cache_schema_version: u32,
    language: Language,
    stamp: FileStamp,
    parsed: ParsedFile,
}

struct EntrypointTargetCandidate {
    path: String,
    symbol: Option<String>,
    file_confidence: Confidence,
    function_confidence: Confidence,
    resolution: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestDependency {
    name: String,
    kind: String,
    ecosystem: String,
    version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestEntrypoint {
    label: String,
    kind: String,
    ecosystem: String,
    target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrameworkRoute {
    framework: String,
    method: String,
    path: String,
    handler: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FrameworkConfig {
    framework: String,
    label: String,
    config_kind: String,
    value: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoModuleRoot {
    module: String,
    dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalImportTarget {
    target: String,
    candidates: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CustomRules {
    forbidden_dependencies: Vec<ForbiddenDependencyRule>,
    forbidden_edges: Vec<ForbiddenEdgeRule>,
    required_configs: Vec<RequiredConfigRule>,
    parse_errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForbiddenDependencyRule {
    id: String,
    package: String,
    ecosystem: Option<String>,
    severity: String,
    message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequiredConfigRule {
    id: String,
    target: String,
    severity: String,
    message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForbiddenEdgeRule {
    id: String,
    edge_kind: Option<String>,
    source_kind: Option<String>,
    source_label: Option<String>,
    source_metadata: BTreeMap<String, String>,
    target_kind: Option<String>,
    target_label: Option<String>,
    target_metadata: BTreeMap<String, String>,
    severity: String,
    message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct GraphAnnotations {
    node_annotations: Vec<NodeAnnotationRule>,
    parse_errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeAnnotationRule {
    id: String,
    kind: Option<String>,
    label: Option<String>,
    language: Option<String>,
    item_kind: Option<String>,
    metadata: BTreeMap<String, String>,
    set: BTreeMap<String, String>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_ignored: false,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            ignored_names: default_ignored_names(),
            ignored_globs: BTreeSet::new(),
            parse_cache_dir: None,
        }
    }
}

impl IndexOptions {
    pub fn with_parse_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.parse_cache_dir = Some(dir.into());
        self
    }
}

pub fn configured_index_options(
    root: impl AsRef<Path>,
    overrides: &IndexOptionOverrides,
) -> Result<IndexOptions, IndexError> {
    let root = root.as_ref();
    let mut options = IndexOptions::default();
    let path = root.join(".codegraph").join("config.toml");
    if path.is_file() {
        let source = fs::read_to_string(&path).map_err(|source| IndexError::ConfigRead {
            path: path.clone(),
            source,
        })?;
        let value = toml::Value::Table(source.parse::<toml::Table>().map_err(|source| {
            IndexError::ConfigParse {
                path: path.clone(),
                source,
            }
        })?);
        apply_project_config(&mut options, &path, &value)?;
    }

    if overrides.include_hidden {
        options.include_hidden = true;
    }
    if overrides.include_ignored {
        options.include_ignored = true;
    }
    if let Some(max_file_size) = overrides.max_file_size {
        if max_file_size == 0 {
            return Err(IndexError::ConfigInvalid {
                path,
                message: "max_file_size must be greater than zero".to_string(),
            });
        }
        options.max_file_size = max_file_size;
    }

    Ok(options)
}

fn apply_project_config(
    options: &mut IndexOptions,
    path: &Path,
    value: &toml::Value,
) -> Result<(), IndexError> {
    let Some(scan) = value.get("scan") else {
        return Ok(());
    };
    let Some(scan) = scan.as_table() else {
        return Err(config_invalid(path, "[scan] must be a table"));
    };

    if let Some(include_hidden) = optional_bool(path, scan, "include_hidden")? {
        options.include_hidden = include_hidden;
    }
    if let Some(include_ignored) = optional_bool(path, scan, "include_ignored")? {
        options.include_ignored = include_ignored;
    }
    if let Some(max_file_size) = optional_u64(path, scan, "max_file_size")? {
        if max_file_size == 0 {
            return Err(config_invalid(
                path,
                "scan.max_file_size must be greater than zero",
            ));
        }
        options.max_file_size = max_file_size;
    }
    if let Some(ignored_names) = optional_string_array(path, scan, "ignored_names")? {
        options.ignored_names = ignored_names.into_iter().collect();
    }
    if let Some(extra_ignored_names) = optional_string_array(path, scan, "extra_ignored_names")? {
        options.ignored_names.extend(extra_ignored_names);
    }
    if let Some(ignored_globs) = optional_string_array(path, scan, "ignored_globs")? {
        validate_ignored_globs(path, "ignored_globs", &ignored_globs)?;
        options.ignored_globs = ignored_globs
            .into_iter()
            .map(|pattern| normalize_glob_pattern(&pattern))
            .collect();
    }
    if let Some(extra_ignored_globs) = optional_string_array(path, scan, "extra_ignored_globs")? {
        validate_ignored_globs(path, "extra_ignored_globs", &extra_ignored_globs)?;
        options.ignored_globs.extend(
            extra_ignored_globs
                .into_iter()
                .map(|pattern| normalize_glob_pattern(&pattern)),
        );
    }

    Ok(())
}

fn optional_bool(
    path: &Path,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<bool>, IndexError> {
    table
        .get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| config_invalid(path, &format!("scan.{key} must be a boolean")))
        })
        .transpose()
}

fn optional_u64(
    path: &Path,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<u64>, IndexError> {
    table
        .get(key)
        .map(|value| {
            value
                .as_integer()
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| {
                    config_invalid(path, &format!("scan.{key} must be a positive integer"))
                })
        })
        .transpose()
}

fn optional_string_array(
    path: &Path,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<Vec<String>>, IndexError> {
    table
        .get(key)
        .map(|value| {
            let Some(values) = value.as_array() else {
                return Err(config_invalid(
                    path,
                    &format!("scan.{key} must be an array"),
                ));
            };
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(|value| value.trim().to_string())
                        .ok_or_else(|| {
                            config_invalid(path, &format!("scan.{key} entries must be strings"))
                        })
                })
                .filter(|value| value.as_ref().is_ok_and(|value| !value.is_empty()))
                .collect()
        })
        .transpose()
}

fn validate_ignored_globs(path: &Path, key: &str, patterns: &[String]) -> Result<(), IndexError> {
    for pattern in patterns {
        let normalized = normalize_glob_pattern(pattern);
        for expanded in expanded_ignored_glob_patterns(&normalized) {
            Glob::new(&expanded).map_err(|error| {
                config_invalid(path, &format!("scan.{key} contains invalid glob: {error}"))
            })?;
        }
    }
    Ok(())
}

fn compile_ignored_globs(patterns: &BTreeSet<String>) -> Result<Option<GlobSet>, IndexError> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        for expanded in expanded_ignored_glob_patterns(pattern) {
            builder.add(
                Glob::new(&expanded).map_err(|error| IndexError::InvalidOptions {
                    message: format!("ignored glob `{pattern}` is invalid: {error}"),
                })?,
            );
        }
    }
    Ok(Some(builder.build().map_err(|error| {
        IndexError::InvalidOptions {
            message: format!("ignored glob set is invalid: {error}"),
        }
    })?))
}

fn expanded_ignored_glob_patterns(pattern: &str) -> Vec<String> {
    let normalized = normalize_glob_pattern(pattern);
    let mut patterns = vec![normalized.clone()];
    for suffix in ["/**", "/**/*"] {
        if let Some(prefix) = normalized.strip_suffix(suffix)
            && !prefix.is_empty()
        {
            patterns.push(prefix.to_string());
        }
    }
    patterns
}

fn normalize_glob_pattern(pattern: &str) -> String {
    let mut normalized = pattern.trim().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }
    normalized
}

fn config_invalid(path: &Path, message: &str) -> IndexError {
    IndexError::ConfigInvalid {
        path: path.to_path_buf(),
        message: message.to_string(),
    }
}

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

fn scan_project_with_scope(
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
    let c_include_dirs = c_include_dirs(root, options, &ignored_globs);
    let custom_rules = custom_rules(root);
    let annotations = graph_annotations(root);
    let mut context = IndexContext {
        graph: CodeGraph::new(root_label),
        function_symbols: BTreeMap::new(),
        file_nodes: BTreeMap::new(),
        external_dependencies: BTreeMap::new(),
        cargo_workspace_dependencies,
        go_modules,
        c_include_dirs,
        custom_rules,
        annotations,
        pending_calls: Vec::new(),
        pending_local_imports: Vec::new(),
        pending_entrypoint_targets: Vec::new(),
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
            let id = context.graph.add_node(NodeKind::Directory, label);
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

    resolve_pending_calls(&mut context);
    resolve_pending_local_imports(&mut context);
    resolve_pending_entrypoint_targets(&mut context);
    apply_graph_annotations(&mut context);
    apply_custom_rules(&mut context);

    Ok(context.graph)
}

#[derive(Debug)]
struct ScanScope {
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

fn normalize_scan_scope_path(path: &str) -> String {
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
        }
    }

    Ok(report)
}

fn index_skipped_file(
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

fn index_file(context: &mut IndexContext, path: &Path, label: &str, options: &IndexOptions) {
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
        script_entrypoint = index_script_entrypoint(context, file_id, label, source);
        index_manifest_facts(context, file_id, path, label, source);
        index_framework_configs(context, file_id, label, language, source);
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
                for item in parsed.items.iter().filter(|item| is_symbol_item(item.kind)) {
                    let node_kind = match item.kind {
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint => NodeKind::Function,
                        ParsedItemKind::Type => NodeKind::Type,
                        ParsedItemKind::Module => NodeKind::Module,
                        ParsedItemKind::Import => NodeKind::ExternalDependency,
                        ParsedItemKind::Call
                        | ParsedItemKind::EnvironmentRead
                        | ParsedItemKind::ConfigRead
                        | ParsedItemKind::Error => {
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
                        local_import_target(language, label, &item.label, &context.c_include_dirs)
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
                            )
                        } else {
                            None
                        };
                    if let Some(local_import) = local_import.as_ref() {
                        item_metadata.insert("import_scope".to_string(), "local".to_string());
                        item_metadata
                            .insert("import_target".to_string(), local_import.target.clone());
                        item_metadata.insert("resolution".to_string(), "pending".to_string());
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
                }

                if let Some(entrypoint_id) = script_entrypoint
                    && let Some(main_id) = resolve_local_function(&local_functions, "main")
                {
                    add_entrypoint_reference(
                        &mut context.graph,
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
                        ParsedItemKind::Error => NodeKind::Unknown,
                        _ => unreachable!("only effect facts are processed here"),
                    };
                    let edge_kind = match item.kind {
                        ParsedItemKind::EnvironmentRead => EdgeKind::ReadsEnvironment,
                        ParsedItemKind::ConfigRead => EdgeKind::ReadsConfig,
                        ParsedItemKind::Error => EdgeKind::MayError,
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
                        &mut context.graph,
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

fn parse_source_cached(
    options: &IndexOptions,
    path: &Path,
    label: &str,
    source: &[u8],
    adapter: &dyn LanguageAdapter,
) -> Result<ParsedFile, codegraph_parser::ParseError> {
    let Some(cache_dir) = options.parse_cache_dir.as_deref() else {
        return adapter.parse(Path::new(label), source);
    };
    let Some(stamp) = file_stamp(path) else {
        return adapter.parse(Path::new(label), source);
    };
    let language = adapter.language();

    if let Some(parsed) = load_cached_parse(cache_dir, label, language, stamp) {
        return Ok(parsed);
    }

    let parsed = adapter.parse(Path::new(label), source)?;
    store_cached_parse(cache_dir, label, language, stamp, &parsed);
    Ok(parsed)
}

fn load_cached_parse(
    cache_dir: &Path,
    label: &str,
    language: Language,
    stamp: FileStamp,
) -> Option<ParsedFile> {
    let path = parse_cache_path(cache_dir, label, language);
    let bytes = fs::read(path).ok()?;
    let record: ParseCacheRecord = serde_json::from_slice(&bytes).ok()?;
    if record.cache_schema_version == PARSE_CACHE_SCHEMA_VERSION
        && record.language == language
        && record.stamp == stamp
    {
        Some(record.parsed)
    } else {
        None
    }
}

fn store_cached_parse(
    cache_dir: &Path,
    label: &str,
    language: Language,
    stamp: FileStamp,
    parsed: &ParsedFile,
) {
    let record = ParseCacheRecord {
        cache_schema_version: PARSE_CACHE_SCHEMA_VERSION,
        language,
        stamp,
        parsed: parsed.clone(),
    };
    if fs::create_dir_all(cache_dir).is_err() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&record) {
        let _ = fs::write(parse_cache_path(cache_dir, label, language), bytes);
    }
}

fn parse_cache_path(cache_dir: &Path, label: &str, language: Language) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    language.hash(&mut hasher);
    label.hash(&mut hasher);
    cache_dir.join(format!("parse-{:016x}.json", hasher.finish()))
}

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());
    Some(FileStamp {
        len: metadata.len(),
        modified_ns,
    })
}

fn index_manifest_facts(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    index_manifest_dependencies(context, file_id, path, source);
    index_manifest_entrypoints(context, file_id, path, label, source);
}

fn index_script_entrypoint(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) -> Option<NodeId> {
    let (interpreter, language) = shebang_interpreter(source)?;
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "script_entrypoint".to_string());
    metadata.insert("entrypoint_kind".to_string(), "script".to_string());
    metadata.insert("source".to_string(), "shebang".to_string());
    metadata.insert("target".to_string(), label.to_string());
    metadata.insert("interpreter".to_string(), interpreter.to_string());
    metadata.insert("language".to_string(), language.to_string());

    let entrypoint_id = context.graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        format!("script:{label}"),
        None,
        metadata,
    );
    add_edge_once(
        &mut context.graph,
        file_id,
        entrypoint_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    let root_id = context.graph.root;
    add_edge_once(
        &mut context.graph,
        root_id,
        entrypoint_id,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    add_entrypoint_reference(
        &mut context.graph,
        entrypoint_id,
        file_id,
        "entrypoint_file",
        "shebang_path",
        Confidence::Exact,
        None,
    );

    Some(entrypoint_id)
}

fn index_framework_routes(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Language,
    source: &str,
    local_functions: &BTreeMap<String, NodeId>,
) {
    for route in framework_routes(language, source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "framework_route".to_string());
        metadata.insert("entrypoint_kind".to_string(), "route".to_string());
        metadata.insert("source".to_string(), "framework".to_string());
        metadata.insert("language".to_string(), language.to_string());
        metadata.insert("framework".to_string(), route.framework.clone());
        metadata.insert("method".to_string(), route.method.clone());
        metadata.insert("path".to_string(), route.path.clone());
        metadata.insert("target".to_string(), label.to_string());
        metadata.insert("line".to_string(), route.line.to_string());
        if let Some(handler) = route.handler.as_deref() {
            metadata.insert("handler".to_string(), handler.to_string());
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("route {} {}", route.method, route.path),
            Some(line_span(label, source, route.line)),
            metadata,
        );
        add_edge_once(
            &mut context.graph,
            file_id,
            entrypoint_id,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        let root_id = context.graph.root;
        add_edge_once(
            &mut context.graph,
            root_id,
            entrypoint_id,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
        );
        add_entrypoint_reference(
            &mut context.graph,
            entrypoint_id,
            file_id,
            "entrypoint_file",
            "framework_route_file",
            Confidence::Syntactic,
            None,
        );

        if let Some(handler) = route.handler.as_deref()
            && let Some(handler_id) = resolve_local_function(local_functions, handler)
        {
            add_entrypoint_reference(
                &mut context.graph,
                entrypoint_id,
                handler_id,
                "entrypoint_function",
                "framework_route_handler",
                Confidence::Syntactic,
                Some(handler),
            );
        }
    }
}

fn framework_routes(language: Language, source: &str) -> Vec<FrameworkRoute> {
    match language {
        Language::Python => python_framework_routes(source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => js_framework_routes(source),
        Language::Rust => rust_framework_routes(source),
        Language::Go => go_framework_routes(source),
        Language::Php => php_framework_routes(source),
        Language::C | Language::Cpp | Language::Bash => Vec::new(),
    }
}

fn index_commonjs_require_imports(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Language,
    source: &str,
) {
    if !matches!(
        language,
        Language::JavaScript | Language::TypeScript | Language::Tsx
    ) {
        return;
    }

    for (index, line) in source.lines().enumerate() {
        let Some(require_call) = commonjs_require_call(line) else {
            continue;
        };

        let mut metadata = BTreeMap::new();
        metadata.insert("language".to_string(), language.to_string());
        metadata.insert("parser".to_string(), "syntax-pattern".to_string());
        metadata.insert("item_kind".to_string(), "import".to_string());
        metadata.insert("import_style".to_string(), "commonjs".to_string());

        let local_import = local_import_target(language, label, &require_call, &[]);
        if let Some(local_import) = local_import.as_ref() {
            metadata.insert("import_scope".to_string(), "local".to_string());
            metadata.insert("import_target".to_string(), local_import.target.clone());
            metadata.insert("resolution".to_string(), "pending".to_string());
        }

        let import_id = context.graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            require_call,
            Some(line_span(label, source, index as u32 + 1)),
            metadata,
        );
        add_edge_once(
            &mut context.graph,
            file_id,
            import_id,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        if let Some(local_import) = local_import {
            context.pending_local_imports.push(PendingLocalImport {
                import_node: import_id,
                target: local_import.target,
                candidates: local_import.candidates,
                mark_unresolved: true,
            });
        }
    }
}

fn commonjs_require_call(line: &str) -> Option<String> {
    let mut search_start = 0;
    while let Some(offset) = line[search_start..].find("require(") {
        let start = search_start + offset;
        let before = line[..start].chars().next_back();
        if before.is_none_or(|character| !is_identifier_or_member_character(character)) {
            let rest = &line[start..];
            let module = first_quoted_value_after(rest, "require(")?;
            return Some(format!("require(\"{module}\")"));
        }
        search_start = start + "require(".len();
    }
    None
}

fn is_identifier_or_member_character(character: char) -> bool {
    character == '_' || character == '$' || character == '.' || character.is_ascii_alphanumeric()
}

fn index_framework_configs(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    language: Option<Language>,
    source: &str,
) {
    for config in framework_configs(label, language, source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "framework_config".to_string());
        metadata.insert("source".to_string(), "framework".to_string());
        metadata.insert("framework".to_string(), config.framework.clone());
        metadata.insert("config_kind".to_string(), config.config_kind.clone());
        metadata.insert("target".to_string(), label.to_string());
        metadata.insert("line".to_string(), config.line.to_string());
        if let Some(language) = language {
            metadata.insert("language".to_string(), language.to_string());
        }
        if let Some(value) = config.value.as_deref() {
            metadata.insert("value".to_string(), value.to_string());
        }

        let config_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            config.label,
            Some(line_span(label, source, config.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "framework".to_string());
        edge_metadata.insert("framework".to_string(), config.framework);
        edge_metadata.insert("config_kind".to_string(), config.config_kind);
        add_edge_once_with_metadata(
            &mut context.graph,
            file_id,
            config_id,
            EdgeKind::ReadsConfig,
            Confidence::Syntactic,
            edge_metadata,
        );
    }
}

fn framework_configs(
    label: &str,
    language: Option<Language>,
    source: &str,
) -> Vec<FrameworkConfig> {
    let mut configs = BTreeSet::new();
    configs.extend(file_framework_configs(label));

    match language {
        Some(Language::Python) => configs.extend(python_framework_configs(source)),
        Some(Language::JavaScript | Language::TypeScript | Language::Tsx) => {
            configs.extend(js_framework_configs(source))
        }
        Some(Language::Rust) => configs.extend(rust_framework_configs(source)),
        Some(Language::Go) => configs.extend(go_framework_configs(source)),
        Some(Language::Php) => configs.extend(php_framework_configs(source)),
        Some(Language::Bash) => configs.extend(bash_framework_configs(source)),
        Some(Language::C | Language::Cpp) | None => {}
    }

    configs.into_iter().collect()
}

fn line_span(path: &str, source: &str, line: u32) -> SourceSpan {
    let line = line.max(1);
    let line_text = source
        .lines()
        .nth(line.saturating_sub(1) as usize)
        .unwrap_or("");
    SourceSpan {
        path: path.to_string(),
        start_line: line,
        start_column: 1,
        end_line: line,
        end_column: line_text.chars().count() as u32 + 1,
    }
}

fn apply_graph_annotations(context: &mut IndexContext) {
    let annotations = context.annotations.clone();
    for message in annotations.parse_errors {
        add_annotation_parse_error(&mut context.graph, message);
    }

    for annotation in annotations.node_annotations {
        for node in &mut context.graph.nodes {
            if !node_annotation_matches(node, &annotation) {
                continue;
            }
            let ids = append_metadata_list(
                node.metadata.remove("annotation_ids"),
                annotation.id.as_str(),
            );
            node.metadata.insert("annotation_ids".to_string(), ids);
            node.metadata
                .insert("annotation_source".to_string(), "user".to_string());
            for (key, value) in &annotation.set {
                node.metadata
                    .insert(format!("annotation.{key}"), value.clone());
            }
        }
    }
}

fn add_annotation_parse_error(graph: &mut CodeGraph, message: String) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "annotation_error".to_string());
    metadata.insert("source".to_string(), "annotation".to_string());
    metadata.insert("message".to_string(), message);
    let id =
        graph.add_node_with_metadata(NodeKind::Unknown, "annotation parse error", None, metadata);
    let root_id = graph.root;
    add_edge_once(graph, root_id, id, EdgeKind::Contains, Confidence::Exact);
}

fn node_annotation_matches(node: &codegraph_core::Node, annotation: &NodeAnnotationRule) -> bool {
    annotation
        .kind
        .as_deref()
        .is_none_or(|expected| text_matches(node_kind_name(&node.kind), expected))
        && annotation
            .label
            .as_deref()
            .is_none_or(|expected| text_matches(&node.label, expected))
        && annotation.language.as_deref().is_none_or(|expected| {
            node.metadata
                .get("language")
                .is_some_and(|value| text_matches(value, expected))
        })
        && annotation.item_kind.as_deref().is_none_or(|expected| {
            node.metadata
                .get("item_kind")
                .is_some_and(|value| text_matches(value, expected))
        })
        && annotation.metadata.iter().all(|(key, expected)| {
            node.metadata
                .get(key)
                .is_some_and(|value| text_matches(value, expected))
        })
}

fn append_metadata_list(existing: Option<String>, value: &str) -> String {
    let mut values: BTreeSet<String> = existing
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    values.insert(value.to_string());
    values.into_iter().collect::<Vec<_>>().join(",")
}

fn graph_annotations(root: &Path) -> GraphAnnotations {
    let path = root.join(".codegraph").join("annotations.toml");
    if !path.is_file() {
        return GraphAnnotations::default();
    }

    let Ok(source) = fs::read_to_string(&path) else {
        return GraphAnnotations {
            parse_errors: vec![format!(
                "Could not read graph annotations from {}",
                path.display()
            )],
            ..GraphAnnotations::default()
        };
    };

    let Ok(value) = toml::from_str::<toml::Value>(&source) else {
        return GraphAnnotations {
            parse_errors: vec![format!(
                "Could not parse graph annotations from {}",
                path.display()
            )],
            ..GraphAnnotations::default()
        };
    };

    let annotations = value.get("annotations").unwrap_or(&value);
    GraphAnnotations {
        node_annotations: rule_array(annotations, "node")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| node_annotation_rule(value, index + 1))
            .collect(),
        parse_errors: Vec::new(),
    }
}

fn node_annotation_rule(value: &toml::Value, index: usize) -> Option<NodeAnnotationRule> {
    let set = string_table(value.get("set")?)?;
    if set.is_empty() {
        return None;
    }

    let id = value
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("node:{index}"));

    Some(NodeAnnotationRule {
        id,
        kind: optional_string(value, "kind"),
        label: optional_string(value, "label"),
        language: optional_string(value, "language"),
        item_kind: optional_string(value, "item_kind"),
        metadata: value
            .get("metadata")
            .and_then(string_table)
            .unwrap_or_default(),
        set,
    })
}

fn optional_string(value: &toml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_table(value: &toml::Value) -> Option<BTreeMap<String, String>> {
    Some(
        value
            .as_table()?
            .iter()
            .filter_map(|(key, value)| {
                toml_scalar_to_string(value).map(|value| (key.to_string(), value))
            })
            .collect(),
    )
}

fn toml_scalar_to_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(value) => Some(value.clone()),
        toml::Value::Integer(value) => Some(value.to_string()),
        toml::Value::Float(value) => Some(value.to_string()),
        toml::Value::Boolean(value) => Some(value.to_string()),
        _ => None,
    }
}

fn text_matches(actual: &str, expected: &str) -> bool {
    let actual = actual.to_ascii_lowercase();
    let expected = expected.trim().to_ascii_lowercase();
    !expected.is_empty() && (actual == expected || actual.contains(&expected))
}

fn node_kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Repository => "repository",
        NodeKind::Directory => "directory",
        NodeKind::File => "file",
        NodeKind::Module => "module",
        NodeKind::Function => "function",
        NodeKind::Entrypoint => "entrypoint",
        NodeKind::Type => "type",
        NodeKind::Config => "config",
        NodeKind::Environment => "environment",
        NodeKind::ExternalDependency => "external_dependency",
        NodeKind::Unknown => "unknown",
    }
}

fn edge_kind_name(kind: &EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Contains => "contains",
        EdgeKind::Imports => "imports",
        EdgeKind::Calls => "calls",
        EdgeKind::Defines => "defines",
        EdgeKind::References => "references",
        EdgeKind::ReadsConfig => "reads_config",
        EdgeKind::ReadsEnvironment => "reads_environment",
        EdgeKind::MayError => "may_error",
        EdgeKind::Entrypoint => "entrypoint",
        EdgeKind::DependsOn => "depends_on",
    }
}

fn apply_custom_rules(context: &mut IndexContext) {
    let rules = context.custom_rules.clone();
    for message in rules.parse_errors {
        add_custom_rule_violation(
            context,
            "rules_parse_error",
            "parse_error",
            "error",
            message,
            None,
        );
    }

    for rule in rules.forbidden_dependencies {
        let matches = matching_dependency_nodes(&context.graph, &rule);
        for dependency in matches {
            let dependency_label =
                graph_node_label(&context.graph, dependency).unwrap_or("unknown");
            let message = rule.message.clone().unwrap_or_else(|| {
                format!(
                    "Dependency `{dependency_label}` is forbidden by custom rule `{}`",
                    rule.id
                )
            });
            add_custom_rule_violation(
                context,
                &rule.id,
                "forbidden_dependency",
                &rule.severity,
                message,
                Some(dependency),
            );
        }
    }

    for rule in rules.forbidden_edges {
        let matches = matching_forbidden_edges(&context.graph, &rule);
        for edge_match in matches {
            let source = graph_node_label(&context.graph, edge_match.source).unwrap_or("unknown");
            let target = graph_node_label(&context.graph, edge_match.target).unwrap_or("unknown");
            let message = rule.message.clone().unwrap_or_else(|| {
                format!(
                    "Edge `{source}` -> `{target}` violates custom rule `{}`",
                    rule.id
                )
            });
            add_custom_rule_violation_with_targets(
                context,
                &rule.id,
                "forbidden_edge",
                &rule.severity,
                message,
                &[edge_match.source, edge_match.target],
                Some(edge_match.edge_index),
            );
        }
    }

    for rule in rules.required_configs {
        if custom_rule_config_exists(&context.graph, &rule.target) {
            continue;
        }
        let message = rule.message.clone().unwrap_or_else(|| {
            format!(
                "Required config or environment target `{}` is missing for custom rule `{}`",
                rule.target, rule.id
            )
        });
        add_custom_rule_violation(
            context,
            &rule.id,
            "required_config",
            &rule.severity,
            message,
            None,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ForbiddenEdgeMatch {
    edge_index: usize,
    source: NodeId,
    target: NodeId,
}

fn graph_node_label(graph: &CodeGraph, id: NodeId) -> Option<&str> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| node.label.as_str())
}

fn matching_dependency_nodes(graph: &CodeGraph, rule: &ForbiddenDependencyRule) -> Vec<NodeId> {
    graph
        .nodes
        .iter()
        .filter_map(|node| {
            if node.kind != NodeKind::ExternalDependency
                || node
                    .metadata
                    .get("item_kind")
                    .is_none_or(|kind| kind != "dependency")
            {
                return None;
            }

            let package_id = node.metadata.get("package_id")?;
            if forbidden_dependency_matches(package_id, &node.label, rule) {
                Some(node.id)
            } else {
                None
            }
        })
        .collect()
}

fn matching_forbidden_edges(
    graph: &CodeGraph,
    rule: &ForbiddenEdgeRule,
) -> Vec<ForbiddenEdgeMatch> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(edge_index, edge)| {
            if rule
                .edge_kind
                .as_deref()
                .is_some_and(|expected| !text_matches(edge_kind_name(&edge.kind), expected))
            {
                return None;
            }
            let source = graph.nodes.iter().find(|node| node.id == edge.source)?;
            let target = graph.nodes.iter().find(|node| node.id == edge.target)?;
            if endpoint_rule_matches(
                source,
                rule.source_kind.as_deref(),
                rule.source_label.as_deref(),
                &rule.source_metadata,
            ) && endpoint_rule_matches(
                target,
                rule.target_kind.as_deref(),
                rule.target_label.as_deref(),
                &rule.target_metadata,
            ) {
                Some(ForbiddenEdgeMatch {
                    edge_index,
                    source: edge.source,
                    target: edge.target,
                })
            } else {
                None
            }
        })
        .collect()
}

fn endpoint_rule_matches(
    node: &codegraph_core::Node,
    kind: Option<&str>,
    label: Option<&str>,
    metadata: &BTreeMap<String, String>,
) -> bool {
    kind.is_none_or(|expected| text_matches(node_kind_name(&node.kind), expected))
        && label.is_none_or(|expected| text_matches(&node.label, expected))
        && metadata.iter().all(|(key, expected)| {
            node.metadata
                .get(key)
                .is_some_and(|value| text_matches(value, expected))
        })
}

fn forbidden_dependency_matches(
    package_id: &str,
    package_label: &str,
    rule: &ForbiddenDependencyRule,
) -> bool {
    let Some((ecosystem, package)) = package_id.split_once(':') else {
        return false;
    };
    if rule
        .ecosystem
        .as_deref()
        .is_some_and(|expected| !expected.eq_ignore_ascii_case(ecosystem))
    {
        return false;
    }

    let expected = canonical_package_name(ecosystem, &rule.package);
    package.eq_ignore_ascii_case(&expected)
        || package_label.eq_ignore_ascii_case(&rule.package)
        || package_label.eq_ignore_ascii_case(&expected)
}

fn custom_rule_config_exists(graph: &CodeGraph, target: &str) -> bool {
    graph.nodes.iter().any(|node| {
        if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
            return false;
        }
        custom_rule_text_matches(&node.label, target)
            || node
                .metadata
                .values()
                .any(|value| custom_rule_text_matches(value, target))
    })
}

fn custom_rule_text_matches(value: &str, expected: &str) -> bool {
    let value = value.to_ascii_lowercase();
    let expected = expected.trim().to_ascii_lowercase();
    !expected.is_empty() && (value == expected || value.contains(&expected))
}

fn add_custom_rule_violation(
    context: &mut IndexContext,
    rule_id: &str,
    rule_kind: &str,
    severity: &str,
    message: String,
    target: Option<NodeId>,
) {
    let targets = target.into_iter().collect::<Vec<_>>();
    add_custom_rule_violation_with_targets(
        context, rule_id, rule_kind, severity, message, &targets, None,
    );
}

fn add_custom_rule_violation_with_targets(
    context: &mut IndexContext,
    rule_id: &str,
    rule_kind: &str,
    severity: &str,
    message: String,
    targets: &[NodeId],
    violated_edge_index: Option<usize>,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "custom_rule_violation".to_string());
    metadata.insert("source".to_string(), "custom_rule".to_string());
    metadata.insert("rule_id".to_string(), rule_id.to_string());
    metadata.insert("rule_kind".to_string(), rule_kind.to_string());
    metadata.insert(
        "severity".to_string(),
        normalize_rule_severity(severity).to_string(),
    );
    metadata.insert("message".to_string(), message.clone());
    if let Some(edge_index) = violated_edge_index {
        metadata.insert("violated_edge_index".to_string(), edge_index.to_string());
    }

    let violation = context.graph.add_node_with_metadata(
        NodeKind::Unknown,
        format!("custom rule violation:{rule_id}"),
        None,
        metadata,
    );
    let root_id = context.graph.root;
    add_edge_once(
        &mut context.graph,
        root_id,
        violation,
        EdgeKind::Contains,
        Confidence::Exact,
    );

    for target in targets {
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "custom_rule".to_string());
        edge_metadata.insert("relation".to_string(), "custom_rule_target".to_string());
        edge_metadata.insert("rule_id".to_string(), rule_id.to_string());
        add_edge_once_with_metadata(
            &mut context.graph,
            violation,
            *target,
            EdgeKind::References,
            Confidence::Exact,
            edge_metadata,
        );
    }
}

fn custom_rules(root: &Path) -> CustomRules {
    let path = root.join(".codegraph").join("rules.toml");
    if !path.is_file() {
        return CustomRules::default();
    }

    let Ok(source) = fs::read_to_string(&path) else {
        return CustomRules {
            parse_errors: vec![format!(
                "Could not read custom rules from {}",
                path.display()
            )],
            ..CustomRules::default()
        };
    };

    let Ok(value) = toml::from_str::<toml::Value>(&source) else {
        return CustomRules {
            parse_errors: vec![format!(
                "Could not parse custom rules from {}",
                path.display()
            )],
            ..CustomRules::default()
        };
    };

    let rules = value.get("rules").unwrap_or(&value);
    CustomRules {
        forbidden_dependencies: rule_array(rules, "forbidden_dependency")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| forbidden_dependency_rule(value, index + 1))
            .collect(),
        forbidden_edges: rule_array(rules, "forbidden_edge")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| forbidden_edge_rule(value, index + 1))
            .collect(),
        required_configs: rule_array(rules, "required_config")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| required_config_rule(value, index + 1))
            .collect(),
        parse_errors: Vec::new(),
    }
}

fn rule_array<'a>(rules: &'a toml::Value, name: &str) -> Vec<&'a toml::Value> {
    rules
        .get(name)
        .and_then(|value| value.as_array())
        .map(|values| values.iter().collect())
        .unwrap_or_default()
}

fn forbidden_dependency_rule(value: &toml::Value, index: usize) -> Option<ForbiddenDependencyRule> {
    let package = value.get("package")?.as_str()?.trim().to_string();
    if package.is_empty() {
        return None;
    }
    Some(ForbiddenDependencyRule {
        id: rule_id(value, "forbidden_dependency", index, &package),
        package,
        ecosystem: value
            .get("ecosystem")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty()),
        severity: rule_severity(value),
        message: rule_message(value),
    })
}

fn forbidden_edge_rule(value: &toml::Value, index: usize) -> Option<ForbiddenEdgeRule> {
    let source_metadata = value
        .get("source_metadata")
        .and_then(string_table)
        .unwrap_or_default();
    let target_metadata = value
        .get("target_metadata")
        .and_then(string_table)
        .unwrap_or_default();
    let source_kind = optional_string(value, "source_kind");
    let source_label = optional_string(value, "source_label");
    let target_kind = optional_string(value, "target_kind");
    let target_label = optional_string(value, "target_label");

    if source_metadata.is_empty()
        && target_metadata.is_empty()
        && source_kind.is_none()
        && source_label.is_none()
        && target_kind.is_none()
        && target_label.is_none()
    {
        return None;
    }

    Some(ForbiddenEdgeRule {
        id: rule_id(value, "forbidden_edge", index, "edge"),
        edge_kind: optional_string(value, "edge_kind"),
        source_kind,
        source_label,
        source_metadata,
        target_kind,
        target_label,
        target_metadata,
        severity: rule_severity(value),
        message: rule_message(value),
    })
}

fn required_config_rule(value: &toml::Value, index: usize) -> Option<RequiredConfigRule> {
    let target = value.get("target")?.as_str()?.trim().to_string();
    if target.is_empty() {
        return None;
    }
    Some(RequiredConfigRule {
        id: rule_id(value, "required_config", index, &target),
        target,
        severity: rule_severity(value),
        message: rule_message(value),
    })
}

fn rule_id(value: &toml::Value, kind: &str, index: usize, fallback: &str) -> String {
    value
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{kind}:{index}:{fallback}"))
}

fn rule_severity(value: &toml::Value) -> String {
    value
        .get("severity")
        .and_then(|value| value.as_str())
        .map(normalize_rule_severity)
        .unwrap_or("warning")
        .to_string()
}

fn normalize_rule_severity(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => "error",
        "info" => "info",
        _ => "warning",
    }
}

fn rule_message(value: &toml::Value) -> Option<String> {
    value
        .get("message")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn index_manifest_dependencies(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    source: &str,
) {
    let dependencies = manifest_dependencies(path, source, &context.cargo_workspace_dependencies);
    for dependency in dependencies {
        let package_name = canonical_package_name(&dependency.ecosystem, &dependency.name);
        let package_id = package_id(&dependency.ecosystem, &package_name);
        let dependency_id = if let Some(id) = context.external_dependencies.get(&package_id) {
            *id
        } else {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "dependency".to_string());
            metadata.insert("ecosystem".to_string(), dependency.ecosystem.clone());
            metadata.insert("package_id".to_string(), package_id.clone());
            metadata.insert("source".to_string(), "manifest".to_string());
            if package_name != dependency.name {
                metadata.insert("declared_name".to_string(), dependency.name.clone());
            }
            let id = context.graph.add_node_with_metadata(
                NodeKind::ExternalDependency,
                package_name,
                None,
                metadata,
            );
            context.external_dependencies.insert(package_id, id);
            id
        };

        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("dependency_kind".to_string(), dependency.kind);
        edge_metadata.insert("source".to_string(), "manifest".to_string());
        if let Some(version) = dependency.version {
            edge_metadata.insert("dependency_version".to_string(), version);
        }
        add_edge_once_with_metadata(
            &mut context.graph,
            file_id,
            dependency_id,
            EdgeKind::DependsOn,
            Confidence::Exact,
            edge_metadata,
        );
    }
}

fn index_manifest_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    for entrypoint in manifest_entrypoints(path, source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "manifest_entrypoint".to_string());
        metadata.insert("entrypoint_kind".to_string(), entrypoint.kind.clone());
        metadata.insert("ecosystem".to_string(), entrypoint.ecosystem.clone());
        metadata.insert("source".to_string(), "manifest".to_string());
        if let Some(target) = entrypoint.target.as_deref() {
            metadata.insert("target".to_string(), target.to_string());
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            entrypoint.label,
            None,
            metadata,
        );
        add_edge_once(
            &mut context.graph,
            file_id,
            entrypoint_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            &mut context.graph,
            root_id,
            entrypoint_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        if let Some(target) = entrypoint.target {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target,
                    ecosystem: entrypoint.ecosystem,
                    entrypoint_kind: entrypoint.kind,
                });
        }
    }
}

fn manifest_dependencies(
    path: &Path,
    source: &str,
    cargo_workspace_dependencies: &BTreeMap<String, Option<String>>,
) -> Vec<ManifestDependency> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_dependencies(source, cargo_workspace_dependencies),
        Some("package.json") => package_json_dependencies(source),
        Some("go.mod") => go_mod_dependencies(source),
        Some("requirements.txt") => requirements_dependencies(source),
        Some("pyproject.toml") => pyproject_dependencies(source),
        Some("composer.json") => composer_dependencies(source),
        _ => Vec::new(),
    }
}

fn manifest_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_entrypoints(path, source),
        Some("package.json") => package_json_entrypoints(source),
        Some("go.mod") => go_mod_entrypoints(path, source),
        Some("pyproject.toml") => pyproject_entrypoints(source),
        Some("composer.json") => composer_entrypoints(source),
        Some("CMakeLists.txt") => cmake_entrypoints(source),
        _ => Vec::new(),
    }
}

fn cargo_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();

    if let Some(package_name) = value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str())
        && path
            .parent()
            .map(|parent| parent.join("src").join("main.rs").is_file())
            .unwrap_or(false)
    {
        entrypoints.push(manifest_entrypoint(
            format!("cargo bin:{package_name}"),
            "binary",
            "cargo",
            Some("src/main.rs".to_string()),
        ));
    }

    collect_cargo_target_entrypoints(&value, "bin", "binary", &mut entrypoints);
    collect_cargo_target_entrypoints(&value, "example", "example", &mut entrypoints);
    entrypoints
}

fn collect_cargo_target_entrypoints(
    value: &toml::Value,
    table_name: &str,
    entrypoint_kind: &str,
    entrypoints: &mut Vec<ManifestEntrypoint>,
) {
    let Some(targets) = value.get(table_name).and_then(|value| value.as_array()) else {
        return;
    };

    for target in targets {
        let Some(name) = target
            .get("name")
            .and_then(|name| name.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        let target_path = target
            .get("path")
            .and_then(|path| path.as_str())
            .map(str::to_string);
        entrypoints.push(manifest_entrypoint(
            format!("cargo {entrypoint_kind}:{name}"),
            entrypoint_kind,
            "cargo",
            target_path,
        ));
    }
}

fn package_json_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();
    let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) else {
        return entrypoints;
    };

    for (name, command) in scripts {
        entrypoints.push(manifest_entrypoint(
            format!("npm script:{name}"),
            "script",
            "npm",
            command.as_str().map(str::to_string),
        ));
    }
    entrypoints
}

fn go_mod_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    let Some(module) = go_module_name(source) else {
        return Vec::new();
    };
    let Some(root) = path.parent() else {
        return Vec::new();
    };

    let mut entrypoints = Vec::new();
    if root.join("main.go").is_file() {
        entrypoints.push(manifest_entrypoint(
            format!("go module:{module}"),
            "module",
            "go",
            Some("main.go".to_string()),
        ));
    }

    let cmd_dir = root.join("cmd");
    if let Ok(commands) = fs::read_dir(&cmd_dir) {
        for command in commands.flatten() {
            let command_path = command.path();
            if !command_path.join("main.go").is_file() {
                continue;
            }
            let Some(name) = command_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            entrypoints.push(manifest_entrypoint(
                format!("go command:{name}"),
                "command",
                "go",
                Some(format!("cmd/{name}/main.go")),
            ));
        }
    }

    entrypoints
}

fn pyproject_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();

    if let Some(project) = value.get("project") {
        collect_toml_entrypoint_keys(
            project,
            "scripts",
            "console_script",
            "python",
            &mut entrypoints,
        );
        collect_toml_entrypoint_keys(
            project,
            "gui-scripts",
            "gui_script",
            "python",
            &mut entrypoints,
        );
    }

    if let Some(poetry) = value.get("tool").and_then(|value| value.get("poetry")) {
        collect_toml_entrypoint_keys(
            poetry,
            "scripts",
            "poetry_script",
            "python",
            &mut entrypoints,
        );
    }

    entrypoints
}

fn composer_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut entrypoints = Vec::new();

    if let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) {
        for (name, command) in scripts {
            let target = command.as_str().map(str::to_string).or_else(|| {
                command.as_array().map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .collect::<Vec<_>>()
                        .join(" && ")
                })
            });
            entrypoints.push(manifest_entrypoint(
                format!("composer script:{name}"),
                "script",
                "composer",
                target,
            ));
        }
    }

    if let Some(bins) = value.get("bin").and_then(|value| value.as_array()) {
        for bin in bins {
            if let Some(path) = bin.as_str() {
                entrypoints.push(manifest_entrypoint(
                    format!("composer bin:{path}"),
                    "binary",
                    "composer",
                    Some(path.to_string()),
                ));
            }
        }
    }

    entrypoints
}

fn cmake_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    cmake_command_bodies(source, "add_executable")
        .into_iter()
        .filter_map(|body| {
            let args = cmake_command_args(&body);
            let name = args.first()?.trim();
            if name.is_empty()
                || args.iter().any(|arg| arg.eq_ignore_ascii_case("IMPORTED"))
                || args
                    .get(1)
                    .is_some_and(|arg| arg.eq_ignore_ascii_case("ALIAS"))
            {
                return None;
            }

            let target = args
                .iter()
                .skip(1)
                .find(|arg| is_cmake_source_argument(arg))
                .cloned();
            Some(manifest_entrypoint(
                format!("cmake executable:{name}"),
                "executable",
                "cmake",
                target,
            ))
        })
        .collect()
}

fn cargo_dependencies(
    source: &str,
    cargo_workspace_dependencies: &BTreeMap<String, Option<String>>,
) -> Vec<ManifestDependency> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_toml_table_keys(
        &value,
        "dependencies",
        "runtime",
        "cargo",
        &mut dependencies,
        Some(cargo_workspace_dependencies),
    );
    collect_toml_table_keys(
        &value,
        "dev-dependencies",
        "dev",
        "cargo",
        &mut dependencies,
        Some(cargo_workspace_dependencies),
    );
    collect_toml_table_keys(
        &value,
        "build-dependencies",
        "build",
        "cargo",
        &mut dependencies,
        Some(cargo_workspace_dependencies),
    );

    if let Some(targets) = value.get("target").and_then(|value| value.as_table()) {
        for target in targets.values() {
            collect_toml_table_keys(
                target,
                "dependencies",
                "runtime",
                "cargo",
                &mut dependencies,
                Some(cargo_workspace_dependencies),
            );
            collect_toml_table_keys(
                target,
                "dev-dependencies",
                "dev",
                "cargo",
                &mut dependencies,
                Some(cargo_workspace_dependencies),
            );
            collect_toml_table_keys(
                target,
                "build-dependencies",
                "build",
                "cargo",
                &mut dependencies,
                Some(cargo_workspace_dependencies),
            );
        }
    }

    dependencies
}

fn pyproject_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();

    if let Some(project) = value.get("project") {
        if let Some(values) = project
            .get("dependencies")
            .and_then(|value| value.as_array())
        {
            for value in values {
                if let Some((name, version)) = value
                    .as_str()
                    .and_then(package_name_and_version_from_requirement)
                {
                    dependencies.push(manifest_dependency(name, "runtime", "python", version));
                }
            }
        }
        if let Some(optional) = project
            .get("optional-dependencies")
            .and_then(|value| value.as_table())
        {
            for values in optional.values() {
                if let Some(values) = values.as_array() {
                    for value in values {
                        if let Some((name, version)) = value
                            .as_str()
                            .and_then(package_name_and_version_from_requirement)
                        {
                            dependencies
                                .push(manifest_dependency(name, "optional", "python", version));
                        }
                    }
                }
            }
        }
    }

    if let Some(poetry) = value.get("tool").and_then(|value| value.get("poetry")) {
        collect_toml_table_keys(
            poetry,
            "dependencies",
            "runtime",
            "python",
            &mut dependencies,
            None,
        );
        collect_toml_table_keys(
            poetry,
            "dev-dependencies",
            "dev",
            "python",
            &mut dependencies,
            None,
        );
    }

    dependencies
}

fn package_json_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_json_object_keys(&value, "dependencies", "runtime", "npm", &mut dependencies);
    collect_json_object_keys(&value, "devDependencies", "dev", "npm", &mut dependencies);
    collect_json_object_keys(&value, "peerDependencies", "peer", "npm", &mut dependencies);
    collect_json_object_keys(
        &value,
        "optionalDependencies",
        "optional",
        "npm",
        &mut dependencies,
    );
    dependencies
}

fn composer_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_json_object_keys(&value, "require", "runtime", "composer", &mut dependencies);
    collect_json_object_keys(&value, "require-dev", "dev", "composer", &mut dependencies);
    dependencies.retain(|dependency| dependency.name != "php");
    dependencies
}

fn go_mod_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut in_require_block = false;
    for raw_line in source.lines() {
        let line = raw_line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "require (" {
            in_require_block = true;
            continue;
        }
        if in_require_block && line == ")" {
            in_require_block = false;
            continue;
        }
        let requirement = if in_require_block {
            line
        } else if let Some(rest) = line.strip_prefix("require ") {
            rest.trim()
        } else {
            continue;
        };
        let mut parts = requirement.split_whitespace();
        if let Some(name) = parts.next() {
            let version = parts.next().map(str::to_string);
            dependencies.push(manifest_dependency(
                name.to_string(),
                "runtime",
                "go",
                version,
            ));
        }
    }
    dependencies
}

fn go_module_name(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let line = line.split("//").next().unwrap_or("").trim();
        line.strip_prefix("module ")
            .map(str::trim)
            .filter(|module| !module.is_empty())
            .map(str::to_string)
    })
}

fn shebang_interpreter(source: &str) -> Option<(&'static str, &'static str)> {
    let line = source.lines().next()?.trim();
    let command = line.strip_prefix("#!")?.trim();
    if command.is_empty() {
        return None;
    }

    let mut parts = command.split_whitespace();
    let executable = parts.next()?.rsplit('/').next().unwrap_or("");
    let interpreter = if executable == "env" {
        parts
            .find(|part| !part.starts_with('-') && !part.contains('='))
            .unwrap_or("")
    } else {
        executable
    };
    let interpreter = interpreter
        .rsplit('/')
        .next()
        .unwrap_or(interpreter)
        .split_once('.')
        .map_or(interpreter, |(base, _)| base);

    match interpreter {
        "bash" => Some(("bash", "bash")),
        "sh" => Some(("sh", "bash")),
        "zsh" => Some(("zsh", "bash")),
        "ksh" => Some(("ksh", "bash")),
        "python" | "python2" | "python3" => Some(("python", "python")),
        "node" | "nodejs" => Some(("node", "javascript")),
        "php" => Some(("php", "php")),
        _ => None,
    }
}

fn shebang_language(source: &str) -> Option<Language> {
    match shebang_interpreter(source)?.1 {
        "bash" => Some(Language::Bash),
        "python" => Some(Language::Python),
        "javascript" => Some(Language::JavaScript),
        "php" => Some(Language::Php),
        _ => None,
    }
}

fn file_framework_configs(label: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    let file_name = Path::new(label)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(label);
    let lower_label = label.to_ascii_lowercase();
    let lower_name = file_name.to_ascii_lowercase();

    if lower_name == "settings.py" {
        configs.push(framework_config(
            "django",
            format!("django settings:{label}"),
            "settings_module",
            Some(label.to_string()),
            1,
        ));
    }

    for (prefix, framework, kind) in [
        ("next.config.", "nextjs", "config_file"),
        ("vite.config.", "vite", "config_file"),
        ("nuxt.config.", "nuxt", "config_file"),
        ("webpack.config.", "webpack", "config_file"),
        ("svelte.config.", "sveltekit", "config_file"),
    ] {
        if lower_name.starts_with(prefix) {
            configs.push(framework_config(
                framework,
                format!("{framework} config:{label}"),
                kind,
                Some(label.to_string()),
                1,
            ));
        }
    }

    if lower_label.starts_with("config/") && lower_label.ends_with(".php") {
        configs.push(framework_config(
            "laravel",
            format!("laravel config:{label}"),
            "config_file",
            Some(label.to_string()),
            1,
        ));
    }

    if lower_label.starts_with("config/packages/")
        && (lower_label.ends_with(".yaml")
            || lower_label.ends_with(".yml")
            || lower_label.ends_with(".xml")
            || lower_label.ends_with(".php"))
    {
        configs.push(framework_config(
            "symfony",
            format!("symfony config:{label}"),
            "config_file",
            Some(label.to_string()),
            1,
        ));
    }

    configs
}

fn python_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains(".config.from_pyfile(")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "flask",
                format!("flask config:{value}"),
                "config_file",
                Some(value),
                line_number,
            ));
        }

        if lower.contains(".config.from_object(")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "flask",
                format!("flask config object:{value}"),
                "config_object",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("settingsconfigdict(")
            && lower.contains("env_file")
            && let Some(value) = first_quoted_value(trimmed)
        {
            configs.push(framework_config(
                "pydantic",
                format!("pydantic env file:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }

        if lower.starts_with("class ")
            && lower.contains("basesettings")
            && let Some(class_name) = trimmed
                .strip_prefix("class ")
                .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
                .filter(|name| !name.is_empty())
        {
            configs.push(framework_config(
                "pydantic",
                format!("pydantic settings:{class_name}"),
                "settings_class",
                Some(class_name.to_string()),
                line_number,
            ));
        }
    }
    configs
}

fn js_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("dotenv.config(") {
            let value = first_quoted_value(trimmed).unwrap_or_else(|| ".env".to_string());
            configs.push(framework_config(
                "dotenv",
                format!("dotenv config:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }

        if let Some(setting) = express_setting(trimmed) {
            configs.push(framework_config(
                "express",
                format!("express setting:{setting}"),
                "setting",
                Some(setting),
                line_number,
            ));
        }
    }
    configs
}

fn rust_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("dotenv") && lower.contains("dotenv(") {
            configs.push(framework_config(
                "dotenv",
                "dotenv config:.env".to_string(),
                "env_file",
                Some(".env".to_string()),
                line_number,
            ));
        }

        if lower.contains("environment::with_prefix(")
            && let Some(value) = first_quoted_value_after(trimmed, "Environment::with_prefix(")
        {
            configs.push(framework_config(
                "config-rs",
                format!("config-rs env prefix:{value}"),
                "env_prefix",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn go_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("viper.setconfigname(")
            && let Some(value) = first_quoted_value_after(trimmed, "SetConfigName(")
        {
            configs.push(framework_config(
                "viper",
                format!("viper config:{value}"),
                "config_name",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("viper.addconfigpath(")
            && let Some(value) = first_quoted_value_after(trimmed, "AddConfigPath(")
        {
            configs.push(framework_config(
                "viper",
                format!("viper config path:{value}"),
                "config_path",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("godotenv.load(") {
            let value =
                first_quoted_value_after(trimmed, "Load(").unwrap_or_else(|| ".env".to_string());
            configs.push(framework_config(
                "godotenv",
                format!("godotenv config:{value}"),
                "env_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn php_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("config(")
            && let Some(value) = first_quoted_value_after(trimmed, "config(")
        {
            configs.push(framework_config(
                "laravel",
                format!("laravel config key:{value}"),
                "config_key",
                Some(value),
                line_number,
            ));
        }

        if lower.contains("->configure(")
            && let Some(value) = first_quoted_value_after(trimmed, "->configure(")
        {
            configs.push(framework_config(
                "lumen",
                format!("lumen config:{value}"),
                "config_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn bash_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if let Some(value) = sourced_shell_config(trimmed) {
            configs.push(framework_config(
                "shell",
                format!("shell config:{value}"),
                "source_file",
                Some(value),
                line_number,
            ));
        }
    }
    configs
}

fn framework_config(
    framework: &str,
    label: String,
    config_kind: &str,
    value: Option<String>,
    line: u32,
) -> FrameworkConfig {
    FrameworkConfig {
        framework: framework.to_string(),
        label,
        config_kind: config_kind.to_string(),
        value,
        line,
    }
}

fn express_setting(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let setting_index = lower
        .find(".set(")
        .or_else(|| lower.find("app.set("))
        .or_else(|| lower.find("server.set("))?;
    let receiver = line[..setting_index]
        .rsplit(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        })
        .next()
        .unwrap_or("")
        .trim_start_matches('$');
    if !["app", "server", "router"].iter().any(|allowed| {
        receiver.eq_ignore_ascii_case(allowed)
            || lower[setting_index..].starts_with(&format!("{allowed}.set("))
    }) {
        return None;
    }
    first_quoted_value(line)
}

fn sourced_shell_config(line: &str) -> Option<String> {
    let without_comment = line.split('#').next().unwrap_or("").trim();
    let rest = without_comment
        .strip_prefix("source ")
        .or_else(|| without_comment.strip_prefix(". "))?
        .trim();
    let value = rest
        .split_whitespace()
        .next()
        .map(|value| value.trim_matches(['"', '\'']).to_string())?;
    if value.contains("env") || value.contains("config") || value.ends_with(".conf") {
        Some(value)
    } else {
        None
    }
}

fn python_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = Vec::new();
    let mut pending = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with('@') {
            if let Some(mut route) = route_from_python_decorator(trimmed, line_number) {
                route.handler = None;
                pending.push(route);
            }
            continue;
        }

        if let Some(function) = trimmed
            .strip_prefix("def ")
            .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
            .filter(|name| !name.is_empty())
        {
            for mut route in pending.drain(..) {
                route.handler = Some(function.to_string());
                routes.push(route);
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            pending.clear();
        }
    }

    routes
}

fn route_from_python_decorator(line: &str, line_number: u32) -> Option<FrameworkRoute> {
    let lower = line.to_ascii_lowercase();
    if !(lower.contains(".route(")
        || route_methods()
            .iter()
            .any(|method| lower.contains(&format!(".{}(", method.to_ascii_lowercase()))))
    {
        return None;
    }
    let path = first_quoted_value(line)?;
    let method = route_methods()
        .iter()
        .find(|method| lower.contains(&format!(".{}(", method.to_ascii_lowercase())))
        .copied()
        .or_else(|| method_from_python_route_methods(line))
        .unwrap_or("ROUTE")
        .to_string();
    let framework = if method != "ROUTE" || lower.contains("fastapi") || lower.contains("router.") {
        "fastapi"
    } else {
        "flask"
    };

    Some(FrameworkRoute {
        framework: framework.to_string(),
        method,
        path,
        handler: None,
        line: line_number,
    })
}

fn method_from_python_route_methods(line: &str) -> Option<&'static str> {
    let lower = line.to_ascii_uppercase();
    route_methods()
        .iter()
        .find(|method| {
            lower.contains(&format!("\"{method}\"")) || lower.contains(&format!("'{method}'"))
        })
        .copied()
}

fn js_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            route_from_call_line(
                line,
                index as u32 + 1,
                "express",
                &["app", "router", "server", "routes"],
            )
        })
        .collect()
}

fn rust_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            let trimmed = line.trim();
            if !trimmed.contains(".route(") {
                return None;
            }
            let path = first_quoted_value(trimmed)?;
            let method = route_methods()
                .iter()
                .find(|method| trimmed.contains(&format!("{}(", method.to_ascii_lowercase())))
                .copied()
                .unwrap_or("ROUTE")
                .to_string();
            let handler = handler_from_rust_route(trimmed);
            Some(FrameworkRoute {
                framework: "axum".to_string(),
                method,
                path,
                handler,
                line: line_number,
            })
        })
        .collect()
}

fn go_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            let trimmed = line.trim();
            if trimmed.contains("HandleFunc(") {
                let path = first_quoted_value(trimmed)?;
                let handler = handler_after_first_comma(trimmed);
                return Some(FrameworkRoute {
                    framework: "net/http".to_string(),
                    method: "ROUTE".to_string(),
                    path,
                    handler,
                    line: line_number,
                });
            }
            route_from_call_line(
                trimmed,
                line_number,
                "go-router",
                &["r", "router", "engine", "api", "group", "v1", "v2"],
            )
        })
        .collect()
}

fn php_framework_routes(source: &str) -> Vec<FrameworkRoute> {
    let mut routes = Vec::new();
    let mut pending = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();
        if trimmed.starts_with("#[") && trimmed.contains("Route(") {
            if let Some(path) = first_quoted_value(trimmed) {
                pending.push(FrameworkRoute {
                    framework: "php-attribute".to_string(),
                    method: method_from_php_route(trimmed)
                        .unwrap_or("ROUTE")
                        .to_string(),
                    path,
                    handler: None,
                    line: line_number,
                });
            }
            continue;
        }
        if let Some(function) = trimmed
            .strip_prefix("function ")
            .and_then(|rest| rest.split_once('(').map(|(name, _)| name.trim()))
            .filter(|name| !name.is_empty())
        {
            for mut route in pending.drain(..) {
                route.handler = Some(function.to_string());
                routes.push(route);
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            pending.clear();
        }
    }

    routes
}

fn route_from_call_line(
    line: &str,
    line_number: u32,
    framework: &str,
    allowed_receivers: &[&str],
) -> Option<FrameworkRoute> {
    let lower = line.to_ascii_lowercase();
    let method = route_methods()
        .iter()
        .find(|method| {
            let method = method.to_ascii_lowercase();
            route_receiver_matches(&lower, &method, allowed_receivers)
        })
        .copied()?;
    let path = first_quoted_value(line)?;
    let handler = handler_after_first_comma(line);
    Some(FrameworkRoute {
        framework: framework.to_string(),
        method: method.to_string(),
        path,
        handler,
        line: line_number,
    })
}

fn route_receiver_matches(line: &str, method: &str, allowed_receivers: &[&str]) -> bool {
    let Some(method_index) = line
        .find(&format!(".{method}("))
        .or_else(|| line.find(&format!("->{method}(")))
    else {
        return false;
    };
    let receiver = line[..method_index]
        .rsplit(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        })
        .next()
        .unwrap_or("")
        .trim_start_matches('$');
    allowed_receivers
        .iter()
        .any(|allowed| receiver.eq_ignore_ascii_case(allowed))
}

fn handler_from_rust_route(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    for method in route_methods() {
        let needle = format!("{}(", method.to_ascii_lowercase());
        if let Some(start) = lower.find(&needle) {
            let rest = &line[start + needle.len()..];
            let handler = rest
                .split([',', ')'])
                .next()
                .map(|value| value.trim().trim_start_matches("move ").trim())
                .filter(|value| !value.is_empty())?;
            return Some(handler.to_string());
        }
    }
    None
}

fn handler_after_first_comma(line: &str) -> Option<String> {
    let handler = line
        .split_once(',')
        .map(|(_, rest)| rest.trim())
        .and_then(|rest| rest.split([',', ')']).next())
        .map(|value| {
            value
                .trim()
                .trim_start_matches('&')
                .trim_start_matches("::")
                .trim_matches(['"', '\'', '`'])
                .to_string()
        })
        .filter(|value| !value.is_empty())?;
    if handler.starts_with('|') || handler.starts_with("function") || handler.starts_with("async") {
        None
    } else {
        Some(handler)
    }
}

fn method_from_php_route(line: &str) -> Option<&'static str> {
    let upper = line.to_ascii_uppercase();
    route_methods()
        .iter()
        .find(|method| {
            upper.contains(&format!("\"{method}\"")) || upper.contains(&format!("'{method}'"))
        })
        .copied()
}

fn first_quoted_value(value: &str) -> Option<String> {
    let quote_index = value.find(['"', '\'', '`'])?;
    let quote = value[quote_index..].chars().next()?;
    let rest = &value[quote_index + quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn first_quoted_value_after(value: &str, needle: &str) -> Option<String> {
    let lower_value = value.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let start = lower_value.find(&lower_needle)?;
    first_quoted_value(&value[start + needle.len()..])
}

fn route_methods() -> &'static [&'static str] {
    &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"]
}

fn requirements_dependencies(source: &str) -> Vec<ManifestDependency> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('-') {
                return None;
            }
            package_name_and_version_from_requirement(line)
                .map(|(name, version)| manifest_dependency(name, "runtime", "python", version))
        })
        .collect()
}

fn collect_toml_table_keys(
    value: &toml::Value,
    table_name: &str,
    dependency_kind: &str,
    ecosystem: &str,
    dependencies: &mut Vec<ManifestDependency>,
    cargo_workspace_dependencies: Option<&BTreeMap<String, Option<String>>>,
) {
    let Some(table) = value.get(table_name).and_then(|value| value.as_table()) else {
        return;
    };
    for (name, value) in table {
        let package_name = value
            .as_table()
            .and_then(|table| table.get("package"))
            .and_then(|value| value.as_str())
            .unwrap_or(name)
            .to_string();
        let version = dependency_version_from_toml_value(
            name,
            value,
            ecosystem,
            cargo_workspace_dependencies,
        );
        dependencies.push(manifest_dependency(
            package_name,
            dependency_kind,
            ecosystem,
            version,
        ));
    }
}

fn collect_toml_entrypoint_keys(
    value: &toml::Value,
    table_name: &str,
    entrypoint_kind: &str,
    ecosystem: &str,
    entrypoints: &mut Vec<ManifestEntrypoint>,
) {
    let Some(table) = value.get(table_name).and_then(|value| value.as_table()) else {
        return;
    };
    for (name, target) in table {
        entrypoints.push(manifest_entrypoint(
            format!("{ecosystem} {entrypoint_kind}:{name}"),
            entrypoint_kind,
            ecosystem,
            target.as_str().map(str::to_string),
        ));
    }
}

fn collect_json_object_keys(
    value: &serde_json::Value,
    object_name: &str,
    dependency_kind: &str,
    ecosystem: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(object) = value.get(object_name).and_then(|value| value.as_object()) else {
        return;
    };
    for (name, value) in object {
        let version = value.as_str().map(str::to_string);
        dependencies.push(manifest_dependency(
            name.clone(),
            dependency_kind,
            ecosystem,
            version,
        ));
    }
}

fn cargo_workspace_dependencies(root: &Path) -> BTreeMap<String, Option<String>> {
    let Ok(source) = fs::read_to_string(root.join("Cargo.toml")) else {
        return BTreeMap::new();
    };
    let Ok(value) = toml::from_str::<toml::Value>(&source) else {
        return BTreeMap::new();
    };
    let Some(table) = value
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|dependencies| dependencies.as_table())
    else {
        return BTreeMap::new();
    };

    table
        .iter()
        .map(|(name, value)| (name.clone(), direct_toml_dependency_version(value)))
        .collect()
}

fn go_module_roots(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<GoModuleRoot> {
    let mut modules = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("go.mod")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Some(module) = go_module_name(&source) else {
            continue;
        };
        let dir = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        modules.push(GoModuleRoot { module, dir });
    }
    modules.sort_by(|left, right| {
        right
            .module
            .len()
            .cmp(&left.module.len())
            .then_with(|| left.dir.cmp(&right.dir))
    });
    modules
}

fn c_include_dirs(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<String> {
    let mut dirs = cmake_include_dirs(root, options, ignored_globs);
    dirs.extend(compile_commands_include_dirs(root, options, ignored_globs));
    dedup_preserving_order(&mut dirs);
    dirs
}

fn cmake_include_dirs(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<String> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("CMakeLists.txt")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let base = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        dirs.extend(cmake_include_dirs_from_source(base.as_deref(), &source));
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

fn compile_commands_include_dirs(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<String> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("compile_commands.json")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let base = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        dirs.extend(compile_commands_include_dirs_from_source(
            root,
            base.as_deref(),
            &source,
        ));
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

fn compile_commands_include_dirs_from_source(
    root: &Path,
    base: Option<&str>,
    source: &str,
) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let Some(commands) = value.as_array() else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    for command in commands {
        let command_base = compile_command_base(root, base, command);
        if let Some(arguments) = command.get("arguments").and_then(|value| value.as_array()) {
            let args = arguments
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>();
            dirs.extend(include_dirs_from_compiler_args(
                command_base.as_deref(),
                &args,
            ));
        } else if let Some(command_line) = command.get("command").and_then(|value| value.as_str()) {
            let args = split_command_tokens(command_line);
            dirs.extend(include_dirs_from_compiler_args(
                command_base.as_deref(),
                &args,
            ));
        }
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

fn compile_command_base(
    root: &Path,
    base: Option<&str>,
    command: &serde_json::Value,
) -> Option<String> {
    command
        .get("directory")
        .and_then(|value| value.as_str())
        .and_then(|directory| normalize_compile_command_directory(root, base, directory))
        .or_else(|| base.map(str::to_string))
}

fn normalize_compile_command_directory(
    root: &Path,
    base: Option<&str>,
    directory: &str,
) -> Option<String> {
    let value = directory.trim();
    if value.is_empty() {
        return base.map(str::to_string);
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return path
            .strip_prefix(root)
            .ok()
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .map(|relative| {
                if relative.is_empty() {
                    ".".to_string()
                } else {
                    relative
                }
            });
    }
    Some(join_path(base, value))
}

fn include_dirs_from_compiler_args(base: Option<&str>, args: &[String]) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].trim();
        let mut consumed_next = false;
        let candidate = if let Some(rest) = arg.strip_prefix("-I") {
            if rest.is_empty() {
                consumed_next = true;
                args.get(index + 1).map(String::as_str)
            } else {
                Some(rest)
            }
        } else if let Some(rest) = arg.strip_prefix("-isystem") {
            if rest.is_empty() {
                consumed_next = true;
                args.get(index + 1).map(String::as_str)
            } else {
                Some(rest)
            }
        } else if let Some(rest) = arg.strip_prefix("-iquote") {
            if rest.is_empty() {
                consumed_next = true;
                args.get(index + 1).map(String::as_str)
            } else {
                Some(rest)
            }
        } else if matches!(arg, "/I" | "-idirafter") {
            consumed_next = true;
            args.get(index + 1).map(String::as_str)
        } else if arg.starts_with("/I") {
            arg.strip_prefix("/I").filter(|rest| !rest.is_empty())
        } else {
            None
        };

        if let Some(candidate) = candidate.and_then(|value| compiler_include_dir_arg(base, value)) {
            dirs.push(candidate);
        }
        index += if consumed_next { 2 } else { 1 };
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

fn compiler_include_dir_arg(base: Option<&str>, arg: &str) -> Option<String> {
    let value = arg.trim().trim_matches(['"', '\'']);
    if value.is_empty() || value.starts_with('$') || value.starts_with('<') {
        return None;
    }
    if Path::new(value).is_absolute() {
        return None;
    }
    let path = join_path(base, value);
    if path.is_empty() { None } else { Some(path) }
}

fn cmake_include_dirs_from_source(base: Option<&str>, source: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    for body in cmake_command_bodies(source, "include_directories") {
        for arg in cmake_command_args(&body) {
            if let Some(dir) = cmake_include_dir_arg(base, &arg) {
                dirs.push(dir);
            }
        }
    }
    for body in cmake_command_bodies(source, "target_include_directories") {
        for arg in cmake_command_args(&body).into_iter().skip(1) {
            if is_cmake_include_scope_or_option(&arg) {
                continue;
            }
            if let Some(dir) = cmake_include_dir_arg(base, &arg) {
                dirs.push(dir);
            }
        }
    }
    dedup_preserving_order(&mut dirs);
    dirs
}

fn cmake_include_dir_arg(base: Option<&str>, arg: &str) -> Option<String> {
    let mut value = arg.trim().trim_matches(['"', '\'']).to_string();
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('$') && !value.starts_with("${")
        || value.starts_with("$<")
        || is_cmake_include_scope_or_option(&value)
    {
        return None;
    }

    let current_dir = base.unwrap_or(".");
    let root_relative =
        value.contains("${PROJECT_SOURCE_DIR}") || value.contains("${CMAKE_SOURCE_DIR}");
    value = value
        .replace("${CMAKE_CURRENT_SOURCE_DIR}", current_dir)
        .replace("${CMAKE_CURRENT_LIST_DIR}", current_dir)
        .replace("${PROJECT_SOURCE_DIR}", ".")
        .replace("${CMAKE_SOURCE_DIR}", ".");
    if value.contains('$') || value.starts_with('/') {
        return None;
    }

    let path = if value == "." {
        if root_relative {
            ".".to_string()
        } else {
            current_dir.to_string()
        }
    } else if root_relative {
        normalize_path(&value)
    } else {
        join_path(base, &value)
    };
    if path.is_empty() { None } else { Some(path) }
}

fn is_cmake_include_scope_or_option(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "PUBLIC" | "PRIVATE" | "INTERFACE" | "SYSTEM" | "BEFORE" | "AFTER"
    )
}

fn dependency_version_from_toml_value(
    name: &str,
    value: &toml::Value,
    ecosystem: &str,
    cargo_workspace_dependencies: Option<&BTreeMap<String, Option<String>>>,
) -> Option<String> {
    if ecosystem == "cargo"
        && value
            .as_table()
            .and_then(|table| table.get("workspace"))
            .and_then(|value| value.as_bool())
            .is_some_and(|enabled| enabled)
    {
        return cargo_workspace_dependencies
            .and_then(|dependencies| dependencies.get(name))
            .cloned()
            .flatten();
    }
    direct_toml_dependency_version(value)
}

fn direct_toml_dependency_version(value: &toml::Value) -> Option<String> {
    if let Some(version) = value.as_str() {
        return Some(version.to_string());
    }
    let table = value.as_table()?;
    table
        .get("version")
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn package_name_and_version_from_requirement(
    requirement: &str,
) -> Option<(String, Option<String>)> {
    let trimmed = requirement.trim();
    let end = trimmed
        .find(|character: char| {
            matches!(
                character,
                '<' | '>' | '=' | '!' | '~' | '[' | ';' | ',' | ' '
            )
        })
        .unwrap_or(trimmed.len());
    let name = trimmed[..end].trim();
    if name.is_empty() {
        None
    } else {
        let version = trimmed[end..].trim();
        Some((
            name.to_string(),
            (!version.is_empty()).then(|| version.to_string()),
        ))
    }
}

fn package_id(ecosystem: &str, package_name: &str) -> String {
    format!("{ecosystem}:{package_name}")
}

fn canonical_package_name(ecosystem: &str, name: &str) -> String {
    let trimmed = name.trim();
    match ecosystem {
        "python" => {
            let mut normalized = String::new();
            let mut previous_separator = false;
            for character in trimmed.chars() {
                if matches!(character, '-' | '_' | '.') {
                    if !previous_separator {
                        normalized.push('-');
                    }
                    previous_separator = true;
                } else {
                    normalized.extend(character.to_lowercase());
                    previous_separator = false;
                }
            }
            normalized
        }
        "cargo" | "npm" | "composer" => trimmed.to_ascii_lowercase(),
        "go" => trimmed.to_string(),
        _ => trimmed.to_string(),
    }
}

fn local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    cmake_include_dirs: &[String],
) -> Option<LocalImportTarget> {
    match language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            js_local_import_target(source_label, import_label)
        }
        Language::Python => python_local_import_target(source_label, import_label),
        Language::C | Language::Cpp => {
            c_local_import_target(source_label, import_label, cmake_include_dirs)
        }
        Language::Php => php_local_import_target(source_label, import_label),
        Language::Bash => bash_local_import_target(source_label, import_label),
        Language::Rust => rust_local_import_target(source_label, import_label),
        Language::Go => go_local_import_target(source_label, import_label),
    }
}

fn possible_local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    go_modules: &[GoModuleRoot],
) -> Option<LocalImportTarget> {
    match language {
        Language::Python => python_absolute_local_import_target(source_label, import_label),
        Language::Go => go_module_import_target(import_label, go_modules),
        _ => None,
    }
}

fn js_local_import_target(source_label: &str, import_label: &str) -> Option<LocalImportTarget> {
    let module = first_quoted_value(import_label)?;
    if !(module.starts_with("./") || module.starts_with("../")) {
        return None;
    }
    Some(LocalImportTarget {
        target: module.clone(),
        candidates: module_file_candidates(
            source_label,
            &module,
            &["js", "ts", "tsx", "mjs", "cjs"],
        ),
    })
}

fn python_absolute_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let value = import_label.trim();
    let (module, imported) = if let Some(rest) = value.strip_prefix("import ") {
        (
            rest.split([',', ' ', '\n', '\t'])
                .find(|part| !part.is_empty())?,
            None,
        )
    } else if let Some(rest) = value.strip_prefix("from ") {
        let module = rest.split_whitespace().next()?;
        if module.starts_with('.') {
            return None;
        }
        let imported = rest.split_once(" import ").and_then(|(_, imported)| {
            imported
                .split([',', ' ', '\n', '\t'])
                .find(|part| !part.is_empty())
        });
        (module, imported)
    } else {
        return None;
    };
    if module.is_empty() || module.starts_with('.') {
        return None;
    }

    let relative = module.replace('.', "/");
    let mut candidates = Vec::new();
    if let Some(imported) = imported {
        candidates.extend(python_module_candidates(&format!(
            "{relative}/{}",
            imported.replace('.', "/")
        )));
    }
    candidates.extend(python_module_candidates(&relative));
    if let Some(dir) = path_dir(source_label) {
        if let Some(imported) = imported {
            candidates.extend(python_module_candidates(&join_path(
                Some(&dir),
                &format!("{relative}/{}", imported.replace('.', "/")),
            )));
        }
        candidates.extend(python_module_candidates(&join_path(Some(&dir), &relative)));
    }
    dedup_preserving_order(&mut candidates);

    Some(LocalImportTarget {
        target: module.to_string(),
        candidates,
    })
}

fn python_local_import_target(source_label: &str, import_label: &str) -> Option<LocalImportTarget> {
    let value = import_label.trim();
    let rest = value.strip_prefix("from ")?;
    let dot_count = rest
        .chars()
        .take_while(|character| *character == '.')
        .count();
    if dot_count == 0 {
        return None;
    }
    let rest = &rest[dot_count..];
    let (module, imported) = rest.split_once(" import ")?;
    let imported = imported
        .split([',', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())
        .unwrap_or("");
    let target = if module.trim().is_empty() {
        imported.to_string()
    } else {
        module.trim().to_string()
    };
    if target.is_empty() {
        return None;
    }

    let mut base = path_dir(source_label);
    for _ in 1..dot_count {
        if let Some(parent) = base.as_deref().and_then(path_dir) {
            base = Some(parent);
        } else {
            base = None;
        }
    }
    let relative = target.replace('.', "/");
    let module_path = join_path(base.as_deref(), &relative);
    let candidates = python_module_candidates(&module_path);
    Some(LocalImportTarget {
        target: format!("{}{}", ".".repeat(dot_count), target),
        candidates,
    })
}

fn python_module_candidates(module_path: &str) -> Vec<String> {
    let mut candidates = with_file_extensions(module_path, &["py"]);
    candidates.push(normalize_path(&format!("{module_path}/__init__.py")));
    candidates
}

fn dedup_preserving_order(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn c_local_import_target(
    source_label: &str,
    import_label: &str,
    cmake_include_dirs: &[String],
) -> Option<LocalImportTarget> {
    let header = first_quoted_value(import_label)?;
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &header)];
    candidates.extend(
        cmake_include_dirs
            .iter()
            .map(|include_dir| join_path(Some(include_dir), &header)),
    );
    dedup_preserving_order(&mut candidates);
    Some(LocalImportTarget {
        target: header.clone(),
        candidates,
    })
}

fn php_local_import_target(source_label: &str, import_label: &str) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if path.contains("://") || path.starts_with('/') {
        return None;
    }
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &path)];
    if !path_has_extension(&path) {
        candidates.push(join_path(
            path_dir(source_label).as_deref(),
            &format!("{path}.php"),
        ));
    }
    Some(LocalImportTarget {
        target: path,
        candidates,
    })
}

fn bash_local_import_target(source_label: &str, import_label: &str) -> Option<LocalImportTarget> {
    let mut parts = import_label.split_whitespace();
    let command = parts.next()?;
    if !matches!(command, "source" | ".") {
        return None;
    }
    let path = parts.next()?.trim_matches(['"', '\'']);
    if path.starts_with('/') || path.starts_with('$') || path.contains("://") {
        return None;
    }
    Some(LocalImportTarget {
        target: path.to_string(),
        candidates: vec![join_path(path_dir(source_label).as_deref(), path)],
    })
}

fn rust_local_import_target(source_label: &str, import_label: &str) -> Option<LocalImportTarget> {
    let value = import_label.trim().strip_prefix("use ")?.trim();
    let (base, rest) = if let Some(rest) = value.strip_prefix("crate::") {
        (rust_crate_root(source_label), rest)
    } else if let Some(rest) = value.strip_prefix("self::") {
        (path_dir(source_label), rest)
    } else if let Some(rest) = value.strip_prefix("super::") {
        (
            path_dir(source_label).and_then(|path| path_dir(&path)),
            rest,
        )
    } else {
        return None;
    };
    let module = rest
        .split([':', ';', ',', '{', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())?;
    if module.is_empty() || matches!(module, "self" | "super" | "crate") {
        return None;
    }
    let module_path = join_path(base.as_deref(), module);
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates: vec![
            normalize_path(&format!("{module_path}.rs")),
            normalize_path(&format!("{module_path}/mod.rs")),
        ],
    })
}

fn go_local_import_target(source_label: &str, import_label: &str) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if !(path.starts_with("./") || path.starts_with("../")) {
        return None;
    }
    let package_dir = join_path(path_dir(source_label).as_deref(), &path);
    Some(LocalImportTarget {
        target: path,
        candidates: vec![directory_candidate(&package_dir)],
    })
}

fn go_module_import_target(
    import_label: &str,
    go_modules: &[GoModuleRoot],
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if path.starts_with("./") || path.starts_with("../") || path.starts_with('/') {
        return None;
    }
    let module = go_modules
        .iter()
        .find(|module| path == module.module || path.starts_with(&format!("{}/", module.module)))?;
    let suffix = path
        .strip_prefix(&module.module)
        .unwrap_or("")
        .trim_start_matches('/');
    let package_dir = join_path(module.dir.as_deref(), suffix);
    Some(LocalImportTarget {
        target: path,
        candidates: vec![directory_candidate(&package_dir)],
    })
}

fn directory_candidate(path: &str) -> String {
    let normalized = normalize_path(path);
    if normalized.is_empty() {
        "/".to_string()
    } else {
        format!("{normalized}/")
    }
}

fn module_file_candidates(source_label: &str, module: &str, extensions: &[&str]) -> Vec<String> {
    let path = join_path(path_dir(source_label).as_deref(), module);
    let mut candidates = with_file_extensions(&path, extensions);
    for extension in extensions {
        candidates.push(normalize_path(&format!("{path}/index.{extension}")));
    }
    candidates
}

fn with_file_extensions(path: &str, extensions: &[&str]) -> Vec<String> {
    if path_has_extension(path) {
        vec![normalize_path(path)]
    } else {
        extensions
            .iter()
            .map(|extension| normalize_path(&format!("{path}.{extension}")))
            .collect()
    }
}

fn path_has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

fn rust_crate_root(source_label: &str) -> Option<String> {
    if source_label == "src/main.rs" || source_label == "src/lib.rs" {
        return Some("src".to_string());
    }
    source_label
        .strip_prefix("src/")
        .map(|_| "src".to_string())
        .or_else(|| path_dir(source_label))
}

fn path_dir(path: &str) -> Option<String> {
    path.rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .filter(|dir| !dir.is_empty())
}

fn join_path(base: Option<&str>, relative: &str) -> String {
    let path = match base {
        Some(base) if !base.is_empty() => format!("{base}/{relative}"),
        _ => relative.to_string(),
    };
    normalize_path(&path)
}

fn normalize_path(path: &str) -> String {
    let mut parts = Vec::new();
    let normalized = path.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}

fn manifest_dependency(
    name: impl Into<String>,
    dependency_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    version: Option<String>,
) -> ManifestDependency {
    ManifestDependency {
        name: name.into(),
        kind: dependency_kind.into(),
        ecosystem: ecosystem.into(),
        version,
    }
}

fn manifest_entrypoint(
    label: impl Into<String>,
    entrypoint_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    target: Option<String>,
) -> ManifestEntrypoint {
    ManifestEntrypoint {
        label: label.into(),
        kind: entrypoint_kind.into(),
        ecosystem: ecosystem.into(),
        target,
    }
}

fn resolve_pending_calls(context: &mut IndexContext) {
    let pending_calls = std::mem::take(&mut context.pending_calls);

    for call in pending_calls {
        let targets = resolve_function_targets(&context.function_symbols, &call.label);
        if targets.is_empty() {
            let mut metadata = BTreeMap::new();
            metadata.insert("language".to_string(), call.language);
            metadata.insert("parser".to_string(), "tree-sitter".to_string());
            metadata.insert("item_kind".to_string(), "call".to_string());
            metadata.insert("resolution".to_string(), "unresolved".to_string());
            let call_id = context.graph.add_node_with_metadata(
                NodeKind::ExternalDependency,
                call.label,
                Some(call.span),
                metadata,
            );
            add_edge_once(
                &mut context.graph,
                call.caller,
                call_id,
                EdgeKind::Calls,
                Confidence::Heuristic,
            );
            continue;
        }

        for target in targets {
            add_edge_once(
                &mut context.graph,
                call.caller,
                target,
                EdgeKind::Calls,
                Confidence::Heuristic,
            );
        }
    }
}

fn resolve_pending_local_imports(context: &mut IndexContext) {
    let pending_imports = std::mem::take(&mut context.pending_local_imports);

    for import in pending_imports {
        let resolved = resolve_local_import_candidate(&context.file_nodes, &import.candidates);

        if let Some((candidate, file_id)) = resolved {
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "import_scope",
                "local",
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "import_target",
                import.target.clone(),
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolution",
                "resolved",
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolved_path",
                candidate,
            );
            let mut metadata = BTreeMap::new();
            metadata.insert("relation".to_string(), "local_import_file".to_string());
            metadata.insert("source".to_string(), "syntax".to_string());
            metadata.insert("resolution".to_string(), "local_import_file".to_string());
            metadata.insert("target".to_string(), import.target);
            add_edge_once_with_metadata(
                &mut context.graph,
                import.import_node,
                file_id,
                EdgeKind::References,
                Confidence::Syntactic,
                metadata,
            );
        } else if import.mark_unresolved {
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "resolution",
                "unresolved",
            );
            add_node_metadata(
                &mut context.graph,
                import.import_node,
                "candidate_paths",
                import.candidates.join(","),
            );
        }
    }
}

fn resolve_local_import_candidate(
    file_nodes: &BTreeMap<String, NodeId>,
    candidates: &[String],
) -> Option<(String, NodeId)> {
    for candidate in candidates {
        if let Some(file_id) = file_nodes.get(candidate).copied() {
            return Some((candidate.clone(), file_id));
        }
        if let Some((path, file_id)) = resolve_directory_import_candidate(file_nodes, candidate) {
            return Some((path, file_id));
        }
    }
    None
}

fn resolve_directory_import_candidate(
    file_nodes: &BTreeMap<String, NodeId>,
    candidate: &str,
) -> Option<(String, NodeId)> {
    let prefix = candidate.strip_suffix('/')?;
    let mut package_files = file_nodes.iter().filter(|(path, _)| {
        is_go_file(path)
            && if prefix.is_empty() {
                !path.contains('/')
            } else {
                path.strip_prefix(prefix)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .is_some_and(|rest| !rest.contains('/'))
            }
    });

    package_files
        .find(|(path, _)| !path.ends_with("_test.go"))
        .or_else(|| package_files.next())
        .map(|(path, file_id)| (path.clone(), *file_id))
}

fn is_go_file(path: &str) -> bool {
    path.ends_with(".go")
}

fn resolve_pending_entrypoint_targets(context: &mut IndexContext) {
    let pending_targets = std::mem::take(&mut context.pending_entrypoint_targets);

    for pending in pending_targets {
        for candidate in entrypoint_target_candidates(&pending) {
            if let Some(file_id) = context.file_nodes.get(&candidate.path).copied() {
                add_entrypoint_reference(
                    &mut context.graph,
                    pending.entrypoint,
                    file_id,
                    "entrypoint_file",
                    candidate.resolution,
                    candidate.file_confidence,
                    None,
                );
            }

            let Some(symbol) = candidate.symbol.as_deref() else {
                continue;
            };
            let function_targets =
                function_targets_in_file(&context.graph, &candidate.path, symbol);
            for target in function_targets {
                add_entrypoint_reference(
                    &mut context.graph,
                    pending.entrypoint,
                    target,
                    "entrypoint_function",
                    candidate.resolution,
                    candidate.function_confidence,
                    Some(symbol),
                );
            }
        }
    }
}

fn entrypoint_target_candidates(
    pending: &PendingEntrypointTarget,
) -> Vec<EntrypointTargetCandidate> {
    match pending.ecosystem.as_str() {
        "cargo" => manifest_path_candidate(
            pending,
            &pending.target,
            Some("main".to_string()),
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "python" => python_entrypoint_candidates(pending),
        "go" => manifest_path_candidate(
            pending,
            &pending.target,
            Some("main".to_string()),
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "cmake" => manifest_path_candidate(
            pending,
            &pending.target,
            Some("main".to_string()),
            Confidence::Exact,
            Confidence::Syntactic,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "npm" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "command_path",
            })
            .into_iter()
            .collect(),
        "composer" if pending.entrypoint_kind == "binary" => manifest_path_candidate(
            pending,
            &pending.target,
            None,
            Confidence::Exact,
            Confidence::Exact,
            "manifest_path",
        )
        .into_iter()
        .collect(),
        "composer" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "command_path",
            })
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn manifest_path_candidate(
    pending: &PendingEntrypointTarget,
    target: &str,
    symbol: Option<String>,
    file_confidence: Confidence,
    function_confidence: Confidence,
    resolution: &'static str,
) -> Option<EntrypointTargetCandidate> {
    normalize_manifest_relative_path(&pending.manifest_label, target).map(|path| {
        EntrypointTargetCandidate {
            path,
            symbol,
            file_confidence,
            function_confidence,
            resolution,
        }
    })
}

fn python_entrypoint_candidates(
    pending: &PendingEntrypointTarget,
) -> Vec<EntrypointTargetCandidate> {
    let Some((module, symbol)) = pending.target.split_once(':') else {
        return Vec::new();
    };
    let module = module.trim();
    let symbol = simple_symbol_name(symbol.trim());
    if module.is_empty() || symbol.is_empty() {
        return Vec::new();
    }

    let module_path = module.replace('.', "/");
    [
        format!("{module_path}.py"),
        format!("{module_path}/__init__.py"),
    ]
    .into_iter()
    .filter_map(|path| {
        manifest_path_candidate(
            pending,
            &path,
            Some(symbol.clone()),
            Confidence::Heuristic,
            Confidence::Heuristic,
            "python_module",
        )
    })
    .collect()
}

fn command_path_candidate(pending: &PendingEntrypointTarget) -> Option<String> {
    command_source_path_candidate(&pending.target)
        .and_then(|path| normalize_manifest_relative_path(&pending.manifest_label, &path))
}

fn command_source_path_candidate(command: &str) -> Option<String> {
    split_command_tokens(command)
        .into_iter()
        .find(|token| is_command_path_candidate(token))
}

fn cmake_command_bodies(source: &str, command_name: &str) -> Vec<String> {
    let source = strip_cmake_comments(source);
    let lowered = source.to_ascii_lowercase();
    let needle = command_name.to_ascii_lowercase();
    let mut bodies = Vec::new();
    let mut search_from = 0;

    while let Some(offset) = lowered[search_from..].find(&needle) {
        let start = search_from + offset;
        let before = source[..start].chars().next_back();
        let after_name = start + needle.len();
        let after = source[after_name..].chars().next();
        if before.is_some_and(is_cmake_ident_char) || after.is_some_and(is_cmake_ident_char) {
            search_from = after_name;
            continue;
        }

        let Some(open_offset) = source[after_name..].find('(') else {
            break;
        };
        let open = after_name + open_offset;
        if !source[after_name..open]
            .chars()
            .all(|character| character.is_whitespace())
        {
            search_from = after_name;
            continue;
        }

        let Some((body, close)) = cmake_parenthesized_body(&source, open) else {
            break;
        };
        bodies.push(body);
        search_from = close + 1;
    }

    bodies
}

fn strip_cmake_comments(source: &str) -> String {
    let mut stripped = String::with_capacity(source.len());
    let mut quote = None;
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        match quote {
            Some(current_quote) if character == current_quote => {
                quote = None;
                stripped.push(character);
            }
            Some(_) => stripped.push(character),
            None if character == '"' || character == '\'' => {
                quote = Some(character);
                stripped.push(character);
            }
            None if character == '#' => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        stripped.push('\n');
                        break;
                    }
                }
            }
            None => stripped.push(character),
        }
    }
    stripped
}

fn cmake_parenthesized_body(source: &str, open: usize) -> Option<(String, usize)> {
    let mut depth = 0usize;
    let mut quote = None;
    let body_start = open + '('.len_utf8();

    for (index, character) in source[open..].char_indices() {
        let absolute = open + index;
        match quote {
            Some(current_quote) if character == current_quote => quote = None,
            Some(_) => {}
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character == '(' => depth += 1,
            None if character == ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((source[body_start..absolute].to_string(), absolute));
                }
            }
            None => {}
        }
    }

    None
}

fn cmake_command_args(body: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for character in body.chars() {
        match quote {
            Some(current_quote) if character == current_quote => quote = None,
            Some(_) => current.push(character),
            None if character == '"' || character == '\'' => quote = Some(character),
            None if character.is_whitespace() || character == ';' => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            None => current.push(character),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }

    args
}

fn is_cmake_source_argument(value: &str) -> bool {
    let value = value.trim();
    !value.starts_with('$') && !value.starts_with('<') && has_known_source_extension(value)
}

fn is_cmake_ident_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn split_command_tokens(command: &str) -> Vec<String> {
    command
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '"' | '\'' | '`' | ';' | '&' | '|')
        })
        .filter_map(clean_command_token)
        .collect()
}

fn clean_command_token(token: &str) -> Option<String> {
    let token = token
        .trim()
        .trim_matches(|character: char| matches!(character, '(' | ')' | '[' | ']' | '{' | '}'))
        .trim_matches(|character: char| matches!(character, ',' | ':'));
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn is_command_path_candidate(token: &str) -> bool {
    if token.starts_with('-')
        || token.starts_with('$')
        || token.contains("://")
        || token.contains('=')
        || token.contains('*')
    {
        return false;
    }
    token.contains('/') || has_known_source_extension(token)
}

fn has_known_source_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension,
                "rs" | "py"
                    | "pyw"
                    | "js"
                    | "mjs"
                    | "cjs"
                    | "ts"
                    | "mts"
                    | "cts"
                    | "tsx"
                    | "go"
                    | "c"
                    | "h"
                    | "cc"
                    | "cpp"
                    | "cxx"
                    | "hpp"
                    | "hh"
                    | "hxx"
                    | "php"
                    | "phtml"
                    | "sh"
                    | "bash"
                    | "zsh"
                    | "ksh"
            )
        })
}

fn normalize_manifest_relative_path(manifest_label: &str, target: &str) -> Option<String> {
    let target = target.trim().trim_matches('"').trim_matches('\'');
    if target.is_empty()
        || target.starts_with('-')
        || target.starts_with('$')
        || target.contains("://")
    {
        return None;
    }
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return None;
    }

    let base = Path::new(manifest_label)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let joined = base.map_or_else(|| PathBuf::from(target), |base| base.join(target));
    normalize_relative_path(&joined)
}

fn normalize_relative_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop()?;
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn function_targets_in_file(graph: &CodeGraph, path: &str, symbol: &str) -> Vec<NodeId> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Function
                && node.span.as_ref().is_some_and(|span| span.path == path)
                && function_symbol_matches(&node.label, symbol)
        })
        .map(|node| node.id)
        .collect()
}

fn function_symbol_matches(label: &str, symbol: &str) -> bool {
    symbol_keys(label)
        .into_iter()
        .any(|key| key == symbol || simple_symbol_name(&key) == symbol)
}

fn add_entrypoint_reference(
    graph: &mut CodeGraph,
    source: NodeId,
    target: NodeId,
    relation: &str,
    resolution: &str,
    confidence: Confidence,
    target_symbol: Option<&str>,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("relation".to_string(), relation.to_string());
    let fact_source = if resolution.starts_with("shebang") {
        "shebang"
    } else if resolution.starts_with("framework") {
        "framework"
    } else {
        "manifest"
    };
    metadata.insert("source".to_string(), fact_source.to_string());
    metadata.insert("resolution".to_string(), resolution.to_string());
    if let Some(target_symbol) = target_symbol {
        metadata.insert("target_symbol".to_string(), target_symbol.to_string());
    }
    add_edge_once_with_metadata(
        graph,
        source,
        target,
        EdgeKind::References,
        confidence,
        metadata,
    );
}

fn register_function_symbol(symbols: &mut BTreeMap<String, Vec<NodeId>>, label: &str, id: NodeId) {
    for key in symbol_keys(label) {
        let values = symbols.entry(key).or_default();
        if !values.contains(&id) {
            values.push(id);
        }
    }
}

fn register_local_function(symbols: &mut BTreeMap<String, NodeId>, label: &str, id: NodeId) {
    for key in symbol_keys(label) {
        symbols.entry(key).or_insert(id);
    }
}

fn resolve_local_function(symbols: &BTreeMap<String, NodeId>, label: &str) -> Option<NodeId> {
    symbol_keys(label)
        .into_iter()
        .find_map(|key| symbols.get(&key).copied())
}

fn resolve_function_targets(symbols: &BTreeMap<String, Vec<NodeId>>, label: &str) -> Vec<NodeId> {
    let mut targets = Vec::new();
    for key in symbol_keys(label) {
        if let Some(ids) = symbols.get(&key) {
            for id in ids {
                if !targets.contains(id) {
                    targets.push(*id);
                }
            }
        }
    }
    targets
}

fn symbol_keys(label: &str) -> Vec<String> {
    let compact = label.trim().trim_end_matches('!').to_string();
    let simple = simple_symbol_name(&compact);
    if compact == simple {
        vec![compact]
    } else {
        vec![compact, simple]
    }
}

fn simple_symbol_name(label: &str) -> String {
    label
        .rsplit([':', '.', '\\', '>'])
        .find(|part| !part.is_empty() && *part != "-")
        .unwrap_or(label)
        .trim()
        .to_string()
}

fn add_edge_once(
    graph: &mut CodeGraph,
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    confidence: Confidence,
) {
    add_edge_once_with_metadata(graph, source, target, kind, confidence, BTreeMap::new());
}

fn add_edge_once_with_metadata(
    graph: &mut CodeGraph,
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    confidence: Confidence,
    metadata: BTreeMap<String, String>,
) {
    if graph
        .edges
        .iter()
        .any(|edge| edge.source == source && edge.target == target && edge.kind == kind)
    {
        return;
    }
    graph.add_edge_with_metadata(source, target, kind, confidence, metadata);
}

fn add_file_metadata(
    graph: &mut CodeGraph,
    file_id: codegraph_core::NodeId,
    key: &str,
    value: impl Into<String>,
) {
    add_node_metadata(graph, file_id, key, value);
}

fn add_node_metadata(
    graph: &mut CodeGraph,
    node_id: codegraph_core::NodeId,
    key: &str,
    value: impl Into<String>,
) {
    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == node_id) {
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
        ParsedItemKind::Call => "call",
        ParsedItemKind::EnvironmentRead => "environment_read",
        ParsedItemKind::ConfigRead => "config_read",
        ParsedItemKind::Error => "error",
    }
}

fn is_symbol_item(kind: ParsedItemKind) -> bool {
    matches!(
        kind,
        ParsedItemKind::Function
            | ParsedItemKind::Entrypoint
            | ParsedItemKind::Type
            | ParsedItemKind::Module
            | ParsedItemKind::Import
    )
}

fn is_effect_item(kind: ParsedItemKind) -> bool {
    matches!(
        kind,
        ParsedItemKind::EnvironmentRead | ParsedItemKind::ConfigRead | ParsedItemKind::Error
    )
}

fn should_enter(
    entry: &DirEntry,
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> bool {
    if entry.path() == root {
        return true;
    }

    entry_exclusion(entry, root, options, ignored_globs).is_none()
}

fn entry_exclusion(
    entry: &DirEntry,
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Option<EntryExclusion> {
    if !options.include_hidden && is_hidden(entry) {
        return Some(EntryExclusion::Hidden);
    }

    if !options.include_ignored && is_ignored_name(entry, &options.ignored_names) {
        return Some(EntryExclusion::IgnoredName);
    }

    if !options.include_ignored && is_ignored_glob(entry.path(), root, ignored_globs) {
        return Some(EntryExclusion::IgnoredGlob);
    }

    None
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

fn is_ignored_glob(path: &Path, root: &Path, ignored_globs: &Option<GlobSet>) -> bool {
    let Some(ignored_globs) = ignored_globs else {
        return false;
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    !relative.is_empty() && ignored_globs.is_match(relative)
}

fn default_ignored_names() -> BTreeSet<String> {
    [
        ".git",
        ".codegraph",
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

pub fn is_index_relevant_file(path: &Path) -> bool {
    if Language::detect(path).is_some() {
        return true;
    }

    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "go.mod"
                | "pyproject.toml"
                | "setup.py"
                | "requirements.txt"
                | "composer.json"
                | "CMakeLists.txt"
                | "compile_commands.json"
        )
    )
}

fn is_probably_source_file(path: &Path, max_file_size: u64) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.len() <= max_file_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn scan_project_skips_default_ignored_directories() {
        let root = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::create_dir_all(root.join(".codegraph")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("target").join("debug.log"), "noise\n").unwrap();
        fs::write(root.join(".codegraph").join("graph.json"), "{}\n").unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let labels: Vec<_> = graph.nodes.iter().map(|node| node.label.as_str()).collect();

        assert!(labels.contains(&"src/main.rs"));
        assert!(!labels.contains(&"target"));
        assert!(!labels.contains(&"target/debug.log"));
        assert!(!labels.contains(&".codegraph"));
        assert!(!labels.contains(&".codegraph/graph.json"));

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
        }));

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
        fs::write(
            root.join("server.js"),
            r#"const port = process.env.PORT || "3000";
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

        let graph = scan_project(&root, &IndexOptions::default()).unwrap();
        let defaults = graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::Environment && node.label == "PORT")
            .filter_map(|node| node.metadata.get("default_value").map(String::as_str))
            .collect::<BTreeSet<_>>();

        assert_eq!(defaults, BTreeSet::from(["3000", "8000", "8080"]));

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
  "devDependencies": { "vite": "^7.0.0" }
}"#,
        )
        .unwrap();
        fs::write(
            root.join("go.mod"),
            "module example.com/demo\n\nrequire github.com/gin-gonic/gin v1.10.0\n",
        )
        .unwrap();
        fs::write(root.join("requirements.txt"), "fastapi==0.115.0\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = ["pydantic>=2"]
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
            "github.com/gin-gonic/gin",
            "fastapi",
            "pydantic",
            "monolog/monolog",
        ] {
            assert!(dependency_labels.contains(expected), "missing {expected}");
        }
        assert!(!dependency_labels.contains("php"));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn && edge.confidence == Confidence::Exact
        }));
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
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("dependency_version")
                    .is_some_and(|value| value == "v1.10.0")
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("dependency_version")
                    .is_some_and(|value| value == ">=2")
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("dependency_version")
                    .is_some_and(|value| value == "^3.0")
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
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Entrypoint && edge.confidence == Confidence::Exact
        }));
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
        let python_main = function_id_in_file(&graph, "main", "codegraph/cli.py");
        let composer_entrypoint =
            node_id(&graph, NodeKind::Entrypoint, "composer bin:bin/codegraph");
        let composer_file = node_id(&graph, NodeKind::File, "bin/codegraph");
        let cmake_entrypoint = node_id(&graph, NodeKind::Entrypoint, "cmake executable:demo_c");
        let cmake_file = node_id(&graph, NodeKind::File, "src/main.c");
        let cmake_main = function_id_in_file(&graph, "main", "src/main.c");
        let go_module_entrypoint =
            node_id(&graph, NodeKind::Entrypoint, "go module:example.com/demo");
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
            "use axum::{routing::get, Router};\nasync fn status() {}\nfn app() -> Router { Router::new().route(\"/status\", get(status)) }\n",
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
}
