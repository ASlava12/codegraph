//! Machine-readable API discovery: project/language listings and the
//! full /api/schema contract — endpoint groups, parameter bounds, body
//! and response field docs, examples, and capability listings.

use codegraph_analysis::{
    KNOWN_INSIGHT_KINDS, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT,
    MAX_REPORT_COMMUNITY_LIMIT, MAX_REPORT_FILE_SUMMARY_LIMIT, MAX_REPORT_HOTSPOT_LIMIT,
    MAX_REPORT_INSIGHT_LIMIT, MAX_REPORT_LANGUAGE_LINK_LIMIT, MAX_REPORT_NODE_SUMMARY_LIMIT,
};
use codegraph_core::CODEGRAPH_SCHEMA_VERSION;
use codegraph_lsp::{MAX_SEMANTIC_REQUEST_TIMEOUT_MS, MAX_SEMANTIC_WORK_ITEM_LIMIT};
use codegraph_parser::language_adapters;
use std::collections::BTreeMap;

#[allow(unused_imports)]
use crate::*;

pub(crate) fn project_responses(state: &AppState) -> Vec<ProjectResponse> {
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

pub(crate) fn language_responses() -> Vec<LanguageResponse> {
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

pub(crate) fn api_schema_response() -> ApiSchemaResponse {
    ApiSchemaResponse {
        name: "CodeGraph API",
        server_version: SERVER_VERSION,
        api_version: 1,
        graph_schema_version: CODEGRAPH_SCHEMA_VERSION,
        description: "Machine-readable API contract for CodeGraph clients and agents.",
        common_response_headers: api_schema_common_response_headers(),
        groups: api_schema_groups(),
        enum_values: BTreeMap::from([
            (
                "export_format",
                vec![
                    "json",
                    "dot",
                    "ndjson",
                    "graphml",
                    "svg",
                    "mermaid_html",
                    "cypher",
                    "falkordb",
                ],
            ),
            ("report_format", vec!["json", "markdown"]),
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
                    "control_flow",
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
                "workflow_block_kind",
                vec![
                    "start",
                    "call",
                    "config_read",
                    "environment_read",
                    "dependency",
                    "import",
                    "branch",
                    "loop",
                    "async",
                    "return",
                    "error",
                    "reference",
                    "external_boundary",
                    "unknown",
                ],
            ),
            (
                "entrypoint_kind",
                vec![
                    "route",
                    "script",
                    "binary",
                    "console_script",
                    "executable",
                    "make_target",
                    "workflow_job",
                    "pipeline_job",
                    "service",
                    "workload",
                    "ingress",
                    "entrypoint",
                    "cmd",
                ],
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
                    "surprising_links",
                    "hotspots",
                    "communities",
                    "file_summaries",
                    "node_summaries",
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
                    "docs",
                    "sql",
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
                    "stable_id",
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
                    "stable_id",
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
                    "stable_id",
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
                    "stable_id",
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
                    "stable_id",
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
                "graph_query_document_term",
                vec![
                    "id",
                    "node",
                    "node_id",
                    "stable_id",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "node_kind",
                    "item_kind",
                    "document_kind",
                    "doc_kind",
                    "type",
                    "heading",
                    "anchor",
                    "path",
                    "source_path",
                    "file",
                    "file_path",
                    "path_prefix",
                    "target",
                    "relation",
                    "edge_kind",
                    "confidence",
                    "direction",
                    "dir",
                    "edge_limit",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_sql_term",
                vec![
                    "id",
                    "node",
                    "node_id",
                    "stable_id",
                    "label",
                    "search",
                    "language",
                    "kind",
                    "node_kind",
                    "item_kind",
                    "table",
                    "table_name",
                    "table_key",
                    "column",
                    "column_name",
                    "column_key",
                    "operation",
                    "query",
                    "resolution",
                    "unresolved",
                    "path",
                    "source_path",
                    "file",
                    "file_path",
                    "path_prefix",
                    "target",
                    "relation",
                    "edge_kind",
                    "confidence",
                    "direction",
                    "dir",
                    "edge_limit",
                    "metadata.*",
                ],
            ),
            (
                "graph_query_config_term",
                vec![
                    "id",
                    "node_id",
                    "stable_id",
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
                    "stable_id",
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
                    "stable_id",
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
                    "stable_id",
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
                    "version_kind",
                    "dependency_version_kind",
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
                    "stable_id",
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
                    "stable_id",
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

pub(crate) fn api_schema_common_response_headers() -> Vec<ApiHeaderSpec> {
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

pub(crate) fn export_response_headers() -> Vec<ApiHeaderSpec> {
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

pub(crate) fn api_schema_groups() -> Vec<ApiSchemaGroup> {
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
                )
                .with_response_fields(scan_options_response_fields())
                .with_example("/api/scan-options?path=."),
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
                )
                .with_response_fields(scan_coverage_response_fields())
                .with_example("/api/coverage?path=."),
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
                    "Queue a long-running scan job. The job builds the syntactic graph and can be cancelled; ask for language-server facts with the semantic jobs, which `/api/scan` and the analysis endpoints apply on their own.",
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
                    "Return graph result for a completed scan job. Syntactic, as the job built it.",
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
                .with_response_headers(export_response_headers())
                .with_example("/api/export?path=.&format=dot"),
                api_get(
                    "/api/graph",
                    "Read a server-side paged and filtered graph slice. Returned edges include metadata.edge_index for exact edge explanation and UI selection.",
                    graph_slice_params(),
                    "GraphSlice",
                )
                .with_response_fields(graph_slice_response_fields())
                .with_example("/api/graph?path=.&node_limit=250&kind=function&search=scan"),
                api_get(
                    "/api/node-context",
                    "Read selected node context with neighboring edges. Returned edges include metadata.edge_index for exact edge explanation and UI selection.",
                    vec![
                        path_param(),
                        query_param("node_id", true, "string", None, "Node id: numeric (42), n-prefixed (n42), or the durable cg-* id the scan stamps on every node."),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("24"),
                            "Maximum context edges.",
                        )
                        .with_range(1, MAX_NODE_CONTEXT_EDGE_LIMIT)
                        .with_capability_limit("max_node_context_edge_limit"),
                    ],
                    "NodeContext",
                )
                .with_response_fields(node_context_response_fields())
                .with_example("/api/node-context?path=.&node_id=n42&edge_limit=40"),
                api_get(
                    "/api/node-card",
                    "Read selected node investigation card with neighboring edges, dependency summary facets, file-level summaries, source preview, related risks including file-scoped contained-node risks, risk summaries, exact edge indexes, and suggested focused graph query actions.",
                    vec![
                        path_param(),
                        query_param("node_id", true, "string", None, "A label such as `main`, or a node id: numeric (42), n-prefixed (n42), or the durable cg-* id the scan stamps on every node. A label several definitions answer to is picked by rank, and `notes` says which."),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("24"),
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
                        query_param(
                            "include_insights",
                            false,
                            "bool",
                            Some("false"),
                            "Run repository-wide insight analysis for related risks.",
                        ),
                    ],
                    "NodeCard",
                )
                .with_response_fields(node_card_response_fields())
                .with_example("/api/node-card?path=.&node_id=n42&source_context=8"),
                api_get(
                    "/api/focus",
                    "Build a focused subgraph from node ids and edge indexes. Returned edges include metadata.edge_index for exact edge explanation and UI selection.",
                    vec![
                        path_param(),
                        query_param(
                            "node_ids",
                            false,
                            "csv<string>",
                            None,
                            "Node ids to include: durable cg-* ids, numeric, or n-prefixed.",
                        ),
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
                        query_param(
                            "compact",
                            false,
                            "bool",
                            Some("false"),
                            "Collapse repeated low-signal nodes in the focused graph.",
                        ),
                    ],
                    "QueryResult",
                )
                .with_response_fields(query_result_response_fields())
                .with_example("/api/focus?path=.&node_ids=42,43&edge_limit=100"),
                api_get(
                    "/api/summary",
                    "Summarize graph node/edge facts and facets.",
                    vec![path_param()],
                    "GraphSummary",
                )
                .with_response_fields(graph_summary_response_fields())
                .with_example("/api/summary?path=."),
                api_get(
                    "/api/query",
                    "Run a focused graph query expression such as nodes, edges, calls, neighbors, path, dependents, symbols, files, docs, sql, entrypoints, routes, packages, configs, errors, cycles, hotspots, unreachable, diagnostics, annotations, or insights. QueryResult includes returned counts, edge metadata.edge_index values, and facets for node kinds, edge kinds, languages, item kinds, and confidence.",
                    vec![
                        path_param(),
                        query_param(
                            "q",
                            true,
                            "string",
                            None,
                            "Graph query expression, for example `symbols label:load_config direction:out`, `files path:src/main.rs direction:out`, `docs target:src/main.rs relation:markdown_link`, `sql table:users operation:select`, `entrypoints language:rust`, `routes method:GET path:/users`, `packages package:serde ecosystem:cargo`, `configs target:DATABASE_URL`, `errors target:panic`, `cycles edge_kind:calls`, `hotspots language:rust min_score:5`, `unreachable scope:errors search:LegacyError`, `diagnostics severity:error language:rust`, `annotations key:domain value:payments`, or `insights severity:error`.",
                        )
                        .with_max_length(MAX_GRAPH_QUERY_LENGTH)
                        .with_capability_limit("max_graph_query_length"),
                        query_param(
                            "compact",
                            false,
                            "bool",
                            Some("false"),
                            "Collapse repeated low-signal nodes in the query result.",
                        ),
                    ],
                    "QueryResult",
                )
                .with_response_fields(query_result_response_fields())
                .with_example("/api/query?path=.&q=neighbors label:main direction:out depth:2 edge_kind:calls"),
                api_get(
                    "/api/ask",
                    "Map a natural-language investigation question to a deterministic bounded graph query, run it, and return the generated query, rule, confidence, alternatives, and QueryResult.",
                    vec![
                        path_param(),
                        query_param(
                            "q",
                            true,
                            "string",
                            None,
                            "Natural-language question, for example `Where is DATABASE_URL read?`, `Кто вызывает load_config?`, or `Show path from main to init_db`.",
                        )
                        .with_max_length(MAX_GRAPH_QUERY_LENGTH)
                        .with_capability_limit("max_graph_query_length"),
                        query_param(
                            "compact",
                            false,
                            "bool",
                            Some("false"),
                            "Collapse repeated low-signal nodes in the generated query result.",
                        ),
                    ],
                    "NaturalQueryReport",
                )
                .with_response_fields(natural_query_response_fields())
                .with_example("/api/ask?path=.&q=Where is DATABASE_URL read?"),
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
                .with_response_fields(edge_explanation_response_fields())
                .with_example("/api/explain-edge?path=.&edge_index=12430"),
            ],
        },
        ApiSchemaGroup {
            group: "analysis",
            endpoints: vec![
                api_get(
                    "/api/report",
                    "Return a production project report snapshot with cache, coverage, summary, compact node/file summaries, full-project risk scoring, quality gate, topology, and hotspots.",
                    report_params(),
                    "ProjectReportResponse",
                )
                .with_response_fields(project_report_response_fields())
                .with_example("/api/report?path=.&format=json"),
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
                .with_response_fields(architecture_map_response_fields())
                .with_example("/api/architecture?path=."),
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
                .with_response_fields(language_dependency_response_fields())
                .with_example("/api/language-dependencies?path=.&limit=20"),
                api_get(
                    "/api/surprising-links",
                    "Rank cross-area, cross-language, low-confidence, and boundary dependency links with exact edge evidence.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("50"),
                            "Maximum surprising links.",
                        )
                        .with_range(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT)
                        .with_capability_limit("max_report_architecture_edge_limit"),
                    ],
                    "SurprisingLinkReport",
                )
                .with_response_fields(surprising_link_response_fields())
                .with_example("/api/surprising-links?path=.&limit=10"),
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
                .with_response_fields(hotspot_response_fields())
                .with_example("/api/hotspots?path=.&limit=10"),
                api_get(
                    "/api/communities",
                    "List deterministic graph communities/subsystems with sample nodes and edge indexes.",
                    vec![
                        path_param(),
                        query_param("limit", false, "usize", Some("25"), "Maximum communities.")
                            .with_range(1, MAX_REPORT_COMMUNITY_LIMIT)
                            .with_capability_limit("max_report_community_limit"),
                    ],
                    "CommunityReport",
                )
                .with_response_fields(community_response_fields())
                .with_example("/api/communities?path=.&limit=10"),
                api_get(
                    "/api/entrypoints",
                    "List detected entrypoint candidate nodes, ranked with programs first.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            None,
                            "Maximum entrypoints to return; omitted returns all of them.",
                        ),
                    ],
                    "Node[]",
                )
                .with_example("/api/entrypoints?path=.&limit=20"),
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
                .with_response_fields(entrypoint_trace_response_fields())
                .with_example("/api/entrypoint-traces?path=.&search=server&depth=3"),
                api_get(
                    "/api/entrypoint-workflows",
                    "Convert detected entrypoints into block-style workflow reports.",
                    vec![
                        path_param(),
                        query_param(
                            "search",
                            false,
                            "string",
                            None,
                            "Filter entrypoints by label/kind/language/metadata.",
                        ),
                        query_param(
                            "entrypoint_kind",
                            false,
                            "entrypoint_kind",
                            None,
                            "Restrict entrypoints to a matching entrypoint_kind metadata value such as route, workflow_job, pipeline_job, make_target, service, or cmd.",
                        ),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("4"),
                            "Maximum workflow traversal depth.",
                        )
                        .with_range(1, 32),
                        query_param(
                            "block_limit",
                            false,
                            "usize",
                            Some("200"),
                            "Maximum workflow blocks per entrypoint.",
                        )
                        .with_range(1, 1_000),
                        query_param(
                            "include_risks",
                            false,
                            "bool",
                            Some("false"),
                            "Run repository-wide risk analysis and include it in the score.",
                        ),
                        query_param(
                            "max_fanout",
                            false,
                            "usize",
                            None,
                            "Cap outgoing edges expanded per node (calls first) so the block budget follows the call chain into depth. Unset means unbounded.",
                        )
                        .with_range(1, 200),
                        query_param(
                            "edge_kind",
                            false,
                            "graph_edge_kind",
                            None,
                            "Restrict workflow traversal to matching edge kinds.",
                        ),
                        query_param(
                            "confidence",
                            false,
                            "graph_confidence",
                            None,
                            "Restrict workflow traversal to matching edge confidence.",
                        ),
                        query_param(
                            "language",
                            false,
                            "string",
                            None,
                            "Restrict returned workflow blocks to matching node language metadata.",
                        ),
                        query_param(
                            "risk_severity",
                            false,
                            "insight_severity",
                            None,
                            "Restrict returned workflow blocks and transitions to matching risk severity.",
                        ),
                        query_param(
                            "block_kind",
                            false,
                            "workflow_block_kind",
                            None,
                            "Restrict returned workflow blocks to matching workflow block kinds.",
                        ),
                        query_param(
                            "compact",
                            false,
                            "bool",
                            Some("false"),
                            "Collapse repeated low-signal workflow blocks into aggregate blocks.",
                        ),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("25"),
                            "Maximum entrypoint workflows.",
                        )
                        .with_range(1, 500),
                    ],
                    "EntrypointWorkflowReport",
                )
                .with_response_fields(entrypoint_workflow_response_fields())
                .with_example("/api/entrypoint-workflows?path=.&entrypoint_kind=route&depth=4"),
                api_get(
                    "/api/insights",
                    "List investigation insights with severity, kind, and search filters.",
                    insight_params(),
                    "InsightReport",
                )
                .with_response_fields(insight_report_response_fields())
                .with_example("/api/insights?path=.&severity=warning&limit=20"),
                api_get(
                    "/api/check",
                    "Run a quality gate over insights.",
                    check_params(),
                    "CheckReport",
                )
                .with_response_fields(check_report_response_fields())
                .with_example("/api/check?path=.&fail_on=error"),
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
                .with_response_fields(trace_result_response_fields())
                .with_example("/api/trace?path=.&label=main&depth=2"),
                api_get(
                    "/api/workflow",
                    "Convert an outgoing trace from a node id or label into block-style workflow steps.",
                    vec![
                        path_param(),
                        query_param(
                            "label",
                            false,
                            "string",
                            None,
                            "Start node or entrypoint label.",
                        ),
                        query_param("node_id", false, "u64", None, "Start node id."),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("4"),
                            "Maximum workflow traversal depth.",
                        )
                        .with_range(1, 32),
                        query_param(
                            "block_limit",
                            false,
                            "usize",
                            Some("200"),
                            "Maximum returned workflow blocks.",
                        )
                        .with_range(1, 1_000),
                        query_param(
                            "edge_kind",
                            false,
                            "graph_edge_kind",
                            None,
                            "Restrict workflow traversal to matching edge kinds.",
                        ),
                        query_param(
                            "confidence",
                            false,
                            "graph_confidence",
                            None,
                            "Restrict workflow traversal to matching edge confidence.",
                        ),
                        query_param(
                            "language",
                            false,
                            "string",
                            None,
                            "Restrict returned workflow blocks to matching node language metadata.",
                        ),
                        query_param(
                            "risk_severity",
                            false,
                            "insight_severity",
                            None,
                            "Restrict returned workflow blocks and transitions to matching risk severity.",
                        ),
                        query_param(
                            "block_kind",
                            false,
                            "workflow_block_kind",
                            None,
                            "Restrict returned workflow blocks to matching workflow block kinds.",
                        ),
                        query_param(
                            "compact",
                            false,
                            "bool",
                            Some("false"),
                            "Collapse repeated low-signal workflow blocks into aggregate blocks.",
                        ),
                        query_param(
                            "max_fanout",
                            false,
                            "usize",
                            None,
                            "Cap outgoing edges expanded per node (calls first) so the block budget follows the call chain into depth instead of one wide node. Try 8.",
                        ),
                    ],
                    "WorkflowReport?",
                )
                .with_response_fields(workflow_response_fields())
                .with_example("/api/workflow?path=.&label=main&depth=4&max_fanout=8"),
                api_get(
                    "/api/workflow-query",
                    "Convert graph query result nodes into block-style workflow reports.",
                    vec![
                        path_param(),
                        query_param(
                            "q",
                            true,
                            "graph_query",
                            None,
                            "Graph query expression whose returned nodes become workflow starts.",
                        )
                        .with_max_length(MAX_GRAPH_QUERY_LENGTH),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("4"),
                            "Maximum workflow traversal depth.",
                        )
                        .with_range(1, 32),
                        query_param(
                            "block_limit",
                            false,
                            "usize",
                            Some("200"),
                            "Maximum returned workflow blocks per query node.",
                        )
                        .with_range(1, 1_000),
                        query_param(
                            "max_fanout",
                            false,
                            "usize",
                            None,
                            "Cap outgoing edges expanded per node (calls first) so the block budget follows the call chain into depth. Unset means unbounded.",
                        )
                        .with_range(1, 200),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("25"),
                            "Maximum query-node workflows.",
                        )
                        .with_range(1, 500),
                        query_param(
                            "edge_kind",
                            false,
                            "graph_edge_kind",
                            None,
                            "Restrict workflow traversal to matching edge kinds.",
                        ),
                        query_param(
                            "confidence",
                            false,
                            "graph_confidence",
                            None,
                            "Restrict workflow traversal to matching edge confidence.",
                        ),
                        query_param(
                            "language",
                            false,
                            "string",
                            None,
                            "Restrict returned workflow blocks to matching node language metadata.",
                        ),
                        query_param(
                            "risk_severity",
                            false,
                            "insight_severity",
                            None,
                            "Restrict returned workflow blocks and transitions to matching risk severity.",
                        ),
                        query_param(
                            "block_kind",
                            false,
                            "workflow_block_kind",
                            None,
                            "Restrict returned workflow blocks to matching workflow block kinds.",
                        ),
                        query_param(
                            "compact",
                            false,
                            "bool",
                            Some("false"),
                            "Collapse repeated low-signal workflow blocks into aggregate blocks.",
                        ),
                    ],
                    "WorkflowQueryReport",
                )
                .with_response_fields(workflow_query_response_fields())
                .with_example("/api/workflow-query?path=.&q=entrypoints language:rust&depth=4"),
                api_get(
                    "/api/journey",
                    "Expand the shortest entrypoint-to-target path into a step-numbered execution journey built from workflow blocks.",
                    vec![
                        path_param(),
                        query_param(
                            "from",
                            true,
                            "string",
                            None,
                            "Journey start label or node id: a label such as main, the durable cg-* id the scan stamps, or n12.",
                        ),
                        query_param(
                            "to",
                            true,
                            "string",
                            None,
                            "Journey target label or node id: a label such as load_config, the durable cg-* id the scan stamps, or n42.",
                        ),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("8"),
                            "Maximum path search depth between the endpoints.",
                        )
                        .with_range(1, 32),
                        query_param(
                            "paths",
                            false,
                            "usize",
                            Some("3"),
                            "Maximum ranked alternative paths to return; alternatives avoid edges used by better-ranked paths.",
                        )
                        .with_range(1, 10),
                    ],
                    "JourneyReport",
                )
                .with_response_fields(journey_response_fields())
                .with_example("/api/journey?path=.&from=main&to=scan_project&depth=8&paths=3"),
                api_post(
                    "/api/mcp",
                    "HTTP MCP transport: handle one MCP JSON-RPC 2.0 message (initialize, ping, tools/list, tools/call) against the scanned graph; protected by the same optional bearer token as every /api/ route. Notifications return 202 with no body.",
                    vec![path_param()],
                    Some("McpJsonRpcMessage"),
                    "McpJsonRpcResponse",
                    false,
                )
                .with_body_fields(vec![
                    body_field(
                        "jsonrpc",
                        true,
                        "string",
                        Some("2.0"),
                        "JSON-RPC protocol version; must be `2.0`.",
                    ),
                    body_field(
                        "id",
                        false,
                        "number|string",
                        None,
                        "Request id. Messages without an id are notifications and receive 202 with no body.",
                    ),
                    body_field(
                        "method",
                        true,
                        "string",
                        None,
                        "MCP method: initialize, ping, tools/list, or tools/call.",
                    ),
                    body_field(
                        "params",
                        false,
                        "object",
                        None,
                        "Method parameters; for tools/call: `name` plus tool `arguments` matching the tools/list input schemas.",
                    ),
                ])
                .with_response_fields(vec![
                    response_field(
                        "jsonrpc",
                        true,
                        "string",
                        "JSON-RPC protocol version, always `2.0`.",
                    ),
                    response_field("id", true, "number|string|null", "Echoed request id."),
                    response_field(
                        "result",
                        false,
                        "object",
                        "Successful method result; tool payloads arrive as MCP text content with `isError`.",
                    ),
                    response_field(
                        "error",
                        false,
                        "object",
                        "JSON-RPC error with `code` and `message` for parse errors, unsupported methods, and batch requests.",
                    ),
                ])
                .with_example("POST /api/mcp {\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}"),
                api_get(
                    "/api/component-dependencies",
                    "Group a node's incoming/outgoing dependencies by architecture area, package, and language.",
                    vec![
                        path_param(),
                        query_param(
                            "target",
                            true,
                            "string",
                            None,
                            "Component target label or node id: a label such as load_config, the durable cg-* id the scan stamps, or n42.",
                        ),
                        query_param(
                            "group_limit",
                            false,
                            "usize",
                            Some("25"),
                            "Maximum groups per facet (areas, packages, languages).",
                        )
                        .with_range(1, 100),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("10"),
                            "Maximum sample edge indexes per group.",
                        )
                        .with_range(1, 50),
                    ],
                    "ComponentDependencyReport",
                )
                .with_response_fields(component_dependency_response_fields())
                .with_example("/api/component-dependencies?path=.&target=scan_project"),
                api_get(
                    "/api/component-contract",
                    "List the exact dependency edges between two architecture areas with confidence and related risks.",
                    vec![
                        path_param(),
                        query_param(
                            "source",
                            true,
                            "string",
                            None,
                            "Source architecture area name or unambiguous fragment.",
                        ),
                        query_param(
                            "target",
                            true,
                            "string",
                            None,
                            "Target architecture area name or unambiguous fragment.",
                        ),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum listed contract edges.",
                        )
                        .with_range(1, 500),
                    ],
                    "ComponentContractReport",
                )
                .with_response_fields(component_contract_response_fields())
                .with_example("/api/component-contract?path=.&source=docs&target=crates/codegraph-analysis"),
                api_get(
                    "/api/impact",
                    "Report the blast radius of changing a node: dependents, affected entrypoints/routes/tests, and a risk-weighted impact score.",
                    vec![
                        path_param(),
                        query_param(
                            "target",
                            true,
                            "string",
                            None,
                            "Impact target label or node id: a label such as load_config, the durable cg-* id the scan stamps, or n42.",
                        ),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("6"),
                            "Maximum reverse dependency depth.",
                        )
                        .with_range(1, 32),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("40"),
                            "Maximum listed dependents.",
                        )
                        .with_range(1, 1_000),
                    ],
                    "ImpactReport",
                )
                .with_response_fields(impact_response_fields())
                .with_example("/api/impact?path=.&target=scan_project&depth=6"),
                api_get(
                    "/api/seams",
                    "Rank cross-area boundaries by coupling friction: safest seams to extract and most tangled boundaries needing work.",
                    vec![
                        path_param(),
                        query_param(
                            "limit",
                            false,
                            "usize",
                            Some("25"),
                            "Maximum ranked boundaries per list.",
                        )
                        .with_range(1, 100),
                        query_param(
                            "edge_limit",
                            false,
                            "usize",
                            Some("10"),
                            "Maximum sample edge indexes per boundary.",
                        )
                        .with_range(1, 50),
                    ],
                    "SeamReport",
                )
                .with_response_fields(seam_response_fields())
                .with_example("/api/seams?path=.&limit=25"),
                api_get(
                    "/api/pr-impact",
                    "PR impact dashboard: map changed files onto graph communities, hotspots, blast radius, and risky findings, with optional CI/review context.",
                    vec![
                        path_param(),
                        query_param(
                            "base",
                            false,
                            "string",
                            Some("HEAD"),
                            "Git base ref diffed against the working tree for the changed-file list; ignored when `files` is set.",
                        ),
                        query_param(
                            "files",
                            false,
                            "string",
                            None,
                            "Comma-separated explicit changed files; skips git entirely.",
                        ),
                        query_param(
                            "ci_state",
                            false,
                            "string",
                            None,
                            "CI state stamped into the report, for example passing or failing.",
                        ),
                        query_param(
                            "review_state",
                            false,
                            "string",
                            None,
                            "Review state stamped into the report, for example approved.",
                        ),
                    ],
                    "PrImpactReport",
                )
                .with_example("/api/pr-impact?path=.&base=origin/main&ci_state=passing"),
                api_get(
                    "/api/refactor-context",
                    "Emit a one-shot refactor context bundle: blast-radius impact, component dependencies, optional entrypoint journey, related risks, and a target source preview.",
                    vec![
                        path_param(),
                        query_param(
                            "target",
                            true,
                            "string",
                            None,
                            "Refactor target label or node id: a label such as load_config, the durable cg-* id the scan stamps, or n42.",
                        ),
                        query_param(
                            "from",
                            false,
                            "string",
                            None,
                            "Optional journey start label or node id such as an entrypoint.",
                        ),
                        query_param(
                            "depth",
                            false,
                            "usize",
                            Some("8"),
                            "Maximum traversal depth for impact and journey.",
                        )
                        .with_range(1, 32),
                        query_param(
                            "paths",
                            false,
                            "usize",
                            Some("3"),
                            "Maximum ranked journey paths when from is provided.",
                        )
                        .with_range(1, 10),
                        query_param(
                            "dependent_limit",
                            false,
                            "usize",
                            Some("100"),
                            "Maximum listed impact dependents.",
                        )
                        .with_range(1, 1_000),
                        query_param(
                            "risk_limit",
                            false,
                            "usize",
                            Some("50"),
                            "Maximum bundled risks touching the target or its dependents.",
                        )
                        .with_range(1, 200),
                        query_param(
                            "source_context",
                            false,
                            "u32",
                            Some("6"),
                            "Source preview context lines around the target span.",
                        )
                        .with_range(0, 50),
                    ],
                    "RefactorContextBundle",
                )
                .with_response_fields(refactor_context_response_fields())
                .with_example("/api/refactor-context?path=.&target=scan_project&from=main&depth=8"),
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
                .with_response_fields(trace_result_response_fields())
                .with_example("/api/dependents?path=.&label=scan_project&depth=3"),
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
                .with_response_fields(config_trace_response_fields())
                .with_example("/api/trace-config?path=.&target=DATABASE_URL&depth=6"),
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
                .with_response_fields(error_trace_response_fields())
                .with_example("/api/trace-errors?path=.&target=failed to load&depth=6"),
            ],
        },
        ApiSchemaGroup {
            group: "source",
            endpoints: vec![
                api_get(
                    "/api/source",
                    "Read a source snippet by project root, source file, and line span.",
                    vec![
                        path_param(),
                        query_param(
                            "file",
                            true,
                            "path",
                            None,
                            "Source file inside the project root.",
                        ),
                        query_param("start_line", false, "u32", None, "First line."),
                        query_param("end_line", false, "u32", None, "Last line."),
                        query_param("context", false, "u32", None, "Context lines around span.")
                            .with_range(0, MAX_SOURCE_CONTEXT as usize)
                            .with_capability_limit("max_source_context"),
                    ],
                    "SourceResponse",
                )
                .with_response_fields(source_preview_response_fields())
                .with_example("/api/source?path=.&file=crates/codegraph-cli/src/main.rs&start_line=10&end_line=40"),
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
                .with_response_fields(source_search_response_fields())
                .with_example("/api/source-search?path=.&q=failed to read&limit=10"),
            ],
        },
    ]
}

