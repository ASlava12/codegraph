use anyhow::{Context, Result};
use async_stream::stream;
use axum::extract::{DefaultBodyLimit, Path as AxumPath, Query, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::Parser;
use codegraph_analysis::{
    CheckReport, ConfigTraceRequest, ConfigTraceResult, DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT,
    DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT, DEFAULT_REPORT_HOTSPOT_LIMIT,
    DEFAULT_REPORT_INSIGHT_LIMIT, DEFAULT_REPORT_LANGUAGE_LINK_LIMIT, EntrypointTraceReport,
    EntrypointTraceRequest, ErrorTraceRequest, ErrorTraceResult, ExplainEdgeRequest, FocusRequest,
    GraphSlice, GraphSliceRequest, GraphSummary, InsightFilter, InsightReport, InsightSeverity,
    KNOWN_INSIGHT_KINDS, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT,
    MAX_REPORT_HOTSPOT_LIMIT, MAX_REPORT_INSIGHT_LIMIT, MAX_REPORT_LANGUAGE_LINK_LIMIT, NodeCard,
    NodeContext, ProjectReport, ProjectReportLimits, SourcePreview, SourceSearchRequest,
    SourceSearchResult, TraceRequest, TraceStart, architecture_map, check_insights, entrypoints,
    explain_edge, export_dot, export_ndjson, filter_insight_report, focus_subgraph, hotspots,
    insights, language_dependencies, node_card, node_context, project_report, query_graph,
    read_source_preview, search_source, slice_graph, summarize, trace, trace_config,
    trace_dependents, trace_entrypoints, trace_errors,
};
use codegraph_core::{CODEGRAPH_SCHEMA_VERSION, CodeGraph};
use codegraph_indexer::{
    IndexOptionOverrides, IndexOptions, configured_index_options, scan_coverage,
};
use codegraph_lsp::{
    DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS, DEFAULT_SEMANTIC_WORK_ITEM_LIMIT, LspDiscoveryReport,
    MAX_SEMANTIC_REQUEST_TIMEOUT_MS, MAX_SEMANTIC_WORK_ITEM_LIMIT, SemanticEnrichmentPlan,
    SemanticGraphApplyReport, SemanticGraphApplyResult, SemanticGraphPatch, SemanticLspCache,
    SemanticLspCacheInfo, SemanticLspResponse, SemanticLspRunOptions, SemanticReadinessReport,
    SemanticWorkItemFilter, apply_semantic_graph_patch, discover_lsp_servers,
    normalize_semantic_request_timeout_ms, run_semantic_execution_batch_cached,
    semantic_enrichment_plan_with_filter, semantic_execution_batch,
    semantic_graph_patch_from_responses, semantic_readiness,
};
use codegraph_parser::language_adapters;
use codegraph_storage::{
    CacheInfo, CacheStatus, GraphCache, default_cache_dir, scan_project_cached,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::sleep;

const DEFAULT_MAX_SCAN_JOBS: usize = 64;
const DEFAULT_MAX_SEMANTIC_JOBS: usize = 64;
const DEFAULT_MAX_SCAN_CONCURRENCY: usize = 2;
const DEFAULT_MAX_SEMANTIC_CONCURRENCY: usize = 1;
const DEFAULT_JOB_LIST_LIMIT: usize = 50;
const MAX_JOB_LIST_LIMIT: usize = 500;
const DEFAULT_INCREMENTAL_REPORT_LIMIT: usize = 100;
const MAX_INCREMENTAL_REPORT_LIMIT: usize = 10_000;
const DEFAULT_GRAPH_NODE_LIMIT: usize = 250;
const MAX_GRAPH_NODE_LIMIT: usize = 1000;
const DEFAULT_GRAPH_EDGE_LIMIT: usize = 500;
const MAX_GRAPH_EDGE_LIMIT: usize = 2000;
const DEFAULT_NODE_CONTEXT_EDGE_LIMIT: usize = 80;
const MAX_NODE_CONTEXT_EDGE_LIMIT: usize = 500;
const DEFAULT_NODE_CARD_SOURCE_CONTEXT: u32 = 5;
const MAX_NODE_CARD_SOURCE_CONTEXT: u32 = 40;
const DEFAULT_NODE_CARD_INSIGHT_LIMIT: usize = 8;
const MAX_NODE_CARD_INSIGHT_LIMIT: usize = 500;
const DEFAULT_FOCUS_EDGE_LIMIT: usize = 200;
const MAX_FOCUS_EDGE_LIMIT: usize = 1000;
const DEFAULT_GRAPH_QUERY_LIMIT: usize = 100;
const MAX_GRAPH_QUERY_LIMIT: usize = 1000;
const MAX_GRAPH_QUERY_LENGTH: usize = 4096;
const DEFAULT_INSIGHT_LIMIT: usize = 50;
const MAX_INSIGHT_LIMIT: usize = 500;
const DEFAULT_SOURCE_CONTEXT: u32 = 4;
const MAX_SOURCE_CONTEXT: u32 = 40;
const DEFAULT_SOURCE_SEARCH_LIMIT: usize = 50;
const MAX_SOURCE_SEARCH_LIMIT: usize = 1000;
const MAX_SOURCE_SEARCH_QUERY_LENGTH: usize = 4096;
const DEFAULT_SOURCE_SEARCH_CONTEXT: usize = 2;
const MAX_SOURCE_SEARCH_CONTEXT: usize = 20;
const DEFAULT_API_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_API_BODY_BYTES: usize = 256 * 1024 * 1024;
const EXPORT_NODES_HEADER: &str = "x-codegraph-export-nodes";
const EXPORT_EDGES_HEADER: &str = "x-codegraph-export-edges";
const EXPORT_BYTES_HEADER: &str = "x-codegraph-export-bytes";
const RESPONSE_TIME_HEADER: &str = "x-response-time-ms";
const STATIC_ASSET_CACHE_CONTROL: &str = "no-cache";
const DYNAMIC_CACHE_CONTROL: &str = "no-store";
const APP_JS: &str = include_str!("../../codegraph-web/static/app.js");
const INDEX_HTML: &str = include_str!("../../codegraph-web/static/index.html");
const LABEL_POLICY_JS: &str = include_str!("../../codegraph-web/static/label-policy.js");
const STYLES_CSS: &str = include_str!("../../codegraph-web/static/styles.css");
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

tokio::task_local! {
    static CURRENT_REQUEST_ID: String;
}

#[derive(Debug, Parser)]
#[command(name = "codegraph-server")]
#[command(about = "Serve the CodeGraph API and web interface")]
#[command(version)]
struct Args {
    /// Project root exposed to the scanner.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Additional local project roots that may be opened from the web UI.
    #[arg(long = "project")]
    projects: Vec<PathBuf>,

    /// HTTP bind host.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// HTTP bind port.
    #[arg(long, default_value_t = 3765)]
    port: u16,

    /// Include hidden files and directories.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,

    /// Maximum bytes to read from any single file during scans.
    #[arg(long)]
    max_file_size: Option<u64>,

    /// Allow scanning paths outside the configured root.
    #[arg(long)]
    allow_any_path: bool,

    /// Disable persistent graph cache.
    #[arg(long)]
    no_cache: bool,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    cache_dir: Option<PathBuf>,

    /// Maximum in-memory scan jobs retained after completion.
    #[arg(long, default_value_t = DEFAULT_MAX_SCAN_JOBS)]
    max_scan_jobs: usize,

    /// Maximum in-memory semantic enrichment jobs retained after completion.
    #[arg(long, default_value_t = DEFAULT_MAX_SEMANTIC_JOBS)]
    max_semantic_jobs: usize,

    /// Maximum scan jobs allowed to run at the same time.
    #[arg(long, default_value_t = DEFAULT_MAX_SCAN_CONCURRENCY)]
    max_scan_concurrency: usize,

    /// Maximum semantic enrichment jobs allowed to run at the same time.
    #[arg(long, default_value_t = DEFAULT_MAX_SEMANTIC_CONCURRENCY)]
    max_semantic_concurrency: usize,

    /// Maximum accepted HTTP request body bytes for JSON API requests.
    #[arg(long, default_value_t = DEFAULT_API_BODY_BYTES)]
    max_api_body_bytes: usize,

    /// Disable per-request access logs on stderr.
    #[arg(long)]
    quiet_access_log: bool,

    /// Require a token for all /api/* routes. Also read from CODEGRAPH_API_TOKEN.
    #[arg(long)]
    api_token: Option<String>,
}

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    started_at: Instant,
    projects: Arc<Vec<ProjectRoot>>,
    option_overrides: IndexOptionOverrides,
    allow_any_path: bool,
    cache: Option<GraphCache>,
    jobs: Arc<RwLock<BTreeMap<String, ScanJob>>>,
    semantic_jobs: Arc<RwLock<BTreeMap<String, SemanticJob>>>,
    max_scan_jobs: usize,
    max_semantic_jobs: usize,
    max_scan_concurrency: usize,
    max_semantic_concurrency: usize,
    max_api_body_bytes: usize,
    access_log_enabled: bool,
    scan_permits: Arc<Semaphore>,
    semantic_permits: Arc<Semaphore>,
    next_job_id: Arc<AtomicU64>,
}

#[derive(Clone)]
struct ApiAuth {
    token: Option<Arc<str>>,
}

impl ApiAuth {
    fn new(token: Option<String>) -> Self {
        Self {
            token: token.map(Arc::<str>::from),
        }
    }

    fn enabled(&self) -> bool {
        self.token.is_some()
    }
}

#[derive(Debug, Deserialize)]
struct ScanQuery {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct SemanticPlanQuery {
    path: Option<PathBuf>,
    work_item_limit: Option<usize>,
    work_language: Option<String>,
    work_status: Option<String>,
    work_capability: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SemanticPatchRequest {
    path: Option<PathBuf>,
    work_item_limit: Option<usize>,
    work_language: Option<String>,
    work_status: Option<String>,
    work_capability: Option<String>,
    responses: Vec<SemanticLspResponse>,
}

#[derive(Debug, Clone, Deserialize)]
struct SemanticEnrichRequest {
    path: Option<PathBuf>,
    work_item_limit: Option<usize>,
    work_language: Option<String>,
    work_status: Option<String>,
    work_capability: Option<String>,
    request_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct SemanticEnrichResponse {
    graph: CodeGraph,
    summary: GraphSummary,
    report: SemanticGraphApplyReport,
    responses: usize,
    response_errors: usize,
    unmatched_locations: usize,
    semantic_cache: SemanticLspCacheInfo,
}

#[derive(Debug, Clone, Serialize)]
struct SemanticJob {
    id: String,
    status: ScanJobStatus,
    path: String,
    message: String,
    created_at_unix: u64,
    updated_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at_unix: Option<u64>,
    responses: Option<usize>,
    response_errors: Option<usize>,
    unmatched_locations: Option<usize>,
    report: Option<SemanticGraphApplyReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<SemanticEnrichResponse>,
}

#[derive(Debug, Serialize)]
struct SemanticJobResult {
    id: String,
    root: String,
    result: SemanticEnrichResponse,
}

#[derive(Debug, Serialize)]
struct SemanticJobListResponse {
    jobs: Vec<SemanticJob>,
    total: usize,
    returned: usize,
    limit: usize,
    status: Option<ScanJobStatus>,
    summary: JobStoreHealth,
}

#[derive(Debug, Deserialize)]
struct ScanOptionsQuery {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct ArchitectureQuery {
    path: Option<PathBuf>,
    group_limit: Option<usize>,
    edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct LanguageDependencyQuery {
    path: Option<PathBuf>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct HotspotQuery {
    path: Option<PathBuf>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CacheDiffQuery {
    path: Option<PathBuf>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SourceQuery {
    root: Option<PathBuf>,
    path: PathBuf,
    start_line: Option<u32>,
    end_line: Option<u32>,
    context: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct SourceSearchQuery {
    path: Option<PathBuf>,
    q: String,
    path_filter: Option<String>,
    case_sensitive: Option<bool>,
    limit: Option<usize>,
    context: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct TraceQuery {
    path: Option<PathBuf>,
    label: Option<String>,
    node_id: Option<u64>,
    depth: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct EntrypointTraceQuery {
    path: Option<PathBuf>,
    search: Option<String>,
    depth: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ConfigTraceQuery {
    path: Option<PathBuf>,
    target: String,
    depth: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ErrorTraceQuery {
    path: Option<PathBuf>,
    target: String,
    depth: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GraphQuery {
    path: Option<PathBuf>,
    q: String,
}

#[derive(Debug, Deserialize)]
struct ExplainEdgeQuery {
    path: Option<PathBuf>,
    edge_index: Option<usize>,
    source: Option<String>,
    target: Option<String>,
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InsightQuery {
    path: Option<PathBuf>,
    severity: Option<String>,
    kind: Option<String>,
    search: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CheckQuery {
    path: Option<PathBuf>,
    fail_on: Option<String>,
    kind: Option<String>,
    search: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ProjectReportQuery {
    path: Option<PathBuf>,
    architecture_group_limit: Option<usize>,
    architecture_edge_limit: Option<usize>,
    language_link_limit: Option<usize>,
    hotspot_limit: Option<usize>,
    insight_limit: Option<usize>,
    fail_on: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphSliceQuery {
    path: Option<PathBuf>,
    node_offset: Option<usize>,
    node_limit: Option<usize>,
    edge_offset: Option<usize>,
    edge_limit: Option<usize>,
    path_prefix: Option<String>,
    kind: Option<String>,
    search: Option<String>,
    language: Option<String>,
    item_kind: Option<String>,
    edge_kind: Option<String>,
    confidence: Option<String>,
    edge_relation: Option<String>,
    edge_source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NodeContextQuery {
    path: Option<PathBuf>,
    node_id: u64,
    edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct NodeCardQuery {
    path: Option<PathBuf>,
    node_id: u64,
    edge_limit: Option<usize>,
    source_context: Option<u32>,
    insight_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct FocusQuery {
    path: Option<PathBuf>,
    node_ids: Option<String>,
    edge_indexes: Option<String>,
    edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ExportQuery {
    path: Option<PathBuf>,
    format: Option<ExportFormat>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExportFormat {
    Json,
    Dot,
    Ndjson,
}

#[derive(Debug, Deserialize)]
struct ScanJobRequest {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct JobListQuery {
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct ScanJob {
    id: String,
    status: ScanJobStatus,
    path: String,
    message: String,
    created_at_unix: u64,
    updated_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at_unix: Option<u64>,
    cache: Option<CacheInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<codegraph_analysis::GraphSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graph: Option<CodeGraph>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ScanJobStatus {
    Queued,
    Running,
    Complete,
    Failed,
    Canceled,
}

impl ScanJobStatus {
    fn is_terminal(self) -> bool {
        matches!(
            self,
            ScanJobStatus::Complete | ScanJobStatus::Failed | ScanJobStatus::Canceled
        )
    }
}

#[derive(Debug, Serialize)]
struct ScanJobResult {
    id: String,
    root: String,
    graph: CodeGraph,
}

#[derive(Debug, Serialize)]
struct ScanJobListResponse {
    jobs: Vec<ScanJob>,
    total: usize,
    returned: usize,
    limit: usize,
    status: Option<ScanJobStatus>,
    summary: JobStoreHealth,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectRoot {
    name: String,
    path: PathBuf,
    default: bool,
}

#[derive(Debug, Serialize)]
struct ProjectResponse {
    name: String,
    path: String,
    default: bool,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    server_version: &'static str,
    root: String,
    max_file_size: u64,
    cache_dir: Option<String>,
    max_scan_jobs: usize,
    scan_jobs: JobStoreHealth,
    scan_concurrency: ConcurrencyHealth,
    max_semantic_jobs: usize,
    semantic_jobs: JobStoreHealth,
    semantic_concurrency: ConcurrencyHealth,
}

#[derive(Debug, Serialize)]
struct ProbeResponse {
    status: &'static str,
    server_version: &'static str,
    api_version: u32,
    graph_schema_version: u32,
    root: String,
    cache_enabled: bool,
}

#[derive(Debug, Serialize)]
struct MetricsResponse {
    status: &'static str,
    server_version: &'static str,
    api_version: u32,
    graph_schema_version: u32,
    uptime_seconds: u64,
    root: String,
    projects: usize,
    languages: usize,
    features: usize,
    max_file_size: u64,
    cache: CacheCapabilityResponse,
    scan_jobs: JobPoolMetricsResponse,
    semantic_jobs: JobPoolMetricsResponse,
}

#[derive(Debug, Serialize)]
struct JobPoolMetricsResponse {
    max_retained: usize,
    store: JobStoreHealth,
    concurrency: ConcurrencyHealth,
}

#[derive(Debug, Serialize)]
struct CapabilitiesResponse {
    name: &'static str,
    server_version: &'static str,
    api_version: u32,
    graph_schema_version: u32,
    root: String,
    projects: Vec<ProjectResponse>,
    languages: Vec<LanguageResponse>,
    export_formats: Vec<&'static str>,
    features: Vec<&'static str>,
    endpoints: Vec<EndpointGroupResponse>,
    scan: ScanCapabilityResponse,
    limits: RuntimeLimitsResponse,
    cache: CacheCapabilityResponse,
}

#[derive(Debug, Serialize)]
struct ApiSchemaResponse {
    name: &'static str,
    server_version: &'static str,
    api_version: u32,
    graph_schema_version: u32,
    description: &'static str,
    common_response_headers: Vec<ApiHeaderSpec>,
    groups: Vec<ApiSchemaGroup>,
    enum_values: BTreeMap<&'static str, Vec<&'static str>>,
}

#[derive(Debug, Serialize)]
struct ApiHeaderSpec {
    name: &'static str,
    value_type: &'static str,
    required: bool,
    description: &'static str,
}

#[derive(Debug, Serialize)]
struct ApiSchemaGroup {
    group: &'static str,
    endpoints: Vec<ApiEndpointSpec>,
}

#[derive(Debug, Serialize)]
struct ApiEndpointSpec {
    method: &'static str,
    path: &'static str,
    summary: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parameters: Vec<ApiParameterSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body_capability_limit: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    body_fields: Vec<ApiParameterSpec>,
    response: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    response_fields: Vec<ApiParameterSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    response_headers: Vec<ApiHeaderSpec>,
    streaming: bool,
}

impl ApiEndpointSpec {
    fn with_body_fields(mut self, body_fields: Vec<ApiParameterSpec>) -> Self {
        self.body_fields = body_fields;
        self
    }

    fn with_response_fields(mut self, response_fields: Vec<ApiParameterSpec>) -> Self {
        self.response_fields = response_fields;
        self
    }

    fn with_response_headers(mut self, response_headers: Vec<ApiHeaderSpec>) -> Self {
        self.response_headers = response_headers;
        self
    }
}

#[derive(Debug, Serialize)]
struct ApiParameterSpec {
    name: &'static str,
    location: &'static str,
    required: bool,
    value_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capability_limit: Option<&'static str>,
    description: &'static str,
}

impl ApiParameterSpec {
    fn with_range(mut self, minimum: usize, maximum: usize) -> Self {
        self.minimum = Some(minimum);
        self.maximum = Some(maximum);
        self
    }

    fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    fn with_capability_limit(mut self, capability_limit: &'static str) -> Self {
        self.capability_limit = Some(capability_limit);
        self
    }
}

#[derive(Debug, Serialize)]
struct EndpointGroupResponse {
    group: &'static str,
    endpoints: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct ScanCapabilityResponse {
    include_hidden: bool,
    include_ignored: bool,
    allow_any_path: bool,
    max_file_size: u64,
}

#[derive(Debug, Serialize)]
struct RuntimeLimitsResponse {
    max_scan_jobs: usize,
    max_semantic_jobs: usize,
    max_scan_concurrency: usize,
    max_semantic_concurrency: usize,
    default_api_body_bytes: usize,
    max_api_body_bytes: usize,
    max_configurable_api_body_bytes: usize,
    default_job_list_limit: usize,
    max_job_list_limit: usize,
    default_semantic_work_item_limit: usize,
    max_semantic_work_item_limit: usize,
    default_semantic_request_timeout_ms: u64,
    max_semantic_request_timeout_ms: u64,
    default_incremental_report_limit: usize,
    max_incremental_report_limit: usize,
    default_graph_node_limit: usize,
    max_graph_node_limit: usize,
    default_graph_edge_limit: usize,
    max_graph_edge_limit: usize,
    default_node_context_edge_limit: usize,
    max_node_context_edge_limit: usize,
    default_node_card_source_context: u32,
    max_node_card_source_context: u32,
    default_node_card_insight_limit: usize,
    max_node_card_insight_limit: usize,
    default_focus_edge_limit: usize,
    max_focus_edge_limit: usize,
    default_graph_query_limit: usize,
    max_graph_query_limit: usize,
    max_graph_query_length: usize,
    default_insight_limit: usize,
    max_insight_limit: usize,
    default_report_architecture_group_limit: usize,
    max_report_architecture_group_limit: usize,
    default_report_architecture_edge_limit: usize,
    max_report_architecture_edge_limit: usize,
    default_report_language_link_limit: usize,
    max_report_language_link_limit: usize,
    default_report_hotspot_limit: usize,
    max_report_hotspot_limit: usize,
    default_report_insight_limit: usize,
    max_report_insight_limit: usize,
    default_source_context: u32,
    max_source_context: u32,
    default_source_search_limit: usize,
    max_source_search_limit: usize,
    max_source_search_query_length: usize,
    default_source_search_context: usize,
    max_source_search_context: usize,
}

#[derive(Debug, Serialize)]
struct CacheCapabilityResponse {
    enabled: bool,
    dir: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct JobStoreHealth {
    total: usize,
    queued: usize,
    running: usize,
    complete: usize,
    failed: usize,
    canceled: usize,
}

#[derive(Debug, Serialize)]
struct ConcurrencyHealth {
    limit: usize,
    active: usize,
    available: usize,
}

#[derive(Debug, Serialize)]
struct LanguageResponse {
    language: &'static str,
    parser: &'static str,
    extensions: &'static [&'static str],
    file_names: &'static [&'static str],
}

#[derive(Debug, Serialize)]
struct ScanOptionsResponse {
    root: String,
    config_path: Option<String>,
    include_hidden: bool,
    include_ignored: bool,
    max_file_size: u64,
    ignored_names: Vec<String>,
    ignored_globs: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ScanResponse {
    root: String,
    cache: CacheInfo,
    graph: CodeGraph,
}

#[derive(Debug, Serialize)]
struct ProjectReportResponse {
    root: String,
    generated_at_unix: u64,
    cache: CacheInfo,
    coverage: codegraph_indexer::ScanCoverageReport,
    report: ProjectReport,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let root = args
        .root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", args.root.display()))?;
    let projects = Arc::new(project_roots(&root, args.projects)?);
    let bind_addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;
    let api_auth = ApiAuth::new(configured_api_token(args.api_token.clone()));

    let max_scan_concurrency = args.max_scan_concurrency.max(1);
    let max_semantic_concurrency = args.max_semantic_concurrency.max(1);
    let max_api_body_bytes = normalize_api_body_bytes(args.max_api_body_bytes);
    let state = AppState {
        root,
        started_at: Instant::now(),
        projects,
        option_overrides: IndexOptionOverrides {
            include_hidden: args.include_hidden,
            include_ignored: args.include_ignored,
            max_file_size: args.max_file_size,
        },
        allow_any_path: args.allow_any_path,
        cache: if args.no_cache {
            None
        } else {
            Some(GraphCache::new(
                args.cache_dir.unwrap_or_else(default_cache_dir),
            ))
        },
        jobs: Arc::new(RwLock::new(BTreeMap::new())),
        semantic_jobs: Arc::new(RwLock::new(BTreeMap::new())),
        max_scan_jobs: args.max_scan_jobs.max(1),
        max_semantic_jobs: args.max_semantic_jobs.max(1),
        max_scan_concurrency,
        max_semantic_concurrency,
        max_api_body_bytes,
        access_log_enabled: !args.quiet_access_log,
        scan_permits: Arc::new(Semaphore::new(max_scan_concurrency)),
        semantic_permits: Arc::new(Semaphore::new(max_semantic_concurrency)),
        next_job_id: Arc::new(AtomicU64::new(1)),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/label-policy.js", get(label_policy_js))
        .route("/app.js", get(app_js))
        .route("/styles.css", get(styles_css))
        .route("/api/capabilities", get(capabilities_api))
        .route("/api/schema", get(api_schema_api))
        .route("/api/live", get(live_api))
        .route("/api/ready", get(ready_api))
        .route("/api/health", get(health))
        .route("/api/metrics", get(metrics_api))
        .route("/api/languages", get(languages_api))
        .route("/api/lsp", get(lsp_api))
        .route("/api/semantic-readiness", get(semantic_readiness_api))
        .route("/api/semantic-plan", get(semantic_plan_api))
        .route("/api/semantic-batch", get(semantic_batch_api))
        .route("/api/semantic-patch", post(semantic_patch_api))
        .route("/api/semantic-apply", post(semantic_apply_api))
        .route("/api/semantic-enrich", post(semantic_enrich_api))
        .route(
            "/api/semantic-jobs",
            get(list_semantic_jobs).post(start_semantic_job),
        )
        .route(
            "/api/semantic-jobs/{id}",
            get(semantic_job_status).delete(cancel_semantic_job),
        )
        .route("/api/semantic-jobs/{id}/events", get(semantic_job_events))
        .route("/api/semantic-jobs/{id}/result", get(semantic_job_result))
        .route("/api/projects", get(projects_api))
        .route("/api/scan-options", get(scan_options_api))
        .route("/api/coverage", get(coverage_api))
        .route("/api/scan", get(scan))
        .route("/api/cache-diff", get(cache_diff_api))
        .route("/api/cache-chunks", get(cache_chunks_api))
        .route("/api/incremental-plan", get(incremental_plan_api))
        .route("/api/incremental-scan", get(incremental_scan_api))
        .route(
            "/api/incremental-merge-preview",
            get(incremental_merge_preview_api),
        )
        .route(
            "/api/incremental-update",
            get(incremental_update_api).post(incremental_update_api),
        )
        .route("/api/scan-jobs", get(list_scan_jobs).post(start_scan_job))
        .route(
            "/api/scan-jobs/{id}",
            get(scan_job_status).delete(cancel_scan_job),
        )
        .route("/api/scan-jobs/{id}/events", get(scan_job_events))
        .route("/api/scan-jobs/{id}/result", get(scan_job_result))
        .route("/api/export", get(export_api))
        .route("/api/graph", get(graph_api))
        .route("/api/node-context", get(node_context_api))
        .route("/api/node-card", get(node_card_api))
        .route("/api/focus", get(focus_api))
        .route("/api/report", get(report_api))
        .route("/api/summary", get(summary))
        .route("/api/architecture", get(architecture_api))
        .route("/api/language-dependencies", get(language_dependencies_api))
        .route("/api/hotspots", get(hotspots_api))
        .route("/api/entrypoints", get(entrypoints_api))
        .route("/api/entrypoint-traces", get(entrypoint_traces_api))
        .route("/api/insights", get(insights_api))
        .route("/api/check", get(check_api))
        .route("/api/query", get(query_api))
        .route("/api/explain-edge", get(explain_edge_api))
        .route("/api/trace", get(trace_api))
        .route("/api/dependents", get(dependents_api))
        .route("/api/trace-config", get(trace_config_api))
        .route("/api/trace-errors", get(trace_errors_api))
        .route("/api/source", get(source))
        .route("/api/source-search", get(source_search_api))
        .fallback(not_found)
        .with_state(state)
        .layer(DefaultBodyLimit::max(max_api_body_bytes))
        .layer(middleware::from_fn(cache_headers))
        .layer(middleware::from_fn(security_headers));
    let app = if api_auth.enabled() {
        app.layer(middleware::from_fn_with_state(
            api_auth,
            api_auth_middleware,
        ))
    } else {
        app
    };
    let app = if args.quiet_access_log {
        app
    } else {
        app.layer(middleware::from_fn(access_log))
    };
    let app = app.layer(middleware::from_fn(response_timing_header));
    let app = app.layer(middleware::from_fn(request_id_header));

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    println!("CodeGraph listening on http://{bind_addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server failed")?;
    println!("CodeGraph stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("failed to install Ctrl-C handler: {error}");
        }
    };

    #[cfg(unix)]
    {
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(error) => {
                    eprintln!("failed to install SIGTERM handler: {error}");
                    std::future::pending::<()>().await;
                }
            }
        };

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    apply_security_headers(response.headers_mut());
    response
}

async fn cache_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    apply_cache_headers_for_path(&path, response.headers_mut());
    response
}

async fn response_timing_header(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        HeaderName::from_static(RESPONSE_TIME_HEADER),
        response_time_header_value(started.elapsed()),
    );
    response
}

async fn api_auth_middleware(
    State(auth): State<ApiAuth>,
    request: Request,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/") || api_request_authorized(&request, &auth) {
        return next.run(request).await;
    }

    let mut response = ApiError::unauthorized("authentication required").into_response();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer realm=\"CodeGraph API\""),
    );
    response
}

#[derive(Debug, Clone)]
struct RequestId(String);

async fn request_id_header(mut request: Request, next: Next) -> Response {
    let request_id = incoming_request_id(&request).unwrap_or_else(next_request_id);
    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));
    let mut response = CURRENT_REQUEST_ID
        .scope(request_id.clone(), next.run(request))
        .await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-request-id"), value);
    }
    response
}

async fn access_log(request: Request, next: Next) -> Response {
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .map(|request_id| request_id.0.clone())
        .unwrap_or_else(|| "-".to_string());
    let method = request.method().as_str().to_string();
    let target = request
        .uri()
        .path_and_query()
        .map(|target| target.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let started = Instant::now();
    let response = next.run(request).await;
    let elapsed = started.elapsed();
    eprintln!(
        "{}",
        access_log_line(&request_id, &method, &target, response.status(), elapsed)
    );
    response
}

fn incoming_request_id(request: &Request) -> Option<String> {
    request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| is_valid_request_id(value))
        .map(ToOwned::to_owned)
}

fn configured_api_token(arg_token: Option<String>) -> Option<String> {
    arg_token
        .or_else(|| env::var("CODEGRAPH_API_TOKEN").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn api_request_authorized(request: &Request, auth: &ApiAuth) -> bool {
    let Some(expected) = auth.token.as_deref() else {
        return true;
    };
    request_api_token(request).is_some_and(|candidate| constant_time_eq(candidate, expected))
}

fn request_api_token(request: &Request) -> Option<&str> {
    bearer_token(request)
        .or_else(|| header_token(request, "x-codegraph-token"))
        .or_else(|| cookie_token(request))
}

fn bearer_token(request: &Request) -> Option<&str> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?
        .trim();
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn header_token<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn cookie_token(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(name, value)| (name.trim() == "codegraph_api_token").then_some(value.trim()))
        .filter(|token| !token.is_empty())
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0_u8, |diff, (left, right)| diff | (left ^ right))
        == 0
}

fn next_request_id() -> String {
    format!("req-{}", NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed))
}

fn current_request_id() -> Option<String> {
    CURRENT_REQUEST_ID.try_with(Clone::clone).ok()
}

fn is_valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn access_log_line(
    request_id: &str,
    method: &str,
    target: &str,
    status: StatusCode,
    elapsed: Duration,
) -> String {
    format!(
        "{request_id} {method} {target} -> {} {}ms",
        status.as_u16(),
        elapsed.as_millis()
    )
}

fn response_time_header_value(elapsed: Duration) -> HeaderValue {
    HeaderValue::from_str(&elapsed.as_millis().to_string())
        .expect("elapsed milliseconds are header-safe")
}

fn apply_export_headers(headers: &mut HeaderMap, nodes: usize, edges: usize, bytes: usize) {
    headers.insert(
        HeaderName::from_static(EXPORT_NODES_HEADER),
        usize_header_value(nodes),
    );
    headers.insert(
        HeaderName::from_static(EXPORT_EDGES_HEADER),
        usize_header_value(edges),
    );
    headers.insert(
        HeaderName::from_static(EXPORT_BYTES_HEADER),
        usize_header_value(bytes),
    );
}

fn usize_header_value(value: usize) -> HeaderValue {
    HeaderValue::from_str(&value.to_string()).expect("usize values are header-safe")
}

fn apply_security_headers(headers: &mut HeaderMap) {
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:",
        ),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    );
}

fn apply_cache_headers_for_path(path: &str, headers: &mut HeaderMap) {
    let value = if is_static_asset_path(path) {
        STATIC_ASSET_CACHE_CONTROL
    } else {
        DYNAMIC_CACHE_CONTROL
    };
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
}

fn is_static_asset_path(path: &str) -> bool {
    matches!(path, "/app.js" | "/label-policy.js" | "/styles.css")
}

fn normalize_api_body_bytes(value: usize) -> usize {
    value.clamp(1, MAX_API_BODY_BYTES)
}

fn static_asset_response(
    request_headers: &HeaderMap,
    content_type: &'static str,
    body: &'static str,
) -> Response {
    let etag = static_asset_etag(body);
    let etag_header = HeaderValue::from_str(&etag).expect("static asset etags are header-safe");
    if request_headers
        .get(header::IF_NONE_MATCH)
        .is_some_and(|value| if_none_match_matches(value, &etag))
    {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        response
            .headers_mut()
            .insert(header::ETAG, etag_header.clone());
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        return response;
    }

    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(header::ETAG, etag_header);
    response
}

fn static_asset_etag(body: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in body.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("\"codegraph-{}-{hash:016x}\"", body.len())
}

fn if_none_match_matches(value: &HeaderValue, etag: &str) -> bool {
    let Ok(value) = value.to_str() else {
        return false;
    };
    value.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || candidate == etag || candidate.strip_prefix("W/") == Some(etag)
    })
}

async fn start_scan_job(
    State(state): State<AppState>,
    Json(request): Json<ScanJobRequest>,
) -> Result<Json<ScanJob>, ApiError> {
    let root = resolve_scan_root(&state, request.path.as_deref())?;
    let id = format!("scan-{}", state.next_job_id.fetch_add(1, Ordering::Relaxed));
    let path = root.display().to_string();
    let now = unix_seconds();
    let job = ScanJob {
        id: id.clone(),
        status: ScanJobStatus::Queued,
        path: path.clone(),
        message: "queued; waiting for scan slot".to_string(),
        created_at_unix: now,
        updated_at_unix: now,
        finished_at_unix: None,
        cache: None,
        summary: None,
        graph: None,
    };
    insert_scan_job(&state.jobs, job.clone(), state.max_scan_jobs).await;

    let jobs = Arc::clone(&state.jobs);
    let options = scan_options(&state, &root)?;
    let cache = state.cache.clone();
    let max_jobs = state.max_scan_jobs;
    let scan_permits = Arc::clone(&state.scan_permits);
    tokio::spawn(async move {
        if scan_job_is_canceled(&jobs, &id).await {
            return;
        }

        let Ok(permit) = scan_permits.acquire_owned().await else {
            update_scan_job(
                &jobs,
                &id,
                ScanJobUpdate {
                    status: ScanJobStatus::Failed,
                    message: "scan concurrency limiter was closed".to_string(),
                    cache: None,
                    summary: None,
                    graph: None,
                },
                max_jobs,
            )
            .await;
            return;
        };

        if scan_job_is_canceled(&jobs, &id).await {
            return;
        }

        update_scan_job(
            &jobs,
            &id,
            ScanJobUpdate {
                status: ScanJobStatus::Running,
                message: "scanning project".to_string(),
                cache: None,
                summary: None,
                graph: None,
            },
            max_jobs,
        )
        .await;

        let scan_root = root.clone();
        let result = tokio::task::spawn_blocking(move || {
            scan_project_cached(scan_root, &options, cache.as_ref())
        })
        .await;
        drop(permit);
        match result {
            Ok(Ok(output)) => {
                let graph = output.graph;
                let summary = summarize(&graph);
                let message = match output.cache.status {
                    CacheStatus::Disabled => "complete".to_string(),
                    CacheStatus::Hit => "complete (cache hit)".to_string(),
                    CacheStatus::Miss => "complete (cache refreshed)".to_string(),
                };
                update_scan_job(
                    &jobs,
                    &id,
                    ScanJobUpdate {
                        status: ScanJobStatus::Complete,
                        message,
                        cache: Some(output.cache),
                        summary: Some(summary),
                        graph: Some(graph),
                    },
                    max_jobs,
                )
                .await;
            }
            Ok(Err(error)) => {
                update_scan_job(
                    &jobs,
                    &id,
                    ScanJobUpdate {
                        status: ScanJobStatus::Failed,
                        message: error.to_string(),
                        cache: None,
                        summary: None,
                        graph: None,
                    },
                    max_jobs,
                )
                .await;
            }
            Err(error) => {
                update_scan_job(
                    &jobs,
                    &id,
                    ScanJobUpdate {
                        status: ScanJobStatus::Failed,
                        message: format!("scanner task failed: {error}"),
                        cache: None,
                        summary: None,
                        graph: None,
                    },
                    max_jobs,
                )
                .await;
            }
        }
    });

    Ok(Json(job))
}

async fn list_scan_jobs(
    State(state): State<AppState>,
    Query(query): Query<JobListQuery>,
) -> Result<Json<ScanJobListResponse>, ApiError> {
    let status = parse_optional_job_status(query.status.as_deref())?;
    let limit = job_list_limit(query.limit);
    let jobs = state.jobs.read().await;
    let summary = job_store_health(jobs.values().map(|job| job.status));
    let total = jobs.len();
    let mut list: Vec<_> = jobs
        .values()
        .filter(|job| status.is_none_or(|status| job.status == status))
        .cloned()
        .map(job_without_graph)
        .collect();
    sort_scan_jobs_recent_first(&mut list);
    list.truncate(limit);

    Ok(Json(ScanJobListResponse {
        returned: list.len(),
        jobs: list,
        total,
        limit,
        status,
        summary,
    }))
}

async fn scan_job_status(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ScanJob>, ApiError> {
    let jobs = state.jobs.read().await;
    let job = jobs
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("scan job not found"))?;
    Ok(Json(job_without_graph(job)))
}

async fn cancel_scan_job(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ScanJob>, ApiError> {
    let job = cancel_scan_job_in_store(&state.jobs, &id, state.max_scan_jobs).await?;
    Ok(Json(job_without_graph(job)))
}

async fn scan_job_events(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    {
        let jobs = state.jobs.read().await;
        if !jobs.contains_key(&id) {
            return Err(ApiError::not_found("scan job not found"));
        }
    }

    let jobs = Arc::clone(&state.jobs);
    let stream = stream! {
        loop {
            let job = {
                let jobs = jobs.read().await;
                jobs.get(&id).cloned().map(job_without_graph)
            };

            let Some(job) = job else {
                let data = serde_json::json!({ "error": "scan job not found" }).to_string();
                yield Ok::<Event, Infallible>(Event::default().event("error").data(data));
                break;
            };

            let is_terminal = job.status.is_terminal();
            let data = serde_json::to_string(&job).unwrap_or_else(|error| {
                serde_json::json!({ "error": error.to_string() }).to_string()
            });
            yield Ok::<Event, Infallible>(Event::default().event("status").data(data));

            if is_terminal {
                break;
            }

            sleep(Duration::from_millis(350)).await;
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("codegraph-scan"),
    ))
}

async fn scan_job_result(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ScanJobResult>, ApiError> {
    let jobs = state.jobs.read().await;
    let job = jobs
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("scan job not found"))?;
    match job.status {
        ScanJobStatus::Complete => {
            let graph = job
                .graph
                .ok_or_else(|| ApiError::internal("scan job completed without graph"))?;
            Ok(Json(ScanJobResult {
                id: job.id,
                root: job.path,
                graph,
            }))
        }
        ScanJobStatus::Failed => Err(ApiError::internal(job.message)),
        ScanJobStatus::Canceled => Err(ApiError::bad_request("scan job was canceled")),
        _ => Err(ApiError::bad_request("scan job is not complete")),
    }
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn label_policy_js(headers: HeaderMap) -> Response {
    static_asset_response(
        &headers,
        "application/javascript; charset=utf-8",
        LABEL_POLICY_JS,
    )
}

async fn app_js(headers: HeaderMap) -> Response {
    static_asset_response(&headers, "application/javascript; charset=utf-8", APP_JS)
}

async fn styles_css(headers: HeaderMap) -> Response {
    static_asset_response(&headers, "text/css; charset=utf-8", STYLES_CSS)
}

async fn live_api(State(state): State<AppState>) -> Json<ProbeResponse> {
    Json(probe_response(&state, "ok"))
}

async fn ready_api(State(state): State<AppState>) -> Result<Json<ProbeResponse>, ApiError> {
    let _ = scan_options(&state, &state.root)?;
    Ok(Json(probe_response(&state, "ready")))
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    let options = scan_options(&state, &state.root)?;
    let scan_jobs = {
        let jobs = state.jobs.read().await;
        job_store_health(jobs.values().map(|job| job.status))
    };
    let semantic_jobs = {
        let jobs = state.semantic_jobs.read().await;
        job_store_health(jobs.values().map(|job| job.status))
    };
    Ok(Json(HealthResponse {
        status: "ok",
        server_version: SERVER_VERSION,
        root: state.root.display().to_string(),
        max_file_size: options.max_file_size,
        cache_dir: state
            .cache
            .as_ref()
            .map(|cache| cache.dir().display().to_string()),
        max_scan_jobs: state.max_scan_jobs,
        scan_jobs,
        scan_concurrency: concurrency_health(&state.scan_permits, state.max_scan_concurrency),
        max_semantic_jobs: state.max_semantic_jobs,
        semantic_jobs,
        semantic_concurrency: concurrency_health(
            &state.semantic_permits,
            state.max_semantic_concurrency,
        ),
    }))
}

fn probe_response(state: &AppState, status: &'static str) -> ProbeResponse {
    ProbeResponse {
        status,
        server_version: SERVER_VERSION,
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        root: state.root.display().to_string(),
        cache_enabled: state.cache.is_some(),
    }
}

async fn metrics_api(State(state): State<AppState>) -> Result<Json<MetricsResponse>, ApiError> {
    let options = scan_options(&state, &state.root)?;
    let scan_jobs = {
        let jobs = state.jobs.read().await;
        job_store_health(jobs.values().map(|job| job.status))
    };
    let semantic_jobs = {
        let jobs = state.semantic_jobs.read().await;
        job_store_health(jobs.values().map(|job| job.status))
    };
    Ok(Json(MetricsResponse {
        status: "ok",
        server_version: SERVER_VERSION,
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        uptime_seconds: state.started_at.elapsed().as_secs(),
        root: state.root.display().to_string(),
        projects: state.projects.len(),
        languages: language_adapters().len(),
        features: capability_features(state.cache.is_some(), state.access_log_enabled).len(),
        max_file_size: options.max_file_size,
        cache: CacheCapabilityResponse {
            enabled: state.cache.is_some(),
            dir: state
                .cache
                .as_ref()
                .map(|cache| cache.dir().display().to_string()),
        },
        scan_jobs: JobPoolMetricsResponse {
            max_retained: state.max_scan_jobs,
            store: scan_jobs,
            concurrency: concurrency_health(&state.scan_permits, state.max_scan_concurrency),
        },
        semantic_jobs: JobPoolMetricsResponse {
            max_retained: state.max_semantic_jobs,
            store: semantic_jobs,
            concurrency: concurrency_health(
                &state.semantic_permits,
                state.max_semantic_concurrency,
            ),
        },
    }))
}

async fn capabilities_api(
    State(state): State<AppState>,
) -> Result<Json<CapabilitiesResponse>, ApiError> {
    let options = scan_options(&state, &state.root)?;
    Ok(Json(CapabilitiesResponse {
        name: "CodeGraph",
        server_version: SERVER_VERSION,
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        root: state.root.display().to_string(),
        projects: project_responses(&state),
        languages: language_responses(),
        export_formats: vec!["json", "dot", "ndjson"],
        features: capability_features(state.cache.is_some(), state.access_log_enabled),
        endpoints: capability_endpoints(),
        scan: ScanCapabilityResponse {
            include_hidden: options.include_hidden,
            include_ignored: options.include_ignored,
            allow_any_path: state.allow_any_path,
            max_file_size: options.max_file_size,
        },
        limits: RuntimeLimitsResponse {
            max_scan_jobs: state.max_scan_jobs,
            max_semantic_jobs: state.max_semantic_jobs,
            max_scan_concurrency: state.max_scan_concurrency,
            max_semantic_concurrency: state.max_semantic_concurrency,
            default_api_body_bytes: DEFAULT_API_BODY_BYTES,
            max_api_body_bytes: state.max_api_body_bytes,
            max_configurable_api_body_bytes: MAX_API_BODY_BYTES,
            default_job_list_limit: DEFAULT_JOB_LIST_LIMIT,
            max_job_list_limit: MAX_JOB_LIST_LIMIT,
            default_semantic_work_item_limit: DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            max_semantic_work_item_limit: MAX_SEMANTIC_WORK_ITEM_LIMIT,
            default_semantic_request_timeout_ms: DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS,
            max_semantic_request_timeout_ms: MAX_SEMANTIC_REQUEST_TIMEOUT_MS,
            default_incremental_report_limit: DEFAULT_INCREMENTAL_REPORT_LIMIT,
            max_incremental_report_limit: MAX_INCREMENTAL_REPORT_LIMIT,
            default_graph_node_limit: DEFAULT_GRAPH_NODE_LIMIT,
            max_graph_node_limit: MAX_GRAPH_NODE_LIMIT,
            default_graph_edge_limit: DEFAULT_GRAPH_EDGE_LIMIT,
            max_graph_edge_limit: MAX_GRAPH_EDGE_LIMIT,
            default_node_context_edge_limit: DEFAULT_NODE_CONTEXT_EDGE_LIMIT,
            max_node_context_edge_limit: MAX_NODE_CONTEXT_EDGE_LIMIT,
            default_node_card_source_context: DEFAULT_NODE_CARD_SOURCE_CONTEXT,
            max_node_card_source_context: MAX_NODE_CARD_SOURCE_CONTEXT,
            default_node_card_insight_limit: DEFAULT_NODE_CARD_INSIGHT_LIMIT,
            max_node_card_insight_limit: MAX_NODE_CARD_INSIGHT_LIMIT,
            default_focus_edge_limit: DEFAULT_FOCUS_EDGE_LIMIT,
            max_focus_edge_limit: MAX_FOCUS_EDGE_LIMIT,
            default_graph_query_limit: DEFAULT_GRAPH_QUERY_LIMIT,
            max_graph_query_limit: MAX_GRAPH_QUERY_LIMIT,
            max_graph_query_length: MAX_GRAPH_QUERY_LENGTH,
            default_insight_limit: DEFAULT_INSIGHT_LIMIT,
            max_insight_limit: MAX_INSIGHT_LIMIT,
            default_report_architecture_group_limit: DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
            max_report_architecture_group_limit: MAX_REPORT_ARCHITECTURE_GROUP_LIMIT,
            default_report_architecture_edge_limit: DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT,
            max_report_architecture_edge_limit: MAX_REPORT_ARCHITECTURE_EDGE_LIMIT,
            default_report_language_link_limit: DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
            max_report_language_link_limit: MAX_REPORT_LANGUAGE_LINK_LIMIT,
            default_report_hotspot_limit: DEFAULT_REPORT_HOTSPOT_LIMIT,
            max_report_hotspot_limit: MAX_REPORT_HOTSPOT_LIMIT,
            default_report_insight_limit: DEFAULT_REPORT_INSIGHT_LIMIT,
            max_report_insight_limit: MAX_REPORT_INSIGHT_LIMIT,
            default_source_context: DEFAULT_SOURCE_CONTEXT,
            max_source_context: MAX_SOURCE_CONTEXT,
            default_source_search_limit: DEFAULT_SOURCE_SEARCH_LIMIT,
            max_source_search_limit: MAX_SOURCE_SEARCH_LIMIT,
            max_source_search_query_length: MAX_SOURCE_SEARCH_QUERY_LENGTH,
            default_source_search_context: DEFAULT_SOURCE_SEARCH_CONTEXT,
            max_source_search_context: MAX_SOURCE_SEARCH_CONTEXT,
        },
        cache: CacheCapabilityResponse {
            enabled: state.cache.is_some(),
            dir: state
                .cache
                .as_ref()
                .map(|cache| cache.dir().display().to_string()),
        },
    }))
}

async fn api_schema_api() -> Json<ApiSchemaResponse> {
    Json(api_schema_response())
}

async fn languages_api() -> Json<Vec<LanguageResponse>> {
    Json(language_responses())
}

async fn lsp_api() -> Json<LspDiscoveryReport> {
    Json(discover_lsp_servers())
}

async fn semantic_readiness_api(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<SemanticReadinessReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let summary = summarize(&graph);
    Ok(Json(semantic_readiness(&summary.languages)))
}

async fn semantic_plan_api(
    State(state): State<AppState>,
    Query(query): Query<SemanticPlanQuery>,
) -> Result<Json<SemanticEnrichmentPlan>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(semantic_enrichment_plan_with_filter(
        &graph,
        query
            .work_item_limit
            .unwrap_or(DEFAULT_SEMANTIC_WORK_ITEM_LIMIT),
        SemanticWorkItemFilter {
            language: query.work_language,
            status: query.work_status,
            capability: query.work_capability,
        },
    )))
}

async fn semantic_batch_api(
    State(state): State<AppState>,
    Query(query): Query<SemanticPlanQuery>,
) -> Result<Json<codegraph_lsp::SemanticExecutionBatch>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let graph = scan_graph(&state, Some(root.as_path())).await?;
    Ok(Json(semantic_execution_batch(
        &root,
        &graph,
        query
            .work_item_limit
            .unwrap_or(DEFAULT_SEMANTIC_WORK_ITEM_LIMIT),
        SemanticWorkItemFilter {
            language: query.work_language,
            status: query.work_status,
            capability: query.work_capability,
        },
    )))
}

async fn semantic_patch_api(
    State(state): State<AppState>,
    Json(request): Json<SemanticPatchRequest>,
) -> Result<Json<SemanticGraphPatch>, ApiError> {
    let root = resolve_scan_root(&state, request.path.as_deref())?;
    let graph = scan_graph(&state, Some(root.as_path())).await?;
    let batch = semantic_execution_batch(
        &root,
        &graph,
        request
            .work_item_limit
            .unwrap_or(DEFAULT_SEMANTIC_WORK_ITEM_LIMIT),
        SemanticWorkItemFilter {
            language: request.work_language,
            status: request.work_status,
            capability: request.work_capability,
        },
    );
    Ok(Json(semantic_graph_patch_from_responses(
        &root,
        &graph,
        &batch,
        &request.responses,
    )))
}

async fn semantic_apply_api(
    State(state): State<AppState>,
    Json(request): Json<SemanticPatchRequest>,
) -> Result<Json<SemanticGraphApplyResult>, ApiError> {
    let root = resolve_scan_root(&state, request.path.as_deref())?;
    let graph = scan_graph(&state, Some(root.as_path())).await?;
    let batch = semantic_execution_batch(
        &root,
        &graph,
        request
            .work_item_limit
            .unwrap_or(DEFAULT_SEMANTIC_WORK_ITEM_LIMIT),
        SemanticWorkItemFilter {
            language: request.work_language,
            status: request.work_status,
            capability: request.work_capability,
        },
    );
    let patch = semantic_graph_patch_from_responses(&root, &graph, &batch, &request.responses);
    Ok(Json(apply_semantic_graph_patch(&graph, &patch)))
}

async fn semantic_enrich_api(
    State(state): State<AppState>,
    Json(request): Json<SemanticEnrichRequest>,
) -> Result<Json<SemanticEnrichResponse>, ApiError> {
    let root = resolve_scan_root(&state, request.path.as_deref())?;
    let graph = scan_graph(&state, Some(root.as_path())).await?;
    let semantic_cache = state
        .cache
        .as_ref()
        .map(|cache| SemanticLspCache::new(cache.dir().join("semantic-lsp")));

    let result = tokio::task::spawn_blocking(move || {
        run_semantic_enrichment(root, graph, request, semantic_cache)
    })
    .await
    .map_err(|error| ApiError::internal(format!("semantic enrichment task failed: {error}")))?
    .map_err(|error| ApiError::bad_request(format!("semantic enrichment failed: {error}")))?;

    Ok(Json(result))
}

async fn start_semantic_job(
    State(state): State<AppState>,
    Json(request): Json<SemanticEnrichRequest>,
) -> Result<Json<SemanticJob>, ApiError> {
    let root = resolve_scan_root(&state, request.path.as_deref())?;
    let id = format!(
        "semantic-{}",
        state.next_job_id.fetch_add(1, Ordering::Relaxed)
    );
    let path = root.display().to_string();
    let now = unix_seconds();
    let job = SemanticJob {
        id: id.clone(),
        status: ScanJobStatus::Queued,
        path: path.clone(),
        message: "queued; waiting for semantic slot".to_string(),
        created_at_unix: now,
        updated_at_unix: now,
        finished_at_unix: None,
        responses: None,
        response_errors: None,
        unmatched_locations: None,
        report: None,
        result: None,
    };
    insert_semantic_job(&state.semantic_jobs, job.clone(), state.max_semantic_jobs).await;

    let jobs = Arc::clone(&state.semantic_jobs);
    let options = scan_options(&state, &root)?;
    let cache = state.cache.clone();
    let semantic_cache = cache
        .as_ref()
        .map(|cache| SemanticLspCache::new(cache.dir().join("semantic-lsp")));
    let max_jobs = state.max_semantic_jobs;
    let semantic_permits = Arc::clone(&state.semantic_permits);
    tokio::spawn(async move {
        if semantic_job_is_canceled(&jobs, &id).await {
            return;
        }

        let Ok(permit) = semantic_permits.acquire_owned().await else {
            update_semantic_job(
                &jobs,
                &id,
                SemanticJobUpdate {
                    status: ScanJobStatus::Failed,
                    message: "semantic concurrency limiter was closed".to_string(),
                    result: None,
                },
                max_jobs,
            )
            .await;
            return;
        };

        if semantic_job_is_canceled(&jobs, &id).await {
            return;
        }

        update_semantic_job(
            &jobs,
            &id,
            SemanticJobUpdate {
                status: ScanJobStatus::Running,
                message: "running semantic enrichment".to_string(),
                result: None,
            },
            max_jobs,
        )
        .await;

        let scan_root = root.clone();
        let result = tokio::task::spawn_blocking(move || {
            let output = scan_project_cached(scan_root.clone(), &options, cache.as_ref())
                .map_err(|error| error.to_string())?;
            run_semantic_enrichment(scan_root, output.graph, request, semantic_cache)
                .map_err(|error| error.to_string())
        })
        .await;
        drop(permit);

        match result {
            Ok(Ok(result)) => {
                update_semantic_job(
                    &jobs,
                    &id,
                    SemanticJobUpdate {
                        status: ScanJobStatus::Complete,
                        message: "complete".to_string(),
                        result: Some(result),
                    },
                    max_jobs,
                )
                .await;
            }
            Ok(Err(error)) => {
                update_semantic_job(
                    &jobs,
                    &id,
                    SemanticJobUpdate {
                        status: ScanJobStatus::Failed,
                        message: error,
                        result: None,
                    },
                    max_jobs,
                )
                .await;
            }
            Err(error) => {
                update_semantic_job(
                    &jobs,
                    &id,
                    SemanticJobUpdate {
                        status: ScanJobStatus::Failed,
                        message: format!("semantic enrichment task failed: {error}"),
                        result: None,
                    },
                    max_jobs,
                )
                .await;
            }
        }
    });

    Ok(Json(job))
}

async fn list_semantic_jobs(
    State(state): State<AppState>,
    Query(query): Query<JobListQuery>,
) -> Result<Json<SemanticJobListResponse>, ApiError> {
    let status = parse_optional_job_status(query.status.as_deref())?;
    let limit = job_list_limit(query.limit);
    let jobs = state.semantic_jobs.read().await;
    let summary = job_store_health(jobs.values().map(|job| job.status));
    let total = jobs.len();
    let mut list: Vec<_> = jobs
        .values()
        .filter(|job| status.is_none_or(|status| job.status == status))
        .cloned()
        .map(semantic_job_without_result)
        .collect();
    sort_semantic_jobs_recent_first(&mut list);
    list.truncate(limit);

    Ok(Json(SemanticJobListResponse {
        returned: list.len(),
        jobs: list,
        total,
        limit,
        status,
        summary,
    }))
}

async fn semantic_job_status(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<SemanticJob>, ApiError> {
    let jobs = state.semantic_jobs.read().await;
    let job = jobs
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("semantic job not found"))?;
    Ok(Json(semantic_job_without_result(job)))
}

async fn cancel_semantic_job(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<SemanticJob>, ApiError> {
    let job =
        cancel_semantic_job_in_store(&state.semantic_jobs, &id, state.max_semantic_jobs).await?;
    Ok(Json(semantic_job_without_result(job)))
}

async fn semantic_job_events(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    {
        let jobs = state.semantic_jobs.read().await;
        if !jobs.contains_key(&id) {
            return Err(ApiError::not_found("semantic job not found"));
        }
    }

    let jobs = Arc::clone(&state.semantic_jobs);
    let stream = stream! {
        loop {
            let job = {
                let jobs = jobs.read().await;
                jobs.get(&id).cloned().map(semantic_job_without_result)
            };

            let Some(job) = job else {
                let data = serde_json::json!({ "error": "semantic job not found" }).to_string();
                yield Ok::<Event, Infallible>(Event::default().event("error").data(data));
                break;
            };

            let is_terminal = job.status.is_terminal();
            let data = serde_json::to_string(&job).unwrap_or_else(|error| {
                serde_json::json!({ "error": error.to_string() }).to_string()
            });
            yield Ok::<Event, Infallible>(Event::default().event("status").data(data));

            if is_terminal {
                break;
            }

            sleep(Duration::from_millis(350)).await;
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("codegraph-semantic"),
    ))
}

async fn semantic_job_result(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<SemanticJobResult>, ApiError> {
    let jobs = state.semantic_jobs.read().await;
    let job = jobs
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("semantic job not found"))?;
    match job.status {
        ScanJobStatus::Complete => {
            let result = job
                .result
                .ok_or_else(|| ApiError::internal("semantic job completed without result"))?;
            Ok(Json(SemanticJobResult {
                id: job.id,
                root: job.path,
                result,
            }))
        }
        ScanJobStatus::Failed => Err(ApiError::internal(job.message)),
        ScanJobStatus::Canceled => Err(ApiError::bad_request("semantic job was canceled")),
        _ => Err(ApiError::bad_request("semantic job is not complete")),
    }
}

async fn projects_api(State(state): State<AppState>) -> Json<Vec<ProjectResponse>> {
    Json(project_responses(&state))
}

async fn scan_options_api(
    State(state): State<AppState>,
    Query(query): Query<ScanOptionsQuery>,
) -> Result<Json<ScanOptionsResponse>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = scan_options(&state, &root)?;
    let config_path = root.join(".codegraph").join("config.toml");
    Ok(Json(ScanOptionsResponse {
        root: root.display().to_string(),
        config_path: config_path
            .is_file()
            .then(|| config_path.display().to_string()),
        include_hidden: options.include_hidden,
        include_ignored: options.include_ignored,
        max_file_size: options.max_file_size,
        ignored_names: options.ignored_names.into_iter().collect(),
        ignored_globs: options.ignored_globs.into_iter().collect(),
    }))
}

async fn scan(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<ScanResponse>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = scan_options(&state, &root)?;
    let cache = state.cache.clone();
    let root_label = root.display().to_string();
    let output =
        tokio::task::spawn_blocking(move || scan_project_cached(root, &options, cache.as_ref()))
            .await
            .map_err(|error| ApiError::internal(format!("scanner task failed: {error}")))?
            .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(ScanResponse {
        root: root_label,
        cache: output.cache,
        graph: output.graph,
    }))
}

async fn cache_diff_api(
    State(state): State<AppState>,
    Query(query): Query<CacheDiffQuery>,
) -> Result<Json<codegraph_storage::CacheDiffReport>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let Some(cache) = state.cache.clone() else {
        return Err(ApiError::bad_request(
            "cache diff requires server cache; restart without --no-cache",
        ));
    };
    let options = scan_options(&state, &root)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_INCREMENTAL_REPORT_LIMIT)
        .clamp(1, MAX_INCREMENTAL_REPORT_LIMIT);
    let report = tokio::task::spawn_blocking(move || cache.diff(&root, &options, limit))
        .await
        .map_err(|error| ApiError::internal(format!("cache diff task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(report))
}

async fn cache_chunks_api(
    State(state): State<AppState>,
    Query(query): Query<CacheDiffQuery>,
) -> Result<Json<codegraph_storage::CacheChunkReport>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let Some(cache) = state.cache.clone() else {
        return Err(ApiError::bad_request(
            "cache chunks require server cache; restart without --no-cache",
        ));
    };
    let options = scan_options(&state, &root)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_INCREMENTAL_REPORT_LIMIT)
        .clamp(1, MAX_INCREMENTAL_REPORT_LIMIT);
    let report = tokio::task::spawn_blocking(move || cache.chunks(&root, &options, limit))
        .await
        .map_err(|error| ApiError::internal(format!("cache chunks task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(report))
}

async fn incremental_plan_api(
    State(state): State<AppState>,
    Query(query): Query<CacheDiffQuery>,
) -> Result<Json<codegraph_storage::IncrementalScanPlan>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let Some(cache) = state.cache.clone() else {
        return Err(ApiError::bad_request(
            "incremental plan requires server cache; restart without --no-cache",
        ));
    };
    let options = scan_options(&state, &root)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_INCREMENTAL_REPORT_LIMIT)
        .clamp(1, MAX_INCREMENTAL_REPORT_LIMIT);
    let plan = tokio::task::spawn_blocking(move || cache.incremental_plan(&root, &options, limit))
        .await
        .map_err(|error| ApiError::internal(format!("incremental plan task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(plan))
}

async fn incremental_scan_api(
    State(state): State<AppState>,
    Query(query): Query<CacheDiffQuery>,
) -> Result<Json<codegraph_storage::IncrementalScan>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let Some(cache) = state.cache.clone() else {
        return Err(ApiError::bad_request(
            "incremental scan requires server cache; restart without --no-cache",
        ));
    };
    let options = scan_options(&state, &root)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_INCREMENTAL_REPORT_LIMIT)
        .clamp(1, MAX_INCREMENTAL_REPORT_LIMIT);
    let scan = tokio::task::spawn_blocking(move || cache.incremental_scan(&root, &options, limit))
        .await
        .map_err(|error| ApiError::internal(format!("incremental scan task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(scan))
}

async fn incremental_merge_preview_api(
    State(state): State<AppState>,
    Query(query): Query<CacheDiffQuery>,
) -> Result<Json<codegraph_storage::IncrementalMergePreview>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let Some(cache) = state.cache.clone() else {
        return Err(ApiError::bad_request(
            "incremental merge preview requires server cache; restart without --no-cache",
        ));
    };
    let options = scan_options(&state, &root)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_INCREMENTAL_REPORT_LIMIT)
        .clamp(1, MAX_INCREMENTAL_REPORT_LIMIT);
    let preview = tokio::task::spawn_blocking(move || {
        cache.incremental_merge_preview(&root, &options, limit)
    })
    .await
    .map_err(|error| ApiError::internal(format!("incremental merge preview task failed: {error}")))?
    .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(preview))
}

async fn incremental_update_api(
    State(state): State<AppState>,
    Query(query): Query<CacheDiffQuery>,
) -> Result<Json<codegraph_storage::IncrementalUpdate>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let Some(cache) = state.cache.clone() else {
        return Err(ApiError::bad_request(
            "incremental update requires server cache; restart without --no-cache",
        ));
    };
    let options = scan_options(&state, &root)?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_INCREMENTAL_REPORT_LIMIT)
        .clamp(1, MAX_INCREMENTAL_REPORT_LIMIT);
    let update =
        tokio::task::spawn_blocking(move || cache.incremental_update(&root, &options, limit))
            .await
            .map_err(|error| {
                ApiError::internal(format!("incremental update task failed: {error}"))
            })?
            .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(update))
}

async fn export_api(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let node_count = graph.nodes.len();
    let edge_count = graph.edges.len();
    let format = query.format.unwrap_or(ExportFormat::Json);
    let (content_type, body) = match format {
        ExportFormat::Json => (
            "application/json; charset=utf-8",
            serde_json::to_string_pretty(&graph)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        ),
        ExportFormat::Dot => ("text/vnd.graphviz; charset=utf-8", export_dot(&graph)),
        ExportFormat::Ndjson => (
            "application/x-ndjson; charset=utf-8",
            export_ndjson(&graph).map_err(|error| ApiError::internal(error.to_string()))?,
        ),
    };
    let body_bytes = body.len();
    let mut response = (
        [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
        body,
    )
        .into_response();
    apply_export_headers(response.headers_mut(), node_count, edge_count, body_bytes);
    Ok(response)
}

async fn graph_api(
    State(state): State<AppState>,
    Query(query): Query<GraphSliceQuery>,
) -> Result<Json<GraphSlice>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(slice_graph(
        &graph,
        GraphSliceRequest {
            node_offset: query.node_offset.unwrap_or(0),
            node_limit: query.node_limit.unwrap_or(DEFAULT_GRAPH_NODE_LIMIT),
            edge_offset: query.edge_offset.unwrap_or(0),
            edge_limit: query.edge_limit.unwrap_or(DEFAULT_GRAPH_EDGE_LIMIT),
            path_prefix: normalize_query_string(query.path_prefix),
            kind: normalize_query_string(query.kind),
            search: normalize_query_string(query.search),
            language: normalize_query_string(query.language),
            item_kind: normalize_query_string(query.item_kind),
            edge_kind: normalize_query_string(query.edge_kind),
            confidence: normalize_query_string(query.confidence),
            edge_relation: normalize_query_string(query.edge_relation),
            edge_source: normalize_query_string(query.edge_source),
        },
    )))
}

async fn node_context_api(
    State(state): State<AppState>,
    Query(query): Query<NodeContextQuery>,
) -> Result<Json<NodeContext>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let context = node_context(
        &graph,
        codegraph_core::NodeId(query.node_id),
        query.edge_limit.unwrap_or(DEFAULT_NODE_CONTEXT_EDGE_LIMIT),
    )
    .ok_or_else(|| ApiError::not_found("node not found"))?;
    Ok(Json(context))
}

async fn node_card_api(
    State(state): State<AppState>,
    Query(query): Query<NodeCardQuery>,
) -> Result<Json<NodeCard>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = scan_options(&state, &root)?;
    let cache = state.cache.clone();
    let edge_limit = query.edge_limit.unwrap_or(DEFAULT_NODE_CONTEXT_EDGE_LIMIT);
    let source_context = query
        .source_context
        .unwrap_or(DEFAULT_NODE_CARD_SOURCE_CONTEXT)
        .min(MAX_NODE_CARD_SOURCE_CONTEXT);
    let insight_limit = query
        .insight_limit
        .unwrap_or(DEFAULT_NODE_CARD_INSIGHT_LIMIT);
    let node_id = codegraph_core::NodeId(query.node_id);
    let card = tokio::task::spawn_blocking(move || {
        let output = scan_project_cached(root.clone(), &options, cache.as_ref())
            .map_err(|error| error.to_string())?;
        node_card(
            &output.graph,
            Some(&root),
            node_id,
            edge_limit,
            source_context,
            insight_limit,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| ApiError::internal(format!("node card task failed: {error}")))?
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("node not found"))?;
    Ok(Json(card))
}

async fn focus_api(
    State(state): State<AppState>,
    Query(query): Query<FocusQuery>,
) -> Result<Json<codegraph_analysis::QueryResult>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let node_ids = parse_node_ids(query.node_ids.as_deref())?;
    let edge_indexes = parse_edge_indexes(query.edge_indexes.as_deref())?;
    Ok(Json(focus_subgraph(
        &graph,
        FocusRequest {
            node_ids,
            edge_indexes,
            edge_limit: query.edge_limit.unwrap_or(DEFAULT_FOCUS_EDGE_LIMIT),
        },
    )))
}

async fn report_api(
    State(state): State<AppState>,
    Query(query): Query<ProjectReportQuery>,
) -> Result<Json<ProjectReportResponse>, ApiError> {
    let limits = project_report_limits_from_query(&query)?;
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let root_label = root.display().to_string();
    let options = scan_options(&state, &root)?;
    let cache = state.cache.clone();
    let response = tokio::task::spawn_blocking(move || -> Result<ProjectReportResponse, String> {
        let output = scan_project_cached(root.clone(), &options, cache.as_ref())
            .map_err(|error| error.to_string())?;
        let coverage = scan_coverage(&root, &options).map_err(|error| error.to_string())?;
        let report = project_report(&output.graph, limits);

        Ok(ProjectReportResponse {
            root: root_label,
            generated_at_unix: unix_seconds(),
            cache: output.cache,
            coverage,
            report,
        })
    })
    .await
    .map_err(|error| ApiError::internal(format!("project report task failed: {error}")))?
    .map_err(ApiError::internal)?;

    Ok(Json(response))
}

async fn summary(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<codegraph_analysis::GraphSummary>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(summarize(&graph)))
}

async fn coverage_api(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<codegraph_indexer::ScanCoverageReport>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = scan_options(&state, &root)?;
    let report = tokio::task::spawn_blocking(move || scan_coverage(&root, &options))
        .await
        .map_err(|error| ApiError::internal(format!("coverage task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(report))
}

async fn architecture_api(
    State(state): State<AppState>,
    Query(query): Query<ArchitectureQuery>,
) -> Result<Json<codegraph_analysis::ArchitectureMap>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(architecture_map(
        &graph,
        query.group_limit.unwrap_or(50),
        query.edge_limit.unwrap_or(200),
    )))
}

async fn language_dependencies_api(
    State(state): State<AppState>,
    Query(query): Query<LanguageDependencyQuery>,
) -> Result<Json<codegraph_analysis::LanguageDependencyReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(language_dependencies(
        &graph,
        query.limit.unwrap_or(50),
    )))
}

async fn hotspots_api(
    State(state): State<AppState>,
    Query(query): Query<HotspotQuery>,
) -> Result<Json<codegraph_analysis::HotspotReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(hotspots(&graph, query.limit.unwrap_or(25))))
}

async fn entrypoints_api(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<Vec<codegraph_core::Node>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(entrypoints(&graph)))
}

async fn entrypoint_traces_api(
    State(state): State<AppState>,
    Query(query): Query<EntrypointTraceQuery>,
) -> Result<Json<EntrypointTraceReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(trace_entrypoints(
        &graph,
        EntrypointTraceRequest {
            search: normalize_query_string(query.search),
            max_depth: query.depth.unwrap_or(3).clamp(1, 32),
            limit: query.limit.unwrap_or(25).clamp(1, 500),
        },
    )))
}

async fn insights_api(
    State(state): State<AppState>,
    Query(query): Query<InsightQuery>,
) -> Result<Json<InsightReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = insights(&graph);
    Ok(Json(filter_insight_report(
        report,
        &insight_filter_from_query(query)?,
    )))
}

