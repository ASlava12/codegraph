use anyhow::{Context, Result};
use async_stream::stream;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::Parser;
use codegraph_analysis::{
    CheckReport, ConfigTraceRequest, ConfigTraceResult, EntrypointTraceReport,
    EntrypointTraceRequest, ErrorTraceRequest, ErrorTraceResult, ExplainEdgeRequest, FocusRequest,
    GraphSlice, GraphSliceRequest, GraphSummary, InsightFilter, InsightReport, InsightSeverity,
    NodeContext, ProjectReport, ProjectReportLimits, SourceSearchRequest, SourceSearchResult,
    TraceRequest, TraceStart, architecture_map, check_insights, entrypoints, explain_edge,
    export_dot, export_ndjson, filter_insight_report, focus_subgraph, hotspots, insights,
    language_dependencies, node_context, project_report, query_graph, search_source, slice_graph,
    summarize, trace, trace_config, trace_dependents, trace_entrypoints, trace_errors,
};
use codegraph_core::{CODEGRAPH_SCHEMA_VERSION, CodeGraph};
use codegraph_indexer::{
    IndexOptionOverrides, IndexOptions, configured_index_options, scan_coverage,
};
use codegraph_lsp::{
    DEFAULT_SEMANTIC_WORK_ITEM_LIMIT, LspDiscoveryReport, SemanticEnrichmentPlan,
    SemanticGraphApplyReport, SemanticGraphApplyResult, SemanticGraphPatch, SemanticLspResponse,
    SemanticLspRunOptions, SemanticReadinessReport, SemanticWorkItemFilter,
    apply_semantic_graph_patch, discover_lsp_servers, run_semantic_execution_batch,
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
use std::fs;
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

#[derive(Debug, Parser)]
#[command(name = "codegraph-server")]
#[command(about = "Serve the CodeGraph API and web interface")]
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
    scan_permits: Arc<Semaphore>,
    semantic_permits: Arc<Semaphore>,
    next_job_id: Arc<AtomicU64>,
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
struct MetricsResponse {
    status: &'static str,
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
    api_version: u32,
    graph_schema_version: u32,
    description: &'static str,
    groups: Vec<ApiSchemaGroup>,
    enum_values: BTreeMap<&'static str, Vec<&'static str>>,
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
    response: &'static str,
    streaming: bool,
}

#[derive(Debug, Serialize)]
struct ApiParameterSpec {
    name: &'static str,
    location: &'static str,
    required: bool,
    value_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<&'static str>,
    description: &'static str,
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
    default_job_list_limit: usize,
    max_job_list_limit: usize,
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
struct SourceResponse {
    path: String,
    start_line: u32,
    end_line: u32,
    lines: Vec<SourceLine>,
}

#[derive(Debug, Serialize)]
struct SourceLine {
    number: u32,
    text: String,
    highlight: bool,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
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

    let max_scan_concurrency = args.max_scan_concurrency.max(1);
    let max_semantic_concurrency = args.max_semantic_concurrency.max(1);
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
        scan_permits: Arc::new(Semaphore::new(max_scan_concurrency)),
        semantic_permits: Arc::new(Semaphore::new(max_semantic_concurrency)),
        next_job_id: Arc::new(AtomicU64::new(1)),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/styles.css", get(styles_css))
        .route("/api/capabilities", get(capabilities_api))
        .route("/api/schema", get(api_schema_api))
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
        .route("/api/incremental-plan", get(incremental_plan_api))
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
        .with_state(state);

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    println!("CodeGraph listening on http://{bind_addr}");
    axum::serve(listener, app).await.context("server failed")?;
    Ok(())
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
    Html(include_str!("../../codegraph-web/static/index.html"))
}

async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../../codegraph-web/static/app.js"),
    )
}

