//! Request parsing and the structured API error contract: root/path
//! resolution, filter/limit parsing, node id parsing, and the ApiQuery
//! extractor that maps rejections to JSON errors.

use anyhow::{Context, Result};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use codegraph_analysis::{
    DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT, DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
    DEFAULT_REPORT_COMMUNITY_LIMIT, DEFAULT_REPORT_FILE_SUMMARY_LIMIT,
    DEFAULT_REPORT_HOTSPOT_LIMIT, DEFAULT_REPORT_INSIGHT_LIMIT, DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
    DEFAULT_REPORT_NODE_SUMMARY_LIMIT, InsightFilter, InsightSeverity,
    MAX_REPORT_ARCHITECTURE_EDGE_LIMIT, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT,
    MAX_REPORT_COMMUNITY_LIMIT, MAX_REPORT_FILE_SUMMARY_LIMIT, MAX_REPORT_HOTSPOT_LIMIT,
    MAX_REPORT_INSIGHT_LIMIT, MAX_REPORT_LANGUAGE_LINK_LIMIT, MAX_REPORT_NODE_SUMMARY_LIMIT,
    ProjectReportLimits, WorkflowFilters,
};
use codegraph_core::CodeGraph;
use codegraph_indexer::{IndexOptions, configured_index_options};
use codegraph_storage::scan_project_cached;
use std::path::{Path, PathBuf};

#[allow(unused_imports)]
use crate::*;

pub(crate) async fn scan_graph(
    state: &AppState,
    requested: Option<&Path>,
) -> Result<CodeGraph, ApiError> {
    let root = resolve_scan_root(state, requested)?;
    let options = scan_options(state, &root)?;
    let cache = state.cache.clone();
    tokio::task::spawn_blocking(move || scan_project_cached(root, &options, cache.as_ref()))
        .await
        .map_err(|error| ApiError::internal(format!("scanner task failed: {error}")))?
        .map(|output| output.graph)
        .map_err(|error| ApiError::internal(error.to_string()))
}

pub(crate) fn scan_options(state: &AppState, root: &Path) -> Result<IndexOptions, ApiError> {
    configured_index_options(root, &state.option_overrides)
        .map_err(|error| ApiError::internal(error.to_string()))
}

pub(crate) fn resolve_scan_root(
    state: &AppState,
    requested: Option<&Path>,
) -> Result<PathBuf, ApiError> {
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

pub(crate) fn resolve_path(
    state: &AppState,
    base_root: &Path,
    requested: &Path,
) -> Result<PathBuf, ApiError> {
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        base_root.join(requested)
    };
    resolve_canonical_path(state, candidate)
}

