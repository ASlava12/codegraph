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
    directory_nodes: BTreeMap<String, NodeId>,
    external_dependencies: BTreeMap<String, NodeId>,
    cargo_workspace_dependencies: BTreeMap<String, Option<String>>,
    go_modules: Vec<GoModuleRoot>,
    dart_packages: Vec<DartPackageRoot>,
    c_include_dirs: Vec<String>,
    custom_rules: CustomRules,
    annotations: GraphAnnotations,
    pending_calls: Vec<PendingCall>,
    pending_local_imports: Vec<PendingLocalImport>,
    pending_entrypoint_targets: Vec<PendingEntrypointTarget>,
    pending_compose_config_targets: Vec<PendingComposeConfigTarget>,
    pending_compose_volume_targets: Vec<PendingComposeVolumeTarget>,
    kubernetes_configs: BTreeMap<KubernetesConfigKey, NodeId>,
    kubernetes_services: BTreeMap<KubernetesServiceKey, NodeId>,
    pending_kubernetes_config_refs: Vec<PendingKubernetesConfigRef>,
    pending_kubernetes_service_refs: Vec<PendingKubernetesServiceRef>,
    pending_github_actions_local_actions: Vec<PendingGithubActionsLocalAction>,
    pending_document_path_refs: Vec<PendingDocumentPathRef>,
    pending_document_symbol_refs: Vec<PendingDocumentSymbolRef>,
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

struct PendingComposeConfigTarget {
    config: NodeId,
    manifest_label: String,
    target: String,
}

