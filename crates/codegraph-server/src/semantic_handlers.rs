//! Semantic enrichment endpoints: readiness, planning, batches, patches,
//! apply, and the async enrich flow.

use anyhow::Result;
use async_stream::stream;
use axum::Json;
use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use codegraph_analysis::summarize;
use codegraph_lsp::{
    DEFAULT_SEMANTIC_WORK_ITEM_LIMIT, SemanticEnrichmentPlan, SemanticGraphApplyResult,
    SemanticGraphPatch, SemanticLspCache, SemanticReadinessReport, SemanticWorkItemFilter,
    apply_semantic_graph_patch, semantic_enrichment_plan_with_filter, semantic_execution_batch,
    semantic_graph_patch_from_responses, semantic_readiness,
};
use codegraph_storage::scan_project_cached;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::sleep;

#[allow(unused_imports)]
use crate::*;

pub(crate) async fn semantic_readiness_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ScanQuery>,
) -> Result<Json<SemanticReadinessReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let summary = summarize(&graph);
    Ok(Json(semantic_readiness(&summary.languages)))
}

pub(crate) async fn semantic_plan_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SemanticPlanQuery>,
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

pub(crate) async fn semantic_batch_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SemanticPlanQuery>,
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

pub(crate) async fn semantic_patch_api(
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

pub(crate) async fn semantic_apply_api(
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

pub(crate) async fn semantic_enrich_api(
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

pub(crate) async fn start_semantic_job(
    State(state): State<AppState>,
    Json(request): Json<SemanticEnrichRequest>,
) -> Result<Json<SemanticJob>, ApiError> {
    let root = resolve_scan_root(&state, request.path.as_deref())?;
    // Resolve scan options before inserting the job, so a config error can't
    // leave a Queued job that never reaches a terminal state (see start_scan_job).
    let options = scan_options(&state, &root)?;
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
    insert_semantic_job(&state.semantic_jobs, job.clone(), state.max_semantic_jobs)
        .await
        .map_err(|active| {
            ApiError::too_many_requests(format!(
                "{active} semantic jobs are already queued or running (limit {}); wait or cancel one",
                state.max_semantic_jobs
            ))
        })?;

    let jobs = Arc::clone(&state.semantic_jobs);
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

pub(crate) async fn list_semantic_jobs(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<JobListQuery>,
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

pub(crate) async fn semantic_job_status(
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

pub(crate) async fn cancel_semantic_job(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<SemanticJob>, ApiError> {
    let job =
        cancel_semantic_job_in_store(&state.semantic_jobs, &id, state.max_semantic_jobs).await?;
    Ok(Json(semantic_job_without_result(job)))
}

pub(crate) async fn semantic_job_events(
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

pub(crate) async fn semantic_job_result(
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
