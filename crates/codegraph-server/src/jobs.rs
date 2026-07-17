//! Async scan and semantic job lifecycle: start/list/status/cancel/SSE
//! handlers, bounded job stores, and job health counters.

use anyhow::Result;
use async_stream::stream;
use axum::Json;
use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use codegraph_analysis::summarize;
use codegraph_core::CodeGraph;
use codegraph_lsp::{
    DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS, DEFAULT_SEMANTIC_WORK_ITEM_LIMIT, SemanticLspCache,
    SemanticLspRunOptions, SemanticWorkItemFilter, apply_semantic_graph_patch,
    normalize_semantic_request_timeout_ms, run_semantic_execution_batch_cached,
    semantic_execution_batch, semantic_graph_patch_from_responses,
};
use codegraph_storage::{CacheInfo, CacheStatus, scan_project_cached};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::sleep;

#[allow(unused_imports)]
use crate::*;

pub(crate) async fn start_scan_job(
    State(state): State<AppState>,
    Json(request): Json<ScanJobRequest>,
) -> Result<Json<ScanJob>, ApiError> {
    let root = resolve_scan_root(&state, request.path.as_deref())?;
    // Resolve scan options before inserting the job. If this fails (e.g. a
    // malformed .codegraph/config.toml) we must not leave a Queued job behind
    // that never receives a terminal update and is polled forever over SSE.
    let options = scan_options(&state, &root)?;
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

pub(crate) async fn list_scan_jobs(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<JobListQuery>,
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

pub(crate) async fn scan_job_status(
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

pub(crate) async fn cancel_scan_job(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<ScanJob>, ApiError> {
    let job = cancel_scan_job_in_store(&state.jobs, &id, state.max_scan_jobs).await?;
    Ok(Json(job_without_graph(job)))
}

pub(crate) async fn scan_job_events(
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

pub(crate) async fn scan_job_result(
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

#[derive(Debug)]
pub(crate) struct ScanJobUpdate {
    pub(crate) status: ScanJobStatus,
    pub(crate) message: String,
    pub(crate) cache: Option<CacheInfo>,
    pub(crate) summary: Option<codegraph_analysis::GraphSummary>,
    pub(crate) graph: Option<CodeGraph>,
}

#[derive(Debug)]
pub(crate) struct SemanticJobUpdate {
    pub(crate) status: ScanJobStatus,
    pub(crate) message: String,
    pub(crate) result: Option<SemanticEnrichResponse>,
}

pub(crate) async fn insert_scan_job(
    jobs: &RwLock<BTreeMap<String, ScanJob>>,
    job: ScanJob,
    max_jobs: usize,
) {
    let mut jobs = jobs.write().await;
    jobs.insert(job.id.clone(), job);
    prune_scan_jobs(&mut jobs, max_jobs);
}

pub(crate) async fn update_scan_job(
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

pub(crate) async fn cancel_scan_job_in_store(
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

pub(crate) async fn scan_job_is_canceled(
    jobs: &RwLock<BTreeMap<String, ScanJob>>,
    id: &str,
) -> bool {
    jobs.read()
        .await
        .get(id)
        .is_some_and(|job| job.status == ScanJobStatus::Canceled)
}

pub(crate) fn prune_scan_jobs(jobs: &mut BTreeMap<String, ScanJob>, max_jobs: usize) {
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

pub(crate) fn run_semantic_enrichment(
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

pub(crate) async fn insert_semantic_job(
    jobs: &RwLock<BTreeMap<String, SemanticJob>>,
    job: SemanticJob,
    max_jobs: usize,
) {
    let mut jobs = jobs.write().await;
    jobs.insert(job.id.clone(), job);
    prune_semantic_jobs(&mut jobs, max_jobs);
}

pub(crate) async fn update_semantic_job(
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

pub(crate) async fn cancel_semantic_job_in_store(
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

pub(crate) async fn semantic_job_is_canceled(
    jobs: &RwLock<BTreeMap<String, SemanticJob>>,
    id: &str,
) -> bool {
    jobs.read()
        .await
        .get(id)
        .is_some_and(|job| job.status == ScanJobStatus::Canceled)
}

pub(crate) fn prune_semantic_jobs(jobs: &mut BTreeMap<String, SemanticJob>, max_jobs: usize) {
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

pub(crate) fn job_store_health(
    statuses: impl IntoIterator<Item = ScanJobStatus>,
) -> JobStoreHealth {
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

pub(crate) fn concurrency_health(semaphore: &Semaphore, limit: usize) -> ConcurrencyHealth {
    let limit = limit.max(1);
    let available = semaphore.available_permits().min(limit);
    ConcurrencyHealth {
        limit,
        active: limit.saturating_sub(available),
        available,
    }
}

pub(crate) fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn job_without_graph(mut job: ScanJob) -> ScanJob {
    job.graph = None;
    job
}

pub(crate) fn semantic_job_without_result(mut job: SemanticJob) -> SemanticJob {
    job.result = None;
    job
}