pub(crate) fn graph_slice_params() -> Vec<ApiParameterSpec> {
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

pub(crate) fn report_params() -> Vec<ApiParameterSpec> {
    vec![
        path_param(),
        query_param(
            "format",
            false,
            "report_format",
            Some("json"),
            "Report response format.",
        ),
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
            "community_limit",
            false,
            "usize",
            Some("25"),
            "Maximum graph communities, capped by server capabilities.",
        )
        .with_range(1, MAX_REPORT_COMMUNITY_LIMIT)
        .with_capability_limit("max_report_community_limit"),
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
            "file_summary_limit",
            false,
            "usize",
            Some("25"),
            "Maximum compact file summaries, capped by server capabilities.",
        )
        .with_range(1, MAX_REPORT_FILE_SUMMARY_LIMIT)
        .with_capability_limit("max_report_file_summary_limit"),
        query_param(
            "node_summary_limit",
            false,
            "usize",
            Some("25"),
            "Maximum compact node summaries, capped by server capabilities.",
        )
        .with_range(1, MAX_REPORT_NODE_SUMMARY_LIMIT)
        .with_capability_limit("max_report_node_summary_limit"),
        query_param(
            "fail_on",
            false,
            "insight_severity",
            Some("error"),
            "Quality gate threshold.",
        ),
    ]
}

