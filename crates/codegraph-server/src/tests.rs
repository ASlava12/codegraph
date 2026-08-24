//! Unit tests for the HTTP server: handlers, schema contracts, jobs,
//! middleware, and embedded web assets.

use super::*;
#[allow(unused_imports)]
use anyhow::{Context, Result};
#[allow(unused_imports)]
use async_stream::stream;
#[allow(unused_imports)]
use axum::extract::{DefaultBodyLimit, Path as AxumPath, Request, State};
#[allow(unused_imports)]
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
#[allow(unused_imports)]
use axum::middleware::{self, Next};
#[allow(unused_imports)]
use axum::response::sse::{Event, KeepAlive, Sse};
#[allow(unused_imports)]
use axum::response::{Html, IntoResponse, Response};
#[allow(unused_imports)]
use axum::routing::{get, post};
#[allow(unused_imports)]
use axum::{Json, Router};
#[allow(unused_imports)]
use clap::Parser;
#[allow(unused_imports)]
use codegraph_analysis::pr_impact;
#[allow(unused_imports)]
use codegraph_analysis::{
    CheckReport, ComponentContractReport, ComponentContractRequest, ComponentDependencyReport,
    ComponentDependencyRequest, ConfigTraceRequest, ConfigTraceResult, DEFAULT_MERMAID_EDGE_LIMIT,
    DEFAULT_MERMAID_NODE_LIMIT, DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT,
    DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT, DEFAULT_REPORT_COMMUNITY_LIMIT,
    DEFAULT_REPORT_FILE_SUMMARY_LIMIT, DEFAULT_REPORT_HOTSPOT_LIMIT, DEFAULT_REPORT_INSIGHT_LIMIT,
    DEFAULT_REPORT_LANGUAGE_LINK_LIMIT, DEFAULT_REPORT_NODE_SUMMARY_LIMIT, EntrypointTraceReport,
    EntrypointTraceRequest, EntrypointWorkflowReport, EntrypointWorkflowRequest, ErrorTraceRequest,
    ErrorTraceResult, ExplainEdgeRequest, FocusRequest, GraphSlice, GraphSliceRequest,
    GraphSummary, ImpactReport, ImpactRequest, InsightFilter, InsightReport, InsightSeverity,
    JourneyReport, JourneyRequest, KNOWN_INSIGHT_KINDS, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT,
    MAX_REPORT_ARCHITECTURE_GROUP_LIMIT, MAX_REPORT_COMMUNITY_LIMIT, MAX_REPORT_FILE_SUMMARY_LIMIT,
    MAX_REPORT_HOTSPOT_LIMIT, MAX_REPORT_INSIGHT_LIMIT, MAX_REPORT_LANGUAGE_LINK_LIMIT,
    MAX_REPORT_NODE_SUMMARY_LIMIT, McpEngine, NaturalQueryReport, NaturalQueryRequest, NodeCard,
    NodeContext, ProjectReport, ProjectReportLimits, ProjectReportMarkdownOptions,
    REPORT_QUALITY_GATE_SAMPLE_LIMIT, RefactorContextBundle, RefactorContextRequest, SeamReport,
    SeamRequest, SourcePreview, SourceSearchRequest, SourceSearchResult, TraceRequest, TraceStart,
    WorkflowFilters, WorkflowQueryReport, WorkflowQueryRequest, WorkflowReport, WorkflowRequest,
    architecture_map, check_insights, communities, compact_query_result, component_contract,
    component_dependencies, entrypoints, explain_edge, export_cypher, export_dot, export_falkordb,
    export_graph_mermaid_html, export_graphml, export_ndjson, filter_insight_report,
    focus_subgraph, hotspots, impact, insights, journey, language_dependencies, natural_query,
    node_card, node_context, project_report, project_report_markdown, query_graph,
    read_source_preview, refactor_context, seams, search_source, slice_graph, summarize,
    surprising_links, trace, trace_config, trace_dependents, trace_entrypoints, trace_errors,
    workflow, workflow_entrypoints, workflow_query,
};
#[allow(unused_imports)]
use codegraph_core::{CODEGRAPH_SCHEMA_VERSION, CodeGraph};
#[allow(unused_imports)]
use codegraph_indexer::{
    IndexOptionOverrides, IndexOptions, configured_index_options, scan_coverage,
};
#[allow(unused_imports)]
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
#[allow(unused_imports)]
use codegraph_parser::language_adapters;
#[allow(unused_imports)]
use codegraph_storage::{
    CacheInfo, CacheStatus, GraphCache, default_cache_dir, scan_project_cached,
};
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::convert::Infallible;
#[allow(unused_imports)]
use std::env;
use std::fs;
#[allow(unused_imports)]
use std::net::SocketAddr;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
use std::sync::Arc;
#[allow(unused_imports)]
use std::sync::atomic::{AtomicU64, Ordering};
#[allow(unused_imports)]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[allow(unused_imports)]
use tokio::net::TcpListener;
#[allow(unused_imports)]
use tokio::sync::{RwLock, Semaphore};
#[allow(unused_imports)]
use tokio::time::sleep;

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
        "dart",
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

    let response = static_asset_response(&headers, "application/javascript; charset=utf-8", body);

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
    assert!(without_cache.contains(&"natural_language_queries"));
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
    assert_eq!(response.limits.default_node_context_edge_limit, 24);
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
    assert_eq!(response.limits.default_report_community_limit, 25);
    assert_eq!(response.limits.max_report_community_limit, 500);
    assert_eq!(response.limits.default_report_insight_limit, 50);
    assert_eq!(response.limits.max_report_insight_limit, 500);
    assert_eq!(response.limits.report_quality_gate_sample_limit, 25);
    assert_eq!(response.limits.default_report_file_summary_limit, 25);
    assert_eq!(response.limits.max_report_file_summary_limit, 500);
    assert_eq!(response.limits.default_report_node_summary_limit, 25);
    assert_eq!(response.limits.max_report_node_summary_limit, 500);
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
        format: None,
        architecture_group_limit: Some(usize::MAX),
        architecture_edge_limit: Some(usize::MAX),
        language_link_limit: Some(usize::MAX),
        hotspot_limit: Some(usize::MAX),
        community_limit: Some(usize::MAX),
        insight_limit: Some(usize::MAX),
        file_summary_limit: Some(usize::MAX),
        node_summary_limit: Some(usize::MAX),
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
    assert_eq!(limits.community_limit, MAX_REPORT_COMMUNITY_LIMIT);
    assert_eq!(limits.insight_limit, MAX_REPORT_INSIGHT_LIMIT);
    assert_eq!(limits.file_summary_limit, MAX_REPORT_FILE_SUMMARY_LIMIT);
    assert_eq!(limits.node_summary_limit, MAX_REPORT_NODE_SUMMARY_LIMIT);
}

#[tokio::test]
async fn query_api_rejects_oversized_query_before_scan() {
    let root = temp_server_root();
    fs::create_dir_all(&root).unwrap();
    let state = test_state(root.clone(), vec![], false);
    let query = GraphQuery {
        path: None,
        q: "x".repeat(MAX_GRAPH_QUERY_LENGTH + 1),
        compact: None,
    };

    let error = query_api(State(state), ApiQuery(query))
        .await
        .expect_err("oversized query should fail");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("query expression is too long"));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn entrypoints_api_honours_a_limit_and_leads_with_programs() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join(".github/workflows")).unwrap();
    fs::write(
        root.join(".github/workflows/ci.yml"),
        "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo test\n",
    )
    .unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let state = test_state(root.clone(), vec![], true);

    let all = entrypoints_api(
        State(state.clone()),
        ApiQuery(ScanQuery {
            path: None,
            limit: None,
        }),
    )
    .await
    .expect("entrypoints");
    assert!(all.0.len() > 1, "the fixture declares several entrypoints");

    // Omitting the limit keeps the whole list, as it always has; asking for
    // one returns the one worth opening first, since the list is ranked.
    let capped = entrypoints_api(
        State(state),
        ApiQuery(ScanQuery {
            path: None,
            limit: Some(1),
        }),
    )
    .await
    .expect("entrypoints");
    assert_eq!(capped.0.len(), 1);
    assert_eq!(capped.0[0].id, all.0[0].id);

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn every_answer_is_built_from_the_same_graph() {
    // `/api/node-card`, `/api/scan` and `/api/report` scanned on their own
    // and skipped the automatic semantic pass their neighbours run, so a
    // card described a syntax-only graph while a query beside it described
    // the enriched one. The root node's own record of that pass is the
    // visible half of the difference.
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let state = test_state(root.clone(), vec![], true);

    let graph = scan_graph(&state, Some(root.as_path()))
        .await
        .expect("graph");
    let repository = graph
        .nodes
        .iter()
        .find(|node| node.kind == codegraph_core::NodeKind::Repository)
        .expect("repository node");
    let expected: Vec<&String> = repository
        .metadata
        .keys()
        .filter(|key| key.starts_with("semantic_"))
        .collect();
    assert!(
        !expected.is_empty(),
        "the scan records what the semantic pass did"
    );

    let Json(card) = node_card_api(
        State(state),
        ApiQuery(NodeCardQuery {
            path: Some(root.clone()),
            node_id: format!("n{}", repository.id.0),
            edge_limit: None,
            source_context: None,
            insight_limit: None,
            include_insights: None,
        }),
    )
    .await
    .expect("node card");
    for key in expected {
        assert!(
            card.context.node.metadata.contains_key(key),
            "the card lost `{key}`, so it was built from another graph"
        );
    }

    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn trace_apis_say_which_name_matched_nothing() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let state = test_state(root.clone(), vec![], true);
    let query = |label: Option<String>, node_id: Option<String>| TraceQuery {
        path: Some(root.clone()),
        label,
        node_id,
        depth: Some(2),
    };

    let error = trace_api(
        State(state.clone()),
        ApiQuery(query(Some("nosuchthing".to_string()), None)),
    )
    .await
    .expect_err("expected an error for an unknown label");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("trace start `nosuchthing`"),
        "message: {}",
        error.message
    );

    let error = dependents_api(
        State(state.clone()),
        ApiQuery(query(None, Some("n999999".to_string()))),
    )
    .await
    .expect_err("expected an error for an unknown node id");
    assert_eq!(error.status, StatusCode::NOT_FOUND);

    // A start that resolves still answers.
    let Json(result) = trace_api(
        State(state),
        ApiQuery(query(Some("main".to_string()), None)),
    )
    .await
    .expect("trace response");
    assert_eq!(result.expect("trace result").start.label, "main");
}