struct PendingComposeVolumeTarget {
    volume: NodeId,
    manifest_label: String,
    target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct KubernetesConfigKey {
    namespace: String,
    config_kind: String,
    name: String,
}

struct PendingKubernetesConfigRef {
    config_ref: NodeId,
    namespace: String,
    config_kind: String,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct KubernetesServiceKey {
    namespace: String,
    name: String,
}

struct PendingKubernetesServiceRef {
    service_ref: NodeId,
    namespace: String,
    name: String,
}

struct PendingGithubActionsLocalAction {
    action: NodeId,
    target: String,
}

struct PendingDocumentPathRef {
    source: NodeId,
    target: String,
    candidates: Vec<String>,
    relation: &'static str,
    line: u32,
    text: Option<String>,
}

struct PendingDocumentSymbolRef {
    source: NodeId,
    symbol: String,
    relation: &'static str,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MakefileTarget {
    name: String,
    command: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockerfileEntrypoint {
    instruction: String,
    command: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposeService {
    name: String,
    command: Option<String>,
    command_kind: Option<String>,
    build_context: Option<String>,
    dockerfile: Option<String>,
    depends_on: Vec<String>,
    environment: Vec<ComposeEnvironment>,
    env_files: Vec<ComposeEnvFile>,
    ports: Vec<ComposePort>,
    volumes: Vec<ComposeVolume>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposeEnvironment {
    name: String,
    value_present: bool,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposeEnvFile {
    path: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposePort {
    published: Option<String>,
    target: Option<String>,
    protocol: String,
    host_ip: Option<String>,
    raw: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComposeVolume {
    source: Option<String>,
    target: Option<String>,
    kind: String,
    read_only: bool,
    raw: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlutterAsset {
    path: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DartPlatformChannel {
    name: String,
    channel_kind: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubActionsWorkflow {
    name: String,
    environment: Vec<CiEnvironment>,
    jobs: Vec<GithubActionsJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubActionsJob {
    id: String,
    display_name: Option<String>,
    runs_on: Option<String>,
    needs: Vec<String>,
    environment: Vec<CiEnvironment>,
    steps: Vec<GithubActionsStep>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GithubActionsStep {
    name: Option<String>,
    uses: Option<String>,
    run: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitlabCiJob {
    name: String,
    stage: Option<String>,
    image: Option<String>,
    extends: Vec<String>,
    needs: Vec<String>,
    dependencies: Vec<String>,
    variables: Vec<CiEnvironment>,
    scripts: Vec<GitlabCiScript>,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitlabCiScript {
    command: String,
    script_kind: String,
    ordinal: usize,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CiEnvironment {
    name: String,
    value_present: bool,
    value_kind: String,
    scope: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KubernetesDocument {
    kind: String,
    name: String,
    namespace: String,
    line: u32,
    labels: BTreeMap<String, String>,
    pod_labels: BTreeMap<String, String>,
    selector_labels: BTreeMap<String, String>,
    config_refs: Vec<KubernetesConfigRef>,
    service_ports: Vec<KubernetesServicePort>,
    ingress_backends: Vec<KubernetesIngressBackend>,
    container_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KubernetesConfigRef {
    config_kind: String,
    ref_kind: String,
    name: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KubernetesServicePort {
    name: Option<String>,
    port: Option<String>,
    target_port: Option<String>,
    protocol: String,
    line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KubernetesIngressBackend {
    service_name: String,
    service_port: Option<String>,
    host: Option<String>,
    path: Option<String>,
    path_type: Option<String>,
    line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KubernetesLabelTarget {
    Metadata,
    PodTemplate,
    Selector,
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
    version_kind: Option<String>,
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
struct DartPackageRoot {
    name: String,
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
    let dart_packages = dart_package_roots(root, options, &ignored_globs);
    let c_include_dirs = c_include_dirs(root, options, &ignored_globs);
    let custom_rules = custom_rules(root);
    let annotations = graph_annotations(root);
    let mut context = IndexContext {
        graph: CodeGraph::new(root_label),
        function_symbols: BTreeMap::new(),
        file_nodes: BTreeMap::new(),
        directory_nodes: BTreeMap::new(),
        external_dependencies: BTreeMap::new(),
        cargo_workspace_dependencies,
        go_modules,
        dart_packages,
        c_include_dirs,
        custom_rules,
        annotations,
        pending_calls: Vec::new(),
        pending_local_imports: Vec::new(),
        pending_entrypoint_targets: Vec::new(),
        pending_compose_config_targets: Vec::new(),
        pending_compose_volume_targets: Vec::new(),
        kubernetes_configs: BTreeMap::new(),
        kubernetes_services: BTreeMap::new(),
        pending_kubernetes_config_refs: Vec::new(),
        pending_kubernetes_service_refs: Vec::new(),
        pending_github_actions_local_actions: Vec::new(),
        pending_document_path_refs: Vec::new(),
        pending_document_symbol_refs: Vec::new(),
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

    resolve_pending_calls(&mut context);
    resolve_pending_local_imports(&mut context);
    resolve_pending_entrypoint_targets(&mut context);
    resolve_pending_compose_config_targets(&mut context);
    resolve_pending_compose_volume_targets(&mut context);
    resolve_pending_kubernetes_config_refs(&mut context);
    resolve_pending_kubernetes_service_refs(&mut context);
    resolve_pending_github_actions_local_actions(&mut context);
    resolve_pending_document_path_refs(&mut context);
    resolve_pending_document_symbol_refs(&mut context);
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
        } else if is_markdown_document(path) {
            *report.languages.entry("markdown".to_string()).or_default() += 1;
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
    } else if is_markdown_document(path) {
        metadata.insert("language".to_string(), "markdown".to_string());
        metadata.insert("item_kind".to_string(), "document".to_string());
        metadata.insert("document_kind".to_string(), document_kind(path, label));
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
    } else if is_markdown_document(path) {
        metadata.insert("language".to_string(), "markdown".to_string());
        metadata.insert("item_kind".to_string(), "document".to_string());
        metadata.insert("document_kind".to_string(), document_kind(path, label));
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
        index_framework_configs(context, file_id, label, language, source);
        if language == Some(Language::Dart) {
            index_dart_platform_channels(context, file_id, label, source);
        }
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

fn index_rationale_comments(
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

fn index_markdown_document(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_markdown_document(path) {
        return;
    }

    let doc_kind = document_kind(path, label);
    add_file_metadata(&mut context.graph, file_id, "item_kind", "document");
    add_file_metadata(&mut context.graph, file_id, "source", "markdown");
    add_file_metadata(
        &mut context.graph,
        file_id,
        "document_kind",
        doc_kind.clone(),
    );

    let mut current_section = None;
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        if let Some((level, heading)) = markdown_heading(line) {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "document_section".to_string());
            metadata.insert("source".to_string(), "markdown".to_string());
            metadata.insert("language".to_string(), "markdown".to_string());
            metadata.insert("document_kind".to_string(), doc_kind.clone());
            metadata.insert("heading".to_string(), heading.clone());
            metadata.insert("level".to_string(), level.to_string());
            metadata.insert("anchor".to_string(), markdown_anchor(&heading));
            metadata.insert("line".to_string(), line_number.to_string());
            let section_id = context.graph.add_node_with_metadata(
                NodeKind::Module,
                format!("{label}#{heading}"),
                Some(line_span(label, source, line_number)),
                metadata,
            );
            add_edge_once_with_metadata(
                &mut context.graph,
                file_id,
                section_id,
                EdgeKind::Contains,
                Confidence::Exact,
                BTreeMap::from([("relation".to_string(), "document_section".to_string())]),
            );
            current_section = Some(section_id);
        }

        let source_id = current_section.unwrap_or(file_id);
        for link in markdown_links(line) {
            if let Some(candidates) = markdown_path_candidates(label, &link.target) {
                context
                    .pending_document_path_refs
                    .push(PendingDocumentPathRef {
                        source: source_id,
                        target: link.target,
                        candidates,
                        relation: "markdown_link",
                        line: line_number,
                        text: Some(link.text),
                    });
            }
        }

        for code in inline_code_spans(line) {
            if let Some(candidates) = markdown_path_candidates(label, &code) {
                context
                    .pending_document_path_refs
                    .push(PendingDocumentPathRef {
                        source: source_id,
                        target: code,
                        candidates,
                        relation: "markdown_code_path",
                        line: line_number,
                        text: None,
                    });
            } else if is_document_symbol_reference(&code) {
                context
                    .pending_document_symbol_refs
                    .push(PendingDocumentSymbolRef {
                        source: source_id,
                        symbol: code,
                        relation: "markdown_symbol_reference",
                        line: line_number,
                    });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownLink {
    text: String,
    target: String,
}

fn markdown_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = trimmed[level..].trim_start();
    if rest.is_empty() {
        return None;
    }
    let heading = rest.trim_end_matches('#').trim();
    (!heading.is_empty()).then(|| (level as u8, heading.to_string()))
}

fn markdown_anchor(heading: &str) -> String {
    let mut anchor = String::new();
    let mut previous_dash = false;
    for character in heading.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            anchor.push(character);
            previous_dash = false;
        } else if !previous_dash && !anchor.is_empty() {
            anchor.push('-');
            previous_dash = true;
        }
    }
    anchor.trim_matches('-').to_string()
}

fn markdown_links(line: &str) -> Vec<MarkdownLink> {
    let bytes = line.as_bytes();
    let mut links = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let Some(open_offset) = line[index..].find('[') else {
            break;
        };
        let open = index + open_offset;
        if open > 0 && bytes[open - 1] == b'!' {
            index = open + 1;
            continue;
        }
        let Some(close_offset) = line[open + 1..].find(']') else {
            break;
        };
        let close = open + 1 + close_offset;
        if line[close + 1..].starts_with('(') {
            let target_start = close + 2;
            if let Some(target_offset) = line[target_start..].find(')') {
                let target_end = target_start + target_offset;
                let target = markdown_link_target(&line[target_start..target_end]);
                if !target.is_empty() {
                    links.push(MarkdownLink {
                        text: line[open + 1..close].trim().to_string(),
                        target,
                    });
                }
                index = target_end + 1;
                continue;
            }
        }
        index = close + 1;
    }
    links
}

fn markdown_link_target(raw: &str) -> String {
    raw.trim()
        .trim_matches('<')
        .trim_matches('>')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn inline_code_spans(line: &str) -> Vec<String> {
    if line.contains("```") {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let value = after_start[..end].trim();
        if !value.is_empty() {
            spans.push(value.to_string());
        }
        rest = &after_start[end + 1..];
    }
    spans
}

fn markdown_path_candidates(document_label: &str, raw_target: &str) -> Option<Vec<String>> {
    let target = clean_markdown_path_target(raw_target)?;
    if !is_document_path_reference(&target) {
        return None;
    }

    let mut candidates = Vec::new();
    let relative = join_path(path_dir(document_label).as_deref(), &target);
    if !relative.is_empty() {
        candidates.push(relative);
    }
    let root_relative = normalize_path(&target);
    if !root_relative.is_empty() && !candidates.contains(&root_relative) {
        candidates.push(root_relative);
    }
    (!candidates.is_empty()).then_some(candidates)
}

fn clean_markdown_path_target(raw_target: &str) -> Option<String> {
    let trimmed = raw_target.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with("git@")
    {
        return None;
    }
    let without_fragment = trimmed
        .split_once('#')
        .map(|(path, _)| path)
        .unwrap_or(trimmed);
    let without_query = without_fragment
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(without_fragment)
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    (!without_query.is_empty()).then(|| without_query.to_string())
}

fn is_document_path_reference(target: &str) -> bool {
    target.contains('/')
        || target.starts_with("./")
        || target.starts_with("../")
        || target.rsplit('/').next().is_some_and(|name| {
            name.rsplit_once('.').is_some_and(|(_, extension)| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "rs" | "py"
                        | "js"
                        | "jsx"
                        | "ts"
                        | "tsx"
                        | "go"
                        | "c"
                        | "h"
                        | "cc"
                        | "cpp"
                        | "hpp"
                        | "dart"
                        | "php"
                        | "sh"
                        | "bash"
                        | "md"
                        | "markdown"
                        | "toml"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "lock"
                        | "txt"
                        | "cfg"
                        | "ini"
                        | "env"
                        | "sql"
                )
            })
        })
}

fn is_document_symbol_reference(value: &str) -> bool {
    let value = value.trim().trim_end_matches("()").trim_end_matches('!');
    (3..=96).contains(&value.len())
        && !value.contains(char::is_whitespace)
        && !value.contains('/')
        && !value.starts_with('-')
        && !value.starts_with('$')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_:.#<>".contains(character))
        && value.chars().any(|character| {
            character == '_'
                || character == ':'
                || character == '.'
                || character.is_ascii_lowercase()
        })
}

fn is_markdown_document(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkdn"
            )
        })
}

fn document_kind(path: &Path, label: &str) -> String {
    let normalized = label.to_ascii_lowercase();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(label)
        .to_ascii_lowercase();
    if normalized.contains("/adr/")
        || normalized.contains("/adrs/")
        || file_name.starts_with("adr-")
        || file_name.starts_with("adr_")
    {
        "adr".to_string()
    } else if normalized.contains("/rfc/")
        || normalized.contains("/rfcs/")
        || file_name.starts_with("rfc-")
        || file_name.starts_with("rfc_")
    {
        "rfc".to_string()
    } else {
        "markdown".to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RationaleComment {
    kind: &'static str,
    label: String,
    text: String,
    line: u32,
}

fn rationale_comments(
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

fn rationale_comment_text(language: Option<Language>, line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    match language {
        Some(Language::Python | Language::Bash) => trimmed.strip_prefix('#'),
        Some(
            Language::Rust
            | Language::JavaScript
            | Language::TypeScript
            | Language::Tsx
            | Language::Go
            | Language::C
            | Language::Cpp
            | Language::Dart,
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

fn rationale_marker(text: &str) -> Option<(&'static str, &str)> {
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

fn rationale_kind(marker: &str) -> &'static str {
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

fn normalize_rationale_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn rationale_label(text: &str) -> String {
    let mut label = text.chars().take(96).collect::<String>();
    if text.chars().count() > 96 {
        label.push_str("...");
    }
    label
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
    index_pubspec_assets(context, file_id, path, label, source);
    index_makefile_entrypoints(context, file_id, path, label, source);
    index_dockerfile_entrypoints(context, file_id, path, label, source);
    index_compose_entrypoints(context, file_id, path, label, source);
    index_github_actions_workflow_entrypoints(context, file_id, path, label, source);
    index_gitlab_ci_entrypoints(context, file_id, path, label, source);
    index_kubernetes_manifest_facts(context, file_id, path, label, source);
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
        Language::C | Language::Cpp | Language::Dart | Language::Bash => Vec::new(),
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

        let local_import = local_import_target(language, label, &require_call, &[], &[]);
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

fn index_dart_platform_channels(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
) {
    for channel in dart_platform_channels(source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "platform_channel".to_string());
        metadata.insert("source".to_string(), "dart".to_string());
        metadata.insert("language".to_string(), "dart".to_string());
        metadata.insert("framework".to_string(), "flutter".to_string());
        metadata.insert("channel_kind".to_string(), channel.channel_kind.clone());
        metadata.insert("channel_name".to_string(), channel.name.clone());
        metadata.insert("line".to_string(), channel.line.to_string());
        let channel_id = context.graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            format!("flutter {} channel:{}", channel.channel_kind, channel.name),
            Some(line_span(label, source, channel.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "dart".to_string());
        edge_metadata.insert("relation".to_string(), "platform_channel".to_string());
        edge_metadata.insert("channel_kind".to_string(), channel.channel_kind);
        add_edge_once_with_metadata(
            &mut context.graph,
            file_id,
            channel_id,
            EdgeKind::References,
            Confidence::Syntactic,
            edge_metadata,
        );
    }
}

fn dart_platform_channels(source: &str) -> Vec<DartPlatformChannel> {
    let mut channels = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        for (constructor, channel_kind) in [
            ("MethodChannel(", "method"),
            ("EventChannel(", "event"),
            ("BasicMessageChannel(", "basic_message"),
        ] {
            if let Some(name) = first_quoted_value_after(line, constructor) {
                channels.push(DartPlatformChannel {
                    name,
                    channel_kind: channel_kind.to_string(),
                    line: line_number,
                });
            }
        }
    }
    channels
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
        Some(Language::Dart) => configs.extend(dart_framework_configs(source)),
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
            if let Some(version_kind) = dependency.version_kind {
                edge_metadata.insert("dependency_version_kind".to_string(), version_kind);
            }
        }
        add_manifest_dependency_edge_once(
            &mut context.graph,
            file_id,
            dependency_id,
            edge_metadata,
        );
    }
}

fn add_manifest_dependency_edge_once(
    graph: &mut CodeGraph,
    file_id: NodeId,
    dependency_id: NodeId,
    metadata: BTreeMap<String, String>,
) {
    if graph.edges.iter().any(|edge| {
        edge.source == file_id
            && edge.target == dependency_id
            && edge.kind == EdgeKind::DependsOn
            && edge.metadata.get("dependency_kind") == metadata.get("dependency_kind")
            && edge.metadata.get("dependency_version") == metadata.get("dependency_version")
    }) {
        return;
    }
    graph.add_edge_with_metadata(
        file_id,
        dependency_id,
        EdgeKind::DependsOn,
        Confidence::Exact,
        metadata,
    );
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

fn index_pubspec_assets(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if path.file_name().and_then(|name| name.to_str()) != Some("pubspec.yaml") {
        return;
    }

    for asset in pubspec_flutter_assets(source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "flutter_asset".to_string());
        metadata.insert("source".to_string(), "pubspec".to_string());
        metadata.insert("framework".to_string(), "flutter".to_string());
        metadata.insert("config_kind".to_string(), "flutter_asset".to_string());
        metadata.insert("asset_path".to_string(), asset.path.clone());
        metadata.insert("target".to_string(), label.to_string());
        metadata.insert("line".to_string(), asset.line.to_string());
        let asset_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            format!("flutter asset:{}", asset.path),
            Some(line_span(label, source, asset.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "pubspec".to_string());
        edge_metadata.insert("framework".to_string(), "flutter".to_string());
        edge_metadata.insert("config_kind".to_string(), "flutter_asset".to_string());
        add_edge_once_with_metadata(
            &mut context.graph,
            file_id,
            asset_id,
            EdgeKind::Contains,
            Confidence::Exact,
            edge_metadata,
        );
    }
}

fn index_makefile_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_makefile_path(path) {
        return;
    }

    for target in makefile_targets(source) {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "makefile_target".to_string());
        metadata.insert("entrypoint_kind".to_string(), "make_target".to_string());
        metadata.insert("ecosystem".to_string(), "make".to_string());
        metadata.insert("source".to_string(), "makefile".to_string());
        metadata.insert("target".to_string(), target.name.clone());
        metadata.insert("line".to_string(), target.line.to_string());
        if let Some(command) = target.command.as_deref() {
            metadata.insert("command".to_string(), command.to_string());
            if let Some(command_path) = normalized_command_path_candidate(label, command) {
                metadata.insert("command_path".to_string(), command_path);
            }
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("make target:{}", target.name),
            Some(line_span(label, source, target.line)),
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
            "makefile_target",
            Confidence::Exact,
            None,
        );

        if let Some(command) = target.command {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target: command,
                    ecosystem: "make".to_string(),
                    entrypoint_kind: "make_target".to_string(),
                });
        }
    }
}

fn index_dockerfile_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_dockerfile_path(path) {
        return;
    }

    for entrypoint in dockerfile_entrypoints(source) {
        let instruction = entrypoint.instruction.to_ascii_lowercase();
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "dockerfile_entrypoint".to_string());
        metadata.insert("entrypoint_kind".to_string(), instruction.clone());
        metadata.insert("ecosystem".to_string(), "docker".to_string());
        metadata.insert("source".to_string(), "dockerfile".to_string());
        metadata.insert("instruction".to_string(), entrypoint.instruction.clone());
        metadata.insert("command".to_string(), entrypoint.command.clone());
        metadata.insert("line".to_string(), entrypoint.line.to_string());
        if let Some(command_path) = normalized_command_path_candidate(label, &entrypoint.command) {
            metadata.insert("command_path".to_string(), command_path);
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("docker {instruction}:{}", entrypoint.command),
            Some(line_span(label, source, entrypoint.line)),
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
            "dockerfile_instruction",
            Confidence::Exact,
            None,
        );
        context
            .pending_entrypoint_targets
            .push(PendingEntrypointTarget {
                entrypoint: entrypoint_id,
                manifest_label: label.to_string(),
                target: entrypoint.command,
                ecosystem: "docker".to_string(),
                entrypoint_kind: instruction,
            });
    }
}

fn index_compose_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_compose_file_path(path) {
        return;
    }
    let services = compose_services(source);
    if services.is_empty() {
        return;
    }

    let mut service_nodes = BTreeMap::new();
    for service in &services {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "compose_service".to_string());
        metadata.insert("entrypoint_kind".to_string(), "service".to_string());
        metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
        metadata.insert("source".to_string(), "compose".to_string());
        metadata.insert("service".to_string(), service.name.clone());
        metadata.insert("line".to_string(), service.line.to_string());
        if let Some(command) = service.command.as_deref() {
            metadata.insert("command".to_string(), command.to_string());
            if let Some(kind) = service.command_kind.as_deref() {
                metadata.insert("command_kind".to_string(), kind.to_string());
            }
            if let Some(command_path) = normalized_command_path_candidate(label, command) {
                metadata.insert("command_path".to_string(), command_path);
            }
        }
        if let Some(context_path) = service.build_context.as_deref() {
            metadata.insert("build_context".to_string(), context_path.to_string());
        }
        if let Some(dockerfile) = compose_service_dockerfile_path(label, service) {
            metadata.insert("dockerfile".to_string(), dockerfile);
        }
        if !service.environment.is_empty() {
            metadata.insert(
                "environment_count".to_string(),
                service.environment.len().to_string(),
            );
        }
        if !service.env_files.is_empty() {
            metadata.insert(
                "env_file_count".to_string(),
                service.env_files.len().to_string(),
            );
        }
        if !service.ports.is_empty() {
            metadata.insert("port_count".to_string(), service.ports.len().to_string());
        }
        if !service.volumes.is_empty() {
            metadata.insert(
                "volume_count".to_string(),
                service.volumes.len().to_string(),
            );
        }

        let entrypoint_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("compose service:{}", service.name),
            Some(line_span(label, source, service.line)),
            metadata,
        );
        service_nodes.insert(service.name.clone(), entrypoint_id);
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
            "compose_service",
            Confidence::Exact,
            None,
        );
        if let Some(command) = service.command.clone() {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target: command,
                    ecosystem: "compose".to_string(),
                    entrypoint_kind: "service".to_string(),
                });
        }
        if let Some(dockerfile) = compose_service_dockerfile_target(service) {
            context
                .pending_entrypoint_targets
                .push(PendingEntrypointTarget {
                    entrypoint: entrypoint_id,
                    manifest_label: label.to_string(),
                    target: dockerfile,
                    ecosystem: "compose-dockerfile".to_string(),
                    entrypoint_kind: "service".to_string(),
                });
        }
    }

    for service in &services {
        let Some(service_id) = service_nodes.get(&service.name).copied() else {
            continue;
        };
        for environment in &service.environment {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_environment".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), environment.line.to_string());
            metadata.insert(
                "value_present".to_string(),
                environment.value_present.to_string(),
            );
            if !environment.value_present {
                metadata.insert("value_source".to_string(), "host".to_string());
            }
            let environment_id = context.graph.add_node_with_metadata(
                NodeKind::Environment,
                environment.name.clone(),
                Some(line_span(label, source, environment.line)),
                metadata,
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_environment".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                &mut context.graph,
                service_id,
                environment_id,
                EdgeKind::ReadsEnvironment,
                Confidence::Exact,
                edge_metadata,
            );
        }

        for env_file in &service.env_files {
            let Some(normalized_env_file) = normalize_manifest_relative_path(label, &env_file.path)
            else {
                continue;
            };
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_env_file".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), env_file.line.to_string());
            metadata.insert("env_file".to_string(), env_file.path.clone());
            metadata.insert("env_file_path".to_string(), normalized_env_file.clone());
            let config_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                format!("compose env file:{normalized_env_file}"),
                Some(line_span(label, source, env_file.line)),
                metadata,
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_env_file".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                &mut context.graph,
                service_id,
                config_id,
                EdgeKind::ReadsConfig,
                Confidence::Exact,
                edge_metadata,
            );
            context
                .pending_compose_config_targets
                .push(PendingComposeConfigTarget {
                    config: config_id,
                    manifest_label: label.to_string(),
                    target: env_file.path.clone(),
                });
        }

        for port in &service.ports {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_port".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), port.line.to_string());
            metadata.insert("protocol".to_string(), port.protocol.clone());
            if let Some(raw) = port.raw.as_deref() {
                metadata.insert("raw".to_string(), raw.to_string());
            }
            if let Some(published) = port.published.as_deref() {
                metadata.insert("published_port".to_string(), published.to_string());
            }
            if let Some(target) = port.target.as_deref() {
                metadata.insert("target_port".to_string(), target.to_string());
            }
            if let Some(host_ip) = port.host_ip.as_deref() {
                metadata.insert("host_ip".to_string(), host_ip.to_string());
            }
            let port_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                compose_port_label(port),
                Some(line_span(label, source, port.line)),
                metadata,
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_port".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                &mut context.graph,
                service_id,
                port_id,
                EdgeKind::References,
                Confidence::Exact,
                edge_metadata,
            );
        }

        for volume in &service.volumes {
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "compose_volume".to_string());
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("ecosystem".to_string(), "docker-compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("line".to_string(), volume.line.to_string());
            metadata.insert("volume_kind".to_string(), volume.kind.clone());
            metadata.insert("read_only".to_string(), volume.read_only.to_string());
            if let Some(raw) = volume.raw.as_deref() {
                metadata.insert("raw".to_string(), raw.to_string());
            }
            if let Some(source) = volume.source.as_deref() {
                metadata.insert("source_path".to_string(), source.to_string());
                if let Some(local_source_path) = compose_volume_local_source_path(label, source) {
                    metadata.insert("local_source_path".to_string(), local_source_path);
                }
            }
            if let Some(target) = volume.target.as_deref() {
                metadata.insert("target_path".to_string(), target.to_string());
            }
            let volume_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                compose_volume_label(volume),
                Some(line_span(label, source, volume.line)),
                metadata.clone(),
            );
            let mut edge_metadata = BTreeMap::new();
            edge_metadata.insert("source".to_string(), "compose".to_string());
            edge_metadata.insert("relation".to_string(), "compose_volume".to_string());
            edge_metadata.insert("service".to_string(), service.name.clone());
            add_edge_once_with_metadata(
                &mut context.graph,
                service_id,
                volume_id,
                EdgeKind::References,
                Confidence::Exact,
                edge_metadata,
            );
            if let Some(source) = volume.source.as_deref()
                && compose_volume_local_source_path(label, source).is_some()
            {
                context
                    .pending_compose_volume_targets
                    .push(PendingComposeVolumeTarget {
                        volume: volume_id,
                        manifest_label: label.to_string(),
                        target: source.to_string(),
                    });
            }
        }
    }

    for service in &services {
        let Some(source_id) = service_nodes.get(&service.name).copied() else {
            continue;
        };
        for dependency in &service.depends_on {
            let Some(target_id) = service_nodes.get(dependency).copied() else {
                continue;
            };
            let mut metadata = BTreeMap::new();
            metadata.insert(
                "relation".to_string(),
                "compose_service_depends_on".to_string(),
            );
            metadata.insert("source".to_string(), "compose".to_string());
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("dependency".to_string(), dependency.clone());
            add_edge_once_with_metadata(
                &mut context.graph,
                source_id,
                target_id,
                EdgeKind::DependsOn,
                Confidence::Exact,
                metadata,
            );
        }
    }
}

fn index_github_actions_workflow_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_github_actions_workflow_path(label, path) {
        return;
    }
    let workflow = github_actions_workflow(label, source);
    if workflow.jobs.is_empty() {
        return;
    }

    let mut job_nodes = BTreeMap::new();
    for job in &workflow.jobs {
        let uses_count = job.steps.iter().filter(|step| step.uses.is_some()).count();
        let run_count = job.steps.iter().filter(|step| step.run.is_some()).count();
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "github_actions_job".to_string());
        metadata.insert("entrypoint_kind".to_string(), "workflow_job".to_string());
        metadata.insert("ecosystem".to_string(), "github-actions".to_string());
        metadata.insert("source".to_string(), "github-actions".to_string());
        metadata.insert("workflow".to_string(), workflow.name.clone());
        metadata.insert("job".to_string(), job.id.clone());
        metadata.insert("line".to_string(), job.line.to_string());
        metadata.insert("step_count".to_string(), job.steps.len().to_string());
        metadata.insert("uses_count".to_string(), uses_count.to_string());
        metadata.insert("run_count".to_string(), run_count.to_string());
        metadata.insert("needs_count".to_string(), job.needs.len().to_string());
        let environment_count = workflow.environment.len() + job.environment.len();
        metadata.insert(
            "environment_count".to_string(),
            environment_count.to_string(),
        );
        if !job.needs.is_empty() {
            metadata.insert("needs".to_string(), job.needs.join(","));
        }
        if let Some(display_name) = job.display_name.as_deref() {
            metadata.insert("name".to_string(), display_name.to_string());
        }
        if let Some(runs_on) = job.runs_on.as_deref() {
            metadata.insert("runs_on".to_string(), runs_on.to_string());
        }

        let job_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("github workflow:{}/{}", workflow.name, job.id),
            Some(line_span(label, source, job.line)),
            metadata,
        );
        job_nodes.insert(job.id.clone(), job_id);
        add_edge_once(
            &mut context.graph,
            file_id,
            job_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            &mut context.graph,
            root_id,
            job_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        add_entrypoint_reference(
            &mut context.graph,
            job_id,
            file_id,
            "entrypoint_file",
            "github_actions_workflow",
            Confidence::Exact,
            None,
        );

        for step in &job.steps {
            index_github_actions_step(context, job_id, label, source, &workflow, job, step);
        }
        for environment in workflow.environment.iter().chain(job.environment.iter()) {
            index_ci_environment(
                context,
                job_id,
                label,
                source,
                "github-actions",
                &job.id,
                environment,
            );
        }
    }

    for job in &workflow.jobs {
        let Some(source_id) = job_nodes.get(&job.id).copied() else {
            continue;
        };
        for dependency in &job.needs {
            let Some(target_id) = job_nodes.get(dependency).copied() else {
                continue;
            };
            let mut metadata = BTreeMap::new();
            metadata.insert("relation".to_string(), "github_actions_needs".to_string());
            metadata.insert("source".to_string(), "github-actions".to_string());
            metadata.insert("workflow".to_string(), workflow.name.clone());
            metadata.insert("job".to_string(), job.id.clone());
            metadata.insert("dependency".to_string(), dependency.clone());
            add_edge_once_with_metadata(
                &mut context.graph,
                source_id,
                target_id,
                EdgeKind::DependsOn,
                Confidence::Exact,
                metadata,
            );
        }
    }
}

fn index_github_actions_step(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    workflow: &GithubActionsWorkflow,
    job: &GithubActionsJob,
    step: &GithubActionsStep,
) {
    if let Some(action) = step.uses.as_deref() {
        index_github_actions_uses_step(context, job_id, label, source, workflow, job, step, action);
    }
    if let Some(command) = step.run.as_deref() {
        index_github_actions_run_step(context, job_id, label, source, workflow, job, step, command);
    }
}

fn index_github_actions_uses_step(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    workflow: &GithubActionsWorkflow,
    job: &GithubActionsJob,
    step: &GithubActionsStep,
    action: &str,
) {
    if let Some(local_path) = github_actions_local_action_path(action) {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "item_kind".to_string(),
            "github_actions_local_action".to_string(),
        );
        metadata.insert("source".to_string(), "github-actions".to_string());
        metadata.insert("ecosystem".to_string(), "github-actions".to_string());
        metadata.insert("workflow".to_string(), workflow.name.clone());
        metadata.insert("job".to_string(), job.id.clone());
        metadata.insert("line".to_string(), step.line.to_string());
        metadata.insert("uses".to_string(), action.to_string());
        metadata.insert("local_action_path".to_string(), local_path.clone());
        if let Some(name) = step.name.as_deref() {
            metadata.insert("name".to_string(), name.to_string());
        }
        let action_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            format!("github action:{local_path}"),
            Some(line_span(label, source, step.line)),
            metadata,
        );
        add_github_actions_uses_edge(&mut context.graph, job_id, action_id, workflow, job, action);
        context
            .pending_github_actions_local_actions
            .push(PendingGithubActionsLocalAction {
                action: action_id,
                target: local_path,
            });
        return;
    }

    let (name, version) = github_actions_remote_action(action);
    let action_id = github_actions_external_action_node(context, &name, version.as_deref());
    add_github_actions_uses_edge(&mut context.graph, job_id, action_id, workflow, job, action);
}

fn index_github_actions_run_step(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    workflow: &GithubActionsWorkflow,
    job: &GithubActionsJob,
    step: &GithubActionsStep,
    command: &str,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "item_kind".to_string(),
        "github_actions_run_step".to_string(),
    );
    metadata.insert("source".to_string(), "github-actions".to_string());
    metadata.insert("ecosystem".to_string(), "github-actions".to_string());
    metadata.insert("workflow".to_string(), workflow.name.clone());
    metadata.insert("job".to_string(), job.id.clone());
    metadata.insert("line".to_string(), step.line.to_string());
    metadata.insert("command".to_string(), command.to_string());
    if let Some(command_path) = root_relative_command_path_candidate(command) {
        metadata.insert("command_path".to_string(), command_path);
    }
    if let Some(name) = step.name.as_deref() {
        metadata.insert("name".to_string(), name.to_string());
    }
    let step_id = context.graph.add_node_with_metadata(
        NodeKind::Config,
        format!("github run:{}/{}/{}", workflow.name, job.id, step.line),
        Some(line_span(label, source, step.line)),
        metadata,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("source".to_string(), "github-actions".to_string());
    edge_metadata.insert("relation".to_string(), "github_actions_run".to_string());
    edge_metadata.insert("workflow".to_string(), workflow.name.clone());
    edge_metadata.insert("job".to_string(), job.id.clone());
    add_edge_once_with_metadata(
        &mut context.graph,
        job_id,
        step_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );
    context
        .pending_entrypoint_targets
        .push(PendingEntrypointTarget {
            entrypoint: job_id,
            manifest_label: label.to_string(),
            target: command.to_string(),
            ecosystem: "github-actions".to_string(),
            entrypoint_kind: "workflow_job".to_string(),
        });
}

fn add_github_actions_uses_edge(
    graph: &mut CodeGraph,
    job_id: NodeId,
    action_id: NodeId,
    workflow: &GithubActionsWorkflow,
    job: &GithubActionsJob,
    action: &str,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("source".to_string(), "github-actions".to_string());
    metadata.insert("relation".to_string(), "github_actions_uses".to_string());
    metadata.insert("workflow".to_string(), workflow.name.clone());
    metadata.insert("job".to_string(), job.id.clone());
    metadata.insert("uses".to_string(), action.to_string());
    if let Some((_, version)) = action.rsplit_once('@')
        && !version.trim().is_empty()
    {
        metadata.insert("version".to_string(), version.trim().to_string());
    }
    add_edge_once_with_metadata(
        graph,
        job_id,
        action_id,
        EdgeKind::DependsOn,
        Confidence::Exact,
        metadata,
    );
}

fn github_actions_external_action_node(
    context: &mut IndexContext,
    action: &str,
    version: Option<&str>,
) -> NodeId {
    let package_id = format!("github-actions:{action}");
    if let Some(id) = context.external_dependencies.get(&package_id).copied() {
        return id;
    }
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "dependency".to_string());
    metadata.insert("ecosystem".to_string(), "github-actions".to_string());
    metadata.insert("package_id".to_string(), package_id.clone());
    metadata.insert("source".to_string(), "github-actions".to_string());
    if let Some(version) = version {
        metadata.insert("version".to_string(), version.to_string());
    }
    let id = context.graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        format!("github action:{action}"),
        None,
        metadata,
    );
    context.external_dependencies.insert(package_id, id);
    id
}

fn index_gitlab_ci_entrypoints(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_gitlab_ci_path(label, path) {
        return;
    }
    let jobs = gitlab_ci_jobs(source);
    if jobs.is_empty() {
        return;
    }

    let mut job_nodes = BTreeMap::new();
    for job in &jobs {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "gitlab_ci_job".to_string());
        metadata.insert("entrypoint_kind".to_string(), "pipeline_job".to_string());
        metadata.insert("ecosystem".to_string(), "gitlab-ci".to_string());
        metadata.insert("source".to_string(), "gitlab-ci".to_string());
        metadata.insert("job".to_string(), job.name.clone());
        metadata.insert("line".to_string(), job.line.to_string());
        metadata.insert("script_count".to_string(), job.scripts.len().to_string());
        metadata.insert("needs_count".to_string(), job.needs.len().to_string());
        metadata.insert(
            "dependencies_count".to_string(),
            job.dependencies.len().to_string(),
        );
        metadata.insert(
            "environment_count".to_string(),
            job.variables.len().to_string(),
        );
        if !job.needs.is_empty() {
            metadata.insert("needs".to_string(), job.needs.join(","));
        }
        if !job.dependencies.is_empty() {
            metadata.insert("dependencies".to_string(), job.dependencies.join(","));
        }
        if let Some(stage) = job.stage.as_deref() {
            metadata.insert("stage".to_string(), stage.to_string());
        }
        if let Some(image) = job.image.as_deref() {
            metadata.insert("image".to_string(), image.to_string());
        }
        if !job.extends.is_empty() {
            metadata.insert("extends".to_string(), job.extends.join(","));
        }

        let job_id = context.graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("gitlab job:{}", job.name),
            Some(line_span(label, source, job.line)),
            metadata,
        );
        job_nodes.insert(job.name.clone(), job_id);
        add_edge_once(
            &mut context.graph,
            file_id,
            job_id,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        let root_id = context.graph.root;
        add_edge_once(
            &mut context.graph,
            root_id,
            job_id,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        add_entrypoint_reference(
            &mut context.graph,
            job_id,
            file_id,
            "entrypoint_file",
            "gitlab_ci_config",
            Confidence::Exact,
            None,
        );

        for script in &job.scripts {
            index_gitlab_ci_script(context, job_id, label, source, job, script);
        }
        for variable in &job.variables {
            index_ci_environment(
                context,
                job_id,
                label,
                source,
                "gitlab-ci",
                &job.name,
                variable,
            );
        }
    }

    for job in &jobs {
        let Some(source_id) = job_nodes.get(&job.name).copied() else {
            continue;
        };
        for dependency in &job.needs {
            add_gitlab_ci_job_dependency_edge(
                &mut context.graph,
                source_id,
                &job_nodes,
                job,
                dependency,
                "gitlab_ci_needs",
            );
        }
        for dependency in &job.dependencies {
            add_gitlab_ci_job_dependency_edge(
                &mut context.graph,
                source_id,
                &job_nodes,
                job,
                dependency,
                "gitlab_ci_dependencies",
            );
        }
    }
}

fn index_gitlab_ci_script(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    job: &GitlabCiJob,
    script: &GitlabCiScript,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "gitlab_ci_script".to_string());
    metadata.insert("source".to_string(), "gitlab-ci".to_string());
    metadata.insert("ecosystem".to_string(), "gitlab-ci".to_string());
    metadata.insert("job".to_string(), job.name.clone());
    metadata.insert("line".to_string(), script.line.to_string());
    metadata.insert("ordinal".to_string(), script.ordinal.to_string());
    metadata.insert("script_kind".to_string(), script.script_kind.clone());
    metadata.insert("command".to_string(), script.command.clone());
    if let Some(stage) = job.stage.as_deref() {
        metadata.insert("stage".to_string(), stage.to_string());
    }
    if let Some(command_path) = root_relative_command_path_candidate(&script.command) {
        metadata.insert("command_path".to_string(), command_path);
    }
    let script_id = context.graph.add_node_with_metadata(
        NodeKind::Config,
        format!(
            "gitlab script:{}/{}#{}",
            job.name, script.line, script.ordinal
        ),
        Some(line_span(label, source, script.line)),
        metadata,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("source".to_string(), "gitlab-ci".to_string());
    edge_metadata.insert("relation".to_string(), "gitlab_ci_script".to_string());
    edge_metadata.insert("job".to_string(), job.name.clone());
    edge_metadata.insert("script_kind".to_string(), script.script_kind.clone());
    add_edge_once_with_metadata(
        &mut context.graph,
        job_id,
        script_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );
    context
        .pending_entrypoint_targets
        .push(PendingEntrypointTarget {
            entrypoint: job_id,
            manifest_label: label.to_string(),
            target: script.command.clone(),
            ecosystem: "gitlab-ci".to_string(),
            entrypoint_kind: "pipeline_job".to_string(),
        });
}

fn index_ci_environment(
    context: &mut IndexContext,
    job_id: NodeId,
    label: &str,
    source: &str,
    ci_source: &str,
    job_name: &str,
    environment: &CiEnvironment,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "ci_environment".to_string());
    metadata.insert("source".to_string(), ci_source.to_string());
    metadata.insert("ecosystem".to_string(), ci_source.to_string());
    metadata.insert("job".to_string(), job_name.to_string());
    metadata.insert("scope".to_string(), environment.scope.clone());
    metadata.insert("line".to_string(), environment.line.to_string());
    metadata.insert(
        "value_present".to_string(),
        environment.value_present.to_string(),
    );
    metadata.insert("value_kind".to_string(), environment.value_kind.clone());
    if !environment.value_present {
        metadata.insert("value_source".to_string(), "runner".to_string());
    }
    let environment_id = context.graph.add_node_with_metadata(
        NodeKind::Environment,
        environment.name.clone(),
        Some(line_span(label, source, environment.line)),
        metadata,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("source".to_string(), ci_source.to_string());
    edge_metadata.insert("relation".to_string(), "ci_environment".to_string());
    edge_metadata.insert("job".to_string(), job_name.to_string());
    edge_metadata.insert("scope".to_string(), environment.scope.clone());
    add_edge_once_with_metadata(
        &mut context.graph,
        job_id,
        environment_id,
        EdgeKind::ReadsEnvironment,
        Confidence::Exact,
        edge_metadata,
    );
}

fn add_gitlab_ci_job_dependency_edge(
    graph: &mut CodeGraph,
    source_id: NodeId,
    job_nodes: &BTreeMap<String, NodeId>,
    job: &GitlabCiJob,
    dependency: &str,
    relation: &str,
) {
    let Some(target_id) = job_nodes.get(dependency).copied() else {
        return;
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("relation".to_string(), relation.to_string());
    metadata.insert("source".to_string(), "gitlab-ci".to_string());
    metadata.insert("job".to_string(), job.name.clone());
    metadata.insert("dependency".to_string(), dependency.to_string());
    add_edge_once_with_metadata(
        graph,
        source_id,
        target_id,
        EdgeKind::DependsOn,
        Confidence::Exact,
        metadata,
    );
}

fn index_kubernetes_manifest_facts(
    context: &mut IndexContext,
    file_id: NodeId,
    path: &Path,
    label: &str,
    source: &str,
) {
    if !is_kubernetes_manifest_candidate(path, source) {
        return;
    }
    let documents = kubernetes_documents(source);
    if documents.is_empty() {
        return;
    }

    let mut service_nodes = Vec::new();
    let mut workload_nodes = Vec::new();
    for document in &documents {
        if let Some(config_kind) = kubernetes_config_kind(&document.kind) {
            let key = KubernetesConfigKey {
                namespace: document.namespace.clone(),
                config_kind: config_kind.to_string(),
                name: document.name.clone(),
            };
            let mut metadata = BTreeMap::new();
            metadata.insert("item_kind".to_string(), "kubernetes_config".to_string());
            metadata.insert("source".to_string(), "kubernetes".to_string());
            metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
            metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
            metadata.insert("config_kind".to_string(), config_kind.to_string());
            metadata.insert("name".to_string(), document.name.clone());
            metadata.insert("namespace".to_string(), document.namespace.clone());
            metadata.insert("line".to_string(), document.line.to_string());
            let config_id = context.graph.add_node_with_metadata(
                NodeKind::Config,
                kubernetes_resource_label(config_kind, &document.namespace, &document.name),
                Some(line_span(label, source, document.line)),
                metadata,
            );
            context.kubernetes_configs.insert(key, config_id);
            add_edge_once(
                &mut context.graph,
                file_id,
                config_id,
                EdgeKind::Contains,
                Confidence::Exact,
            );
            continue;
        }

        if document.kind == "Service" {
            let service_id = index_kubernetes_service(context, file_id, label, source, document);
            context.kubernetes_services.insert(
                KubernetesServiceKey {
                    namespace: document.namespace.clone(),
                    name: document.name.clone(),
                },
                service_id,
            );
            service_nodes.push((service_id, document));
            continue;
        }

        if document.kind == "Ingress" {
            index_kubernetes_ingress(context, file_id, label, source, document);
            continue;
        }

        if kubernetes_workload_kind(&document.kind) {
            let workload_id = index_kubernetes_workload(context, file_id, label, source, document);
            workload_nodes.push((workload_id, document));
        }
    }

    link_kubernetes_services_to_workloads(&mut context.graph, &service_nodes, &workload_nodes);
}

fn index_kubernetes_service(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    document: &KubernetesDocument,
) -> NodeId {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "kubernetes_service".to_string());
    metadata.insert("source".to_string(), "kubernetes".to_string());
    metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
    metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
    metadata.insert("name".to_string(), document.name.clone());
    metadata.insert("namespace".to_string(), document.namespace.clone());
    metadata.insert("line".to_string(), document.line.to_string());
    metadata.insert(
        "port_count".to_string(),
        document.service_ports.len().to_string(),
    );
    if !document.selector_labels.is_empty() {
        metadata.insert(
            "selector".to_string(),
            kubernetes_label_selector_string(&document.selector_labels),
        );
    }
    let service_id = context.graph.add_node_with_metadata(
        NodeKind::Config,
        kubernetes_resource_label("service", &document.namespace, &document.name),
        Some(line_span(label, source, document.line)),
        metadata,
    );
    add_edge_once(
        &mut context.graph,
        file_id,
        service_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );

    for port in &document.service_ports {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "item_kind".to_string(),
            "kubernetes_service_port".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
        metadata.insert("service".to_string(), document.name.clone());
        metadata.insert("namespace".to_string(), document.namespace.clone());
        metadata.insert("protocol".to_string(), port.protocol.clone());
        metadata.insert("line".to_string(), port.line.to_string());
        if let Some(name) = port.name.as_deref() {
            metadata.insert("name".to_string(), name.to_string());
        }
        if let Some(value) = port.port.as_deref() {
            metadata.insert("port".to_string(), value.to_string());
        }
        if let Some(value) = port.target_port.as_deref() {
            metadata.insert("target_port".to_string(), value.to_string());
        }
        let port_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            kubernetes_service_port_label(document, port),
            Some(line_span(label, source, port.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "kubernetes".to_string());
        edge_metadata.insert(
            "relation".to_string(),
            "kubernetes_service_port".to_string(),
        );
        add_edge_once_with_metadata(
            &mut context.graph,
            service_id,
            port_id,
            EdgeKind::References,
            Confidence::Exact,
            edge_metadata,
        );
    }
    service_id
}

fn index_kubernetes_ingress(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    document: &KubernetesDocument,
) -> NodeId {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "kubernetes_ingress".to_string());
    metadata.insert("entrypoint_kind".to_string(), "ingress".to_string());
    metadata.insert("source".to_string(), "kubernetes".to_string());
    metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
    metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
    metadata.insert("name".to_string(), document.name.clone());
    metadata.insert("namespace".to_string(), document.namespace.clone());
    metadata.insert("line".to_string(), document.line.to_string());
    metadata.insert(
        "backend_count".to_string(),
        document.ingress_backends.len().to_string(),
    );
    let ingress_id = context.graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        kubernetes_resource_label("ingress", &document.namespace, &document.name),
        Some(line_span(label, source, document.line)),
        metadata,
    );
    add_edge_once(
        &mut context.graph,
        file_id,
        ingress_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    let root_id = context.graph.root;
    add_edge_once(
        &mut context.graph,
        root_id,
        ingress_id,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("relation".to_string(), "entrypoint_file".to_string());
    edge_metadata.insert("resolution".to_string(), "kubernetes_manifest".to_string());
    edge_metadata.insert("source".to_string(), "kubernetes".to_string());
    add_edge_once_with_metadata(
        &mut context.graph,
        ingress_id,
        file_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );

    for backend in &document.ingress_backends {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "item_kind".to_string(),
            "kubernetes_service_ref".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
        metadata.insert("ref_kind".to_string(), "ingress_backend".to_string());
        metadata.insert("name".to_string(), backend.service_name.clone());
        metadata.insert("namespace".to_string(), document.namespace.clone());
        metadata.insert("ingress".to_string(), document.name.clone());
        metadata.insert("line".to_string(), backend.line.to_string());
        if let Some(port) = backend.service_port.as_deref() {
            metadata.insert("service_port".to_string(), port.to_string());
        }
        if let Some(host) = backend.host.as_deref() {
            metadata.insert("host".to_string(), host.to_string());
        }
        if let Some(path) = backend.path.as_deref() {
            metadata.insert("path".to_string(), path.to_string());
        }
        if let Some(path_type) = backend.path_type.as_deref() {
            metadata.insert("path_type".to_string(), path_type.to_string());
        }
        let service_ref_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            kubernetes_service_ref_label(&document.namespace, &backend.service_name),
            Some(line_span(label, source, backend.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "kubernetes".to_string());
        edge_metadata.insert(
            "relation".to_string(),
            "kubernetes_ingress_backend".to_string(),
        );
        if let Some(path) = backend.path.as_deref() {
            edge_metadata.insert("path".to_string(), path.to_string());
        }
        if let Some(host) = backend.host.as_deref() {
            edge_metadata.insert("host".to_string(), host.to_string());
        }
        add_edge_once_with_metadata(
            &mut context.graph,
            ingress_id,
            service_ref_id,
            EdgeKind::References,
            Confidence::Exact,
            edge_metadata,
        );
        context
            .pending_kubernetes_service_refs
            .push(PendingKubernetesServiceRef {
                service_ref: service_ref_id,
                namespace: document.namespace.clone(),
                name: backend.service_name.clone(),
            });
    }

    ingress_id
}

fn index_kubernetes_workload(
    context: &mut IndexContext,
    file_id: NodeId,
    label: &str,
    source: &str,
    document: &KubernetesDocument,
) -> NodeId {
    let kind_slug = document.kind.to_ascii_lowercase();
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "kubernetes_workload".to_string());
    metadata.insert("entrypoint_kind".to_string(), "workload".to_string());
    metadata.insert("source".to_string(), "kubernetes".to_string());
    metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
    metadata.insert("kubernetes_kind".to_string(), document.kind.clone());
    metadata.insert("name".to_string(), document.name.clone());
    metadata.insert("namespace".to_string(), document.namespace.clone());
    metadata.insert("line".to_string(), document.line.to_string());
    metadata.insert(
        "config_ref_count".to_string(),
        document.config_refs.len().to_string(),
    );
    metadata.insert(
        "container_count".to_string(),
        document.container_count.to_string(),
    );
    if !document.labels.is_empty() {
        metadata.insert(
            "labels".to_string(),
            kubernetes_label_selector_string(&document.labels),
        );
    }
    if !document.pod_labels.is_empty() {
        metadata.insert(
            "pod_labels".to_string(),
            kubernetes_label_selector_string(&document.pod_labels),
        );
    }
    let workload_id = context.graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        kubernetes_resource_label(&kind_slug, &document.namespace, &document.name),
        Some(line_span(label, source, document.line)),
        metadata,
    );
    add_edge_once(
        &mut context.graph,
        file_id,
        workload_id,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    let root_id = context.graph.root;
    add_edge_once(
        &mut context.graph,
        root_id,
        workload_id,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    let mut edge_metadata = BTreeMap::new();
    edge_metadata.insert("relation".to_string(), "entrypoint_file".to_string());
    edge_metadata.insert("resolution".to_string(), "kubernetes_manifest".to_string());
    edge_metadata.insert("source".to_string(), "kubernetes".to_string());
    add_edge_once_with_metadata(
        &mut context.graph,
        workload_id,
        file_id,
        EdgeKind::References,
        Confidence::Exact,
        edge_metadata,
    );

    for config_ref in &document.config_refs {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "kubernetes_config_ref".to_string());
        metadata.insert("source".to_string(), "kubernetes".to_string());
        metadata.insert("ecosystem".to_string(), "kubernetes".to_string());
        metadata.insert("config_kind".to_string(), config_ref.config_kind.clone());
        metadata.insert("ref_kind".to_string(), config_ref.ref_kind.clone());
        metadata.insert("name".to_string(), config_ref.name.clone());
        metadata.insert("namespace".to_string(), document.namespace.clone());
        metadata.insert("workload".to_string(), document.name.clone());
        metadata.insert("workload_kind".to_string(), document.kind.clone());
        metadata.insert("line".to_string(), config_ref.line.to_string());
        let config_ref_id = context.graph.add_node_with_metadata(
            NodeKind::Config,
            kubernetes_config_ref_label(
                &config_ref.config_kind,
                &document.namespace,
                &config_ref.name,
            ),
            Some(line_span(label, source, config_ref.line)),
            metadata,
        );
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "kubernetes".to_string());
        edge_metadata.insert("relation".to_string(), "kubernetes_config_ref".to_string());
        edge_metadata.insert("ref_kind".to_string(), config_ref.ref_kind.clone());
        add_edge_once_with_metadata(
            &mut context.graph,
            workload_id,
            config_ref_id,
            EdgeKind::ReadsConfig,
            Confidence::Exact,
            edge_metadata,
        );
        context
            .pending_kubernetes_config_refs
            .push(PendingKubernetesConfigRef {
                config_ref: config_ref_id,
                namespace: document.namespace.clone(),
                config_kind: config_ref.config_kind.clone(),
                name: config_ref.name.clone(),
            });
    }
    workload_id
}

fn link_kubernetes_services_to_workloads(
    graph: &mut CodeGraph,
    service_nodes: &[(NodeId, &KubernetesDocument)],
    workload_nodes: &[(NodeId, &KubernetesDocument)],
) {
    for (service_id, service) in service_nodes {
        if service.selector_labels.is_empty() {
            continue;
        }
        for (workload_id, workload) in workload_nodes {
            if service.namespace != workload.namespace {
                continue;
            }
            let workload_labels = kubernetes_workload_match_labels(workload);
            if !kubernetes_selector_matches(&service.selector_labels, workload_labels) {
                continue;
            }
            let mut metadata = BTreeMap::new();
            metadata.insert("source".to_string(), "kubernetes".to_string());
            metadata.insert(
                "relation".to_string(),
                "kubernetes_service_selector".to_string(),
            );
            metadata.insert(
                "selector".to_string(),
                kubernetes_label_selector_string(&service.selector_labels),
            );
            metadata.insert("service".to_string(), service.name.clone());
            metadata.insert("workload".to_string(), workload.name.clone());
            metadata.insert("namespace".to_string(), service.namespace.clone());
            add_edge_once_with_metadata(
                graph,
                *service_id,
                *workload_id,
                EdgeKind::References,
                Confidence::Exact,
                metadata,
            );
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
        Some("package-lock.json") => package_lock_dependencies(source),
        Some("pnpm-lock.yaml") => pnpm_lock_dependencies(source),
        Some("go.mod") => go_mod_dependencies(source),
        Some("requirements.txt") => requirements_dependencies(source),
        Some("pyproject.toml") => pyproject_dependencies(source),
        Some("setup.py") => setup_py_dependencies(source),
        Some("setup.cfg") => setup_cfg_dependencies(source),
        Some("Pipfile") => pipfile_dependencies(source),
        Some("composer.json") => composer_dependencies(source),
        Some("composer.lock") => composer_lock_dependencies(source),
        Some("vcpkg.json") => vcpkg_dependencies(source),
        Some("conanfile.txt") => conanfile_txt_dependencies(source),
        Some("CMakeLists.txt") => cmake_dependencies(source),
        Some("pubspec.yaml") => pubspec_dependencies(source),
        _ => Vec::new(),
    }
}

fn manifest_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml") => cargo_entrypoints(path, source),
        Some("package.json") => package_json_entrypoints(source),
        Some("go.mod") => go_mod_entrypoints(path, source),
        Some("pyproject.toml") => pyproject_entrypoints(source),
        Some("setup.py") => setup_py_entrypoints(source),
        Some("setup.cfg") => setup_cfg_entrypoints(source),
        Some("composer.json") => composer_entrypoints(source),
        Some("CMakeLists.txt") => cmake_entrypoints(source),
        Some("pubspec.yaml") => pubspec_entrypoints(path, source),
        _ => Vec::new(),
    }
}

fn is_makefile_path(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("Makefile" | "makefile" | "GNUmakefile")
    )
}

fn is_dockerfile_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "Dockerfile" || name.ends_with(".Dockerfile"))
}