async fn styles_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../codegraph-web/static/styles.css"),
    )
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
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        uptime_seconds: state.started_at.elapsed().as_secs(),
        root: state.root.display().to_string(),
        projects: state.projects.len(),
        languages: language_adapters().len(),
        features: capability_features(state.cache.is_some()).len(),
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
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        root: state.root.display().to_string(),
        projects: project_responses(&state),
        languages: language_responses(),
        export_formats: vec!["json", "dot", "ndjson"],
        features: capability_features(state.cache.is_some()),
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
            default_job_list_limit: DEFAULT_JOB_LIST_LIMIT,
            max_job_list_limit: MAX_JOB_LIST_LIMIT,
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

    let result = tokio::task::spawn_blocking(move || run_semantic_enrichment(root, graph, request))
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
            run_semantic_enrichment(scan_root, output.graph, request)
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
    let limit = query.limit.unwrap_or(100).clamp(1, 10_000);
    let report = tokio::task::spawn_blocking(move || cache.diff(&root, &options, limit))
        .await
        .map_err(|error| ApiError::internal(format!("cache diff task failed: {error}")))?
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
    let limit = query.limit.unwrap_or(100).clamp(1, 10_000);
    let plan = tokio::task::spawn_blocking(move || cache.incremental_plan(&root, &options, limit))
        .await
        .map_err(|error| ApiError::internal(format!("incremental plan task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(plan))
}

async fn export_api(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
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
    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
        body,
    )
        .into_response())
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
            node_limit: query.node_limit.unwrap_or(250),
            edge_offset: query.edge_offset.unwrap_or(0),
            edge_limit: query.edge_limit.unwrap_or(500),
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
        query.edge_limit.unwrap_or(80),
    )
    .ok_or_else(|| ApiError::not_found("node not found"))?;
    Ok(Json(context))
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
            edge_limit: query.edge_limit.unwrap_or(200),
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
            limit: query.limit.unwrap_or(50),
        },
    );
    Ok(Json(check_insights(report, fail_on)))
}

async fn query_api(
    State(state): State<AppState>,
    Query(query): Query<GraphQuery>,
) -> Result<Json<codegraph_analysis::QueryResult>, ApiError> {
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
) -> Result<Json<SourceResponse>, ApiError> {
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
    let context = query.context.unwrap_or(4).min(40);
    let visible_start = requested_start.saturating_sub(context).max(1);
    let visible_end = requested_end.saturating_add(context);
    let display_path = path
        .strip_prefix(&source_root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");

    let response = tokio::task::spawn_blocking(move || {
        let bytes = fs::read(&path)
            .map_err(|error| ApiError::internal(format!("failed to read source: {error}")))?;
        let text = String::from_utf8_lossy(&bytes);
        let mut lines = Vec::new();

        for (index, line) in text.lines().enumerate() {
            let number = index as u32 + 1;
            if number < visible_start {
                continue;
            }
            if number > visible_end {
                break;
            }
            lines.push(SourceLine {
                number,
                text: line.to_string(),
                highlight: number >= requested_start && number <= requested_end,
            });
        }

        Ok(SourceResponse {
            path: display_path,
            start_line: requested_start,
            end_line: requested_end,
            lines,
        })
    })
    .await
    .map_err(|error| ApiError::internal(format!("source task failed: {error}")))??;

    Ok(Json(response))
}

async fn source_search_api(
    State(state): State<AppState>,
    Query(query): Query<SourceSearchQuery>,
) -> Result<Json<SourceSearchResult>, ApiError> {
    let search_root = resolve_scan_root(&state, query.path.as_deref())?;
    let search_text = query.q.trim().to_string();
    if search_text.is_empty() {
        return Err(ApiError::bad_request("source-search requires q"));
    }
    let options = scan_options(&state, &search_root)?;
    let request = SourceSearchRequest {
        query: search_text,
        path_filter: normalize_query_string(query.path_filter),
        case_sensitive: query.case_sensitive.unwrap_or(false),
        limit: query.limit.unwrap_or(50).clamp(1, 1_000),
        context: query.context.unwrap_or(2).min(20),
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
        limit: query.limit.unwrap_or(50),
    })
}