async fn check_api(
    State(state): State<AppState>,
    Query(query): Query<CheckQuery>,
) -> Result<Json<CheckReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let fail_on = normalize_query_string(query.fail_on)
        .map(|value| parse_insight_severity(&value))
        .transpose()?
        .unwrap_or(InsightSeverity::Error);
    let report = filter_insight_report(
        insights(&graph),
        &InsightFilter {
            severity: None,
            kind: normalize_query_string(query.kind),
            search: normalize_query_string(query.search),
            limit: query.limit.unwrap_or(DEFAULT_INSIGHT_LIMIT),
        },
    );
    Ok(Json(check_insights(report, fail_on)))
}

async fn query_api(
    State(state): State<AppState>,
    Query(query): Query<GraphQuery>,
) -> Result<Json<codegraph_analysis::QueryResult>, ApiError> {
    if query.q.len() > MAX_GRAPH_QUERY_LENGTH {
        return Err(ApiError::bad_request(format!(
            "query expression is too long; maximum is {MAX_GRAPH_QUERY_LENGTH} bytes"
        )));
    }
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let result =
        query_graph(&graph, &query.q).map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(result))
}

async fn explain_edge_api(
    State(state): State<AppState>,
    Query(query): Query<ExplainEdgeQuery>,
) -> Result<Json<Option<codegraph_analysis::EdgeExplanation>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let result = explain_edge(
        &graph,
        ExplainEdgeRequest {
            edge_index: query.edge_index,
            source: normalize_query_string(query.source),
            target: normalize_query_string(query.target),
            kind: normalize_query_string(query.kind),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(result))
}

