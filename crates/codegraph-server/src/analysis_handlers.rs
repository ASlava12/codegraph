//! Graph and analysis endpoints: scan, cache/incremental, export, graph
//! slices, cards, reports, queries, ask, traces, workflows, journeys,
//! refactoring reports, MCP transport, and source access.

use anyhow::Result;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use codegraph_analysis::pr_impact;
use codegraph_analysis::{
    CheckReport, ComponentContractReport, ComponentContractRequest, ComponentDependencyReport,
    ComponentDependencyRequest, ConfigTraceRequest, ConfigTraceResult, DEFAULT_MERMAID_EDGE_LIMIT,
    DEFAULT_MERMAID_NODE_LIMIT, DEFAULT_REPORT_COMMUNITY_LIMIT, DEFAULT_SVG_EDGE_LIMIT,
    DEFAULT_SVG_NODE_LIMIT, EntrypointTraceReport, EntrypointTraceRequest,
    EntrypointWorkflowReport, EntrypointWorkflowRequest, ErrorTraceRequest, ErrorTraceResult,
    ExplainEdgeRequest, FocusRequest, GraphSlice, GraphSliceRequest, ImpactReport, ImpactRequest,
    InsightFilter, InsightReport, InsightSeverity, JourneyReport, JourneyRequest, McpEngine,
    NaturalQueryReport, NaturalQueryRequest, NodeCard, NodeCardRequest, NodeContext,
    ProjectReportMarkdownOptions, RefactorContextBundle, RefactorContextRequest, SeamReport,
    SeamRequest, SourcePreview, SourceSearchRequest, SourceSearchResult, TraceRequest, TraceStart,
    WorkflowQueryReport, WorkflowQueryRequest, WorkflowReport, WorkflowRequest, architecture_map,
    check_insights, communities, compact_query_result, component_contract, component_dependencies,
    entrypoints, explain_edge, export_cypher, export_dot, export_falkordb,
    export_graph_mermaid_html, export_graphml, export_ndjson, export_svg, filter_insight_report,
    focus_subgraph, hotspots, impact, insights, journey, language_dependencies, natural_query,
    node_card, node_context, project_report, project_report_markdown, query_graph,
    read_source_preview, refactor_context, seams, search_source, slice_graph, summarize,
    surprising_links, trace, trace_config, trace_dependents, trace_entrypoints, trace_errors,
    workflow, workflow_entrypoints, workflow_query,
};
use codegraph_indexer::scan_coverage;
use codegraph_storage::{GraphCache, scan_project_cached};

#[allow(unused_imports)]
use crate::*;

pub(crate) async fn projects_api(State(state): State<AppState>) -> Json<Vec<ProjectResponse>> {
    Json(project_responses(&state))
}

pub(crate) async fn scan_options_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ScanOptionsQuery>,
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

pub(crate) async fn scan(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ScanQuery>,
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

pub(crate) async fn cache_diff_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CacheDiffQuery>,
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

pub(crate) async fn cache_chunks_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CacheDiffQuery>,
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

pub(crate) async fn incremental_plan_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CacheDiffQuery>,
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

pub(crate) async fn incremental_scan_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CacheDiffQuery>,
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

pub(crate) async fn incremental_merge_preview_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CacheDiffQuery>,
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

pub(crate) async fn incremental_update_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CacheDiffQuery>,
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

pub(crate) async fn export_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ExportQuery>,
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
        ExportFormat::Graphml => (
            "application/graphml+xml; charset=utf-8",
            export_graphml(&graph),
        ),
        ExportFormat::MermaidHtml => (
            "text/html; charset=utf-8",
            export_graph_mermaid_html(
                &graph,
                DEFAULT_MERMAID_NODE_LIMIT,
                DEFAULT_MERMAID_EDGE_LIMIT,
            ),
        ),
        ExportFormat::Svg => (
            "image/svg+xml; charset=utf-8",
            export_svg(&graph, DEFAULT_SVG_NODE_LIMIT, DEFAULT_SVG_EDGE_LIMIT),
        ),
        ExportFormat::Cypher => ("text/plain; charset=utf-8", export_cypher(&graph)),
        ExportFormat::Falkordb => (
            "text/plain; charset=utf-8",
            export_falkordb(&graph, "codegraph"),
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

pub(crate) async fn graph_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<GraphSliceQuery>,
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

pub(crate) async fn node_context_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<NodeContextQuery>,
) -> Result<Json<NodeContext>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let context = node_context(
        &graph,
        parse_node_id_param(&query.node_id)?,
        query.edge_limit.unwrap_or(DEFAULT_NODE_CONTEXT_EDGE_LIMIT),
    )
    .ok_or_else(|| ApiError::not_found("node not found"))?;
    Ok(Json(context))
}