fn is_compose_file_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "docker-compose.yml"
                    | "docker-compose.yaml"
                    | "compose.yml"
                    | "compose.yaml"
                    | "docker-compose.override.yml"
                    | "docker-compose.override.yaml"
            ) || name.ends_with(".compose.yml")
                || name.ends_with(".compose.yaml")
        })
}

fn is_kubernetes_manifest_candidate(path: &Path, source: &str) -> bool {
    let yaml_path = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "yaml" | "yml"));
    yaml_path && source.contains("apiVersion:") && source.contains("kind:")
}

fn kubernetes_documents(source: &str) -> Vec<KubernetesDocument> {
    let mut documents = Vec::new();
    let mut start_line = 1u32;
    let mut lines = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        if raw_line.trim() == "---" {
            if let Some(document) = kubernetes_document_from_lines(&lines, start_line) {
                documents.push(document);
            }
            lines.clear();
            start_line = index as u32 + 2;
            continue;
        }
        lines.push(raw_line.to_string());
    }

    if let Some(document) = kubernetes_document_from_lines(&lines, start_line) {
        documents.push(document);
    }
    documents
}

fn kubernetes_document_from_lines(lines: &[String], start_line: u32) -> Option<KubernetesDocument> {
    let mut has_api_version = false;
    let mut kind = None;
    let mut metadata_indent = None;
    let mut name = None;
    let mut namespace = None;
    let mut labels = BTreeMap::new();
    let mut pod_labels = BTreeMap::new();
    let mut selector_labels = BTreeMap::new();
    let mut config_refs = Vec::new();
    let mut service_ports = Vec::new();
    let mut ingress_backends = Vec::new();
    let mut active_ref: Option<(String, String, usize, u32)> = None;
    let mut active_ports_indent = None;
    let mut active_service_port: Option<(KubernetesServicePort, usize)> = None;
    let mut active_ingress_host: Option<String> = None;
    let mut active_ingress_path: Option<String> = None;
    let mut active_ingress_path_type: Option<String> = None;
    let mut active_ingress_backend: Option<(KubernetesIngressBackend, usize)> = None;
    let mut active_ingress_service_indent = None;
    let mut active_containers_indent = None;
    let mut active_template_indent = None;
    let mut active_template_metadata_indent = None;
    let mut active_selector_indent = None;
    let mut active_label_map: Option<(KubernetesLabelTarget, usize)> = None;
    let mut container_count = 0usize;

    for (index, raw_line) in lines.iter().enumerate() {
        let line = start_line + index as u32;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let item_trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let indent = yaml_indent(raw_line);

        if let Some((_target, label_indent)) = active_label_map
            && indent <= label_indent
        {
            active_label_map = None;
        } else if let Some((target, _)) = active_label_map
            && let Some((key, value)) = yaml_key_pair(trimmed)
        {
            match target {
                KubernetesLabelTarget::Metadata => {
                    labels.insert(key, value);
                }
                KubernetesLabelTarget::PodTemplate => {
                    pod_labels.insert(key, value);
                }
                KubernetesLabelTarget::Selector => {
                    selector_labels.insert(key, value);
                }
            }
            continue;
        }

        if let Some((_, _, ref_indent, _)) = active_ref.as_ref()
            && indent <= *ref_indent
        {
            active_ref = None;
        }
        if let Some((_, backend_indent)) = active_ingress_backend.as_ref()
            && indent <= *backend_indent
        {
            flush_kubernetes_ingress_backend(&mut ingress_backends, &mut active_ingress_backend);
            active_ingress_service_indent = None;
        }
        if let Some(service_indent) = active_ingress_service_indent
            && indent <= service_indent
        {
            active_ingress_service_indent = None;
        }
        if let Some((config_kind, ref_kind, _, ref_line)) = active_ref.as_ref()
            && let Some(value) = yaml_key_value(trimmed, "name")
        {
            config_refs.push(KubernetesConfigRef {
                config_kind: config_kind.clone(),
                ref_kind: ref_kind.clone(),
                name: value,
                line: *ref_line,
            });
            active_ref = None;
            continue;
        }

        if indent == 0 {
            if yaml_key_value(trimmed, "apiVersion").is_some() {
                has_api_version = true;
            } else if let Some(value) = yaml_key_value(trimmed, "kind") {
                kind = Some(value);
            } else if yaml_key(trimmed).is_some_and(|key| key == "metadata") {
                metadata_indent = Some(indent);
            }
        }

        if let Some(parent_indent) = metadata_indent {
            if indent <= parent_indent && yaml_key(trimmed).is_some_and(|key| key != "metadata") {
                metadata_indent = None;
            } else if indent > parent_indent {
                if let Some(value) = yaml_key_value(trimmed, "name") {
                    name = Some(value);
                } else if let Some(value) = yaml_key_value(trimmed, "namespace") {
                    namespace = Some(value);
                }
            }
        }

        if let Some(template_indent) = active_template_indent
            && indent <= template_indent
        {
            active_template_indent = None;
            active_template_metadata_indent = None;
        }
        if let Some(template_metadata_indent) = active_template_metadata_indent
            && indent <= template_metadata_indent
        {
            active_template_metadata_indent = None;
        }
        if let Some(selector_indent) = active_selector_indent
            && indent <= selector_indent
        {
            active_selector_indent = None;
        }
        if let Some((_, port_indent)) = active_service_port.as_ref()
            && indent <= *port_indent
        {
            flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
        }
        if let Some(ports_indent) = active_ports_indent
            && indent <= ports_indent
        {
            flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
            active_ports_indent = None;
        }
        if let Some(containers_indent) = active_containers_indent
            && indent <= containers_indent
        {
            active_containers_indent = None;
        }

        if yaml_key(trimmed).is_some_and(|key| key == "ports") {
            active_ports_indent = Some(indent);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "containers") {
            active_containers_indent = Some(indent);
            continue;
        }
        if let Some(value) = yaml_key_value(item_trimmed, "host") {
            active_ingress_host = Some(value);
            continue;
        }
        if let Some(value) = yaml_key_value(item_trimmed, "path") {
            active_ingress_path = Some(value);
            continue;
        }
        if let Some(value) = yaml_key_value(item_trimmed, "pathType") {
            active_ingress_path_type = Some(value);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "backend") {
            flush_kubernetes_ingress_backend(&mut ingress_backends, &mut active_ingress_backend);
            active_ingress_backend = Some((
                KubernetesIngressBackend {
                    service_name: String::new(),
                    service_port: None,
                    host: active_ingress_host.clone(),
                    path: active_ingress_path.clone(),
                    path_type: active_ingress_path_type.clone(),
                    line,
                },
                indent,
            ));
            active_ingress_service_indent = None;
            continue;
        }
        if active_ingress_backend.is_some() && yaml_key(trimmed).is_some_and(|key| key == "service")
        {
            active_ingress_service_indent = Some(indent);
            continue;
        }
        if active_ingress_service_indent.is_some()
            && let Some((backend, _)) = active_ingress_backend.as_mut()
        {
            if let Some(value) = yaml_key_value(trimmed, "name") {
                if backend.service_name.is_empty() {
                    backend.service_name = value;
                    backend.line = line;
                } else if backend.service_port.is_none() {
                    backend.service_port = Some(value);
                }
                continue;
            }
            if let Some(value) = yaml_key_value(trimmed, "number") {
                backend.service_port = Some(value);
                continue;
            }
        }
        if yaml_key(trimmed).is_some_and(|key| key == "template") {
            active_template_indent = Some(indent);
            continue;
        }
        if active_template_indent.is_some()
            && yaml_key(trimmed).is_some_and(|key| key == "metadata")
        {
            active_template_metadata_indent = Some(indent);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "selector") {
            active_selector_indent = Some(indent);
            continue;
        }
        if yaml_key(trimmed).is_some_and(|key| key == "labels") {
            if active_template_metadata_indent.is_some() {
                active_label_map = Some((KubernetesLabelTarget::PodTemplate, indent));
            } else if metadata_indent.is_some_and(|metadata_indent| indent > metadata_indent) {
                active_label_map = Some((KubernetesLabelTarget::Metadata, indent));
            }
            continue;
        }
        if active_selector_indent.is_some()
            && yaml_key(trimmed).is_some_and(|key| key == "matchLabels")
        {
            active_label_map = Some((KubernetesLabelTarget::Selector, indent));
            continue;
        }
        if active_selector_indent.is_some()
            && let Some((key, value)) = yaml_key_pair(trimmed)
            && key != "matchLabels"
        {
            selector_labels.insert(key, value);
            continue;
        }

        if let Some((config_kind, ref_kind)) = kubernetes_ref_key(item_trimmed) {
            if let Some(value) = yaml_key_value(item_trimmed, &ref_kind)
                && let Some(name) = yaml_inline_mapping_value(&value, "name")
            {
                config_refs.push(KubernetesConfigRef {
                    config_kind,
                    ref_kind,
                    name,
                    line,
                });
            } else {
                active_ref = Some((config_kind, ref_kind, indent, line));
            }
            continue;
        }

        if let Some(containers_indent) = active_containers_indent
            && indent == containers_indent + 2
            && trimmed
                .strip_prefix("- ")
                .and_then(|value| yaml_key_value(value, "name"))
                .is_some()
        {
            container_count += 1;
        }

        if active_ports_indent.is_some() {
            if let Some(value) = trimmed.strip_prefix("- ") {
                flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
                let mut port = KubernetesServicePort {
                    name: None,
                    port: None,
                    target_port: None,
                    protocol: "TCP".to_string(),
                    line,
                };
                if let Some((key, value)) = yaml_key_pair(value) {
                    apply_kubernetes_service_port_field(&mut port, key, value);
                }
                active_service_port = Some((port, indent));
            } else if let Some((key, value)) = yaml_key_pair(trimmed)
                && let Some((port, _)) = active_service_port.as_mut()
            {
                apply_kubernetes_service_port_field(port, key, value);
            }
        }
    }

    flush_kubernetes_service_port(&mut service_ports, &mut active_service_port);
    flush_kubernetes_ingress_backend(&mut ingress_backends, &mut active_ingress_backend);

    let kind = kind?;
    let name = name?;
    if !has_api_version || !kubernetes_known_kind(&kind) {
        return None;
    }

    Some(KubernetesDocument {
        kind,
        name,
        namespace: namespace.unwrap_or_else(|| "default".to_string()),
        line: start_line,
        labels,
        pod_labels,
        selector_labels,
        config_refs,
        service_ports,
        ingress_backends,
        container_count,
    })
}

