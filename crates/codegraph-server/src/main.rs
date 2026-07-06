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
    FocusRequest, GraphSlice, GraphSliceRequest, NodeContext, TraceRequest, TraceStart,
    entrypoints, export_dot, export_ndjson, focus_subgraph, insights, node_context, query_graph,
    slice_graph, summarize, trace,
};
use codegraph_core::CodeGraph;
use codegraph_indexer::{IndexOptions, scan_project};
use codegraph_storage::{GraphCache, default_cache_dir};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::time::sleep;

#[derive(Debug, Parser)]
#[command(name = "codegraph-server")]
#[command(about = "Serve the CodeGraph API and web interface")]
struct Args {
    /// Project root exposed to the scanner.
    #[arg(long, default_value = ".")]
    root: PathBuf,

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

    /// Allow scanning paths outside the configured root.
    #[arg(long)]
    allow_any_path: bool,

    /// Disable persistent graph cache.
    #[arg(long)]
    no_cache: bool,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    cache_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    options: IndexOptions,
    allow_any_path: bool,
    cache: Option<GraphCache>,
    jobs: Arc<RwLock<BTreeMap<String, ScanJob>>>,
    next_job_id: Arc<AtomicU64>,
}

#[derive(Debug, Deserialize)]
struct ScanQuery {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct SourceQuery {
    path: PathBuf,
    start_line: Option<u32>,
    end_line: Option<u32>,
    context: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct TraceQuery {
    path: Option<PathBuf>,
    label: Option<String>,
    node_id: Option<u64>,
    depth: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GraphQuery {
    path: Option<PathBuf>,
    q: String,
}

#[derive(Debug, Deserialize)]
struct GraphSliceQuery {
    path: Option<PathBuf>,
    node_offset: Option<usize>,
    node_limit: Option<usize>,
    edge_offset: Option<usize>,
    edge_limit: Option<usize>,
    kind: Option<String>,
    search: Option<String>,
    language: Option<String>,
    item_kind: Option<String>,
    edge_kind: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
struct ScanJob {
    id: String,
    status: ScanJobStatus,
    path: String,
    message: String,
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
}

#[derive(Debug, Serialize)]
struct ScanJobResult {
    id: String,
    root: String,
    graph: CodeGraph,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    root: String,
    cache_dir: Option<String>,
}

#[derive(Debug, Serialize)]
struct ScanResponse {
    root: String,
    cache: CacheInfo,
    graph: CodeGraph,
}

#[derive(Debug, Clone, Serialize)]
struct CacheInfo {
    status: CacheStatus,
    dir: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CacheStatus {
    Disabled,
    Hit,
    Miss,
}

struct ScanOutput {
    graph: CodeGraph,
    cache: CacheInfo,
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
    let bind_addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;

    let state = AppState {
        root,
        options: IndexOptions {
            include_hidden: args.include_hidden,
            include_ignored: args.include_ignored,
            ..IndexOptions::default()
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
        next_job_id: Arc::new(AtomicU64::new(1)),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/styles.css", get(styles_css))
        .route("/api/health", get(health))
        .route("/api/scan", get(scan))
        .route("/api/scan-jobs", post(start_scan_job))
        .route("/api/scan-jobs/{id}", get(scan_job_status))
        .route("/api/scan-jobs/{id}/events", get(scan_job_events))
        .route("/api/scan-jobs/{id}/result", get(scan_job_result))
        .route("/api/export", get(export_api))
        .route("/api/graph", get(graph_api))
        .route("/api/node-context", get(node_context_api))
        .route("/api/focus", get(focus_api))
        .route("/api/summary", get(summary))
        .route("/api/entrypoints", get(entrypoints_api))
        .route("/api/insights", get(insights_api))
        .route("/api/query", get(query_api))
        .route("/api/trace", get(trace_api))
        .route("/api/source", get(source))
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
    let job = ScanJob {
        id: id.clone(),
        status: ScanJobStatus::Queued,
        path: path.clone(),
        message: "queued".to_string(),
        cache: None,
        summary: None,
        graph: None,
    };
    state.jobs.write().await.insert(id.clone(), job.clone());

    let jobs = Arc::clone(&state.jobs);
    let options = state.options.clone();
    let cache = state.cache.clone();
    tokio::spawn(async move {
        update_scan_job(
            &jobs,
            &id,
            ScanJobStatus::Running,
            "scanning project".to_string(),
            None,
            None,
            None,
        )
        .await;

        let scan_root = root.clone();
        let result =
            tokio::task::spawn_blocking(move || scan_project_with_cache(scan_root, options, cache))
                .await;
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
                    ScanJobStatus::Complete,
                    message,
                    Some(output.cache),
                    Some(summary),
                    Some(graph),
                )
                .await;
            }
            Ok(Err(error)) => {
                update_scan_job(
                    &jobs,
                    &id,
                    ScanJobStatus::Failed,
                    error.to_string(),
                    None,
                    None,
                    None,
                )
                .await;
            }
            Err(error) => {
                update_scan_job(
                    &jobs,
                    &id,
                    ScanJobStatus::Failed,
                    format!("scanner task failed: {error}"),
                    None,
                    None,
                    None,
                )
                .await;
            }
        }
    });

    Ok(Json(job))
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

            let is_terminal = matches!(job.status, ScanJobStatus::Complete | ScanJobStatus::Failed);
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

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        root: state.root.display().to_string(),
        cache_dir: state
            .cache
            .as_ref()
            .map(|cache| cache.dir().display().to_string()),
    })
}

async fn scan(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<ScanResponse>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = state.options.clone();
    let cache = state.cache.clone();
    let root_label = root.display().to_string();
    let output = tokio::task::spawn_blocking(move || scan_project_with_cache(root, options, cache))
        .await
        .map_err(|error| ApiError::internal(format!("scanner task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(ScanResponse {
        root: root_label,
        cache: output.cache,
        graph: output.graph,
    }))
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
            kind: normalize_query_string(query.kind),
            search: normalize_query_string(query.search),
            language: normalize_query_string(query.language),
            item_kind: normalize_query_string(query.item_kind),
            edge_kind: normalize_query_string(query.edge_kind),
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

async fn summary(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<codegraph_analysis::GraphSummary>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(summarize(&graph)))
}

async fn entrypoints_api(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<Vec<codegraph_core::Node>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(entrypoints(&graph)))
}

async fn insights_api(
    State(state): State<AppState>,
    Query(query): Query<ScanQuery>,
) -> Result<Json<codegraph_analysis::InsightReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(insights(&graph)))
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