#[tokio::test]
async fn workflow_api_says_which_name_matched_nothing() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let state = test_state(root.clone(), vec![], true);
    let query = |label: Option<String>, node_id: Option<String>| WorkflowQuery {
        path: Some(root.clone()),
        label,
        node_id,
        depth: Some(3),
        block_limit: Some(20),
        edge_kind: None,
        confidence: None,
        language: None,
        risk_severity: None,
        block_kind: None,
        compact: None,
        max_fanout: None,
    };

    // A name that matched nothing is not an empty workflow.
    let error = workflow_api(
        State(state.clone()),
        ApiQuery(query(Some("nosuchthing".to_string()), None)),
    )
    .await
    .expect_err("expected an error for an unknown label");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("workflow target `nosuchthing`"),
        "message: {}",
        error.message
    );

    let error = workflow_api(
        State(state),
        ApiQuery(query(None, Some("n999999".to_string()))),
    )
    .await
    .expect_err("expected an error for an unknown node id");
    assert_eq!(error.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workflow_api_returns_block_report_for_label() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
    helper();
}

fn helper() {
    panic!("broken");
}
"#,
    )
    .unwrap();
    let state = test_state(root.clone(), vec![], true);

    let Json(report) = workflow_api(
        State(state),
        ApiQuery(WorkflowQuery {
            path: Some(root.clone()),
            label: Some("main".to_string()),
            node_id: None,
            depth: Some(3),
            block_limit: Some(20),
            edge_kind: None,
            confidence: None,
            language: None,
            risk_severity: None,
            block_kind: None,
            compact: None,
            max_fanout: None,
        }),
    )
    .await
    .expect("workflow response");
    let report = report.expect("workflow report");

    assert_eq!(report.start.label, "main");
    assert!(report.blocks.iter().any(|block| {
        block.kind == codegraph_analysis::WorkflowBlockKind::Start && block.node.label == "main"
    }));
    assert!(report.blocks.iter().any(|block| {
        block.kind == codegraph_analysis::WorkflowBlockKind::Call && block.node.label == "helper"
    }));
    assert!(
        report
            .transitions
            .iter()
            .any(|transition| transition.edge_index.to_string()
                == transition.edge.metadata["edge_index"])
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn journey_api_returns_step_numbered_chain() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
    if ready() {
        helper();
    }
}

fn helper() {
    load_config();
}

fn load_config() {}

fn ready() -> bool {
    true
}
"#,
    )
    .unwrap();
    let state = test_state(root.clone(), vec![], true);

    let Json(report) = journey_api(
        State(state.clone()),
        ApiQuery(JourneyQuery {
            path: Some(root.clone()),
            from: "main".to_string(),
            to: "load_config".to_string(),
            depth: Some(8),
            paths: None,
        }),
    )
    .await
    .expect("journey response");

    assert_eq!(report.from.label, "main");
    assert_eq!(report.to.label, "load_config");
    assert_eq!(report.total_paths, 1);
    let path = &report.paths[0];
    assert!(path.total_steps >= 3);
    assert_eq!(
        path.steps.iter().map(|step| step.step).collect::<Vec<_>>(),
        (1..=path.total_steps).collect::<Vec<_>>()
    );
    assert_eq!(
        path.steps[0].block.kind,
        codegraph_analysis::WorkflowBlockKind::Start
    );
    assert_eq!(path.steps.last().unwrap().block.node.label, "load_config");
    assert!(path.steps[1..].iter().all(|step| step.transition.is_some()));

    let error = journey_api(
        State(state),
        ApiQuery(JourneyQuery {
            path: Some(root.clone()),
            from: "ghost".to_string(),
            to: "load_config".to_string(),
            depth: Some(8),
            paths: None,
        }),
    )
    .await
    .expect_err("unknown journey start should fail");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("journey start"));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn api_query_rejections_use_the_json_error_contract() {
    let request = axum::http::Request::builder()
        .uri("/api/node-card?node_id=42&edge_limit=not-a-number")
        .body(())
        .unwrap();
    let (mut parts, _) = request.into_parts();
    let result =
        <ApiQuery<NodeCardQuery> as axum::extract::FromRequestParts<()>>::from_request_parts(
            &mut parts,
            &(),
        )
        .await;
    let error = match result {
        Ok(_) => panic!("bad query must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("invalid query parameters"),
        "structured message expected: {}",
        error.message
    );
    // ApiError renders through the JSON error contract with a request id.
    let response = error.into_response();
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
}

#[tokio::test]
async fn source_api_requires_path_and_file() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let state = test_state(root.clone(), vec![], true);

    let preview = source(
        State(state.clone()),
        ApiQuery(SourceQuery {
            path: Some(root.clone()),
            file: Some(PathBuf::from("src/main.rs")),
            start_line: Some(1),
            end_line: Some(1),
            context: Some(0),
        }),
    )
    .await
    .expect("path+file form works");
    assert!(
        preview
            .0
            .lines
            .iter()
            .any(|line| line.text.contains("fn main"))
    );

    let error = source(
        State(state.clone()),
        ApiQuery(SourceQuery {
            path: Some(root.clone()),
            file: None,
            start_line: None,
            end_line: None,
            context: None,
        }),
    )
    .await
    .expect_err("missing file parameter rejected");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("`file`"));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn node_card_api_accepts_n_prefixed_ids() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    helper();\n}\npub fn helper() {}\n",
    )
    .unwrap();
    let state = test_state(root.clone(), vec![], true);

    let graph = scan_graph(&state, Some(&root)).await.expect("graph");
    let helper_id = graph
        .nodes
        .iter()
        .find(|node| node.label == "helper")
        .expect("helper node")
        .id;

    let card = node_card_api(
        State(state.clone()),
        ApiQuery(NodeCardQuery {
            path: Some(root.clone()),
            node_id: format!("n{}", helper_id.0),
            edge_limit: None,
            source_context: None,
            insight_limit: None,
            include_insights: None,
        }),
    )
    .await
    .expect("n-prefixed node id resolves");
    assert_eq!(card.0.context.node.id, helper_id);

    let context = node_context_api(
        State(state.clone()),
        ApiQuery(NodeContextQuery {
            path: Some(root.clone()),
            node_id: helper_id.0.to_string(),
            edge_limit: None,
        }),
    )
    .await
    .expect("bare numeric id still resolves");
    assert_eq!(context.0.node.id, helper_id);

    let error = node_card_api(
        State(state.clone()),
        ApiQuery(NodeCardQuery {
            path: Some(root.clone()),
            node_id: "nope".to_string(),
            edge_limit: None,
            source_context: None,
            insight_limit: None,
            include_insights: None,
        }),
    )
    .await
    .expect_err("invalid id rejected");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("n42"));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn pr_impact_api_maps_explicit_changed_files() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    helper();\n}\n",
    )
    .unwrap();
    fs::write(
            root.join("src").join("util.rs"),
            "// FIXME: helper still calls a missing function\npub fn helper() {\n    missing_helper();\n}\n",
        )
        .unwrap();
    let state = test_state(root.clone(), vec![], true);

    let response = pr_impact_api(
        State(state.clone()),
        ApiQuery(PrImpactQuery {
            path: Some(root.clone()),
            base: None,
            files: Some("src/util.rs, docs/none.md".to_string()),
            ci_state: Some("passing".to_string()),
            review_state: None,
        }),
    )
    .await
    .expect("pr impact report");
    let report = response.0;
    assert_eq!(report.total_changed_files, 2);
    assert_eq!(report.matched_files, 1);
    assert_eq!(report.base, None, "explicit files skip git");
    assert_eq!(report.ci_state.as_deref(), Some("passing"));
    assert!(report.blast.dependents >= 1, "main depends on helper");
    assert!(
        report
            .risks
            .iter()
            .any(|risk| risk.kind == "rationale_risk_comment"),
        "FIXME in changed file must surface: {:?}",
        report.risks
    );
    fs::remove_dir_all(root).unwrap();
}