fn kubernetes_ref_key(trimmed: &str) -> Option<(String, String)> {
    let ref_kind = yaml_key(trimmed)?;
    let config_kind = match ref_kind.as_str() {
        "configMapRef" | "configMapKeyRef" => "configmap",
        "secretRef" | "secretKeyRef" => "secret",
        _ => return None,
    };
    Some((config_kind.to_string(), ref_kind))
}

fn yaml_inline_mapping_value(value: &str, expected_key: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    for part in value.split(',') {
        if let Some(found) = yaml_key_value(part.trim(), expected_key) {
            return Some(found);
        }
    }
    None
}

fn apply_kubernetes_service_port_field(
    port: &mut KubernetesServicePort,
    key: String,
    value: String,
) {
    match key.as_str() {
        "name" if !value.is_empty() => port.name = Some(value),
        "port" if !value.is_empty() => port.port = Some(value),
        "targetPort" if !value.is_empty() => port.target_port = Some(value),
        "protocol" if !value.is_empty() => port.protocol = value.to_ascii_uppercase(),
        _ => {}
    }
}

fn flush_kubernetes_service_port(
    ports: &mut Vec<KubernetesServicePort>,
    active_port: &mut Option<(KubernetesServicePort, usize)>,
) {
    let Some((port, _)) = active_port.take() else {
        return;
    };
    if port.port.is_some() || port.target_port.is_some() {
        ports.push(port);
    }
}

fn flush_kubernetes_ingress_backend(
    backends: &mut Vec<KubernetesIngressBackend>,
    active_backend: &mut Option<(KubernetesIngressBackend, usize)>,
) {
    let Some((backend, _)) = active_backend.take() else {
        return;
    };
    if !backend.service_name.is_empty() {
        backends.push(backend);
    }
}

fn kubernetes_known_kind(kind: &str) -> bool {
    kubernetes_config_kind(kind).is_some()
        || matches!(kind, "Ingress" | "Service")
        || kubernetes_workload_kind(kind)
}

fn kubernetes_config_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "ConfigMap" => Some("configmap"),
        "Secret" => Some("secret"),
        _ => None,
    }
}

fn kubernetes_workload_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Deployment" | "StatefulSet" | "DaemonSet" | "Job" | "CronJob" | "Pod"
    )
}

fn kubernetes_resource_label(kind: &str, namespace: &str, name: &str) -> String {
    format!("k8s {kind}:{namespace}/{name}")
}

fn github_actions_workflow(label: &str, source: &str) -> GithubActionsWorkflow {
    let fallback_name = Path::new(label)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("workflow")
        .to_string();
    let mut workflow_name = None;
    let mut workflow_environment = Vec::new();
    let mut jobs = Vec::new();
    let mut in_jobs = false;
    let mut jobs_indent = 0usize;
    let mut active_workflow_section: Option<(String, usize)> = None;
    let mut active_job: Option<GithubActionsJob> = None;
    let mut active_step: Option<(GithubActionsStep, usize)> = None;
    let mut active_section: Option<(String, usize)> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if !in_jobs {
            if let Some((section, section_indent)) = active_workflow_section.as_ref() {
                if indent <= *section_indent {
                    active_workflow_section = None;
                } else if section == "env" {
                    if let Some(environment) =
                        ci_environment_assignment(trimmed, "workflow", index as u32 + 1)
                    {
                        workflow_environment.push(environment);
                    }
                    continue;
                }
            }
            if indent == 0
                && workflow_name.is_none()
                && let Some(value) = yaml_key_value(trimmed, "name")
            {
                workflow_name = Some(value);
            }
            if indent == 0 && yaml_key(trimmed).is_some_and(|key| key == "env") {
                active_workflow_section = Some(("env".to_string(), indent));
                continue;
            }
            if yaml_key(trimmed).is_some_and(|key| key == "jobs") {
                in_jobs = true;
                jobs_indent = indent;
                active_workflow_section = None;
            }
            continue;
        }

        if indent <= jobs_indent && yaml_key(trimmed).is_some() {
            flush_github_actions_step(&mut active_job, &mut active_step);
            flush_github_actions_job(&mut active_job, &mut jobs);
            break;
        }

        if indent == jobs_indent + 2 {
            flush_github_actions_step(&mut active_job, &mut active_step);
            flush_github_actions_job(&mut active_job, &mut jobs);
            active_section = None;
            let Some(id) = yaml_key(trimmed) else {
                continue;
            };
            active_job = Some(GithubActionsJob {
                id,
                display_name: None,
                runs_on: None,
                needs: Vec::new(),
                environment: Vec::new(),
                steps: Vec::new(),
                line: index as u32 + 1,
            });
            continue;
        }

        if indent == jobs_indent + 4 {
            flush_github_actions_step(&mut active_job, &mut active_step);
            let Some(job) = active_job.as_mut() else {
                continue;
            };
            active_section = None;
            if let Some(value) = yaml_key_value(trimmed, "name") {
                job.display_name = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "runs-on") {
                job.runs_on = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "needs") {
                job.needs.extend(github_actions_needs_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "needs") {
                active_section = Some(("needs".to_string(), indent));
            } else if yaml_key(trimmed).is_some_and(|key| key == "env") {
                active_section = Some(("env".to_string(), indent));
            } else if yaml_key(trimmed).is_some_and(|key| key == "steps") {
                active_section = Some(("steps".to_string(), indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            flush_github_actions_step(&mut active_job, &mut active_step);
            active_section = None;
            continue;
        }

        match section.as_str() {
            "needs" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    let dependency = yaml_clean_scalar(value);
                    if !dependency.is_empty() {
                        if let Some(job) = active_job.as_mut() {
                            job.needs.push(dependency);
                        }
                    }
                }
            }
            "env" => {
                if let Some(environment) =
                    ci_environment_assignment(trimmed, "job", index as u32 + 1)
                    && let Some(job) = active_job.as_mut()
                {
                    job.environment.push(environment);
                }
            }
            "steps" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    flush_github_actions_step(&mut active_job, &mut active_step);
                    let mut step = GithubActionsStep {
                        name: None,
                        uses: None,
                        run: None,
                        line: index as u32 + 1,
                    };
                    apply_github_actions_step_field(&mut step, value);
                    active_step = Some((step, indent));
                } else if let Some((step, step_indent)) = active_step.as_mut()
                    && indent > *step_indent
                {
                    apply_github_actions_step_field(step, trimmed);
                }
            }
            _ => {}
        }
    }

    flush_github_actions_step(&mut active_job, &mut active_step);
    flush_github_actions_job(&mut active_job, &mut jobs);
    GithubActionsWorkflow {
        name: workflow_name.unwrap_or(fallback_name),
        environment: workflow_environment,
        jobs,
    }
}

