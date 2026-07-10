//! CodeGraph HTTP server binary: axum application wiring, graceful
//! shutdown, and module layout. Feature areas live in focused modules;
//! see each module's docs.

use anyhow::{Context, Result};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware as axum_middleware;
use axum::routing::{get, post};
use clap::Parser;
use codegraph_indexer::IndexOptionOverrides;
use codegraph_storage::{GraphCache, default_cache_dir};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio::sync::{RwLock, Semaphore};

mod analysis_handlers;
mod contracts;
mod jobs;
mod limits;
mod middleware;
mod params;
mod schema;
mod semantic_handlers;
mod system_handlers;

#[cfg(test)]
mod tests;

pub(crate) use analysis_handlers::*;
pub(crate) use contracts::*;
pub(crate) use jobs::*;
pub(crate) use limits::*;
pub(crate) use middleware::*;
pub(crate) use params::*;
pub(crate) use schema::*;
pub(crate) use semantic_handlers::*;
pub(crate) use system_handlers::*;

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
        .route("/api/surprising-links", get(surprising_links_api))
        .route("/api/hotspots", get(hotspots_api))
        .route("/api/communities", get(communities_api))
        .route("/api/entrypoints", get(entrypoints_api))
        .route("/api/entrypoint-traces", get(entrypoint_traces_api))
        .route("/api/entrypoint-workflows", get(entrypoint_workflows_api))
        .route("/api/insights", get(insights_api))
        .route("/api/check", get(check_api))
        .route("/api/query", get(query_api))
        .route("/api/ask", get(ask_api))
        .route("/api/explain-edge", get(explain_edge_api))
        .route("/api/trace", get(trace_api))
        .route("/api/workflow", get(workflow_api))
        .route("/api/workflow-query", get(workflow_query_api))
        .route("/api/journey", get(journey_api))
        .route("/api/mcp", post(mcp_api))
        .route(
            "/api/component-dependencies",
            get(component_dependencies_api),
        )
        .route("/api/component-contract", get(component_contract_api))
        .route("/api/impact", get(impact_api))
        .route("/api/seams", get(seams_api))
        .route("/api/pr-impact", get(pr_impact_api))
        .route("/api/refactor-context", get(refactor_context_api))
        .route("/api/dependents", get(dependents_api))
        .route("/api/trace-config", get(trace_config_api))
        .route("/api/trace-errors", get(trace_errors_api))
        .route("/api/source", get(source))
        .route("/api/source-search", get(source_search_api))
        .fallback(not_found)
        .with_state(state)
        .layer(DefaultBodyLimit::max(max_api_body_bytes))
        .layer(axum_middleware::from_fn(cache_headers))
        .layer(axum_middleware::from_fn(security_headers));
    let app = if api_auth.enabled() {
        app.layer(axum_middleware::from_fn_with_state(
            api_auth,
            api_auth_middleware,
        ))
    } else {
        app
    };
    let app = if args.quiet_access_log {
        app
    } else {
        app.layer(axum_middleware::from_fn(access_log))
    };
    let app = app.layer(axum_middleware::from_fn(response_timing_header));
    let app = app.layer(axum_middleware::from_fn(request_id_header));

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