pub(crate) fn insight_params() -> Vec<ApiParameterSpec> {
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

pub(crate) fn check_params() -> Vec<ApiParameterSpec> {
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

pub(crate) fn semantic_filter_params() -> Vec<ApiParameterSpec> {
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

pub(crate) fn scan_job_body_fields() -> Vec<ApiParameterSpec> {
    vec![body_field(
        "path",
        false,
        "path",
        Some("."),
        "Project root path.",
    )]
}

pub(crate) fn semantic_filter_body_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn semantic_patch_body_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn semantic_enrich_body_fields() -> Vec<ApiParameterSpec> {
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

/// The scan policy in force for a project root: the same knobs
/// `/api/coverage` reports having applied.
pub(crate) fn scan_options_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("root", true, "path", "Resolved project root."),
        response_field(
            "config_path",
            false,
            "path",
            "Repository scan policy that was applied, when one exists.",
        ),
        response_field(
            "include_hidden",
            true,
            "bool",
            "Whether hidden entries are walked.",
        ),
        response_field(
            "include_ignored",
            true,
            "bool",
            "Whether default ignored directories are walked.",
        ),
        response_field(
            "max_file_size",
            true,
            "u64",
            "Byte cap above which a file is recorded but not read.",
        ),
        response_field(
            "ignored_names",
            true,
            "string[]",
            "Directory names skipped by default.",
        ),
        response_field(
            "ignored_globs",
            true,
            "string[]",
            "Globs skipped by repository policy.",
        ),
    ]
}

/// What `/api/summary` answers with. Documented here because an agent
/// reads the schema before the response.
pub(crate) fn graph_summary_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("nodes", true, "usize", "Total nodes in the graph."),
        response_field("edges", true, "usize", "Total edges in the graph."),
        response_field(
            "entrypoints",
            true,
            "usize",
            "Nodes a manifest, script, or runtime surface starts from.",
        ),
        response_field(
            "skipped_files",
            true,
            "usize",
            "Files left unread, such as those above the size cap.",
        ),
        response_field(
            "node_kinds",
            true,
            "map<string,usize>",
            "Node count per kind.",
        ),
        response_field(
            "edge_kinds",
            true,
            "map<string,usize>",
            "Edge count per kind.",
        ),
        response_field(
            "edge_confidences",
            true,
            "map<string,usize>",
            "Edge count per confidence level.",
        ),
        response_field(
            "edge_relations",
            true,
            "map<string,usize>",
            "Edge count per `relation` metadata value.",
        ),
        response_field(
            "edge_sources",
            true,
            "map<string,usize>",
            "Edge count per `source` metadata value.",
        ),
        response_field(
            "languages",
            true,
            "map<string,usize>",
            "Node count per detected language.",
        ),
        response_field(
            "annotation_facets",
            true,
            "map<string,map<string,usize>>",
            "Counts per annotation facet declared in `.codegraph/config.toml`.",
        ),
    ]
}