async fn mcp_response_json(response: Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn mcp_api_serves_graph_tools_over_http() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
    helper();
}

fn helper() {}
"#,
    )
    .unwrap();
    let state = test_state(root.clone(), vec![], true);

    let response = mcp_api(
        State(state.clone()),
        ApiQuery(McpQuery {
            path: Some(root.clone()),
        }),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.to_string(),
    )
    .await
    .expect("tools/list response");
    assert_eq!(response.status(), StatusCode::OK);
    let value = mcp_response_json(response).await;
    assert_eq!(value["jsonrpc"], "2.0");
    assert_eq!(
        value["result"]["tools"].as_array().expect("tools").len(),
        14
    );

    let response = mcp_api(
            State(state.clone()),
            ApiQuery(McpQuery {
                path: Some(root.clone()),
            }),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"query_graph","arguments":{"query":"nodes kind:function label:main"}}}"#
                .to_string(),
        )
        .await
        .expect("tools/call response");
    let value = mcp_response_json(response).await;
    assert_eq!(value["result"]["isError"], false);
    let payload: serde_json::Value = serde_json::from_str(
        value["result"]["content"][0]["text"]
            .as_str()
            .expect("text payload"),
    )
    .expect("payload json");
    assert_eq!(payload["total_nodes"], 1);

    let response = mcp_api(
        State(state.clone()),
        ApiQuery(McpQuery {
            path: Some(root.clone()),
        }),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string(),
    )
    .await
    .expect("notification response");
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "notifications return 202 with no body"
    );

    let response = mcp_api(
        State(state.clone()),
        ApiQuery(McpQuery {
            path: Some(root.clone()),
        }),
        "[]".to_string(),
    )
    .await
    .expect("batch response");
    let value = mcp_response_json(response).await;
    assert_eq!(value["error"]["code"], -32600);

    let response = mcp_api(
        State(state),
        ApiQuery(McpQuery {
            path: Some(root.clone()),
        }),
        "{not json".to_string(),
    )
    .await
    .expect("parse error response");
    let value = mcp_response_json(response).await;
    assert_eq!(value["error"]["code"], -32700);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn entrypoint_workflows_api_returns_filtered_reports() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "demo"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "api"
path = "src/main.rs"
"#,
    )
    .unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
    helper();
}