async fn trace_api(
    State(state): State<AppState>,
    Query(query): Query<TraceQuery>,
) -> Result<Json<Option<codegraph_analysis::TraceResult>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let start = match (query.node_id, query.label) {
        (Some(id), _) => TraceStart::NodeId(codegraph_core::NodeId(id)),
        (None, Some(label)) => TraceStart::Label(label),
        (None, None) => {
            return Err(ApiError::bad_request(
                "trace requires either node_id or label query parameter",
            ));
        }
    };
    Ok(Json(trace(
        &graph,
        TraceRequest {
            start,
            max_depth: query.depth.unwrap_or(2).clamp(1, 8),
        },
    )))
}

async fn dependents_api(
    State(state): State<AppState>,
    Query(query): Query<TraceQuery>,
) -> Result<Json<Option<codegraph_analysis::TraceResult>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let start = match (query.node_id, query.label) {
        (Some(id), _) => TraceStart::NodeId(codegraph_core::NodeId(id)),
        (None, Some(label)) => TraceStart::Label(label),
        (None, None) => {
            return Err(ApiError::bad_request(
                "dependents requires either node_id or label query parameter",
            ));
        }
    };
    Ok(Json(trace_dependents(
        &graph,
        TraceRequest {
            start,
            max_depth: query.depth.unwrap_or(3).clamp(1, 16),
        },
    )))
}

async fn trace_config_api(
    State(state): State<AppState>,
    Query(query): Query<ConfigTraceQuery>,
) -> Result<Json<ConfigTraceResult>, ApiError> {
    let target = query.target.trim().to_string();
    if target.is_empty() {
        return Err(ApiError::bad_request("trace-config requires target"));
    }
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(trace_config(
        &graph,
        ConfigTraceRequest {
            target,
            max_depth: query.depth.unwrap_or(6).clamp(1, 32),
            limit: query.limit.unwrap_or(50).clamp(1, 500),
        },
    )))
}

async fn trace_errors_api(
    State(state): State<AppState>,
    Query(query): Query<ErrorTraceQuery>,
) -> Result<Json<ErrorTraceResult>, ApiError> {
    let target = query.target.trim().to_string();
    if target.is_empty() {
        return Err(ApiError::bad_request("trace-errors requires target"));
    }
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(trace_errors(
        &graph,
        ErrorTraceRequest {
            target,
            max_depth: query.depth.unwrap_or(6).clamp(1, 32),
            limit: query.limit.unwrap_or(50).clamp(1, 500),
        },
    )))
}

async fn source(
    State(state): State<AppState>,
    Query(query): Query<SourceQuery>,
) -> Result<Json<SourcePreview>, ApiError> {
    let source_root = resolve_scan_root(&state, query.root.as_deref())?;
    let path = resolve_path(&state, &source_root, &query.path)?;
    if !path.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }

    let requested_start = query.start_line.unwrap_or(1).max(1);
    let requested_end = query
        .end_line
        .unwrap_or(requested_start)
        .max(requested_start);
    let context = query
        .context
        .unwrap_or(DEFAULT_SOURCE_CONTEXT)
        .min(MAX_SOURCE_CONTEXT);

    let response = tokio::task::spawn_blocking(move || {
        read_source_preview(&source_root, &path, requested_start, requested_end, context)
            .map_err(|error| ApiError::internal(format!("failed to read source: {error}")))
    })
    .await
    .map_err(|error| ApiError::internal(format!("source task failed: {error}")))??;

    Ok(Json(response))
}

async fn source_search_api(
    State(state): State<AppState>,
    Query(query): Query<SourceSearchQuery>,
) -> Result<Json<SourceSearchResult>, ApiError> {
    let search_text = query.q.trim().to_string();
    if search_text.is_empty() {
        return Err(ApiError::bad_request("source-search requires q"));
    }
    if search_text.len() > MAX_SOURCE_SEARCH_QUERY_LENGTH {
        return Err(ApiError::bad_request(format!(
            "source-search query is too long; maximum is {MAX_SOURCE_SEARCH_QUERY_LENGTH} bytes"
        )));
    }
    let search_root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = scan_options(&state, &search_root)?;
    let request = SourceSearchRequest {
        query: search_text,
        path_filter: normalize_query_string(query.path_filter),
        case_sensitive: query.case_sensitive.unwrap_or(false),
        limit: query
            .limit
            .unwrap_or(DEFAULT_SOURCE_SEARCH_LIMIT)
            .clamp(1, MAX_SOURCE_SEARCH_LIMIT),
        context: query
            .context
            .unwrap_or(DEFAULT_SOURCE_SEARCH_CONTEXT)
            .min(MAX_SOURCE_SEARCH_CONTEXT),
        include_hidden: options.include_hidden,
        include_ignored: options.include_ignored,
        max_file_size: options.max_file_size,
        ignored_names: options.ignored_names,
        ignored_globs: options.ignored_globs,
    };
    let result = tokio::task::spawn_blocking(move || search_source(&search_root, &request))
        .await
        .map_err(|error| ApiError::internal(format!("source-search task failed: {error}")))?;
    Ok(Json(result))
}

async fn scan_graph(state: &AppState, requested: Option<&Path>) -> Result<CodeGraph, ApiError> {
    let root = resolve_scan_root(state, requested)?;
    let options = scan_options(state, &root)?;
    let cache = state.cache.clone();
    tokio::task::spawn_blocking(move || scan_project_cached(root, &options, cache.as_ref()))
        .await
        .map_err(|error| ApiError::internal(format!("scanner task failed: {error}")))?
        .map(|output| output.graph)
        .map_err(|error| ApiError::internal(error.to_string()))
}

fn scan_options(state: &AppState, root: &Path) -> Result<IndexOptions, ApiError> {
    configured_index_options(root, &state.option_overrides)
        .map_err(|error| ApiError::internal(error.to_string()))
}

fn resolve_scan_root(state: &AppState, requested: Option<&Path>) -> Result<PathBuf, ApiError> {
    let candidate = requested
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                state.root.join(path)
            }
        })
        .unwrap_or_else(|| state.root.clone());

    resolve_canonical_path(state, candidate)
}

fn resolve_path(state: &AppState, base_root: &Path, requested: &Path) -> Result<PathBuf, ApiError> {
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        base_root.join(requested)
    };
    resolve_canonical_path(state, candidate)
}

fn resolve_canonical_path(state: &AppState, candidate: PathBuf) -> Result<PathBuf, ApiError> {
    let canonical = candidate
        .canonicalize()
        .map_err(|error| ApiError::bad_request(format!("invalid path: {error}")))?;

    if !state.allow_any_path
        && !state
            .projects
            .iter()
            .any(|project| canonical.starts_with(&project.path))
    {
        return Err(ApiError::bad_request(
            "path is outside the configured project roots; restart with --project or --allow-any-path to permit it",
        ));
    }

    Ok(canonical)
}