/// What `/api/coverage` answers with: which files the scan read, and what
/// it left out.
pub(crate) fn scan_coverage_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("root", true, "path", "Scanned project root."),
        response_field(
            "include_hidden",
            true,
            "bool",
            "Whether hidden entries were walked.",
        ),
        response_field(
            "include_ignored",
            true,
            "bool",
            "Whether default ignored directories were walked.",
        ),
        response_field(
            "max_file_size",
            true,
            "u64",
            "Byte cap above which a file is recorded but not read.",
        ),
        response_field(
            "config_path",
            false,
            "path",
            "Repository scan policy that was applied, when one exists.",
        ),
        response_field(
            "ignored_names",
            true,
            "string[]",
            "Directory names skipped by default.",
        ),
        response_field(
            "ignored_globs",
            true,
            "string[]",
            "Globs skipped by repository policy.",
        ),
        response_field(
            "directories_seen",
            true,
            "usize",
            "Directories the walk entered.",
        ),
        response_field("files_seen", true, "usize", "Files the walk met."),
        response_field("indexed_files", true, "usize", "Files read into the graph."),
        response_field(
            "skipped_large_files",
            true,
            "usize",
            "Files above `max_file_size`.",
        ),
        response_field(
            "skipped_policy_entries",
            true,
            "usize",
            "Entries skipped by repository policy.",
        ),
        response_field(
            "skipped_hidden_entries",
            true,
            "usize",
            "Hidden entries skipped.",
        ),
        response_field(
            "skipped_ignored_name_entries",
            true,
            "usize",
            "Entries skipped by ignored directory name.",
        ),
        response_field(
            "skipped_ignored_glob_entries",
            true,
            "usize",
            "Entries skipped by ignored glob.",
        ),
        response_field(
            "non_index_files",
            true,
            "usize",
            "Files the indexer has no adapter or rule for.",
        ),
        response_field(
            "non_index_extensions",
            true,
            "object",
            "How many of those files each extension accounts for, so the assets can be told from a language this scan does not read.",
        ),
        response_field("seen_bytes", true, "u64", "Bytes across every file met."),
        response_field("indexed_bytes", true, "u64", "Bytes actually read."),
        response_field(
            "skipped_large_bytes",
            true,
            "u64",
            "Bytes left unread above the size cap.",
        ),
        response_field(
            "languages",
            true,
            "map<string,usize>",
            "Indexed file count per language.",
        ),
    ]
}