fn flush_github_actions_job(
    active_job: &mut Option<GithubActionsJob>,
    jobs: &mut Vec<GithubActionsJob>,
) {
    let Some(mut job) = active_job.take() else {
        return;
    };
    job.needs.sort();
    job.needs.dedup();
    jobs.push(job);
}

fn flush_github_actions_step(
    active_job: &mut Option<GithubActionsJob>,
    active_step: &mut Option<(GithubActionsStep, usize)>,
) {
    let Some((step, _)) = active_step.take() else {
        return;
    };
    if step.uses.is_some() || step.run.is_some() {
        if let Some(job) = active_job.as_mut() {
            job.steps.push(step);
        }
    }
}

fn apply_github_actions_step_field(step: &mut GithubActionsStep, field: &str) {
    if let Some(value) = yaml_key_value(field, "name") {
        step.name = Some(value);
    } else if let Some(value) = yaml_key_value(field, "uses") {
        step.uses = Some(value);
    } else if let Some(value) = yaml_key_value(field, "run") {
        step.run = Some(value);
    }
}

fn github_actions_needs_values(value: &str) -> Vec<String> {
    let inline = yaml_inline_list_values(value);
    if !inline.is_empty() {
        return inline;
    }
    let value = yaml_clean_scalar(value);
    (!value.is_empty()).then_some(value).into_iter().collect()
}

fn github_actions_local_action_path(action: &str) -> Option<String> {
    let action = action.trim().trim_matches('"').trim_matches('\'');
    if !action.starts_with("./") {
        return None;
    }
    normalize_relative_path(Path::new(action))
}

fn github_actions_remote_action(action: &str) -> (String, Option<String>) {
    let action = action.trim();
    let (name, version) = action
        .rsplit_once('@')
        .map(|(name, version)| (name.trim(), Some(version.trim().to_string())))
        .unwrap_or((action, None));
    let name = name.trim_matches('"').trim_matches('\'').to_string();
    let version = version.filter(|value| !value.is_empty());
    (name, version)
}

fn is_github_actions_workflow_path(label: &str, path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str());
    matches!(name, Some(name) if name.ends_with(".yml") || name.ends_with(".yaml"))
        && label.starts_with(".github/workflows/")
}