fn project_roots(root: &Path, additional: Vec<PathBuf>) -> Result<Vec<ProjectRoot>> {
    let mut projects = vec![ProjectRoot {
        name: project_name(root, true),
        path: root.to_path_buf(),
        default: true,
    }];

    for project in additional {
        let canonical = project
            .canonicalize()
            .with_context(|| format!("failed to canonicalize project {}", project.display()))?;
        if projects.iter().any(|existing| existing.path == canonical) {
            continue;
        }
        projects.push(ProjectRoot {
            name: project_name(&canonical, false),
            path: canonical,
            default: false,
        });
    }

    Ok(projects)
}

fn project_name(path: &Path, default: bool) -> String {
    let fallback = if default { "Root" } else { "Project" };
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn normalize_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_optional_job_status(value: Option<&str>) -> Result<Option<ScanJobStatus>, ApiError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(parse_job_status)
        .transpose()
}

fn parse_job_status(value: &str) -> Result<ScanJobStatus, ApiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "queued" => Ok(ScanJobStatus::Queued),
        "running" => Ok(ScanJobStatus::Running),
        "complete" | "completed" => Ok(ScanJobStatus::Complete),
        "failed" => Ok(ScanJobStatus::Failed),
        "canceled" | "cancelled" => Ok(ScanJobStatus::Canceled),
        other => Err(ApiError::bad_request(format!(
            "invalid job status `{other}`; expected queued, running, complete, failed, or canceled"
        ))),
    }
}

fn job_list_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_JOB_LIST_LIMIT)
        .clamp(1, MAX_JOB_LIST_LIMIT)
}

fn sort_scan_jobs_recent_first(jobs: &mut [ScanJob]) {
    jobs.sort_by(|left, right| {
        right
            .updated_at_unix
            .cmp(&left.updated_at_unix)
            .then_with(|| right.created_at_unix.cmp(&left.created_at_unix))
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn sort_semantic_jobs_recent_first(jobs: &mut [SemanticJob]) {
    jobs.sort_by(|left, right| {
        right
            .updated_at_unix
            .cmp(&left.updated_at_unix)
            .then_with(|| right.created_at_unix.cmp(&left.created_at_unix))
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn insight_filter_from_query(query: InsightQuery) -> Result<InsightFilter, ApiError> {
    Ok(InsightFilter {
        severity: normalize_query_string(query.severity)
            .map(|value| parse_insight_severity(&value))
            .transpose()?,
        kind: normalize_query_string(query.kind),
        search: normalize_query_string(query.search),
        limit: query.limit.unwrap_or(DEFAULT_INSIGHT_LIMIT),
    })
}

fn project_report_limits_from_query(
    query: &ProjectReportQuery,
) -> Result<ProjectReportLimits, ApiError> {
    Ok(ProjectReportLimits {
        architecture_group_limit: query
            .architecture_group_limit
            .unwrap_or(DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT)
            .clamp(1, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT),
        architecture_edge_limit: query
            .architecture_edge_limit
            .unwrap_or(DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT)
            .clamp(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT),
        language_link_limit: query
            .language_link_limit
            .unwrap_or(DEFAULT_REPORT_LANGUAGE_LINK_LIMIT)
            .clamp(1, MAX_REPORT_LANGUAGE_LINK_LIMIT),
        hotspot_limit: query
            .hotspot_limit
            .unwrap_or(DEFAULT_REPORT_HOTSPOT_LIMIT)
            .clamp(1, MAX_REPORT_HOTSPOT_LIMIT),
        insight_limit: query
            .insight_limit
            .unwrap_or(DEFAULT_REPORT_INSIGHT_LIMIT)
            .clamp(1, MAX_REPORT_INSIGHT_LIMIT),
        fail_on: normalize_query_string(query.fail_on.clone())
            .map(|value| parse_insight_severity(&value))
            .transpose()?
            .unwrap_or(InsightSeverity::Error),
    })
}

fn parse_insight_severity(value: &str) -> Result<InsightSeverity, ApiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "info" => Ok(InsightSeverity::Info),
        "warning" | "warn" => Ok(InsightSeverity::Warning),
        "error" => Ok(InsightSeverity::Error),
        other => Err(ApiError::bad_request(format!(
            "invalid severity `{other}`; expected info, warning, or error"
        ))),
    }
}

fn parse_node_ids(value: Option<&str>) -> Result<Vec<codegraph_core::NodeId>, ApiError> {
    parse_u64_list(value, "node_ids")
        .map(|ids| ids.into_iter().map(codegraph_core::NodeId).collect())
}

fn parse_edge_indexes(value: Option<&str>) -> Result<Vec<usize>, ApiError> {
    parse_usize_list(value, "edge_indexes")
}

fn parse_u64_list(value: Option<&str>, name: &str) -> Result<Vec<u64>, ApiError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<u64>()
                .map_err(|_| ApiError::bad_request(format!("invalid {name} value `{part}`")))
        })
        .collect()
}

fn parse_usize_list(value: Option<&str>, name: &str) -> Result<Vec<usize>, ApiError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<usize>()
                .map_err(|_| ApiError::bad_request(format!("invalid {name} value `{part}`")))
        })
        .collect()
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: "not found".to_string(),
            request_id: current_request_id(),
        }),
    )
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
struct ScanJobUpdate {
    status: ScanJobStatus,
    message: String,
    cache: Option<CacheInfo>,
    summary: Option<codegraph_analysis::GraphSummary>,
    graph: Option<CodeGraph>,
}

#[derive(Debug)]
struct SemanticJobUpdate {
    status: ScanJobStatus,
    message: String,
    result: Option<SemanticEnrichResponse>,
}

async fn insert_scan_job(jobs: &RwLock<BTreeMap<String, ScanJob>>, job: ScanJob, max_jobs: usize) {
    let mut jobs = jobs.write().await;
    jobs.insert(job.id.clone(), job);
    prune_scan_jobs(&mut jobs, max_jobs);
}

async fn update_scan_job(
    jobs: &RwLock<BTreeMap<String, ScanJob>>,
    id: &str,
    update: ScanJobUpdate,
    max_jobs: usize,
) {
    let mut jobs = jobs.write().await;
    if let Some(job) = jobs.get_mut(id) {
        if job.status == ScanJobStatus::Canceled && update.status != ScanJobStatus::Canceled {
            return;
        }
        let now = unix_seconds();
        job.status = update.status;
        job.message = update.message;
        job.updated_at_unix = now;
        if update.status.is_terminal() {
            job.finished_at_unix = Some(now);
        }
        job.cache = update.cache;
        job.summary = update.summary;
        job.graph = update.graph;
    }
    prune_scan_jobs(&mut jobs, max_jobs);
}

async fn cancel_scan_job_in_store(
    jobs: &RwLock<BTreeMap<String, ScanJob>>,
    id: &str,
    max_jobs: usize,
) -> Result<ScanJob, ApiError> {
    let mut jobs = jobs.write().await;
    let job = jobs
        .get_mut(id)
        .ok_or_else(|| ApiError::not_found("scan job not found"))?;
    match job.status {
        ScanJobStatus::Queued | ScanJobStatus::Running => {
            let now = unix_seconds();
            job.status = ScanJobStatus::Canceled;
            job.message = "canceled".to_string();
            job.updated_at_unix = now;
            job.finished_at_unix = Some(now);
            job.cache = None;
            job.summary = None;
            job.graph = None;
        }
        ScanJobStatus::Canceled => {}
        ScanJobStatus::Complete | ScanJobStatus::Failed => {
            return Err(ApiError::bad_request("scan job is already complete"));
        }
    }
    let job = job.clone();
    prune_scan_jobs(&mut jobs, max_jobs);
    Ok(job)
}

async fn scan_job_is_canceled(jobs: &RwLock<BTreeMap<String, ScanJob>>, id: &str) -> bool {
    jobs.read()
        .await
        .get(id)
        .is_some_and(|job| job.status == ScanJobStatus::Canceled)
}

fn prune_scan_jobs(jobs: &mut BTreeMap<String, ScanJob>, max_jobs: usize) {
    let max_jobs = max_jobs.max(1);
    while jobs.len() > max_jobs {
        let remove_id = jobs
            .iter()
            .filter(|(_, job)| job.status.is_terminal())
            .min_by_key(|(_, job)| {
                (
                    job.finished_at_unix.unwrap_or(job.updated_at_unix),
                    job.updated_at_unix,
                    job.created_at_unix,
                )
            })
            .map(|(id, _)| id.clone());

        let Some(remove_id) = remove_id else {
            break;
        };
        jobs.remove(&remove_id);
    }
}

fn run_semantic_enrichment(
    root: PathBuf,
    graph: CodeGraph,
    request: SemanticEnrichRequest,
    cache: Option<SemanticLspCache>,
) -> Result<SemanticEnrichResponse, codegraph_lsp::SemanticLspRunError> {
    let timeout = Duration::from_millis(normalize_semantic_request_timeout_ms(
        request
            .request_timeout_ms
            .unwrap_or(DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS),
    ));
    let batch = semantic_execution_batch(
        &root,
        &graph,
        request
            .work_item_limit
            .unwrap_or(DEFAULT_SEMANTIC_WORK_ITEM_LIMIT),
        SemanticWorkItemFilter {
            language: request.work_language,
            status: request.work_status,
            capability: request.work_capability,
        },
    );
    let cached_run = run_semantic_execution_batch_cached(
        cache.as_ref(),
        &batch,
        &SemanticLspRunOptions {
            request_timeout: timeout,
        },
    )?;
    let responses = cached_run.responses;
    let patch = semantic_graph_patch_from_responses(&root, &graph, &batch, &responses);
    let response_errors = patch.response_errors.len();
    let unmatched_locations = patch.unmatched_locations.len();
    let apply_result = apply_semantic_graph_patch(&graph, &patch);
    let summary = summarize(&apply_result.graph);
    Ok(SemanticEnrichResponse {
        graph: apply_result.graph,
        summary,
        report: apply_result.report,
        responses: responses.len(),
        response_errors,
        unmatched_locations,
        semantic_cache: cached_run.cache,
    })
}

async fn insert_semantic_job(
    jobs: &RwLock<BTreeMap<String, SemanticJob>>,
    job: SemanticJob,
    max_jobs: usize,
) {
    let mut jobs = jobs.write().await;
    jobs.insert(job.id.clone(), job);
    prune_semantic_jobs(&mut jobs, max_jobs);
}

async fn update_semantic_job(
    jobs: &RwLock<BTreeMap<String, SemanticJob>>,
    id: &str,
    update: SemanticJobUpdate,
    max_jobs: usize,
) {
    let mut jobs = jobs.write().await;
    if let Some(job) = jobs.get_mut(id) {
        if job.status == ScanJobStatus::Canceled && update.status != ScanJobStatus::Canceled {
            return;
        }
        let now = unix_seconds();
        job.status = update.status;
        job.message = update.message;
        job.updated_at_unix = now;
        if update.status.is_terminal() {
            job.finished_at_unix = Some(now);
        }
        if let Some(result) = update.result {
            job.responses = Some(result.responses);
            job.response_errors = Some(result.response_errors);
            job.unmatched_locations = Some(result.unmatched_locations);
            job.report = Some(result.report.clone());
            job.result = Some(result);
        }
    }
    prune_semantic_jobs(&mut jobs, max_jobs);
}

async fn cancel_semantic_job_in_store(
    jobs: &RwLock<BTreeMap<String, SemanticJob>>,
    id: &str,
    max_jobs: usize,
) -> Result<SemanticJob, ApiError> {
    let mut jobs = jobs.write().await;
    let job = jobs
        .get_mut(id)
        .ok_or_else(|| ApiError::not_found("semantic job not found"))?;
    match job.status {
        ScanJobStatus::Queued | ScanJobStatus::Running => {
            let now = unix_seconds();
            job.status = ScanJobStatus::Canceled;
            job.message = "canceled".to_string();
            job.updated_at_unix = now;
            job.finished_at_unix = Some(now);
            job.responses = None;
            job.response_errors = None;
            job.unmatched_locations = None;
            job.report = None;
            job.result = None;
        }
        ScanJobStatus::Canceled => {}
        ScanJobStatus::Complete | ScanJobStatus::Failed => {
            return Err(ApiError::bad_request("semantic job is already complete"));
        }
    }
    let job = job.clone();
    prune_semantic_jobs(&mut jobs, max_jobs);
    Ok(job)
}

async fn semantic_job_is_canceled(jobs: &RwLock<BTreeMap<String, SemanticJob>>, id: &str) -> bool {
    jobs.read()
        .await
        .get(id)
        .is_some_and(|job| job.status == ScanJobStatus::Canceled)
}

fn prune_semantic_jobs(jobs: &mut BTreeMap<String, SemanticJob>, max_jobs: usize) {
    let max_jobs = max_jobs.max(1);
    while jobs.len() > max_jobs {
        let remove_id = jobs
            .iter()
            .filter(|(_, job)| job.status.is_terminal())
            .min_by_key(|(_, job)| {
                (
                    job.finished_at_unix.unwrap_or(job.updated_at_unix),
                    job.updated_at_unix,
                    job.created_at_unix,
                )
            })
            .map(|(id, _)| id.clone());

        let Some(remove_id) = remove_id else {
            break;
        };
        jobs.remove(&remove_id);
    }
}

fn project_responses(state: &AppState) -> Vec<ProjectResponse> {
    state
        .projects
        .iter()
        .map(|project| ProjectResponse {
            name: project.name.clone(),
            path: project.path.display().to_string(),
            default: project.default,
        })
        .collect()
}

fn language_responses() -> Vec<LanguageResponse> {
    language_adapters()
        .iter()
        .map(|adapter| {
            let info = adapter.info();
            LanguageResponse {
                language: info.language,
                parser: info.parser,
                extensions: info.extensions,
                file_names: info.file_names,
            }
        })
        .collect()
}