fn helper() {}
"#,
    )
    .unwrap();
    let state = test_state(root.clone(), vec![], true);

    let Json(report) = entrypoint_workflows_api(
        State(state),
        ApiQuery(EntrypointWorkflowQuery {
            path: Some(root.clone()),
            search: Some("api".to_string()),
            entrypoint_kind: None,
            depth: Some(2),
            block_limit: Some(20),
            limit: Some(10),
            edge_kind: None,
            confidence: None,
            language: None,
            risk_severity: None,
            block_kind: None,
            compact: None,
            max_fanout: None,
        }),
    )
    .await
    .expect("entrypoint workflow response");

    assert_eq!(report.max_depth, 2);
    assert_eq!(report.block_limit, 20);
    assert_eq!(report.total_entrypoints, 1);
    assert_eq!(report.workflows.len(), 1);
    assert!(report.workflows[0].start.label.contains("api"));
    assert!(
        report.workflows[0]
            .blocks
            .iter()
            .any(|block| block.node.label == "main")
    );

    let entrypoint_kind_query = |entrypoint_kind: &str| EntrypointWorkflowQuery {
        path: Some(root.clone()),
        search: None,
        entrypoint_kind: Some(entrypoint_kind.to_string()),
        depth: Some(2),
        block_limit: Some(20),
        limit: Some(10),
        edge_kind: None,
        confidence: None,
        language: None,
        risk_severity: None,
        block_kind: None,
        compact: None,
        max_fanout: None,
    };
    let state = test_state(root.clone(), vec![], true);
    let Json(binary_report) = entrypoint_workflows_api(
        State(state.clone()),
        ApiQuery(entrypoint_kind_query("binary")),
    )
    .await
    .expect("binary entrypoint workflow response");
    assert_eq!(binary_report.entrypoint_kind.as_deref(), Some("binary"));
    assert_eq!(binary_report.total_entrypoints, 2);
    assert!(binary_report.workflows.iter().all(|workflow| {
        workflow
            .start
            .metadata
            .get("entrypoint_kind")
            .map(String::as_str)
            == Some("binary")
    }));
    let Json(route_report) =
        entrypoint_workflows_api(State(state), ApiQuery(entrypoint_kind_query("route")))
            .await
            .expect("route entrypoint workflow response");
    assert_eq!(route_report.total_entrypoints, 0);
    assert!(route_report.workflows.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn workflow_query_api_returns_reports_from_query_nodes() {
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        r#"fn main() {
    helper();
}

fn helper() {}
"#,
    )
    .unwrap();
    let state = test_state(root.clone(), vec![], true);

    let Json(report) = workflow_query_api(
        State(state),
        ApiQuery(WorkflowQuerySliceQuery {
            path: Some(root.clone()),
            q: "nodes kind:function search:main".to_string(),
            depth: Some(2),
            block_limit: Some(20),
            limit: Some(10),
            edge_kind: Some("calls".to_string()),
            // `main` and `helper` share a file, so the call carries the
            // confidence the syntax earns rather than a name match's.
            confidence: Some("syntactic".to_string()),
            language: None,
            risk_severity: None,
            block_kind: Some("call".to_string()),
            compact: None,
            max_fanout: None,
        }),
    )
    .await
    .expect("workflow query response");

    assert_eq!(report.query, "nodes kind:function search:main");
    assert_eq!(report.max_depth, 2);
    assert_eq!(report.block_limit, 20);
    assert_eq!(report.filters.edge_kind.as_deref(), Some("calls"));
    assert_eq!(report.total_query_nodes, 1);
    assert_eq!(report.workflows.len(), 1);
    assert!(report.workflows[0].blocks.iter().any(|block| {
        block.node.label == "main" && block.kind == codegraph_analysis::WorkflowBlockKind::Start
    }));
    assert!(
        report.workflows[0]
            .blocks
            .iter()
            .any(|block| block.node.label == "helper")
    );
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

    let error = source_search_api(State(state), ApiQuery(query))
        .await
        .expect_err("oversized source-search query should fail");

    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(error.message.contains("source-search query is too long"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn capability_endpoints_cover_every_api_route() {
    // The catalog is how a client discovers this server, and it is written by
    // hand: a route added to the router without an entry here is invisible to
    // every consumer. Take the router itself as the truth.
    let router_source = include_str!("main.rs");
    let listed: std::collections::BTreeSet<String> = capability_endpoints()
        .into_iter()
        .flat_map(|group| group.endpoints)
        .map(str::to_string)
        .collect();

    fn mentions_method(chunk: &str, method: &str) -> bool {
        let needle = format!("{method}(");
        chunk.match_indices(&needle).any(|(index, _)| {
            index == 0
                || !chunk[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|character| character.is_alphanumeric() || character == '_')
        })
    }

    let mut missing = Vec::new();
    for chunk in router_source.split(".route(").skip(1) {
        let chunk = chunk
            .split_once(".fallback(")
            .map_or(chunk, |(head, _)| head)
            .split_once(".with_state(")
            .map_or(chunk, |(head, _)| head);
        let Some(path) = chunk.split('"').nth(1) else {
            continue;
        };
        if !path.starts_with("/api/") {
            continue;
        }
        for method in ["get", "post", "put", "delete"] {
            if mentions_method(chunk, method) {
                let entry = format!("{} {path}", method.to_uppercase());
                if !listed.contains(&entry) {
                    missing.push(entry);
                }
            }
        }
    }

    assert!(
        !missing.is_empty() || router_source.contains(".route("),
        "route extraction found nothing — the check would pass vacuously"
    );
    assert!(
        missing.is_empty(),
        "routes missing from the capability catalog: {missing:?}"
    );
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
    assert!(endpoints.contains(&"GET /api/ask"));
    assert!(endpoints.contains(&"GET /api/node-context"));
    assert!(endpoints.contains(&"GET /api/node-card"));
    assert!(endpoints.contains(&"GET /api/surprising-links"));
    assert!(endpoints.contains(&"GET /api/workflow"));
    assert!(endpoints.contains(&"GET /api/workflow-query"));
    assert!(endpoints.contains(&"GET /api/entrypoint-workflows"));
    assert!(endpoints.contains(&"POST /api/scan-jobs"));
    assert!(endpoints.contains(&"POST /api/semantic-jobs"));
}

#[test]
fn capability_catalog_covers_every_routed_api_path() {
    // The /api/capabilities catalog must not silently fall behind main.rs:
    // every routed /api path must appear in some capability group. Parse the
    // routes straight out of main.rs so a new route without a catalog entry
    // fails here.
    let main_source = include_str!("main.rs");
    let mut routed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for line in main_source.lines() {
        let Some(start) = line.find(".route(\"") else {
            continue;
        };
        let rest = &line[start + ".route(\"".len()..];
        let Some(end) = rest.find('"') else { continue };
        let path = &rest[..end];
        if path.starts_with("/api/") {
            routed.insert(path.to_string());
        }
    }
    assert!(
        routed.len() > 40,
        "route extraction should find the API surface, got {routed:?}"
    );

    let cataloged: std::collections::BTreeSet<String> = capability_endpoints()
        .into_iter()
        .flat_map(|group| group.endpoints)
        .map(|endpoint| {
            let path = endpoint
                .split_whitespace()
                .nth(1)
                .unwrap_or(endpoint)
                .split('?')
                .next()
                .unwrap_or(endpoint);
            path.to_string()
        })
        .collect();

    let missing: Vec<_> = routed
        .iter()
        .filter(|path| !cataloged.contains(*path))
        .collect();
    assert!(
        missing.is_empty(),
        "routes missing from capability_endpoints(): {missing:?}"
    );
}

#[test]
fn the_browser_fallback_only_reports_insight_kinds_the_analysis_knows() {
    // The bundle recomputes ten insight kinds itself for the case where no
    // server report is available. Two implementations of one rule set drift
    // silently; a kind renamed on the Rust side would leave the browser
    // publishing a name nothing else recognises.
    let bundle = crate::limits::APP_JS;
    // Only the fallback's own body: `kind:` also names graph slices and
    // workflow blocks elsewhere in the bundle.
    let start = bundle
        .find("function buildClientInsights(")
        .expect("the fallback is still in the bundle");
    let rest = &bundle[start..];
    let end = rest[1..]
        .find("\nfunction ")
        .map(|index| index + 1)
        .unwrap_or(rest.len());
    let fallback = &rest[..end];
    let mut kinds = Vec::new();
    for (index, _) in fallback.match_indices("kind: \"") {
        let start = index + "kind: \"".len();
        let Some(end) = fallback[start..].find('"') else {
            continue;
        };
        kinds.push(&fallback[start..start + end]);
    }
    assert!(
        kinds.len() >= 8,
        "the fallback still computes insights, found {}",
        kinds.len()
    );

    let known = codegraph_analysis::KNOWN_INSIGHT_KINDS;
    let unknown: Vec<&str> = kinds
        .into_iter()
        .filter(|kind| !known.contains(kind))
        .collect();
    assert!(
        unknown.is_empty(),
        "the browser reports kinds the analysis does not define: {unknown:?}"
    );
}

#[test]
fn embedded_web_overview_uses_report_snapshot() {
    let index = include_str!("../../codegraph-web/static/index.html");
    let app = APP_JS;

    assert!(index.contains("riskSummaryList"));
    assert!(index.contains("surprisingLinkList"));
    assert!(index.contains("reportMarkdown"));
    assert!(
        app.contains("fetchOverviewJson(`/api/report?${reportParams.toString()}`, \"report\")")
    );
    assert!(app.contains("reportFormat: \"markdown\""));
    assert!(app.contains("\"export.reportMarkdown\""));
    assert!(app.contains("renderSurprisingLinks"));
    assert!(app.contains("\"empty.noSurprisingLinks\""));
    assert!(app.contains("state.report?.quality_gate"));
    assert!(app.contains("\"risk.gate\""));
    // Refactoring panel: impact, dependencies, seams, contract, and the
    // refactor-context download must be reachable from the web UI.
    assert!(index.contains("refactorImpactButton"));
    assert!(index.contains("refactorSeamsButton"));
    assert!(index.contains("refactorContractButton"));
    assert!(index.contains("prImpactButton"));
    assert!(app.contains("/api/impact"));
    assert!(app.contains("/api/component-dependencies"));
    assert!(app.contains("/api/component-contract"));
    assert!(app.contains("/api/seams"));
    assert!(app.contains("/api/refactor-context"));
    assert!(app.contains("/api/pr-impact"));
    assert!(app.contains("refactor-context.json"));
    // Discoverability: startup panel hints and the large-graph default
    // filter must stay wired.
    assert!(app.contains("renderPanelHints"));
    assert!(app.contains("\"hint.query\""));
    assert!(app.contains("LARGE_GRAPH_FILTER_THRESHOLD"));
    assert!(app.contains("largeGraphDefaults"));
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
    assert!(app.contains("\"export.exporting\""));
    assert!(app.contains("\"export.failedFallback\""));
    assert!(app.contains("t(\"stat.nodes\").toLowerCase()"));
    assert!(app.contains("t(\"stat.edges\").toLowerCase()"));
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
    assert!(index.contains("entryFlowWorkflowButton"));
    assert!(index.contains("entryFlowWorkflowExportButton"));
    assert!(index.contains("entryFlowWorkflowMermaidExportButton"));
    assert!(index.contains("entryFlowWorkflowDotExportButton"));
    assert!(index.contains("entryWorkflowEdgeKindInput"));
    assert!(index.contains("entryWorkflowConfidenceInput"));
    assert!(index.contains("entryWorkflowLanguageInput"));
    assert!(index.contains("entryWorkflowRiskSeverityInput"));
    assert!(index.contains("entryWorkflowBlockKindInput"));
    assert!(index.contains("workflowBlockKindOptions"));
    assert!(app.contains("/api/entrypoint-workflows?"));
    assert!(app.contains("exportLastEntryWorkflowReport"));
    assert!(app.contains("entryWorkflowReportToMermaid"));
    assert!(app.contains("entryWorkflowReportToDot"));
    assert!(app.contains("appendWorkflowFilterParams(params, workflowFilters)"));
    assert!(app.contains("renderWorkflowFilterSummary(report.filters)"));
    assert!(app.contains("renderDatalist(workflowBlockKindOptions"));
    assert!(app.contains("codegraph.entrypoint_workflows.v1"));
    assert!(app.contains("\"button.buildEntryWorkflows\""));
    assert!(app.contains("\"button.downloadEntryWorkflows\""));
    assert!(app.contains("\"button.downloadEntryWorkflowMermaid\""));
    assert!(app.contains("\"button.downloadEntryWorkflowDot\""));
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
    assert!(app.contains("const LABEL_MODE_STORAGE_VERSION = \"17\""));
    assert!(index.contains("data-label-mode=\"minimal\" aria-pressed=\"true\""));
    assert!(app.contains("ambiguous_call_resolution"));
    assert!(app.contains("ambiguous_entrypoint_target"));
    assert!(index.contains("insights kind:ambiguous_call_resolution"));
    assert!(index.contains("insights kind:ambiguous_entrypoint_target"));
    assert!(index.contains("docs edge_limit:300"));
    assert!(index.contains("insights kind:mixed_dependency_scope"));
    assert!(index.contains("insights kind:conflicting_dependency_declaration"));
    assert!(index.contains("insights kind:non_runtime_dependency_import"));
    assert!(index.contains("insights kind:test_only_runtime_dependency"));
    assert!(index.contains("insights kind:sensitive_config_default"));
    assert!(index.contains("sql edge_limit:300"));
    assert!(index.contains("insights kind:unresolved_sql_table_reference"));
    assert!(app.contains("\"queryPreset.ambiguousCalls\""));
    assert!(app.contains("\"queryPreset.ambiguousEntrypoints\""));
    assert!(app.contains("\"queryPreset.dependencyScopes\""));
    assert!(app.contains("\"queryPreset.dependencyVersions\""));
    assert!(app.contains("\"queryPreset.runtimeImports\""));
    assert!(app.contains("\"queryPreset.testOnlyRuntime\""));
    assert!(app.contains("\"queryPreset.sensitiveDefaults\""));
    assert!(app.contains("\"queryPreset.sqlQueries\""));
    assert!(app.contains("\"queryPreset.sqlMissingTables\""));
    assert!(app.contains("\"queryPreset.docs\""));
    assert!(app.contains("\"selection.documentGraph\""));
    assert!(app.contains("\"selection.sqlGraph\""));
    assert!(app.contains("documentGraphQueryForNode"));
    assert!(app.contains("sqlGraphQueryForNode"));
    assert!(app.contains("\"kind.mixed_dependency_scope\""));
    assert!(app.contains("\"kind.non_runtime_dependency_import\""));
    assert!(app.contains("\"kind.test_only_runtime_dependency\""));
    assert!(app.contains("\"kind.conflicting_dependency_declaration\""));
    assert!(app.contains("\"kind.unresolved_sql_table_reference\""));
    assert!(app.contains("\"mixed dependency scope\""));
    assert!(app.contains("\"смешанный scope зависимости\""));
    assert!(app.contains("\"SQL-ссылка на неизвестную таблицу\""));
    assert!(app.contains("phpImportPackage"));
    assert!(app.contains("phpNonComposerNamespaceRoots"));
    assert!(app.contains("scope === \"local\" || scope === \"workspace\""));
    // The browser's own copy of the undeclared-import rule has to skip what
    // the CLI skips: a Node built-in, a specifier that names its source,
    // and a type-only import the `@types` package answers for.
    assert!(app.contains("\"http2\""));
    assert!(app.contains("typesPackageName"));
    assert!(app.contains("Symfony"));
    assert!(app.contains("composer"));
    assert!(app.contains("\"check.running\""));
    assert!(app.contains("\"sourceSearch.enterText\""));
    assert!(app.contains("\"sourceSearch.noMatches\""));
    assert!(app.contains("\"query.enterExpression\""));
    assert!(app.contains("\"query.running\""));
    assert!(app.contains("\"query.tooLong\""));
    assert!(index.contains("askInput"));
    assert!(index.contains("askButton"));
    assert!(index.contains("button.ask"));
    assert!(index.contains("label.question"));
    assert!(app.contains("\"ask.enterQuestion\""));
    assert!(app.contains("\"ask.running\""));
    assert!(app.contains("\"ask.generatedQuery\""));
    assert!(app.contains("runNaturalQuery"));
    assert!(app.contains("/api/ask?"));
    assert!(app.contains("renderNaturalQueryReport"));
    assert!(app.contains("data-ask-alternative"));
    assert!(app.contains("graphQueryWithinClientLimit(expression, queryResult)"));
    assert!(app.contains("graphQueryWithinClientLimit(question, queryResult)"));
    assert!(app.contains("clearLastQueryResult();"));
    assert!(app.contains("graphQueryWithinClientLimit(expression, pathResult)"));
    assert!(app.contains("max_graph_query_length"));
    assert!(app.contains("\"path.enterEndpoints\""));
    assert!(app.contains("\"path.finding\""));
    assert!(app.contains("\"path.failedFallback\""));
    assert!(app.contains("\"path.resultLabel\""));
    assert!(app.contains("t(\"path.resultLabel\")"));
    assert!(index.contains("pathExportButton"));
    assert!(app.contains("exportLastPathResult"));
    assert!(app.contains("codegraph.path_result.v1"));
    assert!(app.contains("\"button.downloadPathResult\""));
    assert!(app.contains("\"export.pathResult\""));
    assert!(app.contains("\"export.noPathResult\""));
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
fn embedded_app_js_bundle_preserves_module_order() {
    // The web UI relies on top-level statement order (dictionaries and DOM
    // wiring before init, function declarations hoisted below): the build
    // script concatenates static/js modules in lexicographic order, and this
    // guard fails if a module is renamed, dropped, or re-ordered.
    let js_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../codegraph-web/static/js");
    let mut names: Vec<String> = std::fs::read_dir(&js_dir)
        .expect("web js module directory")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".js"))
        .collect();
    names.sort();
    assert!(names.len() >= 16, "expected the split app.js modules");

    let mut cursor = 0;
    for name in &names {
        let banner = format!("// {}", name.trim_end_matches(".js"));
        let position = APP_JS[cursor..]
            .find(&banner)
            .unwrap_or_else(|| panic!("bundle should contain {name} at or after byte {cursor}"));
        cursor += position + banner.len();
    }

    // Statement-order anchors the UI depends on: constants and state before
    // DOM wiring, wiring before the init call, Flow view helpers last.
    let anchors = [
        "const I18N = {",
        "const state = {",
        "const canvas = document.querySelector",
        "async function init()",
        "function fitFlowView(",
    ];
    let mut last = 0;
    for anchor in anchors {
        let position = APP_JS
            .find(anchor)
            .unwrap_or_else(|| panic!("bundle should contain anchor {anchor}"));
        assert!(
            position >= last,
            "anchor {anchor} appears out of order in the concatenated bundle"
        );
        last = position;
    }
}

#[test]
fn embedded_web_assets_keep_shareable_investigation_links() {
    let index = include_str!("../../codegraph-web/static/index.html");
    let app = APP_JS;
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
    assert!(app.contains("\"button.buildQueryWorkflows\""));
    assert!(app.contains("data-query-workflows"));
    assert!(app.contains("/api/workflow-query?"));
    assert!(app.contains("renderWorkflowQueryReport"));
    assert!(app.contains("attachWorkflowQueryActions"));
    assert!(app.contains("\"query.workflowStarts\""));
    assert!(app.contains("QUERY_HISTORY_STORAGE_KEY"));
    assert!(app.contains("rememberQuery"));
    assert!(app.contains("renderQueryHistory"));
    assert!(app.contains("\"queryHistory.recent\""));
    assert!(app.contains("mode: \"ask\""));
    assert!(app.contains("natural_query:"));
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
fn embedded_web_assets_include_workflow_panel() {
    let app = APP_JS;
    let styles = include_str!("../../codegraph-web/static/styles.css");

    assert!(app.contains("workflowButton"));
    assert!(app.contains("workflowJsonExportButton"));
    assert!(app.contains("workflowMermaidExportButton"));
    assert!(app.contains("workflowDotExportButton"));
    assert!(app.contains("workflowEdgeKindInput"));
    assert!(app.contains("workflowConfidenceInput"));
    assert!(app.contains("workflowLanguageInput"));
    assert!(app.contains("workflowRiskSeverityInput"));
    assert!(app.contains("workflowBlockKindInput"));
    assert!(app.contains("readWorkflowFilters(\"workflow\")"));
    assert!(app.contains("workflowFilterInputs(\"workflow\")"));
    assert!(app.contains("/api/workflow?"));
    assert!(app.contains("renderWorkflow"));
    assert!(app.contains("exportLastWorkflowReport"));
    assert!(app.contains("workflowReportToMermaid"));
    assert!(app.contains("workflowReportToDot"));
    assert!(app.contains("text/vnd.graphviz"));
    assert!(app.contains("workflow.blockCount"));
    assert!(app.contains("workflow.transitionCount"));
    assert!(app.contains("\"button.downloadWorkflow\""));
    assert!(app.contains("\"button.downloadWorkflowMermaid\""));
    assert!(app.contains("\"button.downloadWorkflowDot\""));
    assert!(app.contains("\"selection.flow\""));
    assert!(styles.contains(".workflow-diagram"));
    assert!(styles.contains(".workflow-export-actions"));
    assert!(styles.contains(".workflow-block"));
    assert!(styles.contains(".workflow-transitions"));
}

#[test]
fn embedded_web_assets_localize_static_aria_labels() {
    let index = include_str!("../../codegraph-web/static/index.html");
    let app = APP_JS;

    for key in [
        "aria.interfaceLanguage",
        "aria.scanControls",
        "aria.graphSummary",
        "aria.runtimeStatus",
        "aria.jobs",
        "aria.projectOverview",
        "aria.entrypointFlows",
        "aria.graphPage",
        "aria.previousGraphPage",
        "aria.nextGraphPage",
        "aria.edgePage",
        "aria.previousEdgePage",
        "aria.nextEdgePage",
        "aria.graphQuery",
        "aria.queryPresets",
        "aria.sourceSearch",
        "aria.cacheDiagnostics",
        "aria.graphExport",
        "aria.graphPath",
        "aria.configurationTrace",
        "aria.errorTrace",
        "aria.graphInsights",
        "aria.graphFilters",
        "aria.selectedNode",
    ] {
        assert!(index.contains(&format!("data-i18n-aria-label=\"{key}\"")));
        assert!(app.contains(&format!("\"{key}\"")));
    }

    assert!(app.contains("\"Язык интерфейса\""));
    assert!(app.contains("\"Следующая страница связей\""));
}

#[test]
fn embedded_web_assets_surface_graph_viewport_hud() {
    let index = include_str!("../../codegraph-web/static/index.html");
    let app = APP_JS;
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
fn embedded_web_assets_surface_flow_view() {
    let index = include_str!("../../codegraph-web/static/index.html");
    let app = APP_JS;
    let styles = include_str!("../../codegraph-web/static/styles.css");

    assert!(index.contains("flowCanvas"));
    assert!(index.contains("flowMinimap"));
    assert!(index.contains("flowHud"));
    assert!(index.contains("data-stage-view=\"graph\""));
    assert!(index.contains("data-stage-view=\"flow\""));
    assert!(index.contains("data-i18n-aria-label=\"aria.flowCanvas\""));
    assert!(index.contains("data-i18n-aria-label=\"aria.flowMinimap\""));
    assert!(app.contains("function setStageView"));
    assert!(app.contains("function openFlowView"));
    assert!(app.contains("function layoutFlow"));
    assert!(app.contains("function drawFlow"));
    assert!(app.contains("function drawFlowMinimap"));
    assert!(app.contains("function onFlowPointerDown"));
    assert!(app.contains("function attachFlowViewActions"));
    assert!(app.contains("function flowTransitionAt"));
    assert!(app.contains("function selectFlowTransition"));
    assert!(app.contains("registerEdgeSelection(\n    transition.edge,"));
    assert!(app.contains("data-flow-view"));
    assert!(app.contains("\"flow.openView\""));
    assert!(app.contains("\"flow.empty\""));
    assert!(app.contains("\"aria.flowCanvas\""));
    assert!(app.contains("\"button.viewFlow\""));
    assert!(styles.contains("#flowCanvas"));
    assert!(styles.contains(".graph-stage[data-view=\"flow\"]"));
}

#[test]
fn embedded_web_assets_surface_journey_panel() {
    let index = include_str!("../../codegraph-web/static/index.html");
    let app = APP_JS;

    assert!(index.contains("journeyFromInput"));
    assert!(index.contains("journeyToInput"));
    assert!(index.contains("journeyRunButton"));
    assert!(index.contains("journeyExportButton"));
    assert!(index.contains("data-i18n-aria-label=\"aria.journey\""));
    assert!(app.contains("async function runJourney"));
    assert!(app.contains("function renderJourneyReport"));
    assert!(app.contains("function attachJourneyActions"));
    assert!(app.contains("data-journey-focus"));
    assert!(app.contains("/api/journey?"));
    assert!(app.contains("codegraph.journey.v1"));
    assert!(app.contains("\"journey.pathTitle\""));
    assert!(app.contains("async function expandJourneyStep"));
    assert!(app.contains("data-journey-expand"));
    assert!(app.contains("data-journey-collapse"));
    assert!(app.contains("\"journey.subflowTitle\""));
    assert!(app.contains("\"journey.fragile\""));
    assert!(app.contains("\"button.buildJourney\""));
}

#[test]
fn embedded_web_assets_highlight_hovered_graph_edges() {
    let app = APP_JS;

    assert!(app.contains("hoveredEdgeKey"));
    assert!(app.contains("edgeEmphasis"));
    assert!(app.contains("edgeTouchesNode"));
    assert!(app.contains("graphNeighborhoodContext"));
    assert!(app.contains("nodeIsNeighborhoodNeighbor"));
    assert!(app.contains("nodeIsNeighborhoodMuted"));
    assert!(app.contains("edgeNeighborhoodAlpha"));
    assert!(app.contains("!muted && shouldShowNodeLabel"));
    assert!(app.contains("\"selected-node\""));
    assert!(app.contains("\"hover-node\""));
    assert!(app.contains("edgeStrokeWidth"));
    assert!(app.contains("\"hover\""));
    assert!(app.contains("onPointerLeave"));
    assert!(app.contains("edgeHighlightColor"));
}

#[test]
fn embedded_web_assets_localize_source_match_cards() {
    let app = APP_JS;

    assert!(app.contains("\"selection.sourceMatch\""));
    assert!(app.contains("\"selection.sourceLoading\""));
    assert!(app.contains("selectionTitle.textContent = t(\"selection.sourceMatch\")"));
    assert!(app.contains("escapeHtml(t(\"selection.source\"))"));
    assert!(app.contains("escapeHtml(t(\"selection.sourceLoading\"))"));
    assert!(app.contains("\"Совпадение в коде\""));
}

#[test]
fn embedded_web_assets_support_keyboard_graph_navigation() {
    let index = include_str!("../../codegraph-web/static/index.html");
    let app = APP_JS;
    let styles = include_str!("../../codegraph-web/static/styles.css");

    assert!(index.contains("id=\"graphCanvas\" tabindex=\"0\""));
    assert!(index.contains("data-i18n-aria-label=\"aria.graphCanvas\""));
    assert!(app.contains("onCanvasKeyDown"));
    assert!(app.contains("panGraphBy"));
    assert!(app.contains("\"aria.graphCanvas\""));
    assert!(app.contains("data-i18n-aria-label"));
    assert!(styles.contains("#graphCanvas:focus-visible"));
}

#[tokio::test]
async fn a_published_default_is_the_one_the_handler_uses() {
    // The schema said `/api/node-card` lists 80 context edges by default
    // and the handler listed 24; `/api/impact` said 100 dependents and
    // the handler took 40. A default is part of the contract an agent
    // plans around.
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    let state = test_state(root.clone(), vec![], true);

    let Json(card) = node_card_api(
        State(state),
        ApiQuery(NodeCardQuery {
            path: Some(root.clone()),
            node_id: "n1".to_string(),
            edge_limit: None,
            source_context: None,
            insight_limit: None,
            include_insights: None,
        }),
    )
    .await
    .expect("node card");

    let schema = api_schema_response();
    let documented = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/node-card")
        .and_then(|endpoint| {
            endpoint
                .parameters
                .iter()
                .find(|parameter| parameter.name == "edge_limit")
                .and_then(|parameter| parameter.default)
        })
        .expect("edge_limit is published with a default");
    assert_eq!(
        documented,
        card.context.edge_limit.to_string(),
        "the published default is not the one the handler applied"
    );

    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn the_schema_describes_the_fields_the_responses_carry() {
    // An agent reads the schema before the response. A field that is
    // returned and not described, or described and not returned, is a
    // contract that drifted: `/api/capabilities` grew `export_formats`
    // and `scan`, `/api/impact` grew `max_depth`, and `/api/summary` and
    // `/api/coverage` described nothing at all.
    let root = temp_server_root();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    helper();\n}\n\nfn helper() {}\n",
    )
    .unwrap();
    let state = test_state(root.clone(), vec![], true);
    let query = || ScanQuery {
        path: Some(root.clone()),
        limit: None,
    };

    let Json(summary_body) = summary(State(state.clone()), ApiQuery(query()))
        .await
        .expect("summary");
    let Json(coverage_body) = coverage_api(State(state.clone()), ApiQuery(query()))
        .await
        .expect("coverage");
    let Json(capabilities_body) = capabilities_api(State(state.clone()))
        .await
        .expect("capabilities");
    // `notes` rides on these two only when a name was shared, so it is
    // described as optional and must not be demanded here.
    let Json(trace_body) = trace_api(
        State(state.clone()),
        ApiQuery(TraceQuery {
            path: Some(root.clone()),
            label: Some("main".to_string()),
            node_id: None,
            depth: Some(2),
        }),
    )
    .await
    .expect("trace");
    let Json(workflow_body) = workflow_api(
        State(state),
        ApiQuery(WorkflowQuery {
            path: Some(root.clone()),
            label: Some("main".to_string()),
            node_id: None,
            depth: Some(2),
            block_limit: Some(20),
            edge_kind: None,
            confidence: None,
            language: None,
            risk_severity: None,
            block_kind: None,
            compact: None,
            max_fanout: None,
        }),
    )
    .await
    .expect("workflow");

    let schema = api_schema_response();
    let fields_of = |path: &str| -> Vec<(&'static str, &'static str, bool)> {
        schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == path)
            .map(|endpoint| {
                endpoint
                    .response_fields
                    .iter()
                    .map(|field| (field.name, field.location, field.required))
                    .collect()
            })
            .unwrap_or_else(|| panic!("{path} is not in the schema"))
    };
    let checks: Vec<(&str, serde_json::Value)> = vec![
        ("/api/summary", serde_json::to_value(summary_body).unwrap()),
        (
            "/api/coverage",
            serde_json::to_value(coverage_body).unwrap(),
        ),
        (
            "/api/capabilities",
            serde_json::to_value(capabilities_body).unwrap(),
        ),
        ("/api/trace", serde_json::to_value(trace_body).unwrap()),
        (
            "/api/workflow",
            serde_json::to_value(workflow_body).unwrap(),
        ),
    ];
    for (path, body) in checks {
        let body = body.as_object().expect("an object response");
        let documented = fields_of(path);
        assert!(
            !documented.is_empty(),
            "{path} describes none of its response fields"
        );
        for (name, location, required) in &documented {
            if *location == "response" && *required {
                assert!(
                    body.contains_key(*name),
                    "{path} describes `{name}` and did not return it"
                );
            }
        }
        for key in body.keys() {
            assert!(
                documented
                    .iter()
                    .any(|(name, location, _)| *location == "response" && name == key),
                "{path} returned `{key}` and describes it nowhere"
            );
        }
    }

    fs::remove_dir_all(root).ok();
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
    assert!(
        schema
            .enum_values
            .get("export_format")
            .is_some_and(|formats| {
                formats
                    == &vec![
                        "json",
                        "dot",
                        "ndjson",
                        "graphml",
                        "svg",
                        "mermaid_html",
                        "cypher",
                        "falkordb",
                    ]
            })
    );
    assert!(
        schema
            .enum_values
            .get("report_format")
            .is_some_and(|formats| formats == &vec!["json", "markdown"])
    );
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
    assert!(
        schema
            .enum_values
            .get("workflow_block_kind")
            .is_some_and(|kinds| kinds.contains(&"start")
                && kinds.contains(&"config_read")
                && kinds.contains(&"branch")
                && kinds.contains(&"loop")
                && kinds.contains(&"async")
                && kinds.contains(&"return")
                && kinds.contains(&"external_boundary"))
    );
    assert!(schema.enum_values.get("insight_kind").is_some_and(|kinds| {
        kinds.contains(&"sensitive_config_default")
            && kinds.contains(&"ambiguous_call_resolution")
            && kinds.contains(&"ambiguous_entrypoint_target")
            && kinds.contains(&"dependency_cycle")
            && kinds.contains(&"mixed_dependency_scope")
            && kinds.contains(&"non_runtime_dependency_import")
            && kinds.contains(&"test_only_runtime_dependency")
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
                && sections.contains(&"surprising_links")
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
    assert!(schema.enum_values.get("cache_status").is_some_and(
        |statuses| statuses.contains(&"hit")
            && statuses.contains(&"miss")
            && statuses.contains(&"disabled")
    ));
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
                    && commands.contains(&"docs")
                    && commands.contains(&"sql")
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
            .get("graph_query_document_term")
            .is_some_and(|terms| {
                terms.contains(&"document_kind")
                    && terms.contains(&"heading")
                    && terms.contains(&"target")
                    && terms.contains(&"relation")
            })
    );
    assert!(
        schema
            .enum_values
            .get("graph_query_sql_term")
            .is_some_and(|terms| {
                terms.contains(&"table")
                    && terms.contains(&"operation")
                    && terms.contains(&"unresolved")
                    && terms.contains(&"relation")
            })
    );
    assert!(
        schema
            .enum_values
            .get("graph_query_package_term")
            .is_some_and(|terms| {
                terms.contains(&"package")
                    && terms.contains(&"ecosystem")
                    && terms.contains(&"version_kind")
                    && terms.contains(&"dependency_version_kind")
            })
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
        header.name == EXPORT_BYTES_HEADER && header.value_type == "usize_bytes" && header.required
    }));
    assert!(endpoints.contains(&("GET", "/api/live")));
    assert!(endpoints.contains(&("GET", "/api/ready")));
    let live_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/live")
        .expect("schema should list live endpoint");
    assert!(
        live_endpoint.response_fields.iter().any(|field| {
            field.name == "status" && field.location == "response" && field.required
        })
    );
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
    assert!(endpoints.contains(&("GET", "/api/ask")));
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
    assert!(
        graph_endpoint.response_fields.iter().any(|field| {
            field.name == "nodes" && field.value_type == "Node[]" && field.required
        })
    );
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
    assert!(focus_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "compact"
            && parameter.value_type == "bool"
            && parameter.default == Some("false")
    }));
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
    assert!(query_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "compact"
            && parameter.value_type == "bool"
            && parameter.default == Some("false")
    }));
    assert!(
        query_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "compact" && field.value_type == "bool" })
    );
    assert!(
        query_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "raw_total_nodes" && field.value_type == "usize" })
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
    let ask_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/ask")
        .expect("schema should list ask endpoint");
    assert!(
        ask_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "q"
                && parameter.required
                && parameter.max_length == Some(MAX_GRAPH_QUERY_LENGTH)
                && parameter.capability_limit == Some("max_graph_query_length"))
    );
    assert!(ask_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "compact"
            && parameter.value_type == "bool"
            && parameter.default == Some("false")
    }));
    assert!(ask_endpoint.response_fields.iter().any(|field| {
        field.name == "generated_query" && field.value_type == "string" && field.required
    }));
    assert!(ask_endpoint.response_fields.iter().any(|field| {
        field.name == "result" && field.value_type == "QueryResult" && field.required
    }));
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
    assert!(report_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "format" && parameter.value_type == "report_format"
    }));
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
    let report_file_summary_limit = report_endpoint
        .parameters
        .iter()
        .find(|parameter| parameter.name == "file_summary_limit")
        .expect("report file_summary_limit");
    assert_eq!(report_file_summary_limit.minimum, Some(1));
    assert_eq!(
        report_file_summary_limit.maximum,
        Some(MAX_REPORT_FILE_SUMMARY_LIMIT)
    );
    assert_eq!(
        report_file_summary_limit.capability_limit,
        Some("max_report_file_summary_limit")
    );
    let report_node_summary_limit = report_endpoint
        .parameters
        .iter()
        .find(|parameter| parameter.name == "node_summary_limit")
        .expect("report node_summary_limit");
    assert_eq!(report_node_summary_limit.minimum, Some(1));
    assert_eq!(
        report_node_summary_limit.maximum,
        Some(MAX_REPORT_NODE_SUMMARY_LIMIT)
    );
    assert_eq!(
        report_node_summary_limit.capability_limit,
        Some("max_report_node_summary_limit")
    );
    assert!(
        report_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "coverage" && field.value_type == "ScanCoverageReport" })
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
    let surprising_links_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/surprising-links")
        .expect("schema should list surprising-links endpoint");
    let surprising_limit = surprising_links_endpoint
        .parameters
        .iter()
        .find(|parameter| parameter.name == "limit")
        .expect("surprising links limit");
    assert_eq!(
        surprising_limit.capability_limit,
        Some("max_report_architecture_edge_limit")
    );
    assert!(
        surprising_links_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "links" && field.value_type == "SurprisingLink[]" })
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
    assert!(
        hotspots_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "architectural_hubs" && field.value_type == "Hotspot[]" })
    );
    assert!(
        hotspots_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "utility_hubs" && field.value_type == "Hotspot[]" })
    );
    let communities_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/communities")
        .expect("schema should list communities endpoint");
    let community_limit = communities_endpoint
        .parameters
        .iter()
        .find(|parameter| parameter.name == "limit")
        .expect("communities limit");
    assert_eq!(
        community_limit.capability_limit,
        Some("max_report_community_limit")
    );
    assert!(
        communities_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "communities" && field.value_type == "GraphCommunity[]" })
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
    let entrypoint_workflows_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/entrypoint-workflows")
        .expect("schema should list entrypoint-workflows endpoint");
    assert!(
        entrypoint_workflows_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "block_limit")
    );
    assert!(
        entrypoint_workflows_endpoint
            .parameters
            .iter()
            .any(|parameter| {
                parameter.name == "edge_kind" && parameter.value_type == "graph_edge_kind"
            })
    );
    assert!(
        entrypoint_workflows_endpoint
            .parameters
            .iter()
            .any(|parameter| {
                parameter.name == "block_kind" && parameter.value_type == "workflow_block_kind"
            })
    );
    assert!(
        entrypoint_workflows_endpoint
            .parameters
            .iter()
            .any(|parameter| {
                parameter.name == "entrypoint_kind" && parameter.value_type == "entrypoint_kind"
            })
    );
    assert!(
        entrypoint_workflows_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "entrypoint_kind" && field.value_type == "string")
    );
    assert!(
        schema
            .enum_values
            .get("entrypoint_kind")
            .is_some_and(|kinds| {
                kinds.contains(&"route")
                    && kinds.contains(&"make_target")
                    && kinds.contains(&"workflow_job")
                    && kinds.contains(&"service")
            })
    );
    assert!(
        entrypoint_workflows_endpoint
            .parameters
            .iter()
            .any(|parameter| {
                parameter.name == "compact"
                    && parameter.value_type == "bool"
                    && parameter.default == Some("false")
            })
    );
    assert!(
        entrypoint_workflows_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "filters" && field.value_type == "WorkflowFilters")
    );
    assert!(
        entrypoint_workflows_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "workflows" && field.value_type == "WorkflowReport[]" })
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
    let workflow_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/workflow")
        .expect("schema should list workflow endpoint");
    assert!(
        workflow_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "blocks" && field.value_type == "WorkflowBlock[]" })
    );
    assert!(workflow_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "confidence" && parameter.value_type == "graph_confidence"
    }));
    assert!(workflow_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "risk_severity" && parameter.value_type == "insight_severity"
    }));
    assert!(workflow_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "compact"
            && parameter.value_type == "bool"
            && parameter.default == Some("false")
    }));
    assert!(
        workflow_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "filters" && field.value_type == "WorkflowFilters" })
    );
    assert!(
        workflow_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "compact" && field.value_type == "bool" })
    );
    assert!(
        workflow_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "raw_total_blocks" && field.value_type == "usize" })
    );
    assert!(workflow_endpoint.response_fields.iter().any(|field| {
        field.name == "transitions" && field.value_type == "WorkflowTransition[]"
    }));
    let workflow_query_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/workflow-query")
        .expect("schema should list workflow-query endpoint");
    assert!(workflow_query_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "q"
            && parameter.value_type == "graph_query"
            && parameter.max_length == Some(MAX_GRAPH_QUERY_LENGTH)
    }));
    assert!(workflow_query_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "block_kind" && parameter.value_type == "workflow_block_kind"
    }));
    assert!(workflow_query_endpoint.parameters.iter().any(|parameter| {
        parameter.name == "compact"
            && parameter.value_type == "bool"
            && parameter.default == Some("false")
    }));
    assert!(
        workflow_query_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "workflows" && field.value_type == "WorkflowReport[]" })
    );
    let journey_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/journey")
        .expect("schema should list journey endpoint");
    assert!(
        journey_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "from" && parameter.required)
    );
    assert!(
        journey_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "to" && parameter.required)
    );
    assert!(
        journey_endpoint
            .parameters
            .iter()
            .any(|parameter| { parameter.name == "depth" && parameter.default == Some("8") })
    );
    assert!(
        journey_endpoint
            .parameters
            .iter()
            .any(|parameter| { parameter.name == "paths" && parameter.default == Some("3") })
    );
    assert!(
        journey_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "paths" && field.value_type == "JourneyPath[]")
    );
    let mcp_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/mcp")
        .expect("schema should list HTTP MCP endpoint");
    assert_eq!(mcp_endpoint.method, "POST");
    assert_eq!(mcp_endpoint.body, Some("McpJsonRpcMessage"));
    assert!(
        mcp_endpoint
            .body_fields
            .iter()
            .any(|field| field.name == "method" && field.required)
    );
    assert!(
        mcp_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "result" && !field.required)
    );
    let component_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/component-dependencies")
        .expect("schema should list component-dependencies endpoint");
    assert!(
        component_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "target" && parameter.required)
    );
    assert!(component_endpoint.response_fields.iter().any(|field| {
        field.name == "areas" && field.value_type == "ComponentDependencyGroup[]"
    }));
    let contract_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/component-contract")
        .expect("schema should list component-contract endpoint");
    assert!(
        contract_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "source" && parameter.required)
    );
    assert!(
        contract_endpoint.response_fields.iter().any(|field| {
            field.name == "edges" && field.value_type == "ComponentContractEdge[]"
        })
    );
    let impact_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/impact")
        .expect("schema should list impact endpoint");
    assert!(
        impact_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "target" && parameter.required)
    );
    assert!(
        impact_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "impact_score" && field.value_type == "usize")
    );
    assert!(
        impact_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "dependents" && field.value_type == "ImpactDependent[]" })
    );
    let seams_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/seams")
        .expect("schema should list seams endpoint");
    assert!(
        seams_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "safest" && field.value_type == "SeamCandidate[]")
    );
    assert!(
        seams_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "most_needed" && field.value_type == "SeamCandidate[]")
    );
    let refactor_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/refactor-context")
        .expect("schema should list refactor-context endpoint");
    assert!(
        refactor_endpoint
            .parameters
            .iter()
            .any(|parameter| parameter.name == "target" && parameter.required)
    );
    assert!(
        refactor_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "impact" && field.value_type == "ImpactReport")
    );
    assert!(
        refactor_endpoint
            .response_fields
            .iter()
            .any(|field| field.name == "journey" && field.value_type == "JourneyReport")
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
    assert!(
        insights_endpoint.response_fields.iter().any(|field| {
            field.name == "by_severity" && field.value_type == "map<string,usize>"
        })
    );
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
    assert!(
        check_endpoint.response_fields.iter().any(|field| {
            field.name == "passed" && field.value_type == "bool" && field.required
        })
    );
    let source_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/source")
        .expect("schema should list source endpoint");
    assert!(
        source_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "lines" && field.value_type == "SourcePreviewLine[]" })
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
        source_search_endpoint
            .response_fields
            .iter()
            .any(|field| { field.name == "matches" && field.value_type == "SourceSearchMatch[]" })
    );
}