fn project_report_limits_from_query(
    query: &ProjectReportQuery,
) -> Result<ProjectReportLimits, ApiError> {
    Ok(ProjectReportLimits {
        architecture_group_limit: query.architecture_group_limit.unwrap_or(50),
        architecture_edge_limit: query.architecture_edge_limit.unwrap_or(200),
        language_link_limit: query.language_link_limit.unwrap_or(50),
        hotspot_limit: query.hotspot_limit.unwrap_or(25),
        insight_limit: query.insight_limit.unwrap_or(50),
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
        }),
    )
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
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
) -> Result<SemanticEnrichResponse, codegraph_lsp::SemanticLspRunError> {
    let timeout = Duration::from_millis(
        request
            .request_timeout_ms
            .unwrap_or(30_000)
            .clamp(1, 300_000),
    );
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
    let responses = run_semantic_execution_batch(
        &batch,
        &SemanticLspRunOptions {
            request_timeout: timeout,
        },
    )?;
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
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        description: "Machine-readable API contract for CodeGraph clients and agents.",
        groups: api_schema_groups(),
        enum_values: BTreeMap::from([
            ("export_format", vec!["json", "dot", "ndjson"]),
            (
                "job_status",
                vec!["queued", "running", "complete", "failed", "canceled"],
            ),
            ("insight_severity", vec!["info", "warning", "error"]),
            (
                "semantic_work_status",
                vec!["ready", "missing_server", "unsupported"],
            ),
            (
                "semantic_work_capability",
                vec![
                    "definitions",
                    "diagnostics",
                    "symbols",
                    "workspace_symbols",
                    "references",
                ],
            ),
        ]),
    }
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
                ),
                api_get(
                    "/api/schema",
                    "Discover this machine-readable endpoint contract.",
                    vec![],
                    "ApiSchemaResponse",
                ),
                api_get(
                    "/api/health",
                    "Read runtime health, retained job-store counts, and concurrency slots.",
                    vec![],
                    "HealthResponse",
                ),
                api_get(
                    "/api/metrics",
                    "Read runtime metrics including versions, cache state, job stores, and concurrency.",
                    vec![],
                    "MetricsResponse",
                ),
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
                        ),
                    ],
                    "CacheDiffReport",
                ),
                api_get(
                    "/api/incremental-plan",
                    "Plan incremental scan work from the persistent cache fingerprint without scanning the full graph.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum paths per plan list.",
                        ),
                    ],
                    "IncrementalScanPlan",
                ),
                api_post(
                    "/api/scan-jobs",
                    "Queue a long-running scan job.",
                    vec![],
                    Some("ScanJobRequest { path?: string }"),
                    "ScanJob",
                    false,
                ),
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
                        ),
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
                ),
                api_post(
                    "/api/semantic-apply",
                    "Apply semantic LSP responses and return enriched graph plus report.",
                    vec![],
                    Some("SemanticPatchRequest"),
                    "SemanticGraphApplyResult",
                    false,
                ),
                api_post(
                    "/api/semantic-enrich",
                    "Run ready semantic LSP work synchronously and return enriched graph plus report.",
                    vec![],
                    Some("SemanticEnrichRequest"),
                    "SemanticEnrichResponse",
                    false,
                ),
                api_post(
                    "/api/semantic-jobs",
                    "Queue semantic enrichment as a retained async job.",
                    vec![],
                    Some("SemanticEnrichRequest"),
                    "SemanticJob",
                    false,
                ),
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
                        ),
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
                ),
                api_get(
                    "/api/graph",
                    "Read a server-side paged and filtered graph slice.",
                    graph_slice_params(),
                    "GraphSlice",
                ),
                api_get(
                    "/api/node-context",
                    "Read selected node context with neighboring edges.",
                    vec![
                        path_param(),
                        query_param("node_id", true, "u64", None, "Node numeric id."),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("80"),
                            "Maximum context edges.",
                        ),
                    ],
                    "NodeContext",
                ),
                api_get(
                    "/api/focus",
                    "Build a focused subgraph from node ids and edge indexes.",
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
                        ),
                    ],
                    "QueryResult",
                ),
                api_get(
                    "/api/summary",
                    "Summarize graph node/edge facts and facets.",
                    vec![path_param()],
                    "GraphSummary",
                ),
                api_get(
                    "/api/query",
                    "Run a focused graph query expression.",
                    vec![
                        path_param(),
                        query_param("q", true, "string", None, "Graph query expression."),
                    ],
                    "QueryResult",
                ),
                api_get(
                    "/api/explain-edge",
                    "Explain why an edge exists with confidence and provenance evidence.",
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
                ),
            ],
        },
        ApiSchemaGroup {
            group: "analysis",
            endpoints: vec![
                api_get(
                    "/api/report",
                    "Return a production project report snapshot with cache, coverage, summary, quality gate, topology, and hotspots.",
                    report_params(),
                    "ProjectReportResponse",
                ),
                api_get(
                    "/api/architecture",
                    "Group files and cross-area dependencies by top-level project area.",
                    vec![
                        path_param(),
                        query_param("group_limit", false, "usize", Some("50"), "Maximum groups."),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("200"),
                            "Maximum inter-group edges.",
                        ),
                    ],
                    "ArchitectureMap",
                ),
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
                        ),
                    ],
                    "LanguageDependencyReport",
                ),
                api_get(
                    "/api/hotspots",
                    "List high-degree files, functions, entrypoints, and config nodes.",
                    vec![
                        path_param(),
                        query_param("limit", false, "usize", Some("25"), "Maximum hotspots."),
                    ],
                    "HotspotReport",
                ),
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
                        query_param("depth", false, "usize", Some("3"), "Maximum trace depth."),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("25"),
                            "Maximum entrypoint traces.",
                        ),
                    ],
                    "EntrypointTraceReport",
                ),
                api_get(
                    "/api/insights",
                    "List investigation insights with severity, kind, and search filters.",
                    insight_params(),
                    "InsightReport",
                ),
                api_get(
                    "/api/check",
                    "Run a quality gate over insights.",
                    check_params(),
                    "CheckReport",
                ),
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
                        query_param("depth", false, "usize", Some("2"), "Maximum trace depth."),
                    ],
                    "TraceResult?",
                ),
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
                        query_param("depth", false, "usize", Some("3"), "Maximum trace depth."),
                    ],
                    "TraceResult?",
                ),
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
                        ),
                        query_param("limit", false, "usize", Some("50"), "Maximum paths."),
                    ],
                    "ConfigTraceResult",
                ),
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
                        ),
                        query_param("limit", false, "usize", Some("50"), "Maximum paths."),
                    ],
                    "ErrorTraceResult",
                ),
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
                        query_param("context", false, "u32", None, "Context lines around span."),
                    ],
                    "SourceResponse",
                ),
                api_get(
                    "/api/source-search",
                    "Search source text with compact context snippets.",
                    vec![
                        path_param(),
                        query_param("q", true, "string", None, "Search text."),
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
                        query_param("limit", false, "usize", Some("50"), "Maximum matches."),
                        query_param(
                            "context",
                            false,
                            "usize",
                            Some("2"),
                            "Context lines per match.",
                        ),
                    ],
                    "SourceSearchResult",
                ),
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
        query_param("node_limit", false, "usize", Some("250"), "Node page size."),
        query_param(
            "edge_offset",
            false,
            "usize",
            Some("0"),
            "Edge page offset.",
        ),
        query_param("edge_limit", false, "usize", Some("500"), "Edge page size."),
        query_param(
            "path_prefix",
            false,
            "string",
            None,
            "Restrict nodes by path prefix.",
        ),
        query_param("kind", false, "string", None, "Restrict nodes by kind."),
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
            "string",
            None,
            "Restrict edges by kind.",
        ),
        query_param(
            "confidence",
            false,
            "string",
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
            "Maximum architecture groups.",
        ),
        query_param(
            "architecture_edge_limit",
            false,
            "usize",
            Some("200"),
            "Maximum architecture edges.",
        ),
        query_param(
            "language_link_limit",
            false,
            "usize",
            Some("50"),
            "Maximum language dependency links.",
        ),
        query_param(
            "hotspot_limit",
            false,
            "usize",
            Some("25"),
            "Maximum hotspots.",
        ),
        query_param(
            "insight_limit",
            false,
            "usize",
            Some("50"),
            "Maximum returned insights; total counts stay complete.",
        ),
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
            "string",
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
        ),
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
            "string",
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
        ),
    ]
}