fn gitlab_ci_jobs(source: &str) -> Vec<GitlabCiJob> {
    let mut jobs = Vec::new();
    let mut global_variables = Vec::new();
    let mut active_job: Option<GitlabCiJob> = None;
    let mut active_global_section: Option<(String, usize)> = None;
    let mut active_section: Option<(String, usize)> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);
        if let Some((section, section_indent)) = active_global_section.as_ref() {
            if indent <= *section_indent {
                active_global_section = None;
            } else if section == "variables" {
                if let Some(variable) =
                    ci_environment_assignment(trimmed, "pipeline", index as u32 + 1)
                {
                    global_variables.push(variable);
                }
                continue;
            }
        }
        if indent == 0 {
            flush_gitlab_ci_job(&mut active_job, &mut jobs);
            active_section = None;
            let Some(name) = yaml_key(trimmed) else {
                continue;
            };
            if name == "variables" {
                active_global_section = Some(("variables".to_string(), indent));
                continue;
            }
            if gitlab_ci_reserved_key(&name) || name.starts_with('.') {
                continue;
            }
            active_job = Some(GitlabCiJob {
                name,
                stage: None,
                image: None,
                extends: Vec::new(),
                needs: Vec::new(),
                dependencies: Vec::new(),
                variables: global_variables.clone(),
                scripts: Vec::new(),
                line: index as u32 + 1,
            });
            continue;
        }

        let Some(job) = active_job.as_mut() else {
            continue;
        };

        if indent == 2 {
            active_section = None;
            if let Some(value) = yaml_key_value(trimmed, "stage") {
                job.stage = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "image") {
                job.image = Some(value);
            } else if let Some(value) = yaml_key_value(trimmed, "extends") {
                job.extends.extend(gitlab_ci_name_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "extends") {
                active_section = Some(("extends".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "needs") {
                job.needs.extend(gitlab_ci_name_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "needs") {
                active_section = Some(("needs".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "dependencies") {
                job.dependencies.extend(gitlab_ci_name_values(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "dependencies") {
                active_section = Some(("dependencies".to_string(), indent));
            } else if yaml_key(trimmed).is_some_and(|key| key == "variables") {
                active_section = Some(("variables".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "script") {
                push_gitlab_ci_script(job, "script", value, index as u32 + 1);
            } else if yaml_key(trimmed).is_some_and(|key| key == "script") {
                active_section = Some(("script".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "before_script") {
                push_gitlab_ci_script(job, "before_script", value, index as u32 + 1);
            } else if yaml_key(trimmed).is_some_and(|key| key == "before_script") {
                active_section = Some(("before_script".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "after_script") {
                push_gitlab_ci_script(job, "after_script", value, index as u32 + 1);
            } else if yaml_key(trimmed).is_some_and(|key| key == "after_script") {
                active_section = Some(("after_script".to_string(), indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            active_section = None;
            continue;
        }

        match section.as_str() {
            "extends" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    job.extends.extend(gitlab_ci_name_values(value));
                }
            }
            "needs" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    job.needs.extend(gitlab_ci_need_values(value));
                } else if let Some(value) = yaml_key_value(trimmed, "job") {
                    job.needs.push(value);
                }
            }
            "dependencies" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    job.dependencies.extend(gitlab_ci_name_values(value));
                }
            }
            "variables" => {
                if let Some(variable) = ci_environment_assignment(trimmed, "job", index as u32 + 1)
                {
                    job.variables.push(variable);
                }
            }
            "script" | "before_script" | "after_script" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    push_gitlab_ci_script(job, section, yaml_clean_scalar(value), index as u32 + 1);
                }
            }
            _ => {}
        }
    }

    flush_gitlab_ci_job(&mut active_job, &mut jobs);
    jobs
}

fn flush_gitlab_ci_job(active_job: &mut Option<GitlabCiJob>, jobs: &mut Vec<GitlabCiJob>) {
    let Some(mut job) = active_job.take() else {
        return;
    };
    job.extends.sort();
    job.extends.dedup();
    job.needs.sort();
    job.needs.dedup();
    job.dependencies.sort();
    job.dependencies.dedup();
    if !job.scripts.is_empty() || !job.needs.is_empty() || !job.dependencies.is_empty() {
        jobs.push(job);
    }
}

fn push_gitlab_ci_script(job: &mut GitlabCiJob, script_kind: &str, command: String, line: u32) {
    let inline = yaml_inline_list_values(&command);
    if !inline.is_empty() {
        for command in inline {
            if !command.is_empty() {
                job.scripts.push(GitlabCiScript {
                    command,
                    script_kind: script_kind.to_string(),
                    ordinal: job.scripts.len() + 1,
                    line,
                });
            }
        }
        return;
    }
    let command = yaml_clean_scalar(&command);
    if command.is_empty() || command == "|" || command == ">" {
        return;
    }
    job.scripts.push(GitlabCiScript {
        command,
        script_kind: script_kind.to_string(),
        ordinal: job.scripts.len() + 1,
        line,
    });
}

fn gitlab_ci_name_values(value: &str) -> Vec<String> {
    let inline = yaml_inline_list_values(value);
    if !inline.is_empty() {
        return inline;
    }
    let value = yaml_clean_scalar(value);
    (!value.is_empty()).then_some(value).into_iter().collect()
}

fn gitlab_ci_need_values(value: &str) -> Vec<String> {
    if let Some(job) = yaml_key_value(value.trim(), "job") {
        return vec![job];
    }
    gitlab_ci_name_values(value)
}

fn ci_environment_assignment(trimmed: &str, scope: &str, line: u32) -> Option<CiEnvironment> {
    let name = yaml_key(trimmed)?;
    let value = yaml_key_value(trimmed, &name);
    let value_present = value.is_some();
    let value_kind = value
        .as_deref()
        .map(ci_environment_value_kind)
        .unwrap_or("runner")
        .to_string();
    Some(CiEnvironment {
        name,
        value_present,
        value_kind,
        scope: scope.to_string(),
        line,
    })
}

fn ci_environment_value_kind(value: &str) -> &'static str {
    let value = value.trim();
    if value.contains("secrets.") || value.contains("vault.") {
        "secret_reference"
    } else if value.contains("${{") || value.contains("$[") {
        "expression"
    } else if value.starts_with('$') || value.contains("${") || value.contains('%') {
        "variable_reference"
    } else {
        "literal"
    }
}

fn gitlab_ci_reserved_key(key: &str) -> bool {
    matches!(
        key,
        "after_script"
            | "before_script"
            | "cache"
            | "default"
            | "image"
            | "include"
            | "services"
            | "stages"
            | "variables"
            | "workflow"
    )
}

fn is_gitlab_ci_path(label: &str, path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".gitlab-ci.yml" || name == ".gitlab-ci.yaml")
        || label.starts_with(".gitlab/ci/")
            && matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some(name) if name.ends_with(".yml") || name.ends_with(".yaml")
            )
}

fn kubernetes_config_ref_label(config_kind: &str, namespace: &str, name: &str) -> String {
    format!("k8s config ref:{config_kind} {namespace}/{name}")
}

fn kubernetes_service_ref_label(namespace: &str, name: &str) -> String {
    format!("k8s service ref:{namespace}/{name}")
}

fn kubernetes_service_port_label(
    document: &KubernetesDocument,
    port: &KubernetesServicePort,
) -> String {
    let port_value = port.port.as_deref().unwrap_or("unknown");
    match port.target_port.as_deref() {
        Some(target) => format!(
            "k8s service port:{}/{}:{}->{}/{}",
            document.namespace, document.name, port_value, target, port.protocol
        ),
        None => format!(
            "k8s service port:{}/{}:{}/{}",
            document.namespace, document.name, port_value, port.protocol
        ),
    }
}

fn kubernetes_workload_match_labels(document: &KubernetesDocument) -> &BTreeMap<String, String> {
    if !document.pod_labels.is_empty() {
        &document.pod_labels
    } else {
        &document.labels
    }
}

fn kubernetes_selector_matches(
    selector: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
) -> bool {
    !selector.is_empty()
        && selector
            .iter()
            .all(|(key, value)| labels.get(key).is_some_and(|label| label == value))
}

fn kubernetes_label_selector_string(labels: &BTreeMap<String, String>) -> String {
    labels
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn compose_services(source: &str) -> Vec<ComposeService> {
    let mut services = Vec::new();
    let mut in_services = false;
    let mut services_indent = 0usize;
    let mut active_service: Option<ComposeService> = None;
    let mut active_section: Option<(String, usize)> = None;
    let mut active_port: Option<ComposePort> = None;
    let mut active_volume: Option<ComposeVolume> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if !in_services {
            if yaml_key(trimmed).is_some_and(|key| key == "services") {
                in_services = true;
                services_indent = indent;
            }
            continue;
        }

        if indent <= services_indent && yaml_key(trimmed).is_some() {
            break;
        }

        if indent == services_indent + 2 {
            if let Some(mut service) = active_service.take() {
                flush_compose_service_port(&mut service, &mut active_port);
                flush_compose_service_volume(&mut service, &mut active_volume);
                services.push(service);
            }
            active_section = None;
            let Some(name) = yaml_key(trimmed).filter(|name| !compose_service_reserved_key(name))
            else {
                continue;
            };
            active_service = Some(ComposeService {
                name,
                command: None,
                command_kind: None,
                build_context: None,
                dockerfile: None,
                depends_on: Vec::new(),
                environment: Vec::new(),
                env_files: Vec::new(),
                ports: Vec::new(),
                volumes: Vec::new(),
                line: index as u32 + 1,
            });
            continue;
        }

        let Some(service) = active_service.as_mut() else {
            continue;
        };

        if indent == services_indent + 4 {
            if active_section
                .as_ref()
                .is_some_and(|(section, _)| section == "ports")
            {
                flush_compose_service_port(service, &mut active_port);
            }
            if active_section
                .as_ref()
                .is_some_and(|(section, _)| section == "volumes")
            {
                flush_compose_service_volume(service, &mut active_volume);
            }
            active_section = None;
            if let Some(value) = yaml_key_value(trimmed, "command") {
                service.command = compose_command_string(&value);
                service.command_kind = Some("command".to_string());
            } else if let Some(value) = yaml_key_value(trimmed, "entrypoint") {
                service.command = compose_command_string(&value);
                service.command_kind = Some("entrypoint".to_string());
            } else if let Some(value) = yaml_key_value(trimmed, "build") {
                service.build_context = Some(value);
            } else if yaml_key(trimmed).is_some_and(|key| key == "build") {
                active_section = Some(("build".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "depends_on") {
                service.depends_on.extend(compose_inline_depends_on(&value));
            } else if yaml_key(trimmed).is_some_and(|key| key == "depends_on") {
                active_section = Some(("depends_on".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "environment") {
                service
                    .environment
                    .extend(compose_inline_environment(&value, index as u32 + 1).into_iter());
            } else if yaml_key(trimmed).is_some_and(|key| key == "environment") {
                active_section = Some(("environment".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "env_file") {
                service
                    .env_files
                    .extend(compose_env_file_values(&value, index as u32 + 1));
            } else if yaml_key(trimmed).is_some_and(|key| key == "env_file") {
                active_section = Some(("env_file".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "ports") {
                service
                    .ports
                    .extend(compose_inline_ports(&value, index as u32 + 1));
            } else if yaml_key(trimmed).is_some_and(|key| key == "ports") {
                active_section = Some(("ports".to_string(), indent));
            } else if let Some(value) = yaml_key_value(trimmed, "volumes") {
                service
                    .volumes
                    .extend(compose_inline_volumes(&value, index as u32 + 1));
            } else if yaml_key(trimmed).is_some_and(|key| key == "volumes") {
                active_section = Some(("volumes".to_string(), indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            active_section = None;
            continue;
        }

        match section.as_str() {
            "build" => {
                if let Some(value) = yaml_key_value(trimmed, "context") {
                    service.build_context = Some(value);
                } else if let Some(value) = yaml_key_value(trimmed, "dockerfile") {
                    service.dockerfile = Some(value);
                }
            }
            "depends_on" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    let dependency = yaml_clean_scalar(value);
                    if !dependency.is_empty() {
                        service.depends_on.push(dependency);
                    }
                } else if let Some(name) = yaml_key(trimmed)
                    && !compose_depends_on_option_key(&name)
                {
                    service.depends_on.push(name);
                }
            }
            "environment" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    if let Some(environment) =
                        compose_environment_assignment(value, index as u32 + 1)
                    {
                        service.environment.push(environment);
                    }
                } else if let Some(name) = yaml_key(trimmed) {
                    let value_present = yaml_key_value(trimmed, &name).is_some();
                    service.environment.push(ComposeEnvironment {
                        name,
                        value_present,
                        line: index as u32 + 1,
                    });
                }
            }
            "env_file" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    let path = yaml_clean_scalar(value);
                    if !path.is_empty() {
                        service.env_files.push(ComposeEnvFile {
                            path,
                            line: index as u32 + 1,
                        });
                    }
                }
            }
            "ports" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    flush_compose_service_port(service, &mut active_port);
                    if let Some(port) = compose_short_port(value, index as u32 + 1) {
                        service.ports.push(port);
                    } else if let Some((key, value)) = yaml_key_pair(value) {
                        active_port = Some(compose_long_port(key, value, index as u32 + 1));
                    }
                } else if let Some((key, value)) = yaml_key_pair(trimmed) {
                    if let Some(port) = active_port.as_mut() {
                        apply_compose_port_field(port, key, value);
                    }
                }
            }
            "volumes" => {
                if let Some(value) = trimmed.strip_prefix("- ") {
                    flush_compose_service_volume(service, &mut active_volume);
                    if let Some(volume) = compose_short_volume(value, index as u32 + 1) {
                        service.volumes.push(volume);
                    } else if let Some((key, value)) = yaml_key_pair(value) {
                        active_volume = Some(compose_long_volume(key, value, index as u32 + 1));
                    }
                } else if let Some((key, value)) = yaml_key_pair(trimmed) {
                    if let Some(volume) = active_volume.as_mut() {
                        apply_compose_volume_field(volume, key, value);
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(mut service) = active_service {
        flush_compose_service_port(&mut service, &mut active_port);
        flush_compose_service_volume(&mut service, &mut active_volume);
        services.push(service);
    }
    dedupe_compose_service_dependencies(&mut services);
    services
}

fn compose_service_reserved_key(name: &str) -> bool {
    matches!(
        name,
        "build"
            | "command"
            | "depends_on"
            | "entrypoint"
            | "image"
            | "networks"
            | "volumes"
            | "environment"
            | "env_file"
            | "ports"
    )
}

fn compose_depends_on_option_key(name: &str) -> bool {
    matches!(
        name,
        "condition" | "restart" | "required" | "service_healthy" | "service_started"
    )
}

fn compose_command_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('[') {
        dockerfile_exec_form_command(value)
    } else {
        Some(value.to_string())
    }
}

fn compose_inline_depends_on(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.starts_with('[') {
        yaml_inline_list_values(value)
    } else if value.is_empty() {
        Vec::new()
    } else {
        vec![value.to_string()]
    }
}

fn compose_inline_environment(value: &str, line: u32) -> Vec<ComposeEnvironment> {
    yaml_inline_list_values(value)
        .into_iter()
        .filter_map(|value| compose_environment_assignment(&value, line))
        .collect()
}

fn compose_environment_assignment(value: &str, line: u32) -> Option<ComposeEnvironment> {
    let value = yaml_clean_scalar(value);
    let (name, value_present) = value
        .split_once('=')
        .map(|(name, _)| (name.trim(), true))
        .unwrap_or((value.trim(), false));
    (!name.is_empty()).then(|| ComposeEnvironment {
        name: name.to_string(),
        value_present,
        line,
    })
}

fn compose_env_file_values(value: &str, line: u32) -> Vec<ComposeEnvFile> {
    let values = if value.trim().starts_with('[') {
        yaml_inline_list_values(value)
    } else {
        vec![yaml_clean_scalar(value)]
    };
    values
        .into_iter()
        .filter(|path| !path.is_empty())
        .map(|path| ComposeEnvFile { path, line })
        .collect()
}

fn compose_inline_ports(value: &str, line: u32) -> Vec<ComposePort> {
    yaml_inline_list_values(value)
        .into_iter()
        .filter_map(|value| compose_short_port(&value, line))
        .collect()
}

fn compose_short_port(value: &str, line: u32) -> Option<ComposePort> {
    let raw = yaml_clean_scalar(value);
    if raw.is_empty() || yaml_key_pair(&raw).is_some() {
        return None;
    }
    let (port_spec, protocol) = compose_port_protocol(&raw);
    let parts: Vec<_> = port_spec.split(':').map(str::trim).collect();
    let (host_ip, published, target) = match parts.as_slice() {
        [target] if !target.is_empty() => (None, None, Some((*target).to_string())),
        [published, target] if !published.is_empty() && !target.is_empty() => (
            None,
            Some((*published).to_string()),
            Some((*target).to_string()),
        ),
        [host_ip, published, target]
            if !host_ip.is_empty() && !published.is_empty() && !target.is_empty() =>
        {
            (
                Some((*host_ip).to_string()),
                Some((*published).to_string()),
                Some((*target).to_string()),
            )
        }
        _ => return None,
    };
    Some(ComposePort {
        published,
        target,
        protocol,
        host_ip,
        raw: Some(raw),
        line,
    })
}

fn compose_long_port(key: String, value: String, line: u32) -> ComposePort {
    let mut port = ComposePort {
        published: None,
        target: None,
        protocol: "tcp".to_string(),
        host_ip: None,
        raw: None,
        line,
    };
    apply_compose_port_field(&mut port, key, value);
    port
}

fn apply_compose_port_field(port: &mut ComposePort, key: String, value: String) {
    match key.as_str() {
        "published" => port.published = Some(value),
        "target" => port.target = Some(value),
        "protocol" if !value.is_empty() => port.protocol = value,
        "host_ip" if !value.is_empty() => port.host_ip = Some(value),
        _ => {}
    }
}

fn flush_compose_service_port(service: &mut ComposeService, active_port: &mut Option<ComposePort>) {
    let Some(port) = active_port.take() else {
        return;
    };
    if port.published.is_some() || port.target.is_some() {
        service.ports.push(port);
    }
}

fn compose_port_protocol(raw: &str) -> (String, String) {
    if let Some((port, protocol)) = raw.rsplit_once('/')
        && !port.is_empty()
        && !protocol.is_empty()
        && protocol
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return (port.to_string(), protocol.to_ascii_lowercase());
    }
    (raw.to_string(), "tcp".to_string())
}

fn compose_port_label(port: &ComposePort) -> String {
    let protocol = port.protocol.as_str();
    match (
        port.host_ip.as_deref(),
        port.published.as_deref(),
        port.target.as_deref(),
    ) {
        (Some(host_ip), Some(published), Some(target)) => {
            format!("compose port:{host_ip}:{published}->{target}/{protocol}")
        }
        (_, Some(published), Some(target)) => {
            format!("compose port:{published}->{target}/{protocol}")
        }
        (_, Some(published), None) => format!("compose port:{published}/{protocol}"),
        (_, None, Some(target)) => format!("compose port:{target}/{protocol}"),
        _ => "compose port:unknown".to_string(),
    }
}

fn compose_inline_volumes(value: &str, line: u32) -> Vec<ComposeVolume> {
    yaml_inline_list_values(value)
        .into_iter()
        .filter_map(|value| compose_short_volume(&value, line))
        .collect()
}

fn compose_short_volume(value: &str, line: u32) -> Option<ComposeVolume> {
    let raw = yaml_clean_scalar(value);
    if raw.is_empty() || yaml_key_pair(&raw).is_some() {
        return None;
    }
    let parts: Vec<_> = raw.split(':').map(str::trim).collect();
    let (source, target, read_only) = match parts.as_slice() {
        [target] if !target.is_empty() => (None, Some((*target).to_string()), false),
        [source, target] if !source.is_empty() && !target.is_empty() => (
            Some((*source).to_string()),
            Some((*target).to_string()),
            false,
        ),
        [source, target, mode] if !source.is_empty() && !target.is_empty() => (
            Some((*source).to_string()),
            Some((*target).to_string()),
            compose_volume_mode_read_only(mode),
        ),
        _ => return None,
    };
    Some(ComposeVolume {
        source,
        target,
        kind: "volume".to_string(),
        read_only,
        raw: Some(raw),
        line,
    })
}

fn compose_long_volume(key: String, value: String, line: u32) -> ComposeVolume {
    let mut volume = ComposeVolume {
        source: None,
        target: None,
        kind: "volume".to_string(),
        read_only: false,
        raw: None,
        line,
    };
    apply_compose_volume_field(&mut volume, key, value);
    volume
}

fn apply_compose_volume_field(volume: &mut ComposeVolume, key: String, value: String) {
    match key.as_str() {
        "type" if !value.is_empty() => volume.kind = value,
        "source" if !value.is_empty() => volume.source = Some(value),
        "target" if !value.is_empty() => volume.target = Some(value),
        "read_only" | "readonly" => volume.read_only = yaml_truthy(&value),
        _ => {}
    }
}

fn flush_compose_service_volume(
    service: &mut ComposeService,
    active_volume: &mut Option<ComposeVolume>,
) {
    let Some(volume) = active_volume.take() else {
        return;
    };
    if volume.source.is_some() || volume.target.is_some() {
        service.volumes.push(volume);
    }
}

fn compose_volume_mode_read_only(mode: &str) -> bool {
    mode.split(',').any(|value| {
        let value = value.trim();
        matches!(value, "ro" | "readonly" | "read_only")
    })
}

fn yaml_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "1" | "on"
    )
}

fn compose_volume_local_source_path(compose_label: &str, source: &str) -> Option<String> {
    let source = source.trim();
    if source.is_empty()
        || source.starts_with('$')
        || source.contains("://")
        || Path::new(source).is_absolute()
    {
        return None;
    }
    if !source.starts_with('.')
        && !source.contains('/')
        && !source.contains('\\')
        && Path::new(source).extension().is_none()
    {
        return None;
    }
    normalize_manifest_relative_path(compose_label, source)
}

fn compose_volume_label(volume: &ComposeVolume) -> String {
    match (volume.source.as_deref(), volume.target.as_deref()) {
        (Some(source), Some(target)) => format!("compose volume:{source}->{target}"),
        (Some(source), None) => format!("compose volume:{source}"),
        (None, Some(target)) => format!("compose volume:{target}"),
        _ => "compose volume:unknown".to_string(),
    }
}

fn yaml_inline_list_values(value: &str) -> Vec<String> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Vec::new();
    }
    let body = value.trim_start_matches('[').trim_end_matches(']');
    let quoted = quoted_strings(body);
    if !quoted.is_empty() {
        return quoted
            .into_iter()
            .map(|value| yaml_clean_scalar(&value))
            .filter(|value| !value.is_empty())
            .collect();
    }
    body.split(',')
        .map(yaml_clean_scalar)
        .filter(|value| !value.is_empty())
        .collect()
}

fn compose_service_dockerfile_path(
    compose_label: &str,
    service: &ComposeService,
) -> Option<String> {
    compose_service_dockerfile_target(service)
        .and_then(|target| normalize_manifest_relative_path(compose_label, &target))
}

fn compose_service_dockerfile_target(service: &ComposeService) -> Option<String> {
    if service.build_context.is_none() && service.dockerfile.is_none() {
        return None;
    }
    let dockerfile = service.dockerfile.as_deref().unwrap_or("Dockerfile").trim();
    let context = service.build_context.as_deref().unwrap_or(".").trim();
    if context.contains("://") || Path::new(context).is_absolute() || dockerfile.is_empty() {
        return None;
    }
    Some(if context == "." {
        dockerfile.to_string()
    } else {
        join_path(Some(context), dockerfile)
    })
}

fn dedupe_compose_service_dependencies(services: &mut [ComposeService]) {
    for service in services {
        service.depends_on.sort();
        service.depends_on.dedup();
        service
            .depends_on
            .retain(|dependency| dependency != &service.name);
    }
}

fn dockerfile_entrypoints(source: &str) -> Vec<DockerfileEntrypoint> {
    dockerfile_logical_lines(source)
        .into_iter()
        .filter_map(|(line, text)| dockerfile_entrypoint_line(&text, line))
        .collect()
}

fn dockerfile_logical_lines(source: &str) -> Vec<(u32, String)> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut start_line = 1;

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let line = raw_line.trim();
        if current.is_empty() {
            start_line = line_number;
        }
        let continued = line.ends_with('\\');
        let part = line.trim_end_matches('\\').trim_end();
        if !current.is_empty() && !part.is_empty() {
            current.push(' ');
        }
        current.push_str(part);
        if !continued {
            lines.push((start_line, current.trim().to_string()));
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        lines.push((start_line, current.trim().to_string()));
    }

    lines
}

fn dockerfile_entrypoint_line(line: &str, line_number: u32) -> Option<DockerfileEntrypoint> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let instruction = parts.next()?.to_ascii_uppercase();
    if !matches!(instruction.as_str(), "ENTRYPOINT" | "CMD") {
        return None;
    }
    let body = parts.next()?.trim();
    if body.is_empty() {
        return None;
    }
    let command = if body.starts_with('[') {
        dockerfile_exec_form_command(body)
    } else {
        Some(body.to_string())
    }?;
    (!command.is_empty()).then_some(DockerfileEntrypoint {
        instruction,
        command,
        line: line_number,
    })
}

fn dockerfile_exec_form_command(body: &str) -> Option<String> {
    let values = quoted_strings(body);
    (!values.is_empty()).then(|| values.join(" "))
}

fn quoted_strings(value: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '"' && character != '\'' {
            continue;
        }
        let quote = character;
        let mut item = String::new();
        let mut escaped = false;
        for next in chars.by_ref() {
            if escaped {
                item.push(next);
                escaped = false;
                continue;
            }
            if next == '\\' {
                escaped = true;
                continue;
            }
            if next == quote {
                break;
            }
            item.push(next);
        }
        values.push(item);
    }
    values
}

fn makefile_targets(source: &str) -> Vec<MakefileTarget> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut targets = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let line_number = index as u32 + 1;
        let Some((target_names, inline_command)) = makefile_target_line(line) else {
            index += 1;
            continue;
        };

        let command = inline_command.or_else(|| {
            lines[index + 1..]
                .iter()
                .take_while(|line| makefile_recipe_or_blank_line(line))
                .find_map(|line| makefile_recipe_command(line))
        });

        for name in target_names {
            targets.push(MakefileTarget {
                name,
                command: command.clone(),
                line: line_number,
            });
        }
        index += 1;
    }

    targets
}

fn makefile_target_line(line: &str) -> Option<(Vec<String>, Option<String>)> {
    if line.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let candidate = line.split('#').next().unwrap_or("").trim_end();
    if candidate.is_empty() {
        return None;
    }
    let colon = candidate.find(':')?;
    if candidate[..colon].contains('=') {
        return None;
    }
    let before = candidate[..colon].trim();
    if before.is_empty() || before.starts_with('.') {
        return None;
    }

    let names = before
        .split_whitespace()
        .filter(|name| is_makefile_task_target(name))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if names.is_empty() {
        return None;
    }

    let inline_command = candidate[colon + 1..]
        .split_once(';')
        .map(|(_, command)| command.trim().to_string())
        .filter(|command| !command.is_empty());
    Some((names, inline_command))
}

fn is_makefile_task_target(name: &str) -> bool {
    let Some(first) = name.chars().next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() || name.contains('/') || name.contains('%') {
        return false;
    }
    name.chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

fn makefile_recipe_or_blank_line(line: &&str) -> bool {
    line.trim().is_empty() || line.starts_with('\t') || line.starts_with("        ")
}

fn makefile_recipe_command(line: &&str) -> Option<String> {
    if !line.starts_with('\t') && !line.starts_with("        ") {
        return None;
    }
    let command = line
        .trim_start()
        .trim_start_matches(['@', '-', '+'])
        .trim()
        .to_string();
    (!command.is_empty()).then_some(command)
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

fn pubspec_entrypoints(path: &Path, source: &str) -> Vec<ManifestEntrypoint> {
    let Some(package_name) = pubspec_package_name(source) else {
        return Vec::new();
    };
    let Some(root) = path.parent() else {
        return Vec::new();
    };

    let mut entrypoints = Vec::new();
    if root.join("lib").join("main.dart").is_file() {
        let ecosystem = if pubspec_uses_flutter(source) {
            "flutter"
        } else {
            "dart"
        };
        let prefix = if ecosystem == "flutter" {
            "flutter app"
        } else {
            "dart package"
        };
        entrypoints.push(manifest_entrypoint(
            format!("{prefix}:{package_name}"),
            "app",
            ecosystem,
            Some("lib/main.dart".to_string()),
        ));
    }

    let bin_dir = root.join("bin");
    if let Ok(commands) = fs::read_dir(&bin_dir) {
        for command in commands.flatten() {
            let command_path = command.path();
            if command_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("dart")
            {
                continue;
            }
            let Some(name) = command_path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            entrypoints.push(manifest_entrypoint(
                format!("dart bin:{name}"),
                "binary",
                "dart",
                Some(format!("bin/{name}.dart")),
            ));
        }
    }

    let test_dir = root.join("test");
    if let Ok(tests) = fs::read_dir(&test_dir) {
        for test in tests.flatten() {
            let test_path = test.path();
            let Some(name) = test_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.ends_with("_test.dart") {
                continue;
            }
            entrypoints.push(manifest_entrypoint(
                format!("dart test:{name}"),
                "test",
                "dart",
                Some(format!("test/{name}")),
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

fn setup_py_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    setup_py_console_scripts(source)
        .into_iter()
        .filter_map(|entrypoint| {
            let (name, target) = python_console_script_name_and_target(&entrypoint)?;
            Some(manifest_entrypoint(
                format!("python console_script:{name}"),
                "console_script",
                "python",
                Some(target),
            ))
        })
        .collect()
}

fn setup_cfg_entrypoints(source: &str) -> Vec<ManifestEntrypoint> {
    let sections = setup_cfg_sections(source);
    setup_cfg_values(&sections, "options.entry_points", "console_scripts")
        .into_iter()
        .filter_map(|entrypoint| {
            let (name, target) = python_console_script_name_and_target(&entrypoint)?;
            Some(manifest_entrypoint(
                format!("python console_script:{name}"),
                "console_script",
                "python",
                Some(target),
            ))
        })
        .collect()
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
        collect_poetry_group_dependencies(poetry, &mut dependencies);
    }

    dependencies
}

fn collect_poetry_group_dependencies(
    poetry: &toml::Value,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(groups) = poetry.get("group").and_then(|value| value.as_table()) else {
        return;
    };
    for (group_name, group) in groups {
        let dependency_kind = poetry_group_dependency_kind(group_name);
        collect_toml_table_keys(
            group,
            "dependencies",
            dependency_kind,
            "python",
            dependencies,
            None,
        );
    }
}

fn poetry_group_dependency_kind(group_name: &str) -> &'static str {
    match group_name.to_ascii_lowercase().as_str() {
        "dev" | "develop" | "development" => "dev",
        "test" | "tests" | "testing" => "test",
        _ => "optional",
    }
}

fn setup_py_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    collect_setup_py_requirement_key(source, "install_requires", "runtime", &mut dependencies);
    collect_setup_py_requirement_key(source, "setup_requires", "build", &mut dependencies);
    collect_setup_py_requirement_key(source, "tests_require", "test", &mut dependencies);

    for requirement in setup_py_dict_list_string_values(source, "extras_require") {
        if let Some((name, version)) = package_name_and_version_from_requirement(&requirement) {
            dependencies.push(manifest_dependency(name, "optional", "python", version));
        }
    }

    dependencies
}

fn collect_setup_py_requirement_key(
    source: &str,
    key: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    for requirement in setup_py_sequence_string_values(source, key) {
        if let Some((name, version)) = package_name_and_version_from_requirement(&requirement) {
            dependencies.push(manifest_dependency(
                name,
                dependency_kind,
                "python",
                version,
            ));
        }
    }
}

fn setup_cfg_dependencies(source: &str) -> Vec<ManifestDependency> {
    let sections = setup_cfg_sections(source);
    let mut dependencies = Vec::new();
    collect_setup_cfg_requirement_key(
        &sections,
        "options",
        "install_requires",
        "runtime",
        &mut dependencies,
    );
    collect_setup_cfg_requirement_key(
        &sections,
        "options",
        "setup_requires",
        "build",
        &mut dependencies,
    );
    collect_setup_cfg_requirement_key(
        &sections,
        "options",
        "tests_require",
        "test",
        &mut dependencies,
    );

    if let Some(extras) = sections.get("options.extras_require") {
        for requirements in extras.values() {
            for requirement in requirements {
                if let Some((name, version)) =
                    package_name_and_version_from_requirement(requirement)
                {
                    dependencies.push(manifest_dependency(name, "optional", "python", version));
                }
            }
        }
    }

    dependencies
}

fn pipfile_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = toml::from_str::<toml::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_pipfile_table(&value, "packages", "runtime", &mut dependencies);
    collect_pipfile_table(&value, "dev-packages", "dev", &mut dependencies);
    dependencies
}

fn collect_pipfile_table(
    value: &toml::Value,
    table_name: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(table) = value.get(table_name).and_then(|value| value.as_table()) else {
        return;
    };
    for (name, value) in table {
        dependencies.push(manifest_dependency(
            name.clone(),
            dependency_kind,
            "python",
            pipfile_dependency_version(value),
        ));
    }
}

fn collect_setup_cfg_requirement_key(
    sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    section: &str,
    key: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    for requirement in setup_cfg_values(sections, section, key) {
        if let Some((name, version)) = package_name_and_version_from_requirement(&requirement) {
            dependencies.push(manifest_dependency(
                name,
                dependency_kind,
                "python",
                version,
            ));
        }
    }
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

fn package_lock_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let Some(root_package) = value
        .get("packages")
        .and_then(|packages| packages.get(""))
        .and_then(|package| package.as_object())
    else {
        return Vec::new();
    };

    let mut dependencies = Vec::new();
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "dependencies",
        "runtime",
        &mut dependencies,
    );
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "devDependencies",
        "dev",
        &mut dependencies,
    );
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "peerDependencies",
        "peer",
        &mut dependencies,
    );
    collect_package_lock_root_dependencies(
        &value,
        root_package,
        "optionalDependencies",
        "optional",
        &mut dependencies,
    );
    dependencies
}