pub(crate) fn capabilities_response_fields() -> Vec<ApiParameterSpec> {
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
        response_field(
            "export_formats",
            true,
            "string[]",
            "Graph export formats `/api/export` accepts.",
        ),
        response_field(
            "scan",
            true,
            "ScanCapabilityResponse",
            "Scan policy in force: file size cap, ignore rules, and job limits.",
        ),
    ]
}

pub(crate) fn api_schema_response_fields() -> Vec<ApiParameterSpec> {
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
            "Machine-readable endpoint groups; investigation endpoints carry a copy-paste-ready `example` request.",
        ),
        response_field(
            "enum_values",
            true,
            "map<string,string[]>",
            "Known enum values and query terms for clients.",
        ),
    ]
}

pub(crate) fn probe_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn health_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn metrics_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn graph_slice_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn node_context_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn node_card_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "notes",
            false,
            "string[]",
            "What the answer had to decide: a label several definitions answer to is picked once, and this says which.",
        ),
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
            "insights_evaluated",
            true,
            "bool",
            "Whether repository-wide insights were evaluated for this card.",
        ),
        response_field(
            "actions",
            true,
            "NodeCardAction[]",
            "Suggested focused graph actions for investigation handoff.",
        ),
    ]
}

pub(crate) fn query_result_response_fields() -> Vec<ApiParameterSpec> {
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
            "compact",
            true,
            "bool",
            "Whether repeated low-signal nodes were compacted.",
        ),
        response_field(
            "raw_total_nodes",
            true,
            "usize",
            "Node count before optional result compaction.",
        ),
        response_field(
            "raw_total_edges",
            true,
            "usize",
            "Edge count before optional result compaction.",
        ),
        response_field(
            "compacted_nodes",
            true,
            "usize",
            "Number of source nodes collapsed into compact aggregate nodes.",
        ),
        response_field(
            "compacted_edges",
            true,
            "usize",
            "Number of source edges collapsed or deduplicated during compaction.",
        ),
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