fn semantic_filter_params() -> Vec<ApiParameterSpec> {
    vec![
        path_param(),
        query_param(
            "work_item_limit",
            false,
            "usize",
            Some("25"),
            "Maximum semantic work items.",
        ),
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
        response,
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
        description: "Retained job id.",
    }
}

fn job_status_param() -> ApiParameterSpec {
    query_param("status", false, "job_status", None, "Filter by job status.")
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
        description,
    }
}

fn capability_features(cache_enabled: bool) -> Vec<&'static str> {
    let mut features = vec![
        "multi_project_roots",
        "api_schema",
        "repository_scan_policy",
        "incremental_scan_plan",
        "mixed_language_syntax_graph",
        "source_preview",
        "graph_paging",
        "node_context",
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
        "sse_job_events",
        "semantic_lsp",
        "web_canvas",
        "i18n_en_ru",
        "dot_export",
        "ndjson_export",
    ];
    if cache_enabled {
        features.push("persistent_graph_cache");
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
                "GET /api/incremental-plan",
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
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn capability_features_reflect_cache_availability() {
        let without_cache = capability_features(false);
        let with_cache = capability_features(true);

        assert!(without_cache.contains(&"api_schema"));
        assert!(without_cache.contains(&"incremental_scan_plan"));
        assert!(without_cache.contains(&"async_scan_jobs"));
        assert!(without_cache.contains(&"job_cancellation"));
        assert!(without_cache.contains(&"runtime_metrics"));
        assert!(without_cache.contains(&"project_report"));
        assert!(without_cache.contains(&"semantic_lsp"));
        assert!(!without_cache.contains(&"persistent_graph_cache"));
        assert!(with_cache.contains(&"persistent_graph_cache"));
    }

    #[test]
    fn capability_endpoints_include_discovery_and_agent_routes() {
        let endpoints: Vec<_> = capability_endpoints()
            .into_iter()
            .flat_map(|group| group.endpoints)
            .collect();

        assert!(endpoints.contains(&"GET /api/capabilities"));
        assert!(endpoints.contains(&"GET /api/schema"));
        assert!(endpoints.contains(&"GET /api/metrics"));
        assert!(endpoints.contains(&"GET /api/report"));
        assert!(endpoints.contains(&"GET /api/incremental-plan"));
        assert!(endpoints.contains(&"GET /api/query"));
        assert!(endpoints.contains(&"GET /api/node-context"));
        assert!(endpoints.contains(&"POST /api/scan-jobs"));
        assert!(endpoints.contains(&"POST /api/semantic-jobs"));
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
        assert!(schema.enum_values.contains_key("export_format"));
        assert!(endpoints.contains(&("GET", "/api/schema")));
        assert!(endpoints.contains(&("GET", "/api/report")));
        assert!(endpoints.contains(&("GET", "/api/cache-diff")));
        assert!(endpoints.contains(&("GET", "/api/incremental-plan")));
        assert!(endpoints.contains(&("GET", "/api/query")));
        assert!(endpoints.contains(&("POST", "/api/scan-jobs")));
        assert!(endpoints.contains(&("GET", "/api/scan-jobs/{id}/events")));
        assert!(
            schema
                .groups
                .iter()
                .flat_map(|group| group.endpoints.iter())
                .find(|endpoint| endpoint.path == "/api/query")
                .is_some_and(|endpoint| endpoint
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == "q" && parameter.required))
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