fn collect_package_lock_root_dependencies(
    value: &serde_json::Value,
    root_package: &serde_json::Map<String, serde_json::Value>,
    object_name: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(object) = root_package
        .get(object_name)
        .and_then(|value| value.as_object())
    else {
        return;
    };
    for (name, declared) in object {
        let locked_version = package_lock_package_version(value, name);
        let declared_version = declared
            .as_str()
            .map(str::trim)
            .filter(|version| !version.is_empty())
            .map(str::to_string);
        let (version, version_kind) = if let Some(version) = locked_version {
            (Some(version), Some("locked"))
        } else {
            (declared_version, Some("constraint"))
        };
        dependencies.push(manifest_dependency_with_version_kind(
            name.clone(),
            dependency_kind,
            "npm",
            version,
            version_kind,
        ));
    }
}

fn package_lock_package_version(value: &serde_json::Value, name: &str) -> Option<String> {
    value
        .get("packages")
        .and_then(|packages| packages.get(format!("node_modules/{name}")))
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(str::to_string)
}

#[derive(Debug)]
struct PendingPnpmDependency {
    name: String,
    kind: String,
    indent: usize,
    specifier: Option<String>,
    version: Option<String>,
}

fn pnpm_lock_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut in_importers = false;
    let mut in_importer = false;
    let mut active_section: Option<(&str, usize)> = None;
    let mut pending: Option<PendingPnpmDependency> = None;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if indent == 0 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            in_importers = trimmed == "importers:";
            in_importer = false;
            active_section = None;
            continue;
        }

        if !in_importers {
            continue;
        }

        if indent == 2 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            in_importer = yaml_key(trimmed).is_some();
            active_section = None;
            continue;
        }

        if !in_importer {
            continue;
        }

        if indent == 4 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            active_section = pnpm_dependency_section(trimmed).map(|kind| (kind, indent));
            continue;
        }

        let Some((dependency_kind, section_indent)) = active_section else {
            continue;
        };
        if indent <= section_indent {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            active_section = None;
            continue;
        }

        if indent == section_indent + 2 {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            if let Some(name) = yaml_key(trimmed) {
                pending = Some(PendingPnpmDependency {
                    name,
                    kind: dependency_kind.to_string(),
                    indent,
                    specifier: None,
                    version: None,
                });
            }
            continue;
        }

        let Some(dependency) = pending.as_mut() else {
            continue;
        };
        if indent <= dependency.indent {
            flush_pnpm_dependency(&mut pending, &mut dependencies);
            continue;
        }
        if let Some(value) = yaml_key_value(trimmed, "specifier") {
            dependency.specifier = Some(value);
        } else if let Some(value) = yaml_key_value(trimmed, "version") {
            dependency.version = Some(pnpm_clean_version(&value));
        }
    }

    flush_pnpm_dependency(&mut pending, &mut dependencies);
    dependencies
}

fn flush_pnpm_dependency(
    pending: &mut Option<PendingPnpmDependency>,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(dependency) = pending.take() else {
        return;
    };
    let (version, version_kind) = if let Some(version) = dependency.version {
        (Some(version), Some("locked"))
    } else {
        (
            dependency.specifier.filter(|value| !value.is_empty()),
            Some("constraint"),
        )
    };
    dependencies.push(manifest_dependency_with_version_kind(
        dependency.name,
        dependency.kind,
        "npm",
        version,
        version_kind,
    ));
}

fn pnpm_dependency_section(trimmed: &str) -> Option<&'static str> {
    match yaml_key(trimmed)?.as_str() {
        "dependencies" => Some("runtime"),
        "devDependencies" => Some("dev"),
        "peerDependencies" => Some("peer"),
        "optionalDependencies" => Some("optional"),
        _ => None,
    }
}

fn yaml_indent(raw_line: &str) -> usize {
    raw_line
        .chars()
        .take_while(|character| *character == ' ')
        .count()
}

fn yaml_key(trimmed: &str) -> Option<String> {
    let (key, _) = trimmed.split_once(':')?;
    let key = yaml_clean_scalar(key);
    (!key.is_empty()).then_some(key)
}

fn yaml_key_value(trimmed: &str, expected_key: &str) -> Option<String> {
    let (key, value) = trimmed.split_once(':')?;
    if yaml_clean_scalar(key) != expected_key {
        return None;
    }
    let value = yaml_clean_scalar(value);
    (!value.is_empty()).then_some(value)
}

fn yaml_key_pair(trimmed: &str) -> Option<(String, String)> {
    let (key, value) = trimmed.split_once(':')?;
    if value
        .chars()
        .next()
        .is_none_or(|character| !character.is_whitespace())
    {
        return None;
    }
    let key = yaml_clean_scalar(key);
    let value = yaml_clean_scalar(value);
    (!key.is_empty() && !value.is_empty()).then_some((key, value))
}

fn yaml_list_scalar(trimmed: &str) -> Option<String> {
    let value = trimmed.strip_prefix('-')?.trim();
    if value.is_empty() || yaml_key_pair(value).is_some() {
        None
    } else {
        Some(yaml_clean_scalar(value))
    }
}

fn yaml_clean_scalar(value: &str) -> String {
    value
        .split(" #")
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn pnpm_clean_version(value: &str) -> String {
    value.split('(').next().unwrap_or(value).trim().to_string()
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

fn composer_lock_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let mut dependencies = Vec::new();
    collect_composer_lock_packages(&value, "packages", "runtime", &mut dependencies);
    collect_composer_lock_packages(&value, "packages-dev", "dev", &mut dependencies);
    dependencies
}

fn collect_composer_lock_packages(
    value: &serde_json::Value,
    array_name: &str,
    dependency_kind: &str,
    dependencies: &mut Vec<ManifestDependency>,
) {
    let Some(packages) = value.get(array_name).and_then(|value| value.as_array()) else {
        return;
    };
    for package in packages {
        let Some(name) = package
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let version = package
            .get("version")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        dependencies.push(manifest_dependency_with_version_kind(
            name.to_string(),
            dependency_kind,
            "composer",
            version,
            Some("locked"),
        ));
    }
}

fn vcpkg_dependencies(source: &str) -> Vec<ManifestDependency> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return Vec::new();
    };
    let override_versions = vcpkg_override_versions(&value);
    let mut dependencies = Vec::new();
    let Some(values) = value.get("dependencies").and_then(|value| value.as_array()) else {
        return dependencies;
    };

    for value in values {
        let name = value
            .as_str()
            .map(str::to_string)
            .or_else(|| {
                value
                    .as_object()
                    .and_then(|object| object.get("name"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty());
        let Some(name) = name else {
            continue;
        };
        let version = value
            .as_object()
            .and_then(vcpkg_dependency_version)
            .or_else(|| override_versions.get(&name.to_ascii_lowercase()).cloned());
        dependencies.push(manifest_dependency(name, "runtime", "vcpkg", version));
    }

    dependencies
}

fn vcpkg_override_versions(value: &serde_json::Value) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    let Some(overrides) = value.get("overrides").and_then(|value| value.as_array()) else {
        return versions;
    };
    for override_value in overrides {
        let Some(object) = override_value.as_object() else {
            continue;
        };
        let Some(name) = object
            .get("name")
            .and_then(|value| value.as_str())
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        if let Some(version) = vcpkg_dependency_version(object) {
            versions.insert(name, version);
        }
    }
    versions
}

fn vcpkg_dependency_version(object: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    [
        "version>=",
        "version",
        "version-string",
        "version-date",
        "version-semver",
    ]
    .into_iter()
    .find_map(|key| {
        object
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                if key == "version>=" {
                    format!(">={value}")
                } else {
                    value.to_string()
                }
            })
    })
}

fn conanfile_txt_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut section: Option<String> = None;
    for raw_line in source.lines() {
        let line = raw_line
            .split('#')
            .next()
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = Some(name.trim().to_ascii_lowercase());
            continue;
        }
        let Some(section_name) = section.as_deref() else {
            continue;
        };
        let dependency_kind = match section_name {
            "requires" => "runtime",
            "tool_requires" | "build_requires" => "build",
            "test_requires" => "test",
            _ => continue,
        };
        let Some((name, version)) = conan_reference_name_and_version(line) else {
            continue;
        };
        dependencies.push(manifest_dependency(name, dependency_kind, "conan", version));
    }
    dependencies
}

fn conan_reference_name_and_version(line: &str) -> Option<(String, Option<String>)> {
    let reference = line
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('"')
        .trim_matches('\'')
        .trim();
    let (name, rest) = reference.split_once('/')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let version = rest
        .split('@')
        .next()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(str::to_string);
    Some((name.to_string(), version))
}

fn cmake_dependencies(source: &str) -> Vec<ManifestDependency> {
    cmake_command_bodies(source, "find_package")
        .into_iter()
        .filter_map(|body| {
            let args = cmake_command_args(&body);
            cmake_find_package_dependency(&args)
        })
        .collect()
}

fn cmake_find_package_dependency(args: &[String]) -> Option<ManifestDependency> {
    let name = args
        .first()
        .map(String::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| !name.starts_with('$'))?;
    if is_cmake_find_package_option(name) {
        return None;
    }
    let version = args
        .iter()
        .skip(1)
        .find(|arg| is_cmake_version_argument(arg))
        .cloned();
    Some(manifest_dependency(
        name.to_string(),
        "runtime",
        "cmake",
        version,
    ))
}

fn is_cmake_version_argument(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
}

fn is_cmake_find_package_option(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "REQUIRED"
            | "QUIET"
            | "MODULE"
            | "CONFIG"
            | "NO_MODULE"
            | "COMPONENTS"
            | "OPTIONAL_COMPONENTS"
            | "EXACT"
    )
}

fn go_mod_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut in_require_block = false;
    for raw_line in source.lines() {
        let dependency_kind = if raw_line.contains("// indirect") {
            "indirect"
        } else {
            "runtime"
        };
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
                dependency_kind,
                "go",
                version,
            ));
        }
    }
    dependencies
}

fn pubspec_dependencies(source: &str) -> Vec<ManifestDependency> {
    let mut dependencies = Vec::new();
    let mut active_section: Option<(String, usize)> = None;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if indent == 0 {
            active_section = None;
            if let Some(section) = yaml_key(trimmed).filter(|section| {
                matches!(
                    section.as_str(),
                    "dependencies" | "dev_dependencies" | "dependency_overrides"
                )
            }) {
                active_section = Some((section, indent));
            }
            continue;
        }

        let Some((section, section_indent)) = active_section.as_ref() else {
            continue;
        };
        if indent <= *section_indent {
            active_section = None;
            continue;
        }
        if indent != section_indent + 2 {
            continue;
        }
        let Some((name, value)) = yaml_key_pair(trimmed) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let kind = match section.as_str() {
            "dependencies" => "runtime",
            "dev_dependencies" => "dev",
            "dependency_overrides" => "override",
            _ => "runtime",
        };
        let version = (!value.is_empty()
            && !matches!(
                value.as_str(),
                "{}" | "[]" | "null" | "~" | "sdk" | "path" | "git"
            ))
        .then_some(value);
        dependencies.push(manifest_dependency(name, kind, "dart", version));
    }

    dependencies
}

fn pubspec_flutter_assets(source: &str) -> Vec<FlutterAsset> {
    let mut assets = Vec::new();
    let mut in_flutter: Option<usize> = None;
    let mut in_assets: Option<usize> = None;

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = yaml_indent(raw_line);

        if let Some(flutter_indent) = in_flutter
            && indent <= flutter_indent
            && yaml_key(trimmed).is_some()
        {
            in_flutter = None;
            in_assets = None;
        }
        if let Some(assets_indent) = in_assets
            && indent <= assets_indent
            && yaml_key(trimmed).is_some()
        {
            in_assets = None;
        }

        if indent == 0 && yaml_key(trimmed).is_some_and(|key| key == "flutter") {
            in_flutter = Some(indent);
            in_assets = None;
            continue;
        }
        if in_flutter.is_some() && yaml_key(trimmed).is_some_and(|key| key == "assets") {
            in_assets = Some(indent);
            continue;
        }
        if in_assets.is_some()
            && let Some(asset) = yaml_list_scalar(trimmed)
            && is_flutter_asset_path(&asset)
        {
            assets.push(FlutterAsset {
                path: asset,
                line: index as u32 + 1,
            });
        }
    }

    assets
}

fn is_flutter_asset_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('$')
        && !value.contains("://")
        && !Path::new(value).is_absolute()
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

fn pubspec_package_name(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            return None;
        }
        yaml_key_value(trimmed, "name").filter(|name| pubspec_package_name_valid(name))
    })
}

fn pubspec_package_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn pubspec_uses_flutter(source: &str) -> bool {
    source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "flutter:" || trimmed.starts_with("sdk: flutter")
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

fn dart_framework_configs(source: &str) -> Vec<FrameworkConfig> {
    let mut configs = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line_number = index as u32 + 1;
        let trimmed = line.trim();

        for needle in [
            "rootBundle.loadString(",
            "rootBundle.load(",
            "AssetImage(",
            "Image.asset(",
            "SvgPicture.asset(",
        ] {
            if let Some(value) = first_quoted_value_after(trimmed, needle)
                && is_flutter_asset_path(&value)
            {
                configs.push(framework_config(
                    "flutter",
                    format!("flutter asset read:{value}"),
                    "flutter_asset_read",
                    Some(value),
                    line_number,
                ));
            }
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
    let lines = source.lines().collect::<Vec<_>>();
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            let trimmed = line.trim();
            find_unquoted(trimmed, ".route(")?;
            let call = rust_route_call_window(&lines, index);
            let route_index = find_unquoted(&call, ".route(")?;
            let route_args = &call[route_index + ".route(".len()..];
            let path = first_quoted_value(route_args)?;
            let lower_args = route_args.to_ascii_lowercase();
            let method = route_methods()
                .iter()
                .find(|method| {
                    find_unquoted(&lower_args, &format!("{}(", method.to_ascii_lowercase()))
                        .is_some()
                })
                .copied()
                .unwrap_or("ROUTE")
                .to_string();
            let handler = handler_from_rust_route(route_args);
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
        if let Some(start) = find_unquoted(&lower, &needle) {
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

fn rust_route_call_window(lines: &[&str], start_index: usize) -> String {
    let mut call = String::new();
    for line in lines.iter().skip(start_index).take(12) {
        if !call.is_empty() {
            call.push(' ');
        }
        call.push_str(line.trim());
        if rust_route_call_closed(&call) {
            break;
        }
    }
    call
}

fn rust_route_call_closed(value: &str) -> bool {
    let Some(route_index) = find_unquoted(value, ".route(") else {
        return false;
    };
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0_i32;
    let mut started = false;

    for (_, character) in value[route_index..].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
            continue;
        }
        if character == '(' {
            depth += 1;
            started = true;
        } else if character == ')' && started {
            depth -= 1;
            if depth == 0 {
                return true;
            }
        }
    }

    false
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

fn find_unquoted(value: &str, needle: &str) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
            continue;
        }
        if value[index..].starts_with(needle) {
            return Some(index);
        }
    }

    None
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

fn setup_py_sequence_string_values(source: &str, key: &str) -> Vec<String> {
    let Some(value) = setup_py_keyword_value(source, key) else {
        return Vec::new();
    };
    extract_python_quoted_strings(&value)
}

fn setup_py_dict_list_string_values(source: &str, key: &str) -> Vec<String> {
    let Some(value) = setup_py_keyword_value(source, key) else {
        return Vec::new();
    };
    python_dict_list_values(&value)
        .into_iter()
        .flat_map(|value| extract_python_quoted_strings(&value))
        .collect()
}

fn setup_py_console_scripts(source: &str) -> Vec<String> {
    let Some(value) = setup_py_keyword_value(source, "entry_points") else {
        return Vec::new();
    };
    python_dict_list_values_for_key(&value, "console_scripts")
        .into_iter()
        .flat_map(|value| extract_python_quoted_strings(&value))
        .collect()
}

fn setup_cfg_sections(source: &str) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    let mut sections: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    let mut current_section: Option<String> = None;
    let mut current_key: Option<String> = None;

    for raw_line in source.lines() {
        let is_continuation = raw_line.starts_with([' ', '\t']);
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            let section = section.trim().to_ascii_lowercase();
            if section.is_empty() {
                current_section = None;
            } else {
                sections.entry(section.clone()).or_default();
                current_section = Some(section);
            }
            current_key = None;
            continue;
        }

        let Some(section) = current_section.as_deref() else {
            continue;
        };

        if is_continuation {
            let Some(key) = current_key.as_deref() else {
                continue;
            };
            let value = setup_cfg_clean_value(line);
            if !value.is_empty() {
                sections
                    .entry(section.to_string())
                    .or_default()
                    .entry(key.to_string())
                    .or_default()
                    .push(value);
            }
            continue;
        }

        let Some((key, value)) = line.split_once('=').or_else(|| line.split_once(':')) else {
            current_key = None;
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty() {
            current_key = None;
            continue;
        }
        sections
            .entry(section.to_string())
            .or_default()
            .entry(key.clone())
            .or_default();
        current_key = Some(key.clone());
        let value = setup_cfg_clean_value(value);
        if !value.is_empty() {
            sections
                .entry(section.to_string())
                .or_default()
                .entry(key)
                .or_default()
                .push(value);
        }
    }

    sections
}

