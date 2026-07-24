//! HTTP request/response contracts: CLI arguments, shared application
//! state, query structs for every endpoint, and API schema spec types.

use clap::Parser;
use codegraph_analysis::{GraphSummary, ProjectReport};
use codegraph_core::CodeGraph;
use codegraph_indexer::IndexOptionOverrides;
use codegraph_lsp::{SemanticGraphApplyReport, SemanticLspCacheInfo, SemanticLspResponse};
use codegraph_storage::{CacheInfo, GraphCache};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use tokio::sync::{RwLock, Semaphore};

#[allow(unused_imports)]
use crate::*;

#[derive(Debug, Parser)]
#[command(name = "codegraph-server")]
#[command(about = "Serve the CodeGraph API and web interface")]
#[command(version)]
pub(crate) struct Args {
    /// Project root exposed to the scanner.
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,

    /// Additional local project roots that may be opened from the web UI.
    #[arg(long = "project")]
    pub(crate) projects: Vec<PathBuf>,

    /// HTTP bind host.
    #[arg(long, default_value = "127.0.0.1")]
    pub(crate) host: String,

    /// HTTP bind port.
    #[arg(long, default_value_t = 3765)]
    pub(crate) port: u16,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    /// Maximum bytes to read from any single file during scans.
    #[arg(long)]
    pub(crate) max_file_size: Option<u64>,

    /// Allow scanning paths outside the configured root.
    #[arg(long)]
    pub(crate) allow_any_path: bool,

    /// Disable persistent graph cache.
    #[arg(long)]
    pub(crate) no_cache: bool,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    pub(crate) cache_dir: Option<PathBuf>,

    /// Maximum in-memory scan jobs retained after completion.
    #[arg(long, default_value_t = DEFAULT_MAX_SCAN_JOBS)]
    pub(crate) max_scan_jobs: usize,

    /// Maximum in-memory semantic enrichment jobs retained after completion.
    #[arg(long, default_value_t = DEFAULT_MAX_SEMANTIC_JOBS)]
    pub(crate) max_semantic_jobs: usize,

    /// Maximum scan jobs allowed to run at the same time.
    #[arg(long, default_value_t = DEFAULT_MAX_SCAN_CONCURRENCY)]
    pub(crate) max_scan_concurrency: usize,

    /// Maximum semantic enrichment jobs allowed to run at the same time.
    #[arg(long, default_value_t = DEFAULT_MAX_SEMANTIC_CONCURRENCY)]
    pub(crate) max_semantic_concurrency: usize,

    /// Maximum accepted HTTP request body bytes for JSON API requests.
    #[arg(long, default_value_t = DEFAULT_API_BODY_BYTES)]
    pub(crate) max_api_body_bytes: usize,

    /// Disable per-request access logs on stderr.
    #[arg(long)]
    pub(crate) quiet_access_log: bool,

    /// Require a token for all /api/* routes. Also read from CODEGRAPH_API_TOKEN.
    #[arg(long)]
    pub(crate) api_token: Option<String>,
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) root: PathBuf,
    pub(crate) started_at: Instant,
    pub(crate) projects: Arc<Vec<ProjectRoot>>,
    pub(crate) option_overrides: IndexOptionOverrides,
    pub(crate) allow_any_path: bool,
    pub(crate) cache: Option<GraphCache>,
    pub(crate) jobs: Arc<RwLock<BTreeMap<String, ScanJob>>>,
    pub(crate) semantic_jobs: Arc<RwLock<BTreeMap<String, SemanticJob>>>,
    pub(crate) max_scan_jobs: usize,
    pub(crate) max_semantic_jobs: usize,
    pub(crate) max_scan_concurrency: usize,
    pub(crate) max_semantic_concurrency: usize,
    pub(crate) max_api_body_bytes: usize,
    pub(crate) access_log_enabled: bool,
    pub(crate) scan_permits: Arc<Semaphore>,
    pub(crate) semantic_permits: Arc<Semaphore>,
    pub(crate) next_job_id: Arc<AtomicU64>,
}