pub(crate) fn natural_query_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "schema",
            true,
            "string",
            "Report schema id, currently codegraph.ask.v1.",
        ),
        response_field(
            "question",
            true,
            "string",
            "Original natural-language question.",
        ),
        response_field(
            "generated_query",
            true,
            "string",
            "Deterministic graph query generated from the question.",
        ),
        response_field(
            "cli_snippet",
            true,
            "string",
            "Copy-paste-ready CLI equivalent of the generated query, for example `codegraph query 'configs target:DATABASE_URL depth:6' .`.",
        ),
        response_field(
            "rule",
            true,
            "string",
            "Rule name that selected the generated query.",
        ),
        response_field(
            "confidence",
            true,
            "string",
            "Heuristic confidence for the mapping.",
        ),
        response_field(
            "result",
            true,
            "QueryResult",
            "Result of running the generated graph query.",
        ),
        response_field(
            "alternatives",
            true,
            "string[]",
            "Other deterministic query expressions worth trying.",
        ),
    ]
}

pub(crate) fn edge_explanation_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn project_report_response_fields() -> Vec<ApiParameterSpec> {
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
            "Production project report with summary, compact node/file summaries, surprising links, risks, quality gate, topology, and hotspots.",
        ),
    ]
}

pub(crate) fn architecture_map_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn language_dependency_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn surprising_link_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "links",
            true,
            "SurprisingLink[]",
            "Ranked dependency links with source/target nodes, reasons, score, and edge_index evidence.",
        ),
        response_field(
            "total_candidates",
            true,
            "usize",
            "Total surprising link candidates before limiting.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more surprising links exist beyond the limit.",
        ),
    ]
}

pub(crate) fn hotspot_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "hotspots",
            true,
            "Hotspot[]",
            "High-degree files, functions, entrypoints, and config nodes. Degree counts the dependency edges a node takes part in -- calls, imports, references, reads -- and not the containment that holds a file's symbols.",
        ),
        response_field(
            "architectural_hubs",
            true,
            "Hotspot[]",
            "High-degree hotspots after filtering out common utility-style hubs.",
        ),
        response_field(
            "utility_hubs",
            true,
            "Hotspot[]",
            "High-degree hotspots likely caused by generic helper names or unresolved utility calls.",
        ),
        response_field(
            "total_candidates",
            true,
            "usize",
            "Total hotspot candidates before limiting.",
        ),
        response_field(
            "total_architectural_hubs",
            true,
            "usize",
            "Total architectural hotspot candidates before limiting.",
        ),
        response_field(
            "total_utility_hubs",
            true,
            "usize",
            "Total utility hotspot candidates before limiting.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more hotspots exist beyond the limit.",
        ),
    ]
}

pub(crate) fn community_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "communities",
            true,
            "GraphCommunity[]",
            "Deterministic graph communities/subsystems with sample nodes and the share of each one's edges that stay inside it.",
        ),
        response_field(
            "total_communities",
            true,
            "usize",
            "Total communities before limiting.",
        ),
        response_field(
            "total_nodes",
            true,
            "usize",
            "Total nodes represented by communities.",
        ),
        response_field(
            "total_internal_edges",
            true,
            "usize",
            "Total internal community edges.",
        ),
        response_field(
            "total_external_edges",
            true,
            "usize",
            "Total incoming plus outgoing external community edges.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more communities exist beyond the limit.",
        ),
    ]
}