pub(crate) async fn node_card_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<NodeCardQuery>,
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
    let node_id = parse_node_id_param(&query.node_id)?;
    let card = tokio::task::spawn_blocking(move || {
        let output = scan_project_cached(root.clone(), &options, cache.as_ref())
            .map_err(|error| error.to_string())?;
        node_card(
            &output.graph,
            Some(&root),
            NodeCardRequest {
                node_id,
                edge_limit,
                source_context,
                insight_limit,
            },
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| ApiError::internal(format!("node card task failed: {error}")))?
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found("node not found"))?;
    Ok(Json(card))
}

pub(crate) async fn focus_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<FocusQuery>,
) -> Result<Json<codegraph_analysis::QueryResult>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let node_ids = parse_node_ids(query.node_ids.as_deref())?;
    let edge_indexes = parse_edge_indexes(query.edge_indexes.as_deref())?;
    let result = focus_subgraph(
        &graph,
        FocusRequest {
            node_ids,
            edge_indexes,
            edge_limit: query.edge_limit.unwrap_or(DEFAULT_FOCUS_EDGE_LIMIT),
        },
    );
    Ok(Json(if query.compact.unwrap_or(false) {
        compact_query_result(result)
    } else {
        result
    }))
}

pub(crate) async fn report_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ProjectReportQuery>,
) -> Result<Response, ApiError> {
    let limits = project_report_limits_from_query(&query)?;
    let format = query.format.unwrap_or(ProjectReportFormat::Json);
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

    let (content_type, body) = match format {
        ProjectReportFormat::Json => (
            "application/json; charset=utf-8",
            serde_json::to_string_pretty(&response)
                .map_err(|error| ApiError::internal(error.to_string()))?,
        ),
        ProjectReportFormat::Markdown => (
            "text/markdown; charset=utf-8",
            project_report_markdown(
                &response.report,
                &ProjectReportMarkdownOptions {
                    title: "CodeGraph Project Report".to_string(),
                    root: Some(response.root.clone()),
                    generated_at_unix: Some(response.generated_at_unix),
                },
            ),
        ),
    };
    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))],
        body,
    )
        .into_response())
}

pub(crate) async fn summary(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ScanQuery>,
) -> Result<Json<codegraph_analysis::GraphSummary>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(summarize(&graph)))
}

pub(crate) async fn coverage_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ScanQuery>,
) -> Result<Json<codegraph_indexer::ScanCoverageReport>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let options = scan_options(&state, &root)?;
    let report = tokio::task::spawn_blocking(move || scan_coverage(&root, &options))
        .await
        .map_err(|error| ApiError::internal(format!("coverage task failed: {error}")))?
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(report))
}

pub(crate) async fn architecture_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ArchitectureQuery>,
) -> Result<Json<codegraph_analysis::ArchitectureMap>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(architecture_map(
        &graph,
        query.group_limit.unwrap_or(50),
        query.edge_limit.unwrap_or(200),
    )))
}

pub(crate) async fn language_dependencies_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<LanguageDependencyQuery>,
) -> Result<Json<codegraph_analysis::LanguageDependencyReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(language_dependencies(
        &graph,
        query.limit.unwrap_or(50),
    )))
}

pub(crate) async fn surprising_links_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SurprisingLinkQuery>,
) -> Result<Json<codegraph_analysis::SurprisingLinkReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(surprising_links(&graph, query.limit.unwrap_or(50))))
}