#[derive(Clone)]
pub(crate) struct ApiAuth {
    pub(crate) token: Option<Arc<str>>,
}

impl ApiAuth {
    pub(crate) fn new(token: Option<String>) -> Self {
        Self {
            token: token.map(Arc::<str>::from),
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.token.is_some()
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ScanQuery {
    pub(crate) path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SemanticPlanQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) work_item_limit: Option<usize>,
    pub(crate) work_language: Option<String>,
    pub(crate) work_status: Option<String>,
    pub(crate) work_capability: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SemanticPatchRequest {
    pub(crate) path: Option<PathBuf>,
    pub(crate) work_item_limit: Option<usize>,
    pub(crate) work_language: Option<String>,
    pub(crate) work_status: Option<String>,
    pub(crate) work_capability: Option<String>,
    pub(crate) responses: Vec<SemanticLspResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SemanticEnrichRequest {
    pub(crate) path: Option<PathBuf>,
    pub(crate) work_item_limit: Option<usize>,
    pub(crate) work_language: Option<String>,
    pub(crate) work_status: Option<String>,
    pub(crate) work_capability: Option<String>,
    pub(crate) request_timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SemanticEnrichResponse {
    pub(crate) graph: CodeGraph,
    pub(crate) summary: GraphSummary,
    pub(crate) report: SemanticGraphApplyReport,
    pub(crate) responses: usize,
    pub(crate) response_errors: usize,
    pub(crate) unmatched_locations: usize,
    pub(crate) semantic_cache: SemanticLspCacheInfo,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SemanticJob {
    pub(crate) id: String,
    pub(crate) status: ScanJobStatus,
    pub(crate) path: String,
    pub(crate) message: String,
    pub(crate) created_at_unix: u64,
    pub(crate) updated_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finished_at_unix: Option<u64>,
    pub(crate) responses: Option<usize>,
    pub(crate) response_errors: Option<usize>,
    pub(crate) unmatched_locations: Option<usize>,
    pub(crate) report: Option<SemanticGraphApplyReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<SemanticEnrichResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SemanticJobResult {
    pub(crate) id: String,
    pub(crate) root: String,
    pub(crate) result: SemanticEnrichResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct SemanticJobListResponse {
    pub(crate) jobs: Vec<SemanticJob>,
    pub(crate) total: usize,
    pub(crate) returned: usize,
    pub(crate) limit: usize,
    pub(crate) status: Option<ScanJobStatus>,
    pub(crate) summary: JobStoreHealth,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ScanOptionsQuery {
    pub(crate) path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ArchitectureQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) group_limit: Option<usize>,
    pub(crate) edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LanguageDependencyQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SurprisingLinkQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HotspotQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommunityQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CacheDiffQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceQuery {
    /// Project root, matching every other endpoint's `path` parameter.
    pub(crate) path: Option<PathBuf>,
    /// Source file inside the root (canonical form).
    pub(crate) file: Option<PathBuf>,
    pub(crate) start_line: Option<u32>,
    pub(crate) end_line: Option<u32>,
    pub(crate) context: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceSearchQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) q: String,
    pub(crate) path_filter: Option<String>,
    pub(crate) case_sensitive: Option<bool>,
    pub(crate) limit: Option<usize>,
    pub(crate) context: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TraceQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) label: Option<String>,
    pub(crate) node_id: Option<String>,
    pub(crate) depth: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) label: Option<String>,
    pub(crate) node_id: Option<String>,
    pub(crate) depth: Option<usize>,
    pub(crate) block_limit: Option<usize>,
    pub(crate) edge_kind: Option<String>,
    pub(crate) confidence: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) risk_severity: Option<String>,
    pub(crate) block_kind: Option<String>,
    pub(crate) compact: Option<bool>,
    pub(crate) max_fanout: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EntrypointTraceQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) search: Option<String>,
    pub(crate) depth: Option<usize>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EntrypointWorkflowQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) search: Option<String>,
    pub(crate) entrypoint_kind: Option<String>,
    pub(crate) depth: Option<usize>,
    pub(crate) block_limit: Option<usize>,
    pub(crate) limit: Option<usize>,
    pub(crate) edge_kind: Option<String>,
    pub(crate) confidence: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) risk_severity: Option<String>,
    pub(crate) block_kind: Option<String>,
    pub(crate) compact: Option<bool>,
    pub(crate) max_fanout: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowQuerySliceQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) q: String,
    pub(crate) depth: Option<usize>,
    pub(crate) block_limit: Option<usize>,
    pub(crate) limit: Option<usize>,
    pub(crate) edge_kind: Option<String>,
    pub(crate) confidence: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) risk_severity: Option<String>,
    pub(crate) block_kind: Option<String>,
    pub(crate) compact: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigTraceQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) target: String,
    pub(crate) depth: Option<usize>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorTraceQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) target: String,
    pub(crate) depth: Option<usize>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) q: String,
    pub(crate) compact: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RefactorContextQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) target: String,
    pub(crate) from: Option<String>,
    pub(crate) depth: Option<usize>,
    pub(crate) paths: Option<usize>,
    pub(crate) dependent_limit: Option<usize>,
    pub(crate) risk_limit: Option<usize>,
    pub(crate) source_context: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SeamQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) limit: Option<usize>,
    pub(crate) edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PrImpactQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) base: Option<String>,
    /// Comma-separated explicit changed files; skips git entirely.
    pub(crate) files: Option<String>,
    pub(crate) ci_state: Option<String>,
    pub(crate) review_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ImpactQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) target: String,
    pub(crate) depth: Option<usize>,
    pub(crate) limit: Option<usize>,
    pub(crate) include_risks: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ComponentDependencyQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) target: String,
    pub(crate) group_limit: Option<usize>,
    pub(crate) edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ComponentContractQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JourneyQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) depth: Option<usize>,
    pub(crate) paths: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct McpQuery {
    pub(crate) path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NaturalGraphQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) q: String,
    pub(crate) compact: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExplainEdgeQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) edge_index: Option<usize>,
    pub(crate) source: Option<String>,
    pub(crate) target: Option<String>,
    pub(crate) kind: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct InsightQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) severity: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) search: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CheckQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) fail_on: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) search: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectReportQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) format: Option<ProjectReportFormat>,
    pub(crate) architecture_group_limit: Option<usize>,
    pub(crate) architecture_edge_limit: Option<usize>,
    pub(crate) language_link_limit: Option<usize>,
    pub(crate) hotspot_limit: Option<usize>,
    pub(crate) community_limit: Option<usize>,
    pub(crate) insight_limit: Option<usize>,
    pub(crate) file_summary_limit: Option<usize>,
    pub(crate) node_summary_limit: Option<usize>,
    pub(crate) fail_on: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProjectReportFormat {
    Json,
    Markdown,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphSliceQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) node_offset: Option<usize>,
    pub(crate) node_limit: Option<usize>,
    pub(crate) edge_offset: Option<usize>,
    pub(crate) edge_limit: Option<usize>,
    pub(crate) path_prefix: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) search: Option<String>,
    pub(crate) language: Option<String>,
    pub(crate) item_kind: Option<String>,
    pub(crate) edge_kind: Option<String>,
    pub(crate) confidence: Option<String>,
    pub(crate) edge_relation: Option<String>,
    pub(crate) edge_source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NodeContextQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) node_id: String,
    pub(crate) edge_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NodeCardQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) node_id: String,
    pub(crate) edge_limit: Option<usize>,
    pub(crate) source_context: Option<u32>,
    pub(crate) insight_limit: Option<usize>,
    pub(crate) include_insights: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FocusQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) node_ids: Option<String>,
    pub(crate) edge_indexes: Option<String>,
    pub(crate) edge_limit: Option<usize>,
    pub(crate) compact: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExportQuery {
    pub(crate) path: Option<PathBuf>,
    pub(crate) format: Option<ExportFormat>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExportFormat {
    Json,
    Dot,
    Ndjson,
    Graphml,
    MermaidHtml,
    Cypher,
    Falkordb,
    Svg,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ScanJobRequest {
    pub(crate) path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JobListQuery {
    pub(crate) status: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScanJob {
    pub(crate) id: String,
    pub(crate) status: ScanJobStatus,
    pub(crate) path: String,
    pub(crate) message: String,
    pub(crate) created_at_unix: u64,
    pub(crate) updated_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finished_at_unix: Option<u64>,
    pub(crate) cache: Option<CacheInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<codegraph_analysis::GraphSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) graph: Option<CodeGraph>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScanJobStatus {
    Queued,
    Running,
    Complete,
    Failed,
    Canceled,
}

impl ScanJobStatus {
    pub(crate) fn is_terminal(self) -> bool {
        matches!(
            self,
            ScanJobStatus::Complete | ScanJobStatus::Failed | ScanJobStatus::Canceled
        )
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ScanJobResult {
    pub(crate) id: String,
    pub(crate) root: String,
    pub(crate) graph: CodeGraph,
}

#[derive(Debug, Serialize)]
pub(crate) struct ScanJobListResponse {
    pub(crate) jobs: Vec<ScanJob>,
    pub(crate) total: usize,
    pub(crate) returned: usize,
    pub(crate) limit: usize,
    pub(crate) status: Option<ScanJobStatus>,
    pub(crate) summary: JobStoreHealth,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProjectRoot {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) default: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProjectResponse {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) default: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    pub(crate) status: &'static str,
    pub(crate) server_version: &'static str,
    pub(crate) root: String,
    pub(crate) max_file_size: u64,
    pub(crate) cache_dir: Option<String>,
    pub(crate) max_scan_jobs: usize,
    pub(crate) scan_jobs: JobStoreHealth,
    pub(crate) scan_concurrency: ConcurrencyHealth,
    pub(crate) max_semantic_jobs: usize,
    pub(crate) semantic_jobs: JobStoreHealth,
    pub(crate) semantic_concurrency: ConcurrencyHealth,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProbeResponse {
    pub(crate) status: &'static str,
    pub(crate) server_version: &'static str,
    pub(crate) api_version: u32,
    pub(crate) graph_schema_version: u32,
    pub(crate) root: String,
    pub(crate) cache_enabled: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct MetricsResponse {
    pub(crate) status: &'static str,
    pub(crate) server_version: &'static str,
    pub(crate) api_version: u32,
    pub(crate) graph_schema_version: u32,
    pub(crate) uptime_seconds: u64,
    pub(crate) root: String,
    pub(crate) projects: usize,
    pub(crate) languages: usize,
    pub(crate) features: usize,
    pub(crate) max_file_size: u64,
    pub(crate) cache: CacheCapabilityResponse,
    pub(crate) scan_jobs: JobPoolMetricsResponse,
    pub(crate) semantic_jobs: JobPoolMetricsResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct JobPoolMetricsResponse {
    pub(crate) max_retained: usize,
    pub(crate) store: JobStoreHealth,
    pub(crate) concurrency: ConcurrencyHealth,
}

#[derive(Debug, Serialize)]
pub(crate) struct CapabilitiesResponse {
    pub(crate) name: &'static str,
    pub(crate) server_version: &'static str,
    pub(crate) api_version: u32,
    pub(crate) graph_schema_version: u32,
    pub(crate) root: String,
    pub(crate) projects: Vec<ProjectResponse>,
    pub(crate) languages: Vec<LanguageResponse>,
    pub(crate) export_formats: Vec<&'static str>,
    pub(crate) features: Vec<&'static str>,
    pub(crate) endpoints: Vec<EndpointGroupResponse>,
    pub(crate) scan: ScanCapabilityResponse,
    pub(crate) limits: RuntimeLimitsResponse,
    pub(crate) cache: CacheCapabilityResponse,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiSchemaResponse {
    pub(crate) name: &'static str,
    pub(crate) server_version: &'static str,
    pub(crate) api_version: u32,
    pub(crate) graph_schema_version: u32,
    pub(crate) description: &'static str,
    pub(crate) common_response_headers: Vec<ApiHeaderSpec>,
    pub(crate) groups: Vec<ApiSchemaGroup>,
    pub(crate) enum_values: BTreeMap<&'static str, Vec<&'static str>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiHeaderSpec {
    pub(crate) name: &'static str,
    pub(crate) value_type: &'static str,
    pub(crate) required: bool,
    pub(crate) description: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiSchemaGroup {
    pub(crate) group: &'static str,
    pub(crate) endpoints: Vec<ApiEndpointSpec>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiEndpointSpec {
    pub(crate) method: &'static str,
    pub(crate) path: &'static str,
    pub(crate) summary: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) parameters: Vec<ApiParameterSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) body: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) body_capability_limit: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) body_fields: Vec<ApiParameterSpec>,
    pub(crate) response: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) response_fields: Vec<ApiParameterSpec>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) response_headers: Vec<ApiHeaderSpec>,
    pub(crate) streaming: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) example: Option<&'static str>,
}

impl ApiEndpointSpec {
    pub(crate) fn with_body_fields(mut self, body_fields: Vec<ApiParameterSpec>) -> Self {
        self.body_fields = body_fields;
        self
    }

    pub(crate) fn with_example(mut self, example: &'static str) -> Self {
        self.example = Some(example);
        self
    }

    pub(crate) fn with_response_fields(mut self, response_fields: Vec<ApiParameterSpec>) -> Self {
        self.response_fields = response_fields;
        self
    }

    pub(crate) fn with_response_headers(mut self, response_headers: Vec<ApiHeaderSpec>) -> Self {
        self.response_headers = response_headers;
        self
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiParameterSpec {
    pub(crate) name: &'static str,
    pub(crate) location: &'static str,
    pub(crate) required: bool,
    pub(crate) value_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) default: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) minimum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) maximum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) capability_limit: Option<&'static str>,
    pub(crate) description: &'static str,
}

impl ApiParameterSpec {
    pub(crate) fn with_range(mut self, minimum: usize, maximum: usize) -> Self {
        self.minimum = Some(minimum);
        self.maximum = Some(maximum);
        self
    }

    pub(crate) fn with_max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    pub(crate) fn with_capability_limit(mut self, capability_limit: &'static str) -> Self {
        self.capability_limit = Some(capability_limit);
        self
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct EndpointGroupResponse {
    pub(crate) group: &'static str,
    pub(crate) endpoints: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ScanCapabilityResponse {
    pub(crate) include_hidden: bool,
    pub(crate) include_ignored: bool,
    pub(crate) allow_any_path: bool,
    pub(crate) max_file_size: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RuntimeLimitsResponse {
    pub(crate) max_scan_jobs: usize,
    pub(crate) max_semantic_jobs: usize,
    pub(crate) max_scan_concurrency: usize,
    pub(crate) max_semantic_concurrency: usize,
    pub(crate) default_api_body_bytes: usize,
    pub(crate) max_api_body_bytes: usize,
    pub(crate) max_configurable_api_body_bytes: usize,
    pub(crate) default_job_list_limit: usize,
    pub(crate) max_job_list_limit: usize,
    pub(crate) default_semantic_work_item_limit: usize,
    pub(crate) max_semantic_work_item_limit: usize,
    pub(crate) default_semantic_request_timeout_ms: u64,
    pub(crate) max_semantic_request_timeout_ms: u64,
    pub(crate) default_incremental_report_limit: usize,
    pub(crate) max_incremental_report_limit: usize,
    pub(crate) default_graph_node_limit: usize,
    pub(crate) max_graph_node_limit: usize,
    pub(crate) default_graph_edge_limit: usize,
    pub(crate) max_graph_edge_limit: usize,
    pub(crate) default_node_context_edge_limit: usize,
    pub(crate) max_node_context_edge_limit: usize,
    pub(crate) default_node_card_source_context: u32,
    pub(crate) max_node_card_source_context: u32,
    pub(crate) default_node_card_insight_limit: usize,
    pub(crate) max_node_card_insight_limit: usize,
    pub(crate) default_focus_edge_limit: usize,
    pub(crate) max_focus_edge_limit: usize,
    pub(crate) default_graph_query_limit: usize,
    pub(crate) max_graph_query_limit: usize,
    pub(crate) max_graph_query_length: usize,
    pub(crate) default_insight_limit: usize,
    pub(crate) max_insight_limit: usize,
    pub(crate) default_report_architecture_group_limit: usize,
    pub(crate) max_report_architecture_group_limit: usize,
    pub(crate) default_report_architecture_edge_limit: usize,
    pub(crate) max_report_architecture_edge_limit: usize,
    pub(crate) default_report_language_link_limit: usize,
    pub(crate) max_report_language_link_limit: usize,
    pub(crate) default_report_hotspot_limit: usize,
    pub(crate) max_report_hotspot_limit: usize,
    pub(crate) default_report_community_limit: usize,
    pub(crate) max_report_community_limit: usize,
    pub(crate) default_report_insight_limit: usize,
    pub(crate) max_report_insight_limit: usize,
    pub(crate) report_quality_gate_sample_limit: usize,
    pub(crate) default_report_file_summary_limit: usize,
    pub(crate) max_report_file_summary_limit: usize,
    pub(crate) default_report_node_summary_limit: usize,
    pub(crate) max_report_node_summary_limit: usize,
    pub(crate) default_source_context: u32,
    pub(crate) max_source_context: u32,
    pub(crate) default_source_search_limit: usize,
    pub(crate) max_source_search_limit: usize,
    pub(crate) max_source_search_query_length: usize,
    pub(crate) default_source_search_context: usize,
    pub(crate) max_source_search_context: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct CacheCapabilityResponse {
    pub(crate) enabled: bool,
    pub(crate) dir: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct JobStoreHealth {
    pub(crate) total: usize,
    pub(crate) queued: usize,
    pub(crate) running: usize,
    pub(crate) complete: usize,
    pub(crate) failed: usize,
    pub(crate) canceled: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConcurrencyHealth {
    pub(crate) limit: usize,
    pub(crate) active: usize,
    pub(crate) available: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct LanguageResponse {
    pub(crate) language: &'static str,
    pub(crate) parser: &'static str,
    pub(crate) extensions: &'static [&'static str],
    pub(crate) file_names: &'static [&'static str],
}

#[derive(Debug, Serialize)]
pub(crate) struct ScanOptionsResponse {
    pub(crate) root: String,
    pub(crate) config_path: Option<String>,
    pub(crate) include_hidden: bool,
    pub(crate) include_ignored: bool,
    pub(crate) max_file_size: u64,
    pub(crate) ignored_names: Vec<String>,
    pub(crate) ignored_globs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ScanResponse {
    pub(crate) root: String,
    pub(crate) cache: CacheInfo,
    pub(crate) graph: CodeGraph,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProjectReportResponse {
    pub(crate) root: String,
    pub(crate) generated_at_unix: u64,
    pub(crate) cache: CacheInfo,
    pub(crate) coverage: codegraph_indexer::ScanCoverageReport,
    pub(crate) report: ProjectReport,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorBody {
    pub(crate) error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) request_id: Option<String>,
}