fn setup_cfg_values(
    sections: &BTreeMap<String, BTreeMap<String, Vec<String>>>,
    section: &str,
    key: &str,
) -> Vec<String> {
    sections
        .get(&section.to_ascii_lowercase())
        .and_then(|values| values.get(&key.to_ascii_lowercase()))
        .cloned()
        .unwrap_or_default()
}

fn setup_cfg_clean_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn setup_py_keyword_value(source: &str, key: &str) -> Option<String> {
    let lower = source.to_ascii_lowercase();
    let key_lower = key.to_ascii_lowercase();
    let mut search_start = 0;
    while let Some(offset) = lower[search_start..].find(&key_lower) {
        let key_start = search_start + offset;
        let key_end = key_start + key.len();
        if !is_python_identifier_boundary(source, key_start, key_end) {
            search_start = key_end;
            continue;
        }
        let mut cursor = skip_ascii_whitespace(source, key_end);
        if source[cursor..].chars().next() != Some('=') {
            search_start = key_end;
            continue;
        }
        cursor = skip_ascii_whitespace(source, cursor + 1);
        return python_value_literal_at(source, cursor);
    }
    None
}

fn is_python_identifier_boundary(source: &str, start: usize, end: usize) -> bool {
    let before = source[..start].chars().next_back();
    let after = source[end..].chars().next();
    before.is_none_or(|character| !is_python_identifier_character(character))
        && after.is_none_or(|character| !is_python_identifier_character(character))
}

fn is_python_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn python_value_literal_at(source: &str, start: usize) -> Option<String> {
    let first = source[start..].chars().next()?;
    if matches!(first, '[' | '(' | '{') {
        return balanced_python_delimited_value(source, start);
    }
    if matches!(first, '"' | '\'') {
        let quoted = extract_python_quoted_string_at(source, start)?;
        return Some(quoted.raw);
    }
    let end = source[start..]
        .find([',', '\n'])
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    Some(source[start..end].trim().to_string()).filter(|value| !value.is_empty())
}

fn balanced_python_delimited_value(source: &str, start: usize) -> Option<String> {
    let open = source[start..].chars().next()?;
    let close = match open {
        '[' => ']',
        '(' => ')',
        '{' => '}',
        _ => return None,
    };
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in source[start..].char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if character == '\\' {
                escaped = true;
                continue;
            }
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'') {
            quote = Some(character);
            continue;
        }
        if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                let end = start + relative + character.len_utf8();
                return Some(source[start..end].to_string());
            }
        }
    }
    None
}

#[derive(Debug)]
struct PythonQuotedString {
    raw: String,
    value: String,
    end: usize,
}

fn extract_python_quoted_strings(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        if let Some(relative) = source[cursor..].find(['"', '\'']) {
            let start = cursor + relative;
            if let Some(quoted) = extract_python_quoted_string_at(source, start) {
                values.push(quoted.value);
                cursor = quoted.end;
                continue;
            }
            cursor = start + 1;
        } else {
            break;
        }
    }
    values
}

fn extract_python_quoted_string_at(source: &str, start: usize) -> Option<PythonQuotedString> {
    let quote = source[start..].chars().next()?;
    if !matches!(quote, '"' | '\'') {
        return None;
    }
    let mut escaped = false;
    let mut value = String::new();
    for (relative, character) in source[start + quote.len_utf8()..].char_indices() {
        if escaped {
            value.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == quote {
            let end = start + quote.len_utf8() + relative + character.len_utf8();
            return Some(PythonQuotedString {
                raw: source[start..end].to_string(),
                value,
                end,
            });
        }
        value.push(character);
    }
    None
}

fn python_dict_list_values(source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find(['[', '(']) else {
            break;
        };
        let start = cursor + relative;
        if let Some(value) = balanced_python_delimited_value(source, start) {
            cursor = start + value.len();
            values.push(value);
        } else {
            cursor = start + 1;
        }
    }
    values
}

fn python_dict_list_values_for_key(source: &str, wanted_key: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find(['"', '\'']) else {
            break;
        };
        let key_start = cursor + relative;
        let Some(key) = extract_python_quoted_string_at(source, key_start) else {
            cursor = key_start + 1;
            continue;
        };
        let mut after_key = skip_ascii_whitespace(source, key.end);
        if source[after_key..].chars().next() != Some(':') {
            cursor = key.end;
            continue;
        }
        after_key = skip_ascii_whitespace(source, after_key + 1);
        if key.value == wanted_key
            && let Some(value) = python_value_literal_at(source, after_key)
        {
            values.push(value);
        }
        cursor = after_key.saturating_add(1);
    }
    values
}

fn python_console_script_name_and_target(value: &str) -> Option<(String, String)> {
    let (name, target) = value.split_once('=')?;
    let name = name.trim();
    let target = target.trim();
    if name.is_empty() || target.is_empty() {
        None
    } else {
        Some((name.to_string(), target.to_string()))
    }
}

fn skip_ascii_whitespace(value: &str, mut cursor: usize) -> usize {
    while cursor < value.len()
        && value[cursor..]
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_whitespace())
    {
        cursor += value[cursor..].chars().next().unwrap().len_utf8();
    }
    cursor
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

fn dart_package_roots(
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Vec<DartPackageRoot> {
    let mut packages = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_enter(entry, root, options, ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("pubspec.yaml")
            || !is_probably_source_file(path, options.max_file_size)
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Some(name) = pubspec_package_name(&source) else {
            continue;
        };
        let dir = path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .map(|relative| normalize_path(&relative))
            .filter(|relative| !relative.is_empty());
        packages.push(DartPackageRoot { name, dir });
    }
    packages.sort_by(|left, right| right.name.len().cmp(&left.name.len()));
    packages
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

fn pipfile_dependency_version(value: &toml::Value) -> Option<String> {
    let version = direct_toml_dependency_version(value)?;
    let version = version.trim();
    if version.is_empty() || version == "*" {
        None
    } else {
        Some(version.to_string())
    }
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
        "cargo" | "npm" | "composer" | "vcpkg" | "conan" | "cmake" | "dart" => {
            trimmed.to_ascii_lowercase()
        }
        "go" => trimmed.to_string(),
        _ => trimmed.to_string(),
    }
}

fn local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    cmake_include_dirs: &[String],
    dart_packages: &[DartPackageRoot],
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
        Language::Dart => dart_local_import_target(source_label, import_label, dart_packages),
    }
}

fn possible_local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    go_modules: &[GoModuleRoot],
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    match language {
        Language::Python => python_absolute_local_import_target(source_label, import_label),
        Language::Go => go_module_import_target(import_label, go_modules),
        Language::Dart => dart_package_import_target(import_label, dart_packages),
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

fn dart_local_import_target(
    source_label: &str,
    import_label: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let uri = dart_import_uri(import_label)?;
    if uri.starts_with("./") || uri.starts_with("../") {
        return Some(LocalImportTarget {
            target: uri.clone(),
            candidates: vec![join_path(path_dir(source_label).as_deref(), &uri)],
        });
    }
    if uri.ends_with(".dart") && !uri.starts_with("package:") && !uri.contains("://") {
        return Some(LocalImportTarget {
            target: uri.clone(),
            candidates: vec![join_path(path_dir(source_label).as_deref(), &uri)],
        });
    }
    dart_package_uri_target(&uri, dart_packages)
}

fn dart_package_import_target(
    import_label: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let uri = dart_import_uri(import_label)?;
    dart_package_uri_target(&uri, dart_packages)
}

fn dart_package_uri_target(
    uri: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let rest = uri.strip_prefix("package:")?;
    let (package, path) = rest.split_once('/')?;
    if package.is_empty() || path.is_empty() {
        return None;
    }
    let package_root = dart_packages.iter().find(|root| root.name == package)?;
    let target = join_path(package_root.dir.as_deref(), &format!("lib/{path}"));
    Some(LocalImportTarget {
        target: uri.to_string(),
        candidates: vec![target],
    })
}

fn dart_import_uri(import_label: &str) -> Option<String> {
    first_quoted_value(import_label)
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
    manifest_dependency_with_version_kind(
        name,
        dependency_kind,
        ecosystem,
        version,
        Some("constraint"),
    )
}

fn manifest_dependency_with_version_kind(
    name: impl Into<String>,
    dependency_kind: impl Into<String>,
    ecosystem: impl Into<String>,
    version: Option<String>,
    version_kind: Option<&str>,
) -> ManifestDependency {
    let version_kind = if version.is_some() {
        version_kind.map(str::to_string)
    } else {
        None
    };
    ManifestDependency {
        name: name.into(),
        kind: dependency_kind.into(),
        ecosystem: ecosystem.into(),
        version,
        version_kind,
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

        let mut metadata = BTreeMap::new();
        metadata.insert("call_label".to_string(), call.label.clone());
        metadata.insert(
            "resolution".to_string(),
            if targets.len() > 1 {
                "ambiguous".to_string()
            } else {
                "resolved".to_string()
            },
        );
        metadata.insert("language".to_string(), call.language);

        for target in targets {
            add_edge_once_with_metadata(
                &mut context.graph,
                call.caller,
                target,
                EdgeKind::Calls,
                Confidence::Heuristic,
                metadata.clone(),
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

fn resolve_pending_compose_config_targets(context: &mut IndexContext) {
    let pending_targets = std::mem::take(&mut context.pending_compose_config_targets);

    for pending in pending_targets {
        let Some(path) = normalize_manifest_relative_path(&pending.manifest_label, &pending.target)
        else {
            continue;
        };
        let Some(file_id) = context.file_nodes.get(&path).copied() else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "config_file".to_string());
        metadata.insert(
            "resolution".to_string(),
            "compose_env_file_path".to_string(),
        );
        metadata.insert("source".to_string(), "compose".to_string());
        add_edge_once_with_metadata(
            &mut context.graph,
            pending.config,
            file_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

fn resolve_pending_compose_volume_targets(context: &mut IndexContext) {
    let pending_targets = std::mem::take(&mut context.pending_compose_volume_targets);

    for pending in pending_targets {
        let Some(path) = compose_volume_local_source_path(&pending.manifest_label, &pending.target)
        else {
            continue;
        };
        let target_id = context
            .file_nodes
            .get(&path)
            .copied()
            .or_else(|| context.directory_nodes.get(&path).copied());
        let Some(target_id) = target_id else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "volume_source".to_string());
        metadata.insert(
            "resolution".to_string(),
            "compose_volume_source_path".to_string(),
        );
        metadata.insert("source".to_string(), "compose".to_string());
        add_edge_once_with_metadata(
            &mut context.graph,
            pending.volume,
            target_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

fn resolve_pending_kubernetes_config_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_kubernetes_config_refs);

    for pending in pending_refs {
        let key = KubernetesConfigKey {
            namespace: pending.namespace,
            config_kind: pending.config_kind,
            name: pending.name,
        };
        let Some(config_id) = context.kubernetes_configs.get(&key).copied() else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "config_definition".to_string());
        metadata.insert(
            "resolution".to_string(),
            "kubernetes_config_ref".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        add_edge_once_with_metadata(
            &mut context.graph,
            pending.config_ref,
            config_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

fn resolve_pending_kubernetes_service_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_kubernetes_service_refs);

    for pending in pending_refs {
        let key = KubernetesServiceKey {
            namespace: pending.namespace,
            name: pending.name,
        };
        let Some(service_id) = context.kubernetes_services.get(&key).copied() else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), "service_definition".to_string());
        metadata.insert(
            "resolution".to_string(),
            "kubernetes_service_ref".to_string(),
        );
        metadata.insert("source".to_string(), "kubernetes".to_string());
        add_edge_once_with_metadata(
            &mut context.graph,
            pending.service_ref,
            service_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

fn resolve_pending_github_actions_local_actions(context: &mut IndexContext) {
    let pending_actions = std::mem::take(&mut context.pending_github_actions_local_actions);

    for pending in pending_actions {
        let Some(target_id) = github_actions_local_action_target(context, &pending.target) else {
            continue;
        };
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "relation".to_string(),
            "github_actions_local_action".to_string(),
        );
        metadata.insert(
            "resolution".to_string(),
            "github_actions_local_action_path".to_string(),
        );
        metadata.insert("source".to_string(), "github-actions".to_string());
        add_edge_once_with_metadata(
            &mut context.graph,
            pending.action,
            target_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

fn resolve_pending_document_path_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_document_path_refs);

    for pending in pending_refs {
        let target = pending.candidates.iter().find_map(|candidate| {
            context
                .file_nodes
                .get(candidate)
                .copied()
                .or_else(|| context.directory_nodes.get(candidate).copied())
                .map(|id| (candidate.clone(), id))
        });
        let Some((resolved_path, target_id)) = target else {
            continue;
        };

        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), pending.relation.to_string());
        metadata.insert("resolution".to_string(), "document_path".to_string());
        metadata.insert("source".to_string(), "markdown".to_string());
        metadata.insert("target".to_string(), pending.target);
        metadata.insert("resolved_path".to_string(), resolved_path);
        metadata.insert("line".to_string(), pending.line.to_string());
        if let Some(text) = pending.text {
            metadata.insert("text".to_string(), text);
        }
        add_edge_once_with_metadata(
            &mut context.graph,
            pending.source,
            target_id,
            EdgeKind::References,
            Confidence::Exact,
            metadata,
        );
    }
}

fn resolve_pending_document_symbol_refs(context: &mut IndexContext) {
    let pending_refs = std::mem::take(&mut context.pending_document_symbol_refs);

    for pending in pending_refs {
        let targets = resolve_function_targets(&context.function_symbols, &pending.symbol);
        if targets.is_empty() {
            continue;
        }

        let mut metadata = BTreeMap::new();
        metadata.insert("relation".to_string(), pending.relation.to_string());
        metadata.insert("resolution".to_string(), "document_symbol".to_string());
        metadata.insert("source".to_string(), "markdown".to_string());
        metadata.insert("symbol".to_string(), pending.symbol);
        metadata.insert("line".to_string(), pending.line.to_string());

        for target in targets {
            add_edge_once_with_metadata(
                &mut context.graph,
                pending.source,
                target,
                EdgeKind::References,
                Confidence::Heuristic,
                metadata.clone(),
            );
        }
    }
}

fn github_actions_local_action_target(context: &IndexContext, target: &str) -> Option<NodeId> {
    let candidates = [
        target.to_string(),
        format!("{target}/action.yml"),
        format!("{target}/action.yaml"),
        format!("{target}/Dockerfile"),
    ];
    for candidate in candidates {
        if let Some(id) = context.directory_nodes.get(&candidate).copied() {
            return Some(id);
        }
        if let Some(id) = context.file_nodes.get(&candidate).copied() {
            return Some(id);
        }
    }
    None
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
        "dart" | "flutter" => manifest_path_candidate(
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
        "make" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "make_command_path",
            })
            .into_iter()
            .collect(),
        "docker" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "docker_command_path",
            })
            .into_iter()
            .collect(),
        "compose" => command_path_candidate(pending)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "compose_command_path",
            })
            .into_iter()
            .collect(),
        "compose-dockerfile" => manifest_path_candidate(
            pending,
            &pending.target,
            None,
            Confidence::Exact,
            Confidence::Exact,
            "compose_dockerfile",
        )
        .into_iter()
        .collect(),
        "github-actions" => github_actions_run_command_path_candidate(&pending.target)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "github_actions_run_command_path",
            })
            .into_iter()
            .collect(),
        "gitlab-ci" => root_relative_command_path_candidate(&pending.target)
            .map(|path| EntrypointTargetCandidate {
                path,
                symbol: None,
                file_confidence: Confidence::Heuristic,
                function_confidence: Confidence::Heuristic,
                resolution: "gitlab_ci_script_command_path",
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
    normalized_command_path_candidate(&pending.manifest_label, &pending.target)
}

fn normalized_command_path_candidate(manifest_label: &str, command: &str) -> Option<String> {
    split_command_tokens(command)
        .into_iter()
        .filter(|token| is_command_path_candidate(token))
        .find_map(|path| normalize_manifest_relative_path(manifest_label, &path))
}

fn github_actions_run_command_path_candidate(command: &str) -> Option<String> {
    root_relative_command_path_candidate(command)
}

fn root_relative_command_path_candidate(command: &str) -> Option<String> {
    split_command_tokens(command)
        .into_iter()
        .filter(|token| is_command_path_candidate(token))
        .find_map(|path| normalize_relative_path(Path::new(&path)))
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
    } else if resolution.starts_with("make") {
        "makefile"
    } else if resolution.starts_with("docker") {
        "dockerfile"
    } else if resolution.starts_with("compose") {
        "compose"
    } else if resolution.starts_with("github_actions") {
        "github-actions"
    } else if resolution.starts_with("gitlab_ci") {
        "gitlab-ci"
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
    if !options.include_hidden && is_ci_infrastructure_path(entry.path(), root) {
        return entry_exclusion_without_hidden(entry, root, options, ignored_globs).is_none();
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

fn entry_exclusion_without_hidden(
    entry: &DirEntry,
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Option<EntryExclusion> {
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

fn is_ci_infrastructure_path(path: &Path, root: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let parts: Vec<_> = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    matches!(
        parts.as_slice(),
        [".github"]
            | [".github", "workflows", ..]
            | [".github", "actions", ..]
            | [".gitlab-ci.yml"]
            | [".gitlab-ci.yaml"]
            | [".gitlab"]
            | [".gitlab", "ci", ..]
    )
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
    if is_markdown_document(path) {
        return true;
    }

    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "package-lock.json"
                | "pnpm-lock.yaml"
                | "go.mod"
                | "pubspec.yaml"
                | "pyproject.toml"
                | "setup.py"
                | "setup.cfg"
                | "Pipfile"
                | "requirements.txt"
                | "composer.json"
                | "composer.lock"
                | "vcpkg.json"
                | "conanfile.txt"
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

        assert_eq!(ambiguous_edges.len(), 2);
        assert!(ambiguous_edges.iter().all(|edge| {
            edge.metadata.get("resolution").map(String::as_str) == Some("ambiguous")
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
        let setup_py_entrypoint =
            node_id(&graph, NodeKind::Entrypoint, "python console_script:legacy");
        let setup_cfg_entrypoint = node_id(
            &graph,
            NodeKind::Entrypoint,
            "python console_script:cfglegacy",
        );
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
            !graph.nodes.iter().any(|node| {
                node.kind == NodeKind::Entrypoint && node.label == "make target:%.o"
            })
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
            !graph.nodes.iter().any(|node| {
                node.kind == NodeKind::Entrypoint && node.label == "gitlab job:.base"
            })
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