pub(crate) async fn hotspots_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<HotspotQuery>,
) -> Result<Json<codegraph_analysis::HotspotReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(hotspots(&graph, query.limit.unwrap_or(25))))
}

pub(crate) async fn communities_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CommunityQuery>,
) -> Result<Json<codegraph_analysis::CommunityReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(communities(
        &graph,
        query.limit.unwrap_or(DEFAULT_REPORT_COMMUNITY_LIMIT),
    )))
}

pub(crate) async fn entrypoints_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ScanQuery>,
) -> Result<Json<Vec<codegraph_core::Node>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(entrypoints(&graph)))
}

pub(crate) async fn entrypoint_traces_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<EntrypointTraceQuery>,
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

pub(crate) async fn entrypoint_workflows_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<EntrypointWorkflowQuery>,
) -> Result<Json<EntrypointWorkflowReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(workflow_entrypoints(
        &graph,
        EntrypointWorkflowRequest {
            search: normalize_query_string(query.search),
            entrypoint_kind: normalize_query_string(query.entrypoint_kind),
            max_depth: query.depth.unwrap_or(4).clamp(1, 32),
            block_limit: query.block_limit.unwrap_or(200).clamp(1, 1_000),
            limit: query.limit.unwrap_or(25).clamp(1, 500),
            filters: workflow_filters_from_query(
                query.edge_kind,
                query.confidence,
                query.language,
                query.risk_severity,
                query.block_kind,
            ),
            compact: query.compact.unwrap_or(false),
        },
    )))
}

pub(crate) async fn insights_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<InsightQuery>,
) -> Result<Json<InsightReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = insights(&graph);
    Ok(Json(filter_insight_report(
        report,
        &insight_filter_from_query(query)?,
    )))
}

pub(crate) async fn check_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<CheckQuery>,
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

pub(crate) async fn query_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<GraphQuery>,
) -> Result<Json<codegraph_analysis::QueryResult>, ApiError> {
    if query.q.len() > MAX_GRAPH_QUERY_LENGTH {
        return Err(ApiError::bad_request(format!(
            "query expression is too long; maximum is {MAX_GRAPH_QUERY_LENGTH} bytes"
        )));
    }
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let result =
        query_graph(&graph, &query.q).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let result = if query.compact.unwrap_or(false) {
        compact_query_result(result)
    } else {
        result
    };
    Ok(Json(result))
}

pub(crate) async fn ask_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<NaturalGraphQuery>,
) -> Result<Json<NaturalQueryReport>, ApiError> {
    if query.q.len() > MAX_GRAPH_QUERY_LENGTH {
        return Err(ApiError::bad_request(format!(
            "natural-language question is too long; maximum is {MAX_GRAPH_QUERY_LENGTH} bytes"
        )));
    }
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = natural_query(
        &graph,
        NaturalQueryRequest {
            question: query.q,
            compact: query.compact.unwrap_or(false),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(report))
}

pub(crate) async fn explain_edge_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ExplainEdgeQuery>,
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

pub(crate) async fn trace_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<TraceQuery>,
) -> Result<Json<Option<codegraph_analysis::TraceResult>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let start = match (query.node_id, query.label) {
        (Some(id), _) => TraceStart::NodeId(parse_node_id_param(&id)?),
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

pub(crate) async fn workflow_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<WorkflowQuery>,
) -> Result<Json<Option<WorkflowReport>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let start = match (query.node_id, query.label) {
        (Some(id), _) => TraceStart::NodeId(parse_node_id_param(&id)?),
        (None, Some(label)) => TraceStart::Label(label),
        (None, None) => {
            return Err(ApiError::bad_request(
                "workflow requires either node_id or label query parameter",
            ));
        }
    };
    Ok(Json(workflow(
        &graph,
        WorkflowRequest {
            start,
            max_depth: query.depth.unwrap_or(4).clamp(1, 32),
            block_limit: query.block_limit.unwrap_or(200).clamp(1, 1_000),
            filters: workflow_filters_from_query(
                query.edge_kind,
                query.confidence,
                query.language,
                query.risk_severity,
                query.block_kind,
            ),
            compact: query.compact.unwrap_or(false),
            max_fanout: query.max_fanout.map(|value| value.clamp(1, 200)),
        },
    )))
}

