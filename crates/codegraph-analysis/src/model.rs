//! Public analysis contracts: request/response structs for summaries,
//! reports, traces, workflows, journeys, refactoring, queries, insights,
//! and cards.

use codegraph_core::{CodeGraph, Edge, Node, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[allow(unused_imports)]
use crate::*;

/// Agent-facing previews stay small enough to fit beside graph context in a
/// single model turn. Callers can use source-search for a different window.
pub(crate) const SOURCE_PREVIEW_LINE_LIMIT: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSummary {
    pub nodes: usize,
    pub edges: usize,
    pub node_kinds: BTreeMap<String, usize>,
    pub edge_kinds: BTreeMap<String, usize>,
    pub edge_confidences: BTreeMap<String, usize>,
    pub edge_relations: BTreeMap<String, usize>,
    pub edge_sources: BTreeMap<String, usize>,
    pub languages: BTreeMap<String, usize>,
    pub annotation_facets: BTreeMap<String, BTreeMap<String, usize>>,
    pub entrypoints: usize,
    pub skipped_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureMap {
    pub groups: Vec<ArchitectureGroup>,
    pub edges: Vec<ArchitectureEdge>,
    pub total_groups: usize,
    pub total_edges: usize,
    pub truncated_groups: bool,
    pub truncated_edges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureGroup {
    pub id: String,
    pub label: String,
    pub files: usize,
    pub symbols: usize,
    pub entrypoints: usize,
    pub skipped_files: usize,
    pub languages: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchitectureEdge {
    pub source: String,
    pub target: String,
    pub count: usize,
    pub edge_kinds: BTreeMap<String, usize>,
    pub confidences: BTreeMap<String, usize>,
    pub edge_indexes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageDependencyReport {
    pub links: Vec<LanguageDependency>,
    pub total_links: usize,
    pub total_edges: usize,
    pub cross_language_edges: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageDependency {
    pub source_language: String,
    pub target_language: String,
    pub count: usize,
    pub edge_kinds: BTreeMap<String, usize>,
    pub confidences: BTreeMap<String, usize>,
    pub edge_indexes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurprisingLinkReport {
    pub links: Vec<SurprisingLink>,
    pub total_candidates: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurprisingLink {
    pub source: Node,
    pub target: Node,
    pub source_area: String,
    pub target_area: String,
    pub source_language: String,
    pub target_language: String,
    pub edge_kind: String,
    pub confidence: String,
    pub score: usize,
    pub reasons: Vec<String>,
    pub edge_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotspotReport {
    pub hotspots: Vec<Hotspot>,
    pub architectural_hubs: Vec<Hotspot>,
    pub utility_hubs: Vec<Hotspot>,
    pub total_candidates: usize,
    pub total_architectural_hubs: usize,
    pub total_utility_hubs: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotspot {
    pub node: Node,
    pub score: usize,
    pub incoming: usize,
    pub outgoing: usize,
    pub edge_kinds: BTreeMap<String, usize>,
    pub hub_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityReport {
    pub communities: Vec<GraphCommunity>,
    pub total_communities: usize,
    pub total_nodes: usize,
    pub total_internal_edges: usize,
    pub total_external_edges: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphCommunity {
    pub id: String,
    pub label: String,
    pub node_count: usize,
    pub files: usize,
    pub entrypoints: usize,
    pub internal_edges: usize,
    pub incoming_external_edges: usize,
    pub outgoing_external_edges: usize,
    /// The share of this subsystem's edges that stay inside it, in whole
    /// percent. mastodon's `app/javascript` is 100 -- a frontend and a
    /// backend sharing a repository -- while `app/helpers` is 33, which is
    /// what a subsystem that exists to be used by others looks like.
    pub cohesion_percent: usize,
    pub languages: BTreeMap<String, usize>,
    pub node_kinds: BTreeMap<String, usize>,
    pub sample_nodes: Vec<Node>,
    pub edge_indexes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSearchRequest {
    pub query: String,
    pub path_filter: Option<String>,
    pub case_sensitive: bool,
    pub limit: usize,
    pub context: usize,
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: u64,
    pub ignored_names: BTreeSet<String>,
    pub ignored_globs: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSearchResult {
    pub query: String,
    pub path_filter: Option<String>,
    pub case_sensitive: bool,
    pub total_matches: usize,
    pub matches: Vec<SourceSearchMatch>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSearchMatch {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub line_text: String,
    pub context: Vec<SourceSearchLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSearchLine {
    pub number: u32,
    pub text: String,
    pub highlight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRequest {
    pub start: TraceStart,
    pub max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStart {
    NodeId(NodeId),
    Label(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceResult {
    pub start: Node,
    pub max_depth: usize,
    pub nodes: Vec<TraceNode>,
    pub edges: Vec<Edge>,
    pub truncated: bool,
    /// Which definition this answer is about when the name it was given
    /// belongs to more than one; see `shared_name_note`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRequest {
    pub start: TraceStart,
    pub max_depth: usize,
    pub block_limit: usize,
    pub filters: WorkflowFilters,
    #[serde(default)]
    pub compact: bool,
    /// Cap on outgoing edges expanded per node, prioritizing calls. Lets the
    /// block budget reach depth on wide entrypoints (e.g. a CLI `main` that
    /// calls hundreds of things) instead of a shallow, wide fan. `None` keeps
    /// the unbounded breadth-first behavior.
    #[serde(default)]
    pub max_fanout: Option<usize>,
}

/// Node investigation card options, shared by the CLI `node-card`
/// command, `/api/node-card`, and the MCP `get_node_card` tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCardRequest {
    pub node_id: NodeId,
    pub edge_limit: usize,
    pub source_context: u32,
    pub insight_limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowFilters {
    pub edge_kind: Option<String>,
    pub confidence: Option<String>,
    pub language: Option<String>,
    pub risk_severity: Option<String>,
    pub block_kind: Option<String>,
}

impl WorkflowFilters {
    /// Build filters from raw user-supplied parts, trimming whitespace and
    /// dropping empty values, so CLI flags and API query parameters
    /// normalize identically.
    pub fn from_parts(
        edge_kind: Option<String>,
        confidence: Option<String>,
        language: Option<String>,
        risk_severity: Option<String>,
        block_kind: Option<String>,
    ) -> Self {
        fn normalize(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        }
        Self {
            edge_kind: normalize(edge_kind),
            confidence: normalize(confidence),
            language: normalize(language),
            risk_severity: normalize(risk_severity),
            block_kind: normalize(block_kind),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowReport {
    pub start: Node,
    pub max_depth: usize,
    pub block_limit: usize,
    pub filters: WorkflowFilters,
    pub compact: bool,
    pub blocks: Vec<WorkflowBlock>,
    pub transitions: Vec<WorkflowTransition>,
    pub total_blocks: usize,
    pub total_transitions: usize,
    pub raw_total_blocks: usize,
    pub raw_total_transitions: usize,
    pub truncated: bool,
    /// Which definition this answer is about when the name it was given
    /// belongs to more than one; see `shared_name_note`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowBlock {
    pub id: String,
    pub kind: WorkflowBlockKind,
    pub node: Node,
    pub depth: usize,
    pub source_node_ids: Vec<NodeId>,
    pub risk_refs: Vec<WorkflowRiskRef>,
    #[serde(default)]
    pub compacted: bool,
    #[serde(default)]
    pub compacted_count: usize,
    /// Outgoing edges dropped from this node by the fan-out cap — how many more
    /// steps are reachable by drilling into it. 0 when nothing was trimmed.
    #[serde(default)]
    pub truncated_children: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBlockKind {
    Start,
    Call,
    ConfigRead,
    EnvironmentRead,
    Dependency,
    Import,
    Branch,
    Loop,
    Async,
    Return,
    Error,
    Reference,
    ExternalBoundary,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTransition {
    pub id: String,
    pub source: String,
    pub target: String,
    pub source_node_id: NodeId,
    pub target_node_id: NodeId,
    pub edge: Edge,
    pub edge_index: usize,
    pub risk_refs: Vec<WorkflowRiskRef>,
    #[serde(default)]
    pub compacted: bool,
    #[serde(default)]
    pub compacted_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRiskRef {
    pub insight_index: usize,
    pub kind: String,
    pub severity: InsightSeverity,
    pub message: String,
    pub edge_indexes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointTraceRequest {
    pub search: Option<String>,
    pub max_depth: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointTraceReport {
    pub max_depth: usize,
    pub total_entrypoints: usize,
    pub traces: Vec<TraceResult>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointWorkflowRequest {
    pub search: Option<String>,
    #[serde(default)]
    pub entrypoint_kind: Option<String>,
    pub max_depth: usize,
    pub block_limit: usize,
    pub limit: usize,
    pub filters: WorkflowFilters,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub max_fanout: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointWorkflowReport {
    pub max_depth: usize,
    pub block_limit: usize,
    #[serde(default)]
    pub entrypoint_kind: Option<String>,
    pub filters: WorkflowFilters,
    pub total_entrypoints: usize,
    pub workflows: Vec<WorkflowReport>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JourneyRequest {
    pub from: String,
    pub to: String,
    pub max_depth: usize,
    #[serde(default)]
    pub path_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JourneyHopExplanation {
    pub confidence: String,
    pub confidence_note: String,
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub provenance_source: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JourneyStep {
    pub step: usize,
    #[serde(default)]
    pub transition: Option<WorkflowTransition>,
    #[serde(default)]
    pub explanation: Option<JourneyHopExplanation>,
    #[serde(default)]
    pub fragile: bool,
    #[serde(default)]
    pub fragile_reasons: Vec<String>,
    pub block: WorkflowBlock,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JourneyRiskSummary {
    pub risky_steps: usize,
    pub risky_transitions: usize,
    pub fragile_transitions: usize,
    pub low_confidence_hops: usize,
    pub unresolved_calls: usize,
    pub ambiguous_calls: usize,
    pub duplicate_labels: usize,
    pub cycle_back_edges: usize,
    pub severity_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JourneyPath {
    pub rank: usize,
    pub total_steps: usize,
    pub confidence_score: usize,
    #[serde(default)]
    pub lowest_confidence: Option<String>,
    #[serde(default)]
    pub risk_summary: JourneyRiskSummary,
    pub steps: Vec<JourneyStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDependencyRequest {
    pub target: String,
    pub group_limit: usize,
    pub edge_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDependencyGroup {
    pub key: String,
    pub incoming: usize,
    pub outgoing: usize,
    pub edge_kinds: BTreeMap<String, usize>,
    pub confidence_counts: BTreeMap<String, usize>,
    pub sample_edge_indexes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentDependencyReport {
    pub target: Node,
    #[serde(default)]
    pub area: Option<String>,
    pub total_incoming: usize,
    pub total_outgoing: usize,
    pub areas: Vec<ComponentDependencyGroup>,
    pub packages: Vec<ComponentDependencyGroup>,
    pub languages: Vec<ComponentDependencyGroup>,
    pub truncated: bool,
    /// Which definition this answer is about when the name it was given
    /// belongs to more than one; see `shared_name_note`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentContractRequest {
    pub source: String,
    pub target: String,
    pub edge_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentContractEdge {
    pub edge_index: usize,
    pub edge: Edge,
    pub source_label: String,
    pub target_label: String,
    pub risk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentContractReport {
    pub source_area: String,
    pub target_area: String,
    pub total_edges: usize,
    pub edge_kinds: BTreeMap<String, usize>,
    pub confidence_counts: BTreeMap<String, usize>,
    pub edges: Vec<ComponentContractEdge>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorContextRequest {
    pub target: String,
    #[serde(default)]
    pub from: Option<String>,
    pub max_depth: usize,
    pub path_limit: usize,
    pub dependent_limit: usize,
    pub risk_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactorContextBundle {
    pub schema: String,
    pub target: Node,
    #[serde(default)]
    pub area: Option<String>,
    pub impact: ImpactReport,
    pub dependencies: ComponentDependencyReport,
    #[serde(default)]
    pub journey: Option<JourneyReport>,
    pub total_risks: usize,
    pub risks: Vec<Insight>,
    pub risks_truncated: bool,
    #[serde(default)]
    pub target_source: Option<SourcePreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeamRequest {
    pub limit: usize,
    pub edge_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeamCandidate {
    pub source_area: String,
    pub target_area: String,
    pub edge_count: usize,
    pub edge_kinds: BTreeMap<String, usize>,
    pub confidence_counts: BTreeMap<String, usize>,
    pub low_confidence_edges: usize,
    pub risk_count: usize,
    pub friction_score: usize,
    pub sample_edge_indexes: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeamReport {
    pub total_pairs: usize,
    pub safest: Vec<SeamCandidate>,
    pub most_needed: Vec<SeamCandidate>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactRequest {
    pub target: String,
    pub max_depth: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactDependent {
    pub node: Node,
    pub distance: usize,
    pub is_test: bool,
    pub risk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactEntrypoint {
    pub node: Node,
    #[serde(default)]
    pub entrypoint_kind: Option<String>,
    pub distance: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    #[serde(default)]
    pub schema: String,
    pub target: Node,
    #[serde(default)]
    pub area: Option<String>,
    pub max_depth: usize,
    pub total_dependents: usize,
    pub affected_entrypoints: Vec<ImpactEntrypoint>,
    pub affected_routes: usize,
    pub affected_tests: usize,
    pub languages: BTreeMap<String, usize>,
    pub areas: BTreeMap<String, usize>,
    pub severity_counts: BTreeMap<String, usize>,
    /// False for the latency-optimized agent path. Request risks explicitly
    /// or use refactor-context when repository-wide findings are needed.
    #[serde(default)]
    pub risks_evaluated: bool,
    pub impact_score: usize,
    pub dependents: Vec<ImpactDependent>,
    pub truncated: bool,
    #[serde(default)]
    pub suggested_commands: Vec<String>,
    /// Which definition this answer is about when the name it was given
    /// belongs to more than one; see `shared_name_note`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JourneyReport {
    #[serde(default)]
    pub schema: String,
    pub from: Node,
    pub to: Node,
    pub max_depth: usize,
    pub total_paths: usize,
    pub paths: Vec<JourneyPath>,
    pub truncated: bool,
    #[serde(default)]
    pub suggested_commands: Vec<String>,
    /// Which definitions these answers are about when a name it was
    /// given belongs to more than one; see `shared_name_note`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowQueryRequest {
    pub query: String,
    pub max_depth: usize,
    pub block_limit: usize,
    pub limit: usize,
    pub filters: WorkflowFilters,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub max_fanout: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowQueryReport {
    pub query: String,
    pub max_depth: usize,
    pub block_limit: usize,
    pub filters: WorkflowFilters,
    pub total_query_nodes: usize,
    pub total_query_edges: usize,
    pub total_candidates: usize,
    pub workflows: Vec<WorkflowReport>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigTraceRequest {
    pub target: String,
    pub max_depth: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigTraceResult {
    pub target: String,
    pub max_depth: usize,
    pub matches: Vec<ConfigTraceMatch>,
    pub total_matches: usize,
    pub total_readers: usize,
    pub total_paths: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigTraceMatch {
    pub target: Node,
    pub readers: Vec<ConfigReader>,
    pub paths: Vec<ConfigTracePath>,
    pub total_readers: usize,
    pub total_paths: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigReader {
    pub node: Node,
    pub edge: Edge,
    pub edge_index: usize,
    /// `reads` or `sets`: a workflow job or a Compose service that assigns
    /// the variable holds the same edge kind as the code that reads it,
    /// and calling both a reader tells the wrong story.
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigTracePath {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub edge_indexes: Vec<usize>,
    pub reached_entrypoint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorTraceRequest {
    pub target: String,
    pub max_depth: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorTraceResult {
    pub target: String,
    pub max_depth: usize,
    pub matches: Vec<ErrorTraceMatch>,
    pub total_matches: usize,
    pub total_sources: usize,
    pub total_paths: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorTraceMatch {
    pub error: Node,
    pub sources: Vec<ErrorSource>,
    pub paths: Vec<ErrorTracePath>,
    pub total_sources: usize,
    pub total_paths: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorSource {
    pub node: Node,
    pub edge: Edge,
    pub edge_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorTracePath {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub edge_indexes: Vec<usize>,
    pub reached_entrypoint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceNode {
    pub node: Node,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResult {
    pub query: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub total_nodes: usize,
    pub total_edges: usize,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub raw_total_nodes: usize,
    #[serde(default)]
    pub raw_total_edges: usize,
    #[serde(default)]
    pub compacted_nodes: usize,
    #[serde(default)]
    pub compacted_edges: usize,
    #[serde(default)]
    pub returned_nodes: usize,
    #[serde(default)]
    pub returned_edges: usize,
    pub truncated: bool,
    #[serde(default)]
    pub facets: QueryFacets,
    /// What the answer had to decide for itself. A query that names a
    /// label several definitions answer to has to pick one, and saying
    /// which — and that there were others — is the difference between an
    /// answer and a claim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaturalQueryRequest {
    pub question: String,
    #[serde(default)]
    pub compact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaturalQueryReport {
    #[serde(default)]
    pub schema: String,
    pub question: String,
    pub generated_query: String,
    #[serde(default)]
    pub cli_snippet: String,
    pub rule: String,
    pub confidence: String,
    pub result: QueryResult,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryFacets {
    pub node_kinds: BTreeMap<String, usize>,
    pub edge_kinds: BTreeMap<String, usize>,
    pub languages: BTreeMap<String, usize>,
    pub item_kinds: BTreeMap<String, usize>,
    pub edge_confidences: BTreeMap<String, usize>,
}

impl QueryResult {
    pub(crate) fn new(
        graph: &CodeGraph,
        query: impl Into<String>,
        nodes: Vec<Node>,
        edges: Vec<Edge>,
        total_nodes: usize,
        total_edges: usize,
        truncated: bool,
    ) -> Self {
        let edges = edges_with_indexes(graph, edges);
        let returned_nodes = nodes.len();
        let returned_edges = edges.len();
        let facets = QueryFacets::from_graph_parts(&nodes, &edges);
        Self {
            query: query.into(),
            nodes,
            edges,
            total_nodes,
            total_edges,
            compact: false,
            raw_total_nodes: total_nodes,
            raw_total_edges: total_edges,
            compacted_nodes: 0,
            compacted_edges: 0,
            returned_nodes,
            returned_edges,
            truncated,
            facets,
            notes: Vec::new(),
        }
    }

    pub(crate) fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

pub(crate) const EDGE_INDEX_METADATA_KEY: &str = "edge_index";
pub(crate) const EDGE_EXPLANATION_INSIGHT_LIMIT: usize = 25;

impl QueryFacets {
    pub(crate) fn from_graph_parts(nodes: &[Node], edges: &[Edge]) -> Self {
        let mut node_kinds = BTreeMap::new();
        let mut edge_kinds = BTreeMap::new();
        let mut languages = BTreeMap::new();
        let mut item_kinds = BTreeMap::new();
        let mut edge_confidences = BTreeMap::new();

        for node in nodes {
            increment_facet(&mut node_kinds, kind_name(&node.kind));
            if let Some(language) = node.metadata.get("language") {
                increment_facet(&mut languages, language.clone());
            }
            if let Some(item_kind) = node.metadata.get("item_kind") {
                increment_facet(&mut item_kinds, item_kind.clone());
            }
        }

        for edge in edges {
            increment_facet(&mut edge_kinds, edge_kind_name(&edge.kind));
            increment_facet(&mut edge_confidences, confidence_name(edge.confidence));
        }

        Self {
            node_kinds,
            edge_kinds,
            languages,
            item_kinds,
            edge_confidences,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainEdgeRequest {
    pub edge_index: Option<usize>,
    pub source: Option<String>,
    pub target: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeExplanation {
    pub edge_index: usize,
    pub total_matches: usize,
    pub source: Node,
    pub target: Node,
    pub edge: Edge,
    pub summary: String,
    pub evidence: Vec<String>,
    #[serde(default)]
    pub insight_summary: NodeInsightSummary,
    #[serde(default)]
    pub insights: Vec<Insight>,
    #[serde(default)]
    pub total_insights: usize,
    #[serde(default)]
    pub insight_limit: usize,
    #[serde(default)]
    pub truncated_insights: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusRequest {
    pub node_ids: Vec<NodeId>,
    pub edge_indexes: Vec<usize>,
    pub edge_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSliceRequest {
    pub node_offset: usize,
    pub node_limit: usize,
    pub edge_offset: usize,
    pub edge_limit: usize,
    pub path_prefix: Option<String>,
    pub kind: Option<String>,
    pub search: Option<String>,
    pub language: Option<String>,
    pub item_kind: Option<String>,
    pub edge_kind: Option<String>,
    pub confidence: Option<String>,
    pub edge_relation: Option<String>,
    pub edge_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSlice {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub total_nodes: usize,
    pub total_edges: usize,
    pub node_offset: usize,
    pub node_limit: usize,
    pub edge_offset: usize,
    pub edge_limit: usize,
    pub truncated_nodes: bool,
    pub truncated_edges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeContext {
    pub node: Node,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub total_edges: usize,
    pub edge_limit: usize,
    pub truncated_edges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePreview {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub lines: Vec<SourcePreviewLine>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePreviewLine {
    pub number: u32,
    pub text: String,
    pub highlight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCard {
    pub context: NodeContext,
    #[serde(default)]
    pub dependency_summary: NodeDependencySummary,
    #[serde(default)]
    pub insight_summary: NodeInsightSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_summary: Option<FileNodeSummary>,
    pub source: Option<SourcePreview>,
    pub insights: Vec<Insight>,
    pub total_insights: usize,
    pub insight_limit: usize,
    pub truncated_insights: bool,
    #[serde(default)]
    pub insights_evaluated: bool,
    #[serde(default)]
    pub actions: Vec<NodeCardAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeCardAction {
    pub kind: String,
    pub label: String,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NodeDependencySummary {
    pub incoming: usize,
    pub outgoing: usize,
    pub edge_kinds: BTreeMap<String, usize>,
    pub incoming_edge_kinds: BTreeMap<String, usize>,
    pub outgoing_edge_kinds: BTreeMap<String, usize>,
    pub confidences: BTreeMap<String, usize>,
    pub neighbor_kinds: BTreeMap<String, usize>,
    pub neighbor_languages: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FileNodeSummary {
    pub contained_nodes: usize,
    pub code_symbols: usize,
    pub imports: usize,
    pub direct_dependencies: usize,
    pub trace_edges: usize,
    pub calls: usize,
    pub config_reads: usize,
    pub environment_reads: usize,
    pub error_facts: usize,
    pub unresolved_calls: usize,
    pub contained_kinds: BTreeMap<String, usize>,
    pub contained_item_kinds: BTreeMap<String, usize>,
    pub trace_edge_kinds: BTreeMap<String, usize>,
    pub trace_confidences: BTreeMap<String, usize>,
    pub trace_target_kinds: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NodeInsightSummary {
    pub by_severity: BTreeMap<String, usize>,
    pub by_kind: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    message: String,
}

impl QueryError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryError {}