pub(crate) fn entrypoint_trace_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn entrypoint_workflow_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "max_depth",
            true,
            "usize",
            "Applied workflow traversal depth.",
        ),
        response_field(
            "block_limit",
            true,
            "usize",
            "Applied maximum block count per entrypoint.",
        ),
        response_field(
            "entrypoint_kind",
            false,
            "string",
            "Applied entrypoint kind filter such as route, workflow_job, pipeline_job, make_target, service, or cmd.",
        ),
        response_field(
            "filters",
            true,
            "WorkflowFilters",
            "Applied workflow block and traversal filters.",
        ),
        response_field(
            "total_entrypoints",
            true,
            "usize",
            "Total matched entrypoints before limiting.",
        ),
        response_field(
            "workflows",
            true,
            "WorkflowReport[]",
            "Block-style workflow reports from matched entrypoints.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more entrypoint workflows exist or any workflow was truncated.",
        ),
    ]
}

pub(crate) fn trace_result_response_fields() -> Vec<ApiParameterSpec> {
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
        response_field(
            "notes",
            false,
            "string[]",
            "What the answer is about when the request left it open, such as which definition a shared name was taken to mean. Absent when the request named one thing.",
        ),
    ]
}

pub(crate) fn workflow_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("start", true, "Node", "Workflow start node."),
        response_field(
            "max_depth",
            true,
            "usize",
            "Applied workflow traversal depth.",
        ),
        response_field(
            "block_limit",
            true,
            "usize",
            "Applied maximum returned block count.",
        ),
        response_field(
            "filters",
            true,
            "WorkflowFilters",
            "Applied workflow block and traversal filters.",
        ),
        response_field(
            "compact",
            true,
            "bool",
            "Whether repeated low-signal workflow blocks were compacted.",
        ),
        response_field(
            "blocks",
            true,
            "WorkflowBlock[]",
            "Block-style execution steps with node ids, kinds, depth, source node ids, and risk references.",
        ),
        response_field(
            "transitions",
            true,
            "WorkflowTransition[]",
            "Directed transitions between workflow blocks with edge indexes and confidence metadata.",
        ),
        response_field(
            "total_blocks",
            true,
            "usize",
            "Returned workflow block count.",
        ),
        response_field(
            "total_transitions",
            true,
            "usize",
            "Returned workflow transition count.",
        ),
        response_field(
            "raw_total_blocks",
            true,
            "usize",
            "Workflow block count before optional compaction.",
        ),
        response_field(
            "raw_total_transitions",
            true,
            "usize",
            "Workflow transition count before optional compaction.",
        ),
        response_field(
            "notes",
            false,
            "string[]",
            "What the answer is about when the request left it open, such as which definition a shared name was taken to mean. Absent when the request named one thing.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether traversal depth or block limits omitted additional steps.",
        ),
    ]
}

pub(crate) fn refactor_context_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "schema",
            true,
            "string",
            "Bundle schema id, currently codegraph.refactor_context.v1.",
        ),
        response_field(
            "notes",
            false,
            "string[]",
            "What the answer had to decide: a label several definitions answer to is picked by rank, and this says which.",
        ),
        response_field("target", true, "Node", "Resolved refactor target node."),
        response_field(
            "area",
            false,
            "string",
            "Architecture area containing the target node.",
        ),
        response_field(
            "impact",
            true,
            "ImpactReport",
            "Blast-radius report for the target.",
        ),
        response_field(
            "dependencies",
            true,
            "ComponentDependencyReport",
            "Component dependency groups for the target.",
        ),
        response_field(
            "journey",
            false,
            "JourneyReport",
            "Ranked entrypoint-to-target journey when from is provided.",
        ),
        response_field(
            "total_risks",
            true,
            "usize",
            "Total insights touching the target or its dependents.",
        ),
        response_field(
            "risks",
            true,
            "Insight[]",
            "Bundled risks capped by risk_limit.",
        ),
        response_field(
            "risks_truncated",
            true,
            "bool",
            "Whether more risks exist than risk_limit.",
        ),
        response_field(
            "target_source",
            false,
            "SourcePreview",
            "Source preview around the target span when available.",
        ),
    ]
}

pub(crate) fn seam_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "total_pairs",
            true,
            "usize",
            "Total directed cross-area boundary pairs with dependency edges.",
        ),
        response_field(
            "safest",
            true,
            "SeamCandidate[]",
            "Boundaries ranked by ascending friction: thin, well-declared seams where extraction is safest.",
        ),
        response_field(
            "most_needed",
            true,
            "SeamCandidate[]",
            "Boundaries ranked by descending friction: tangled seams where splitting is most needed. friction_score = edges + 2*low-confidence edges + 3*edge risks + distinct edge kinds.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more boundary pairs exist than limit.",
        ),
    ]
}

pub(crate) fn impact_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "schema",
            true,
            "string",
            "Report schema id, currently codegraph.impact.v1.",
        ),
        response_field(
            "suggested_commands",
            true,
            "string[]",
            "Copy-paste-ready CLI follow-ups: inspect the target node card, then read or bundle the path from the nearest affected entrypoint.",
        ),
        response_field("target", true, "Node", "Resolved impact target node."),
        response_field(
            "max_depth",
            true,
            "usize",
            "How far the dependent walk was allowed to go.",
        ),
        response_field(
            "area",
            false,
            "string",
            "Architecture area containing the target node.",
        ),
        response_field(
            "total_dependents",
            true,
            "usize",
            "Total transitive dependents within the depth bound (excluding containment).",
        ),
        response_field(
            "affected_entrypoints",
            true,
            "ImpactEntrypoint[]",
            "Entrypoint dependents with entrypoint kind and reverse distance.",
        ),
        response_field(
            "affected_routes",
            true,
            "usize",
            "Affected entrypoints whose kind is route.",
        ),
        response_field(
            "affected_tests",
            true,
            "usize",
            "Dependents located in test-like source paths.",
        ),
        response_field(
            "languages",
            true,
            "map<string,usize>",
            "Dependent counts by language metadata.",
        ),
        response_field(
            "areas",
            true,
            "map<string,usize>",
            "Dependent counts by architecture area.",
        ),
        response_field(
            "severity_counts",
            true,
            "map<string,usize>",
            "Risk severity counts across dependents.",
        ),
        response_field(
            "risks_evaluated",
            true,
            "bool",
            "Whether repository-wide risks were evaluated for this report.",
        ),
        response_field(
            "impact_score",
            true,
            "usize",
            "Risk-weighted score: program dependents (total minus affected_tests) + 5 per program entrypoint + 5/2/1 per error/warning/info risk. Reaching a test is coverage, not risk, and a library declares most of its entrypoints inside its tests, so neither half of the suite is scored.",
        ),
        response_field(
            "dependents",
            true,
            "ImpactDependent[]",
            "Dependents sorted by reverse distance with test flags and risk counts, capped by limit.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether traversal hit the depth bound or the dependent list was capped.",
        ),
    ]
}