pub(crate) async fn refactor_context_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<RefactorContextQuery>,
) -> Result<Json<RefactorContextBundle>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let mut bundle = refactor_context(
        &graph,
        RefactorContextRequest {
            target: query.target,
            from: normalize_query_string(query.from),
            max_depth: query.depth.unwrap_or(8).clamp(1, 32),
            path_limit: query.paths.unwrap_or(3).clamp(1, 10),
            dependent_limit: query.dependent_limit.unwrap_or(100).clamp(1, 1_000),
            risk_limit: query.risk_limit.unwrap_or(50).clamp(1, 200),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if let Some(span) = bundle.target.span.clone() {
        let source_root = resolve_scan_root(&state, query.path.as_deref())?;
        bundle.target_source = read_source_preview(
            &source_root,
            std::path::Path::new(&span.path),
            span.start_line,
            span.end_line,
            query.source_context.unwrap_or(6).min(50),
        )
        .ok();
    }
    Ok(Json(bundle))
}

pub(crate) async fn seams_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SeamQuery>,
) -> Result<Json<SeamReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    Ok(Json(seams(
        &graph,
        SeamRequest {
            limit: query.limit.unwrap_or(25).clamp(1, 100),
            edge_limit: query.edge_limit.unwrap_or(10).clamp(1, 50),
        },
    )))
}

pub(crate) async fn pr_impact_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<PrImpactQuery>,
) -> Result<Json<pr_impact::PrImpactReport>, ApiError> {
    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let explicit: Vec<String> = query
        .files
        .as_deref()
        .map(|files| {
            files
                .split(',')
                .map(str::trim)
                .filter(|file| !file.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let (changed, base_used, branch) = {
        let git_root = root.clone();
        let base = query.base.clone();
        tokio::task::spawn_blocking(move || {
            let branch = pr_impact::git_current_branch(&git_root);
            if explicit.is_empty() {
                let base = base.unwrap_or_else(|| "HEAD".to_string());
                pr_impact::git_changed_files(&git_root, &base)
                    .map(|changed| (changed, Some(base), branch))
            } else {
                Ok((explicit, None, branch))
            }
        })
        .await
        .map_err(|error| ApiError::internal(format!("git task failed: {error}")))?
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    };
    Ok(Json(pr_impact::pr_impact(
        &graph,
        &changed,
        pr_impact::PrImpactContext {
            base: base_used,
            branch,
            ci_state: query.ci_state,
            review_state: query.review_state,
        },
    )))
}

pub(crate) async fn impact_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ImpactQuery>,
) -> Result<Json<ImpactReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = impact(
        &graph,
        ImpactRequest {
            target: query.target,
            max_depth: query.depth.unwrap_or(6).clamp(1, 32),
            limit: query.limit.unwrap_or(100).clamp(1, 1_000),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(report))
}

pub(crate) async fn component_dependencies_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ComponentDependencyQuery>,
) -> Result<Json<ComponentDependencyReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = component_dependencies(
        &graph,
        ComponentDependencyRequest {
            target: query.target,
            group_limit: query.group_limit.unwrap_or(25).clamp(1, 100),
            edge_limit: query.edge_limit.unwrap_or(10).clamp(1, 50),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(report))
}

pub(crate) async fn component_contract_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ComponentContractQuery>,
) -> Result<Json<ComponentContractReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = component_contract(
        &graph,
        ComponentContractRequest {
            source: query.source,
            target: query.target,
            edge_limit: query.edge_limit.unwrap_or(100).clamp(1, 500),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(report))
}

pub(crate) async fn journey_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<JourneyQuery>,
) -> Result<Json<JourneyReport>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = journey(
        &graph,
        JourneyRequest {
            from: query.from,
            to: query.to,
            max_depth: query.depth.unwrap_or(8).clamp(1, 32),
            path_limit: query.paths.unwrap_or(3).clamp(1, 10),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(report))
}