#[test]
fn api_schema_investigation_endpoints_publish_examples() {
    let schema = api_schema_response();
    for group in schema
        .groups
        .iter()
        .filter(|group| matches!(group.group, "graph" | "analysis" | "source"))
    {
        for endpoint in &group.endpoints {
            let example = endpoint
                .example
                .unwrap_or_else(|| panic!("{} should publish an example", endpoint.path));
            assert!(
                example.contains(endpoint.path),
                "{} example should be a copy-paste request for the same path",
                endpoint.path
            );
        }
    }
    let ask_endpoint = schema
        .groups
        .iter()
        .flat_map(|group| group.endpoints.iter())
        .find(|endpoint| endpoint.path == "/api/ask")
        .expect("schema should list ask endpoint");
    assert!(ask_endpoint.response_fields.iter().any(|field| {
        field.name == "cli_snippet" && field.value_type == "string" && field.required
    }));
    for path in ["/api/impact", "/api/journey"] {
        let endpoint = schema
            .groups
            .iter()
            .flat_map(|group| group.endpoints.iter())
            .find(|endpoint| endpoint.path == path)
            .expect("schema should list endpoint");
        assert!(endpoint.response_fields.iter().any(|field| {
            field.name == "schema" && field.value_type == "string" && field.required
        }));
        assert!(
            endpoint.response_fields.iter().any(|field| {
                field.name == "suggested_commands" && field.value_type == "string[]"
            })
        );
    }
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
async fn start_scan_job_with_bad_config_leaves_no_queued_job() {
    // A config error must fail the request before a job is inserted; otherwise a
    // Queued job lingers forever (no terminal update, SSE polls it indefinitely).
    let temp = temp_server_root();
    let root = temp.join("proj");
    fs::create_dir_all(root.join(".codegraph")).unwrap();
    fs::write(
        root.join(".codegraph").join("config.toml"),
        "this is not = valid = toml",
    )
    .unwrap();
    let root = root.canonicalize().unwrap();
    let state = test_state(root, vec![], false);

    let result = start_scan_job(State(state.clone()), Json(ScanJobRequest { path: None })).await;

    assert!(result.is_err(), "malformed config must fail the request");
    assert!(
        state.jobs.read().await.is_empty(),
        "no Queued job may be left behind"
    );
}

#[test]
fn host_value_is_loopback_accepts_local_forms_and_rejects_foreign() {
    for local in [
        "localhost",
        "LOCALHOST",
        "localhost:3765",
        "127.0.0.1",
        "127.0.0.1:3765",
        "[::1]",
        "[::1]:3765",
    ] {
        assert!(host_value_is_loopback(local), "{local} must be accepted");
    }
    for foreign in [
        "evil.example",
        "evil.example:3765",
        "127.0.0.1.evil.example",
        "localhost.evil.example",
        "[2001:db8::1]:3765",
        "[::1",
    ] {
        assert!(
            !host_value_is_loopback(foreign),
            "{foreign} must be rejected"
        );
    }
}

#[tokio::test]
async fn insert_scan_job_rejects_when_active_jobs_reach_limit() {
    // Prune only removes terminal jobs, so the queue must refuse new work once
    // max_jobs non-terminal jobs exist — otherwise a POST flood queues
    // unbounded tasks. Terminal jobs don't count against the limit.
    let jobs = RwLock::new(BTreeMap::new());
    for index in 0..2 {
        insert_scan_job(
            &jobs,
            test_scan_job(&format!("scan-{index}"), ScanJobStatus::Queued, 10, None),
            2,
        )
        .await
        .unwrap();
    }

    let rejected = insert_scan_job(
        &jobs,
        test_scan_job("scan-overflow", ScanJobStatus::Queued, 10, None),
        2,
    )
    .await;
    assert_eq!(rejected, Err(2), "third active job is refused");
    assert!(!jobs.read().await.contains_key("scan-overflow"));

    // A terminal job frees a slot.
    cancel_scan_job_in_store(&jobs, "scan-0", 2).await.unwrap();
    insert_scan_job(
        &jobs,
        test_scan_job("scan-after-cancel", ScanJobStatus::Queued, 11, None),
        2,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn cancel_scan_job_marks_queued_job_terminal() {
    let jobs = RwLock::new(BTreeMap::new());
    insert_scan_job(
        &jobs,
        test_scan_job("scan-1", ScanJobStatus::Queued, 10, None),
        4,
    )
    .await
    .unwrap();

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
    .await
    .unwrap();
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
    .await
    .unwrap();

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
        semantic_auto: false,
        next_job_id: Arc::new(AtomicU64::new(1)),
        scan_cancellations: Arc::new(RwLock::new(BTreeMap::new())),
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

#[tokio::test]
async fn only_the_newest_completed_jobs_keep_their_graph() {
    // A completed job holds its whole graph, which can be hundreds of MB; the
    // store keeps up to max_scan_jobs entries for history, so only the newest
    // few may stay resident.
    let jobs = RwLock::new(BTreeMap::new());
    for index in 0..(MAX_RESIDENT_JOB_GRAPHS + 2) {
        let id = format!("scan-{index}");
        insert_scan_job(
            &jobs,
            test_scan_job(&id, ScanJobStatus::Running, 10 + index as u64, None),
            32,
        )
        .await
        .unwrap();
        update_scan_job(
            &jobs,
            &id,
            ScanJobUpdate {
                status: ScanJobStatus::Complete,
                message: "complete".to_string(),
                cache: None,
                summary: None,
                graph: Some(CodeGraph::new("demo")),
            },
            32,
        )
        .await;
    }

    let store = jobs.read().await;
    let resident: Vec<_> = store
        .values()
        .filter(|job| job.graph.is_some())
        .map(|job| job.id.clone())
        .collect();
    assert_eq!(
        resident.len(),
        MAX_RESIDENT_JOB_GRAPHS,
        "only the newest graphs stay resident: {resident:?}"
    );
    assert!(
        resident.contains(&format!("scan-{}", MAX_RESIDENT_JOB_GRAPHS + 1)),
        "the newest job keeps its graph: {resident:?}"
    );
    assert_eq!(
        store.len(),
        MAX_RESIDENT_JOB_GRAPHS + 2,
        "status history is untouched"
    );
}