fn api_schema_response() -> ApiSchemaResponse {
    ApiSchemaResponse {
        name: "CodeGraph API",
        server_version: SERVER_VERSION,
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        description: "Machine-readable API contract for CodeGraph clients and agents.",
        common_response_headers: api_schema_common_response_headers(),
        groups: api_schema_groups(),
        enum_values: BTreeMap::from([
            ("export_format", vec!["json", "dot", "ndjson"]),
            (
                "graph_node_kind",
                vec![
                    "repository",
                    "directory",
                    "file",
                    "module",
                    "function",
                    "entrypoint",
                    "type",
                    "config",
                    "environment",
                    "external_dependency",
                    "unknown",
                ],
            ),
            (
                "graph_edge_kind",
                vec![
                    "contains",
                    "imports",
                    "calls",
                    "defines",
                    "references",
                    "reads_config",
                    "reads_environment",
                    "may_error",
                    "entrypoint",
                    "depends_on",
                ],
            ),
            (
                "graph_confidence",
                vec!["exact", "semantic", "syntactic", "heuristic", "unknown"],
            ),
            (
                "job_status",
                vec!["queued", "running", "complete", "failed", "canceled"],
            ),
            ("cache_status", vec!["disabled", "hit", "miss"]),
            (
                "cache_record_status",
                vec!["missing", "present", "incompatible"],
            ),
            (
                "cache_reuse_strategy",
                vec!["full_scan", "partial_reuse", "no_changes"],
            ),
            (
                "incremental_plan_action",
                vec!["full_scan", "partial_rescan", "noop"],
            ),
            (
                "incremental_merge_blocker_kind",
                vec![
                    "removed_paths",
                    "incoming_cross_file_edges",
                    "graph_surface_added",
                    "graph_surface_removed",
                ],
            ),
            ("insight_severity", vec!["info", "warning", "error"]),
            ("insight_kind", KNOWN_INSIGHT_KINDS.to_vec()),
            (
                "risk_grade",
                vec!["clean", "low", "medium", "high", "critical"],
            ),
            (
                "project_report_section",
                vec![
                    "summary",
                    "entrypoints",
                    "insights",
                    "risk_summary",
                    "quality_gate",
                    "architecture",
                    "language_dependencies",
                    "hotspots",
                    "cache",
                    "coverage",
                ],
            ),
            (
                "semantic_work_status",
                vec!["ready", "missing_server", "unsupported_language"],
            ),
            (
                "semantic_work_capability",
                vec![
                    "definitions",
                    "diagnostics",
                    "document_symbols",
                    "workspace_symbols",
                    "references",
                    "language_server",
                ],
            ),
            (
                "graph_query_command",
                vec![
                    "nodes",
                    "edges",
                    "calls",
                    "dependencies",
                    "trace",
                    "dependents",
                    "neighbors",
                    "symbols",
                    "files",
                    "entrypoints",
                    "routes",
                    "packages",
                    "configs",
                    "errors",
                    "cycles",
                    "hotspots",
                    "unreachable",
                    "diagnostics",
                    "annotations",
                    "insights",
                    "path",
                ],
            ),
            (
                "graph_query_node_term",
                vec![
                    "id",
                    "kind",
                    "label",
                    "search",
                    "language",
                    "item_kind",
                    "package_id",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_edge_term",
                vec![
                    "edge",
                    "edge_index",
                    "kind",
                    "source",
                    "target",
                    "confidence",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_cycle_term",
                vec![
                    "id",
                    "node",
                    "node_id",
                    "label",
                    "search",
                    "language",
                    "path",
                    "path_prefix",
                    "kind",
                    "edge_kind",
                ],
            ),
            (
                "graph_query_error_term",
                vec![
                    "id",
                    "node_id",
                    "target",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "item_kind",
                    "path",
                    "path_prefix",
                    "depth",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_hotspot_term",
                vec![
                    "id",
                    "node",
                    "node_id",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "node_kind",
                    "item_kind",
                    "path",
                    "path_prefix",
                    "min_score",
                    "min_degree",
                    "score",
                    "edge_kind",
                    "confidence",
                    "direction",
                    "dir",
                    "edge_limit",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_unreachable_term",
                vec![
                    "id",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "item_kind",
                    "package_id",
                    "path_prefix",
                    "scope",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_unreachable_scope",
                vec!["source_files", "config", "errors", "any"],
            ),
            (
                "graph_query_symbol_term",
                vec![
                    "id",
                    "node",
                    "node_id",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "node_kind",
                    "item_kind",
                    "path",
                    "path_prefix",
                    "direction",
                    "dir",
                    "edge_kind",
                    "confidence",
                    "edge_limit",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_file_term",
                vec![
                    "id",
                    "node",
                    "node_id",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "node_kind",
                    "item_kind",
                    "path",
                    "source_path",
                    "file",
                    "file_path",
                    "path_prefix",
                    "direction",
                    "dir",
                    "edge_kind",
                    "confidence",
                    "edge_limit",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_config_term",
                vec![
                    "id",
                    "node_id",
                    "target",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "item_kind",
                    "path",
                    "path_prefix",
                    "depth",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_entrypoint_term",
                vec![
                    "id",
                    "node_id",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "item_kind",
                    "entrypoint_kind",
                    "path",
                    "path_prefix",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_route_term",
                vec![
                    "id",
                    "node_id",
                    "label",
                    "search",
                    "language",
                    "framework",
                    "method",
                    "route_method",
                    "http_method",
                    "path",
                    "route_path",
                    "url",
                    "handler",
                    "source_path",
                    "file",
                    "file_path",
                    "path_prefix",
                    "depth",
                    "edge_limit",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_package_term",
                vec![
                    "id",
                    "node_id",
                    "label",
                    "search",
                    "package",
                    "package_id",
                    "ecosystem",
                    "language",
                    "kind",
                    "item_kind",
                    "source",
                    "dependency_source",
                    "dependency_kind",
                    "version",
                    "dependency_version",
                    "path",
                    "source_path",
                    "file",
                    "file_path",
                    "path_prefix",
                    "edge_kind",
                    "kind_edge",
                    "confidence",
                    "edge_limit",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_diagnostic_term",
                vec![
                    "id",
                    "label",
                    "message",
                    "severity",
                    "source",
                    "diagnostic_source",
                    "code",
                    "diagnostic_code",
                    "path",
                    "path_prefix",
                    "language",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_annotation_term",
                vec![
                    "id",
                    "node",
                    "node_id",
                    "label",
                    "search",
                    "key",
                    "annotation",
                    "annotation_key",
                    "value",
                    "annotation_value",
                    "language",
                    "kind",
                    "node_kind",
                    "item_kind",
                    "path",
                    "path_prefix",
                    "direction",
                    "dir",
                    "edge_kind",
                    "confidence",
                    "edge_limit",
                    "metadata.*",
                    "annotation.*",
                ],
            ),
            (
                "graph_query_insight_term",
                vec![
                    "severity",
                    "kind",
                    "message",
                    "search",
                    "node",
                    "node_id",
                    "id",
                    "edge",
                    "edge_index",
                    "path",
                    "path_prefix",
                    "language",
                ],
            ),
            (
                "web_deep_link_param",
                vec![
                    "path",
                    "node",
                    "edge",
                    "query",
                    "query_focus",
                    "node_offset",
                    "node_limit",
                    "edge_offset",
                    "edge_limit",
                    "path_prefix",
                    "kind",
                    "item_kind",
                    "language",
                    "search",
                    "edge_kind",
                    "confidence",
                    "edge_relation",
                    "edge_source",
                ],
            ),
        ]),
    }
}

fn api_schema_common_response_headers() -> Vec<ApiHeaderSpec> {
    vec![
        ApiHeaderSpec {
            name: "x-request-id",
            value_type: "string",
            required: true,
            description: "Per-response request correlation id; safe client-provided x-request-id values are echoed.",
        },
        ApiHeaderSpec {
            name: RESPONSE_TIME_HEADER,
            value_type: "u64_ms",
            required: true,
            description: "Server-side response time in elapsed whole milliseconds.",
        },
        ApiHeaderSpec {
            name: "cache-control",
            value_type: "string",
            required: true,
            description: "Runtime/API responses use no-store; embedded unversioned static assets use no-cache revalidation.",
        },
        ApiHeaderSpec {
            name: "content-security-policy",
            value_type: "string",
            required: true,
            description: "Server-wide content security policy for the embedded web UI and API responses.",
        },
        ApiHeaderSpec {
            name: "x-content-type-options",
            value_type: "string",
            required: true,
            description: "Always nosniff.",
        },
        ApiHeaderSpec {
            name: "x-frame-options",
            value_type: "string",
            required: true,
            description: "Always DENY.",
        },
        ApiHeaderSpec {
            name: "referrer-policy",
            value_type: "string",
            required: true,
            description: "Always no-referrer.",
        },
        ApiHeaderSpec {
            name: "permissions-policy",
            value_type: "string",
            required: true,
            description: "Disables camera, microphone, geolocation, and payment APIs.",
        },
        ApiHeaderSpec {
            name: "etag",
            value_type: "http_etag",
            required: false,
            description: "Present on embedded static assets and supports If-None-Match revalidation.",
        },
    ]
}

fn export_response_headers() -> Vec<ApiHeaderSpec> {
    vec![
        ApiHeaderSpec {
            name: EXPORT_NODES_HEADER,
            value_type: "usize",
            required: true,
            description: "Total graph nodes included in the full export response.",
        },
        ApiHeaderSpec {
            name: EXPORT_EDGES_HEADER,
            value_type: "usize",
            required: true,
            description: "Total graph edges included in the full export response.",
        },
        ApiHeaderSpec {
            name: EXPORT_BYTES_HEADER,
            value_type: "usize_bytes",
            required: true,
            description: "Serialized export body size in bytes.",
        },
    ]
}

fn api_schema_groups() -> Vec<ApiSchemaGroup> {
    vec![
        ApiSchemaGroup {
            group: "system",
            endpoints: vec![
                api_get(
                    "/api/capabilities",
                    "Discover server features, limits, route groups, cache state, and configured projects.",
                    vec![],
                    "CapabilitiesResponse",
                )
                .with_response_fields(capabilities_response_fields()),
                api_get(
                    "/api/schema",
                    "Discover this machine-readable endpoint contract.",
                    vec![],
                    "ApiSchemaResponse",
                )
                .with_response_fields(api_schema_response_fields()),
                api_get(
                    "/api/live",
                    "Read a lightweight liveness probe for process supervision.",
                    vec![],
                    "ProbeResponse",
                )
                .with_response_fields(probe_response_fields()),
                api_get(
                    "/api/ready",
                    "Read a lightweight readiness probe that validates server scan configuration.",
                    vec![],
                    "ProbeResponse",
                )
                .with_response_fields(probe_response_fields()),
                api_get(
                    "/api/health",
                    "Read runtime health, retained job-store counts, and concurrency slots.",
                    vec![],
                    "HealthResponse",
                )
                .with_response_fields(health_response_fields()),
                api_get(
                    "/api/metrics",
                    "Read runtime metrics including versions, cache state, job stores, and concurrency.",
                    vec![],
                    "MetricsResponse",
                )
                .with_response_fields(metrics_response_fields()),
                api_get(
                    "/api/projects",
                    "List configured project roots available to the server.",
                    vec![],
                    "ProjectResponse[]",
                ),
                api_get(
                    "/api/languages",
                    "List built-in language adapters and detection patterns.",
                    vec![],
                    "LanguageResponse[]",
                ),
                api_get(
                    "/api/lsp",
                    "List semantic language server discovery results.",
                    vec![],
                    "LspDiscoveryReport",
                ),
                api_get(
                    "/api/scan-options",
                    "Show effective scan policy for a project root.",
                    vec![path_param()],
                    "ScanOptionsResponse",
                ),
            ],
        },
        ApiSchemaGroup {
            group: "scan",
            endpoints: vec![
                api_get(
                    "/api/scan",
                    "Scan a project and return the full graph plus cache status.",
                    vec![path_param()],
                    "ScanResponse",
                ),
                api_get(
                    "/api/coverage",
                    "Explain scan coverage, indexed files, skip counts, and language counts.",
                    vec![path_param()],
                    "ScanCoverageReport",
                ),
                api_get(
                    "/api/cache-diff",
                    "Explain cache fingerprint changes and reusable file/byte estimates without a full graph scan.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum changed entries per list.",
                        )
                        .with_range(1, MAX_INCREMENTAL_REPORT_LIMIT)
                        .with_capability_limit("max_incremental_report_limit"),
                    ],
                    "CacheDiffReport",
                ),
                api_get(
                    "/api/cache-chunks",
                    "List persistent per-file graph chunk scopes from the graph cache.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum chunk entries to return.",
                        )
                        .with_range(1, MAX_INCREMENTAL_REPORT_LIMIT)
                        .with_capability_limit("max_incremental_report_limit"),
                    ],
                    "CacheChunkReport",
                ),
                api_get(
                    "/api/incremental-plan",
                    "Plan incremental scan work and impacted cached graph nodes/edges from the persistent cache fingerprint without scanning the full graph.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum paths per plan list.",
                        )
                        .with_range(1, MAX_INCREMENTAL_REPORT_LIMIT)
                        .with_capability_limit("max_incremental_report_limit"),
                    ],
                    "IncrementalScanPlan",
                ),
                api_get(
                    "/api/incremental-scan",
                    "Scan only the changed current files from the incremental cache plan, returning a changed-scope graph and the plan used.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum changed current paths to include in the focused scan.",
                        )
                        .with_range(1, MAX_INCREMENTAL_REPORT_LIMIT)
                        .with_capability_limit("max_incremental_report_limit"),
                    ],
                    "IncrementalScan",
                ),
                api_get(
                    "/api/incremental-merge-preview",
                    "Preview a graph assembled from cached unchanged files plus changed-file rescans.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum changed current paths to include in the merge preview.",
                        )
                        .with_range(1, MAX_INCREMENTAL_REPORT_LIMIT)
                        .with_capability_limit("max_incremental_report_limit"),
                    ],
                    "IncrementalMergePreview",
                ),
                api_post(
                    "/api/incremental-update",
                    "Update the persistent graph cache when the incremental result is complete; incomplete partial previews are reported but not stored.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum changed paths to inspect while planning the update.",
                        )
                        .with_range(1, MAX_INCREMENTAL_REPORT_LIMIT)
                        .with_capability_limit("max_incremental_report_limit"),
                    ],
                    None,
                    "IncrementalUpdate",
                    false,
                ),
                api_get(
                    "/api/incremental-update",
                    "Legacy compatibility alias for POST /api/incremental-update.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum changed paths to inspect while planning the update.",
                        )
                        .with_range(1, MAX_INCREMENTAL_REPORT_LIMIT)
                        .with_capability_limit("max_incremental_report_limit"),
                    ],
                    "IncrementalUpdate",
                ),
                api_post(
                    "/api/scan-jobs",
                    "Queue a long-running scan job.",
                    vec![],
                    Some("ScanJobRequest { path?: string }"),
                    "ScanJob",
                    false,
                )
                .with_body_fields(scan_job_body_fields()),
                api_get(
                    "/api/scan-jobs",
                    "List retained scan jobs, optionally filtered by status.",
                    vec![
                        job_status_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("50"),
                            "Maximum jobs to return.",
                        )
                        .with_range(1, MAX_JOB_LIST_LIMIT)
                        .with_capability_limit("max_job_list_limit"),
                    ],
                    "ScanJobListResponse",
                ),
                api_get(
                    "/api/scan-jobs/{id}",
                    "Read retained scan job status by id.",
                    vec![id_param()],
                    "ScanJob",
                ),
                api_delete(
                    "/api/scan-jobs/{id}",
                    "Cancel a queued or running scan job.",
                    vec![id_param()],
                    "ScanJob",
                ),
                api_get_stream(
                    "/api/scan-jobs/{id}/events",
                    "Stream scan job status events with server-sent events.",
                    vec![id_param()],
                    "SSE<ScanJob>",
                ),
                api_get(
                    "/api/scan-jobs/{id}/result",
                    "Return graph result for a completed scan job.",
                    vec![id_param()],
                    "ScanJobResult",
                ),
            ],
        },
        ApiSchemaGroup {
            group: "semantic",
            endpoints: vec![
                api_get(
                    "/api/semantic-readiness",
                    "Report language coverage by available semantic language servers.",
                    vec![path_param()],
                    "SemanticReadinessReport",
                ),
                api_get(
                    "/api/semantic-plan",
                    "Plan semantic LSP work with optional work item filters.",
                    semantic_filter_params(),
                    "SemanticEnrichmentPlan",
                ),
                api_get(
                    "/api/semantic-batch",
                    "Group semantic LSP work into executable language-server batches.",
                    semantic_filter_params(),
                    "SemanticExecutionBatch",
                ),
                api_post(
                    "/api/semantic-patch",
                    "Map semantic LSP responses into graph patch operations.",
                    vec![],
                    Some("SemanticPatchRequest"),
                    "SemanticGraphPatch",
                    false,
                )
                .with_body_fields(semantic_patch_body_fields()),
                api_post(
                    "/api/semantic-apply",
                    "Apply semantic LSP responses and return enriched graph plus report.",
                    vec![],
                    Some("SemanticPatchRequest"),
                    "SemanticGraphApplyResult",
                    false,
                )
                .with_body_fields(semantic_patch_body_fields()),
                api_post(
                    "/api/semantic-enrich",
                    "Run ready semantic LSP work synchronously and return enriched graph plus report.",
                    vec![],
                    Some("SemanticEnrichRequest"),
                    "SemanticEnrichResponse",
                    false,
                )
                .with_body_fields(semantic_enrich_body_fields()),
                api_post(
                    "/api/semantic-jobs",
                    "Queue semantic enrichment as a retained async job.",
                    vec![],
                    Some("SemanticEnrichRequest"),
                    "SemanticJob",
                    false,
                )
                .with_body_fields(semantic_enrich_body_fields()),
                api_get(
                    "/api/semantic-jobs",
                    "List retained semantic jobs, optionally filtered by status.",
                    vec![
                        job_status_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("50"),
                            "Maximum jobs to return.",
                        )
                        .with_range(1, MAX_JOB_LIST_LIMIT)
                        .with_capability_limit("max_job_list_limit"),
                    ],
                    "SemanticJobListResponse",
                ),
                api_get(
                    "/api/semantic-jobs/{id}",
                    "Read retained semantic job status by id.",
                    vec![id_param()],
                    "SemanticJob",
                ),
                api_delete(
                    "/api/semantic-jobs/{id}",
                    "Cancel a queued or running semantic job.",
                    vec![id_param()],
                    "SemanticJob",
                ),
                api_get_stream(
                    "/api/semantic-jobs/{id}/events",
                    "Stream semantic job status events with server-sent events.",
                    vec![id_param()],
                    "SSE<SemanticJob>",
                ),
                api_get(
                    "/api/semantic-jobs/{id}/result",
                    "Return enriched graph result for a completed semantic job.",
                    vec![id_param()],
                    "SemanticJobResult",
                ),
            ],
        },
        ApiSchemaGroup {
            group: "graph",
            endpoints: vec![
                api_get(
                    "/api/export",
                    "Export a full graph as JSON, DOT, or NDJSON.",
                    vec![
                        path_param(),
                        query_param(
                            "format",
                            false,
                            "export_format",
                            Some("json"),
                            "Export format.",
                        ),
                    ],
                    "CodeGraph | DOT | NDJSON",
                )
                .with_response_headers(export_response_headers()),
                api_get(
                    "/api/graph",
                    "Read a server-side paged and filtered graph slice. Returned edges include metadata.edge_index for exact edge explanation and UI selection.",
                    graph_slice_params(),
                    "GraphSlice",
                )
                .with_response_fields(graph_slice_response_fields()),
                api_get(
                    "/api/node-context",
                    "Read selected node context with neighboring edges. Returned edges include metadata.edge_index for exact edge explanation and UI selection.",
                    vec![
                        path_param(),
                        query_param("node_id", true, "u64", None, "Node numeric id."),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("80"),
                            "Maximum context edges.",
                        )
                        .with_range(1, MAX_NODE_CONTEXT_EDGE_LIMIT)
                        .with_capability_limit("max_node_context_edge_limit"),
                    ],
                    "NodeContext",
                )
                .with_response_fields(node_context_response_fields()),
                api_get(
                    "/api/node-card",
                    "Read selected node investigation card with neighboring edges, dependency summary facets, file-level summaries, source preview, related risks including file-scoped contained-node risks, risk summaries, exact edge indexes, and suggested focused graph query actions.",
                    vec![
                        path_param(),
                        query_param("node_id", true, "u64", None, "Node numeric id."),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("80"),
                            "Maximum context edges.",
                        )
                        .with_range(1, MAX_NODE_CONTEXT_EDGE_LIMIT)
                        .with_capability_limit("max_node_context_edge_limit"),
                        query_param(
                            "source_context",
                            false,
                            "u32",
                            Some("5"),
                            "Source context lines around the node span.",
                        )
                        .with_range(0, MAX_NODE_CARD_SOURCE_CONTEXT as usize)
                        .with_capability_limit("max_node_card_source_context"),
                        query_param(
                            "insight_limit",
                            false,
                            "usize",
                            Some("8"),
                            "Maximum related risks.",
                        )
                        .with_range(1, MAX_NODE_CARD_INSIGHT_LIMIT)
                        .with_capability_limit("max_node_card_insight_limit"),
                    ],
                    "NodeCard",
                )
                .with_response_fields(node_card_response_fields()),
                api_get(
                    "/api/focus",
                    "Build a focused subgraph from node ids and edge indexes. Returned edges include metadata.edge_index for exact edge explanation and UI selection.",
                    vec![
                        path_param(),
                        query_param("node_ids", false, "csv<u64>", None, "Node ids to include."),
                        query_param(
                            "edge_indexes",
                            false,
                            "csv<usize>",
                            None,
                            "Edge indexes to include.",
                        ),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("200"),
                            "Maximum incident edges.",
                        )
                        .with_range(1, MAX_FOCUS_EDGE_LIMIT)
                        .with_capability_limit("max_focus_edge_limit"),
                    ],
                    "QueryResult",
                )
                .with_response_fields(query_result_response_fields()),
                api_get(
                    "/api/summary",
                    "Summarize graph node/edge facts and facets.",
                    vec![path_param()],
                    "GraphSummary",
                ),
                api_get(
                    "/api/query",
                    "Run a focused graph query expression such as nodes, edges, calls, neighbors, path, dependents, symbols, files, entrypoints, routes, packages, configs, errors, cycles, hotspots, unreachable, diagnostics, annotations, or insights. QueryResult includes returned counts, edge metadata.edge_index values, and facets for node kinds, edge kinds, languages, item kinds, and confidence.",
                    vec![
                        path_param(),
                        query_param(
                            "q",
                            true,
                            "string",
                            None,
                            "Graph query expression, for example `symbols label:load_config direction:out`, `files path:src/main.rs direction:out`, `entrypoints language:rust`, `routes method:GET path:/users`, `packages package:serde ecosystem:cargo`, `configs target:DATABASE_URL`, `errors target:panic`, `cycles edge_kind:calls`, `hotspots language:rust min_score:5`, `unreachable scope:errors search:LegacyError`, `diagnostics severity:error language:rust`, `annotations key:domain value:payments`, or `insights severity:error`.",
                        )
                        .with_max_length(MAX_GRAPH_QUERY_LENGTH)
                        .with_capability_limit("max_graph_query_length"),
                    ],
                    "QueryResult",
                )
                .with_response_fields(query_result_response_fields()),
                api_get(
                    "/api/explain-edge",
                    "Explain why an edge exists with confidence, provenance evidence, and related edge-scoped risk findings.",
                    vec![
                        path_param(),
                        query_param("edge_index", false, "usize", None, "Exact edge index."),
                        query_param(
                            "source",
                            false,
                            "string",
                            None,
                            "Source id or label substring.",
                        ),
                        query_param(
                            "target",
                            false,
                            "string",
                            None,
                            "Target id or label substring.",
                        ),
                        query_param("kind", false, "string", None, "Edge kind substring."),
                    ],
                    "EdgeExplanation?",
                )
                .with_response_fields(edge_explanation_response_fields()),
            ],
        },
        ApiSchemaGroup {
            group: "analysis",
            endpoints: vec![
                api_get(
                    "/api/report",
                    "Return a production project report snapshot with cache, coverage, summary, full-project risk scoring, quality gate, topology, and hotspots.",
                    report_params(),
                    "ProjectReportResponse",
                )
                .with_response_fields(project_report_response_fields()),
                api_get(
                    "/api/architecture",
                    "Group files and cross-area dependencies by top-level project area.",
                    vec![
                        path_param(),
                        query_param("group_limit", false, "usize", Some("50"), "Maximum groups.")
                            .with_range(1, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT)
                            .with_capability_limit("max_report_architecture_group_limit"),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("200"),
                            "Maximum inter-group edges.",
                        )
                        .with_range(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT)
                        .with_capability_limit("max_report_architecture_edge_limit"),
                    ],
                    "ArchitectureMap",
                )
                .with_response_fields(architecture_map_response_fields()),
                api_get(
                    "/api/language-dependencies",
                    "Summarize mixed-language dependency links.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("50"),
                            "Maximum language links.",
                        )
                        .with_range(1, MAX_REPORT_LANGUAGE_LINK_LIMIT)
                        .with_capability_limit("max_report_language_link_limit"),
                    ],
                    "LanguageDependencyReport",
                )
                .with_response_fields(language_dependency_response_fields()),
                api_get(
                    "/api/hotspots",
                    "List high-degree files, functions, entrypoints, and config nodes.",
                    vec![
                        path_param(),
                        query_param("limit", false, "usize", Some("25"), "Maximum hotspots.")
                            .with_range(1, MAX_REPORT_HOTSPOT_LIMIT)
                            .with_capability_limit("max_report_hotspot_limit"),
                    ],
                    "HotspotReport",
                )
                .with_response_fields(hotspot_response_fields()),
                api_get(
                    "/api/entrypoints",
                    "List detected entrypoint candidate nodes.",
                    vec![path_param()],
                    "Node[]",
                ),
                api_get(
                    "/api/entrypoint-traces",
                    "Trace outgoing dependency flows from entrypoints.",
                    vec![
                        path_param(),
                        query_param(
                            "search",
                            false,
                            "string",
                            None,
                            "Filter entrypoints by label/kind/language/metadata.",
                        ),
                        query_param("depth", false, "usize", Some("3"), "Maximum trace depth.")
                            .with_range(1, 32),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("25"),
                            "Maximum entrypoint traces.",
                        )
                        .with_range(1, 500),
                    ],
                    "EntrypointTraceReport",
                )
                .with_response_fields(entrypoint_trace_response_fields()),
                api_get(
                    "/api/insights",
                    "List investigation insights with severity, kind, and search filters.",
                    insight_params(),
                    "InsightReport",
                )
                .with_response_fields(insight_report_response_fields()),
                api_get(
                    "/api/check",
                    "Run a quality gate over insights.",
                    check_params(),
                    "CheckReport",
                )
                .with_response_fields(check_report_response_fields()),
                api_get(
                    "/api/trace",
                    "Trace outgoing dependencies from a node id or label.",
                    vec![
                        path_param(),
                        query_param(
                            "label",
                            false,
                            "string",
                            None,
                            "Start node label substring.",
                        ),
                        query_param("node_id", false, "u64", None, "Start node id."),
                        query_param("depth", false, "usize", Some("2"), "Maximum trace depth.")
                            .with_range(1, 8),
                    ],
                    "TraceResult?",
                )
                .with_response_fields(trace_result_response_fields()),
                api_get(
                    "/api/dependents",
                    "Trace incoming dependents that can reach a node.",
                    vec![
                        path_param(),
                        query_param(
                            "label",
                            false,
                            "string",
                            None,
                            "Target node label substring.",
                        ),
                        query_param("node_id", false, "u64", None, "Target node id."),
                        query_param("depth", false, "usize", Some("3"), "Maximum trace depth.")
                            .with_range(1, 16),
                    ],
                    "TraceResult?",
                )
                .with_response_fields(trace_result_response_fields()),
                api_get(
                    "/api/trace-config",
                    "Trace config/environment readers and paths from entrypoints.",
                    vec![
                        path_param(),
                        query_param(
                            "target",
                            true,
                            "string",
                            None,
                            "Config or environment target.",
                        ),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("6"),
                            "Maximum upstream depth.",
                        )
                        .with_range(1, 32),
                        query_param("limit", false, "usize", Some("50"), "Maximum paths.")
                            .with_range(1, 500),
                    ],
                    "ConfigTraceResult",
                )
                .with_response_fields(config_trace_response_fields()),
                api_get(
                    "/api/trace-errors",
                    "Trace potential error/exception paths back to sources and entrypoints.",
                    vec![
                        path_param(),
                        query_param(
                            "target",
                            true,
                            "string",
                            None,
                            "Error label or metadata substring.",
                        ),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("6"),
                            "Maximum upstream depth.",
                        )
                        .with_range(1, 32),
                        query_param("limit", false, "usize", Some("50"), "Maximum paths.")
                            .with_range(1, 500),
                    ],
                    "ErrorTraceResult",
                )
                .with_response_fields(error_trace_response_fields()),
            ],
        },
        ApiSchemaGroup {
            group: "source",
            endpoints: vec![
                api_get(
                    "/api/source",
                    "Read a source snippet by project root, path, and line span.",
                    vec![
                        query_param("root", false, "path", Some("."), "Project root."),
                        query_param("path", true, "path", None, "Source path inside root."),
                        query_param("start_line", false, "u32", None, "First line."),
                        query_param("end_line", false, "u32", None, "Last line."),
                        query_param("context", false, "u32", None, "Context lines around span.")
                            .with_range(0, MAX_SOURCE_CONTEXT as usize)
                            .with_capability_limit("max_source_context"),
                    ],
                    "SourceResponse",
                )
                .with_response_fields(source_preview_response_fields()),
                api_get(
                    "/api/source-search",
                    "Search source text with compact context snippets.",
                    vec![
                        path_param(),
                        query_param("q", true, "string", None, "Search text.")
                            .with_max_length(MAX_SOURCE_SEARCH_QUERY_LENGTH)
                            .with_capability_limit("max_source_search_query_length"),
                        query_param(
                            "path_filter",
                            false,
                            "string",
                            None,
                            "Only paths containing substring.",
                        ),
                        query_param(
                            "case_sensitive",
                            false,
                            "bool",
                            Some("false"),
                            "Match case exactly.",
                        ),
                        query_param("limit", false, "usize", Some("50"), "Maximum matches.")
                            .with_range(1, MAX_SOURCE_SEARCH_LIMIT)
                            .with_capability_limit("max_source_search_limit"),
                        query_param(
                            "context",
                            false,
                            "usize",
                            Some("2"),
                            "Context lines per match.",
                        )
                        .with_range(0, MAX_SOURCE_SEARCH_CONTEXT)
                        .with_capability_limit("max_source_search_context"),
                    ],
                    "SourceSearchResult",
                )
                .with_response_fields(source_search_response_fields()),
            ],
        },
    ]
}

fn graph_slice_params() -> Vec<ApiParameterSpec> {
    vec![
        path_param(),
        query_param(
            "node_offset",
            false,
            "usize",
            Some("0"),
            "Node page offset.",
        ),
        query_param("node_limit", false, "usize", Some("250"), "Node page size.")
            .with_range(1, MAX_GRAPH_NODE_LIMIT)
            .with_capability_limit("max_graph_node_limit"),
        query_param(
            "edge_offset",
            false,
            "usize",
            Some("0"),
            "Edge page offset.",
        ),
        query_param("edge_limit", false, "usize", Some("500"), "Edge page size.")
            .with_range(1, MAX_GRAPH_EDGE_LIMIT)
            .with_capability_limit("max_graph_edge_limit"),
        query_param(
            "path_prefix",
            false,
            "string",
            None,
            "Restrict nodes by path prefix.",
        ),
        query_param(
            "kind",
            false,
            "graph_node_kind",
            None,
            "Restrict nodes by kind.",
        ),
        query_param(
            "search",
            false,
            "string",
            None,
            "Search labels, ids, and metadata.",
        ),
        query_param(
            "language",
            false,
            "string",
            None,
            "Restrict nodes by language metadata.",
        ),
        query_param(
            "item_kind",
            false,
            "string",
            None,
            "Restrict nodes by item_kind metadata.",
        ),
        query_param(
            "edge_kind",
            false,
            "graph_edge_kind",
            None,
            "Restrict edges by kind.",
        ),
        query_param(
            "confidence",
            false,
            "graph_confidence",
            None,
            "Restrict edges by confidence.",
        ),
        query_param(
            "edge_relation",
            false,
            "string",
            None,
            "Restrict edges by relation metadata.",
        ),
        query_param(
            "edge_source",
            false,
            "string",
            None,
            "Restrict edges by source metadata.",
        ),
    ]
}

fn report_params() -> Vec<ApiParameterSpec> {
    vec![
        path_param(),
        query_param(
            "architecture_group_limit",
            false,
            "usize",
            Some("50"),
            "Maximum architecture groups, capped by server capabilities.",
        )
        .with_range(1, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT)
        .with_capability_limit("max_report_architecture_group_limit"),
        query_param(
            "architecture_edge_limit",
            false,
            "usize",
            Some("200"),
            "Maximum architecture edges, capped by server capabilities.",
        )
        .with_range(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT)
        .with_capability_limit("max_report_architecture_edge_limit"),
        query_param(
            "language_link_limit",
            false,
            "usize",
            Some("50"),
            "Maximum language dependency links, capped by server capabilities.",
        )
        .with_range(1, MAX_REPORT_LANGUAGE_LINK_LIMIT)
        .with_capability_limit("max_report_language_link_limit"),
        query_param(
            "hotspot_limit",
            false,
            "usize",
            Some("25"),
            "Maximum hotspots, capped by server capabilities.",
        )
        .with_range(1, MAX_REPORT_HOTSPOT_LIMIT)
        .with_capability_limit("max_report_hotspot_limit"),
        query_param(
            "insight_limit",
            false,
            "usize",
            Some("50"),
            "Maximum returned insights, capped by server capabilities; total counts stay complete.",
        )
        .with_range(1, MAX_REPORT_INSIGHT_LIMIT)
        .with_capability_limit("max_report_insight_limit"),
        query_param(
            "fail_on",
            false,
            "insight_severity",
            Some("error"),
            "Quality gate threshold.",
        ),
    ]
}

fn insight_params() -> Vec<ApiParameterSpec> {
    vec![
        path_param(),
        query_param(
            "severity",
            false,
            "insight_severity",
            None,
            "Filter by exact severity.",
        ),
        query_param(
            "kind",
            false,
            "insight_kind",
            None,
            "Filter insight kind by substring.",
        ),
        query_param(
            "search",
            false,
            "string",
            None,
            "Filter kind, message, node ids, or edge indexes.",
        ),
        query_param(
            "limit",
            false,
            "usize",
            Some("50"),
            "Maximum returned insights.",
        )
        .with_range(1, MAX_INSIGHT_LIMIT)
        .with_capability_limit("max_insight_limit"),
    ]
}

fn check_params() -> Vec<ApiParameterSpec> {
    vec![
        path_param(),
        query_param(
            "fail_on",
            false,
            "insight_severity",
            Some("error"),
            "Quality gate threshold.",
        ),
        query_param(
            "kind",
            false,
            "insight_kind",
            None,
            "Restrict insight kinds by substring.",
        ),
        query_param(
            "search",
            false,
            "string",
            None,
            "Restrict insights by kind, message, node ids, or edge indexes.",
        ),
        query_param(
            "limit",
            false,
            "usize",
            Some("50"),
            "Maximum insights in nested report.",
        )
        .with_range(1, MAX_INSIGHT_LIMIT)
        .with_capability_limit("max_insight_limit"),
    ]
}

fn semantic_filter_params() -> Vec<ApiParameterSpec> {
    vec![
        path_param(),
        query_param(
            "work_item_limit",
            false,
            "usize",
            Some("100"),
            "Maximum semantic work items.",
        )
        .with_range(1, MAX_SEMANTIC_WORK_ITEM_LIMIT)
        .with_capability_limit("max_semantic_work_item_limit"),
        query_param(
            "work_language",
            false,
            "string",
            None,
            "Restrict work items by language.",
        ),
        query_param(
            "work_status",
            false,
            "semantic_work_status",
            None,
            "Restrict work items by status.",
        ),
        query_param(
            "work_capability",
            false,
            "semantic_work_capability",
            None,
            "Restrict work items by capability.",
        ),
    ]
}

fn scan_job_body_fields() -> Vec<ApiParameterSpec> {
    vec![body_field(
        "path",
        false,
        "path",
        Some("."),
        "Project root path.",
    )]
}

fn semantic_filter_body_fields() -> Vec<ApiParameterSpec> {
    vec![
        body_field("path", false, "path", Some("."), "Project root path."),
        body_field(
            "work_item_limit",
            false,
            "usize",
            Some("100"),
            "Maximum semantic work items.",
        )
        .with_range(1, MAX_SEMANTIC_WORK_ITEM_LIMIT)
        .with_capability_limit("max_semantic_work_item_limit"),
        body_field(
            "work_language",
            false,
            "string",
            None,
            "Restrict work items by language.",
        ),
        body_field(
            "work_status",
            false,
            "semantic_work_status",
            None,
            "Restrict work items by status.",
        ),
        body_field(
            "work_capability",
            false,
            "semantic_work_capability",
            None,
            "Restrict work items by capability.",
        ),
    ]
}

fn semantic_patch_body_fields() -> Vec<ApiParameterSpec> {
    let mut fields = semantic_filter_body_fields();
    fields.push(body_field(
        "responses",
        true,
        "SemanticLspResponse[]",
        None,
        "Language-server responses to map into graph patches.",
    ));
    fields
}

fn semantic_enrich_body_fields() -> Vec<ApiParameterSpec> {
    let mut fields = semantic_filter_body_fields();
    fields.push(
        body_field(
            "request_timeout_ms",
            false,
            "u64",
            Some("30000"),
            "Milliseconds to wait for each language-server response.",
        )
        .with_range(1, MAX_SEMANTIC_REQUEST_TIMEOUT_MS as usize)
        .with_capability_limit("max_semantic_request_timeout_ms"),
    );
    fields
}

fn capabilities_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("name", true, "string", "Product name."),
        response_field(
            "server_version",
            true,
            "semver",
            "CodeGraph server package version.",
        ),
        response_field("api_version", true, "u32", "HTTP API contract version."),
        response_field(
            "graph_schema_version",
            true,
            "u32",
            "Serialized graph schema version.",
        ),
        response_field("root", true, "path", "Resolved primary project root."),
        response_field(
            "projects",
            true,
            "ProjectResponse[]",
            "Configured project roots available to the server.",
        ),
        response_field(
            "languages",
            true,
            "LanguageResponse[]",
            "Built-in language adapters.",
        ),
        response_field(
            "features",
            true,
            "string[]",
            "Advertised runtime feature keys.",
        ),
        response_field(
            "endpoints",
            true,
            "EndpointGroupResponse[]",
            "Grouped route summaries.",
        ),
        response_field(
            "limits",
            true,
            "RuntimeLimitsResponse",
            "Published runtime limits for clients.",
        ),
        response_field(
            "cache",
            true,
            "CacheCapabilityResponse",
            "Persistent graph cache status.",
        ),
    ]
}