async fn source(
    State(state): State<AppState>,
    Query(query): Query<SourceQuery>,
) -> Result<Json<SourceResponse>, ApiError> {
    let path = resolve_path(&state, &query.path)?;
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
        .strip_prefix(&state.root)
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

async fn scan_graph(state: &AppState, requested: Option<&Path>) -> Result<CodeGraph, ApiError> {
    let root = resolve_scan_root(state, requested)?;
    let options = state.options.clone();
    let cache = state.cache.clone();
    tokio::task::spawn_blocking(move || scan_project_with_cache(root, options, cache))
        .await
        .map_err(|error| ApiError::internal(format!("scanner task failed: {error}")))?
        .map(|output| output.graph)
        .map_err(|error| ApiError::internal(error.to_string()))
}

fn scan_project_with_cache(
    root: PathBuf,
    options: IndexOptions,
    cache: Option<GraphCache>,
) -> Result<ScanOutput> {
    let Some(cache) = cache else {
        return Ok(ScanOutput {
            graph: scan_project(root, &options)?,
            cache: CacheInfo {
                status: CacheStatus::Disabled,
                dir: None,
            },
        });
    };

    let fingerprint = GraphCache::fingerprint_project(&root, &options)
        .with_context(|| format!("failed to fingerprint {}", root.display()))?;
    if let Ok(Some(graph)) = cache.load(&root, &options, &fingerprint) {
        return Ok(ScanOutput {
            graph,
            cache: CacheInfo {
                status: CacheStatus::Hit,
                dir: Some(cache.dir().display().to_string()),
            },
        });
    }

    let graph = scan_project(&root, &options)?;
    let _ = cache.store(&root, &options, fingerprint, &graph);
    Ok(ScanOutput {
        graph,
        cache: CacheInfo {
            status: CacheStatus::Miss,
            dir: Some(cache.dir().display().to_string()),
        },
    })
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

fn resolve_path(state: &AppState, requested: &Path) -> Result<PathBuf, ApiError> {
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        state.root.join(requested)
    };
    resolve_canonical_path(state, candidate)
}

fn resolve_canonical_path(state: &AppState, candidate: PathBuf) -> Result<PathBuf, ApiError> {
    let canonical = candidate
        .canonicalize()
        .map_err(|error| ApiError::bad_request(format!("invalid path: {error}")))?;

    if !state.allow_any_path && !canonical.starts_with(&state.root) {
        return Err(ApiError::bad_request(
            "path is outside the configured root; restart with --allow-any-path to permit it",
        ));
    }

    Ok(canonical)
}

fn normalize_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

async fn update_scan_job(
    jobs: &RwLock<BTreeMap<String, ScanJob>>,
    id: &str,
    status: ScanJobStatus,
    message: String,
    cache: Option<CacheInfo>,
    summary: Option<codegraph_analysis::GraphSummary>,
    graph: Option<CodeGraph>,
) {
    if let Some(job) = jobs.write().await.get_mut(id) {
        job.status = status;
        job.message = message;
        job.cache = cache;
        job.summary = summary;
        job.graph = graph;
    }
}

fn job_without_graph(mut job: ScanJob) -> ScanJob {
    job.graph = None;
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