/// HTTP MCP transport: one JSON-RPC 2.0 message per POST body, protected by
/// the same optional bearer token as every other `/api/` route. Notifications
/// return 202 with no body; batch arrays are rejected per request.
pub(crate) async fn mcp_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<McpQuery>,
    body: String,
) -> Result<Response, ApiError> {
    let message: serde_json::Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(error) => {
            return Ok(Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": {"code": -32700, "message": format!("parse error: {error}")},
            }))
            .into_response());
        }
    };
    if message.is_array() {
        return Ok(Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": serde_json::Value::Null,
            "error": {"code": -32600, "message": "batch requests are not supported; send one JSON-RPC message per request"},
        }))
        .into_response());
    }

    let root = resolve_scan_root(&state, query.path.as_deref())?;
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let fingerprint = {
        let options = scan_options(&state, &root)?;
        let fingerprint_root = root.clone();
        tokio::task::spawn_blocking(move || {
            GraphCache::fingerprint_project(&fingerprint_root, &options)
                .ok()
                .map(|fingerprint| fingerprint.hash)
        })
        .await
        .ok()
        .flatten()
    };
    let engine = McpEngine {
        graph: &graph,
        root: Some(&root),
        fingerprint: fingerprint.as_deref(),
    };
    let (response, _audit) = engine.handle_message(&message);
    Ok(match response {
        Some(value) => Json(value).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    })
}

pub(crate) async fn workflow_query_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<WorkflowQuerySliceQuery>,
) -> Result<Json<WorkflowQueryReport>, ApiError> {
    if query.q.len() > MAX_GRAPH_QUERY_LENGTH {
        return Err(ApiError::bad_request(format!(
            "query expression is too long; maximum is {MAX_GRAPH_QUERY_LENGTH} bytes"
        )));
    }
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let report = workflow_query(
        &graph,
        WorkflowQueryRequest {
            query: query.q,
            max_depth: query.depth.unwrap_or(4).clamp(1, 32),
            block_limit: query.block_limit.unwrap_or(200).clamp(1, 1_000),
            limit: query.limit.unwrap_or(25).clamp(1, 500),
            filters: workflow_filters_from_query(
                query.edge_kind,
                query.confidence,
                query.language,
                query.risk_severity,
                query.block_kind,
            ),
            compact: query.compact.unwrap_or(false),
        },
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(report))
}

pub(crate) async fn dependents_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<TraceQuery>,
) -> Result<Json<Option<codegraph_analysis::TraceResult>>, ApiError> {
    let graph = scan_graph(&state, query.path.as_deref()).await?;
    let start = match (query.node_id, query.label) {
        (Some(id), _) => TraceStart::NodeId(parse_node_id_param(&id)?),
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

pub(crate) async fn trace_config_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ConfigTraceQuery>,
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

pub(crate) async fn trace_errors_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<ErrorTraceQuery>,
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

pub(crate) async fn source(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SourceQuery>,
) -> Result<Json<SourcePreview>, ApiError> {
    // Canonical form: `path` = project root, `file` = source file. The
    // pre-unification form (`root` = project root, `path` = file) stays
    // accepted for older clients (audit F9).
    let (root_param, file_param) = match (&query.file, &query.root, &query.path) {
        (Some(file), _, _) => (
            query.path.clone().or_else(|| query.root.clone()),
            file.clone(),
        ),
        (None, Some(_), Some(file)) => (query.root.clone(), file.clone()),
        _ => {
            return Err(ApiError::bad_request(
                "source requires a `file` parameter (source file inside the `path` project root)",
            ));
        }
    };
    let source_root = resolve_scan_root(&state, root_param.as_deref())?;
    let path = resolve_path(&state, &source_root, &file_param)?;
    if !path.is_file() {
        return Err(ApiError::bad_request("file is not a file"));
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

pub(crate) async fn source_search_api(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SourceSearchQuery>,
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