fn api_schema_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("name", true, "string", "API contract name."),
        response_field(
            "server_version",
            true,
            "semver",
            "CodeGraph server package version.",
        ),
        response_field("api_version", true, "u32", "HTTP API contract version."),
        response_field(
            "graph_schema_version",
            true,
            "u32",
            "Serialized graph schema version.",
        ),
        response_field("description", true, "string", "API contract description."),
        response_field(
            "common_response_headers",
            true,
            "ApiHeaderSpec[]",
            "HTTP response headers attached across CodeGraph web and API responses.",
        ),
        response_field(
            "groups",
            true,
            "ApiSchemaGroup[]",
            "Machine-readable endpoint groups.",
        ),
        response_field(
            "enum_values",
            true,
            "map<string,string[]>",
            "Known enum values and query terms for clients.",
        ),
    ]
}

fn probe_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "status",
            true,
            "string",
            "Probe status: ok for liveness and ready for readiness.",
        ),
        response_field(
            "server_version",
            true,
            "semver",
            "CodeGraph server package version.",
        ),
        response_field("api_version", true, "u32", "HTTP API contract version."),
        response_field(
            "graph_schema_version",
            true,
            "u32",
            "Serialized graph schema version.",
        ),
        response_field("root", true, "path", "Resolved primary project root."),
        response_field(
            "cache_enabled",
            true,
            "bool",
            "Whether the server persistent graph cache is enabled.",
        ),
    ]
}

fn health_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("status", true, "string", "Runtime health status."),
        response_field(
            "server_version",
            true,
            "semver",
            "CodeGraph server package version.",
        ),
        response_field("root", true, "path", "Resolved primary project root."),
        response_field(
            "max_file_size",
            true,
            "u64?",
            "Effective maximum indexed source file size in bytes.",
        ),
        response_field(
            "cache_dir",
            true,
            "path?",
            "Persistent graph cache directory when cache is enabled.",
        ),
        response_field(
            "max_scan_jobs",
            true,
            "usize",
            "Maximum retained scan jobs.",
        ),
        response_field(
            "scan_jobs",
            true,
            "JobStoreHealth",
            "Retained scan job counters by status.",
        ),
        response_field(
            "scan_concurrency",
            true,
            "ConcurrencyHealth",
            "Scan concurrency limit, active slots, and available slots.",
        ),
        response_field(
            "max_semantic_jobs",
            true,
            "usize",
            "Maximum retained semantic jobs.",
        ),
        response_field(
            "semantic_jobs",
            true,
            "JobStoreHealth",
            "Retained semantic job counters by status.",
        ),
        response_field(
            "semantic_concurrency",
            true,
            "ConcurrencyHealth",
            "Semantic job concurrency limit, active slots, and available slots.",
        ),
    ]
}

fn metrics_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("status", true, "string", "Runtime metrics status."),
        response_field(
            "server_version",
            true,
            "semver",
            "CodeGraph server package version.",
        ),
        response_field("api_version", true, "u32", "HTTP API contract version."),
        response_field(
            "graph_schema_version",
            true,
            "u32",
            "Serialized graph schema version.",
        ),
        response_field(
            "uptime_seconds",
            true,
            "u64",
            "Seconds since this server process started.",
        ),
        response_field("root", true, "path", "Resolved primary project root."),
        response_field(
            "projects",
            true,
            "usize",
            "Number of configured project roots.",
        ),
        response_field(
            "languages",
            true,
            "usize",
            "Number of built-in language adapters.",
        ),
        response_field(
            "features",
            true,
            "usize",
            "Number of advertised runtime capability features.",
        ),
        response_field(
            "max_file_size",
            true,
            "u64?",
            "Effective maximum indexed source file size in bytes.",
        ),
        response_field(
            "cache",
            true,
            "CacheCapabilityResponse",
            "Persistent graph cache status and directory.",
        ),
        response_field(
            "scan_jobs",
            true,
            "JobPoolMetricsResponse",
            "Scan job retention and concurrency metrics.",
        ),
        response_field(
            "semantic_jobs",
            true,
            "JobPoolMetricsResponse",
            "Semantic job retention and concurrency metrics.",
        ),
    ]
}

fn graph_slice_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("nodes", true, "Node[]", "Returned graph nodes."),
        response_field(
            "edges",
            true,
            "Edge[]",
            "Returned graph edges with metadata.edge_index values.",
        ),
        response_field(
            "total_nodes",
            true,
            "usize",
            "Total nodes matching the graph filters.",
        ),
        response_field(
            "total_edges",
            true,
            "usize",
            "Total edges matching the graph filters.",
        ),
        response_field("node_offset", true, "usize", "Returned node page offset."),
        response_field("node_limit", true, "usize", "Returned node page limit."),
        response_field("edge_offset", true, "usize", "Returned edge page offset."),
        response_field("edge_limit", true, "usize", "Returned edge page limit."),
        response_field(
            "truncated_nodes",
            true,
            "bool",
            "Whether more matching nodes exist beyond this page.",
        ),
        response_field(
            "truncated_edges",
            true,
            "bool",
            "Whether more matching edges exist beyond this page.",
        ),
    ]
}

fn node_context_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("node", true, "Node", "Selected graph node."),
        response_field(
            "nodes",
            true,
            "Node[]",
            "Selected node plus neighboring nodes referenced by returned edges.",
        ),
        response_field(
            "edges",
            true,
            "Edge[]",
            "Neighboring edges with metadata.edge_index values.",
        ),
        response_field(
            "total_edges",
            true,
            "usize",
            "Total neighboring edges before limiting.",
        ),
        response_field(
            "edge_limit",
            true,
            "usize",
            "Applied neighboring edge limit.",
        ),
        response_field(
            "truncated_edges",
            true,
            "bool",
            "Whether more neighboring edges exist beyond the limit.",
        ),
    ]
}

fn node_card_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "context",
            true,
            "NodeContext",
            "Selected node, neighboring nodes, and neighboring edges.",
        ),
        response_field(
            "dependency_summary",
            true,
            "NodeDependencySummary",
            "Incoming/outgoing dependency counts and neighbor facets.",
        ),
        response_field(
            "insight_summary",
            true,
            "NodeInsightSummary",
            "Related risk counts by severity and kind.",
        ),
        response_field(
            "file_summary",
            false,
            "FileNodeSummary?",
            "File-level contained symbol, import, config, error, and trace facts.",
        ),
        response_field(
            "source",
            false,
            "SourcePreview?",
            "Source snippet around the selected node when available.",
        ),
        response_field(
            "insights",
            true,
            "Insight[]",
            "Capped related risk findings for this node or contained file facts.",
        ),
        response_field(
            "total_insights",
            true,
            "usize",
            "Total related risk findings before limiting.",
        ),
        response_field(
            "insight_limit",
            true,
            "usize",
            "Applied related risk limit.",
        ),
        response_field(
            "truncated_insights",
            true,
            "bool",
            "Whether more related risks exist beyond the limit.",
        ),
        response_field(
            "actions",
            true,
            "NodeCardAction[]",
            "Suggested focused graph actions for investigation handoff.",
        ),
    ]
}

fn query_result_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "query",
            true,
            "string",
            "Normalized graph query expression.",
        ),
        response_field("nodes", true, "Node[]", "Returned query nodes."),
        response_field(
            "edges",
            true,
            "Edge[]",
            "Returned query edges with metadata.edge_index values.",
        ),
        response_field(
            "total_nodes",
            true,
            "usize",
            "Total nodes matching the query before paging or limiting.",
        ),
        response_field(
            "total_edges",
            true,
            "usize",
            "Total edges matching the query before paging or limiting.",
        ),
        response_field("returned_nodes", true, "usize", "Returned node count."),
        response_field("returned_edges", true, "usize", "Returned edge count."),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether the result was capped by query limits.",
        ),
        response_field(
            "facets",
            true,
            "QueryFacets",
            "Returned node/edge facets for triage.",
        ),
    ]
}

fn edge_explanation_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("edge_index", true, "usize", "Exact graph edge index."),
        response_field(
            "total_matches",
            true,
            "usize",
            "Total matching edges for the lookup.",
        ),
        response_field("source", true, "Node", "Edge source node."),
        response_field("target", true, "Node", "Edge target node."),
        response_field("edge", true, "Edge", "Explained graph edge."),
        response_field(
            "summary",
            true,
            "string",
            "Human-readable explanation summary.",
        ),
        response_field(
            "evidence",
            true,
            "string[]",
            "Provenance and confidence evidence for the edge.",
        ),
        response_field(
            "insight_summary",
            true,
            "NodeInsightSummary",
            "Edge-scoped related risk counts by severity and kind.",
        ),
        response_field(
            "insights",
            true,
            "Insight[]",
            "Capped edge-scoped related risk findings.",
        ),
        response_field(
            "total_insights",
            true,
            "usize",
            "Total edge-scoped related risks before limiting.",
        ),
        response_field(
            "insight_limit",
            true,
            "usize",
            "Applied edge-scoped risk limit.",
        ),
        response_field(
            "truncated_insights",
            true,
            "bool",
            "Whether more edge-scoped risks exist beyond the limit.",
        ),
    ]
}

fn project_report_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("root", true, "path", "Resolved project root."),
        response_field(
            "generated_at_unix",
            true,
            "u64",
            "Unix timestamp when the report snapshot was generated.",
        ),
        response_field(
            "cache",
            true,
            "CacheInfo",
            "Graph cache status for the scan.",
        ),
        response_field(
            "coverage",
            true,
            "ScanCoverageReport",
            "Indexed, skipped, and non-indexed file coverage.",
        ),
        response_field(
            "report",
            true,
            "ProjectReport",
            "Production project report with summary, risks, quality gate, topology, and hotspots.",
        ),
    ]
}

fn architecture_map_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "groups",
            true,
            "ArchitectureGroup[]",
            "Top-level project area groups.",
        ),
        response_field(
            "edges",
            true,
            "ArchitectureEdge[]",
            "Cross-area dependency edges with edge_indexes.",
        ),
        response_field(
            "total_groups",
            true,
            "usize",
            "Total architecture groups before limiting.",
        ),
        response_field(
            "total_edges",
            true,
            "usize",
            "Total cross-area edges before limiting.",
        ),
        response_field(
            "truncated_groups",
            true,
            "bool",
            "Whether more groups exist beyond the limit.",
        ),
        response_field(
            "truncated_edges",
            true,
            "bool",
            "Whether more cross-area edges exist beyond the limit.",
        ),
    ]
}

fn language_dependency_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "links",
            true,
            "LanguageDependency[]",
            "Language-to-language dependency links.",
        ),
        response_field(
            "total_links",
            true,
            "usize",
            "Total language links before limiting.",
        ),
        response_field(
            "total_edges",
            true,
            "usize",
            "Total dependency edges represented by language links.",
        ),
        response_field(
            "cross_language_edges",
            true,
            "usize",
            "Dependency edges crossing language boundaries.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more links exist beyond the limit.",
        ),
    ]
}

fn hotspot_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "hotspots",
            true,
            "Hotspot[]",
            "High-degree files, functions, entrypoints, and config nodes.",
        ),
        response_field(
            "total_candidates",
            true,
            "usize",
            "Total hotspot candidates before limiting.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more hotspots exist beyond the limit.",
        ),
    ]
}

fn entrypoint_trace_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("max_depth", true, "usize", "Applied trace depth limit."),
        response_field(
            "total_entrypoints",
            true,
            "usize",
            "Total detected entrypoints before limiting.",
        ),
        response_field(
            "traces",
            true,
            "TraceResult[]",
            "Outgoing dependency traces from entrypoints.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more entrypoint traces exist beyond the limit.",
        ),
    ]
}

fn trace_result_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("start", true, "Node", "Trace start node."),
        response_field("max_depth", true, "usize", "Applied trace depth limit."),
        response_field(
            "nodes",
            true,
            "TraceNode[]",
            "Reached trace nodes with depth.",
        ),
        response_field(
            "edges",
            true,
            "Edge[]",
            "Trace edges with metadata.edge_index values.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether the trace was capped by depth or traversal limits.",
        ),
    ]
}

fn config_trace_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "target",
            true,
            "string",
            "Requested config/environment target.",
        ),
        response_field("max_depth", true, "usize", "Applied upstream depth limit."),
        response_field(
            "matches",
            true,
            "ConfigTraceMatch[]",
            "Matched config/environment facts with readers and entrypoint paths.",
        ),
        response_field(
            "total_matches",
            true,
            "usize",
            "Total matched config/environment facts.",
        ),
        response_field(
            "total_readers",
            true,
            "usize",
            "Total reader edges across matches.",
        ),
        response_field(
            "total_paths",
            true,
            "usize",
            "Total entrypoint paths across matches.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether readers or paths were capped by limits.",
        ),
    ]
}

fn error_trace_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "target",
            true,
            "string",
            "Requested error label or metadata target.",
        ),
        response_field("max_depth", true, "usize", "Applied upstream depth limit."),
        response_field(
            "matches",
            true,
            "ErrorTraceMatch[]",
            "Matched error facts with sources and entrypoint paths.",
        ),
        response_field("total_matches", true, "usize", "Total matched error facts."),
        response_field(
            "total_sources",
            true,
            "usize",
            "Total error source edges across matches.",
        ),
        response_field(
            "total_paths",
            true,
            "usize",
            "Total entrypoint paths across matches.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether sources or paths were capped by limits.",
        ),
    ]
}

fn insight_report_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "total",
            true,
            "usize",
            "Total matching insights before limiting.",
        ),
        response_field(
            "by_severity",
            true,
            "map<string,usize>",
            "Insight counts by severity.",
        ),
        response_field(
            "by_kind",
            true,
            "map<string,usize>",
            "Insight counts by kind.",
        ),
        response_field(
            "insights",
            true,
            "Insight[]",
            "Returned investigation insights.",
        ),
    ]
}

fn check_report_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("passed", true, "bool", "Whether the quality gate passed."),
        response_field(
            "fail_on",
            true,
            "risk_severity",
            "Quality gate severity threshold.",
        ),
        response_field(
            "failing_insights",
            true,
            "usize",
            "Number of insights at or above the threshold.",
        ),
        response_field(
            "report",
            true,
            "InsightReport",
            "Filtered insight report used by the gate.",
        ),
    ]
}

fn source_preview_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("path", true, "path", "Source path inside the project root."),
        response_field("start_line", true, "u32", "First returned line."),
        response_field("end_line", true, "u32", "Last returned line."),
        response_field(
            "lines",
            true,
            "SourcePreviewLine[]",
            "Returned source lines with highlight markers.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether the source preview was capped.",
        ),
    ]
}

fn source_search_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("query", true, "string", "Normalized searched text."),
        response_field(
            "path_filter",
            false,
            "string?",
            "Optional path substring filter.",
        ),
        response_field(
            "case_sensitive",
            true,
            "bool",
            "Whether search matched case exactly.",
        ),
        response_field(
            "total_matches",
            true,
            "usize",
            "Total matching source locations before limiting.",
        ),
        response_field(
            "matches",
            true,
            "SourceSearchMatch[]",
            "Returned source matches with compact context.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more matches exist beyond the limit.",
        ),
    ]
}

fn api_get(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    response: &'static str,
) -> ApiEndpointSpec {
    api_endpoint("GET", path, summary, parameters, None, response, false)
}

fn api_get_stream(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    response: &'static str,
) -> ApiEndpointSpec {
    api_endpoint("GET", path, summary, parameters, None, response, true)
}

fn api_post(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    body: Option<&'static str>,
    response: &'static str,
    streaming: bool,
) -> ApiEndpointSpec {
    api_endpoint("POST", path, summary, parameters, body, response, streaming)
}

fn api_delete(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    response: &'static str,
) -> ApiEndpointSpec {
    api_endpoint("DELETE", path, summary, parameters, None, response, false)
}

fn api_endpoint(
    method: &'static str,
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    body: Option<&'static str>,
    response: &'static str,
    streaming: bool,
) -> ApiEndpointSpec {
    ApiEndpointSpec {
        method,
        path,
        summary,
        parameters,
        body,
        body_capability_limit: body.map(|_| "max_api_body_bytes"),
        body_fields: Vec::new(),
        response,
        response_fields: Vec::new(),
        response_headers: Vec::new(),
        streaming,
    }
}

fn path_param() -> ApiParameterSpec {
    query_param("path", false, "path", Some("."), "Project root path.")
}

fn id_param() -> ApiParameterSpec {
    ApiParameterSpec {
        name: "id",
        location: "path",
        required: true,
        value_type: "string",
        default: None,
        minimum: None,
        maximum: None,
        max_length: None,
        capability_limit: None,
        description: "Retained job id.",
    }
}

fn job_status_param() -> ApiParameterSpec {
    query_param("status", false, "job_status", None, "Filter by job status.")
}

fn body_field(
    name: &'static str,
    required: bool,
    value_type: &'static str,
    default: Option<&'static str>,
    description: &'static str,
) -> ApiParameterSpec {
    ApiParameterSpec {
        name,
        location: "body",
        required,
        value_type,
        default,
        minimum: None,
        maximum: None,
        max_length: None,
        capability_limit: None,
        description,
    }
}

fn response_field(
    name: &'static str,
    required: bool,
    value_type: &'static str,
    description: &'static str,
) -> ApiParameterSpec {
    ApiParameterSpec {
        name,
        location: "response",
        required,
        value_type,
        default: None,
        minimum: None,
        maximum: None,
        max_length: None,
        capability_limit: None,
        description,
    }
}

fn query_param(
    name: &'static str,
    required: bool,
    value_type: &'static str,
    default: Option<&'static str>,
    description: &'static str,
) -> ApiParameterSpec {
    ApiParameterSpec {
        name,
        location: "query",
        required,
        value_type,
        default,
        minimum: None,
        maximum: None,
        max_length: None,
        capability_limit: None,
        description,
    }
}

fn capability_features(cache_enabled: bool, access_log_enabled: bool) -> Vec<&'static str> {
    let mut features = vec![
        "multi_project_roots",
        "api_schema",
        "repository_scan_policy",
        "incremental_scan_plan",
        "incremental_changed_scan",
        "incremental_merge_preview",
        "safe_incremental_cache_update",
        "mixed_language_syntax_graph",
        "source_preview",
        "graph_paging",
        "node_context",
        "node_cards",
        "focused_subgraphs",
        "query_language",
        "entrypoint_traces",
        "config_traces",
        "error_traces",
        "reverse_dependents",
        "insights",
        "quality_checks",
        "project_report",
        "edge_explanations",
        "source_search",
        "async_scan_jobs",
        "async_semantic_jobs",
        "job_listing",
        "job_cancellation",
        "runtime_metrics",
        "runtime_probes",
        "graceful_shutdown",
        "request_ids",
        "response_timing_headers",
        "api_body_limits",
        "sse_job_events",
        "semantic_lsp",
        "web_canvas",
        "i18n_en_ru",
        "dot_export",
        "ndjson_export",
    ];
    if access_log_enabled {
        features.push("access_log");
    }
    if cache_enabled {
        features.push("persistent_graph_cache");
        features.push("persistent_graph_chunks");
        features.push("semantic_lsp_cache");
    }
    features
}

fn capability_endpoints() -> Vec<EndpointGroupResponse> {
    vec![
        EndpointGroupResponse {
            group: "system",
            endpoints: vec![
                "GET /api/capabilities",
                "GET /api/schema",
                "GET /api/live",
                "GET /api/ready",
                "GET /api/health",
                "GET /api/metrics",
                "GET /api/projects",
                "GET /api/languages",
                "GET /api/scan-options",
            ],
        },
        EndpointGroupResponse {
            group: "scan",
            endpoints: vec![
                "GET /api/scan",
                "GET /api/coverage",
                "GET /api/cache-diff",
                "GET /api/cache-chunks",
                "GET /api/incremental-plan",
                "GET /api/incremental-scan",
                "GET /api/incremental-merge-preview",
                "POST /api/incremental-update",
                "GET /api/incremental-update",
                "POST /api/scan-jobs",
                "GET /api/scan-jobs",
                "GET /api/scan-jobs/{id}",
                "DELETE /api/scan-jobs/{id}",
                "GET /api/scan-jobs/{id}/events",
                "GET /api/scan-jobs/{id}/result",
            ],
        },
        EndpointGroupResponse {
            group: "semantic",
            endpoints: vec![
                "GET /api/lsp",
                "GET /api/semantic-readiness",
                "GET /api/semantic-plan",
                "GET /api/semantic-batch",
                "POST /api/semantic-patch",
                "POST /api/semantic-apply",
                "POST /api/semantic-enrich",
                "POST /api/semantic-jobs",
                "GET /api/semantic-jobs",
                "GET /api/semantic-jobs/{id}",
                "DELETE /api/semantic-jobs/{id}",
                "GET /api/semantic-jobs/{id}/events",
                "GET /api/semantic-jobs/{id}/result",
            ],
        },
        EndpointGroupResponse {
            group: "graph",
            endpoints: vec![
                "GET /api/graph",
                "GET /api/node-context",
                "GET /api/node-card",
                "GET /api/focus",
                "GET /api/summary",
                "GET /api/query",
                "GET /api/explain-edge",
            ],
        },
        EndpointGroupResponse {
            group: "analysis",
            endpoints: vec![
                "GET /api/report",
                "GET /api/architecture",
                "GET /api/language-dependencies",
                "GET /api/hotspots",
                "GET /api/entrypoints",
                "GET /api/entrypoint-traces",
                "GET /api/insights",
                "GET /api/check",
                "GET /api/trace",
                "GET /api/dependents",
                "GET /api/trace-config",
                "GET /api/trace-errors",
            ],
        },
        EndpointGroupResponse {
            group: "source",
            endpoints: vec!["GET /api/source", "GET /api/source-search"],
        },
        EndpointGroupResponse {
            group: "export",
            endpoints: vec![
                "GET /api/export?format=json",
                "GET /api/export?format=dot",
                "GET /api/export?format=ndjson",
            ],
        },
    ]
}

fn job_store_health(statuses: impl IntoIterator<Item = ScanJobStatus>) -> JobStoreHealth {
    let mut health = JobStoreHealth::default();
    for status in statuses {
        health.total += 1;
        match status {
            ScanJobStatus::Queued => health.queued += 1,
            ScanJobStatus::Running => health.running += 1,
            ScanJobStatus::Complete => health.complete += 1,
            ScanJobStatus::Failed => health.failed += 1,
            ScanJobStatus::Canceled => health.canceled += 1,
        }
    }
    health
}