pub(crate) fn component_dependency_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("target", true, "Node", "Resolved component target node."),
        response_field(
            "area",
            false,
            "string",
            "Architecture area containing the target node.",
        ),
        response_field(
            "total_incoming",
            true,
            "usize",
            "Total non-containment incoming edges.",
        ),
        response_field(
            "total_outgoing",
            true,
            "usize",
            "Total non-containment outgoing edges.",
        ),
        response_field(
            "areas",
            true,
            "ComponentDependencyGroup[]",
            "Dependency groups keyed by architecture area with incoming/outgoing counts, edge kinds, confidence counts, and sample edge indexes.",
        ),
        response_field(
            "packages",
            true,
            "ComponentDependencyGroup[]",
            "Dependency groups keyed by canonical package id.",
        ),
        response_field(
            "languages",
            true,
            "ComponentDependencyGroup[]",
            "Dependency groups keyed by neighbor language metadata.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether any facet had more groups than group_limit.",
        ),
    ]
}

pub(crate) fn component_contract_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "source_area",
            true,
            "string",
            "Resolved source architecture area.",
        ),
        response_field(
            "target_area",
            true,
            "string",
            "Resolved target architecture area.",
        ),
        response_field(
            "total_edges",
            true,
            "usize",
            "Total directed dependency edges from source to target area.",
        ),
        response_field(
            "edge_kinds",
            true,
            "map<string,usize>",
            "Edge kind counts across the contract.",
        ),
        response_field(
            "confidence_counts",
            true,
            "map<string,usize>",
            "Confidence counts across the contract.",
        ),
        response_field(
            "edges",
            true,
            "ComponentContractEdge[]",
            "Exact contract edges with stable edge_index, endpoint labels, and related risk counts.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether more contract edges exist than edge_limit.",
        ),
    ]
}

pub(crate) fn journey_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field(
            "schema",
            true,
            "string",
            "Report schema id, currently codegraph.journey.v1.",
        ),
        response_field(
            "suggested_commands",
            true,
            "string[]",
            "Copy-paste-ready CLI follow-ups: impact, refactor-context, and node-card commands for the journey target.",
        ),
        response_field("from", true, "Node", "Resolved journey start node."),
        response_field("to", true, "Node", "Resolved journey target node."),
        response_field(
            "max_depth",
            true,
            "usize",
            "Applied maximum path search depth.",
        ),
        response_field(
            "total_paths",
            true,
            "usize",
            "Returned journey path count; 0 when no directed path exists.",
        ),
        response_field(
            "paths",
            true,
            "JourneyPath[]",
            "Execution chains ranked by edge confidence then length; each path carries rank, confidence_score, lowest_confidence, a risk_summary (risky steps/transitions, fragile transitions, low-confidence hops, unresolved/ambiguous calls, duplicate labels, cycle back edges, severity counts), and step-numbered blocks whose transitions include edge provenance, per-hop explanations, fragile flags with reasons, and risk references.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether the path search hit the depth bound before finding a path.",
        ),
        response_field(
            "notes",
            false,
            "string[]",
            "What the answer is about when the request left it open, such as which definition a shared name was taken to mean. Absent when the request named one thing.",
        ),
    ]
}

pub(crate) fn workflow_query_response_fields() -> Vec<ApiParameterSpec> {
    vec![
        response_field("query", true, "string", "Graph query expression."),
        response_field(
            "max_depth",
            true,
            "usize",
            "Applied workflow traversal depth.",
        ),
        response_field(
            "block_limit",
            true,
            "usize",
            "Applied maximum returned block count per query node.",
        ),
        response_field(
            "filters",
            true,
            "WorkflowFilters",
            "Applied workflow block and traversal filters.",
        ),
        response_field(
            "total_query_nodes",
            true,
            "usize",
            "Total query result nodes before workflow limiting.",
        ),
        response_field(
            "total_query_edges",
            true,
            "usize",
            "Total query result edges.",
        ),
        response_field(
            "total_candidates",
            true,
            "usize",
            "Returned query nodes considered as workflow starts.",
        ),
        response_field(
            "workflows",
            true,
            "WorkflowReport[]",
            "Block-style workflow reports from query result nodes.",
        ),
        response_field(
            "truncated",
            true,
            "bool",
            "Whether the query, workflow count, or any workflow was truncated.",
        ),
    ]
}

pub(crate) fn config_trace_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn error_trace_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn insight_report_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn check_report_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn source_preview_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn source_search_response_fields() -> Vec<ApiParameterSpec> {
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

pub(crate) fn api_get(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    response: &'static str,
) -> ApiEndpointSpec {
    api_endpoint("GET", path, summary, parameters, None, response, false)
}

pub(crate) fn api_get_stream(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    response: &'static str,
) -> ApiEndpointSpec {
    api_endpoint("GET", path, summary, parameters, None, response, true)
}

pub(crate) fn api_post(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    body: Option<&'static str>,
    response: &'static str,
    streaming: bool,
) -> ApiEndpointSpec {
    api_endpoint("POST", path, summary, parameters, body, response, streaming)
}

pub(crate) fn api_delete(
    path: &'static str,
    summary: &'static str,
    parameters: Vec<ApiParameterSpec>,
    response: &'static str,
) -> ApiEndpointSpec {
    api_endpoint("DELETE", path, summary, parameters, None, response, false)
}

pub(crate) fn api_endpoint(
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
        example: None,
    }
}

pub(crate) fn path_param() -> ApiParameterSpec {
    query_param("path", false, "path", Some("."), "Project root path.")
}

pub(crate) fn id_param() -> ApiParameterSpec {
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

pub(crate) fn job_status_param() -> ApiParameterSpec {
    query_param("status", false, "job_status", None, "Filter by job status.")
}

pub(crate) fn body_field(
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

pub(crate) fn response_field(
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

pub(crate) fn query_param(
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

pub(crate) fn capability_features(
    cache_enabled: bool,
    access_log_enabled: bool,
) -> Vec<&'static str> {
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
        "natural_language_queries",
        "entrypoint_traces",
        "entrypoint_workflows",
        "workflow_filters",
        "workflow_query",
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

pub(crate) fn capability_endpoints() -> Vec<EndpointGroupResponse> {
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
                "GET /api/ask",
                "GET /api/explain-edge",
                "GET /api/workflow",
                "GET /api/workflow-query",
            ],
        },
        EndpointGroupResponse {
            group: "analysis",
            endpoints: vec![
                "GET /api/report",
                "GET /api/architecture",
                "GET /api/language-dependencies",
                "GET /api/surprising-links",
                "GET /api/hotspots",
                "GET /api/communities",
                "GET /api/entrypoints",
                "GET /api/entrypoint-traces",
                "GET /api/entrypoint-workflows",
                "GET /api/insights",
                "GET /api/check",
                "GET /api/trace",
                "GET /api/dependents",
                "GET /api/trace-config",
                "GET /api/trace-errors",
                "GET /api/journey",
                "GET /api/impact",
                "GET /api/refactor-context",
                "GET /api/component-dependencies",
                "GET /api/component-contract",
                "GET /api/seams",
                "GET /api/pr-impact",
                "POST /api/mcp",
            ],
        },
        EndpointGroupResponse {
            group: "source",
            endpoints: vec!["GET /api/source", "GET /api/source-search"],
        },
        EndpointGroupResponse {
            group: "export",
            // One endpoint; formats are enumerated by `export_formats`.
            endpoints: vec!["GET /api/export"],
        },
    ]
}