pub(crate) fn resolve_canonical_path(
    state: &AppState,
    candidate: PathBuf,
) -> Result<PathBuf, ApiError> {
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

pub(crate) fn project_roots(root: &Path, additional: Vec<PathBuf>) -> Result<Vec<ProjectRoot>> {
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

pub(crate) fn project_name(path: &Path, default: bool) -> String {
    let fallback = if default { "Root" } else { "Project" };
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

pub(crate) fn normalize_query_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn workflow_filters_from_query(
    edge_kind: Option<String>,
    confidence: Option<String>,
    language: Option<String>,
    risk_severity: Option<String>,
    block_kind: Option<String>,
) -> WorkflowFilters {
    WorkflowFilters {
        edge_kind: normalize_query_string(edge_kind),
        confidence: normalize_query_string(confidence),
        language: normalize_query_string(language),
        risk_severity: normalize_query_string(risk_severity),
        block_kind: normalize_query_string(block_kind),
    }
}

pub(crate) fn parse_optional_job_status(
    value: Option<&str>,
) -> Result<Option<ScanJobStatus>, ApiError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(parse_job_status)
        .transpose()
}

pub(crate) fn parse_job_status(value: &str) -> Result<ScanJobStatus, ApiError> {
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

pub(crate) fn job_list_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_JOB_LIST_LIMIT)
        .clamp(1, MAX_JOB_LIST_LIMIT)
}

pub(crate) fn sort_scan_jobs_recent_first(jobs: &mut [ScanJob]) {
    jobs.sort_by(|left, right| {
        right
            .updated_at_unix
            .cmp(&left.updated_at_unix)
            .then_with(|| right.created_at_unix.cmp(&left.created_at_unix))
            .then_with(|| right.id.cmp(&left.id))
    });
}

pub(crate) fn sort_semantic_jobs_recent_first(jobs: &mut [SemanticJob]) {
    jobs.sort_by(|left, right| {
        right
            .updated_at_unix
            .cmp(&left.updated_at_unix)
            .then_with(|| right.created_at_unix.cmp(&left.created_at_unix))
            .then_with(|| right.id.cmp(&left.id))
    });
}

pub(crate) fn insight_filter_from_query(query: InsightQuery) -> Result<InsightFilter, ApiError> {
    Ok(InsightFilter {
        severity: normalize_query_string(query.severity)
            .map(|value| parse_insight_severity(&value))
            .transpose()?,
        kind: normalize_query_string(query.kind),
        search: normalize_query_string(query.search),
        limit: query.limit.unwrap_or(DEFAULT_INSIGHT_LIMIT),
    })
}

pub(crate) fn project_report_limits_from_query(
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
        community_limit: query
            .community_limit
            .unwrap_or(DEFAULT_REPORT_COMMUNITY_LIMIT)
            .clamp(1, MAX_REPORT_COMMUNITY_LIMIT),
        insight_limit: query
            .insight_limit
            .unwrap_or(DEFAULT_REPORT_INSIGHT_LIMIT)
            .clamp(1, MAX_REPORT_INSIGHT_LIMIT),
        file_summary_limit: query
            .file_summary_limit
            .unwrap_or(DEFAULT_REPORT_FILE_SUMMARY_LIMIT)
            .clamp(1, MAX_REPORT_FILE_SUMMARY_LIMIT),
        node_summary_limit: query
            .node_summary_limit
            .unwrap_or(DEFAULT_REPORT_NODE_SUMMARY_LIMIT)
            .clamp(1, MAX_REPORT_NODE_SUMMARY_LIMIT),
        fail_on: normalize_query_string(query.fail_on.clone())
            .map(|value| parse_insight_severity(&value))
            .transpose()?
            .unwrap_or(InsightSeverity::Error),
    })
}

pub(crate) fn parse_insight_severity(value: &str) -> Result<InsightSeverity, ApiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "info" => Ok(InsightSeverity::Info),
        "warning" | "warn" => Ok(InsightSeverity::Warning),
        "error" => Ok(InsightSeverity::Error),
        other => Err(ApiError::bad_request(format!(
            "invalid severity `{other}`; expected info, warning, or error"
        ))),
    }
}

/// Node ids arrive as bare numerics (`42`) or n-prefixed (`n42`) — the form
/// printed by query results and web deep links (audit F8).
pub(crate) fn parse_node_id_param(value: &str) -> Result<codegraph_core::NodeId, ApiError> {
    codegraph_analysis::parse_node_id(value).map_err(|_| {
        ApiError::bad_request(format!(
            "invalid node_id `{value}`; use a numeric id or the n-prefixed form such as n42"
        ))
    })
}

pub(crate) fn parse_node_ids(value: Option<&str>) -> Result<Vec<codegraph_core::NodeId>, ApiError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse_node_id_param)
        .collect()
}

pub(crate) fn parse_edge_indexes(value: Option<&str>) -> Result<Vec<usize>, ApiError> {
    parse_usize_list(value, "edge_indexes")
}

pub(crate) fn parse_usize_list(value: Option<&str>, name: &str) -> Result<Vec<usize>, ApiError> {
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

pub(crate) async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorBody {
            error: "not found".to_string(),
            request_id: current_request_id(),
        }),
    )
}

/// Query extractor returning the structured JSON error contract (with
/// `request_id`) instead of axum's plain-text rejection when query-string
/// deserialization fails (audit F9).
pub(crate) struct ApiQuery<T>(pub(crate) T);

impl<S, T> axum::extract::FromRequestParts<S> for ApiQuery<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(query) => Ok(ApiQuery(query.0)),
            Err(rejection) => Err(ApiError::bad_request(format!(
                "invalid query parameters: {}",
                rejection.body_text()
            ))),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiError {
    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
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