fn concurrency_health(semaphore: &Semaphore, limit: usize) -> ConcurrencyHealth {
    let limit = limit.max(1);
    let available = semaphore.available_permits().min(limit);
    ConcurrencyHealth {
        limit,
        active: limit.saturating_sub(available),
        available,
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn job_without_graph(mut job: ScanJob) -> ScanJob {
    job.graph = None;
    job
}

fn semantic_job_without_result(mut job: SemanticJob) -> SemanticJob {
    job.result = None;
    job
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
                request_id: current_request_id(),
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn resolve_scan_root_allows_configured_projects() {
        let temp = temp_server_root();
        let root = temp.join("root");
        let sibling = temp.join("sibling");
        let outside = temp.join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let root = root.canonicalize().unwrap();
        let sibling = sibling.canonicalize().unwrap();
        let outside = outside.canonicalize().unwrap();
        let state = test_state(root.clone(), vec![sibling.clone()], false);

        assert_eq!(resolve_scan_root(&state, None).unwrap(), root);
        assert_eq!(
            resolve_scan_root(&state, Some(sibling.as_path())).unwrap(),
            sibling
        );
        assert!(resolve_scan_root(&state, Some(outside.as_path())).is_err());

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn resolve_source_path_uses_selected_project_root() {
        let temp = temp_server_root();
        let root = temp.join("root");
        let sibling = temp.join("sibling");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(sibling.join("src")).unwrap();
        fs::write(sibling.join("src").join("main.py"), "print('hi')\n").unwrap();

        let root = root.canonicalize().unwrap();
        let sibling = sibling.canonicalize().unwrap();
        let state = test_state(root, vec![sibling.clone()], false);

        assert_eq!(
            resolve_path(&state, &sibling, Path::new("src/main.py")).unwrap(),
            sibling.join("src").join("main.py").canonicalize().unwrap()
        );

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn language_responses_include_required_mixed_language_set() {
        let languages: Vec<_> = language_responses()
            .into_iter()
            .map(|language| language.language)
            .collect();

        for language in [
            "rust",
            "python",
            "javascript",
            "go",
            "c",
            "cpp",
            "php",
            "bash",
        ] {
            assert!(languages.contains(&language), "missing {language}");
        }
    }

    #[test]
    fn security_headers_are_attached_to_responses() {
        let mut headers = HeaderMap::new();
        apply_security_headers(&mut headers);

        let csp = headers
            .get("content-security-policy")
            .and_then(|value| value.to_str().ok())
            .expect("content-security-policy should be present");
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("connect-src 'self'"));
        assert_eq!(
            headers.get("x-content-type-options"),
            Some(&HeaderValue::from_static("nosniff"))
        );
        assert_eq!(
            headers.get("x-frame-options"),
            Some(&HeaderValue::from_static("DENY"))
        );
        assert_eq!(
            headers.get("referrer-policy"),
            Some(&HeaderValue::from_static("no-referrer"))
        );
        assert_eq!(
            headers.get("permissions-policy"),
            Some(&HeaderValue::from_static(
                "camera=(), microphone=(), geolocation=(), payment=()"
            ))
        );
    }

    #[test]
    fn cache_headers_keep_runtime_responses_uncached() {
        for path in ["/", "/api/graph", "/api/health", "/missing"] {
            let mut headers = HeaderMap::new();
            apply_cache_headers_for_path(path, &mut headers);

            assert_eq!(
                headers.get(header::CACHE_CONTROL),
                Some(&HeaderValue::from_static(DYNAMIC_CACHE_CONTROL)),
                "{path} should not be cached"
            );
        }
    }

    #[test]
    fn cache_headers_force_revalidation_for_unversioned_web_assets() {
        for path in ["/app.js", "/label-policy.js", "/styles.css"] {
            let mut headers = HeaderMap::new();
            apply_cache_headers_for_path(path, &mut headers);

            assert_eq!(
                headers.get(header::CACHE_CONTROL),
                Some(&HeaderValue::from_static(STATIC_ASSET_CACHE_CONTROL)),
                "{path} should be revalidated"
            );
        }
    }

    #[test]
    fn api_body_byte_limits_are_clamped() {
        assert_eq!(normalize_api_body_bytes(0), 1);
        assert_eq!(
            normalize_api_body_bytes(DEFAULT_API_BODY_BYTES),
            DEFAULT_API_BODY_BYTES
        );
        assert_eq!(normalize_api_body_bytes(usize::MAX), MAX_API_BODY_BYTES);
    }

    #[test]
    fn static_asset_etags_are_stable_and_content_sensitive() {
        let etag = static_asset_etag("asset-body");

        assert_eq!(etag, static_asset_etag("asset-body"));
        assert_ne!(etag, static_asset_etag("asset-body!"));
        assert!(etag.starts_with("\"codegraph-"));
        assert!(etag.ends_with('"'));
    }

    #[test]
    fn static_asset_if_none_match_accepts_lists_and_weak_tags() {
        let etag = static_asset_etag("asset-body");
        let exact = HeaderValue::from_str(&format!("\"other\", {etag}")).unwrap();
        let weak = HeaderValue::from_str(&format!("W/{etag}")).unwrap();
        let miss = HeaderValue::from_static("\"other\"");

        assert!(if_none_match_matches(&exact, &etag));
        assert!(if_none_match_matches(&weak, &etag));
        assert!(if_none_match_matches(&HeaderValue::from_static("*"), &etag));
        assert!(!if_none_match_matches(&miss, &etag));
    }

    #[test]
    fn static_asset_response_returns_not_modified_for_matching_etag() {
        let body = "console.log('asset');\n";
        let etag = static_asset_etag(body);
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_str(&etag).unwrap());

        let response =
            static_asset_response(&headers, "application/javascript; charset=utf-8", body);

        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(
            response.headers().get(header::ETAG),
            Some(&HeaderValue::from_str(&etag).unwrap())
        );
    }

    #[test]
    fn access_log_line_includes_method_target_status_and_latency() {
        assert_eq!(
            access_log_line(
                "req-42",
                "GET",
                "/api/health?verbose=1",
                StatusCode::OK,
                Duration::from_millis(42),
            ),
            "req-42 GET /api/health?verbose=1 -> 200 42ms"
        );
    }

    #[test]
    fn response_time_header_value_formats_elapsed_milliseconds() {
        assert_eq!(
            response_time_header_value(Duration::from_millis(0)),
            HeaderValue::from_static("0")
        );
        assert_eq!(
            response_time_header_value(Duration::from_millis(42)),
            HeaderValue::from_static("42")
        );
        assert_eq!(
            response_time_header_value(Duration::from_micros(42_999)),
            HeaderValue::from_static("42")
        );
    }

    #[test]
    fn export_headers_publish_graph_and_body_sizes() {
        let mut headers = HeaderMap::new();
        apply_export_headers(&mut headers, 7, 11, 2048);

        assert_eq!(
            headers.get(EXPORT_NODES_HEADER),
            Some(&HeaderValue::from_static("7"))
        );
        assert_eq!(
            headers.get(EXPORT_EDGES_HEADER),
            Some(&HeaderValue::from_static("11"))
        );
        assert_eq!(
            headers.get(EXPORT_BYTES_HEADER),
            Some(&HeaderValue::from_static("2048"))
        );
    }

    #[test]
    fn request_ids_accept_safe_values_and_reject_header_injection() {
        assert!(is_valid_request_id("req-123"));
        assert!(is_valid_request_id("trace.root:span_1"));
        assert!(!is_valid_request_id(""));
        assert!(!is_valid_request_id("contains space"));
        assert!(!is_valid_request_id("bad\nheader"));
        assert!(!is_valid_request_id(&"x".repeat(129)));
    }

    #[test]
    fn configured_api_token_trims_empty_values() {
        assert_eq!(
            configured_api_token(Some(" secret ".to_string())),
            Some("secret".to_string())
        );
        assert_eq!(configured_api_token(Some("   ".to_string())), None);
    }

    #[test]
    fn api_auth_accepts_bearer_header_token_and_cookie() {
        let auth = ApiAuth::new(Some("secret-token".to_string()));
        let bearer = Request::builder()
            .uri("/api/health")
            .header(header::AUTHORIZATION, "Bearer secret-token")
            .body(axum::body::Body::empty())
            .unwrap();
        let header = Request::builder()
            .uri("/api/health")
            .header("x-codegraph-token", "secret-token")
            .body(axum::body::Body::empty())
            .unwrap();
        let cookie = Request::builder()
            .uri("/api/health")
            .header(
                header::COOKIE,
                "theme=dark; codegraph_api_token=secret-token",
            )
            .body(axum::body::Body::empty())
            .unwrap();
        let wrong = Request::builder()
            .uri("/api/health")
            .header(header::AUTHORIZATION, "Bearer wrong")
            .body(axum::body::Body::empty())
            .unwrap();
        let disabled = ApiAuth::new(None);

        assert!(api_request_authorized(&bearer, &auth));
        assert!(api_request_authorized(&header, &auth));
        assert!(api_request_authorized(&cookie, &auth));
        assert!(!api_request_authorized(&wrong, &auth));
        assert!(api_request_authorized(&wrong, &disabled));
    }

    #[tokio::test]
    async fn current_request_id_reads_task_local_scope() {
        assert_eq!(current_request_id(), None);

        CURRENT_REQUEST_ID
            .scope("req-test".to_string(), async {
                assert_eq!(current_request_id(), Some("req-test".to_string()));
            })
            .await;

        assert_eq!(current_request_id(), None);
    }

    #[test]
    fn capability_features_reflect_cache_availability() {
        let without_cache = capability_features(false, true);
        let with_cache = capability_features(true, true);
        let quiet = capability_features(true, false);

        assert!(without_cache.contains(&"api_schema"));
        assert!(without_cache.contains(&"incremental_scan_plan"));
        assert!(without_cache.contains(&"incremental_changed_scan"));
        assert!(without_cache.contains(&"incremental_merge_preview"));
        assert!(without_cache.contains(&"safe_incremental_cache_update"));
        assert!(without_cache.contains(&"async_scan_jobs"));
        assert!(without_cache.contains(&"job_cancellation"));
        assert!(without_cache.contains(&"runtime_metrics"));
        assert!(without_cache.contains(&"runtime_probes"));
        assert!(without_cache.contains(&"graceful_shutdown"));
        assert!(without_cache.contains(&"request_ids"));
        assert!(without_cache.contains(&"response_timing_headers"));
        assert!(without_cache.contains(&"api_body_limits"));
        assert!(without_cache.contains(&"access_log"));
        assert!(without_cache.contains(&"project_report"));
        assert!(without_cache.contains(&"semantic_lsp"));
        assert!(without_cache.contains(&"node_cards"));
        assert!(!without_cache.contains(&"persistent_graph_cache"));
        assert!(!without_cache.contains(&"persistent_graph_chunks"));
        assert!(!without_cache.contains(&"semantic_lsp_cache"));
        assert!(with_cache.contains(&"persistent_graph_cache"));
        assert!(with_cache.contains(&"persistent_graph_chunks"));
        assert!(with_cache.contains(&"semantic_lsp_cache"));
        assert!(!quiet.contains(&"access_log"));
    }

    #[tokio::test]
    async fn capabilities_publish_runtime_graph_and_query_limits() {
        let root = temp_server_root();
        fs::create_dir_all(&root).unwrap();
        let Json(response) = capabilities_api(State(test_state(root.clone(), vec![], false)))
            .await
            .expect("capabilities response");

        assert_eq!(response.limits.default_incremental_report_limit, 100);
        assert_eq!(response.server_version, SERVER_VERSION);
        assert_eq!(response.limits.max_incremental_report_limit, 10000);
        assert_eq!(
            response.limits.default_api_body_bytes,
            DEFAULT_API_BODY_BYTES
        );
        assert_eq!(response.limits.max_api_body_bytes, DEFAULT_API_BODY_BYTES);
        assert_eq!(
            response.limits.max_configurable_api_body_bytes,
            MAX_API_BODY_BYTES
        );
        assert_eq!(response.limits.default_semantic_work_item_limit, 100);
        assert_eq!(response.limits.max_semantic_work_item_limit, 1000);
        assert_eq!(response.limits.default_semantic_request_timeout_ms, 30000);
        assert_eq!(response.limits.max_semantic_request_timeout_ms, 300000);
        assert_eq!(response.limits.default_graph_node_limit, 250);
        assert_eq!(response.limits.max_graph_node_limit, 1000);
        assert_eq!(response.limits.default_graph_edge_limit, 500);
        assert_eq!(response.limits.max_graph_edge_limit, 2000);
        assert_eq!(response.limits.default_node_context_edge_limit, 80);
        assert_eq!(response.limits.max_node_context_edge_limit, 500);
        assert_eq!(response.limits.default_node_card_source_context, 5);
        assert_eq!(response.limits.max_node_card_source_context, 40);
        assert_eq!(response.limits.default_node_card_insight_limit, 8);
        assert_eq!(response.limits.max_node_card_insight_limit, 500);
        assert_eq!(response.limits.default_focus_edge_limit, 200);
        assert_eq!(response.limits.max_focus_edge_limit, 1000);
        assert_eq!(response.limits.default_graph_query_limit, 100);
        assert_eq!(response.limits.max_graph_query_limit, 1000);
        assert_eq!(response.limits.max_graph_query_length, 4096);
        assert_eq!(response.limits.default_insight_limit, 50);
        assert_eq!(response.limits.max_insight_limit, 500);
        assert_eq!(response.limits.default_report_architecture_group_limit, 50);
        assert_eq!(response.limits.max_report_architecture_group_limit, 500);
        assert_eq!(response.limits.default_report_architecture_edge_limit, 200);
        assert_eq!(response.limits.max_report_architecture_edge_limit, 2000);
        assert_eq!(response.limits.default_report_language_link_limit, 50);
        assert_eq!(response.limits.max_report_language_link_limit, 500);
        assert_eq!(response.limits.default_report_hotspot_limit, 25);
        assert_eq!(response.limits.max_report_hotspot_limit, 500);
        assert_eq!(response.limits.default_report_insight_limit, 50);
        assert_eq!(response.limits.max_report_insight_limit, 500);
        assert_eq!(response.limits.max_source_search_query_length, 4096);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn probes_return_lightweight_runtime_status() {
        let root = temp_server_root();
        fs::create_dir_all(&root).unwrap();

        let Json(live) = live_api(State(test_state(root.clone(), vec![], false))).await;
        assert_eq!(live.status, "ok");
        assert_eq!(live.server_version, SERVER_VERSION);
        assert_eq!(live.api_version, 1);
        assert_eq!(live.graph_schema_version, CODEGRAPH_SCHEMA_VERSION);
        assert_eq!(live.root, root.display().to_string());
        assert!(!live.cache_enabled);

        let Json(ready) = ready_api(State(test_state(root.clone(), vec![], false)))
            .await
            .expect("ready probe");
        assert_eq!(ready.status, "ready");
        assert_eq!(ready.server_version, SERVER_VERSION);
        assert_eq!(ready.api_version, 1);
        assert_eq!(ready.graph_schema_version, CODEGRAPH_SCHEMA_VERSION);
        assert_eq!(ready.root, root.display().to_string());
        assert!(!ready.cache_enabled);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_report_query_limits_are_clamped_to_capabilities() {
        let limits = project_report_limits_from_query(&ProjectReportQuery {
            path: None,
            architecture_group_limit: Some(usize::MAX),
            architecture_edge_limit: Some(usize::MAX),
            language_link_limit: Some(usize::MAX),
            hotspot_limit: Some(usize::MAX),
            insight_limit: Some(usize::MAX),
            fail_on: None,
        })
        .expect("report limits");

        assert_eq!(
            limits.architecture_group_limit,
            MAX_REPORT_ARCHITECTURE_GROUP_LIMIT
        );
        assert_eq!(
            limits.architecture_edge_limit,
            MAX_REPORT_ARCHITECTURE_EDGE_LIMIT
        );
        assert_eq!(limits.language_link_limit, MAX_REPORT_LANGUAGE_LINK_LIMIT);
        assert_eq!(limits.hotspot_limit, MAX_REPORT_HOTSPOT_LIMIT);
        assert_eq!(limits.insight_limit, MAX_REPORT_INSIGHT_LIMIT);
    }

    #[tokio::test]
    async fn query_api_rejects_oversized_query_before_scan() {
        let root = temp_server_root();
        fs::create_dir_all(&root).unwrap();
        let state = test_state(root.clone(), vec![], false);
        let query = GraphQuery {
            path: None,
            q: "x".repeat(MAX_GRAPH_QUERY_LENGTH + 1),
        };

        let error = query_api(State(state), Query(query))
            .await
            .expect_err("oversized query should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("query expression is too long"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn source_search_api_rejects_oversized_query_before_scan() {
        let root = temp_server_root();
        fs::create_dir_all(&root).unwrap();
        let state = test_state(root.clone(), vec![], false);
        let query = SourceSearchQuery {
            path: None,
            q: "x".repeat(MAX_SOURCE_SEARCH_QUERY_LENGTH + 1),
            path_filter: None,
            case_sensitive: None,
            limit: None,
            context: None,
        };

        let error = source_search_api(State(state), Query(query))
            .await
            .expect_err("oversized source-search query should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert!(error.message.contains("source-search query is too long"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn capability_endpoints_include_discovery_and_agent_routes() {
        let endpoints: Vec<_> = capability_endpoints()
            .into_iter()
            .flat_map(|group| group.endpoints)
            .collect();

        assert!(endpoints.contains(&"GET /api/capabilities"));
        assert!(endpoints.contains(&"GET /api/schema"));
        assert!(endpoints.contains(&"GET /api/live"));
        assert!(endpoints.contains(&"GET /api/ready"));
        assert!(endpoints.contains(&"GET /api/metrics"));
        assert!(endpoints.contains(&"GET /api/report"));
        assert!(endpoints.contains(&"GET /api/incremental-plan"));
        assert!(endpoints.contains(&"GET /api/incremental-scan"));
        assert!(endpoints.contains(&"GET /api/incremental-merge-preview"));
        assert!(endpoints.contains(&"POST /api/incremental-update"));
        assert!(endpoints.contains(&"GET /api/incremental-update"));
        assert!(endpoints.contains(&"GET /api/cache-chunks"));
        assert!(endpoints.contains(&"GET /api/query"));
        assert!(endpoints.contains(&"GET /api/node-context"));
        assert!(endpoints.contains(&"GET /api/node-card"));
        assert!(endpoints.contains(&"POST /api/scan-jobs"));
        assert!(endpoints.contains(&"POST /api/semantic-jobs"));
    }

    #[test]
    fn embedded_web_overview_uses_report_snapshot() {
        let index = include_str!("../../codegraph-web/static/index.html");
        let app = include_str!("../../codegraph-web/static/app.js");

        assert!(index.contains("riskSummaryList"));
        assert!(app.contains("apiFetch(`/api/report?${reportParams.toString()}`)"));
        assert!(app.contains("state.report?.quality_gate"));
        assert!(app.contains("\"risk.gate\""));
        assert!(index.contains("insightExportButton"));
        assert!(index.contains("checkExportButton"));
        assert!(app.contains("exportCurrentInsights"));
        assert!(app.contains("codegraph.insights_export.v1"));
        assert!(app.contains("\"button.downloadInsights\""));
        assert!(app.contains("exportLastCheckResult"));
        assert!(app.contains("codegraph.check_result.v1"));
        assert!(app.contains("\"button.downloadCheck\""));
        assert!(app.contains("capabilities.server_version"));
        assert!(app.contains("\"cap.server\""));
        assert!(app.contains("\"cap.apiBody\""));
        assert!(app.contains("\"cap.headers\""));
        assert!(app.contains("common_response_headers"));
        assert!(app.contains("\"runtime.lastApi\""));
        assert!(app.contains("lastApiResponse"));
        assert!(app.contains("x-response-time-ms"));
        assert!(app.contains("x-codegraph-export-nodes"));
        assert!(app.contains("x-codegraph-export-edges"));
        assert!(app.contains("x-codegraph-export-bytes"));
        assert!(index.contains("exportSliceButton"));
        assert!(app.contains("exportVisibleGraphSlice"));
        assert!(app.contains("codegraph.visible_slice.v1"));
        assert!(app.contains("\"button.downloadSlice\""));
        assert!(app.contains("openSourceFileGraph"));
        assert!(app.contains("data-source-file-graph"));
        assert!(app.contains("\"button.graphFile\""));
        assert!(index.contains("sourceSearchExportButton"));
        assert!(app.contains("exportLastSourceSearchResult"));
        assert!(app.contains("codegraph.source_search_result.v1"));
        assert!(app.contains("\"button.downloadSourceResults\""));
        assert!(index.contains("entryFlowExportButton"));
        assert!(app.contains("exportLastEntryFlowReport"));
        assert!(app.contains("codegraph.entrypoint_traces.v1"));
        assert!(app.contains("\"button.downloadEntryFlows\""));
        assert!(index.contains("configTraceExportButton"));
        assert!(app.contains("exportLastConfigTraceReport"));
        assert!(app.contains("codegraph.config_trace.v1"));
        assert!(app.contains("\"button.downloadConfigTrace\""));
        assert!(index.contains("errorTraceExportButton"));
        assert!(app.contains("exportLastErrorTraceReport"));
        assert!(app.contains("codegraph.error_trace.v1"));
        assert!(app.contains("\"button.downloadErrorTrace\""));
        assert!(app.contains("exportLastSelectionCard"));
        assert!(app.contains("codegraph.selection_card.v1"));
        assert!(app.contains("data-export-selection-card"));
        assert!(app.contains("\"button.downloadCard\""));
        assert!(app.contains("\"export.selectionCard\""));
        assert!(app.contains("const DEFAULT_LABEL_MODE = \"minimal\""));
        assert!(app.contains("const LABEL_MODE_STORAGE_VERSION = \"13\""));
        assert!(index.contains("data-label-mode=\"minimal\" aria-pressed=\"true\""));
        assert!(app.contains("ambiguous_call_resolution"));
        assert!(app.contains("ambiguous_entrypoint_target"));
        assert!(index.contains("insights kind:ambiguous_call_resolution"));
        assert!(index.contains("insights kind:ambiguous_entrypoint_target"));
        assert!(app.contains("\"queryPreset.ambiguousCalls\""));
        assert!(app.contains("\"queryPreset.ambiguousEntrypoints\""));
        assert!(app.contains("\"check.running\""));
        assert!(app.contains("\"sourceSearch.enterText\""));
        assert!(app.contains("\"sourceSearch.noMatches\""));
        assert!(app.contains("\"entryFlows.tracing\""));
        assert!(app.contains("\"entryFlows.noMatches\""));
        assert!(app.contains("\"entryFlows.reportTruncated\""));
        assert!(app.contains("\"trace.noOutgoing\""));
        assert!(app.contains("\"empty.noHotspots\""));
        assert!(app.contains("\"empty.noAnnotations\""));
        assert!(app.contains("\"empty.noEntrypoints\""));
        assert!(app.contains("\"empty.noScanPolicy\""));
        assert!(app.contains("\"empty.noCoverage\""));
        assert!(app.contains("\"empty.noLspStatus\""));
        assert!(app.contains("\"empty.noSemanticWork\""));
        assert!(app.contains("\"empty.noArchitecture\""));
        assert!(app.contains("\"empty.noLanguageDependencies\""));
        assert!(app.contains("\"focus.hotspot\""));
        assert!(app.contains("\"focus.entrypoint\""));
        assert!(app.contains("\"focus.semantic\""));
        assert!(app.contains("\"focus.architectureEdge\""));
        assert!(app.contains("\"focus.languageDependency\""));
        assert!(app.contains("t(\"focus.hotspot\")"));
        assert!(app.contains("t(\"focus.entrypoint\")"));
        assert!(app.contains("t(\"overview.crossLanguage\")"));
        assert!(app.contains("t(\"overview.areaEdges\")"));
        assert!(app.contains("t(\"empty.noScanPolicy\")"));
        assert!(app.contains("\"configTrace.tracing\""));
        assert!(app.contains("\"configTrace.noReaders\""));
        assert!(app.contains("\"errorTrace.tracing\""));
        assert!(app.contains("\"errorTrace.noSources\""));
        assert!(app.contains("clientEntrypointReachableIds"));
        assert!(app.contains("unreachable_error_flow"));
        assert!(index.contains("unreachable scope:config"));
        assert!(index.contains("unreachable scope:errors"));
        assert!(app.contains("\"queryPreset.unreachableConfig\""));
        assert!(app.contains("\"queryPreset.unreachableErrors\""));
        assert!(index.contains("annotations key:domain edge_limit:300"));
        assert!(app.contains("\"queryPreset.annotations\""));
        assert!(app.contains("limits.max_api_body_bytes"));
        assert!(app.contains("data-risk-gate"));
        assert!(app.contains("checkFailOnInput.value"));
        for endpoint in [
            "/api/summary?",
            "/api/entrypoints?",
            "/api/coverage?",
            "/api/architecture?",
            "/api/language-dependencies?",
            "/api/hotspots?",
        ] {
            assert!(
                !app.contains(endpoint),
                "web overview should use /api/report instead of {endpoint}"
            );
        }
    }

    #[test]
    fn embedded_web_assets_keep_shareable_investigation_links() {
        let index = include_str!("../../codegraph-web/static/index.html");
        let app = include_str!("../../codegraph-web/static/app.js");
        let styles = include_str!("../../codegraph-web/static/styles.css");

        assert!(index.contains("queryCopyButton"));
        assert!(index.contains("button.copyQueryLink"));
        assert!(index.contains("queryExportButton"));
        assert!(index.contains("queryHistory"));
        assert!(index.contains("clearQueryHistoryButton"));
        assert!(index.contains("pageCopyButton"));
        assert!(index.contains("pageClearButton"));
        assert!(index.contains("button.copyPageLink"));
        assert!(index.contains("button.clearFilters"));
        assert!(app.contains("buildSelectionUrl"));
        assert!(app.contains("buildQueryUrl"));
        assert!(app.contains("exportLastQueryResult"));
        assert!(app.contains("codegraph.query_result.v1"));
        assert!(app.contains("\"button.downloadQueryResult\""));
        assert!(app.contains("QUERY_HISTORY_STORAGE_KEY"));
        assert!(app.contains("rememberQuery"));
        assert!(app.contains("renderQueryHistory"));
        assert!(app.contains("\"queryHistory.recent\""));
        assert!(app.contains("buildGraphPageUrl"));
        assert!(app.contains("pendingQueryLink"));
        assert!(app.contains("restorePendingQueryLink"));
        assert!(app.contains("query_focus"));
        assert!(app.contains("pendingGraphPageLink"));
        assert!(app.contains("node_offset"));
        assert!(app.contains("edge_offset"));
        assert!(app.contains("copyCurrentQueryLink"));
        assert!(app.contains("copyGraphPageLink"));
        assert!(app.contains("clearGraphPageFilters"));
        assert!(app.contains("data-copy-selection-link=\"node\""));
        assert!(app.contains("data-copy-selection-link=\"edge\""));
        assert!(app.contains("url.searchParams.delete(\"query\")"));
        assert!(app.contains("url.searchParams.delete(\"node\")"));
        assert!(app.contains("navigator.clipboard?.writeText"));
        assert!(styles.contains(".query-history"));
    }

    #[test]
    fn embedded_web_assets_surface_graph_viewport_hud() {
        let index = include_str!("../../codegraph-web/static/index.html");
        let app = include_str!("../../codegraph-web/static/app.js");
        let styles = include_str!("../../codegraph-web/static/styles.css");

        assert!(index.contains("graphHud"));
        assert!(index.contains("graph-hud"));
        assert!(index.contains("pageScope"));
        assert!(index.contains("edgePrevButton"));
        assert!(index.contains("edgeNextButton"));
        assert!(index.contains("clearCanvasFiltersButton"));
        assert!(index.contains("graphMinimap"));
        assert!(index.contains("data-i18n-aria-label=\"aria.graphMinimap\""));
        assert!(app.contains("renderGraphHud"));
        assert!(app.contains("renderGraphPageScope"));
        assert!(app.contains("shiftEdgePage"));
        assert!(app.contains("setNodeKindFilter"));
        assert!(app.contains("syncKindFilterControls"));
        assert!(app.contains("data-node-kind"));
        assert!(app.contains("clearCanvasFilters"));
        assert!(app.contains("canvasFilterCount"));
        assert!(app.contains("drawGraphMinimap"));
        assert!(app.contains("onMinimapPointerDown"));
        assert!(app.contains("edge_offset: String(state.graphPage.edgeOffset)"));
        assert!(app.contains("\"graph.zoom\""));
        assert!(app.contains("\"graph.layout\""));
        assert!(app.contains("\"graph.slice\""));
        assert!(app.contains("\"graph.filters\""));
        assert!(app.contains("\"legend.kindFilter\""));
        assert!(app.contains("\"aria.graphMinimap\""));
        assert!(app.contains("\"button.clearCanvasFilters\""));
        assert!(app.contains("truncated_edges"));
        assert!(styles.contains(".graph-hud"));
        assert!(styles.contains(".page-scope"));
        assert!(styles.contains(".page-action-info"));
        assert!(styles.contains(".filters > button"));
        assert!(styles.contains(".legend-item.node-kind"));
        assert!(styles.contains(".graph-minimap"));
    }

    #[test]
    fn embedded_web_assets_highlight_hovered_graph_edges() {
        let app = include_str!("../../codegraph-web/static/app.js");

        assert!(app.contains("hoveredEdgeKey"));
        assert!(app.contains("edgeEmphasis"));
        assert!(app.contains("\"hover\""));
        assert!(app.contains("onPointerLeave"));
        assert!(app.contains("edgeHighlightColor"));
    }

    #[test]
    fn embedded_web_assets_support_keyboard_graph_navigation() {
        let index = include_str!("../../codegraph-web/static/index.html");
        let app = include_str!("../../codegraph-web/static/app.js");
        let styles = include_str!("../../codegraph-web/static/styles.css");

        assert!(index.contains("id=\"graphCanvas\" tabindex=\"0\""));
        assert!(index.contains("data-i18n-aria-label=\"aria.graphCanvas\""));
        assert!(app.contains("onCanvasKeyDown"));
        assert!(app.contains("panGraphBy"));
        assert!(app.contains("\"aria.graphCanvas\""));
        assert!(app.contains("data-i18n-aria-label"));
        assert!(styles.contains("#graphCanvas:focus-visible"));
    }

    #[test]
    fn api_schema_lists_agent_contracts() {
        let schema = api_schema_response();
        let endpoints: Vec<_> = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .map(|endpoint| (endpoint.method, endpoint.path))
            .collect();

        assert_eq!(schema.api_version, 1);
        assert_eq!(schema.server_version, SERVER_VERSION);
        assert!(schema.enum_values.contains_key("export_format"));
        assert!(
            schema
                .enum_values
                .get("graph_node_kind")
                .is_some_and(|kinds| kinds.contains(&"function") && kinds.contains(&"environment"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_edge_kind")
                .is_some_and(|kinds| kinds.contains(&"calls") && kinds.contains(&"depends_on"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_confidence")
                .is_some_and(|confidences| confidences.contains(&"semantic")
                    && confidences.contains(&"heuristic"))
        );
        assert!(schema.enum_values.get("insight_kind").is_some_and(|kinds| {
            kinds.contains(&"sensitive_config_default")
                && kinds.contains(&"ambiguous_call_resolution")
                && kinds.contains(&"ambiguous_entrypoint_target")
                && kinds.contains(&"dependency_cycle")
                && kinds.contains(&"custom_rule_*")
        }));
        assert!(
            schema
                .enum_values
                .get("risk_grade")
                .is_some_and(|grades| grades.contains(&"clean") && grades.contains(&"critical"))
        );
        assert!(
            schema
                .enum_values
                .get("project_report_section")
                .is_some_and(|sections| sections.contains(&"risk_summary")
                    && sections.contains(&"quality_gate")
                    && sections.contains(&"coverage"))
        );
        assert!(
            schema
                .enum_values
                .get("semantic_work_status")
                .is_some_and(|statuses| statuses.contains(&"unsupported_language")
                    && !statuses.contains(&"unsupported"))
        );
        assert!(
            schema
                .enum_values
                .get("semantic_work_capability")
                .is_some_and(|capabilities| capabilities.contains(&"document_symbols")
                    && capabilities.contains(&"language_server")
                    && !capabilities.contains(&"symbols"))
        );
        assert!(
            schema
                .enum_values
                .get("cache_status")
                .is_some_and(|statuses| statuses.contains(&"hit")
                    && statuses.contains(&"miss")
                    && statuses.contains(&"disabled"))
        );
        assert!(
            schema
                .enum_values
                .get("cache_record_status")
                .is_some_and(|statuses| statuses.contains(&"present")
                    && statuses.contains(&"missing")
                    && statuses.contains(&"incompatible"))
        );
        assert!(
            schema
                .enum_values
                .get("cache_reuse_strategy")
                .is_some_and(|strategies| strategies.contains(&"partial_reuse")
                    && strategies.contains(&"full_scan")
                    && strategies.contains(&"no_changes"))
        );
        assert!(
            schema
                .enum_values
                .get("incremental_plan_action")
                .is_some_and(|actions| actions.contains(&"partial_rescan")
                    && actions.contains(&"full_scan")
                    && actions.contains(&"noop"))
        );
        assert!(
            schema
                .enum_values
                .get("incremental_merge_blocker_kind")
                .is_some_and(|kinds| kinds.contains(&"incoming_cross_file_edges")
                    && kinds.contains(&"graph_surface_added")
                    && kinds.contains(&"removed_paths"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_command")
                .is_some_and(|commands| {
                    commands.contains(&"entrypoints")
                        && commands.contains(&"symbols")
                        && commands.contains(&"files")
                        && commands.contains(&"routes")
                        && commands.contains(&"packages")
                        && commands.contains(&"configs")
                        && commands.contains(&"errors")
                        && commands.contains(&"cycles")
                        && commands.contains(&"hotspots")
                        && commands.contains(&"unreachable")
                        && commands.contains(&"diagnostics")
                        && commands.contains(&"annotations")
                        && commands.contains(&"insights")
                })
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_entrypoint_term")
                .is_some_and(|terms| terms.contains(&"search") && terms.contains(&"language"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_node_term")
                .is_some_and(|terms| terms.contains(&"metadata.*") && terms.contains(&"search"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_edge_term")
                .is_some_and(|terms| terms.contains(&"edge_index") && terms.contains(&"confidence"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_route_term")
                .is_some_and(|terms| terms.contains(&"method") && terms.contains(&"edge_limit"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_symbol_term")
                .is_some_and(|terms| terms.contains(&"direction") && terms.contains(&"path"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_file_term")
                .is_some_and(|terms| terms.contains(&"path") && terms.contains(&"edge_limit"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_package_term")
                .is_some_and(|terms| terms.contains(&"package") && terms.contains(&"ecosystem"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_config_term")
                .is_some_and(|terms| terms.contains(&"target") && terms.contains(&"depth"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_error_term")
                .is_some_and(|terms| terms.contains(&"target") && terms.contains(&"depth"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_cycle_term")
                .is_some_and(|terms| terms.contains(&"edge_kind") && terms.contains(&"language"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_hotspot_term")
                .is_some_and(|terms| terms.contains(&"min_score") && terms.contains(&"edge_limit"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_unreachable_term")
                .is_some_and(|terms| terms.contains(&"scope") && terms.contains(&"search"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_unreachable_scope")
                .is_some_and(|scopes| scopes.contains(&"source_files")
                    && scopes.contains(&"config")
                    && scopes.contains(&"errors")
                    && scopes.contains(&"any"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_diagnostic_term")
                .is_some_and(|terms| terms.contains(&"severity") && terms.contains(&"language"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_annotation_term")
                .is_some_and(|terms| terms.contains(&"key")
                    && terms.contains(&"value")
                    && terms.contains(&"annotation.*"))
        );
        assert!(
            schema
                .enum_values
                .get("graph_query_insight_term")
                .is_some_and(|terms| terms.contains(&"severity") && terms.contains(&"kind"))
        );
        assert!(
            schema
                .enum_values
                .get("web_deep_link_param")
                .is_some_and(|params| params.contains(&"node")
                    && params.contains(&"edge")
                    && params.contains(&"query")
                    && params.contains(&"query_focus")
                    && params.contains(&"node_offset")
                    && params.contains(&"edge_offset")
                    && params.contains(&"edge_kind")
                    && params.contains(&"edge_source"))
        );
        assert!(schema.common_response_headers.iter().any(|header| {
            header.name == "x-request-id" && header.value_type == "string" && header.required
        }));
        assert!(schema.common_response_headers.iter().any(|header| {
            header.name == RESPONSE_TIME_HEADER && header.value_type == "u64_ms" && header.required
        }));
        assert!(schema.common_response_headers.iter().any(|header| {
            header.name == "etag" && header.value_type == "http_etag" && !header.required
        }));
        assert!(endpoints.contains(&("GET", "/api/schema")));
        let capabilities_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/capabilities")
            .expect("schema should list capabilities endpoint");
        assert!(capabilities_endpoint.response_fields.iter().any(|field| {
            field.name == "server_version" && field.value_type == "semver" && field.required
        }));
        let schema_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/schema")
            .expect("schema should list schema endpoint");
        assert!(schema_endpoint.response_fields.iter().any(|field| {
            field.name == "enum_values" && field.value_type == "map<string,string[]>"
        }));
        assert!(schema_endpoint.response_fields.iter().any(|field| {
            field.name == "common_response_headers" && field.value_type == "ApiHeaderSpec[]"
        }));
        let export_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/export")
            .expect("schema should list export endpoint");
        assert!(export_endpoint.response_headers.iter().any(|header| {
            header.name == EXPORT_NODES_HEADER && header.value_type == "usize" && header.required
        }));
        assert!(export_endpoint.response_headers.iter().any(|header| {
            header.name == EXPORT_EDGES_HEADER && header.value_type == "usize" && header.required
        }));
        assert!(export_endpoint.response_headers.iter().any(|header| {
            header.name == EXPORT_BYTES_HEADER
                && header.value_type == "usize_bytes"
                && header.required
        }));
        assert!(endpoints.contains(&("GET", "/api/live")));
        assert!(endpoints.contains(&("GET", "/api/ready")));
        let live_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/live")
            .expect("schema should list live endpoint");
        assert!(live_endpoint.response_fields.iter().any(|field| {
            field.name == "status" && field.location == "response" && field.required
        }));
        assert!(live_endpoint.response_fields.iter().any(|field| {
            field.name == "server_version" && field.value_type == "semver" && field.required
        }));
        assert!(live_endpoint.response_fields.iter().any(|field| {
            field.name == "cache_enabled" && field.value_type == "bool" && field.required
        }));
        let metrics_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/metrics")
            .expect("schema should list metrics endpoint");
        assert!(metrics_endpoint.response_fields.iter().any(|field| {
            field.name == "uptime_seconds" && field.value_type == "u64" && field.required
        }));
        assert!(metrics_endpoint.response_fields.iter().any(|field| {
            field.name == "scan_jobs" && field.value_type == "JobPoolMetricsResponse"
        }));
        assert!(endpoints.contains(&("GET", "/api/report")));
        assert!(endpoints.contains(&("GET", "/api/cache-diff")));
        assert!(endpoints.contains(&("GET", "/api/cache-chunks")));
        assert!(endpoints.contains(&("GET", "/api/incremental-plan")));
        assert!(endpoints.contains(&("GET", "/api/incremental-scan")));
        assert!(endpoints.contains(&("GET", "/api/incremental-merge-preview")));
        assert!(endpoints.contains(&("POST", "/api/incremental-update")));
        assert!(endpoints.contains(&("GET", "/api/incremental-update")));
        assert!(endpoints.contains(&("GET", "/api/node-card")));
        assert!(endpoints.contains(&("GET", "/api/query")));
        assert!(endpoints.contains(&("POST", "/api/scan-jobs")));
        assert!(endpoints.contains(&("GET", "/api/scan-jobs/{id}/events")));
        let graph_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/graph")
            .expect("schema should list graph endpoint");
        assert!(graph_endpoint.parameters.iter().any(|parameter| {
            parameter.name == "kind" && parameter.value_type == "graph_node_kind"
        }));
        assert!(graph_endpoint.parameters.iter().any(|parameter| {
            parameter.name == "edge_kind" && parameter.value_type == "graph_edge_kind"
        }));
        assert!(graph_endpoint.parameters.iter().any(|parameter| {
            parameter.name == "confidence" && parameter.value_type == "graph_confidence"
        }));
        let graph_node_limit = graph_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "node_limit")
            .expect("graph node_limit");
        assert_eq!(graph_node_limit.minimum, Some(1));
        assert_eq!(graph_node_limit.maximum, Some(MAX_GRAPH_NODE_LIMIT));
        assert_eq!(
            graph_node_limit.capability_limit,
            Some("max_graph_node_limit")
        );
        assert!(graph_endpoint.response_fields.iter().any(|field| {
            field.name == "nodes" && field.value_type == "Node[]" && field.required
        }));
        assert!(
            graph_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "truncated_edges" && field.value_type == "bool" })
        );
        let node_card_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/node-card")
            .expect("schema should list node-card endpoint");
        assert!(node_card_endpoint.response_fields.iter().any(|field| {
            field.name == "context" && field.value_type == "NodeContext" && field.required
        }));
        assert!(
            node_card_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "actions" && field.value_type == "NodeCardAction[]" })
        );
        let focus_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/focus")
            .expect("schema should list focus endpoint");
        assert!(
            focus_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "facets" && field.value_type == "QueryFacets" })
        );
        let semantic_plan_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/semantic-plan")
            .expect("schema should list semantic-plan endpoint");
        let semantic_work_item_limit = semantic_plan_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "work_item_limit")
            .expect("semantic work_item_limit");
        assert_eq!(semantic_work_item_limit.default, Some("100"));
        assert_eq!(semantic_work_item_limit.minimum, Some(1));
        assert_eq!(
            semantic_work_item_limit.maximum,
            Some(MAX_SEMANTIC_WORK_ITEM_LIMIT)
        );
        assert_eq!(
            semantic_work_item_limit.capability_limit,
            Some("max_semantic_work_item_limit")
        );
        let semantic_enrich_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/semantic-enrich")
            .expect("schema should list semantic-enrich endpoint");
        assert_eq!(semantic_enrich_endpoint.body, Some("SemanticEnrichRequest"));
        let enrich_timeout = semantic_enrich_endpoint
            .body_fields
            .iter()
            .find(|field| field.name == "request_timeout_ms")
            .expect("semantic enrich request_timeout_ms");
        assert_eq!(enrich_timeout.location, "body");
        assert_eq!(enrich_timeout.minimum, Some(1));
        assert_eq!(
            enrich_timeout.maximum,
            Some(MAX_SEMANTIC_REQUEST_TIMEOUT_MS as usize)
        );
        assert_eq!(
            enrich_timeout.capability_limit,
            Some("max_semantic_request_timeout_ms")
        );
        let semantic_patch_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/semantic-patch")
            .expect("schema should list semantic-patch endpoint");
        assert_eq!(
            semantic_patch_endpoint.body_capability_limit,
            Some("max_api_body_bytes")
        );
        assert!(
            schema
                .groups
                .iter()
                .flat_map(|group| group.endpoints.iter())
                .filter(|endpoint| endpoint.body.is_some())
                .all(|endpoint| endpoint.body_capability_limit == Some("max_api_body_bytes"))
        );
        assert!(semantic_patch_endpoint.body_fields.iter().any(|field| {
            field.name == "responses"
                && field.location == "body"
                && field.required
                && field.value_type == "SemanticLspResponse[]"
        }));
        let query_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/query")
            .expect("schema should list query endpoint");
        assert!(
            query_endpoint
                .parameters
                .iter()
                .any(|parameter| parameter.name == "q"
                    && parameter.required
                    && parameter.max_length == Some(MAX_GRAPH_QUERY_LENGTH)
                    && parameter.capability_limit == Some("max_graph_query_length"))
        );
        assert!(query_endpoint.response_fields.iter().any(|field| {
            field.name == "returned_nodes" && field.value_type == "usize" && field.required
        }));
        assert!(
            query_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "facets" && field.value_type == "QueryFacets" })
        );
        let explain_edge_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/explain-edge")
            .expect("schema should list explain-edge endpoint");
        assert!(explain_edge_endpoint.response_fields.iter().any(|field| {
            field.name == "edge_index" && field.value_type == "usize" && field.required
        }));
        assert!(
            explain_edge_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "evidence" && field.value_type == "string[]" })
        );
        let report_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/report")
            .expect("schema should list report endpoint");
        let report_insight_limit = report_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "insight_limit")
            .expect("report insight_limit");
        assert_eq!(report_insight_limit.minimum, Some(1));
        assert_eq!(report_insight_limit.maximum, Some(MAX_REPORT_INSIGHT_LIMIT));
        assert_eq!(
            report_insight_limit.capability_limit,
            Some("max_report_insight_limit")
        );
        assert!(
            report_endpoint.response_fields.iter().any(|field| {
                field.name == "coverage" && field.value_type == "ScanCoverageReport"
            })
        );
        assert!(report_endpoint.response_fields.iter().any(|field| {
            field.name == "report" && field.value_type == "ProjectReport" && field.required
        }));
        let architecture_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/architecture")
            .expect("schema should list architecture endpoint");
        let architecture_group_limit = architecture_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "group_limit")
            .expect("architecture group_limit");
        assert_eq!(
            architecture_group_limit.capability_limit,
            Some("max_report_architecture_group_limit")
        );
        assert!(architecture_endpoint.response_fields.iter().any(|field| {
            field.name == "groups" && field.value_type == "ArchitectureGroup[]" && field.required
        }));
        assert!(
            architecture_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "truncated_edges" && field.value_type == "bool" })
        );
        let language_dependencies_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/language-dependencies")
            .expect("schema should list language-dependencies endpoint");
        let language_link_limit = language_dependencies_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "limit")
            .expect("language dependencies limit");
        assert_eq!(
            language_link_limit.capability_limit,
            Some("max_report_language_link_limit")
        );
        assert!(
            language_dependencies_endpoint
                .response_fields
                .iter()
                .any(|field| field.name == "cross_language_edges" && field.value_type == "usize")
        );
        let hotspots_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/hotspots")
            .expect("schema should list hotspots endpoint");
        let hotspot_limit = hotspots_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "limit")
            .expect("hotspots limit");
        assert_eq!(
            hotspot_limit.capability_limit,
            Some("max_report_hotspot_limit")
        );
        assert!(
            hotspots_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "hotspots" && field.value_type == "Hotspot[]" })
        );
        let entrypoint_traces_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/entrypoint-traces")
            .expect("schema should list entrypoint-traces endpoint");
        assert!(
            entrypoint_traces_endpoint
                .response_fields
                .iter()
                .any(|field| field.name == "traces" && field.value_type == "TraceResult[]")
        );
        let trace_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/trace")
            .expect("schema should list trace endpoint");
        assert!(
            trace_endpoint
                .response_fields
                .iter()
                .any(|field| { field.name == "edges" && field.value_type == "Edge[]" })
        );
        let config_trace_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/trace-config")
            .expect("schema should list trace-config endpoint");
        assert!(
            config_trace_endpoint
                .response_fields
                .iter()
                .any(|field| field.name == "matches" && field.value_type == "ConfigTraceMatch[]")
        );
        let error_trace_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/trace-errors")
            .expect("schema should list trace-errors endpoint");
        assert!(
            error_trace_endpoint
                .response_fields
                .iter()
                .any(|field| field.name == "matches" && field.value_type == "ErrorTraceMatch[]")
        );
        let insights_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/insights")
            .expect("schema should list insights endpoint");
        let insight_limit = insights_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "limit")
            .expect("insights limit");
        assert_eq!(insight_limit.maximum, Some(MAX_INSIGHT_LIMIT));
        assert_eq!(insight_limit.capability_limit, Some("max_insight_limit"));
        assert!(insights_endpoint.response_fields.iter().any(|field| {
            field.name == "by_severity" && field.value_type == "map<string,usize>"
        }));
        let check_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/check")
            .expect("schema should list check endpoint");
        let check_limit = check_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "limit")
            .expect("check limit");
        assert_eq!(check_limit.maximum, Some(MAX_INSIGHT_LIMIT));
        assert_eq!(check_limit.capability_limit, Some("max_insight_limit"));
        assert!(check_endpoint.response_fields.iter().any(|field| {
            field.name == "passed" && field.value_type == "bool" && field.required
        }));
        let source_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/source")
            .expect("schema should list source endpoint");
        assert!(
            source_endpoint.response_fields.iter().any(|field| {
                field.name == "lines" && field.value_type == "SourcePreviewLine[]"
            })
        );
        let source_search_endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == "/api/source-search")
            .expect("schema should list source-search endpoint");
        let source_search_query = source_search_endpoint
            .parameters
            .iter()
            .find(|parameter| parameter.name == "q")
            .expect("source-search q");
        assert_eq!(
            source_search_query.max_length,
            Some(MAX_SOURCE_SEARCH_QUERY_LENGTH)
        );
        assert_eq!(
            source_search_query.capability_limit,
            Some("max_source_search_query_length")
        );
        assert!(
            source_search_endpoint.response_fields.iter().any(|field| {
                field.name == "matches" && field.value_type == "SourceSearchMatch[]"
            })
        );
    }

    #[test]
    fn prune_scan_jobs_removes_oldest_terminal_jobs_first() {
        let mut jobs = BTreeMap::new();
        jobs.insert(
            "scan-1".to_string(),
            test_scan_job("scan-1", ScanJobStatus::Complete, 10, Some(20)),
        );
        jobs.insert(
            "scan-2".to_string(),
            test_scan_job("scan-2", ScanJobStatus::Running, 11, None),
        );
        jobs.insert(
            "scan-3".to_string(),
            test_scan_job("scan-3", ScanJobStatus::Failed, 12, Some(30)),
        );

        prune_scan_jobs(&mut jobs, 2);

        assert!(!jobs.contains_key("scan-1"));
        assert!(jobs.contains_key("scan-2"));
        assert!(jobs.contains_key("scan-3"));

        prune_scan_jobs(&mut jobs, 1);

        assert_eq!(jobs.len(), 1);
        assert!(jobs.contains_key("scan-2"));
    }

    #[test]
    fn prune_semantic_jobs_keeps_active_jobs_when_over_limit() {
        let mut jobs = BTreeMap::new();
        jobs.insert(
            "semantic-1".to_string(),
            test_semantic_job("semantic-1", ScanJobStatus::Queued, 10, None),
        );
        jobs.insert(
            "semantic-2".to_string(),
            test_semantic_job("semantic-2", ScanJobStatus::Running, 11, None),
        );

        prune_semantic_jobs(&mut jobs, 1);

        assert_eq!(jobs.len(), 2);
        assert!(jobs.contains_key("semantic-1"));
        assert!(jobs.contains_key("semantic-2"));
    }

    #[test]
    fn job_store_health_counts_statuses() {
        let health = job_store_health([
            ScanJobStatus::Queued,
            ScanJobStatus::Running,
            ScanJobStatus::Complete,
            ScanJobStatus::Complete,
            ScanJobStatus::Failed,
            ScanJobStatus::Canceled,
        ]);

        assert_eq!(health.total, 6);
        assert_eq!(health.queued, 1);
        assert_eq!(health.running, 1);
        assert_eq!(health.complete, 2);
        assert_eq!(health.failed, 1);
        assert_eq!(health.canceled, 1);
    }

    #[test]
    fn concurrency_health_counts_active_and_available_permits() {
        let semaphore = Semaphore::new(3);
        let _permit = semaphore.try_acquire().unwrap();

        let health = concurrency_health(&semaphore, 3);

        assert_eq!(health.limit, 3);
        assert_eq!(health.active, 1);
        assert_eq!(health.available, 2);
    }

    #[test]
    fn parse_job_status_accepts_supported_values() {
        assert_eq!(parse_job_status("queued").unwrap(), ScanJobStatus::Queued);
        assert_eq!(
            parse_job_status("completed").unwrap(),
            ScanJobStatus::Complete
        );
        assert_eq!(
            parse_job_status("cancelled").unwrap(),
            ScanJobStatus::Canceled
        );
        assert!(parse_job_status("waiting").is_err());
        assert_eq!(job_list_limit(Some(usize::MAX)), MAX_JOB_LIST_LIMIT);
    }

    #[test]
    fn sort_scan_jobs_orders_recent_jobs_first() {
        let mut jobs = vec![
            test_scan_job("scan-1", ScanJobStatus::Complete, 10, Some(20)),
            test_scan_job("scan-3", ScanJobStatus::Complete, 10, Some(20)),
            test_scan_job("scan-2", ScanJobStatus::Running, 30, None),
        ];

        sort_scan_jobs_recent_first(&mut jobs);

        let ids: Vec<_> = jobs.into_iter().map(|job| job.id).collect();
        assert_eq!(ids, vec!["scan-2", "scan-3", "scan-1"]);
    }

    #[tokio::test]
    async fn cancel_scan_job_marks_queued_job_terminal() {
        let jobs = RwLock::new(BTreeMap::new());
        insert_scan_job(
            &jobs,
            test_scan_job("scan-1", ScanJobStatus::Queued, 10, None),
            4,
        )
        .await;

        let job = cancel_scan_job_in_store(&jobs, "scan-1", 4).await.unwrap();

        assert_eq!(job.status, ScanJobStatus::Canceled);
        assert!(job.finished_at_unix.is_some());
        assert!(scan_job_is_canceled(&jobs, "scan-1").await);
    }

    #[tokio::test]
    async fn canceled_scan_job_is_not_overwritten_by_worker_update() {
        let jobs = RwLock::new(BTreeMap::new());
        insert_scan_job(
            &jobs,
            test_scan_job("scan-1", ScanJobStatus::Running, 10, None),
            4,
        )
        .await;
        cancel_scan_job_in_store(&jobs, "scan-1", 4).await.unwrap();

        update_scan_job(
            &jobs,
            "scan-1",
            ScanJobUpdate {
                status: ScanJobStatus::Complete,
                message: "complete".to_string(),
                cache: None,
                summary: None,
                graph: None,
            },
            4,
        )
        .await;

        let job = jobs.read().await.get("scan-1").cloned().unwrap();
        assert_eq!(job.status, ScanJobStatus::Canceled);
        assert_eq!(job.message, "canceled");
    }

    #[tokio::test]
    async fn cancel_semantic_job_marks_queued_job_terminal() {
        let jobs = RwLock::new(BTreeMap::new());
        insert_semantic_job(
            &jobs,
            test_semantic_job("semantic-1", ScanJobStatus::Queued, 10, None),
            4,
        )
        .await;

        let job = cancel_semantic_job_in_store(&jobs, "semantic-1", 4)
            .await
            .unwrap();

        assert_eq!(job.status, ScanJobStatus::Canceled);
        assert!(job.finished_at_unix.is_some());
        assert!(semantic_job_is_canceled(&jobs, "semantic-1").await);
    }

    fn test_state(root: PathBuf, additional: Vec<PathBuf>, allow_any_path: bool) -> AppState {
        AppState {
            projects: Arc::new(project_roots(&root, additional).unwrap()),
            root,
            started_at: Instant::now(),
            option_overrides: IndexOptionOverrides::default(),
            allow_any_path,
            cache: None,
            jobs: Arc::new(RwLock::new(BTreeMap::new())),
            semantic_jobs: Arc::new(RwLock::new(BTreeMap::new())),
            max_scan_jobs: DEFAULT_MAX_SCAN_JOBS,
            max_semantic_jobs: DEFAULT_MAX_SEMANTIC_JOBS,
            max_scan_concurrency: DEFAULT_MAX_SCAN_CONCURRENCY,
            max_semantic_concurrency: DEFAULT_MAX_SEMANTIC_CONCURRENCY,
            max_api_body_bytes: DEFAULT_API_BODY_BYTES,
            access_log_enabled: true,
            scan_permits: Arc::new(Semaphore::new(DEFAULT_MAX_SCAN_CONCURRENCY)),
            semantic_permits: Arc::new(Semaphore::new(DEFAULT_MAX_SEMANTIC_CONCURRENCY)),
            next_job_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn test_scan_job(
        id: &str,
        status: ScanJobStatus,
        created_at_unix: u64,
        finished_at_unix: Option<u64>,
    ) -> ScanJob {
        ScanJob {
            id: id.to_string(),
            status,
            path: ".".to_string(),
            message: status_message(status),
            created_at_unix,
            updated_at_unix: finished_at_unix.unwrap_or(created_at_unix),
            finished_at_unix,
            cache: None,
            summary: None,
            graph: None,
        }
    }

    fn test_semantic_job(
        id: &str,
        status: ScanJobStatus,
        created_at_unix: u64,
        finished_at_unix: Option<u64>,
    ) -> SemanticJob {
        SemanticJob {
            id: id.to_string(),
            status,
            path: ".".to_string(),
            message: status_message(status),
            created_at_unix,
            updated_at_unix: finished_at_unix.unwrap_or(created_at_unix),
            finished_at_unix,
            responses: None,
            response_errors: None,
            unmatched_locations: None,
            report: None,
            result: None,
        }
    }

    fn status_message(status: ScanJobStatus) -> String {
        match status {
            ScanJobStatus::Queued => "queued",
            ScanJobStatus::Running => "running",
            ScanJobStatus::Complete => "complete",
            ScanJobStatus::Failed => "failed",
            ScanJobStatus::Canceled => "canceled",
        }
        .to_string()
    }

    fn temp_server_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("codegraph-server-test-{nanos}-{id}"))
    }
}
