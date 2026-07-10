//! System and discovery endpoints: embedded UI assets, probes, health,
//! metrics, capabilities, schema, languages, and LSP discovery.

use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, Response};
use codegraph_analysis::{
    DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT, DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
    DEFAULT_REPORT_COMMUNITY_LIMIT, DEFAULT_REPORT_FILE_SUMMARY_LIMIT,
    DEFAULT_REPORT_HOTSPOT_LIMIT, DEFAULT_REPORT_INSIGHT_LIMIT, DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
    DEFAULT_REPORT_NODE_SUMMARY_LIMIT, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT,
    MAX_REPORT_ARCHITECTURE_GROUP_LIMIT, MAX_REPORT_COMMUNITY_LIMIT, MAX_REPORT_FILE_SUMMARY_LIMIT,
    MAX_REPORT_HOTSPOT_LIMIT, MAX_REPORT_INSIGHT_LIMIT, MAX_REPORT_LANGUAGE_LINK_LIMIT,
    MAX_REPORT_NODE_SUMMARY_LIMIT, REPORT_QUALITY_GATE_SAMPLE_LIMIT,
};
use codegraph_core::CODEGRAPH_SCHEMA_VERSION;
use codegraph_lsp::{
    DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS, DEFAULT_SEMANTIC_WORK_ITEM_LIMIT, LspDiscoveryReport,
    MAX_SEMANTIC_REQUEST_TIMEOUT_MS, MAX_SEMANTIC_WORK_ITEM_LIMIT, discover_lsp_servers,
};
use codegraph_parser::language_adapters;

#[allow(unused_imports)]
use crate::*;

pub(crate) async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub(crate) async fn label_policy_js(headers: HeaderMap) -> Response {
    static_asset_response(
        &headers,
        "application/javascript; charset=utf-8",
        LABEL_POLICY_JS,
    )
}

pub(crate) async fn app_js(headers: HeaderMap) -> Response {
    static_asset_response(&headers, "application/javascript; charset=utf-8", APP_JS)
}

pub(crate) async fn styles_css(headers: HeaderMap) -> Response {
    static_asset_response(&headers, "text/css; charset=utf-8", STYLES_CSS)
}

pub(crate) async fn live_api(State(state): State<AppState>) -> Json<ProbeResponse> {
    Json(probe_response(&state, "ok"))
}

pub(crate) async fn ready_api(
    State(state): State<AppState>,
) -> Result<Json<ProbeResponse>, ApiError> {
    let _ = scan_options(&state, &state.root)?;
    Ok(Json(probe_response(&state, "ready")))
}

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, ApiError> {
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

pub(crate) fn probe_response(state: &AppState, status: &'static str) -> ProbeResponse {
    ProbeResponse {
        status,
        server_version: SERVER_VERSION,
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        root: state.root.display().to_string(),
        cache_enabled: state.cache.is_some(),
    }
}

pub(crate) async fn metrics_api(
    State(state): State<AppState>,
) -> Result<Json<MetricsResponse>, ApiError> {
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

pub(crate) async fn capabilities_api(
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
        export_formats: vec![
            "json",
            "dot",
            "ndjson",
            "graphml",
            "svg",
            "mermaid_html",
            "cypher",
            "falkordb",
        ],
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
            default_report_community_limit: DEFAULT_REPORT_COMMUNITY_LIMIT,
            max_report_community_limit: MAX_REPORT_COMMUNITY_LIMIT,
            default_report_insight_limit: DEFAULT_REPORT_INSIGHT_LIMIT,
            max_report_insight_limit: MAX_REPORT_INSIGHT_LIMIT,
            report_quality_gate_sample_limit: REPORT_QUALITY_GATE_SAMPLE_LIMIT,
            default_report_file_summary_limit: DEFAULT_REPORT_FILE_SUMMARY_LIMIT,
            max_report_file_summary_limit: MAX_REPORT_FILE_SUMMARY_LIMIT,
            default_report_node_summary_limit: DEFAULT_REPORT_NODE_SUMMARY_LIMIT,
            max_report_node_summary_limit: MAX_REPORT_NODE_SUMMARY_LIMIT,
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

pub(crate) async fn api_schema_api() -> Json<ApiSchemaResponse> {
    Json(api_schema_response())
}

pub(crate) async fn languages_api() -> Json<Vec<LanguageResponse>> {
    Json(language_responses())
}

pub(crate) async fn lsp_api() -> Json<LspDiscoveryReport> {
    Json(discover_lsp_servers())
}
