//! Shared indexing state: the in-progress graph, pending cross-file
//! resolution queues, and descriptor structs for manifest, CI, container,
//! and Kubernetes facts collected during the walk.

use std::collections::{BTreeMap, BTreeSet};

use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind, SourceSpan};
use codegraph_parser::{Language, ParsedFile};
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use crate::*;

/// What reading one file yielded: the language it was read as, and its
/// facts or the error that stopped them.
pub(crate) type ParsedSource = (
    codegraph_parser::Language,
    Result<codegraph_parser::ParsedFile, codegraph_parser::ParseError>,
);

pub(crate) struct IndexContext {
    pub(crate) graph: CodeGraph,
    /// Lazily synced `(source, target, kind)` keys of `graph.edges`, kept by
    /// `add_edge_once*` so edge dedup is a set probe instead of a linear scan
    /// per insert (O(E^2) over a whole scan). `edge_keys_synced` marks how
    /// many leading edges the set has absorbed; passes that push edges
    /// directly are caught up on the next `add_edge_once*` call.
    pub(crate) edge_keys: BTreeSet<(NodeId, NodeId, EdgeKind)>,
    pub(crate) edge_keys_synced: usize,
    pub(crate) function_symbols: BTreeMap<String, Vec<NodeId>>,
    /// Canonical node per (language, namespace label) for languages whose
    /// namespace declaration reopens one entity across many files.
    pub(crate) namespace_nodes: BTreeMap<(&'static str, String), NodeId>,
    /// What the repository's own `.gitignore` says it builds rather than
    /// keeps. An import of `release.h` finds nothing because `make`
    /// writes that file, and the project has written down which paths
    /// those are.
    pub(crate) build_products: Option<BuildProducts>,
    /// Canonical node per (`environment`|`config`, key) read from source, so a
    /// key that many files read stays one entity; the reading edge carries the
    /// per-read facts (default value, language, file, line).
    pub(crate) effect_entities: BTreeMap<(&'static str, String), NodeId>,
    pub(crate) type_symbols: BTreeMap<String, Vec<NodeId>>,
    pub(crate) file_nodes: BTreeMap<String, NodeId>,
    pub(crate) directory_nodes: BTreeMap<String, NodeId>,
    pub(crate) external_dependencies: BTreeMap<String, NodeId>,
    /// Per-file map of an import qualifier (`states`, `strings`) to the
    /// package it names, so a qualified call can be answered by the import
    /// list instead of by every same-named declaration in the project.
    pub(crate) file_import_qualifiers: BTreeMap<String, BTreeMap<String, ImportedPackage>>,
    /// Per file, the bare names an import binds and where they come from —
    /// what `from flask import Blueprint` tells a later `Blueprint()` that
    /// matching by name alone cannot.
    pub(crate) file_imported_names: BTreeMap<String, BTreeMap<String, ImportedPackage>>,
    /// One placeholder node per (language, label) for unresolved call targets.
    pub(crate) unresolved_call_placeholders: BTreeMap<(String, String), NodeId>,
    pub(crate) cargo_workspace_dependencies: BTreeMap<String, Option<String>>,
    pub(crate) go_modules: Vec<GoModuleRoot>,
    pub(crate) dart_packages: Vec<DartPackageRoot>,
    pub(crate) npm_packages: Vec<NpmPackageRoot>,
    /// Package names the project's own manifests claim. Nothing declares
    /// a dependency on itself.
    pub(crate) own_package_ids: BTreeSet<String>,
    pub(crate) c_include_dirs: Vec<String>,
    /// The names a Julia package exports and the names an R package's
    /// NAMESPACE lists. Both are written in one place for the whole
    /// package, away from the files that define them.
    pub(crate) julia_exports: BTreeSet<String>,
    pub(crate) r_exports: BTreeSet<String>,
    pub(crate) custom_rules: CustomRules,
    pub(crate) annotations: GraphAnnotations,
    pub(crate) pending_calls: Vec<PendingCall>,
    pub(crate) pending_type_references: Vec<PendingTypeReference>,
    pub(crate) pending_local_imports: Vec<PendingLocalImport>,
    /// `using Polly.Telemetry;` names a namespace the project may declare
    /// somewhere else entirely, so it can only be answered once every file
    /// has been read.
    pub(crate) pending_namespace_imports: Vec<PendingNamespaceImport>,
    pub(crate) pending_entrypoint_targets: Vec<PendingEntrypointTarget>,
    pub(crate) pending_route_handlers: Vec<PendingRouteHandler>,
    pub(crate) pending_compose_config_targets: Vec<PendingComposeConfigTarget>,
    pub(crate) pending_compose_volume_targets: Vec<PendingComposeVolumeTarget>,
    pub(crate) kubernetes_configs: BTreeMap<KubernetesConfigKey, NodeId>,
    pub(crate) kubernetes_services: BTreeMap<KubernetesServiceKey, NodeId>,
    pub(crate) pending_kubernetes_config_refs: Vec<PendingKubernetesConfigRef>,
    pub(crate) pending_kubernetes_service_refs: Vec<PendingKubernetesServiceRef>,
    pub(crate) pending_github_actions_local_actions: Vec<PendingGithubActionsLocalAction>,
    pub(crate) pending_document_path_refs: Vec<PendingDocumentPathRef>,
    pub(crate) pending_document_symbol_refs: Vec<PendingDocumentSymbolRef>,
    pub(crate) sql_tables: BTreeMap<String, NodeId>,
    pub(crate) sql_columns: BTreeMap<String, NodeId>,
    pub(crate) pending_sql_foreign_keys: Vec<PendingSqlForeignKey>,
    pub(crate) pending_sql_query_table_refs: Vec<PendingSqlQueryTableRef>,
    pub(crate) pending_native_channel_handlers: Vec<PendingNativeChannelHandler>,
    pub(crate) pending_sql_joins: Vec<PendingSqlJoin>,
    pub(crate) pending_sql_alter_refs: Vec<PendingSqlAlterRef>,
    pub(crate) sql_migrations: Vec<SqlMigrationFile>,
    pub(crate) pending_orm_table_refs: Vec<PendingOrmTableRef>,
    pub(crate) pending_migration_dir_refs: Vec<PendingMigrationDirRef>,
    pub(crate) sql_migration_dirs: BTreeMap<String, Vec<NodeId>>,
    pub(crate) pending_mcp_local_refs: Vec<PendingMcpLocalRef>,
    /// Files read into facts ahead of the walk, on every core. Emptied as
    /// the walk reaches each one.
    pub(crate) parsed_ahead: BTreeMap<String, ParsedSource>,
}

/// A path-like MCP server command/argument waiting to be matched against
/// scanned files.
pub(crate) struct PendingMcpLocalRef {
    pub(crate) server: NodeId,
    pub(crate) candidate: String,
}

/// An ORM table mapping found in application code, waiting for the table
/// node to exist.
pub(crate) struct PendingOrmTableRef {
    pub(crate) file: NodeId,
    pub(crate) table: String,
    pub(crate) pattern: &'static str,
    pub(crate) line: u32,
}

/// A migrations-directory reference from code or database config, waiting to
/// be matched against scanned migration files.
pub(crate) struct PendingMigrationDirRef {
    pub(crate) file: NodeId,
    pub(crate) dir: String,
    pub(crate) source_kind: &'static str,
    pub(crate) line: u32,
}

/// A JOIN between two tables found in a SQL statement, waiting for both
/// table nodes to exist.
pub(crate) struct PendingSqlJoin {
    pub(crate) left: String,
    pub(crate) right: String,
    pub(crate) condition: Option<String>,
    pub(crate) line: u32,
}

/// An ALTER/DROP TABLE statement waiting to be matched to an indexed table.
pub(crate) struct PendingSqlAlterRef {
    pub(crate) file: NodeId,
    pub(crate) table: String,
    pub(crate) operation: &'static str,
    pub(crate) line: u32,
}

/// A migration file with its ordering key, for migration_order edges.
pub(crate) struct SqlMigrationFile {
    pub(crate) file: NodeId,
    pub(crate) label: String,
    pub(crate) dir: String,
    pub(crate) sequence: u128,
    pub(crate) sequence_text: String,
}

/// A Flutter platform-channel registration found in native Android/iOS
/// source, waiting to be matched against Dart channel declarations.
pub(crate) struct PendingNativeChannelHandler {
    pub(crate) file: NodeId,
    pub(crate) label: String,
    pub(crate) name: String,
    pub(crate) channel_kind: String,
    pub(crate) platform: &'static str,
    pub(crate) line: u32,
}

pub(crate) struct PendingCall {
    pub(crate) caller: NodeId,
    pub(crate) label: String,
    pub(crate) span: SourceSpan,
    pub(crate) language: String,
    /// The declared type of the call's receiver, when the enclosing
    /// signature names it (`func (b *Backend)` makes `b.Configure()` a call on
    /// `Backend`). Lets resolution pick the method of that type instead of
    /// every method sharing the name.
    pub(crate) receiver_type: Option<String>,
    /// What the call names as its receiver, when the source writes one:
    /// `[NSURL URLWithString:url]` messages a class Foundation provides,
    /// and the selector alone cannot say so.
    pub(crate) receiver: Option<String>,
    /// The call goes through a value the body binds, not to a definition.
    pub(crate) callee_is_value: bool,
}

pub(crate) struct PendingTypeReference {
    pub(crate) source: NodeId,
    pub(crate) label: String,
    pub(crate) language: String,
    pub(crate) span: SourceSpan,
}

pub(crate) struct PendingNamespaceImport {
    pub(crate) import_node: NodeId,
    pub(crate) language: &'static str,
    pub(crate) namespace: String,
}

pub(crate) struct PendingLocalImport {
    pub(crate) import_node: NodeId,
    pub(crate) target: String,
    pub(crate) candidates: Vec<String>,
    pub(crate) mark_unresolved: bool,
    /// Whether a file whose path merely ends with the candidate may answer.
    /// A C include is searched for along a path the compiler is told about,
    /// so the suffix is evidence; `crate::ser` names a module of this crate
    /// and nothing else, so a file of that name in a sibling crate is not.
    pub(crate) allow_suffix_fallback: bool,
}

pub(crate) struct PendingRouteHandler {
    pub(crate) entrypoint: NodeId,
    pub(crate) handler: String,
}

pub(crate) struct PendingEntrypointTarget {
    pub(crate) entrypoint: NodeId,
    pub(crate) manifest_label: String,
    pub(crate) target: String,
    /// The directory the command runs in, when the file says so.
    pub(crate) base_dir: Option<String>,
    pub(crate) ecosystem: String,
    pub(crate) entrypoint_kind: String,
}

pub(crate) struct PendingComposeConfigTarget {
    pub(crate) config: NodeId,
    pub(crate) manifest_label: String,
    pub(crate) target: String,
}

pub(crate) struct PendingComposeVolumeTarget {
    pub(crate) volume: NodeId,
    pub(crate) manifest_label: String,
    pub(crate) target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct KubernetesConfigKey {
    pub(crate) namespace: String,
    pub(crate) config_kind: String,
    pub(crate) name: String,
}

pub(crate) struct PendingKubernetesConfigRef {
    pub(crate) config_ref: NodeId,
    pub(crate) namespace: String,
    pub(crate) config_kind: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct KubernetesServiceKey {
    pub(crate) namespace: String,
    pub(crate) name: String,
}

pub(crate) struct PendingKubernetesServiceRef {
    pub(crate) service_ref: NodeId,
    pub(crate) namespace: String,
    pub(crate) name: String,
}

pub(crate) struct PendingGithubActionsLocalAction {
    pub(crate) action: NodeId,
    pub(crate) target: String,
}

pub(crate) struct PendingDocumentPathRef {
    pub(crate) source: NodeId,
    pub(crate) target: String,
    pub(crate) candidates: Vec<String>,
    pub(crate) relation: &'static str,
    pub(crate) line: u32,
    pub(crate) text: Option<String>,
    /// `#L42` / `#L42-L50` citation anchor from the link fragment.
    pub(crate) line_ref: Option<String>,
}

pub(crate) struct PendingDocumentSymbolRef {
    pub(crate) source: NodeId,
    pub(crate) symbol: String,
    pub(crate) relation: &'static str,
    pub(crate) line: u32,
}

pub(crate) struct PendingSqlForeignKey {
    pub(crate) source: NodeId,
    pub(crate) source_table: String,
    pub(crate) source_column: Option<String>,
    pub(crate) target_table: String,
    pub(crate) target_column: Option<String>,
    pub(crate) line: u32,
}

pub(crate) struct PendingSqlQueryTableRef {
    pub(crate) query: NodeId,
    pub(crate) table: String,
    pub(crate) operation: String,
    pub(crate) role: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MakefileTarget {
    pub(crate) name: String,
    pub(crate) command: Option<String>,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DockerfileEntrypoint {
    pub(crate) instruction: String,
    pub(crate) command: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposeService {
    pub(crate) name: String,
    pub(crate) command: Option<String>,
    pub(crate) command_kind: Option<String>,
    pub(crate) build_context: Option<String>,
    pub(crate) dockerfile: Option<String>,
    pub(crate) depends_on: Vec<String>,
    pub(crate) environment: Vec<ComposeEnvironment>,
    pub(crate) env_files: Vec<ComposeEnvFile>,
    pub(crate) ports: Vec<ComposePort>,
    pub(crate) volumes: Vec<ComposeVolume>,
    pub(crate) line: u32,
    /// The last line of the service's block, so the node covers the ports
    /// and volumes written under it.
    pub(crate) end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposeEnvironment {
    pub(crate) name: String,
    pub(crate) value_present: bool,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposeEnvFile {
    pub(crate) path: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposePort {
    pub(crate) published: Option<String>,
    pub(crate) target: Option<String>,
    pub(crate) protocol: String,
    pub(crate) host_ip: Option<String>,
    pub(crate) raw: Option<String>,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposeVolume {
    pub(crate) source: Option<String>,
    pub(crate) target: Option<String>,
    pub(crate) kind: String,
    pub(crate) read_only: bool,
    pub(crate) raw: Option<String>,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlutterAsset {
    pub(crate) path: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DartPlatformChannel {
    pub(crate) name: String,
    pub(crate) channel_kind: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubActionsWorkflow {
    pub(crate) name: String,
    pub(crate) environment: Vec<CiEnvironment>,
    pub(crate) jobs: Vec<GithubActionsJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubActionsJob {
    pub(crate) id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) runs_on: Option<String>,
    pub(crate) needs: Vec<String>,
    pub(crate) environment: Vec<CiEnvironment>,
    pub(crate) steps: Vec<GithubActionsStep>,
    pub(crate) line: u32,
    /// The last line the job's block claims, so the job spans its steps
    /// rather than only the line its name is written on.
    pub(crate) end_line: u32,
    /// `defaults: run: working-directory:` moves every step in the job.
    pub(crate) working_directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GithubActionsStep {
    pub(crate) name: Option<String>,
    pub(crate) uses: Option<String>,
    pub(crate) run: Option<String>,
    /// `working-directory: pkgs/http` moves what the step's command paths
    /// are relative to, the way a leading `cd` does inside one.
    pub(crate) working_directory: Option<String>,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitlabCiJob {
    pub(crate) name: String,
    pub(crate) stage: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) extends: Vec<String>,
    pub(crate) needs: Vec<String>,
    pub(crate) dependencies: Vec<String>,
    pub(crate) variables: Vec<CiEnvironment>,
    pub(crate) scripts: Vec<GitlabCiScript>,
    pub(crate) line: u32,
    /// The last line of the job's block, so the node covers its scripts.
    pub(crate) end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitlabCiScript {
    pub(crate) command: String,
    pub(crate) script_kind: String,
    pub(crate) ordinal: usize,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CiEnvironment {
    pub(crate) name: String,
    pub(crate) value_present: bool,
    pub(crate) value_kind: String,
    pub(crate) scope: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KubernetesDocument {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) namespace: String,
    pub(crate) line: u32,
    /// The last line of the document, which is a whole resource.
    pub(crate) end_line: u32,
    pub(crate) labels: BTreeMap<String, String>,
    pub(crate) pod_labels: BTreeMap<String, String>,
    pub(crate) selector_labels: BTreeMap<String, String>,
    pub(crate) config_refs: Vec<KubernetesConfigRef>,
    pub(crate) service_ports: Vec<KubernetesServicePort>,
    pub(crate) ingress_backends: Vec<KubernetesIngressBackend>,
    pub(crate) container_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KubernetesConfigRef {
    pub(crate) config_kind: String,
    pub(crate) ref_kind: String,
    pub(crate) name: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KubernetesServicePort {
    pub(crate) name: Option<String>,
    pub(crate) port: Option<String>,
    pub(crate) target_port: Option<String>,
    pub(crate) protocol: String,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KubernetesIngressBackend {
    pub(crate) service_name: String,
    pub(crate) service_port: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) path_type: Option<String>,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KubernetesLabelTarget {
    Metadata,
    PodTemplate,
    Selector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileStamp {
    pub(crate) len: u64,
    pub(crate) modified_ns: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ParseCacheRecord {
    pub(crate) cache_schema_version: u32,
    /// Which build extracted these facts; see [`build_identity`].
    #[serde(default)]
    pub(crate) build_identity: String,
    pub(crate) language: Language,
    pub(crate) stamp: FileStamp,
    pub(crate) parsed: ParsedFile,
}

pub(crate) struct EntrypointTargetCandidate {
    pub(crate) path: String,
    pub(crate) symbol: Option<String>,
    pub(crate) file_confidence: Confidence,
    pub(crate) function_confidence: Confidence,
    pub(crate) resolution: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestDependency {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) ecosystem: String,
    pub(crate) version: Option<String>,
    pub(crate) version_kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestEntrypoint {
    pub(crate) label: String,
    pub(crate) kind: String,
    pub(crate) ecosystem: String,
    pub(crate) target: Option<String>,
    /// The line the extractor read the entry on, when it read text rather
    /// than a parsed document. Nothing else can find it as reliably.
    pub(crate) line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameworkRoute {
    pub(crate) framework: String,
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) handler: Option<String>,
    /// What the handler was written under, when it was written qualified:
    /// django-oscar's sandbox routes name `views.index`, and `views` is
    /// `django.contrib.sitemaps.views` rather than anything the project
    /// declares.
    pub(crate) handler_qualifier: Option<String>,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FrameworkConfig {
    pub(crate) framework: String,
    pub(crate) label: String,
    pub(crate) config_kind: String,
    pub(crate) value: Option<String>,
    pub(crate) line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoModuleRoot {
    pub(crate) module: String,
    pub(crate) dir: Option<String>,
}

/// A package.json inside the repository and where it sits. A workspace
/// import like `@vue/shared` names one of these, not a dependency that
/// left the repository — without the distinction every monorepo package
/// would be reported as external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NpmPackageRoot {
    pub(crate) name: String,
    pub(crate) dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DartPackageRoot {
    pub(crate) name: String,
    pub(crate) dir: Option<String>,
    /// Package-URI directory inside the package root (usually `lib`).
    pub(crate) lib_dir: String,
}

/// Where an import qualifier points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImportedPackage {
    /// A module inside the repository: a Go package directory (recorded with
    /// a trailing slash) or the file candidates a Python module can live in
    /// (`pkg/views.py`, `pkg/views/__init__.py`).
    Local(Vec<String>),
    /// A package outside the repository: no local declaration can be the
    /// target of a call through this qualifier.
    External,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalImportTarget {
    pub(crate) target: String,
    pub(crate) candidates: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CustomRules {
    pub(crate) forbidden_dependencies: Vec<ForbiddenDependencyRule>,
    pub(crate) forbidden_edges: Vec<ForbiddenEdgeRule>,
    pub(crate) required_configs: Vec<RequiredConfigRule>,
    pub(crate) parse_errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForbiddenDependencyRule {
    pub(crate) id: String,
    pub(crate) package: String,
    pub(crate) ecosystem: Option<String>,
    pub(crate) severity: String,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequiredConfigRule {
    pub(crate) id: String,
    pub(crate) target: String,
    pub(crate) severity: String,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForbiddenEdgeRule {
    pub(crate) id: String,
    pub(crate) edge_kind: Option<String>,
    pub(crate) source_kind: Option<String>,
    pub(crate) source_label: Option<String>,
    pub(crate) source_metadata: BTreeMap<String, String>,
    pub(crate) target_kind: Option<String>,
    pub(crate) target_label: Option<String>,
    pub(crate) target_metadata: BTreeMap<String, String>,
    pub(crate) severity: String,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GraphAnnotations {
    pub(crate) node_annotations: Vec<NodeAnnotationRule>,
    pub(crate) parse_errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeAnnotationRule {
    pub(crate) id: String,
    pub(crate) kind: Option<String>,
    pub(crate) label: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) item_kind: Option<String>,
    pub(crate) metadata: BTreeMap<String, String>,
    pub(crate) set: BTreeMap<String, String>,
}

/// The one node an environment variable or config key gets, however many
/// places name it. A workflow that sets `DATABASE_URL` and the function
/// that reads it mean the same variable, so they must meet on one node --
/// each site's own facts (its value, its line, the job that sets it)
/// travel on the edge.
pub(crate) fn shared_effect_entity(
    context: &mut IndexContext,
    entity_kind: &'static str,
    node_kind: NodeKind,
    label: &str,
    span: SourceSpan,
    metadata: BTreeMap<String, String>,
) -> NodeId {
    let key = (entity_kind, label.to_string());
    if let Some(existing) = context.effect_entities.get(&key) {
        return *existing;
    }
    let mut entity_metadata = metadata;
    entity_metadata.insert("declaration_scope".to_string(), "shared".to_string());
    let id = context.graph.add_node_with_metadata(
        node_kind,
        label.to_string(),
        Some(span),
        entity_metadata,
    );
    context.effect_entities.insert(key, id);
    id
}
