use codegraph_core::{CodeGraph, Confidence, Edge, EdgeKind, Node, NodeId, NodeKind};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

const SOURCE_PREVIEW_LINE_LIMIT: usize = 240;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRequest {
    pub start: TraceStart,
    pub max_depth: usize,
    pub block_limit: usize,
    pub filters: WorkflowFilters,
    #[serde(default)]
    pub compact: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowFilters {
    pub edge_kind: Option<String>,
    pub confidence: Option<String>,
    pub language: Option<String>,
    pub risk_severity: Option<String>,
    pub block_kind: Option<String>,
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
    pub max_depth: usize,
    pub block_limit: usize,
    pub limit: usize,
    pub filters: WorkflowFilters,
    #[serde(default)]
    pub compact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointWorkflowReport {
    pub max_depth: usize,
    pub block_limit: usize,
    pub filters: WorkflowFilters,
    pub total_entrypoints: usize,
    pub workflows: Vec<WorkflowReport>,
    pub truncated: bool,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaturalQueryRequest {
    pub question: String,
    #[serde(default)]
    pub compact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaturalQueryReport {
    pub question: String,
    pub generated_query: String,
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
    fn new(
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
        }
    }
}

const EDGE_INDEX_METADATA_KEY: &str = "edge_index";
const EDGE_EXPLANATION_INSIGHT_LIMIT: usize = 25;

impl QueryFacets {
    fn from_graph_parts(nodes: &[Node], edges: &[Edge]) -> Self {
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
    fn new(message: impl Into<String>) -> Self {
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

pub fn search_source(root: &Path, request: &SourceSearchRequest) -> SourceSearchResult {
    let query = request.query.trim();
    if query.is_empty() {
        return SourceSearchResult {
            query: request.query.clone(),
            path_filter: request.path_filter.clone(),
            case_sensitive: request.case_sensitive,
            total_matches: 0,
            matches: Vec::new(),
            truncated: false,
        };
    }

    let needle = if request.case_sensitive {
        query.to_string()
    } else {
        query.to_ascii_lowercase()
    };
    let path_filter = request
        .path_filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if request.case_sensitive {
                value.to_string()
            } else {
                value.to_ascii_lowercase()
            }
        });
    let limit = request.limit.clamp(1, 1_000);
    let context = request.context.min(20);
    let ignored_globs = compile_ignored_globs(&request.ignored_globs);
    let mut matches = Vec::new();
    let mut total_matches = 0usize;

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| should_search_entry(entry, root, request, &ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if !is_searchable_file(path, request.max_file_size) {
            continue;
        }
        let label = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if path_filter.as_deref().is_some_and(|filter| {
            let haystack = if request.case_sensitive {
                label.clone()
            } else {
                label.to_ascii_lowercase()
            };
            !haystack.contains(filter)
        }) {
            continue;
        }

        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let lines = text.lines().collect::<Vec<_>>();
        for (line_index, line) in lines.iter().enumerate() {
            let haystack = if request.case_sensitive {
                line.to_string()
            } else {
                line.to_ascii_lowercase()
            };
            let mut search_from = 0usize;
            while let Some(offset) = haystack[search_from..].find(&needle) {
                total_matches += 1;
                if matches.len() < limit {
                    let column = search_from + offset + 1;
                    matches.push(SourceSearchMatch {
                        path: label.clone(),
                        line: line_index as u32 + 1,
                        column: column as u32,
                        line_text: (*line).to_string(),
                        context: source_search_context(&lines, line_index, context),
                    });
                }
                search_from += offset + needle.len().max(1);
            }
        }
    }

    SourceSearchResult {
        query: request.query.clone(),
        path_filter: request.path_filter.clone(),
        case_sensitive: request.case_sensitive,
        total_matches,
        truncated: total_matches > matches.len(),
        matches,
    }
}

fn source_search_context(
    lines: &[&str],
    line_index: usize,
    context: usize,
) -> Vec<SourceSearchLine> {
    let start = line_index.saturating_sub(context);
    let end = (line_index + context + 1).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, line)| {
            let index = start + offset;
            SourceSearchLine {
                number: index as u32 + 1,
                text: (*line).to_string(),
                highlight: index == line_index,
            }
        })
        .collect()
}

fn should_search_entry(
    entry: &DirEntry,
    root: &Path,
    request: &SourceSearchRequest,
    ignored_globs: &Option<GlobSet>,
) -> bool {
    if entry.path() == root {
        return true;
    }

    if !request.include_hidden && is_hidden_entry(entry) {
        return false;
    }
    if !request.include_ignored
        && (is_ignored_name(entry, request) || is_ignored_glob(entry.path(), root, ignored_globs))
    {
        return false;
    }
    true
}

fn is_ignored_name(entry: &DirEntry, request: &SourceSearchRequest) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| request.ignored_names.contains(name))
}

fn is_ignored_glob(path: &Path, root: &Path, ignored_globs: &Option<GlobSet>) -> bool {
    let Some(ignored_globs) = ignored_globs else {
        return false;
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    !relative.is_empty() && ignored_globs.is_match(relative)
}

fn compile_ignored_globs(patterns: &BTreeSet<String>) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        for expanded in expanded_ignored_glob_patterns(pattern) {
            builder.add(Glob::new(&expanded).ok()?);
        }
    }
    builder.build().ok()
}

fn expanded_ignored_glob_patterns(pattern: &str) -> Vec<String> {
    let normalized = normalize_glob_pattern(pattern);
    let mut patterns = vec![normalized.clone()];
    for suffix in ["/**", "/**/*"] {
        if let Some(prefix) = normalized.strip_suffix(suffix)
            && !prefix.is_empty()
        {
            patterns.push(prefix.to_string());
        }
    }
    patterns
}

fn normalize_glob_pattern(pattern: &str) -> String {
    let mut normalized = pattern.trim().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }
    normalized
}

fn is_hidden_entry(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.') && name != "." && name != "..")
}

fn is_searchable_file(path: &Path, max_file_size: u64) -> bool {
    path.metadata()
        .map(|metadata| metadata.len() <= max_file_size)
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsightReport {
    pub total: usize,
    pub by_severity: BTreeMap<String, usize>,
    pub by_kind: BTreeMap<String, usize>,
    pub insights: Vec<Insight>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsightFilter {
    pub severity: Option<InsightSeverity>,
    pub kind: Option<String>,
    pub search: Option<String>,
    pub limit: usize,
}

pub const KNOWN_INSIGHT_KINDS: &[&str] = &[
    "ambiguous_call_resolution",
    "ambiguous_entrypoint_target",
    "conflicting_config_default",
    "conflicting_dependency_declaration",
    "cross_language_heuristic_edge",
    "custom_rule_*",
    "dependency_cycle",
    "duplicate_compose_published_port",
    "duplicate_entrypoint_label",
    "duplicate_framework_route",
    "duplicate_function_label",
    "entrypoint_dead_end",
    "mixed_dependency_scope",
    "mixed_config_requirement",
    "non_runtime_dependency_import",
    "orphan_function",
    "parse_error",
    "potential_error_flow",
    "rationale_risk_comment",
    "semantic_diagnostic",
    "sensitive_ci_environment_literal",
    "sensitive_config_default",
    "skipped_large_file",
    "syntax_error",
    "test_only_runtime_dependency",
    "undeclared_flutter_asset",
    "undeclared_external_import",
    "unreachable_config_read",
    "unreachable_error_flow",
    "unreachable_source_file",
    "unresolved_call",
    "unresolved_compose_command_path",
    "unresolved_compose_env_file_path",
    "unresolved_compose_volume_source_path",
    "unresolved_dockerfile_command_path",
    "unresolved_entrypoint_target",
    "unresolved_framework_route_handler",
    "unresolved_github_actions_job_need",
    "unresolved_github_actions_local_action",
    "unresolved_github_actions_run_path",
    "unresolved_gitlab_ci_job_dependency",
    "unresolved_gitlab_ci_script_path",
    "unresolved_kubernetes_config_ref",
    "unresolved_kubernetes_ingress_backend",
    "unresolved_kubernetes_service_selector",
    "unresolved_local_import",
    "unresolved_makefile_command_path",
    "unresolved_sql_table_reference",
    "unused_declared_dependency",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Insight {
    pub kind: String,
    pub severity: InsightSeverity,
    pub message: String,
    pub nodes: Vec<NodeId>,
    pub edges: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReport {
    pub passed: bool,
    pub fail_on: String,
    pub failing_insights: usize,
    pub report: InsightReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRiskSummary {
    pub score: usize,
    pub grade: String,
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub top_kinds: Vec<ProjectRiskKindSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRiskKindSummary {
    pub kind: String,
    pub severity: String,
    pub count: usize,
}

pub const DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT: usize = 50;
pub const MAX_REPORT_ARCHITECTURE_GROUP_LIMIT: usize = 500;
pub const DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT: usize = 200;
pub const MAX_REPORT_ARCHITECTURE_EDGE_LIMIT: usize = 2_000;
pub const DEFAULT_REPORT_LANGUAGE_LINK_LIMIT: usize = 50;
pub const MAX_REPORT_LANGUAGE_LINK_LIMIT: usize = 500;
pub const DEFAULT_REPORT_HOTSPOT_LIMIT: usize = 25;
pub const MAX_REPORT_HOTSPOT_LIMIT: usize = 500;
pub const DEFAULT_REPORT_COMMUNITY_LIMIT: usize = 25;
pub const MAX_REPORT_COMMUNITY_LIMIT: usize = 500;
pub const DEFAULT_REPORT_INSIGHT_LIMIT: usize = 50;
pub const MAX_REPORT_INSIGHT_LIMIT: usize = 500;
pub const DEFAULT_REPORT_FILE_SUMMARY_LIMIT: usize = 25;
pub const MAX_REPORT_FILE_SUMMARY_LIMIT: usize = 500;
pub const DEFAULT_REPORT_NODE_SUMMARY_LIMIT: usize = 25;
pub const MAX_REPORT_NODE_SUMMARY_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReportLimits {
    pub architecture_group_limit: usize,
    pub architecture_edge_limit: usize,
    pub language_link_limit: usize,
    pub hotspot_limit: usize,
    pub community_limit: usize,
    pub insight_limit: usize,
    pub file_summary_limit: usize,
    pub node_summary_limit: usize,
    pub fail_on: InsightSeverity,
}

impl Default for ProjectReportLimits {
    fn default() -> Self {
        Self {
            architecture_group_limit: DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
            architecture_edge_limit: DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT,
            language_link_limit: DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
            hotspot_limit: DEFAULT_REPORT_HOTSPOT_LIMIT,
            community_limit: DEFAULT_REPORT_COMMUNITY_LIMIT,
            insight_limit: DEFAULT_REPORT_INSIGHT_LIMIT,
            file_summary_limit: DEFAULT_REPORT_FILE_SUMMARY_LIMIT,
            node_summary_limit: DEFAULT_REPORT_NODE_SUMMARY_LIMIT,
            fail_on: InsightSeverity::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactFileSummaryReport {
    pub files: Vec<ProjectCompactFileSummary>,
    pub total_files: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactFileSummary {
    pub node: Node,
    pub summary: FileNodeSummary,
    pub insight_summary: NodeInsightSummary,
    pub score: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactNodeSummaryReport {
    pub nodes: Vec<ProjectCompactNodeSummary>,
    pub total_nodes: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompactNodeSummary {
    pub node: Node,
    pub dependency_summary: NodeDependencySummary,
    pub insight_summary: NodeInsightSummary,
    pub roles: Vec<String>,
    pub score: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReport {
    pub graph_schema_version: u32,
    pub summary: GraphSummary,
    pub entrypoints: Vec<Node>,
    pub insights: InsightReport,
    pub risk_summary: ProjectRiskSummary,
    pub quality_gate: CheckReport,
    pub architecture: ArchitectureMap,
    pub language_dependencies: LanguageDependencyReport,
    pub surprising_links: SurprisingLinkReport,
    pub hotspots: HotspotReport,
    pub communities: CommunityReport,
    pub file_summaries: ProjectCompactFileSummaryReport,
    pub node_summaries: ProjectCompactNodeSummaryReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReportMarkdownOptions {
    pub title: String,
    pub root: Option<String>,
    pub generated_at_unix: Option<u64>,
}

impl Default for ProjectReportMarkdownOptions {
    fn default() -> Self {
        Self {
            title: "CodeGraph Project Report".to_string(),
            root: None,
            generated_at_unix: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightSeverity {
    Info,
    Warning,
    Error,
}

impl Default for InsightFilter {
    fn default() -> Self {
        Self {
            severity: None,
            kind: None,
            search: None,
            limit: 50,
        }
    }
}

pub fn export_dot(graph: &CodeGraph) -> String {
    let mut output = String::from(
        "digraph CodeGraph {\n  rankdir=LR;\n  node [shape=box, style=\"rounded,filled\", fontname=\"Inter\"];\n  edge [fontname=\"Inter\"];\n",
    );

    for node in &graph.nodes {
        output.push_str(&format!(
            "  {} [label=\"{}\", fillcolor=\"{}\"];\n",
            node.id,
            dot_escape(&format!("{}\\n{}", node.label, kind_name(&node.kind))),
            dot_color(&node.kind)
        ));
    }

    for edge in &graph.edges {
        output.push_str(&format!(
            "  {} -> {} [label=\"{}\"];\n",
            edge.source,
            edge.target,
            dot_escape(&edge_kind_name(&edge.kind))
        ));
    }

    output.push_str("}\n");
    output
}

pub fn export_ndjson(graph: &CodeGraph) -> Result<String, serde_json::Error> {
    let mut lines = Vec::with_capacity(graph.nodes.len() + graph.edges.len() + 1);
    lines.push(serde_json::to_string(&json!({
        "record_type": "graph",
        "schema_version": graph.schema_version,
        "root": graph.root,
    }))?);

    for node in &graph.nodes {
        lines.push(serde_json::to_string(&json!({
            "record_type": "node",
            "node": node,
        }))?);
    }

    for edge in &graph.edges {
        lines.push(serde_json::to_string(&json!({
            "record_type": "edge",
            "edge": edge,
        }))?);
    }

    Ok(format!("{}\n", lines.join("\n")))
}

pub fn summarize(graph: &CodeGraph) -> GraphSummary {
    let mut node_kinds = BTreeMap::new();
    let mut edge_kinds = BTreeMap::new();
    let mut edge_confidences = BTreeMap::new();
    let mut edge_relations = BTreeMap::new();
    let mut edge_sources = BTreeMap::new();
    let mut languages = BTreeMap::new();
    let mut annotation_facets: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut skipped_files = 0;

    for node in &graph.nodes {
        *node_kinds.entry(kind_name(&node.kind)).or_insert(0) += 1;
        if matches!(node.kind, NodeKind::File)
            && node
                .metadata
                .get("skipped")
                .is_some_and(|value| value == "true")
        {
            skipped_files += 1;
        }
        if let Some(language) = node.metadata.get("language") {
            *languages.entry(language.clone()).or_insert(0) += 1;
        }
        for (key, value) in &node.metadata {
            if !key.starts_with("annotation.") || value.trim().is_empty() {
                continue;
            }
            *annotation_facets
                .entry(key.clone())
                .or_default()
                .entry(value.clone())
                .or_insert(0) += 1;
        }
    }

    for edge in &graph.edges {
        *edge_kinds.entry(edge_kind_name(&edge.kind)).or_insert(0) += 1;
        *edge_confidences
            .entry(confidence_name(edge.confidence))
            .or_insert(0) += 1;
        if let Some(relation) = edge
            .metadata
            .get("relation")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            *edge_relations.entry(relation.to_string()).or_insert(0) += 1;
        }
        if let Some(source) = edge
            .metadata
            .get("source")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            *edge_sources.entry(source.to_string()).or_insert(0) += 1;
        }
    }

    GraphSummary {
        nodes: graph.nodes.len(),
        edges: graph.edges.len(),
        node_kinds,
        edge_kinds,
        edge_confidences,
        edge_relations,
        edge_sources,
        languages,
        annotation_facets,
        entrypoints: graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Entrypoint)
            .count(),
        skipped_files,
    }
}

pub fn architecture_map(
    graph: &CodeGraph,
    group_limit: usize,
    edge_limit: usize,
) -> ArchitectureMap {
    let group_limit = group_limit.clamp(1, 500);
    let edge_limit = edge_limit.clamp(1, 2_000);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut node_groups = BTreeMap::new();
    let mut groups: BTreeMap<String, ArchitectureGroup> = BTreeMap::new();

    for node in &graph.nodes {
        if node.kind != NodeKind::File {
            continue;
        }
        let (id, label) = architecture_group_for_path(&node.label);
        let group = groups
            .entry(id.clone())
            .or_insert_with(|| ArchitectureGroup {
                id: id.clone(),
                label,
                files: 0,
                symbols: 0,
                entrypoints: 0,
                skipped_files: 0,
                languages: BTreeMap::new(),
            });
        group.files += 1;
        if node
            .metadata
            .get("skipped")
            .is_some_and(|value| value == "true")
        {
            group.skipped_files += 1;
        }
        if let Some(language) = node.metadata.get("language") {
            *group.languages.entry(language.clone()).or_insert(0) += 1;
        }
        node_groups.insert(node.id, id);
    }

    for edge in &graph.edges {
        if edge.kind != EdgeKind::Contains {
            continue;
        }
        let Some(source_group) = node_groups.get(&edge.source).cloned() else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target) else {
            continue;
        };
        node_groups.entry(target.id).or_insert(source_group);
    }

    for node in &graph.nodes {
        let Some(group_id) = node_groups.get(&node.id) else {
            continue;
        };
        let Some(group) = groups.get_mut(group_id) else {
            continue;
        };
        if is_architecture_symbol(&node.kind) {
            group.symbols += 1;
        }
        if node.kind == NodeKind::Entrypoint {
            group.entrypoints += 1;
        }
    }

    let mut edges: BTreeMap<(String, String), ArchitectureEdge> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind) {
            continue;
        }
        let Some(source_group) = node_groups.get(&edge.source) else {
            continue;
        };
        let Some(target_group) = node_groups.get(&edge.target) else {
            continue;
        };
        if source_group == target_group {
            continue;
        }
        let key = (source_group.clone(), target_group.clone());
        let architecture_edge = edges.entry(key).or_insert_with(|| ArchitectureEdge {
            source: source_group.clone(),
            target: target_group.clone(),
            count: 0,
            edge_kinds: BTreeMap::new(),
            confidences: BTreeMap::new(),
            edge_indexes: Vec::new(),
        });
        architecture_edge.count += 1;
        if architecture_edge.edge_indexes.len() < 100 {
            architecture_edge.edge_indexes.push(edge_index);
        }
        *architecture_edge
            .edge_kinds
            .entry(edge_kind_name(&edge.kind))
            .or_insert(0) += 1;
        *architecture_edge
            .confidences
            .entry(confidence_name(edge.confidence))
            .or_insert(0) += 1;
    }

    let total_groups = groups.len();
    let total_edges = edges.len();
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|left, right| {
        right
            .files
            .cmp(&left.files)
            .then_with(|| right.symbols.cmp(&left.symbols))
            .then_with(|| left.label.cmp(&right.label))
    });
    let mut edges: Vec<_> = edges.into_values().collect();
    edges.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
    });
    groups.truncate(group_limit);
    edges.truncate(edge_limit);

    ArchitectureMap {
        groups,
        edges,
        total_groups,
        total_edges,
        truncated_groups: total_groups > group_limit,
        truncated_edges: total_edges > edge_limit,
    }
}

pub fn language_dependencies(graph: &CodeGraph, limit: usize) -> LanguageDependencyReport {
    let limit = limit.clamp(1, 500);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut links: BTreeMap<(String, String), LanguageDependency> = BTreeMap::new();
    let mut total_edges = 0;
    let mut cross_language_edges = 0;

    for (edge_index, edge) in graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| is_architecture_dependency_edge(&edge.kind))
    {
        let source_language = node_language(&nodes_by_id, edge.source);
        let target_language = node_language(&nodes_by_id, edge.target);
        if source_language == "unknown" && target_language == "unknown" {
            continue;
        }
        total_edges += 1;
        if source_language != target_language {
            cross_language_edges += 1;
        }

        let link = links
            .entry((source_language.clone(), target_language.clone()))
            .or_insert_with(|| LanguageDependency {
                source_language: source_language.clone(),
                target_language: target_language.clone(),
                count: 0,
                edge_kinds: BTreeMap::new(),
                confidences: BTreeMap::new(),
                edge_indexes: Vec::new(),
            });
        link.count += 1;
        *link
            .edge_kinds
            .entry(edge_kind_name(&edge.kind))
            .or_insert(0) += 1;
        *link
            .confidences
            .entry(confidence_name(edge.confidence))
            .or_insert(0) += 1;
        link.edge_indexes.push(edge_index);
    }

    let total_links = links.len();
    let mut links: Vec<_> = links.into_values().collect();
    links.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.source_language.cmp(&right.source_language))
            .then_with(|| left.target_language.cmp(&right.target_language))
    });
    links.truncate(limit);

    LanguageDependencyReport {
        links,
        total_links,
        total_edges,
        cross_language_edges,
        truncated: total_links > limit,
    }
}

pub fn surprising_links(graph: &CodeGraph, limit: usize) -> SurprisingLinkReport {
    let limit = limit.clamp(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let node_areas = node_architecture_areas(graph, &nodes_by_id);
    let mut links = Vec::new();

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind) {
            continue;
        }
        let Some(source) = nodes_by_id.get(&edge.source) else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target) else {
            continue;
        };
        let source_area = node_areas
            .get(&edge.source)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let target_area = node_areas
            .get(&edge.target)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let source_language = node_language(&nodes_by_id, edge.source);
        let target_language = node_language(&nodes_by_id, edge.target);
        let mut score = 0;
        let mut reasons = Vec::new();

        if source_area != "unknown" && target_area != "unknown" && source_area != target_area {
            score += 5;
            reasons.push("cross_area".to_string());
        }
        if source_language != "unknown"
            && target_language != "unknown"
            && source_language != target_language
        {
            score += 4;
            reasons.push("cross_language".to_string());
        }
        match edge.confidence {
            Confidence::Heuristic => {
                score += 3;
                reasons.push("heuristic_confidence".to_string());
            }
            Confidence::Unknown => {
                score += 2;
                reasons.push("unknown_confidence".to_string());
            }
            Confidence::Semantic | Confidence::Syntactic | Confidence::Exact => {}
        }
        if matches!(
            edge.kind,
            EdgeKind::MayError
                | EdgeKind::ReadsConfig
                | EdgeKind::ReadsEnvironment
                | EdgeKind::DependsOn
        ) {
            score += 2;
            reasons.push(format!("edge_kind:{}", edge_kind_name(&edge.kind)));
        }
        if source.kind == NodeKind::Entrypoint || target.kind == NodeKind::Entrypoint {
            score += 1;
            reasons.push("entrypoint_boundary".to_string());
        }
        if score == 0 {
            continue;
        }

        links.push(SurprisingLink {
            source: (*source).clone(),
            target: (*target).clone(),
            source_area,
            target_area,
            source_language,
            target_language,
            edge_kind: edge_kind_name(&edge.kind),
            confidence: confidence_name(edge.confidence),
            score,
            reasons,
            edge_index,
        });
    }

    links.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.source_area.cmp(&right.source_area))
            .then_with(|| left.target_area.cmp(&right.target_area))
            .then_with(|| left.edge_index.cmp(&right.edge_index))
    });
    let total_candidates = links.len();
    links.truncate(limit);

    SurprisingLinkReport {
        links,
        total_candidates,
        truncated: total_candidates > limit,
    }
}

pub fn hotspots(graph: &CodeGraph, limit: usize) -> HotspotReport {
    let limit = limit.clamp(1, 500);
    let mut hotspots = hotspot_stats(graph, |_| true, NeighborDirection::Both);
    let total_candidates = hotspots.len();
    let architectural_candidates: Vec<_> = hotspots
        .iter()
        .filter(|hotspot| hotspot.hub_kind == "architectural")
        .cloned()
        .collect();
    let utility_candidates: Vec<_> = hotspots
        .iter()
        .filter(|hotspot| hotspot.hub_kind == "utility")
        .cloned()
        .collect();
    let total_architectural_hubs = architectural_candidates.len();
    let total_utility_hubs = utility_candidates.len();
    let mut architectural_hubs = architectural_candidates;
    let mut utility_hubs = utility_candidates;
    hotspots.truncate(limit);
    architectural_hubs.truncate(limit);
    utility_hubs.truncate(limit);

    HotspotReport {
        hotspots,
        architectural_hubs,
        utility_hubs,
        total_candidates,
        total_architectural_hubs,
        total_utility_hubs,
        truncated: total_candidates > limit,
    }
}

pub fn communities(graph: &CodeGraph, limit: usize) -> CommunityReport {
    let limit = limit.clamp(1, MAX_REPORT_COMMUNITY_LIMIT);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut node_community: BTreeMap<NodeId, String> = BTreeMap::new();
    let mut components: BTreeMap<String, BTreeSet<NodeId>> = BTreeMap::new();

    for node in graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
    {
        let (group_id, _) = architecture_group_for_path(&node.label);
        let community_id = format!("area:{group_id}");
        node_community.insert(node.id, community_id.clone());
        components.entry(community_id).or_default().insert(node.id);
    }

    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Contains)
    {
        let Some(source_community) = node_community.get(&edge.source).cloned() else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target) else {
            continue;
        };
        if is_architecture_symbol(&target.kind) {
            node_community.insert(edge.target, source_community.clone());
            components
                .entry(source_community)
                .or_default()
                .insert(edge.target);
        }
    }

    for node in graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File || is_architecture_symbol(&node.kind))
    {
        if node_community.contains_key(&node.id) {
            continue;
        }
        let community_id = format!("isolated:{}", node.id.0);
        node_community.insert(node.id, community_id.clone());
        components.entry(community_id).or_default().insert(node.id);
    }

    let mut communities: Vec<_> = components
        .into_iter()
        .map(|(community_id, component)| {
            graph_community(
                graph,
                &nodes_by_id,
                &node_community,
                community_id,
                component,
            )
        })
        .collect();
    let total_communities = communities.len();
    let total_nodes = communities
        .iter()
        .map(|community| community.node_count)
        .sum();
    let total_internal_edges = communities
        .iter()
        .map(|community| community.internal_edges)
        .sum();
    let total_external_edges = graph
        .edges
        .iter()
        .filter(|edge| is_community_report_edge(edge))
        .filter_map(|edge| {
            let source_community = node_community.get(&edge.source)?;
            let target_community = node_community.get(&edge.target)?;
            (source_community != target_community).then_some(())
        })
        .count();
    communities.sort_by(|left, right| {
        right
            .node_count
            .cmp(&left.node_count)
            .then_with(|| right.internal_edges.cmp(&left.internal_edges))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });
    communities.truncate(limit);

    CommunityReport {
        communities,
        total_communities,
        total_nodes,
        total_internal_edges,
        total_external_edges,
        truncated: total_communities > limit,
    }
}

fn graph_community(
    graph: &CodeGraph,
    nodes_by_id: &BTreeMap<NodeId, &Node>,
    node_community: &BTreeMap<NodeId, String>,
    community_id: String,
    component: BTreeSet<NodeId>,
) -> GraphCommunity {
    let mut files = 0;
    let mut entrypoints = 0;
    let mut languages = BTreeMap::new();
    let mut node_kinds = BTreeMap::new();
    let mut area_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut nodes: Vec<Node> = component
        .iter()
        .filter_map(|id| nodes_by_id.get(id).map(|node| (*node).clone()))
        .collect();
    nodes.sort_by(|left, right| {
        node_rank(&left.kind)
            .cmp(&node_rank(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });

    for node in &nodes {
        if node.kind == NodeKind::File {
            files += 1;
            let (_, area) = architecture_group_for_path(&node.label);
            *area_counts.entry(area).or_insert(0) += 1;
        }
        if node.kind == NodeKind::Entrypoint {
            entrypoints += 1;
        }
        if let Some(language) = node
            .metadata
            .get("language")
            .map(String::as_str)
            .filter(|language| !language.trim().is_empty())
        {
            *languages.entry(language.to_string()).or_insert(0) += 1;
        }
        *node_kinds.entry(kind_name(&node.kind)).or_insert(0) += 1;
    }

    let mut internal_edges = 0;
    let mut incoming_external_edges = 0;
    let mut outgoing_external_edges = 0;
    let mut edge_indexes = Vec::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_community_report_edge(edge) {
            continue;
        }
        let source_inside = component.contains(&edge.source);
        let target_inside = component.contains(&edge.target);
        match (source_inside, target_inside) {
            (true, true) => {
                internal_edges += 1;
                if edge_indexes.len() < 100 {
                    edge_indexes.push(edge_index);
                }
            }
            (true, false) if node_community.contains_key(&edge.target) => {
                outgoing_external_edges += 1;
                if edge_indexes.len() < 100 {
                    edge_indexes.push(edge_index);
                }
            }
            (false, true) if node_community.contains_key(&edge.source) => {
                incoming_external_edges += 1;
                if edge_indexes.len() < 100 {
                    edge_indexes.push(edge_index);
                }
            }
            _ => {}
        }
    }

    let label = community_label(&area_counts, &languages, nodes.first());
    GraphCommunity {
        id: community_id,
        label: if label.is_empty() {
            "Community".to_string()
        } else {
            label
        },
        node_count: nodes.len(),
        files,
        entrypoints,
        internal_edges,
        incoming_external_edges,
        outgoing_external_edges,
        languages,
        node_kinds,
        sample_nodes: nodes.into_iter().take(8).collect(),
        edge_indexes,
    }
}

fn is_community_report_edge(edge: &Edge) -> bool {
    edge.kind == EdgeKind::Contains || is_architecture_dependency_edge(&edge.kind)
}

fn community_label(
    area_counts: &BTreeMap<String, usize>,
    languages: &BTreeMap<String, usize>,
    first_node: Option<&Node>,
) -> String {
    if let Some((area, _)) = area_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
    {
        return area.clone();
    }
    if let Some((language, _)) = languages
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
    {
        return format!("{language} symbols");
    }
    first_node
        .map(|node| node.label.clone())
        .unwrap_or_else(|| "Community".to_string())
}

fn node_rank(kind: &NodeKind) -> usize {
    match kind {
        NodeKind::Entrypoint => 0,
        NodeKind::File => 1,
        NodeKind::Module => 2,
        NodeKind::Type => 3,
        NodeKind::Function => 4,
        NodeKind::Config => 5,
        NodeKind::Environment => 6,
        NodeKind::ExternalDependency => 7,
        NodeKind::Directory => 8,
        NodeKind::Repository => 9,
        NodeKind::Unknown => 10,
    }
}

fn hotspot_stats<F>(graph: &CodeGraph, edge_filter: F, direction: NeighborDirection) -> Vec<Hotspot>
where
    F: Fn(&Edge) -> bool,
{
    let candidate_ids: BTreeSet<_> = graph
        .nodes
        .iter()
        .filter(|node| is_hotspot_candidate(&node.kind))
        .map(|node| node.id)
        .collect();
    let nodes_by_id: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut stats: BTreeMap<NodeId, Hotspot> = BTreeMap::new();

    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.kind != EdgeKind::Contains && edge_filter(edge))
    {
        if direction != NeighborDirection::In
            && candidate_ids.contains(&edge.source)
            && let Some(node) = nodes_by_id.get(&edge.source)
        {
            let hotspot = stats.entry(edge.source).or_insert_with(|| Hotspot {
                node: (*node).clone(),
                score: 0,
                incoming: 0,
                outgoing: 0,
                edge_kinds: BTreeMap::new(),
                hub_kind: String::new(),
            });
            hotspot.outgoing += 1;
            hotspot.score += 1;
            *hotspot
                .edge_kinds
                .entry(edge_kind_name(&edge.kind))
                .or_insert(0) += 1;
        }
        if direction != NeighborDirection::Out
            && candidate_ids.contains(&edge.target)
            && let Some(node) = nodes_by_id.get(&edge.target)
        {
            let hotspot = stats.entry(edge.target).or_insert_with(|| Hotspot {
                node: (*node).clone(),
                score: 0,
                incoming: 0,
                outgoing: 0,
                edge_kinds: BTreeMap::new(),
                hub_kind: String::new(),
            });
            hotspot.incoming += 1;
            hotspot.score += 1;
            *hotspot
                .edge_kinds
                .entry(edge_kind_name(&edge.kind))
                .or_insert(0) += 1;
        }
    }

    let mut hotspots: Vec<_> = stats.into_values().collect();
    for hotspot in &mut hotspots {
        hotspot.hub_kind = hotspot_kind(hotspot).to_string();
    }
    hotspots.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.incoming.cmp(&left.incoming))
            .then_with(|| right.outgoing.cmp(&left.outgoing))
            .then_with(|| left.node.label.cmp(&right.node.label))
    });
    hotspots
}

fn hotspot_kind(hotspot: &Hotspot) -> &'static str {
    if is_utility_hotspot(hotspot) {
        "utility"
    } else {
        "architectural"
    }
}

fn is_utility_hotspot(hotspot: &Hotspot) -> bool {
    let label = hotspot.node.label.trim();
    let normalized = label.trim_matches('_').to_ascii_lowercase();
    if matches!(
        hotspot.node.kind,
        NodeKind::Config
            | NodeKind::Environment
            | NodeKind::File
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Module
    ) {
        return false;
    }
    if matches!(
        normalized.as_str(),
        "new"
            | "default"
            | "clone"
            | "copy"
            | "drop"
            | "fmt"
            | "debug"
            | "to_string"
            | "to_owned"
            | "into"
            | "from"
            | "as_ref"
            | "as_mut"
            | "unwrap"
            | "expect"
            | "ok"
            | "err"
            | "some"
            | "none"
            | "get"
            | "set"
            | "len"
            | "is_empty"
            | "map"
            | "and_then"
            | "or_else"
            | "parse"
            | "open"
            | "read"
            | "write"
    ) {
        return true;
    }
    if normalized.len() <= 2 && hotspot.score >= 4 {
        return true;
    }
    if normalized.starts_with("get_") || normalized.starts_with("set_") {
        return true;
    }
    hotspot.node.kind == NodeKind::ExternalDependency
        && hotspot
            .node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| kind == "call")
        && hotspot
            .node
            .metadata
            .get("resolution")
            .is_some_and(|resolution| resolution == "unresolved")
}

pub fn entrypoints(graph: &CodeGraph) -> Vec<Node> {
    let mut ids = BTreeSet::new();
    for edge in &graph.edges {
        if edge.kind == EdgeKind::Entrypoint {
            ids.insert(edge.target);
        }
    }

    graph
        .nodes
        .iter()
        .filter(|node| ids.contains(&node.id))
        .cloned()
        .collect()
}

pub fn insights(graph: &CodeGraph) -> InsightReport {
    let mut insights = Vec::new();
    add_parse_error_insights(graph, &mut insights);
    add_semantic_diagnostic_insights(graph, &mut insights);
    add_unresolved_call_insights(graph, &mut insights);
    add_ambiguous_call_resolution_insights(graph, &mut insights);
    add_unresolved_local_import_insights(graph, &mut insights);
    add_unresolved_sql_table_reference_insights(graph, &mut insights);
    add_cross_language_heuristic_edge_insights(graph, &mut insights);
    add_duplicate_function_insights(graph, &mut insights);
    add_duplicate_compose_published_port_insights(graph, &mut insights);
    add_duplicate_entrypoint_insights(graph, &mut insights);
    add_ambiguous_entrypoint_target_insights(graph, &mut insights);
    add_orphan_function_insights(graph, &mut insights);
    add_error_flow_insights(graph, &mut insights);
    add_unresolved_entrypoint_insights(graph, &mut insights);
    add_unresolved_compose_command_path_insights(graph, &mut insights);
    add_unresolved_compose_env_file_path_insights(graph, &mut insights);
    add_unresolved_compose_volume_source_path_insights(graph, &mut insights);
    add_unresolved_github_actions_job_need_insights(graph, &mut insights);
    add_unresolved_github_actions_local_action_insights(graph, &mut insights);
    add_unresolved_github_actions_run_path_insights(graph, &mut insights);
    add_unresolved_gitlab_ci_job_dependency_insights(graph, &mut insights);
    add_unresolved_gitlab_ci_script_path_insights(graph, &mut insights);
    add_unresolved_kubernetes_config_ref_insights(graph, &mut insights);
    add_unresolved_kubernetes_ingress_backend_insights(graph, &mut insights);
    add_unresolved_kubernetes_service_selector_insights(graph, &mut insights);
    add_unresolved_dockerfile_command_path_insights(graph, &mut insights);
    add_unresolved_makefile_command_path_insights(graph, &mut insights);
    add_entrypoint_dead_end_insights(graph, &mut insights);
    add_unreachable_config_read_insights(graph, &mut insights);
    add_unreachable_error_flow_insights(graph, &mut insights);
    add_unreachable_source_file_insights(graph, &mut insights);
    add_conflicting_config_default_insights(graph, &mut insights);
    add_mixed_config_requirement_insights(graph, &mut insights);
    add_undeclared_flutter_asset_insights(graph, &mut insights);
    add_rationale_risk_comment_insights(graph, &mut insights);
    add_sensitive_ci_environment_literal_insights(graph, &mut insights);
    add_sensitive_config_default_insights(graph, &mut insights);
    add_undeclared_import_insights(graph, &mut insights);
    add_unused_dependency_insights(graph, &mut insights);
    add_conflicting_dependency_insights(graph, &mut insights);
    add_mixed_dependency_scope_insights(graph, &mut insights);
    add_non_runtime_dependency_import_insights(graph, &mut insights);
    add_test_only_runtime_dependency_insights(graph, &mut insights);
    add_unresolved_framework_route_handler_insights(graph, &mut insights);
    add_duplicate_framework_route_insights(graph, &mut insights);
    add_custom_rule_violation_insights(graph, &mut insights);
    add_dependency_cycle_insights(graph, &mut insights);
    insights.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.message.cmp(&right.message))
    });

    let mut by_severity = BTreeMap::new();
    let mut by_kind = BTreeMap::new();
    for insight in &insights {
        *by_severity
            .entry(severity_name(insight.severity).to_string())
            .or_insert(0) += 1;
        *by_kind.entry(insight.kind.clone()).or_insert(0) += 1;
    }

    InsightReport {
        total: insights.len(),
        by_severity,
        by_kind,
        insights,
    }
}

pub fn filter_insight_report(report: InsightReport, filter: &InsightFilter) -> InsightReport {
    let kind = filter.kind.as_ref().map(|value| value.to_ascii_lowercase());
    let search = filter
        .search
        .as_ref()
        .map(|value| value.to_ascii_lowercase());
    let mut insights: Vec<_> = report
        .insights
        .into_iter()
        .filter(|insight| {
            filter
                .severity
                .is_none_or(|expected| insight.severity == expected)
                && kind
                    .as_deref()
                    .is_none_or(|expected| insight.kind.to_ascii_lowercase().contains(expected))
                && search
                    .as_deref()
                    .is_none_or(|expected| insight_search_matches(insight, expected))
        })
        .collect();
    let total = insights.len();
    let (by_severity, by_kind) = insight_breakdowns(&insights);
    insights.truncate(filter.limit.clamp(1, 500));

    InsightReport {
        total,
        by_severity,
        by_kind,
        insights,
    }
}

pub fn check_insights(report: InsightReport, fail_on: InsightSeverity) -> CheckReport {
    let failing_insights = failing_insight_count(&report, fail_on);
    CheckReport {
        passed: failing_insights == 0,
        fail_on: severity_name(fail_on).to_string(),
        failing_insights,
        report,
    }
}

pub fn project_report(graph: &CodeGraph, limits: ProjectReportLimits) -> ProjectReport {
    let limits = normalize_project_report_limits(limits);
    let full_insight_report = insights(graph);
    let risk_summary = project_risk_summary(&full_insight_report);
    let quality_gate = check_insights(full_insight_report.clone(), limits.fail_on);
    let file_summaries =
        compact_file_summaries(graph, &full_insight_report, limits.file_summary_limit);
    let node_summaries =
        compact_node_summaries(graph, &full_insight_report, limits.node_summary_limit);
    let insight_report = filter_insight_report(
        full_insight_report,
        &InsightFilter {
            severity: None,
            kind: None,
            search: None,
            limit: limits.insight_limit,
        },
    );

    ProjectReport {
        graph_schema_version: graph.schema_version,
        summary: summarize(graph),
        entrypoints: entrypoints(graph),
        insights: insight_report,
        risk_summary,
        quality_gate,
        architecture: architecture_map(
            graph,
            limits.architecture_group_limit,
            limits.architecture_edge_limit,
        ),
        language_dependencies: language_dependencies(graph, limits.language_link_limit),
        surprising_links: surprising_links(graph, limits.architecture_edge_limit),
        hotspots: hotspots(graph, limits.hotspot_limit),
        communities: communities(graph, limits.community_limit),
        file_summaries,
        node_summaries,
    }
}

pub fn project_report_markdown(
    report: &ProjectReport,
    options: &ProjectReportMarkdownOptions,
) -> String {
    let mut output = String::new();
    writeln!(output, "# {}", markdown_text(&options.title)).unwrap();
    writeln!(output).unwrap();
    if let Some(root) = options.root.as_deref().filter(|value| !value.is_empty()) {
        writeln!(output, "- Root: `{}`", markdown_code(root)).unwrap();
    }
    if let Some(generated_at_unix) = options.generated_at_unix {
        writeln!(output, "- Generated at unix: `{generated_at_unix}`").unwrap();
    }
    writeln!(
        output,
        "- Graph schema version: `{}`",
        report.graph_schema_version
    )
    .unwrap();
    writeln!(
        output,
        "- Quality gate: **{}** (`fail_on={}`, failing_insights={})",
        if report.quality_gate.passed {
            "passed"
        } else {
            "failed"
        },
        markdown_code(&report.quality_gate.fail_on),
        report.quality_gate.failing_insights
    )
    .unwrap();
    writeln!(
        output,
        "- Risk: **{}** (score {}, total {}, errors {}, warnings {}, infos {})",
        markdown_text(&report.risk_summary.grade),
        report.risk_summary.score,
        report.risk_summary.total,
        report.risk_summary.errors,
        report.risk_summary.warnings,
        report.risk_summary.infos
    )
    .unwrap();

    writeln!(output, "\n## Summary").unwrap();
    writeln!(output, "| Metric | Count |").unwrap();
    writeln!(output, "| --- | ---: |").unwrap();
    writeln!(output, "| Nodes | {} |", report.summary.nodes).unwrap();
    writeln!(output, "| Edges | {} |", report.summary.edges).unwrap();
    writeln!(output, "| Entrypoints | {} |", report.summary.entrypoints).unwrap();
    writeln!(
        output,
        "| Skipped files | {} |",
        report.summary.skipped_files
    )
    .unwrap();

    write_count_table(
        &mut output,
        "Languages",
        "Language",
        &report.summary.languages,
        12,
    );
    write_count_table(
        &mut output,
        "Node Kinds",
        "Kind",
        &report.summary.node_kinds,
        12,
    );
    write_count_table(
        &mut output,
        "Edge Confidence",
        "Confidence",
        &report.summary.edge_confidences,
        12,
    );

    writeln!(output, "\n## Compact Node Summaries").unwrap();
    if report.node_summaries.nodes.is_empty() {
        writeln!(output, "No node summaries were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | Node | Kind | Roles | In | Out | Risks | Edge kinds | Source |"
        )
        .unwrap();
        writeln!(
            output,
            "| ---: | --- | --- | --- | ---: | ---: | --- | --- | --- |"
        )
        .unwrap();
        for node in report.node_summaries.nodes.iter().take(20) {
            writeln!(
                output,
                "| {} | {} | `{}` | {} | {} | {} | {} | {} | {} |",
                node.score,
                node_ref(&node.node),
                markdown_code(&kind_name(&node.node.kind)),
                markdown_table_cell(&node.roles.join(", ")),
                node.dependency_summary.incoming,
                node.dependency_summary.outgoing,
                count_map_inline(&node.insight_summary.by_severity, 4),
                count_map_inline(&node.dependency_summary.edge_kinds, 5),
                node_span_ref(&node.node)
            )
            .unwrap();
        }
        if report.node_summaries.truncated {
            writeln!(
                output,
                "\nNode summaries are truncated: showing {} of {} important nodes.",
                report.node_summaries.nodes.len(),
                report.node_summaries.total_nodes
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Compact File Summaries").unwrap();
    if report.file_summaries.files.is_empty() {
        writeln!(output, "No file summaries were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | File | Symbols | Trace | Imports | Config | Env | Errors | Unresolved | Risks | Trace kinds |"
        )
        .unwrap();
        writeln!(
            output,
            "| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |"
        )
        .unwrap();
        for file in report.file_summaries.files.iter().take(20) {
            writeln!(
                output,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                file.score,
                node_ref(&file.node),
                file.summary.code_symbols,
                file.summary.trace_edges,
                file.summary.imports,
                file.summary.config_reads,
                file.summary.environment_reads,
                file.summary.error_facts,
                file.summary.unresolved_calls,
                count_map_inline(&file.insight_summary.by_severity, 4),
                count_map_inline(&file.summary.trace_edge_kinds, 5)
            )
            .unwrap();
        }
        if report.file_summaries.truncated {
            writeln!(
                output,
                "\nFile summaries are truncated: showing {} of {} files.",
                report.file_summaries.files.len(),
                report.file_summaries.total_files
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Confidence Guide").unwrap();
    writeln!(
        output,
        "| CodeGraph confidence | Report wording | How to read it |"
    )
    .unwrap();
    writeln!(output, "| --- | --- | --- |").unwrap();
    writeln!(
        output,
        "| `exact` | extracted | Exact project metadata, deterministic manifests, or compiler-like facts. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `semantic` | resolved | Resolved through a semantic analyzer or language server. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `syntactic` | extracted from syntax | Extracted directly from source syntax. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `heuristic` | inferred | Inferred by a named rule and worth reviewing at architectural or runtime boundaries. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `unknown` | ambiguous | Legacy, imported, or ambiguous evidence. |"
    )
    .unwrap();

    writeln!(output, "\n## Key Concepts").unwrap();
    let key_hotspots = if report.hotspots.architectural_hubs.is_empty() {
        &report.hotspots.hotspots
    } else {
        &report.hotspots.architectural_hubs
    };
    if key_hotspots.is_empty() {
        writeln!(output, "No architectural hub candidates were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | Node | Kind | Hub kind | In | Out | Edge kinds |"
        )
        .unwrap();
        writeln!(output, "| ---: | --- | --- | --- | ---: | ---: | --- |").unwrap();
        for hotspot in key_hotspots.iter().take(15) {
            writeln!(
                output,
                "| {} | {} | `{}` | `{}` | {} | {} | {} |",
                hotspot.score,
                node_ref(&hotspot.node),
                markdown_code(&kind_name(&hotspot.node.kind)),
                markdown_code(&hotspot.hub_kind),
                hotspot.incoming,
                hotspot.outgoing,
                count_map_inline(&hotspot.edge_kinds, 6)
            )
            .unwrap();
        }
        if report.hotspots.truncated {
            writeln!(
                output,
                "\nHotspots are truncated: showing {} key hubs out of {} candidates ({} architectural, {} utility).",
                key_hotspots.len(),
                report.hotspots.total_candidates,
                report.hotspots.total_architectural_hubs,
                report.hotspots.total_utility_hubs
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Communities").unwrap();
    if report.communities.communities.is_empty() {
        writeln!(output, "No graph communities were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Community | Nodes | Files | Entrypoints | Internal edges | External edges | Languages | Evidence |"
        )
        .unwrap();
        writeln!(
            output,
            "| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |"
        )
        .unwrap();
        for community in report.communities.communities.iter().take(12) {
            let external_edges =
                community.incoming_external_edges + community.outgoing_external_edges;
            writeln!(
                output,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
                markdown_code(&community.label),
                community.node_count,
                community.files,
                community.entrypoints,
                community.internal_edges,
                external_edges,
                count_map_inline(&community.languages, 5),
                edge_index_refs(&community.edge_indexes, 5)
            )
            .unwrap();
        }
        if report.communities.truncated {
            writeln!(
                output,
                "\nCommunities are truncated: showing {} of {} communities.",
                report.communities.communities.len(),
                report.communities.total_communities
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Entrypoints").unwrap();
    if report.entrypoints.is_empty() {
        writeln!(output, "No entrypoint candidates were found.").unwrap();
    } else {
        writeln!(output, "| Node | Kind | Source |").unwrap();
        writeln!(output, "| --- | --- | --- |").unwrap();
        for node in report.entrypoints.iter().take(20) {
            writeln!(
                output,
                "| {} | `{}` | {} |",
                node_ref(node),
                markdown_code(&kind_name(&node.kind)),
                node_span_ref(node)
            )
            .unwrap();
        }
        if report.entrypoints.len() > 20 {
            writeln!(
                output,
                "\nEntrypoints are truncated: showing 20 of {}.",
                report.entrypoints.len()
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Architecture Links").unwrap();
    if report.architecture.edges.is_empty() {
        writeln!(output, "No cross-area architecture links were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Source | Target | Count | Edge kinds | Confidence | Evidence |"
        )
        .unwrap();
        writeln!(output, "| --- | --- | ---: | --- | --- | --- |").unwrap();
        for edge in report.architecture.edges.iter().take(15) {
            writeln!(
                output,
                "| `{}` | `{}` | {} | {} | {} | {} |",
                markdown_code(&edge.source),
                markdown_code(&edge.target),
                edge.count,
                count_map_inline(&edge.edge_kinds, 5),
                count_map_inline(&edge.confidences, 5),
                edge_index_refs(&edge.edge_indexes, 5)
            )
            .unwrap();
        }
        if report.architecture.truncated_edges {
            writeln!(
                output,
                "\nArchitecture links are truncated: showing {} of {} links.",
                report.architecture.edges.len(),
                report.architecture.total_edges
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Surprising Links").unwrap();
    if report.surprising_links.links.is_empty() {
        writeln!(output, "No surprising dependency links were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | Source | Target | Areas | Languages | Edge | Confidence | Reasons | Evidence |"
        )
        .unwrap();
        writeln!(
            output,
            "| ---: | --- | --- | --- | --- | --- | --- | --- | --- |"
        )
        .unwrap();
        for link in report.surprising_links.links.iter().take(15) {
            writeln!(
                output,
                "| {} | {} | {} | `{}` -> `{}` | `{}` -> `{}` | `{}` | `{}` | {} | #{} |",
                link.score,
                node_ref(&link.source),
                node_ref(&link.target),
                markdown_code(&link.source_area),
                markdown_code(&link.target_area),
                markdown_code(&link.source_language),
                markdown_code(&link.target_language),
                markdown_code(&link.edge_kind),
                markdown_code(&link.confidence),
                markdown_table_cell(&link.reasons.join(", ")),
                link.edge_index
            )
            .unwrap();
        }
        if report.surprising_links.truncated {
            writeln!(
                output,
                "\nSurprising links are truncated: showing {} of {} candidates.",
                report.surprising_links.links.len(),
                report.surprising_links.total_candidates
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Risks And Insights").unwrap();
    if report.risk_summary.top_kinds.is_empty() {
        writeln!(output, "No investigation insights were reported.").unwrap();
    } else {
        writeln!(output, "| Kind | Severity | Count |").unwrap();
        writeln!(output, "| --- | --- | ---: |").unwrap();
        for risk in &report.risk_summary.top_kinds {
            writeln!(
                output,
                "| `{}` | `{}` | {} |",
                markdown_code(&risk.kind),
                markdown_code(&risk.severity),
                risk.count
            )
            .unwrap();
        }
    }
    if !report.insights.insights.is_empty() {
        writeln!(output, "\n### Insight Evidence").unwrap();
        writeln!(output, "| Severity | Kind | Message | Evidence |").unwrap();
        writeln!(output, "| --- | --- | --- | --- |").unwrap();
        for insight in report.insights.insights.iter().take(20) {
            let node_refs = insight
                .nodes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let evidence = if insight.edges.is_empty() {
                markdown_table_cell(&node_refs)
            } else if node_refs.is_empty() {
                edge_index_refs(&insight.edges, 8)
            } else {
                markdown_table_cell(&format!(
                    "nodes: {node_refs}; edges: {}",
                    edge_index_refs(&insight.edges, 8)
                ))
            };
            writeln!(
                output,
                "| `{}` | `{}` | {} | {} |",
                markdown_code(severity_name(insight.severity)),
                markdown_code(&insight.kind),
                markdown_table_cell(&insight.message),
                evidence
            )
            .unwrap();
        }
        if report.insights.insights.len() < report.insights.total {
            writeln!(
                output,
                "\nInsights are truncated: showing {} of {}.",
                report.insights.insights.len(),
                report.insights.total
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Suggested Questions").unwrap();
    for question in project_report_suggested_questions(report).iter().take(6) {
        writeln!(output, "- {}", markdown_text(question)).unwrap();
    }

    output
}

fn write_count_table(
    output: &mut String,
    title: &str,
    label: &str,
    values: &BTreeMap<String, usize>,
    limit: usize,
) {
    writeln!(output, "\n### {}", markdown_text(title)).unwrap();
    if values.is_empty() {
        writeln!(output, "No {} values were found.", title.to_lowercase()).unwrap();
        return;
    }
    writeln!(output, "| {} | Count |", markdown_table_cell(label)).unwrap();
    writeln!(output, "| --- | ---: |").unwrap();
    for (key, count) in sorted_count_entries(values).into_iter().take(limit) {
        writeln!(output, "| `{}` | {} |", markdown_code(key), count).unwrap();
    }
    if values.len() > limit {
        writeln!(output, "\nShowing {} of {} values.", limit, values.len()).unwrap();
    }
}

fn sorted_count_entries(values: &BTreeMap<String, usize>) -> Vec<(&String, &usize)> {
    let mut entries: Vec<_> = values.iter().collect();
    entries.sort_by(|(left_key, left_count), (right_key, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_key.cmp(right_key))
    });
    entries
}

fn count_map_inline(values: &BTreeMap<String, usize>, limit: usize) -> String {
    if values.is_empty() {
        return "-".to_string();
    }
    let mut parts: Vec<_> = sorted_count_entries(values)
        .into_iter()
        .take(limit)
        .map(|(key, count)| format!("`{}`={count}", markdown_code(key)))
        .collect();
    if values.len() > limit {
        parts.push(format!("+{} more", values.len() - limit));
    }
    markdown_table_cell(&parts.join(", "))
}

fn edge_index_refs(indexes: &[usize], limit: usize) -> String {
    if indexes.is_empty() {
        return "-".to_string();
    }
    let mut refs: Vec<_> = indexes
        .iter()
        .take(limit)
        .map(|index| format!("#{index}"))
        .collect();
    if indexes.len() > limit {
        refs.push(format!("+{} more", indexes.len() - limit));
    }
    markdown_table_cell(&refs.join(", "))
}

fn node_ref(node: &Node) -> String {
    markdown_table_cell(&format!("`{}` `{}`", node.id, markdown_code(&node.label)))
}

fn node_span_ref(node: &Node) -> String {
    node.span
        .as_ref()
        .map(|span| {
            markdown_table_cell(&format!(
                "`{}:{}-{}`",
                markdown_code(&span.path),
                span.start_line,
                span.end_line
            ))
        })
        .unwrap_or_else(|| "-".to_string())
}

fn project_report_suggested_questions(report: &ProjectReport) -> Vec<String> {
    let mut questions = Vec::new();
    if let Some(entrypoint) = report.entrypoints.first() {
        questions.push(format!(
            "What startup flow is reachable from {}?",
            entrypoint.label
        ));
    }
    if let Some(hotspot) = report
        .hotspots
        .architectural_hubs
        .first()
        .or_else(|| report.hotspots.hotspots.first())
    {
        questions.push(format!(
            "Why is {} a central graph hotspot?",
            hotspot.node.label
        ));
    }
    if let Some(community) = report.communities.communities.first() {
        questions.push(format!(
            "What responsibilities and external dependencies does the {} community have?",
            community.label
        ));
    }
    if let Some(edge) = report.architecture.edges.first() {
        questions.push(format!(
            "What evidence explains the architecture link from {} to {}?",
            edge.source, edge.target
        ));
    }
    if let Some(link) = report.surprising_links.links.first() {
        questions.push(format!(
            "Why is the {} edge from {} to {} surprising?",
            link.edge_kind, link.source.label, link.target.label
        ));
    }
    if let Some(risk) = report.risk_summary.top_kinds.first() {
        questions.push(format!(
            "Which code paths are involved in {} findings?",
            risk.kind
        ));
    }
    questions.push(
        "Which low-confidence or heuristic edges should be reviewed before changing shared code?"
            .to_string(),
    );
    questions
}

fn markdown_text(value: &str) -> String {
    value
        .replace('\n', " ")
        .replace('\r', " ")
        .trim()
        .to_string()
}

fn markdown_table_cell(value: &str) -> String {
    let value = markdown_text(value);
    let value = value.replace('|', "\\|");
    if value.is_empty() {
        "-".to_string()
    } else {
        value
    }
}

fn markdown_code(value: &str) -> String {
    markdown_table_cell(&value.replace('`', "'"))
}

fn project_risk_summary(report: &InsightReport) -> ProjectRiskSummary {
    let errors = severity_count(report, InsightSeverity::Error);
    let warnings = severity_count(report, InsightSeverity::Warning);
    let infos = severity_count(report, InsightSeverity::Info);
    let score = errors * 100 + warnings * 10 + infos;
    let mut kind_severities: BTreeMap<String, InsightSeverity> = BTreeMap::new();
    for insight in &report.insights {
        kind_severities
            .entry(insight.kind.clone())
            .and_modify(|severity| *severity = (*severity).max(insight.severity))
            .or_insert(insight.severity);
    }
    let mut top_kinds: Vec<_> = report
        .by_kind
        .iter()
        .map(|(kind, count)| ProjectRiskKindSummary {
            kind: kind.clone(),
            severity: severity_name(
                kind_severities
                    .get(kind)
                    .copied()
                    .unwrap_or(InsightSeverity::Info),
            )
            .to_string(),
            count: *count,
        })
        .collect();
    top_kinds.sort_by(|left, right| {
        parse_report_severity(&right.severity)
            .cmp(&parse_report_severity(&left.severity))
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    top_kinds.truncate(10);

    ProjectRiskSummary {
        score,
        grade: risk_grade(score).to_string(),
        total: report.total,
        errors,
        warnings,
        infos,
        top_kinds,
    }
}

fn severity_count(report: &InsightReport, severity: InsightSeverity) -> usize {
    report
        .by_severity
        .get(severity_name(severity))
        .copied()
        .unwrap_or(0)
}

fn risk_grade(score: usize) -> &'static str {
    match score {
        0 => "clean",
        1..=19 => "low",
        20..=99 => "medium",
        100..=499 => "high",
        _ => "critical",
    }
}

fn normalize_project_report_limits(limits: ProjectReportLimits) -> ProjectReportLimits {
    ProjectReportLimits {
        architecture_group_limit: limits
            .architecture_group_limit
            .clamp(1, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT),
        architecture_edge_limit: limits
            .architecture_edge_limit
            .clamp(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT),
        language_link_limit: limits
            .language_link_limit
            .clamp(1, MAX_REPORT_LANGUAGE_LINK_LIMIT),
        hotspot_limit: limits.hotspot_limit.clamp(1, MAX_REPORT_HOTSPOT_LIMIT),
        community_limit: limits.community_limit.clamp(1, MAX_REPORT_COMMUNITY_LIMIT),
        insight_limit: limits.insight_limit.clamp(1, MAX_REPORT_INSIGHT_LIMIT),
        file_summary_limit: limits
            .file_summary_limit
            .clamp(1, MAX_REPORT_FILE_SUMMARY_LIMIT),
        node_summary_limit: limits
            .node_summary_limit
            .clamp(1, MAX_REPORT_NODE_SUMMARY_LIMIT),
        fail_on: limits.fail_on,
    }
}

fn compact_file_summaries(
    graph: &CodeGraph,
    insight_report: &InsightReport,
    limit: usize,
) -> ProjectCompactFileSummaryReport {
    let path_index = node_path_index(graph);
    let mut files: Vec<ProjectCompactFileSummary> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .filter_map(|node| {
            let summary = file_node_summary(graph, node)?;
            let related_insights: Vec<Insight> = insight_report
                .insights
                .iter()
                .filter(|insight| {
                    node_card_insight_matches(graph, node, Some(&path_index), insight)
                })
                .cloned()
                .collect();
            let insight_summary = node_insight_summary(&related_insights);
            let score = compact_file_summary_score(&summary, &insight_summary);
            Some(ProjectCompactFileSummary {
                node: node.clone(),
                summary,
                insight_summary,
                score,
            })
        })
        .collect();

    files.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.node.label.cmp(&right.node.label))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    let total_files = files.len();
    let truncated = files.len() > limit;
    files.truncate(limit);

    ProjectCompactFileSummaryReport {
        files,
        total_files,
        truncated,
    }
}

fn compact_file_summary_score(
    summary: &FileNodeSummary,
    insight_summary: &NodeInsightSummary,
) -> usize {
    let risk_score: usize = insight_summary
        .by_severity
        .iter()
        .map(|(severity, count)| match severity.as_str() {
            "error" => *count * 100,
            "warning" => *count * 40,
            "info" => *count * 10,
            _ => *count,
        })
        .sum();

    risk_score
        + summary.trace_edges * 10
        + summary.unresolved_calls * 20
        + summary.error_facts * 20
        + summary.config_reads * 8
        + summary.environment_reads * 8
        + summary.code_symbols * 5
        + summary.direct_dependencies * 3
        + summary.imports
}

fn compact_node_summaries(
    graph: &CodeGraph,
    insight_report: &InsightReport,
    limit: usize,
) -> ProjectCompactNodeSummaryReport {
    let mut nodes: Vec<ProjectCompactNodeSummary> = graph
        .nodes
        .iter()
        .filter(|node| compact_node_summary_candidate(&node.kind))
        .filter_map(|node| {
            let dependency_summary = node_dependency_summary(graph, node.id);
            let related_insights: Vec<Insight> = insight_report
                .insights
                .iter()
                .filter(|insight| node_card_insight_matches(graph, node, None, insight))
                .cloned()
                .collect();
            let insight_summary = node_insight_summary(&related_insights);
            let roles = compact_node_summary_roles(node, &dependency_summary, &insight_summary);
            if roles.is_empty()
                && dependency_summary.incoming == 0
                && dependency_summary.outgoing == 0
                && insight_summary.by_severity.is_empty()
            {
                return None;
            }
            let score = compact_node_summary_score(node, &dependency_summary, &insight_summary);
            Some(ProjectCompactNodeSummary {
                node: node.clone(),
                dependency_summary,
                insight_summary,
                roles,
                score,
            })
        })
        .collect();

    nodes.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| node_rank(&left.node.kind).cmp(&node_rank(&right.node.kind)))
            .then_with(|| left.node.label.cmp(&right.node.label))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    let total_nodes = nodes.len();
    let truncated = nodes.len() > limit;
    nodes.truncate(limit);

    ProjectCompactNodeSummaryReport {
        nodes,
        total_nodes,
        truncated,
    }
}

fn compact_node_summary_candidate(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Module
            | NodeKind::Function
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Config
            | NodeKind::Environment
            | NodeKind::ExternalDependency
    )
}

fn compact_node_summary_roles(
    node: &Node,
    dependency_summary: &NodeDependencySummary,
    insight_summary: &NodeInsightSummary,
) -> Vec<String> {
    let mut roles = Vec::new();
    if node.kind == NodeKind::Entrypoint
        || dependency_summary
            .incoming_edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Entrypoint))
        || dependency_summary
            .outgoing_edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Entrypoint))
    {
        roles.push("entrypoint".to_string());
    }
    if !insight_summary.by_severity.is_empty() {
        roles.push("risk".to_string());
    }
    if node.kind == NodeKind::Config
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::ReadsConfig))
    {
        roles.push("config".to_string());
    }
    if node.kind == NodeKind::Environment
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::ReadsEnvironment))
    {
        roles.push("environment".to_string());
    }
    if node.kind == NodeKind::ExternalDependency
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::DependsOn))
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Imports))
    {
        roles.push("external_boundary".to_string());
    }
    if dependency_summary
        .edge_kinds
        .contains_key(&edge_kind_name(&EdgeKind::MayError))
    {
        roles.push("error_flow".to_string());
    }
    if dependency_summary.incoming + dependency_summary.outgoing >= 3 {
        roles.push("hub".to_string());
    }
    if roles.is_empty() && is_code_symbol(&node.kind) {
        roles.push("code_symbol".to_string());
    }
    roles
}

fn compact_node_summary_score(
    node: &Node,
    dependency_summary: &NodeDependencySummary,
    insight_summary: &NodeInsightSummary,
) -> usize {
    let risk_score: usize = insight_summary
        .by_severity
        .iter()
        .map(|(severity, count)| match severity.as_str() {
            "error" => *count * 100,
            "warning" => *count * 40,
            "info" => *count * 10,
            _ => *count,
        })
        .sum();
    let entrypoint_score = if node.kind == NodeKind::Entrypoint
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Entrypoint))
    {
        50
    } else {
        0
    };
    let boundary_score = if matches!(
        node.kind,
        NodeKind::Config | NodeKind::Environment | NodeKind::ExternalDependency
    ) {
        15
    } else {
        0
    };

    risk_score
        + entrypoint_score
        + boundary_score
        + dependency_summary.incoming * 4
        + dependency_summary.outgoing * 6
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::Calls))
            .copied()
            .unwrap_or(0)
            * 5
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::MayError))
            .copied()
            .unwrap_or(0)
            * 10
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::ReadsConfig))
            .copied()
            .unwrap_or(0)
            * 8
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::ReadsEnvironment))
            .copied()
            .unwrap_or(0)
            * 8
}

fn failing_insight_count(report: &InsightReport, fail_on: InsightSeverity) -> usize {
    report
        .by_severity
        .iter()
        .filter_map(|(severity, count)| {
            parse_report_severity(severity)
                .filter(|severity| *severity >= fail_on)
                .map(|_| *count)
        })
        .sum()
}

fn parse_report_severity(value: &str) -> Option<InsightSeverity> {
    match value {
        "info" => Some(InsightSeverity::Info),
        "warning" => Some(InsightSeverity::Warning),
        "error" => Some(InsightSeverity::Error),
        _ => None,
    }
}

fn insight_breakdowns(insights: &[Insight]) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let mut by_severity = BTreeMap::new();
    let mut by_kind = BTreeMap::new();
    for insight in insights {
        *by_severity
            .entry(severity_name(insight.severity).to_string())
            .or_insert(0) += 1;
        *by_kind.entry(insight.kind.clone()).or_insert(0) += 1;
    }
    (by_severity, by_kind)
}

fn format_backtick_list<'a>(values: impl Iterator<Item = &'a str>, limit: usize) -> String {
    let values = values.collect::<Vec<_>>();
    let rendered = values
        .iter()
        .take(limit)
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = values.len().saturating_sub(limit);
    if remaining == 0 {
        rendered
    } else if rendered.is_empty() {
        format!("{remaining} more")
    } else {
        format!("{rendered}, and {remaining} more")
    }
}

pub fn trace(graph: &CodeGraph, request: TraceRequest) -> Option<TraceResult> {
    trace_with_direction(graph, request, TraceDirection::Outgoing)
}

pub fn trace_dependents(graph: &CodeGraph, request: TraceRequest) -> Option<TraceResult> {
    trace_with_direction(graph, request, TraceDirection::Incoming)
}

fn trace_with_direction(
    graph: &CodeGraph,
    request: TraceRequest,
    direction: TraceDirection,
) -> Option<TraceResult> {
    let start = match &request.start {
        TraceStart::NodeId(id) => graph.nodes.iter().find(|node| node.id == *id)?,
        TraceStart::Label(label) => graph.nodes.iter().find(|node| node.label == *label)?,
    }
    .clone();

    let mut visited = BTreeSet::new();
    let mut depths = BTreeMap::new();
    let mut queue = VecDeque::new();
    let mut edges = Vec::new();
    let mut truncated = false;

    visited.insert(start.id);
    depths.insert(start.id, 0);
    queue.push_back((start.id, 0));

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= request.max_depth {
            if trace_edges_from(graph, node_id, direction).next().is_some() {
                truncated = true;
            }
            continue;
        }

        for edge in trace_edges_from(graph, node_id, direction) {
            edges.push(edge.clone());
            let next = trace_next_node(edge, node_id, direction);
            if visited.insert(next) {
                depths.insert(next, depth + 1);
                queue.push_back((next, depth + 1));
            }
        }
    }

    let nodes = graph
        .nodes
        .iter()
        .filter_map(|node| {
            depths.get(&node.id).map(|depth| TraceNode {
                node: node.clone(),
                depth: *depth,
            })
        })
        .collect();

    Some(TraceResult {
        start,
        max_depth: request.max_depth,
        nodes,
        edges,
        truncated,
    })
}

pub fn trace_entrypoints(
    graph: &CodeGraph,
    request: EntrypointTraceRequest,
) -> EntrypointTraceReport {
    let max_depth = request.max_depth.clamp(1, 32);
    let limit = request.limit.clamp(1, 500);
    let search = request
        .search
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let matched: Vec<_> = entrypoints(graph)
        .into_iter()
        .filter(|node| search.is_none_or(|expected| node_search_matches(node, expected)))
        .collect();

    let traces: Vec<_> = matched
        .iter()
        .take(limit)
        .filter_map(|node| {
            trace(
                graph,
                TraceRequest {
                    start: TraceStart::NodeId(node.id),
                    max_depth,
                },
            )
        })
        .collect();
    let truncated = matched.len() > traces.len() || traces.iter().any(|trace| trace.truncated);

    EntrypointTraceReport {
        max_depth,
        total_entrypoints: matched.len(),
        traces,
        truncated,
    }
}

pub fn workflow(graph: &CodeGraph, request: WorkflowRequest) -> Option<WorkflowReport> {
    let insight_report = insights(graph);
    workflow_with_insight_report(graph, request, &insight_report)
}

pub fn workflow_entrypoints(
    graph: &CodeGraph,
    request: EntrypointWorkflowRequest,
) -> EntrypointWorkflowReport {
    let max_depth = request.max_depth.clamp(1, 32);
    let block_limit = request.block_limit.clamp(1, 1_000);
    let limit = request.limit.clamp(1, 500);
    let search = request
        .search
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let matched = entrypoints(graph)
        .into_iter()
        .filter(|node| search.is_none_or(|expected| node_search_matches(node, expected)))
        .collect::<Vec<_>>();
    let insight_report = insights(graph);
    let workflows = matched
        .iter()
        .take(limit)
        .filter_map(|node| {
            workflow_with_insight_report(
                graph,
                WorkflowRequest {
                    start: TraceStart::NodeId(node.id),
                    max_depth,
                    block_limit,
                    filters: request.filters.clone(),
                    compact: request.compact,
                },
                &insight_report,
            )
        })
        .collect::<Vec<_>>();
    let truncated =
        matched.len() > workflows.len() || workflows.iter().any(|workflow| workflow.truncated);

    EntrypointWorkflowReport {
        max_depth,
        block_limit,
        filters: normalize_workflow_filters(request.filters),
        total_entrypoints: matched.len(),
        workflows,
        truncated,
    }
}

pub fn workflow_query(
    graph: &CodeGraph,
    request: WorkflowQueryRequest,
) -> Result<WorkflowQueryReport, QueryError> {
    let max_depth = request.max_depth.clamp(1, 32);
    let block_limit = request.block_limit.clamp(1, 1_000);
    let limit = request.limit.clamp(1, 500);
    let filters = normalize_workflow_filters(request.filters);
    let query_result = query_graph(graph, &request.query)?;
    let candidates = query_result.nodes.clone();
    let insight_report = insights(graph);
    let workflows = candidates
        .iter()
        .take(limit)
        .filter_map(|node| {
            workflow_with_insight_report(
                graph,
                WorkflowRequest {
                    start: TraceStart::NodeId(node.id),
                    max_depth,
                    block_limit,
                    filters: filters.clone(),
                    compact: request.compact,
                },
                &insight_report,
            )
        })
        .collect::<Vec<_>>();
    let truncated = query_result.truncated
        || candidates.len() > workflows.len()
        || workflows.iter().any(|workflow| workflow.truncated);

    Ok(WorkflowQueryReport {
        query: query_result.query,
        max_depth,
        block_limit,
        filters,
        total_query_nodes: query_result.total_nodes,
        total_query_edges: query_result.total_edges,
        total_candidates: candidates.len(),
        workflows,
        truncated,
    })
}

fn workflow_with_insight_report(
    graph: &CodeGraph,
    request: WorkflowRequest,
    insight_report: &InsightReport,
) -> Option<WorkflowReport> {
    let max_depth = request.max_depth.clamp(1, 32);
    let block_limit = request.block_limit.clamp(1, 1_000);
    let filters = normalize_workflow_filters(request.filters);
    let start = match &request.start {
        TraceStart::NodeId(id) => graph.nodes.iter().find(|node| node.id == *id)?,
        TraceStart::Label(label) => graph.nodes.iter().find(|node| node.label == *label)?,
    }
    .clone();

    let mut visited = BTreeSet::new();
    let mut depths = BTreeMap::new();
    let mut incoming = BTreeMap::new();
    let mut queue = VecDeque::new();
    let mut transition_indexes = BTreeSet::new();
    let mut truncated = false;

    visited.insert(start.id);
    depths.insert(start.id, 0);
    queue.push_back((start.id, 0));

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            if trace_edges_from_indexed(graph, node_id, TraceDirection::Outgoing)
                .filter(|(_, edge)| workflow_edge_filter_matches(edge, &filters))
                .next()
                .is_some()
            {
                truncated = true;
            }
            continue;
        }

        for (edge_index, edge) in trace_edges_from_indexed(graph, node_id, TraceDirection::Outgoing)
        {
            if !workflow_edge_filter_matches(edge, &filters) {
                continue;
            }
            let next = edge.target;
            if visited.contains(&next) {
                transition_indexes.insert(edge_index);
                continue;
            }
            if visited.len() >= block_limit {
                truncated = true;
                continue;
            }
            visited.insert(next);
            depths.insert(next, depth + 1);
            incoming.insert(next, edge_index);
            transition_indexes.insert(edge_index);
            queue.push_back((next, depth + 1));
        }
    }

    let all_blocks = graph
        .nodes
        .iter()
        .filter(|node| visited.contains(&node.id))
        .map(|node| {
            let incoming_edge = incoming
                .get(&node.id)
                .and_then(|edge_index| graph.edges.get(*edge_index));
            WorkflowBlock {
                id: workflow_block_id(node.id),
                kind: workflow_block_kind(node, incoming_edge, node.id == start.id),
                node: node.clone(),
                depth: depths.get(&node.id).copied().unwrap_or(0),
                source_node_ids: vec![node.id],
                risk_refs: workflow_risk_refs_for_node(insight_report, node.id),
                compacted: false,
                compacted_count: 1,
            }
        })
        .collect::<Vec<_>>();

    let all_transitions = transition_indexes
        .iter()
        .filter_map(|edge_index| {
            let edge = graph.edges.get(*edge_index)?;
            if !visited.contains(&edge.source) || !visited.contains(&edge.target) {
                return None;
            }
            Some(WorkflowTransition {
                id: format!("wt-{edge_index}"),
                source: workflow_block_id(edge.source),
                target: workflow_block_id(edge.target),
                source_node_id: edge.source,
                target_node_id: edge.target,
                edge: edge_with_index(*edge_index, edge),
                edge_index: *edge_index,
                risk_refs: workflow_risk_refs_for_edge(insight_report, *edge_index),
                compacted: false,
                compacted_count: 1,
            })
        })
        .collect::<Vec<_>>();
    let included_node_ids =
        workflow_included_node_ids(&start, &all_blocks, &all_transitions, &filters);
    let blocks = all_blocks
        .into_iter()
        .filter(|block| included_node_ids.contains(&block.node.id))
        .collect::<Vec<_>>();
    let transitions = all_transitions
        .into_iter()
        .filter(|transition| {
            included_node_ids.contains(&transition.source_node_id)
                && included_node_ids.contains(&transition.target_node_id)
                && workflow_transition_filter_matches(transition, &filters)
        })
        .collect::<Vec<_>>();
    let raw_total_blocks = blocks.len();
    let raw_total_transitions = transitions.len();
    let (blocks, transitions) = if request.compact {
        compact_workflow_blocks_and_transitions(&start, blocks, transitions)
    } else {
        (blocks, transitions)
    };

    Some(WorkflowReport {
        start,
        max_depth,
        block_limit,
        filters,
        compact: request.compact,
        total_blocks: blocks.len(),
        total_transitions: transitions.len(),
        raw_total_blocks,
        raw_total_transitions,
        blocks,
        transitions,
        truncated,
    })
}

fn compact_workflow_blocks_and_transitions(
    _start: &Node,
    blocks: Vec<WorkflowBlock>,
    transitions: Vec<WorkflowTransition>,
) -> (Vec<WorkflowBlock>, Vec<WorkflowTransition>) {
    let mut group_members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, block) in blocks.iter().enumerate() {
        if let Some(group_key) = workflow_compaction_group_key(block) {
            group_members.entry(group_key).or_default().push(index);
        }
    }
    group_members.retain(|_, members| members.len() > 1);
    if group_members.is_empty() {
        return (blocks, transitions);
    }

    let mut original_to_compact_block = BTreeMap::new();
    let mut compact_blocks = Vec::new();
    for (group_index, (group_key, members)) in group_members.iter().enumerate() {
        let compact_id = format!("wc-{}", group_index + 1);
        for member in members {
            original_to_compact_block.insert(blocks[*member].id.clone(), compact_id.clone());
        }
        compact_blocks.push(workflow_compacted_block(
            group_index,
            group_key,
            members.iter().map(|index| &blocks[*index]).collect(),
        ));
    }

    let mut visible_blocks = blocks
        .iter()
        .filter(|block| !original_to_compact_block.contains_key(&block.id))
        .cloned()
        .collect::<Vec<_>>();
    visible_blocks.extend(compact_blocks);
    visible_blocks.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| {
                workflow_block_kind_label(&left.kind).cmp(workflow_block_kind_label(&right.kind))
            })
            .then_with(|| left.node.label.cmp(&right.node.label))
            .then_with(|| left.id.cmp(&right.id))
    });

    let compact_block_ids = visible_blocks
        .iter()
        .filter(|block| block.compacted)
        .map(|block| block.id.clone())
        .collect::<BTreeSet<_>>();
    let mut transition_by_key: BTreeMap<(String, String, String, String), WorkflowTransition> =
        BTreeMap::new();
    for transition in transitions {
        let source = original_to_compact_block
            .get(&transition.source)
            .cloned()
            .unwrap_or_else(|| transition.source.clone());
        let target = original_to_compact_block
            .get(&transition.target)
            .cloned()
            .unwrap_or_else(|| transition.target.clone());
        if source == target {
            continue;
        }

        let key = (
            source.clone(),
            target.clone(),
            edge_kind_name(&transition.edge.kind),
            confidence_name(transition.edge.confidence),
        );
        if let Some(existing) = transition_by_key.get_mut(&key) {
            existing.compacted = true;
            existing.compacted_count += transition.compacted_count.max(1);
            existing.risk_refs.extend(transition.risk_refs);
            continue;
        }

        let mut transition = transition;
        transition.id = format!("wtc-{}", transition.edge_index);
        transition.source = source.clone();
        transition.target = target.clone();
        transition.compacted = transition.compacted
            || compact_block_ids.contains(&source)
            || compact_block_ids.contains(&target);
        transition.compacted_count = transition.compacted_count.max(1);
        transition_by_key.insert(key, transition);
    }

    (visible_blocks, transition_by_key.into_values().collect())
}

fn workflow_compaction_group_key(block: &WorkflowBlock) -> Option<String> {
    if block.compacted || !block.risk_refs.is_empty() || block.kind == WorkflowBlockKind::Start {
        return None;
    }
    let low_signal_kind = matches!(
        block.kind,
        WorkflowBlockKind::Call
            | WorkflowBlockKind::Import
            | WorkflowBlockKind::Reference
            | WorkflowBlockKind::Unknown
    );
    if !low_signal_kind {
        return None;
    }
    let language = block
        .node
        .metadata
        .get("language")
        .map(String::as_str)
        .unwrap_or("unknown");
    Some(format!(
        "{}:{}:{}",
        block.depth,
        workflow_block_kind_filter_name(&block.kind),
        language
    ))
}

fn workflow_compacted_block(
    group_index: usize,
    group_key: &str,
    members: Vec<&WorkflowBlock>,
) -> WorkflowBlock {
    let representative = members
        .first()
        .expect("workflow compaction groups are non-empty");
    let count = members.len();
    let label = workflow_compacted_label(group_key, count, &representative.kind);
    let mut source_node_ids = members
        .iter()
        .flat_map(|block| block.source_node_ids.iter().copied())
        .collect::<Vec<_>>();
    source_node_ids.sort();
    source_node_ids.dedup();

    WorkflowBlock {
        id: format!("wc-{}", group_index + 1),
        kind: representative.kind.clone(),
        node: Node {
            id: NodeId(9_000_000_000 + group_index as u64 + 1),
            kind: NodeKind::Unknown,
            label,
            span: None,
            metadata: BTreeMap::from([
                ("compacted".to_string(), "true".to_string()),
                ("compacted_count".to_string(), count.to_string()),
                (
                    "compacted_kind".to_string(),
                    workflow_block_kind_filter_name(&representative.kind),
                ),
            ]),
        },
        depth: representative.depth,
        source_node_ids,
        risk_refs: Vec::new(),
        compacted: true,
        compacted_count: count,
    }
}

fn workflow_compacted_label(group_key: &str, count: usize, kind: &WorkflowBlockKind) -> String {
    let mut parts = group_key.split(':');
    let _depth = parts.next();
    let kind_name = parts
        .next()
        .unwrap_or_else(|| workflow_block_kind_label(kind));
    let language = parts.next().unwrap_or("unknown");
    if language == "unknown" {
        format!("{count} compacted {kind_name} blocks")
    } else {
        format!("{count} compacted {language} {kind_name} blocks")
    }
}

pub fn workflow_mermaid(report: &WorkflowReport) -> String {
    let mut lines = vec!["flowchart TD".to_string()];
    for block in &report.blocks {
        lines.push(format!(
            "  {}[\"{}\"]",
            mermaid_report_block_id(&block.id),
            mermaid_escape(&format!(
                "{}: {}",
                workflow_block_kind_label(&block.kind),
                block.node.label
            ))
        ));
    }
    for transition in &report.transitions {
        let edge_label = format!(
            "{}/{}",
            edge_kind_name(&transition.edge.kind),
            confidence_name(transition.edge.confidence)
        );
        lines.push(format!(
            "  {} -->|{}| {}",
            mermaid_report_block_id(&transition.source),
            mermaid_escape(&edge_label),
            mermaid_report_block_id(&transition.target)
        ));
    }
    lines.join("\n")
}

pub fn trace_config(graph: &CodeGraph, request: ConfigTraceRequest) -> ConfigTraceResult {
    let max_depth = request.max_depth.clamp(1, 32);
    let limit = request.limit.clamp(1, 500);
    let target = request.target.trim().to_string();
    let matched_targets: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(node.kind, NodeKind::Config | NodeKind::Environment)
                && config_target_matches(node, &target)
        })
        .cloned()
        .collect();

    let mut matches = Vec::new();
    let mut total_readers = 0;
    let mut total_paths = 0;
    let mut remaining_paths = limit;
    let mut truncated = false;

    for target_node in &matched_targets {
        let reader_edges: Vec<_> = graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.target == target_node.id
                    && matches!(
                        edge.kind,
                        EdgeKind::ReadsConfig | EdgeKind::ReadsEnvironment
                    )
            })
            .collect();
        let total_match_readers = reader_edges.len();
        total_readers += total_match_readers;

        let mut readers = Vec::new();
        let mut paths = Vec::new();
        let mut match_truncated = false;

        for (edge_index, edge) in reader_edges {
            let Some(reader_node) = graph.nodes.iter().find(|node| node.id == edge.source) else {
                continue;
            };
            readers.push(ConfigReader {
                node: reader_node.clone(),
                edge: edge.clone(),
                edge_index,
            });

            let (mut reader_paths, reader_truncated) = config_reader_paths(
                graph,
                reader_node.id,
                edge_index,
                max_depth,
                remaining_paths,
            );
            match_truncated |= reader_truncated;
            total_paths += reader_paths.len();
            remaining_paths = remaining_paths.saturating_sub(reader_paths.len());
            paths.append(&mut reader_paths);
            if remaining_paths == 0 {
                match_truncated = true;
                truncated = true;
                break;
            }
        }

        let total_match_paths = paths.len();
        truncated |= match_truncated;
        matches.push(ConfigTraceMatch {
            target: target_node.clone(),
            readers,
            paths,
            total_readers: total_match_readers,
            total_paths: total_match_paths,
            truncated: match_truncated,
        });

        if remaining_paths == 0 {
            break;
        }
    }

    ConfigTraceResult {
        target,
        max_depth,
        total_matches: matched_targets.len(),
        total_readers,
        total_paths,
        matches,
        truncated,
    }
}

pub fn trace_errors(graph: &CodeGraph, request: ErrorTraceRequest) -> ErrorTraceResult {
    let max_depth = request.max_depth.clamp(1, 32);
    let limit = request.limit.clamp(1, 500);
    let target = request.target.trim().to_string();
    let matched_errors: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "error")
                && error_target_matches(node, &target)
        })
        .cloned()
        .collect();

    let mut matches = Vec::new();
    let mut total_sources = 0;
    let mut total_paths = 0;
    let mut remaining_paths = limit;
    let mut truncated = false;

    for error_node in &matched_errors {
        let source_edges: Vec<_> = graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.target == error_node.id && edge.kind == EdgeKind::MayError)
            .collect();
        let total_match_sources = source_edges.len();
        total_sources += total_match_sources;

        let mut sources = Vec::new();
        let mut paths = Vec::new();
        let mut match_truncated = false;

        for (edge_index, edge) in source_edges {
            let Some(source_node) = graph.nodes.iter().find(|node| node.id == edge.source) else {
                continue;
            };
            sources.push(ErrorSource {
                node: source_node.clone(),
                edge: edge.clone(),
                edge_index,
            });

            let (mut source_paths, source_truncated) = error_source_paths(
                graph,
                source_node.id,
                edge_index,
                max_depth,
                remaining_paths,
            );
            match_truncated |= source_truncated;
            total_paths += source_paths.len();
            remaining_paths = remaining_paths.saturating_sub(source_paths.len());
            paths.append(&mut source_paths);
            if remaining_paths == 0 {
                match_truncated = true;
                truncated = true;
                break;
            }
        }

        let total_match_paths = paths.len();
        truncated |= match_truncated;
        matches.push(ErrorTraceMatch {
            error: error_node.clone(),
            sources,
            paths,
            total_sources: total_match_sources,
            total_paths: total_match_paths,
            truncated: match_truncated,
        });

        if remaining_paths == 0 {
            break;
        }
    }

    ErrorTraceResult {
        target,
        max_depth,
        total_matches: matched_errors.len(),
        total_sources,
        total_paths,
        matches,
        truncated,
    }
}

pub fn query_graph(graph: &CodeGraph, expression: &str) -> Result<QueryResult, QueryError> {
    let spec = QuerySpec::parse(expression)?;
    match spec.command.as_str() {
        "nodes" | "node" => query_nodes(graph, spec),
        "edges" | "edge" => query_edges(graph, spec, None),
        "calls" | "call" => query_edges(graph, spec, Some(EdgeKind::Calls)),
        "dependencies" | "depends" => query_edges(graph, spec, Some(EdgeKind::DependsOn)),
        "trace" => query_trace(graph, spec),
        "dependents" | "impact" | "incoming" => query_dependents(graph, spec),
        "neighbors" | "neighbor" | "neighborhood" => query_neighbors(graph, spec),
        "symbols" | "symbol" | "defs" | "definitions" => query_symbols(graph, spec),
        "files" | "file" | "sources" | "source" => query_files(graph, spec),
        "docs" | "doc" | "documents" | "document" | "adr" | "adrs" | "rfc" | "rfcs" => {
            query_documents(graph, spec)
        }
        "sql" | "schema" | "database" | "db" => query_sql(graph, spec),
        "entrypoints" | "entrypoint" | "starts" | "startup" => query_entrypoints(graph, spec),
        "routes" | "route" | "endpoints" | "endpoint" => query_routes(graph, spec),
        "packages" | "package" | "deps" | "external" | "externals" => query_packages(graph, spec),
        "configs" | "config" | "environment" | "env" => query_configs(graph, spec),
        "errors" | "error" | "exceptions" | "exception" => query_errors(graph, spec),
        "cycles" | "cycle" => query_cycles(graph, spec),
        "hotspots" | "hotspot" | "central" | "hubs" => query_hotspots(graph, spec),
        "unreachable" | "dead" => query_unreachable(graph, spec),
        "diagnostics" | "diagnostic" => query_diagnostics(graph, spec),
        "annotations" | "annotation" | "tags" | "tag" => query_annotations(graph, spec),
        "insights" | "insight" | "risks" | "risk" | "findings" | "finding" => {
            query_insights(graph, spec)
        }
        "path" | "paths" => query_path(graph, spec),
        other => Err(QueryError::new(format!(
            "unknown query command `{other}`; expected nodes, edges, calls, dependencies, trace, dependents, neighbors, symbols, files, docs, sql, entrypoints, routes, packages, configs, errors, cycles, hotspots, unreachable, diagnostics, annotations, insights, or path"
        ))),
    }
}

pub fn compact_query_result(result: QueryResult) -> QueryResult {
    let mut group_members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, node) in result.nodes.iter().enumerate() {
        if let Some(group_key) = query_compaction_group_key(node, &result.edges) {
            group_members.entry(group_key).or_default().push(index);
        }
    }
    group_members.retain(|_, members| members.len() > 1);
    if group_members.is_empty() {
        return QueryResult {
            compact: true,
            raw_total_nodes: result.total_nodes,
            raw_total_edges: result.total_edges,
            ..result
        };
    }

    let raw_total_nodes = result.total_nodes;
    let raw_total_edges = result.total_edges;
    let mut original_to_compact = BTreeMap::new();
    let mut compact_nodes = Vec::new();
    let mut compacted_nodes = 0;
    for (group_index, (group_key, members)) in group_members.iter().enumerate() {
        let compact_id = NodeId(8_000_000_000 + group_index as u64 + 1);
        compacted_nodes += members.len();
        for member in members {
            original_to_compact.insert(result.nodes[*member].id, compact_id);
        }
        compact_nodes.push(query_compacted_node(
            compact_id,
            group_key,
            members.iter().map(|index| &result.nodes[*index]).collect(),
        ));
    }

    let mut nodes = result
        .nodes
        .iter()
        .filter(|node| !original_to_compact.contains_key(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    nodes.extend(compact_nodes);
    nodes.sort_by(|left, right| {
        node_rank(&left.kind)
            .cmp(&node_rank(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut compacted_edges = 0;
    let mut edge_by_key: BTreeMap<(NodeId, NodeId, String, String), Edge> = BTreeMap::new();
    for edge in result.edges {
        let source = original_to_compact
            .get(&edge.source)
            .copied()
            .unwrap_or(edge.source);
        let target = original_to_compact
            .get(&edge.target)
            .copied()
            .unwrap_or(edge.target);
        if source == target {
            compacted_edges += 1;
            continue;
        }
        let key = (
            source,
            target,
            edge_kind_name(&edge.kind),
            confidence_name(edge.confidence),
        );
        if let Some(existing) = edge_by_key.get_mut(&key) {
            compacted_edges += 1;
            let count = existing
                .metadata
                .get("compacted_count")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1)
                + 1;
            existing
                .metadata
                .insert("compacted".to_string(), "true".to_string());
            existing
                .metadata
                .insert("compacted_count".to_string(), count.to_string());
            continue;
        }
        let mut edge = edge;
        if source != edge.source || target != edge.target {
            edge.metadata
                .insert("original_source".to_string(), edge.source.to_string());
            edge.metadata
                .insert("original_target".to_string(), edge.target.to_string());
            edge.metadata
                .insert("compacted".to_string(), "true".to_string());
            edge.metadata
                .insert("compacted_count".to_string(), "1".to_string());
            edge.source = source;
            edge.target = target;
        }
        edge_by_key.insert(key, edge);
    }

    let edges = edge_by_key.into_values().collect::<Vec<_>>();
    let returned_nodes = nodes.len();
    let returned_edges = edges.len();
    let facets = QueryFacets::from_graph_parts(&nodes, &edges);
    QueryResult {
        query: result.query,
        total_nodes: nodes.len(),
        total_edges: edges.len(),
        compact: true,
        raw_total_nodes,
        raw_total_edges,
        compacted_nodes,
        compacted_edges,
        returned_nodes,
        returned_edges,
        truncated: result.truncated,
        facets,
        nodes,
        edges,
    }
}

pub fn natural_query(
    graph: &CodeGraph,
    request: NaturalQueryRequest,
) -> Result<NaturalQueryReport, QueryError> {
    let mut plan = natural_query_plan(&request.question)?;
    let mut alternatives = plan.alternatives.clone();
    let mut result = match query_graph(graph, &plan.generated_query) {
        Ok(result) => result,
        Err(error) => {
            let fallback = natural_query_fallback_query(plan.term.as_deref());
            if fallback == plan.generated_query {
                return Err(error);
            }
            alternatives.insert(0, plan.generated_query.clone());
            plan.generated_query = fallback;
            plan.rule = format!("fallback_after_unmatched_anchor: {}", plan.rule);
            plan.confidence = "low".to_string();
            query_graph(graph, &plan.generated_query)?
        }
    };
    if request.compact {
        result = compact_query_result(result);
    }
    alternatives.retain(|alternative| alternative != &plan.generated_query);
    alternatives.sort();
    alternatives.dedup();
    Ok(NaturalQueryReport {
        question: request.question,
        generated_query: plan.generated_query,
        rule: plan.rule,
        confidence: plan.confidence,
        result,
        alternatives,
    })
}

#[derive(Debug, Clone)]
struct NaturalQueryPlan {
    generated_query: String,
    rule: String,
    confidence: String,
    term: Option<String>,
    alternatives: Vec<String>,
}

fn natural_query_plan(question: &str) -> Result<NaturalQueryPlan, QueryError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(QueryError::new("natural-language question is empty"));
    }

    let lower = question.to_lowercase();
    let candidates = natural_query_candidates(question);
    let term = candidates.first().cloned();
    let quoted_term = term.as_deref().map(quote_query_value);
    let fallback = natural_query_fallback_query(term.as_deref());
    let mut alternatives = vec![fallback.clone()];
    if let Some(term) = quoted_term.as_deref() {
        alternatives.push(format!("insights search:{term}"));
        alternatives.push(format!(
            "symbols search:{term} direction:out edge_limit:300"
        ));
    } else {
        alternatives.push("insights".to_string());
        alternatives.push("entrypoints".to_string());
    }

    let (generated_query, rule, confidence) = if natural_query_mentions_any(
        &lower,
        &[
            "path",
            "between",
            "from ",
            " to ",
            "trace",
            "путь",
            "между",
            "от ",
            " до ",
            "трасс",
        ],
    ) && candidates.len() >= 2
    {
        (
            format!(
                "path from:{} to:{} depth:6",
                quote_query_value(&candidates[0]),
                quote_query_value(&candidates[1])
            ),
            "path_between_anchors".to_string(),
            "medium".to_string(),
        )
    } else if natural_query_mentions_any(
        &lower,
        &[
            "config",
            "environment",
            "env",
            "setting",
            "перемен",
            "конфиг",
            "настрой",
            "окружен",
        ],
    ) && !natural_query_mentions_any(
        &lower,
        &["call", "caller", "callee", "invoke", "вызов", "вызыва"],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("configs target:{term} depth:6"),
                "config_or_environment".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "configs depth:6".to_string(),
                "config_or_environment".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "error",
            "exception",
            "panic",
            "throw",
            "fail",
            "ошиб",
            "исключ",
            "паник",
            "сбой",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("errors target:{term} depth:6"),
                "error_or_exception".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "errors depth:6".to_string(),
                "error_or_exception".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "entrypoint",
            "startup",
            "start",
            "main",
            "boot",
            "точк",
            "запуск",
            "старт",
            "вход",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("entrypoints search:{term}"),
                "entrypoint_or_startup".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "entrypoints".to_string(),
                "entrypoint_or_startup".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "route",
            "endpoint",
            "http",
            "api",
            "handler",
            "маршрут",
            "эндпоинт",
            "ручк",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            let key = if candidates.first().is_some_and(|term| term.starts_with('/')) {
                "path"
            } else {
                "handler"
            };
            (
                format!("routes {key}:{term} depth:4 edge_limit:300"),
                "route_or_endpoint".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "routes depth:4 edge_limit:300".to_string(),
                "route_or_endpoint".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "dependent",
            "impact",
            "who uses",
            "used by",
            "кто использ",
            "кто завис",
            "влияни",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("dependents label:{term} depth:4"),
                "reverse_dependency_or_impact".to_string(),
                "high".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "dependency",
            "dependencies",
            "package",
            "import",
            "crate",
            "завис",
            "пакет",
            "импорт",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("packages package:{term} edge_limit:300"),
                "package_or_import".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "packages edge_limit:300".to_string(),
                "package_or_import".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &["call", "caller", "callee", "invoke", "вызов", "вызыва"],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            let direction = if natural_query_mentions_any(
                &lower,
                &["who calls", "callers", "called by", "кто вызывает"],
            ) {
                "in"
            } else {
                "out"
            };
            (
                format!("neighbors label:{term} direction:{direction} depth:2 edge_kind:calls"),
                "call_neighborhood".to_string(),
                "medium".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(&lower, &["file", "source", "path", "файл", "исход"])
    {
        if let Some(raw_term) = term.as_deref() {
            let term = quote_query_value(raw_term);
            let key = if raw_term.contains('/') || raw_term.contains('.') {
                "path"
            } else {
                "search"
            };
            (
                format!("files {key}:{term} direction:out edge_limit:300"),
                "file_or_source".to_string(),
                "high".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "symbol",
            "function",
            "class",
            "type",
            "method",
            "функц",
            "класс",
            "метод",
            "тип",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("symbols search:{term} direction:out edge_limit:300"),
                "symbol_search".to_string(),
                "high".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "unreachable",
            "dead",
            "unused",
            "orphan",
            "мертв",
            "неисп",
            "недостиж",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("unreachable search:{term}"),
                "unreachable_or_unused".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "unreachable".to_string(),
                "unreachable_or_unused".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "hotspot",
            "hub",
            "central",
            "important",
            "важн",
            "централ",
            "узел",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("hotspots search:{term} min_score:3 edge_limit:300"),
                "hotspot_or_centrality".to_string(),
                "medium".to_string(),
            )
        } else {
            (
                "hotspots min_score:3 edge_limit:300".to_string(),
                "hotspot_or_centrality".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "risk",
            "issue",
            "problem",
            "warning",
            "security",
            "риск",
            "проблем",
            "уязв",
            "предупреж",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("insights search:{term}"),
                "risk_or_insight".to_string(),
                "medium".to_string(),
            )
        } else {
            (
                "insights".to_string(),
                "risk_or_insight".to_string(),
                "medium".to_string(),
            )
        }
    } else {
        (
            fallback.clone(),
            "general_search".to_string(),
            "low".to_string(),
        )
    };

    alternatives.push(generated_query.clone());
    alternatives.retain(|alternative| alternative != &generated_query);
    alternatives.sort();
    alternatives.dedup();

    Ok(NaturalQueryPlan {
        generated_query,
        rule,
        confidence,
        term,
        alternatives,
    })
}

fn natural_query_fallback_query(term: Option<&str>) -> String {
    term.map(|term| format!("nodes search:{} limit:50", quote_query_value(term)))
        .unwrap_or_else(|| "nodes limit:50".to_string())
}

fn natural_query_mentions_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn natural_query_candidates(question: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    candidates.extend(natural_query_quoted_terms(question));

    for token in question.split(|character: char| {
        !(character.is_alphanumeric() || matches!(character, '_' | '.' | '/' | ':' | '-'))
    }) {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '.' | ',' | ';' | ':' | '?' | '!' | '(' | ')' | '[' | ']'
            )
        });
        if token.len() < 2 || candidates.iter().any(|candidate| candidate == token) {
            continue;
        }
        if natural_query_token_looks_specific(token) {
            candidates.push(token.to_string());
        }
    }

    if candidates.is_empty()
        && let Some(token) = question
            .split_whitespace()
            .map(|token| {
                token.trim_matches(|character: char| {
                    !character.is_alphanumeric() && !matches!(character, '_' | '.' | '/' | '-')
                })
            })
            .filter(|token| token.chars().count() >= 3)
            .filter(|token| !natural_query_stop_word(&token.to_lowercase()))
            .last()
    {
        candidates.push(token.to_string());
    }

    candidates
}

fn natural_query_quoted_terms(question: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut quote = None;
    let mut current = String::new();
    for character in question.chars() {
        if matches!(character, '"' | '\'' | '`') {
            if quote == Some(character) {
                let term = current.trim();
                if !term.is_empty() {
                    terms.push(term.to_string());
                }
                current.clear();
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            } else {
                current.push(character);
            }
        } else if quote.is_some() {
            current.push(character);
        }
    }
    terms
}

fn natural_query_token_looks_specific(token: &str) -> bool {
    let lower = token.to_lowercase();
    if natural_query_stop_word(&lower) {
        return false;
    }
    token.contains('_')
        || token.contains('/')
        || token.contains('.')
        || token.contains("::")
        || token
            .chars()
            .any(|character| character.is_ascii_uppercase())
        || token.chars().any(|character| character.is_ascii_digit())
}

fn natural_query_stop_word(token: &str) -> bool {
    matches!(
        token,
        "what"
            | "where"
            | "when"
            | "who"
            | "how"
            | "why"
            | "which"
            | "does"
            | "do"
            | "is"
            | "are"
            | "the"
            | "a"
            | "an"
            | "for"
            | "from"
            | "to"
            | "of"
            | "in"
            | "on"
            | "with"
            | "and"
            | "or"
            | "code"
            | "graph"
            | "где"
            | "как"
            | "что"
            | "кто"
            | "куда"
            | "зачем"
            | "почему"
            | "это"
            | "этот"
            | "эта"
            | "для"
            | "из"
            | "от"
            | "до"
            | "по"
            | "в"
            | "на"
            | "и"
            | "или"
            | "код"
            | "граф"
    )
}

fn query_compaction_group_key(node: &Node, edges: &[Edge]) -> Option<String> {
    if !query_compaction_low_signal_node(node) {
        return None;
    }
    let degree = edges
        .iter()
        .filter(|edge| edge.source == node.id || edge.target == node.id)
        .count();
    if degree > 2 {
        return None;
    }
    let language = node
        .metadata
        .get("language")
        .map(String::as_str)
        .unwrap_or("unknown");
    let item_kind = node
        .metadata
        .get("item_kind")
        .map(String::as_str)
        .unwrap_or("unknown");
    Some(format!(
        "{}:{}:{}",
        kind_name(&node.kind),
        language,
        item_kind
    ))
}

fn query_compaction_low_signal_node(node: &Node) -> bool {
    if node
        .metadata
        .get("compacted")
        .is_some_and(|value| value == "true")
    {
        return false;
    }
    if node
        .metadata
        .get("item_kind")
        .is_some_and(|value| matches!(value.as_str(), "entrypoint" | "route" | "config"))
    {
        return false;
    }
    matches!(
        node.kind,
        NodeKind::Function | NodeKind::Module | NodeKind::Unknown | NodeKind::ExternalDependency
    )
}

fn query_compacted_node(id: NodeId, group_key: &str, members: Vec<&Node>) -> Node {
    let count = members.len();
    let mut source_node_ids = members
        .iter()
        .map(|node| node.id.to_string())
        .collect::<Vec<_>>();
    source_node_ids.sort();
    let mut parts = group_key.split(':');
    let kind = parts.next().unwrap_or("node");
    let language = parts.next().unwrap_or("unknown");
    let item_kind = parts.next().unwrap_or("unknown");
    let label = if language == "unknown" && item_kind == "unknown" {
        format!("{count} compacted {kind} nodes")
    } else {
        format!("{count} compacted {language} {kind} nodes")
    };

    Node {
        id,
        kind: NodeKind::Unknown,
        label,
        span: None,
        metadata: BTreeMap::from([
            ("compacted".to_string(), "true".to_string()),
            ("compacted_count".to_string(), count.to_string()),
            ("compacted_kind".to_string(), kind.to_string()),
            ("compacted_language".to_string(), language.to_string()),
            ("compacted_item_kind".to_string(), item_kind.to_string()),
            ("source_node_ids".to_string(), source_node_ids.join(",")),
        ]),
    }
}

pub fn focus_subgraph(graph: &CodeGraph, request: FocusRequest) -> QueryResult {
    let edge_limit = request.edge_limit.clamp(1, 1000);
    let mut node_ids: BTreeSet<_> = request
        .node_ids
        .into_iter()
        .filter(|id| graph.nodes.iter().any(|node| node.id == *id))
        .collect();

    let mut matched_edges = Vec::new();
    if request.edge_indexes.is_empty() {
        matched_edges.extend(graph.edges.iter().filter(|edge| {
            !node_ids.is_empty()
                && (node_ids.contains(&edge.source) || node_ids.contains(&edge.target))
        }));
    } else {
        matched_edges.extend(
            request
                .edge_indexes
                .iter()
                .filter_map(|index| graph.edges.get(*index)),
        );
    }

    let total_edges = matched_edges.len();
    let edges: Vec<_> = matched_edges
        .into_iter()
        .take(edge_limit)
        .cloned()
        .collect();
    for edge in &edges {
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }

    let nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect();

    QueryResult::new(
        graph,
        "focus",
        nodes,
        edges,
        node_ids.len(),
        total_edges,
        total_edges > edge_limit,
    )
}

pub fn explain_edge(
    graph: &CodeGraph,
    request: ExplainEdgeRequest,
) -> Result<Option<EdgeExplanation>, QueryError> {
    let matches = matching_edge_indexes(graph, &request)?;
    let Some(edge_index) = matches.first().copied() else {
        return Ok(None);
    };
    let edge = graph
        .edges
        .get(edge_index)
        .cloned()
        .ok_or_else(|| QueryError::new(format!("edge index {edge_index} is out of range")))?;
    let source = graph
        .nodes
        .iter()
        .find(|node| node.id == edge.source)
        .cloned()
        .ok_or_else(|| QueryError::new(format!("edge source {} was not found", edge.source)))?;
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == edge.target)
        .cloned()
        .ok_or_else(|| QueryError::new(format!("edge target {} was not found", edge.target)))?;
    let summary = format!(
        "{} {} {} with {} confidence",
        source.label,
        edge_kind_name(&edge.kind),
        target.label,
        confidence_name(edge.confidence)
    );
    let evidence = edge_evidence(edge_index, &source, &target, &edge);
    let related_insights = edge_related_insights(graph, edge_index);
    let insight_summary = node_insight_summary(&related_insights);
    let total_insights = related_insights.len();
    let insight_limit = EDGE_EXPLANATION_INSIGHT_LIMIT;
    let truncated_insights = total_insights > insight_limit;
    let insights = related_insights.into_iter().take(insight_limit).collect();

    Ok(Some(EdgeExplanation {
        edge_index,
        total_matches: matches.len(),
        source,
        target,
        edge,
        summary,
        evidence,
        insight_summary,
        insights,
        total_insights,
        insight_limit,
        truncated_insights,
    }))
}

fn edge_related_insights(graph: &CodeGraph, edge_index: usize) -> Vec<Insight> {
    insights(graph)
        .insights
        .into_iter()
        .filter(|insight| insight.edges.contains(&edge_index))
        .collect()
}

fn edge_with_index(edge_index: usize, edge: &Edge) -> Edge {
    let mut indexed = edge.clone();
    indexed
        .metadata
        .insert(EDGE_INDEX_METADATA_KEY.to_string(), edge_index.to_string());
    indexed
}

fn edges_with_indexes(graph: &CodeGraph, edges: Vec<Edge>) -> Vec<Edge> {
    let mut used = BTreeSet::new();
    edges
        .into_iter()
        .map(|edge| {
            if edge.metadata.contains_key(EDGE_INDEX_METADATA_KEY) {
                return edge;
            }
            let edge_index = graph
                .edges
                .iter()
                .enumerate()
                .find(|(index, candidate)| !used.contains(index) && *candidate == &edge)
                .map(|(index, _)| index);
            if let Some(edge_index) = edge_index {
                used.insert(edge_index);
                edge_with_index(edge_index, &edge)
            } else {
                edge
            }
        })
        .collect()
}

pub fn slice_graph(graph: &CodeGraph, request: GraphSliceRequest) -> GraphSlice {
    let node_offset = request.node_offset;
    let node_limit = request.node_limit.clamp(1, 1000);
    let edge_offset = request.edge_offset;
    let edge_limit = request.edge_limit.clamp(1, 2000);
    let path_index = node_path_index(graph);

    let matched_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| slice_node_matches(node, &request, &path_index))
        .cloned()
        .collect();
    let total_nodes = matched_nodes.len();
    let nodes: Vec<_> = matched_nodes
        .into_iter()
        .skip(node_offset)
        .take(node_limit)
        .collect();
    let page_node_ids: BTreeSet<_> = nodes.iter().map(|node| node.id).collect();

    let matched_edges: Vec<_> = graph
        .edges
        .iter()
        .enumerate()
        .filter(|edge| {
            let edge = edge.1;
            page_node_ids.contains(&edge.source)
                && page_node_ids.contains(&edge.target)
                && request
                    .edge_kind
                    .as_deref()
                    .is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
                && request.confidence.as_deref().is_none_or(|expected| {
                    text_matches(&confidence_name(edge.confidence), expected)
                })
                && request
                    .edge_relation
                    .as_deref()
                    .is_none_or(|expected| edge_metadata_matches(edge, "relation", expected))
                && request
                    .edge_source
                    .as_deref()
                    .is_none_or(|expected| edge_metadata_matches(edge, "source", expected))
        })
        .collect();
    let total_edges = matched_edges.len();
    let edges = matched_edges
        .into_iter()
        .skip(edge_offset)
        .take(edge_limit)
        .map(|(edge_index, edge)| edge_with_index(edge_index, edge))
        .collect();

    GraphSlice {
        nodes,
        edges,
        total_nodes,
        total_edges,
        node_offset,
        node_limit,
        edge_offset,
        edge_limit,
        truncated_nodes: node_offset.saturating_add(node_limit) < total_nodes,
        truncated_edges: edge_offset.saturating_add(edge_limit) < total_edges,
    }
}

pub fn node_context(graph: &CodeGraph, node_id: NodeId, edge_limit: usize) -> Option<NodeContext> {
    let node = graph.nodes.iter().find(|node| node.id == node_id)?.clone();
    let edge_limit = edge_limit.clamp(1, 500);
    let matched_edges: Vec<_> = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.source == node_id || edge.target == node_id)
        .collect();
    let total_edges = matched_edges.len();
    let edges: Vec<_> = matched_edges
        .into_iter()
        .take(edge_limit)
        .map(|(edge_index, edge)| edge_with_index(edge_index, edge))
        .collect();

    let mut node_ids = BTreeSet::from([node_id]);
    for edge in &edges {
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect();

    Some(NodeContext {
        node,
        nodes,
        edges,
        total_edges,
        edge_limit,
        truncated_edges: edge_limit < total_edges,
    })
}

pub fn node_card(
    graph: &CodeGraph,
    root: Option<&Path>,
    node_id: NodeId,
    edge_limit: usize,
    source_context: u32,
    insight_limit: usize,
) -> io::Result<Option<NodeCard>> {
    let Some(context) = node_context(graph, node_id, edge_limit) else {
        return Ok(None);
    };
    let source = match root {
        Some(root) => node_source_preview(root, &context.node, source_context)?,
        None => None,
    };
    let insight_limit = insight_limit.clamp(1, 500);
    let related_insights = node_card_related_insights(graph, &context.node);
    let insight_summary = node_insight_summary(&related_insights);
    let total_insights = related_insights.len();
    let insights = related_insights.into_iter().take(insight_limit).collect();

    Ok(Some(NodeCard {
        actions: node_card_actions(&context.node),
        dependency_summary: node_dependency_summary(graph, node_id),
        insight_summary,
        file_summary: file_node_summary(graph, &context.node),
        context,
        source,
        insights,
        total_insights,
        insight_limit,
        truncated_insights: insight_limit < total_insights,
    }))
}

fn node_insight_summary(insights: &[Insight]) -> NodeInsightSummary {
    let mut summary = NodeInsightSummary::default();
    for insight in insights {
        increment_facet(
            &mut summary.by_severity,
            severity_name(insight.severity).to_string(),
        );
        increment_facet(&mut summary.by_kind, insight.kind.clone());
    }
    summary
}

fn node_card_related_insights(graph: &CodeGraph, node: &Node) -> Vec<Insight> {
    let path_index = (node.kind == NodeKind::File).then(|| node_path_index(graph));
    insights(graph)
        .insights
        .into_iter()
        .filter(|insight| node_card_insight_matches(graph, node, path_index.as_ref(), insight))
        .collect()
}

fn node_card_insight_matches(
    graph: &CodeGraph,
    node: &Node,
    path_index: Option<&BTreeMap<NodeId, String>>,
    insight: &Insight,
) -> bool {
    if insight.nodes.contains(&node.id) {
        return true;
    }
    if node.kind != NodeKind::File {
        return false;
    }

    let Some(path_index) = path_index else {
        return false;
    };
    insight.nodes.iter().any(|node_id| {
        graph
            .nodes
            .iter()
            .find(|candidate| candidate.id == *node_id)
            .is_some_and(|candidate| node_path_matches(candidate, path_index, &node.label))
    })
}

fn file_node_summary(graph: &CodeGraph, node: &Node) -> Option<FileNodeSummary> {
    if node.kind != NodeKind::File {
        return None;
    }

    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut summary = FileNodeSummary::default();
    let mut contained_code_ids = BTreeSet::new();

    for edge in graph.edges.iter().filter(|edge| edge.source == node.id) {
        match edge.kind {
            EdgeKind::Contains => {
                summary.contained_nodes += 1;
                if let Some(target) = nodes_by_id.get(&edge.target) {
                    increment_facet(&mut summary.contained_kinds, kind_name(&target.kind));
                    if let Some(item_kind) = target.metadata.get("item_kind") {
                        increment_facet(&mut summary.contained_item_kinds, item_kind.clone());
                    }
                    if is_code_symbol(&target.kind) {
                        summary.code_symbols += 1;
                        contained_code_ids.insert(edge.target);
                    }
                }
            }
            EdgeKind::Imports => {
                summary.imports += 1;
            }
            EdgeKind::DependsOn => {
                summary.direct_dependencies += 1;
            }
            _ => {}
        }
    }

    for edge in graph
        .edges
        .iter()
        .filter(|edge| contained_code_ids.contains(&edge.source) && is_trace_edge(&edge.kind))
    {
        summary.trace_edges += 1;
        increment_facet(&mut summary.trace_edge_kinds, edge_kind_name(&edge.kind));
        increment_facet(
            &mut summary.trace_confidences,
            confidence_name(edge.confidence),
        );
        if let Some(target) = nodes_by_id.get(&edge.target) {
            increment_facet(&mut summary.trace_target_kinds, kind_name(&target.kind));
        }

        match edge.kind {
            EdgeKind::Calls => {
                summary.calls += 1;
                if nodes_by_id.get(&edge.target).is_some_and(|target| {
                    target
                        .metadata
                        .get("unresolved")
                        .is_some_and(|value| value == "true")
                        || target
                            .metadata
                            .get("resolution")
                            .is_some_and(|value| value == "unresolved")
                }) {
                    summary.unresolved_calls += 1;
                }
            }
            EdgeKind::ReadsConfig => summary.config_reads += 1,
            EdgeKind::ReadsEnvironment => summary.environment_reads += 1,
            EdgeKind::MayError => summary.error_facts += 1,
            _ => {}
        }
    }

    Some(summary)
}

fn node_dependency_summary(graph: &CodeGraph, node_id: NodeId) -> NodeDependencySummary {
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut summary = NodeDependencySummary::default();

    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.source == node_id || edge.target == node_id)
    {
        let edge_kind = edge_kind_name(&edge.kind);
        increment_facet(&mut summary.edge_kinds, edge_kind.clone());
        increment_facet(&mut summary.confidences, confidence_name(edge.confidence));

        if edge.source == node_id {
            summary.outgoing += 1;
            increment_facet(&mut summary.outgoing_edge_kinds, edge_kind.clone());
        }
        if edge.target == node_id {
            summary.incoming += 1;
            increment_facet(&mut summary.incoming_edge_kinds, edge_kind);
        }

        let neighbor_id = if edge.source == node_id {
            edge.target
        } else {
            edge.source
        };
        if neighbor_id == node_id {
            continue;
        }
        if let Some(neighbor) = nodes_by_id.get(&neighbor_id) {
            increment_facet(&mut summary.neighbor_kinds, kind_name(&neighbor.kind));
            increment_facet(
                &mut summary.neighbor_languages,
                node_language(&nodes_by_id, neighbor_id),
            );
        }
    }

    summary
}

fn node_card_actions(node: &Node) -> Vec<NodeCardAction> {
    let mut actions = Vec::new();
    if node.kind == NodeKind::File {
        actions.push(NodeCardAction {
            kind: "file_graph".to_string(),
            label: "File graph".to_string(),
            query: format!(
                "files path:{} direction:out edge_limit:300",
                quote_query_value(&node.label)
            ),
        });
    }
    if is_document_query_node(node) {
        actions.push(NodeCardAction {
            kind: "document_graph".to_string(),
            label: "Document graph".to_string(),
            query: format!("docs node_id:{} edge_limit:300", node.id.0),
        });
    }
    if is_sql_query_node(node) {
        actions.push(NodeCardAction {
            kind: "sql_graph".to_string(),
            label: "SQL graph".to_string(),
            query: format!("sql node_id:{} edge_limit:300", node.id.0),
        });
    }
    if is_code_symbol(&node.kind) {
        actions.push(NodeCardAction {
            kind: "symbol_graph".to_string(),
            label: "Symbol graph".to_string(),
            query: format!("symbols node_id:{} direction:out edge_limit:300", node.id.0),
        });
    }
    if is_package_query_node(node) {
        actions.push(NodeCardAction {
            kind: "package_graph".to_string(),
            label: "Package graph".to_string(),
            query: format!("packages node_id:{} edge_limit:300", node.id.0),
        });
    }
    if matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
        actions.push(NodeCardAction {
            kind: "config_graph".to_string(),
            label: "Config readers".to_string(),
            query: format!("configs node_id:{} depth:6", node.id.0),
        });
    }
    if node
        .metadata
        .get("item_kind")
        .is_some_and(|kind| kind == "error")
    {
        actions.push(NodeCardAction {
            kind: "error_graph".to_string(),
            label: "Error paths".to_string(),
            query: format!("errors node_id:{} depth:6", node.id.0),
        });
    }
    actions
}

fn quote_query_value(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "._/@:+-".contains(character))
    {
        return value.to_string();
    }
    if !value.contains('"') {
        return format!("\"{value}\"");
    }
    if !value.contains('\'') {
        return format!("'{value}'");
    }
    format!("\"{}\"", value.replace('"', "'"))
}

fn node_source_preview(
    root: &Path,
    node: &Node,
    source_context: u32,
) -> io::Result<Option<SourcePreview>> {
    if let Some(span) = node.span.as_ref() {
        return read_source_preview(
            root,
            Path::new(&span.path),
            span.start_line,
            span.end_line,
            source_context,
        )
        .map(Some);
    }

    if node.kind == NodeKind::File {
        return read_file_source_preview(root, Path::new(&node.label)).map(Some);
    }

    Ok(None)
}

fn read_file_source_preview(root: &Path, path: &Path) -> io::Result<SourcePreview> {
    let mut source = read_source_preview(root, path, 1, u32::MAX, 0)?;
    source.end_line = source.lines.last().map(|line| line.number).unwrap_or(1);
    for line in &mut source.lines {
        line.highlight = false;
    }
    Ok(source)
}

pub fn read_source_preview(
    root: &Path,
    path: &Path,
    start_line: u32,
    end_line: u32,
    context: u32,
) -> io::Result<SourcePreview> {
    let requested_start = start_line.max(1);
    let requested_end = end_line.max(requested_start);
    let context = context.min(40);
    let visible_start = requested_start.saturating_sub(context).max(1);
    let visible_end = requested_end.saturating_add(context);
    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let display_path = full_path
        .strip_prefix(root)
        .unwrap_or(&full_path)
        .to_string_lossy()
        .replace('\\', "/");
    let bytes = fs::read(full_path)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = Vec::new();
    let mut truncated = false;

    for (index, line) in text.lines().enumerate() {
        let number = index as u32 + 1;
        if number < visible_start {
            continue;
        }
        if number > visible_end {
            break;
        }
        if lines.len() >= SOURCE_PREVIEW_LINE_LIMIT {
            truncated = true;
            break;
        }
        lines.push(SourcePreviewLine {
            number,
            text: line.to_string(),
            highlight: number >= requested_start && number <= requested_end,
        });
    }

    Ok(SourcePreview {
        path: display_path,
        start_line: requested_start,
        end_line: requested_end,
        lines,
        truncated,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuerySpec {
    original: String,
    command: String,
    terms: BTreeMap<String, String>,
    positional: Vec<String>,
    limit: usize,
}

impl QuerySpec {
    fn parse(expression: &str) -> Result<Self, QueryError> {
        let original = expression.trim();
        if original.is_empty() {
            return Err(QueryError::new("query expression is empty"));
        }

        if let Some(spec) = parse_call_expression(original)? {
            return Ok(spec);
        }

        let tokens = split_query_tokens(original)?;
        let Some(command) = tokens.first() else {
            return Err(QueryError::new("query expression is empty"));
        };

        let mut terms = BTreeMap::new();
        let mut positional = Vec::new();
        let mut limit = 100;
        for token in tokens.iter().skip(1) {
            if let Some((key, value)) = token.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim().to_string();
                if key.is_empty() || value.is_empty() {
                    return Err(QueryError::new(format!("invalid query token `{token}`")));
                }
                if key == "limit" {
                    limit = parse_limit(&value)?;
                } else {
                    terms.insert(key, value);
                }
            } else {
                positional.push(token.clone());
            }
        }

        Ok(Self {
            original: original.to_string(),
            command: command.to_ascii_lowercase(),
            terms,
            positional,
            limit,
        })
    }
}

fn parse_call_expression(expression: &str) -> Result<Option<QuerySpec>, QueryError> {
    let Some(rest) = expression.strip_prefix("calls(") else {
        return Ok(None);
    };
    let Some(inner) = rest.strip_suffix(')') else {
        return Err(QueryError::new("calls(...) query is missing closing `)`"));
    };
    let mut terms = BTreeMap::new();
    for token in inner.split_whitespace() {
        let Some((key, value)) = token.split_once(':') else {
            return Err(QueryError::new(format!(
                "invalid calls(...) token `{token}`"
            )));
        };
        let key = match key {
            "function" => "source",
            other => other,
        };
        terms.insert(key.to_ascii_lowercase(), value.to_string());
    }
    Ok(Some(QuerySpec {
        original: expression.to_string(),
        command: "calls".to_string(),
        terms,
        positional: Vec::new(),
        limit: 100,
    }))
}

fn query_nodes(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_node_terms(&spec)?;
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node_matches(node, &spec.terms))
        .cloned()
        .collect();
    let total_nodes = matched.len();
    let nodes = matched.into_iter().take(spec.limit).collect();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        Vec::new(),
        total_nodes,
        0,
        total_nodes > spec.limit,
    ))
}

fn query_edges(
    graph: &CodeGraph,
    mut spec: QuerySpec,
    fixed_kind: Option<EdgeKind>,
) -> Result<QueryResult, QueryError> {
    if let Some(kind) = fixed_kind {
        spec.terms.insert("kind".to_string(), edge_kind_name(&kind));
    }

    if matches!(
        spec.command.as_str(),
        "calls" | "call" | "dependencies" | "depends"
    ) {
        if let Some(first) = spec.positional.first() {
            spec.terms
                .entry("source".to_string())
                .or_insert(first.clone());
        }
        if let Some(function) = spec.terms.remove("function") {
            spec.terms.entry("source".to_string()).or_insert(function);
        }
    }

    validate_edge_terms(&spec)?;
    let matched: Vec<_> = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(edge_index, edge)| edge_matches(graph, *edge_index, edge, &spec.terms))
        .collect();
    let total_edges = matched.len();
    let edges: Vec<_> = matched
        .into_iter()
        .take(spec.limit)
        .map(|(edge_index, edge)| edge_with_index(edge_index, edge))
        .collect();
    let nodes = endpoint_nodes(graph, &edges);
    let total_nodes = nodes.len();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        total_edges > spec.limit,
    ))
}

fn query_trace(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    query_trace_with(graph, spec, trace)
}

fn query_dependents(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    query_trace_with(graph, spec, trace_dependents)
}

fn query_trace_with(
    graph: &CodeGraph,
    spec: QuerySpec,
    trace_fn: fn(&CodeGraph, TraceRequest) -> Option<TraceResult>,
) -> Result<QueryResult, QueryError> {
    let depth = spec
        .terms
        .get("depth")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 8)))
        .transpose()?
        .unwrap_or(2);
    let start = if let Some(id) = spec.terms.get("id").or_else(|| spec.terms.get("node_id")) {
        TraceStart::NodeId(parse_node_id(id)?)
    } else if let Some(label) = spec
        .terms
        .get("label")
        .or_else(|| spec.terms.get("start"))
        .or_else(|| spec.positional.first())
    {
        TraceStart::Label(label.clone())
    } else {
        return Err(QueryError::new(
            "trace query requires `label:<value>`, `id:<node-id>`, or a positional label",
        ));
    };

    let Some(result) = trace_fn(
        graph,
        TraceRequest {
            start,
            max_depth: depth,
        },
    ) else {
        return Ok(QueryResult::new(
            graph,
            spec.original,
            Vec::new(),
            Vec::new(),
            0,
            0,
            false,
        ));
    };

    let total_nodes = result.nodes.len();
    let total_edges = result.edges.len();
    let nodes = result.nodes.into_iter().map(|node| node.node).collect();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        result.edges,
        total_nodes,
        total_edges,
        result.truncated,
    ))
}

fn query_neighbors(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_neighbor_terms(&spec)?;
    let max_depth = spec
        .terms
        .get("depth")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 16)))
        .transpose()?
        .unwrap_or(1);
    let direction = spec
        .terms
        .get("direction")
        .or_else(|| spec.terms.get("dir"))
        .map(|value| parse_neighbor_direction(value, "neighbors"))
        .transpose()?
        .unwrap_or(NeighborDirection::Both);
    let start = if let Some(id) = spec.terms.get("id").or_else(|| spec.terms.get("node_id")) {
        let id = parse_node_id(id)?;
        graph
            .nodes
            .iter()
            .any(|node| node.id == id)
            .then_some(id)
            .ok_or_else(|| {
                QueryError::new(format!("neighbors start `{id}` did not match a node"))
            })?
    } else if let Some(label) = spec
        .terms
        .get("label")
        .or_else(|| spec.terms.get("start"))
        .or_else(|| spec.terms.get("node"))
        .or_else(|| spec.positional.first())
    {
        resolve_node_reference(graph, label).ok_or_else(|| {
            QueryError::new(format!("neighbors start `{label}` did not match a node"))
        })?
    } else {
        return Err(QueryError::new(
            "neighbors query requires `label:<value>`, `id:<node-id>`, or a positional label",
        ));
    };
    let edge_kind = spec
        .terms
        .get("edge_kind")
        .or_else(|| spec.terms.get("kind"));
    let confidence = spec.terms.get("confidence");

    let mut visited_nodes = BTreeSet::from([start]);
    let mut seen_edges = BTreeSet::new();
    let mut edges = Vec::new();
    let mut total_edges = 0;
    let mut queue = VecDeque::from([(start, 0usize)]);
    let mut truncated = false;

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            if graph.edges.iter().any(|edge| {
                neighbor_edge_matches(
                    edge,
                    node_id,
                    direction,
                    edge_kind.map(String::as_str),
                    confidence.map(String::as_str),
                )
            }) {
                truncated = true;
            }
            continue;
        }

        for (edge_index, edge) in graph.edges.iter().enumerate().filter(|(_, edge)| {
            neighbor_edge_matches(
                edge,
                node_id,
                direction,
                edge_kind.map(String::as_str),
                confidence.map(String::as_str),
            )
        }) {
            if !seen_edges.insert(edge_index) {
                continue;
            }
            total_edges += 1;
            if edges.len() >= spec.limit {
                truncated = true;
                continue;
            }

            edges.push(edge.clone());
            let neighbor = if edge.source == node_id {
                edge.target
            } else {
                edge.source
            };
            if visited_nodes.insert(neighbor) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| visited_nodes.contains(&node.id))
        .cloned()
        .collect();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        visited_nodes.len(),
        total_edges,
        truncated,
    ))
}

fn query_symbols(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_symbol_terms(&spec)?;
    let path_index = node_path_index(graph);
    let edge_kind = spec.terms.get("edge_kind");
    let confidence = spec.terms.get("confidence");
    let direction = spec
        .terms
        .get("direction")
        .or_else(|| spec.terms.get("dir"))
        .map(|value| parse_neighbor_direction(value, "symbols"))
        .transpose()?
        .unwrap_or(NeighborDirection::Both);
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(300);
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| is_code_symbol(&node.kind) && symbol_query_matches(node, &spec, &path_index))
        .cloned()
        .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let mut node_ids = selected_ids.clone();
    let mut edge_indexes = BTreeSet::new();

    for (index, edge) in graph.edges.iter().enumerate() {
        if symbol_definition_edge_matches(edge, &selected_ids, edge_kind.map(String::as_str)) {
            edge_indexes.insert(index);
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
            continue;
        }
        if !is_trace_edge(&edge.kind) {
            continue;
        }
        if !hotspot_edge_touches_selected(edge, &selected_ids, direction) {
            continue;
        }
        if edge_kind.is_some_and(|expected| !text_matches(&edge_kind_name(&edge.kind), expected)) {
            continue;
        }
        if confidence
            .is_some_and(|expected| !text_matches(&confidence_name(edge.confidence), expected))
        {
            continue;
        }
        edge_indexes.insert(index);
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }

    let total_edges = edge_indexes.len();
    let total_nodes = node_ids.len();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .take(edge_limit)
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();
    let mut returned_node_ids = selected_ids.clone();
    for edge in &edges {
        returned_node_ids.insert(edge.source);
        returned_node_ids.insert(edge.target);
    }
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| returned_node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > edge_limit,
    ))
}

fn query_files(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_file_terms(&spec)?;
    let path_index = node_path_index(graph);
    let edge_kind = spec.terms.get("edge_kind");
    let confidence = spec.terms.get("confidence");
    let direction = spec
        .terms
        .get("direction")
        .or_else(|| spec.terms.get("dir"))
        .map(|value| parse_neighbor_direction(value, "files"))
        .transpose()?
        .unwrap_or(NeighborDirection::Both);
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(300);

    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File && file_query_matches(node, &spec, &path_index))
        .cloned()
        .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let contained_ids: BTreeSet<_> = graph
        .edges
        .iter()
        .filter(|edge| selected_ids.contains(&edge.source) && edge.kind == EdgeKind::Contains)
        .map(|edge| edge.target)
        .collect();
    let contained_code_ids: BTreeSet<_> = graph
        .nodes
        .iter()
        .filter(|node| contained_ids.contains(&node.id) && is_code_symbol(&node.kind))
        .map(|node| node.id)
        .collect();

    let mut node_ids = selected_ids.clone();
    let mut edge_indexes = BTreeSet::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if file_structural_edge_matches(edge, &selected_ids, edge_kind.map(String::as_str)) {
            edge_indexes.insert(index);
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
            continue;
        }
        if !is_trace_edge(&edge.kind) {
            continue;
        }
        if !file_trace_edge_touches_selected(edge, &selected_ids, &contained_code_ids, direction) {
            continue;
        }
        if edge_kind.is_some_and(|expected| !text_matches(&edge_kind_name(&edge.kind), expected)) {
            continue;
        }
        if confidence
            .is_some_and(|expected| !text_matches(&confidence_name(edge.confidence), expected))
        {
            continue;
        }
        edge_indexes.insert(index);
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }

    let total_edges = edge_indexes.len();
    let total_nodes = node_ids.len();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .take(edge_limit)
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();
    let mut returned_node_ids = selected_ids.clone();
    for edge in &edges {
        returned_node_ids.insert(edge.source);
        returned_node_ids.insert(edge.target);
    }
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| returned_node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > edge_limit,
    ))
}

fn query_documents(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if matches!(spec.command.as_str(), "adr" | "adrs") {
        spec.terms
            .entry("document_kind".to_string())
            .or_insert("adr".to_string());
    } else if matches!(spec.command.as_str(), "rfc" | "rfcs") {
        spec.terms
            .entry("document_kind".to_string())
            .or_insert("rfc".to_string());
    }
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_document_terms(&spec)?;
    let direction = spec
        .terms
        .get("direction")
        .or_else(|| spec.terms.get("dir"))
        .map(|value| parse_neighbor_direction(value, "docs"))
        .transpose()?
        .unwrap_or(NeighborDirection::Both);
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(500);
    let path_index = node_path_index(graph);
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            is_document_query_node(node) && document_query_matches(graph, node, &spec, &path_index)
        })
        .cloned()
        .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let mut node_ids = selected_ids.clone();
    let mut edge_indexes = BTreeSet::new();

    for (index, edge) in graph.edges.iter().enumerate() {
        if !document_edge_matches(graph, edge, &selected_ids, &spec, &path_index, direction) {
            continue;
        }
        edge_indexes.insert(index);
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }

    let total_edges = edge_indexes.len();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .take(edge_limit)
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let total_nodes = nodes.len();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > edge_limit,
    ))
}

fn query_sql(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_sql_terms(&spec)?;
    let direction = spec
        .terms
        .get("direction")
        .or_else(|| spec.terms.get("dir"))
        .map(|value| parse_neighbor_direction(value, "sql"))
        .transpose()?
        .unwrap_or(NeighborDirection::Both);
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(500);
    let path_index = node_path_index(graph);
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            is_sql_query_node(node) && sql_query_matches(graph, node, &spec, &path_index)
        })
        .cloned()
        .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let mut node_ids = selected_ids.clone();
    let mut edge_indexes = BTreeSet::new();

    for (index, edge) in graph.edges.iter().enumerate() {
        if !sql_edge_matches(graph, edge, &selected_ids, &spec, &path_index, direction) {
            continue;
        }
        edge_indexes.insert(index);
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }

    let total_edges = edge_indexes.len();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .take(edge_limit)
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let total_nodes = nodes.len();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > edge_limit,
    ))
}

fn query_entrypoints(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_entrypoint_terms(&spec)?;

    let path_index = node_path_index(graph);
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Entrypoint && entrypoint_query_matches(node, &spec, &path_index)
        })
        .cloned()
        .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();

    let matched_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            (selected_ids.contains(&edge.target) && edge.kind == EdgeKind::Entrypoint)
                || (selected_ids.contains(&edge.source) && is_trace_edge(&edge.kind))
        })
        .cloned()
        .collect();
    let total_edges = matched_edges.len();
    let edges: Vec<_> = matched_edges.into_iter().take(spec.limit).collect();
    let mut node_ids = selected_ids;
    for edge in &edges {
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > spec.limit,
    ))
}

fn query_routes(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_route_terms(&spec)?;
    let depth = spec
        .terms
        .get("depth")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 16)))
        .transpose()?
        .unwrap_or(2);
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(500);
    let path_index = node_path_index(graph);
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            is_framework_route_node(node) && route_query_matches(node, &spec, &path_index)
        })
        .cloned()
        .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let mut node_ids = selected_ids.clone();
    let mut edge_indexes = BTreeSet::new();
    let mut truncated = matched.len() > spec.limit;

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind == EdgeKind::Entrypoint && selected_ids.contains(&edge.target) {
            edge_indexes.insert(index);
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
        }
    }

    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    for route_id in &selected_ids {
        visited.insert(*route_id);
        queue.push_back((*route_id, 0usize));
    }

    while let Some((node_id, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            if graph
                .edges
                .iter()
                .any(|edge| edge.source == node_id && is_trace_edge(&edge.kind))
            {
                truncated = true;
            }
            continue;
        }

        for (edge_index, edge) in graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.source == node_id && is_trace_edge(&edge.kind))
        {
            edge_indexes.insert(edge_index);
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
            if route_trace_should_expand(edge) && visited.insert(edge.target) {
                queue.push_back((edge.target, current_depth + 1));
            }
        }
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let total_edges = edge_indexes.len();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .take(edge_limit)
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        truncated || total_edges > edge_limit,
    ))
}

fn query_packages(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("package".to_string())
            .or_insert(first.clone());
    }
    validate_package_terms(&spec)?;
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(500);
    let path_index = node_path_index(graph);
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            is_package_query_node(node) && package_query_matches(graph, node, &spec, &path_index)
        })
        .cloned()
        .collect();

    let mut selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let package_keys: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .filter_map(package_node_key)
        .collect();
    if !package_keys.is_empty() {
        for node in graph
            .nodes
            .iter()
            .filter(|node| is_package_query_node(node))
        {
            if package_node_key(node).is_some_and(|key| package_keys.contains(&key)) {
                selected_ids.insert(node.id);
            }
        }
    }

    let mut edge_indexes = BTreeSet::new();
    let mut node_ids = selected_ids.clone();
    for (index, edge) in graph.edges.iter().enumerate() {
        if !matches!(edge.kind, EdgeKind::Imports | EdgeKind::DependsOn) {
            continue;
        }
        if !package_edge_query_matches(graph, edge, &spec, &path_index) {
            continue;
        }
        if selected_ids.contains(&edge.source) || selected_ids.contains(&edge.target) {
            edge_indexes.insert(index);
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
        }
    }

    let total_edges = edge_indexes.len();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .take(edge_limit)
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > edge_limit,
    ))
}

fn query_configs(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_config_terms(&spec)?;
    let max_depth = spec
        .terms
        .get("depth")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 32)))
        .transpose()?
        .unwrap_or(6);
    let path_index = node_path_index(graph);
    let matched_targets: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(node.kind, NodeKind::Config | NodeKind::Environment)
                && config_query_matches(node, &spec, &path_index)
        })
        .cloned()
        .collect();

    let mut node_ids = BTreeSet::new();
    let mut edge_indexes = BTreeSet::new();
    let mut remaining_paths = spec.limit;
    let mut truncated = matched_targets.len() > spec.limit;

    for target in matched_targets.iter().take(spec.limit) {
        node_ids.insert(target.id);
        let reader_edges: Vec<_> = graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.target == target.id
                    && matches!(
                        edge.kind,
                        EdgeKind::ReadsConfig | EdgeKind::ReadsEnvironment
                    )
            })
            .collect();

        for (edge_index, edge) in reader_edges {
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
            edge_indexes.insert(edge_index);

            if remaining_paths == 0 {
                truncated = true;
                continue;
            }

            let (paths, paths_truncated) =
                config_reader_paths(graph, edge.source, edge_index, max_depth, remaining_paths);
            truncated |= paths_truncated;
            remaining_paths = remaining_paths.saturating_sub(paths.len());
            for path in paths {
                for node in path.nodes {
                    node_ids.insert(node.id);
                }
                for index in path.edge_indexes {
                    edge_indexes.insert(index);
                }
            }
        }
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    let total_edges = edges.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        truncated,
    ))
}

fn query_errors(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_error_terms(&spec)?;
    let max_depth = spec
        .terms
        .get("depth")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 32)))
        .transpose()?
        .unwrap_or(6);
    let path_index = node_path_index(graph);
    let matched_errors: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "error")
                && error_query_matches(node, &spec, &path_index)
        })
        .cloned()
        .collect();

    let mut node_ids = BTreeSet::new();
    let mut edge_indexes = BTreeSet::new();
    let mut remaining_paths = spec.limit;
    let mut truncated = matched_errors.len() > spec.limit;

    for error in matched_errors.iter().take(spec.limit) {
        node_ids.insert(error.id);
        let source_edges: Vec<_> = graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.target == error.id && edge.kind == EdgeKind::MayError)
            .collect();

        for (edge_index, edge) in source_edges {
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
            edge_indexes.insert(edge_index);

            if remaining_paths == 0 {
                truncated = true;
                continue;
            }

            let (paths, paths_truncated) =
                error_source_paths(graph, edge.source, edge_index, max_depth, remaining_paths);
            truncated |= paths_truncated;
            remaining_paths = remaining_paths.saturating_sub(paths.len());
            for path in paths {
                for node in path.nodes {
                    node_ids.insert(node.id);
                }
                for index in path.edge_indexes {
                    edge_indexes.insert(index);
                }
            }
        }
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    let total_edges = edges.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        truncated,
    ))
}

fn query_cycles(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_cycle_terms(&spec)?;
    let path_index = node_path_index(graph);
    let matched: Vec<_> = insights(graph)
        .insights
        .into_iter()
        .filter(|insight| insight.kind == "dependency_cycle")
        .filter(|insight| cycle_query_matches(graph, insight, &spec, &path_index))
        .collect();
    let total_matches = matched.len();
    let mut node_ids = BTreeSet::new();
    let mut edge_indexes = BTreeSet::new();
    for insight in matched.iter().take(spec.limit) {
        node_ids.extend(insight.nodes.iter().copied());
        edge_indexes.extend(insight.edges.iter().copied());
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    let total_edges = edges.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        total_matches > spec.limit,
    ))
}

fn query_hotspots(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_hotspot_terms(&spec)?;
    let path_index = node_path_index(graph);
    let edge_kind = spec.terms.get("edge_kind");
    let confidence = spec.terms.get("confidence");
    let direction = spec
        .terms
        .get("direction")
        .or_else(|| spec.terms.get("dir"))
        .map(|value| parse_neighbor_direction(value, "hotspots"))
        .transpose()?
        .unwrap_or(NeighborDirection::Both);
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(300);
    let min_score = spec
        .terms
        .get("min_score")
        .or_else(|| spec.terms.get("min_degree"))
        .or_else(|| spec.terms.get("score"))
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 10_000)))
        .transpose()?
        .unwrap_or(1);

    let matched: Vec<_> = hotspot_stats(
        graph,
        |edge| {
            edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
                && confidence.is_none_or(|expected| {
                    text_matches(&confidence_name(edge.confidence), expected)
                })
        },
        direction,
    )
    .into_iter()
    .filter(|hotspot| {
        hotspot.score >= min_score && hotspot_query_matches(&hotspot.node, &spec, &path_index)
    })
    .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|hotspot| hotspot.node.id)
        .collect();
    let matched_edges: Vec<_> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind != EdgeKind::Contains
                && hotspot_edge_touches_selected(edge, &selected_ids, direction)
                && edge_kind
                    .is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
                && confidence.is_none_or(|expected| {
                    text_matches(&confidence_name(edge.confidence), expected)
                })
        })
        .cloned()
        .collect();
    let total_edges = matched_edges.len();
    let edges: Vec<_> = matched_edges.into_iter().take(edge_limit).collect();
    let mut node_ids = selected_ids;
    for edge in &edges {
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > edge_limit,
    ))
}

fn query_unreachable(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_unreachable_terms(&spec)?;
    let reachable = entrypoint_reachable_nodes(graph);
    if reachable.is_empty() {
        return Ok(QueryResult::new(
            graph,
            spec.original,
            Vec::new(),
            Vec::new(),
            0,
            0,
            false,
        ));
    }

    let path_index = node_path_index(graph);
    let node_terms = unreachable_node_terms(&spec);
    let scope = unreachable_scope(&spec)?;
    if matches!(
        scope,
        UnreachableScope::ConfigReads | UnreachableScope::ErrorFlows
    ) {
        return Ok(query_unreachable_flow_scope(
            graph,
            spec,
            &reachable,
            &path_index,
            scope,
        ));
    }
    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            if scope == UnreachableScope::SourceFiles {
                is_source_file_candidate(graph, node)
                    && !reachable.contains(&node.id)
                    && !file_has_reachable_code(graph, node.id, &reachable)
            } else {
                node.id != graph.root && !reachable.contains(&node.id)
            }
        })
        .filter(|node| node_matches(node, &node_terms))
        .filter(|node| {
            spec.terms
                .get("path_prefix")
                .is_none_or(|expected| node_path_matches(node, &path_index, expected))
        })
        .map(|node| node.id)
        .collect();
    let total_matches = matched.len();
    let selected: BTreeSet<_> = matched.iter().take(spec.limit).copied().collect();
    let edge_limit = spec.limit.saturating_mul(4).clamp(1, 1000);

    let mut result_node_ids = selected.clone();
    let mut matched_edges = Vec::new();
    let mut total_edges = 0usize;
    for edge in graph.edges.iter().filter(|edge| {
        (selected.contains(&edge.source) && edge.kind == EdgeKind::Contains)
            || ((selected.contains(&edge.source) || selected.contains(&edge.target))
                && is_trace_edge(&edge.kind))
    }) {
        total_edges += 1;
        if matched_edges.len() >= edge_limit {
            continue;
        }
        result_node_ids.insert(edge.source);
        result_node_ids.insert(edge.target);
        matched_edges.push(edge.clone());
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| result_node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let truncated = total_matches > spec.limit || total_edges > edge_limit;

    let total_nodes = nodes.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        matched_edges,
        total_nodes,
        total_edges,
        truncated,
    ))
}

fn query_unreachable_flow_scope(
    graph: &CodeGraph,
    spec: QuerySpec,
    reachable: &BTreeSet<NodeId>,
    path_index: &BTreeMap<NodeId, String>,
    scope: UnreachableScope,
) -> QueryResult {
    let node_terms = unreachable_node_terms(&spec);
    let edge_kinds = match scope {
        UnreachableScope::ConfigReads => &[EdgeKind::ReadsConfig, EdgeKind::ReadsEnvironment][..],
        UnreachableScope::ErrorFlows => &[EdgeKind::MayError][..],
        UnreachableScope::SourceFiles | UnreachableScope::AnyNode => &[][..],
    };
    let node_by_id: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();

    let matched: Vec<_> = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge_kinds.contains(&edge.kind) && !reachable.contains(&edge.source))
        .filter(|(_, edge)| {
            unreachable_flow_matches(edge, &node_by_id, &node_terms, &spec, path_index)
        })
        .collect();
    let total_matches = matched.len();
    let edge_limit = spec.limit.clamp(1, 1000);
    let mut result_node_ids = BTreeSet::new();
    let mut edges = Vec::new();

    for (_, edge) in matched.iter().take(edge_limit) {
        result_node_ids.insert(edge.source);
        result_node_ids.insert(edge.target);
        edges.push((*edge).clone());
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| result_node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let truncated = total_matches > edge_limit;
    let total_nodes = nodes.len();
    let total_edges = total_matches;
    QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        truncated,
    )
}

fn unreachable_flow_matches(
    edge: &Edge,
    node_by_id: &BTreeMap<NodeId, &Node>,
    node_terms: &BTreeMap<String, String>,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    let Some(source) = node_by_id.get(&edge.source) else {
        return false;
    };
    let Some(target) = node_by_id.get(&edge.target) else {
        return false;
    };

    let node_match = node_matches(source, node_terms) || node_matches(target, node_terms);
    let path_match = spec.terms.get("path_prefix").is_none_or(|expected| {
        node_path_matches(source, path_index, expected)
            || node_path_matches(target, path_index, expected)
    });
    node_match && path_match
}

fn query_diagnostics(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_diagnostic_terms(&spec)?;
    let path_index = node_path_index(graph);
    let diagnostic_nodes = graph
        .nodes
        .iter()
        .filter(|node| is_lsp_diagnostic_node(node))
        .filter(|node| diagnostic_query_matches(graph, node, &spec, &path_index))
        .map(|node| node.id)
        .collect::<Vec<_>>();
    let total_matches = diagnostic_nodes.len();
    let selected: BTreeSet<_> = diagnostic_nodes.iter().take(spec.limit).copied().collect();
    let edge_limit = spec.limit.saturating_mul(4).clamp(1, 1000);
    let mut result_node_ids = selected.clone();
    let mut matched_edges = Vec::new();
    let mut total_edges = 0usize;

    for edge in graph.edges.iter().filter(|edge| {
        selected.contains(&edge.target)
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "diagnostic")
    }) {
        total_edges += 1;
        result_node_ids.insert(edge.source);
        result_node_ids.insert(edge.target);
        if matched_edges.len() < edge_limit {
            matched_edges.push(edge.clone());
        }
    }

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| result_node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();
    let truncated = total_matches > spec.limit || total_edges > edge_limit;

    let total_nodes = nodes.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        matched_edges,
        total_nodes,
        total_edges,
        truncated,
    ))
}

fn query_annotations(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_annotation_terms(&spec)?;
    let path_index = node_path_index(graph);
    let edge_kind = spec.terms.get("edge_kind");
    let confidence = spec.terms.get("confidence");
    let direction = spec
        .terms
        .get("direction")
        .or_else(|| spec.terms.get("dir"))
        .map(|value| parse_neighbor_direction(value, "annotations"))
        .transpose()?
        .unwrap_or(NeighborDirection::Both);
    let edge_limit = spec
        .terms
        .get("edge_limit")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 2_000)))
        .transpose()?
        .unwrap_or(300);

    let matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node_has_annotation(node) && annotation_query_matches(node, &spec, &path_index)
        })
        .cloned()
        .collect();
    let selected_ids: BTreeSet<_> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let mut node_ids = selected_ids.clone();
    let mut edge_indexes = BTreeSet::new();

    for (index, edge) in graph.edges.iter().enumerate() {
        if !hotspot_edge_touches_selected(edge, &selected_ids, direction) {
            continue;
        }
        if edge_kind.is_some_and(|expected| !text_matches(&edge_kind_name(&edge.kind), expected)) {
            continue;
        }
        if confidence
            .is_some_and(|expected| !text_matches(&confidence_name(edge.confidence), expected))
        {
            continue;
        }
        edge_indexes.insert(index);
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }

    let total_edges = edge_indexes.len();
    let total_nodes = node_ids.len();
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .take(edge_limit)
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();
    let mut returned_node_ids = selected_ids.clone();
    for edge in &edges {
        returned_node_ids.insert(edge.source);
        returned_node_ids.insert(edge.target);
    }
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| returned_node_ids.contains(&node.id))
        .cloned()
        .collect::<Vec<_>>();

    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        matched.len() > spec.limit || total_edges > edge_limit,
    ))
}

fn query_insights(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_insight_terms(&spec)?;
    let path_index = node_path_index(graph);
    let matched: Vec<_> = insights(graph)
        .insights
        .into_iter()
        .filter(|insight| insight_query_matches(graph, insight, &spec, &path_index))
        .collect();
    let total_insights = matched.len();
    let mut node_ids = BTreeSet::new();
    let mut edge_indexes = BTreeSet::new();

    for insight in matched.iter().take(spec.limit) {
        node_ids.extend(insight.nodes.iter().copied());
        edge_indexes.extend(insight.edges.iter().copied());
    }

    let mut edges = Vec::new();
    for edge_index in edge_indexes {
        if let Some(edge) = graph.edges.get(edge_index).cloned() {
            node_ids.insert(edge.source);
            node_ids.insert(edge.target);
            edges.push(edge);
        }
    }

    let nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| node_ids.contains(&node.id))
        .cloned()
        .collect();

    let total_nodes = nodes.len();
    let total_edges = edges.len();
    Ok(QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        total_insights > spec.limit,
    ))
}

fn query_path(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    validate_path_terms(&spec)?;
    let max_depth = spec
        .terms
        .get("depth")
        .map(|value| parse_limit(value).map(|value| value.clamp(1, 32)))
        .transpose()?
        .unwrap_or(8);
    let from = spec
        .terms
        .get("from")
        .or_else(|| spec.terms.get("source"))
        .or_else(|| spec.positional.first())
        .ok_or_else(|| {
            QueryError::new("path query requires `from:<label-or-id>` and `to:<label-or-id>`")
        })?;
    let to = spec
        .terms
        .get("to")
        .or_else(|| spec.terms.get("target"))
        .or_else(|| spec.positional.get(1))
        .ok_or_else(|| {
            QueryError::new("path query requires `from:<label-or-id>` and `to:<label-or-id>`")
        })?;
    let start = resolve_node_reference(graph, from)
        .ok_or_else(|| QueryError::new(format!("path start `{from}` did not match a node")))?;
    let target = resolve_node_reference(graph, to)
        .ok_or_else(|| QueryError::new(format!("path target `{to}` did not match a node")))?;
    let edge_kind = spec
        .terms
        .get("edge_kind")
        .or_else(|| spec.terms.get("kind"));

    if start == target {
        let node = graph.nodes.iter().find(|node| node.id == start).cloned();
        return Ok(QueryResult::new(
            graph,
            spec.original,
            node.into_iter().collect(),
            Vec::new(),
            1,
            0,
            false,
        ));
    }

    let mut visited = BTreeSet::from([start]);
    let mut parents: BTreeMap<NodeId, (NodeId, usize)> = BTreeMap::new();
    let mut queue = VecDeque::from([(start, 0usize)]);
    let mut truncated = false;

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            if graph.edges.iter().any(|edge| {
                edge.source == node_id && path_edge_matches(edge, edge_kind.map(String::as_str))
            }) {
                truncated = true;
            }
            continue;
        }

        for (edge_index, edge) in graph.edges.iter().enumerate().filter(|(_, edge)| {
            edge.source == node_id && path_edge_matches(edge, edge_kind.map(String::as_str))
        }) {
            if !visited.insert(edge.target) {
                continue;
            }
            parents.insert(edge.target, (node_id, edge_index));
            if edge.target == target {
                let edge_indexes = reconstruct_path_edges(start, target, &parents)?;
                let edges: Vec<_> = edge_indexes
                    .iter()
                    .filter_map(|index| graph.edges.get(*index).cloned())
                    .collect();
                let nodes = path_nodes(graph, start, &edges);
                let total_nodes = nodes.len();
                let total_edges = edges.len();
                return Ok(QueryResult::new(
                    graph,
                    spec.original,
                    nodes,
                    edges,
                    total_nodes,
                    total_edges,
                    false,
                ));
            }
            queue.push_back((edge.target, depth + 1));
        }
    }

    Ok(QueryResult::new(
        graph,
        spec.original,
        Vec::new(),
        Vec::new(),
        0,
        0,
        truncated,
    ))
}

fn validate_node_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if is_node_term(key) {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported node query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_edge_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if is_edge_term(key) {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported edge query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_path_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "from" | "to" | "source" | "target" | "depth" | "kind" | "edge_kind"
        ) {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported path query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_neighbor_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node_id"
                | "label"
                | "start"
                | "node"
                | "depth"
                | "direction"
                | "dir"
                | "kind"
                | "edge_kind"
                | "confidence"
        ) {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported neighbors query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_symbol_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node"
                | "node_id"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "node_kind"
                | "item_kind"
                | "path"
                | "path_prefix"
                | "direction"
                | "dir"
                | "edge_kind"
                | "confidence"
                | "edge_limit"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported symbols query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_file_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node"
                | "node_id"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "node_kind"
                | "item_kind"
                | "path"
                | "source_path"
                | "file"
                | "file_path"
                | "path_prefix"
                | "direction"
                | "dir"
                | "edge_kind"
                | "confidence"
                | "edge_limit"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported files query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_document_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node"
                | "node_id"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "node_kind"
                | "item_kind"
                | "document_kind"
                | "doc_kind"
                | "type"
                | "heading"
                | "anchor"
                | "path"
                | "source_path"
                | "file"
                | "file_path"
                | "path_prefix"
                | "target"
                | "relation"
                | "edge_kind"
                | "confidence"
                | "direction"
                | "dir"
                | "edge_limit"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported docs query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_sql_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node"
                | "node_id"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "node_kind"
                | "item_kind"
                | "table"
                | "table_name"
                | "table_key"
                | "column"
                | "column_name"
                | "column_key"
                | "operation"
                | "query"
                | "resolution"
                | "unresolved"
                | "path"
                | "source_path"
                | "file"
                | "file_path"
                | "path_prefix"
                | "target"
                | "relation"
                | "edge_kind"
                | "confidence"
                | "direction"
                | "dir"
                | "edge_limit"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported sql query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_entrypoint_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node_id"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "item_kind"
                | "entrypoint_kind"
                | "path"
                | "path_prefix"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported entrypoints query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_route_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node_id"
                | "label"
                | "search"
                | "language"
                | "framework"
                | "method"
                | "route_method"
                | "http_method"
                | "path"
                | "route_path"
                | "url"
                | "handler"
                | "source_path"
                | "file"
                | "file_path"
                | "path_prefix"
                | "depth"
                | "edge_limit"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported routes query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_package_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node_id"
                | "label"
                | "search"
                | "package"
                | "package_id"
                | "ecosystem"
                | "language"
                | "kind"
                | "item_kind"
                | "source"
                | "dependency_source"
                | "dependency_kind"
                | "version"
                | "dependency_version"
                | "version_kind"
                | "dependency_version_kind"
                | "path"
                | "source_path"
                | "file"
                | "file_path"
                | "path_prefix"
                | "edge_kind"
                | "kind_edge"
                | "confidence"
                | "edge_limit"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported packages query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_config_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node_id"
                | "target"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "item_kind"
                | "path"
                | "path_prefix"
                | "depth"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported configs query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_error_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node_id"
                | "target"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "item_kind"
                | "path"
                | "path_prefix"
                | "depth"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported errors query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_cycle_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node"
                | "node_id"
                | "label"
                | "search"
                | "language"
                | "path"
                | "path_prefix"
                | "kind"
                | "edge_kind"
        ) {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported cycles query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_hotspot_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node"
                | "node_id"
                | "label"
                | "search"
                | "language"
                | "kind"
                | "node_kind"
                | "item_kind"
                | "path"
                | "path_prefix"
                | "min_score"
                | "min_degree"
                | "score"
                | "edge_kind"
                | "confidence"
                | "direction"
                | "dir"
                | "edge_limit"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported hotspots query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_unreachable_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if is_node_term(key) || matches!(key.as_str(), "path_prefix" | "scope" | "search") {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported unreachable query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_diagnostic_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "label"
                | "message"
                | "severity"
                | "source"
                | "diagnostic_source"
                | "code"
                | "diagnostic_code"
                | "path"
                | "path_prefix"
                | "language"
        ) || key.starts_with("metadata.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported diagnostics query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_annotation_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "id" | "node"
                | "node_id"
                | "label"
                | "search"
                | "key"
                | "annotation"
                | "annotation_key"
                | "value"
                | "annotation_value"
                | "language"
                | "kind"
                | "node_kind"
                | "item_kind"
                | "path"
                | "path_prefix"
                | "direction"
                | "dir"
                | "edge_kind"
                | "confidence"
                | "edge_limit"
        ) || key.starts_with("metadata.")
            || key.starts_with("annotation.")
        {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported annotations query term `{key}`"
        )));
    }
    Ok(())
}

fn validate_insight_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if matches!(
            key.as_str(),
            "severity"
                | "kind"
                | "message"
                | "search"
                | "node"
                | "node_id"
                | "id"
                | "edge"
                | "edge_index"
                | "path"
                | "path_prefix"
                | "language"
        ) {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported insights query term `{key}`"
        )));
    }
    Ok(())
}

fn is_node_term(key: &str) -> bool {
    matches!(
        key,
        "id" | "kind" | "label" | "search" | "language" | "item_kind" | "package_id"
    ) || key.starts_with("metadata.")
}

fn is_edge_term(key: &str) -> bool {
    matches!(
        key,
        "kind" | "source" | "target" | "confidence" | "edge" | "edge_index"
    ) || key.starts_with("metadata.")
}

fn is_lsp_diagnostic_node(node: &Node) -> bool {
    node.metadata
        .get("item_kind")
        .is_some_and(|kind| kind == "diagnostic")
        && node
            .metadata
            .get("source")
            .is_some_and(|source| source == "lsp")
}

fn diagnostic_query_matches(
    graph: &CodeGraph,
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    let source_nodes = diagnostic_source_nodes(graph, node.id);
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "message" => node
            .metadata
            .get("message")
            .is_some_and(|value| text_matches(value, expected)),
        "severity" => node
            .metadata
            .get("severity")
            .is_some_and(|value| text_matches(value, expected)),
        "source" | "diagnostic_source" => node
            .metadata
            .get("diagnostic_source")
            .or_else(|| node.metadata.get("source"))
            .is_some_and(|value| text_matches(value, expected)),
        "code" | "diagnostic_code" => node
            .metadata
            .get("diagnostic_code")
            .is_some_and(|value| text_matches(value, expected)),
        "path" | "path_prefix" => {
            diagnostic_path_matches(node, expected)
                || source_nodes
                    .iter()
                    .filter_map(|id| graph.nodes.iter().find(|source| source.id == *id))
                    .any(|source| node_path_matches(source, path_index, expected))
        }
        "language" => source_nodes
            .iter()
            .filter_map(|id| graph.nodes.iter().find(|source| source.id == *id))
            .any(|source| metadata_matches(source, "language", expected)),
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn diagnostic_source_nodes(graph: &CodeGraph, diagnostic_id: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == diagnostic_id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "diagnostic")
        })
        .map(|edge| edge.source)
        .collect()
}

fn annotation_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    if let (Some(key), Some(value)) = (annotation_key_filter(spec), annotation_value_filter(spec))
        && !annotation_pair_matches(node, key, value)
    {
        return false;
    }

    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => node_search_matches(node, expected) || annotation_matches(node, expected),
        "key" | "annotation" | "annotation_key" => annotation_key_matches(node, expected),
        "value" | "annotation_value" => annotation_value_matches(node, expected),
        "language" | "item_kind" => metadata_matches(node, key, expected),
        "kind" | "node_kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "path_prefix" => node_path_matches(node, path_index, expected),
        "direction" | "dir" | "edge_kind" | "confidence" | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        key if key.starts_with("annotation.") => node
            .metadata
            .get(key)
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn annotation_key_filter(spec: &QuerySpec) -> Option<&str> {
    spec.terms
        .get("key")
        .or_else(|| spec.terms.get("annotation"))
        .or_else(|| spec.terms.get("annotation_key"))
        .map(String::as_str)
}

fn annotation_value_filter(spec: &QuerySpec) -> Option<&str> {
    spec.terms
        .get("value")
        .or_else(|| spec.terms.get("annotation_value"))
        .map(String::as_str)
}

fn node_has_annotation(node: &Node) -> bool {
    node.metadata
        .keys()
        .any(|key| key.starts_with("annotation."))
}

fn annotation_matches(node: &Node, expected: &str) -> bool {
    node.metadata.iter().any(|(key, value)| {
        key.starts_with("annotation.")
            && (text_matches(key.trim_start_matches("annotation."), expected)
                || text_matches(key, expected)
                || text_matches(value, expected))
    })
}

fn annotation_key_matches(node: &Node, expected: &str) -> bool {
    node.metadata.keys().any(|key| {
        key.starts_with("annotation.")
            && (text_matches(key.trim_start_matches("annotation."), expected)
                || text_matches(key, expected))
    })
}

fn annotation_value_matches(node: &Node, expected: &str) -> bool {
    node.metadata
        .iter()
        .any(|(key, value)| key.starts_with("annotation.") && text_matches(value, expected))
}

fn annotation_pair_matches(node: &Node, key_expected: &str, value_expected: &str) -> bool {
    node.metadata.iter().any(|(key, value)| {
        key.starts_with("annotation.")
            && (text_matches(key.trim_start_matches("annotation."), key_expected)
                || text_matches(key, key_expected))
            && text_matches(value, value_expected)
    })
}

fn is_framework_route_node(node: &Node) -> bool {
    node.kind == NodeKind::Entrypoint
        && node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| kind == "framework_route")
}

fn insight_query_matches(
    graph: &CodeGraph,
    insight: &Insight,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "severity" => text_matches(severity_name(insight.severity), expected),
        "kind" => text_matches(&insight.kind, expected),
        "message" => text_matches(&insight.message, expected),
        "search" => insight_search_matches(insight, &expected.to_ascii_lowercase()),
        "node" | "node_id" | "id" => insight_node_matches(graph, insight, expected),
        "edge" | "edge_index" => expected
            .parse::<usize>()
            .is_ok_and(|edge_index| insight.edges.contains(&edge_index)),
        "path" | "path_prefix" => insight.nodes.iter().any(|node_id| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == *node_id)
                .is_some_and(|node| node_path_matches(node, path_index, expected))
        }),
        "language" => insight.nodes.iter().any(|node_id| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == *node_id)
                .is_some_and(|node| metadata_matches(node, "language", expected))
        }),
        _ => false,
    })
}

fn entrypoint_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => node_search_matches(node, expected),
        "language" | "item_kind" | "entrypoint_kind" => metadata_matches(node, key, expected),
        "kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "path_prefix" => node_path_matches(node, path_index, expected),
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn symbol_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => node_search_matches(node, expected),
        "language" | "item_kind" => metadata_matches(node, key, expected),
        "kind" | "node_kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "path_prefix" => node_path_matches(node, path_index, expected),
        "direction" | "dir" | "edge_kind" | "confidence" | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn symbol_definition_edge_matches(
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    edge_kind: Option<&str>,
) -> bool {
    selected_ids.contains(&edge.target)
        && matches!(edge.kind, EdgeKind::Contains | EdgeKind::Defines)
        && edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
}

fn file_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => node_search_matches(node, expected),
        "language" | "item_kind" => metadata_matches(node, key, expected),
        "kind" | "node_kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "source_path" | "file" | "file_path" | "path_prefix" => {
            node_path_matches(node, path_index, expected)
        }
        "direction" | "dir" | "edge_kind" | "confidence" | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn file_structural_edge_matches(
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    edge_kind: Option<&str>,
) -> bool {
    selected_ids.contains(&edge.source)
        && matches!(edge.kind, EdgeKind::Contains | EdgeKind::Defines)
        && edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
}

fn file_trace_edge_touches_selected(
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    contained_code_ids: &BTreeSet<NodeId>,
    direction: NeighborDirection,
) -> bool {
    let sources = |node_id| selected_ids.contains(node_id) || contained_code_ids.contains(node_id);
    match direction {
        NeighborDirection::In => {
            selected_ids.contains(&edge.target) || contained_code_ids.contains(&edge.target)
        }
        NeighborDirection::Out => sources(&edge.source),
        NeighborDirection::Both => {
            sources(&edge.source)
                || selected_ids.contains(&edge.target)
                || contained_code_ids.contains(&edge.target)
        }
    }
}

fn is_document_query_node(node: &Node) -> bool {
    node.metadata
        .get("item_kind")
        .is_some_and(|kind| matches!(kind.as_str(), "document" | "document_section"))
        || node
            .metadata
            .get("language")
            .is_some_and(|language| language == "markdown")
}

fn document_query_matches(
    graph: &CodeGraph,
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => {
            node_search_matches(node, expected) || document_edges_search(graph, node.id, expected)
        }
        "language" | "item_kind" => metadata_matches(node, key, expected),
        "document_kind" | "doc_kind" | "type" => metadata_matches(node, "document_kind", expected),
        "heading" => metadata_matches(node, "heading", expected),
        "anchor" => metadata_matches(node, "anchor", expected),
        "kind" | "node_kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "source_path" | "file" | "file_path" | "path_prefix" => {
            node_path_matches(node, path_index, expected)
        }
        "target" => document_node_references_target(graph, node.id, expected),
        "relation" => document_node_has_relation(graph, node.id, expected),
        "edge_kind" | "confidence" | "direction" | "dir" | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn document_edge_matches(
    graph: &CodeGraph,
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
    direction: NeighborDirection,
) -> bool {
    if selected_ids.is_empty() {
        return false;
    }
    if !document_relevant_edge(graph, edge) {
        return false;
    }

    let touches_selected = match direction {
        NeighborDirection::Out => selected_ids.contains(&edge.source),
        NeighborDirection::In => selected_ids.contains(&edge.target),
        NeighborDirection::Both => {
            selected_ids.contains(&edge.source) || selected_ids.contains(&edge.target)
        }
    };
    if !touches_selected {
        return false;
    }

    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "edge_kind" => text_matches(&edge_kind_name(&edge.kind), expected),
        "confidence" => text_matches(&confidence_name(edge.confidence), expected),
        "relation" => {
            edge.kind == EdgeKind::Contains || edge_metadata_matches(edge, "relation", expected)
        }
        "target" => {
            edge.kind == EdgeKind::Contains || document_edge_target_matches(graph, edge, expected)
        }
        "path" | "source_path" | "file" | "file_path" | "path_prefix" => {
            graph.nodes.iter().any(|node| {
                (node.id == edge.source || node.id == edge.target)
                    && node_path_matches(node, path_index, expected)
            })
        }
        _ => true,
    })
}

fn document_relevant_edge(graph: &CodeGraph, edge: &Edge) -> bool {
    let source_is_doc = graph
        .nodes
        .iter()
        .find(|node| node.id == edge.source)
        .is_some_and(is_document_query_node);
    let target_is_doc = graph
        .nodes
        .iter()
        .find(|node| node.id == edge.target)
        .is_some_and(is_document_query_node);
    (edge.kind == EdgeKind::Contains && (source_is_doc || target_is_doc))
        || (edge.kind == EdgeKind::References && (source_is_doc || target_is_doc))
}

fn document_edges_search(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
    graph.edges.iter().any(|edge| {
        (edge.source == node_id || edge.target == node_id)
            && document_relevant_edge(graph, edge)
            && (edge
                .metadata
                .iter()
                .any(|(key, value)| text_matches(key, expected) || text_matches(value, expected))
                || document_edge_target_matches(graph, edge, expected))
    })
}

fn document_node_has_relation(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
    graph.edges.iter().any(|edge| {
        (edge.source == node_id || edge.target == node_id)
            && document_relevant_edge(graph, edge)
            && edge_metadata_matches(edge, "relation", expected)
    })
}

fn document_node_references_target(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
    graph.edges.iter().any(|edge| {
        edge.source == node_id
            && document_relevant_edge(graph, edge)
            && document_edge_target_matches(graph, edge, expected)
    })
}

fn document_edge_target_matches(graph: &CodeGraph, edge: &Edge, expected: &str) -> bool {
    edge.metadata
        .get("target")
        .is_some_and(|value| text_matches(value, expected))
        || edge
            .metadata
            .get("resolved_path")
            .is_some_and(|value| text_matches(value, expected))
        || graph
            .nodes
            .iter()
            .find(|node| node.id == edge.target)
            .is_some_and(|node| node_search_matches(node, expected))
}

fn is_sql_query_node(node: &Node) -> bool {
    node.metadata
        .get("language")
        .is_some_and(|language| language == "sql")
        || node
            .metadata
            .get("source")
            .is_some_and(|source| source == "sql")
        || node.metadata.get("item_kind").is_some_and(|kind| {
            matches!(
                kind.as_str(),
                "sql_schema"
                    | "sql_table"
                    | "sql_column"
                    | "sql_index"
                    | "sql_view"
                    | "app_sql_query"
            )
        })
}

fn sql_query_matches(
    graph: &CodeGraph,
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => {
            node_search_matches(node, expected)
                || sql_edges_search(graph, node.id, expected)
                || sql_table_filter_matches(node, expected)
        }
        "language" | "item_kind" | "operation" | "resolution" => {
            metadata_matches(node, key, expected)
        }
        "kind" | "node_kind" => text_matches(&kind_name(&node.kind), expected),
        "table" | "table_name" | "table_key" | "target" => sql_table_filter_matches(node, expected),
        "column" | "column_name" | "column_key" => sql_column_filter_matches(node, expected),
        "query" => metadata_matches(node, "query", expected),
        "unresolved" => sql_unresolved_filter_matches(node, expected),
        "path" | "source_path" | "file" | "file_path" | "path_prefix" => {
            node_path_matches(node, path_index, expected)
                || sql_source_nodes(graph, node.id)
                    .iter()
                    .any(|source| node_path_matches(source, path_index, expected))
        }
        "relation" => sql_node_has_relation(graph, node.id, expected),
        "edge_kind" | "confidence" | "direction" | "dir" | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn sql_edge_matches(
    graph: &CodeGraph,
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
    direction: NeighborDirection,
) -> bool {
    if !sql_relevant_edge(graph, edge) {
        return false;
    }
    let touches_selected = match direction {
        NeighborDirection::Both => {
            selected_ids.contains(&edge.source) || selected_ids.contains(&edge.target)
        }
        NeighborDirection::Out => selected_ids.contains(&edge.source),
        NeighborDirection::In => selected_ids.contains(&edge.target),
    };
    if !touches_selected {
        return false;
    }
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "relation" => edge_metadata_matches(edge, "relation", expected),
        "edge_kind" => text_matches(&edge_kind_name(&edge.kind), expected),
        "confidence" => text_matches(&confidence_name(edge.confidence), expected),
        "target" => sql_edge_target_matches(graph, edge, expected),
        "table" | "table_name" | "table_key" => {
            edge_metadata_matches(edge, "table", expected)
                || edge_metadata_matches(edge, "source_table", expected)
                || edge_metadata_matches(edge, "target_table", expected)
                || sql_edge_endpoint_matches(graph, edge, expected)
        }
        "column" | "column_name" | "column_key" => {
            edge_metadata_matches(edge, "source_column", expected)
                || edge_metadata_matches(edge, "target_column", expected)
                || sql_edge_endpoint_matches(graph, edge, expected)
        }
        "operation" => edge_metadata_matches(edge, "operation", expected),
        "path" | "source_path" | "file" | "file_path" | "path_prefix" => {
            graph.nodes.iter().any(|node| {
                (node.id == edge.source || node.id == edge.target)
                    && node_path_matches(node, path_index, expected)
            })
        }
        _ => true,
    })
}

fn sql_relevant_edge(graph: &CodeGraph, edge: &Edge) -> bool {
    if edge.metadata.get("relation").is_some_and(|relation| {
        matches!(
            relation.as_str(),
            "sql_table"
                | "sql_column"
                | "sql_index"
                | "sql_view"
                | "sql_index_table"
                | "sql_foreign_key"
                | "app_sql_query"
                | "app_sql_table_reference"
        )
    }) {
        return true;
    }
    graph
        .nodes
        .iter()
        .find(|node| node.id == edge.source)
        .is_some_and(is_sql_query_node)
        || graph
            .nodes
            .iter()
            .find(|node| node.id == edge.target)
            .is_some_and(is_sql_query_node)
}

fn sql_edges_search(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
    graph.edges.iter().any(|edge| {
        (edge.source == node_id || edge.target == node_id)
            && sql_relevant_edge(graph, edge)
            && (edge
                .metadata
                .iter()
                .any(|(key, value)| text_matches(key, expected) || text_matches(value, expected))
                || sql_edge_endpoint_matches(graph, edge, expected))
    })
}

fn sql_node_has_relation(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
    graph.edges.iter().any(|edge| {
        (edge.source == node_id || edge.target == node_id)
            && sql_relevant_edge(graph, edge)
            && edge_metadata_matches(edge, "relation", expected)
    })
}

fn sql_edge_target_matches(graph: &CodeGraph, edge: &Edge, expected: &str) -> bool {
    edge.metadata
        .get("target")
        .is_some_and(|value| text_matches(value, expected))
        || graph
            .nodes
            .iter()
            .find(|node| node.id == edge.target)
            .is_some_and(|node| node_search_matches(node, expected))
}

fn sql_edge_endpoint_matches(graph: &CodeGraph, edge: &Edge, expected: &str) -> bool {
    graph
        .nodes
        .iter()
        .filter(|node| node.id == edge.source || node.id == edge.target)
        .any(|node| node_search_matches(node, expected) || sql_table_filter_matches(node, expected))
}

fn sql_table_filter_matches(node: &Node, expected: &str) -> bool {
    ["table_name", "table_key", "target_table", "source_table"]
        .iter()
        .any(|key| metadata_matches(node, key, expected))
        || node
            .metadata
            .get("tables")
            .is_some_and(|tables| comma_list_matches(tables, expected))
        || node
            .metadata
            .get("unresolved_tables")
            .is_some_and(|tables| comma_list_matches(tables, expected))
        || node_search_matches(node, expected)
}

fn sql_column_filter_matches(node: &Node, expected: &str) -> bool {
    [
        "column_name",
        "column_key",
        "target_column",
        "source_column",
    ]
    .iter()
    .any(|key| metadata_matches(node, key, expected))
        || node_search_matches(node, expected)
}

fn sql_unresolved_filter_matches(node: &Node, expected: &str) -> bool {
    let expected = expected.trim().to_ascii_lowercase();
    let is_unresolved = node
        .metadata
        .get("unresolved_tables")
        .is_some_and(|tables| !tables.trim().is_empty())
        || node
            .metadata
            .get("resolution")
            .is_some_and(|resolution| matches!(resolution.as_str(), "unresolved" | "partial"));
    match expected.as_str() {
        "true" | "yes" | "1" | "missing" => is_unresolved,
        "false" | "no" | "0" | "resolved" => !is_unresolved,
        other => node
            .metadata
            .get("unresolved_tables")
            .is_some_and(|tables| comma_list_matches(tables, other)),
    }
}

fn comma_list_matches(value: &str, expected: &str) -> bool {
    value
        .split(',')
        .map(str::trim)
        .any(|item| !item.is_empty() && text_matches(item, expected))
}

fn sql_source_nodes(graph: &CodeGraph, node_id: NodeId) -> Vec<&Node> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            (edge.source == node_id || edge.target == node_id) && sql_relevant_edge(graph, edge)
        })
        .filter_map(|edge| {
            let other = if edge.source == node_id {
                edge.target
            } else {
                edge.source
            };
            graph.nodes.iter().find(|node| node.id == other)
        })
        .collect()
}

fn route_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => node_search_matches(node, expected),
        "language" | "framework" | "handler" => metadata_matches(node, key, expected),
        "method" | "route_method" | "http_method" => metadata_matches(node, "method", expected),
        "path" | "route_path" | "url" => metadata_matches(node, "path", expected),
        "source_path" | "file" | "file_path" | "path_prefix" => {
            node_path_matches(node, path_index, expected)
        }
        "depth" | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn route_trace_should_expand(edge: &Edge) -> bool {
    !edge
        .metadata
        .values()
        .any(|value| matches!(value.as_str(), "framework_route_file" | "entrypoint_file"))
}

fn is_package_query_node(node: &Node) -> bool {
    node.kind == NodeKind::ExternalDependency
        && node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| matches!(kind.as_str(), "dependency" | "import"))
}

fn package_query_matches(
    graph: &CodeGraph,
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => package_search_matches(node, expected),
        "package" | "package_id" => package_identifier_matches(node, expected),
        "ecosystem" => package_ecosystem(node).is_some_and(|value| text_matches(&value, expected)),
        "language" => {
            metadata_matches(node, "language", expected)
                || package_source_nodes(graph, node.id)
                    .iter()
                    .any(|source| metadata_matches(source, "language", expected))
        }
        "kind" => text_matches(&kind_name(&node.kind), expected),
        "item_kind" => metadata_matches(node, "item_kind", expected),
        "source" | "dependency_source" => {
            metadata_matches(node, "source", expected)
                || package_incoming_edges(graph, node.id)
                    .iter()
                    .any(|edge| edge_metadata_matches(edge, "source", expected))
        }
        "dependency_kind" => package_incoming_edges(graph, node.id)
            .iter()
            .any(|edge| edge_metadata_matches(edge, "dependency_kind", expected)),
        "version" | "dependency_version" => package_incoming_edges(graph, node.id)
            .iter()
            .any(|edge| edge_metadata_matches(edge, "dependency_version", expected)),
        "version_kind" | "dependency_version_kind" => package_incoming_edges(graph, node.id)
            .iter()
            .any(|edge| edge_metadata_matches(edge, "dependency_version_kind", expected)),
        "path" | "source_path" | "file" | "file_path" | "path_prefix" => {
            node_path_matches(node, path_index, expected)
                || package_source_nodes(graph, node.id)
                    .iter()
                    .any(|source| node_path_matches(source, path_index, expected))
        }
        "edge_kind" | "kind_edge" | "confidence" | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn package_edge_query_matches(
    graph: &CodeGraph,
    edge: &Edge,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "edge_kind" | "kind_edge" => text_matches(&edge_kind_name(&edge.kind), expected),
        "confidence" => text_matches(&confidence_name(edge.confidence), expected),
        "source" | "dependency_source" => edge_metadata_matches(edge, "source", expected),
        "dependency_kind" => edge_metadata_matches(edge, "dependency_kind", expected),
        "version" | "dependency_version" => {
            edge_metadata_matches(edge, "dependency_version", expected)
        }
        "version_kind" | "dependency_version_kind" => {
            edge_metadata_matches(edge, "dependency_version_kind", expected)
        }
        "path" | "source_path" | "file" | "file_path" | "path_prefix" => graph
            .nodes
            .iter()
            .find(|node| node.id == edge.source)
            .is_some_and(|node| node_path_matches(node, path_index, expected)),
        _ => true,
    })
}

fn package_search_matches(node: &Node, expected: &str) -> bool {
    text_matches(&node.label, expected)
        || node
            .metadata
            .values()
            .any(|value| text_matches(value, expected))
        || package_node_key(node).is_some_and(|key| text_matches(&key, expected))
}

fn package_identifier_matches(node: &Node, expected: &str) -> bool {
    let expected = expected.trim();
    let label_matches = node
        .metadata
        .get("item_kind")
        .is_some_and(|kind| kind == "dependency")
        && node.label.eq_ignore_ascii_case(expected);
    label_matches
        || node
            .metadata
            .get("package_id")
            .is_some_and(|value| package_key_matches(value, expected))
        || package_node_key(node).is_some_and(|key| package_key_matches(&key, expected))
}

fn package_key_matches(key: &str, expected: &str) -> bool {
    key.eq_ignore_ascii_case(expected)
        || key
            .split_once(':')
            .is_some_and(|(_, package)| package.eq_ignore_ascii_case(expected))
}

fn package_node_key(node: &Node) -> Option<String> {
    if let Some(package_id) = node.metadata.get("package_id") {
        return Some(package_id.clone());
    }
    let language = node.metadata.get("language")?;
    let package = import_package_candidate(language, &node.label)?;
    Some(package_id(&package.ecosystem, &package.package))
}

fn package_ecosystem(node: &Node) -> Option<String> {
    node.metadata.get("ecosystem").cloned().or_else(|| {
        package_node_key(node)
            .and_then(|key| key.split_once(':').map(|(value, _)| value.to_string()))
    })
}

fn package_id(ecosystem: &str, package: &str) -> String {
    format!("{}:{}", ecosystem.trim(), package.trim())
}

fn package_incoming_edges(graph: &CodeGraph, node_id: NodeId) -> Vec<&Edge> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == node_id && matches!(edge.kind, EdgeKind::Imports | EdgeKind::DependsOn)
        })
        .collect()
}

fn package_source_nodes(graph: &CodeGraph, node_id: NodeId) -> Vec<&Node> {
    package_incoming_edges(graph, node_id)
        .iter()
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.source))
        .collect()
}

fn config_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "target" | "label" => text_matches(&node.label, expected),
        "search" => config_target_matches(node, expected),
        "language" | "item_kind" => metadata_matches(node, key, expected),
        "kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "path_prefix" => node_path_matches(node, path_index, expected),
        "depth" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn error_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "target" | "label" => text_matches(&node.label, expected),
        "search" => error_target_matches(node, expected),
        "language" | "item_kind" => metadata_matches(node, key, expected),
        "kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "path_prefix" => node_path_matches(node, path_index, expected),
        "depth" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn cycle_query_matches(
    graph: &CodeGraph,
    insight: &Insight,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node" | "node_id" => insight_node_matches(graph, insight, expected),
        "label" | "search" => {
            insight.nodes.iter().any(|node_id| {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == *node_id)
                    .is_some_and(|node| node_search_matches(node, expected))
            }) || text_matches(&insight.message, expected)
        }
        "language" => insight.nodes.iter().any(|node_id| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == *node_id)
                .is_some_and(|node| metadata_matches(node, "language", expected))
        }),
        "path" | "path_prefix" => insight.nodes.iter().any(|node_id| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == *node_id)
                .is_some_and(|node| node_path_matches(node, path_index, expected))
        }),
        "kind" | "edge_kind" => insight.edges.iter().any(|edge_index| {
            graph
                .edges
                .get(*edge_index)
                .is_some_and(|edge| text_matches(&edge_kind_name(&edge.kind), expected))
        }),
        _ => false,
    })
}

fn hotspot_query_matches(
    node: &Node,
    spec: &QuerySpec,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    spec.terms.iter().all(|(key, expected)| match key.as_str() {
        "id" | "node" | "node_id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "label" => text_matches(&node.label, expected),
        "search" => node_search_matches(node, expected),
        "language" | "item_kind" => metadata_matches(node, key, expected),
        "kind" | "node_kind" => text_matches(&kind_name(&node.kind), expected),
        "path" | "path_prefix" => node_path_matches(node, path_index, expected),
        "min_score" | "min_degree" | "score" | "edge_kind" | "confidence" | "direction" | "dir"
        | "edge_limit" => true,
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn hotspot_edge_touches_selected(
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    direction: NeighborDirection,
) -> bool {
    match direction {
        NeighborDirection::In => selected_ids.contains(&edge.target),
        NeighborDirection::Out => selected_ids.contains(&edge.source),
        NeighborDirection::Both => {
            selected_ids.contains(&edge.source) || selected_ids.contains(&edge.target)
        }
    }
}

fn insight_node_matches(graph: &CodeGraph, insight: &Insight, expected: &str) -> bool {
    parse_node_id(expected).is_ok_and(|id| insight.nodes.contains(&id))
        || insight.nodes.iter().any(|node_id| {
            graph
                .nodes
                .iter()
                .find(|node| node.id == *node_id)
                .is_some_and(|node| {
                    text_matches(&node.label, expected)
                        || text_matches(&kind_name(&node.kind), expected)
                })
        })
}

fn diagnostic_path_matches(node: &Node, expected: &str) -> bool {
    let expected = normalize_path_prefix(expected);
    if expected.is_empty() {
        return true;
    }
    node.metadata
        .get("path")
        .map(|path| normalize_graph_path(path))
        .or_else(|| {
            node.span
                .as_ref()
                .map(|span| normalize_graph_path(&span.path))
        })
        .is_some_and(|path| path == expected || path.starts_with(&format!("{expected}/")))
}

fn unreachable_node_terms(spec: &QuerySpec) -> BTreeMap<String, String> {
    spec.terms
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "path_prefix" | "scope"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnreachableScope {
    SourceFiles,
    ConfigReads,
    ErrorFlows,
    AnyNode,
}

fn unreachable_scope(spec: &QuerySpec) -> Result<UnreachableScope, QueryError> {
    if let Some(scope) = spec.terms.get("scope") {
        return match scope.trim().to_ascii_lowercase().as_str() {
            "source" | "sources" | "source_file" | "source_files" | "file" | "files" => {
                Ok(UnreachableScope::SourceFiles)
            }
            "config" | "configs" | "config_read" | "config_reads" | "environment"
            | "environment_reads" | "env" | "env_reads" => Ok(UnreachableScope::ConfigReads),
            "error" | "errors" | "error_flow" | "error_flows" | "exception" | "exceptions" => {
                Ok(UnreachableScope::ErrorFlows)
            }
            "any" | "all" | "node" | "nodes" => Ok(UnreachableScope::AnyNode),
            other => Err(QueryError::new(format!(
                "invalid unreachable scope `{other}`; expected source_files, config, errors, or any"
            ))),
        };
    }

    if spec.terms.keys().any(|key| {
        matches!(key.as_str(), "id" | "kind" | "item_kind" | "package_id")
            || key.starts_with("metadata.")
    }) {
        Ok(UnreachableScope::AnyNode)
    } else {
        Ok(UnreachableScope::SourceFiles)
    }
}

fn node_matches(node: &Node, terms: &BTreeMap<String, String>) -> bool {
    terms.iter().all(|(key, expected)| match key.as_str() {
        "id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "kind" => text_matches(&kind_name(&node.kind), expected),
        "label" => text_matches(&node.label, expected),
        "search" => node_search_matches(node, expected),
        "language" | "item_kind" | "package_id" => node
            .metadata
            .get(key)
            .is_some_and(|value| text_matches(value, expected)),
        key if key.starts_with("metadata.") => node
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn slice_node_matches(
    node: &Node,
    request: &GraphSliceRequest,
    path_index: &BTreeMap<NodeId, String>,
) -> bool {
    request
        .path_prefix
        .as_deref()
        .is_none_or(|expected| node_path_matches(node, path_index, expected))
        && request
            .kind
            .as_deref()
            .is_none_or(|expected| text_matches(&kind_name(&node.kind), expected))
        && request
            .language
            .as_deref()
            .is_none_or(|expected| metadata_matches(node, "language", expected))
        && request
            .item_kind
            .as_deref()
            .is_none_or(|expected| metadata_matches(node, "item_kind", expected))
        && request
            .search
            .as_deref()
            .is_none_or(|expected| node_search_matches(node, expected))
}

fn node_path_index(graph: &CodeGraph) -> BTreeMap<NodeId, String> {
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut paths = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::File {
            paths.insert(node.id, normalize_graph_path(&node.label));
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for edge in &graph.edges {
            if edge.kind != EdgeKind::Contains {
                continue;
            }
            if paths.contains_key(&edge.target) {
                continue;
            }
            let Some(source_path) = paths.get(&edge.source).cloned() else {
                continue;
            };
            if !nodes_by_id.contains_key(&edge.target) {
                continue;
            }
            paths.insert(edge.target, source_path);
            changed = true;
        }
    }
    paths
}

fn node_path_matches(node: &Node, path_index: &BTreeMap<NodeId, String>, expected: &str) -> bool {
    let expected = normalize_path_prefix(expected);
    if expected.is_empty() {
        return true;
    }
    path_index
        .get(&node.id)
        .is_some_and(|path| path == &expected || path.starts_with(&format!("{expected}/")))
        || node
            .span
            .as_ref()
            .map(|span| normalize_graph_path(&span.path))
            .is_some_and(|path| path == expected || path.starts_with(&format!("{expected}/")))
}

fn normalize_path_prefix(value: &str) -> String {
    normalize_graph_path(value)
        .trim_end_matches('/')
        .to_string()
}

fn normalize_graph_path(value: &str) -> String {
    let mut normalized = value.trim().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }
    normalized
}

fn metadata_matches(node: &Node, key: &str, expected: &str) -> bool {
    node.metadata
        .get(key)
        .is_some_and(|value| text_matches(value, expected))
}

fn edge_metadata_matches(edge: &Edge, key: &str, expected: &str) -> bool {
    edge.metadata
        .get(key)
        .is_some_and(|value| text_matches(value, expected))
}

fn node_search_matches(node: &Node, expected: &str) -> bool {
    text_matches(&node.label, expected)
        || text_matches(&kind_name(&node.kind), expected)
        || node
            .metadata
            .iter()
            .any(|(key, value)| text_matches(key, expected) || text_matches(value, expected))
}

fn edge_matches(
    graph: &CodeGraph,
    edge_index: usize,
    edge: &Edge,
    terms: &BTreeMap<String, String>,
) -> bool {
    terms.iter().all(|(key, expected)| match key.as_str() {
        "edge" | "edge_index" => expected
            .parse::<usize>()
            .is_ok_and(|expected_index| edge_index == expected_index),
        "kind" => text_matches(&edge_kind_name(&edge.kind), expected),
        "source" => endpoint_matches(graph, edge.source, expected),
        "target" => endpoint_matches(graph, edge.target, expected),
        "confidence" => text_matches(&confidence_name(edge.confidence), expected),
        key if key.starts_with("metadata.") => edge
            .metadata
            .get(key.trim_start_matches("metadata."))
            .is_some_and(|value| text_matches(value, expected)),
        _ => false,
    })
}

fn matching_edge_indexes(
    graph: &CodeGraph,
    request: &ExplainEdgeRequest,
) -> Result<Vec<usize>, QueryError> {
    if let Some(index) = request.edge_index {
        return Ok((index < graph.edges.len())
            .then_some(index)
            .into_iter()
            .collect());
    }

    if request.source.is_none() && request.target.is_none() && request.kind.is_none() {
        return Err(QueryError::new(
            "explain edge requires `edge_index` or at least one of `source`, `target`, or `kind`",
        ));
    }

    Ok(graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| {
            request
                .source
                .as_deref()
                .is_none_or(|source| endpoint_matches(graph, edge.source, source))
                && request
                    .target
                    .as_deref()
                    .is_none_or(|target| endpoint_matches(graph, edge.target, target))
                && request
                    .kind
                    .as_deref()
                    .is_none_or(|kind| text_matches(&edge_kind_name(&edge.kind), kind))
        })
        .map(|(index, _)| index)
        .collect())
}

fn edge_evidence(edge_index: usize, source: &Node, target: &Node, edge: &Edge) -> Vec<String> {
    let mut evidence = vec![
        format!("edge_index={edge_index}"),
        format!("edge_kind={}", edge_kind_name(&edge.kind)),
        format!("confidence={}", confidence_name(edge.confidence)),
        format!(
            "source={} {} ({})",
            source.id,
            source.label,
            kind_name(&source.kind)
        ),
        format!(
            "target={} {} ({})",
            target.id,
            target.label,
            kind_name(&target.kind)
        ),
        confidence_evidence(edge.confidence).to_string(),
    ];

    if let Some(span) = &source.span {
        evidence.push(format!(
            "source_span={}:{}:{}-{}:{}",
            span.path, span.start_line, span.start_column, span.end_line, span.end_column
        ));
    }
    if let Some(span) = &target.span {
        evidence.push(format!(
            "target_span={}:{}:{}-{}:{}",
            span.path, span.start_line, span.start_column, span.end_line, span.end_column
        ));
    }
    for (key, value) in &edge.metadata {
        evidence.push(format!("metadata.{key}={value}"));
    }

    evidence
}

fn confidence_evidence(confidence: codegraph_core::Confidence) -> &'static str {
    match confidence {
        codegraph_core::Confidence::Exact => "confidence_note=declared or directly resolved fact",
        codegraph_core::Confidence::Semantic => "confidence_note=semantic tooling fact",
        codegraph_core::Confidence::Syntactic => "confidence_note=syntax-level fact",
        codegraph_core::Confidence::Heuristic => "confidence_note=pattern or name based inference",
        codegraph_core::Confidence::Unknown => "confidence_note=unknown provenance",
    }
}

fn endpoint_matches(graph: &CodeGraph, id: NodeId, expected: &str) -> bool {
    parse_node_id(expected).is_ok_and(|expected_id| expected_id == id)
        || graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .is_some_and(|node| text_matches(&node.label, expected))
}

fn endpoint_nodes(graph: &CodeGraph, edges: &[Edge]) -> Vec<Node> {
    let mut ids = BTreeSet::new();
    for edge in edges {
        ids.insert(edge.source);
        ids.insert(edge.target);
    }
    graph
        .nodes
        .iter()
        .filter(|node| ids.contains(&node.id))
        .cloned()
        .collect()
}

fn resolve_node_reference(graph: &CodeGraph, value: &str) -> Option<NodeId> {
    if let Ok(id) = parse_node_id(value) {
        return graph.nodes.iter().any(|node| node.id == id).then_some(id);
    }

    graph
        .nodes
        .iter()
        .find(|node| node.label == value)
        .or_else(|| {
            graph
                .nodes
                .iter()
                .find(|node| text_matches(&node.label, value))
        })
        .map(|node| node.id)
}

fn path_edge_matches(edge: &Edge, edge_kind: Option<&str>) -> bool {
    is_trace_edge(&edge.kind)
        && edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NeighborDirection {
    In,
    Out,
    Both,
}

fn parse_neighbor_direction(value: &str, query: &str) -> Result<NeighborDirection, QueryError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "in" | "incoming" => Ok(NeighborDirection::In),
        "out" | "outgoing" => Ok(NeighborDirection::Out),
        "both" | "any" | "all" => Ok(NeighborDirection::Both),
        other => Err(QueryError::new(format!(
            "invalid {query} direction `{other}`; expected in, out, or both"
        ))),
    }
}

fn neighbor_edge_matches(
    edge: &Edge,
    node_id: NodeId,
    direction: NeighborDirection,
    edge_kind: Option<&str>,
    confidence: Option<&str>,
) -> bool {
    let direction_matches = match direction {
        NeighborDirection::In => edge.target == node_id,
        NeighborDirection::Out => edge.source == node_id,
        NeighborDirection::Both => edge.source == node_id || edge.target == node_id,
    };
    direction_matches
        && edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
        && confidence
            .is_none_or(|expected| text_matches(&confidence_name(edge.confidence), expected))
}

fn reconstruct_path_edges(
    start: NodeId,
    target: NodeId,
    parents: &BTreeMap<NodeId, (NodeId, usize)>,
) -> Result<Vec<usize>, QueryError> {
    let mut current = target;
    let mut edges = Vec::new();
    while current != start {
        let Some((previous, edge_index)) = parents.get(&current) else {
            return Err(QueryError::new("failed to reconstruct graph path"));
        };
        edges.push(*edge_index);
        current = *previous;
    }
    edges.reverse();
    Ok(edges)
}

fn path_nodes(graph: &CodeGraph, start: NodeId, edges: &[Edge]) -> Vec<Node> {
    let mut ids = Vec::with_capacity(edges.len() + 1);
    ids.push(start);
    for edge in edges {
        ids.push(edge.target);
    }

    ids.into_iter()
        .filter_map(|id| graph.nodes.iter().find(|node| node.id == id).cloned())
        .collect()
}

fn config_target_matches(node: &Node, target: &str) -> bool {
    target.is_empty()
        || text_matches(&node.label, target)
        || node
            .metadata
            .iter()
            .any(|(key, value)| text_matches(key, target) || text_matches(value, target))
}

fn config_reader_paths(
    graph: &CodeGraph,
    reader: NodeId,
    target_edge_index: usize,
    max_depth: usize,
    limit: usize,
) -> (Vec<ConfigTracePath>, bool) {
    if limit == 0 {
        return (Vec::new(), true);
    }

    let mut paths = Vec::new();
    let mut visited = BTreeSet::from([reader]);
    let mut parents: BTreeMap<NodeId, (NodeId, usize)> = BTreeMap::new();
    let mut queue = VecDeque::from([(reader, 0usize)]);
    let mut truncated = false;

    if graph
        .nodes
        .iter()
        .find(|node| node.id == reader)
        .is_some_and(|node| node.kind == NodeKind::Entrypoint)
    {
        if let Some(path) = build_config_path(graph, reader, reader, &parents, target_edge_index) {
            paths.push(path);
        }
        return (paths, false);
    }

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth + 1 >= max_depth {
            if graph
                .edges
                .iter()
                .any(|edge| edge.target == node_id && is_upstream_flow_edge(&edge.kind))
            {
                truncated = true;
            }
            continue;
        }

        for (edge_index, edge) in graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.target == node_id && is_upstream_flow_edge(&edge.kind))
        {
            if !visited.insert(edge.source) {
                continue;
            }
            parents.insert(edge.source, (node_id, edge_index));
            let Some(source_node) = graph.nodes.iter().find(|node| node.id == edge.source) else {
                continue;
            };
            if source_node.kind == NodeKind::Entrypoint {
                if let Some(path) =
                    build_config_path(graph, edge.source, reader, &parents, target_edge_index)
                {
                    paths.push(path);
                }
                if paths.len() >= limit {
                    return (paths, true);
                }
                continue;
            }
            queue.push_back((edge.source, depth + 1));
        }
    }

    if paths.is_empty()
        && let Some(path) = build_config_path(graph, reader, reader, &parents, target_edge_index)
    {
        paths.push(path);
    }

    (paths, truncated)
}

fn build_config_path(
    graph: &CodeGraph,
    start: NodeId,
    reader: NodeId,
    parents: &BTreeMap<NodeId, (NodeId, usize)>,
    target_edge_index: usize,
) -> Option<ConfigTracePath> {
    let mut node_ids = vec![start];
    let mut edge_indexes = Vec::new();
    let mut current = start;
    while current != reader {
        let (next, edge_index) = parents.get(&current)?;
        edge_indexes.push(*edge_index);
        node_ids.push(*next);
        current = *next;
    }
    edge_indexes.push(target_edge_index);
    let target_edge = graph.edges.get(target_edge_index)?;
    if node_ids.last().copied() != Some(target_edge.source) {
        node_ids.push(target_edge.source);
    }
    node_ids.push(target_edge.target);

    let nodes = node_ids
        .into_iter()
        .filter_map(|id| graph.nodes.iter().find(|node| node.id == id).cloned())
        .collect();
    let edges = edge_indexes
        .iter()
        .filter_map(|index| graph.edges.get(*index).cloned())
        .collect();
    let reached_entrypoint = graph
        .nodes
        .iter()
        .find(|node| node.id == start)
        .is_some_and(|node| node.kind == NodeKind::Entrypoint);

    Some(ConfigTracePath {
        nodes,
        edges,
        edge_indexes,
        reached_entrypoint,
    })
}

fn error_target_matches(node: &Node, target: &str) -> bool {
    target.is_empty()
        || text_matches(&node.label, target)
        || node
            .metadata
            .iter()
            .any(|(key, value)| text_matches(key, target) || text_matches(value, target))
}

fn error_source_paths(
    graph: &CodeGraph,
    source: NodeId,
    target_edge_index: usize,
    max_depth: usize,
    limit: usize,
) -> (Vec<ErrorTracePath>, bool) {
    if limit == 0 {
        return (Vec::new(), true);
    }

    let mut paths = Vec::new();
    let mut visited = BTreeSet::from([source]);
    let mut parents: BTreeMap<NodeId, (NodeId, usize)> = BTreeMap::new();
    let mut queue = VecDeque::from([(source, 0usize)]);
    let mut truncated = false;

    if graph
        .nodes
        .iter()
        .find(|node| node.id == source)
        .is_some_and(|node| node.kind == NodeKind::Entrypoint)
    {
        if let Some(path) = build_error_path(graph, source, source, &parents, target_edge_index) {
            paths.push(path);
        }
        return (paths, false);
    }

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth + 1 >= max_depth {
            if graph
                .edges
                .iter()
                .any(|edge| edge.target == node_id && is_upstream_flow_edge(&edge.kind))
            {
                truncated = true;
            }
            continue;
        }

        for (edge_index, edge) in graph
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.target == node_id && is_upstream_flow_edge(&edge.kind))
        {
            if !visited.insert(edge.source) {
                continue;
            }
            parents.insert(edge.source, (node_id, edge_index));
            let Some(source_node) = graph.nodes.iter().find(|node| node.id == edge.source) else {
                continue;
            };
            if source_node.kind == NodeKind::Entrypoint {
                if let Some(path) =
                    build_error_path(graph, edge.source, source, &parents, target_edge_index)
                {
                    paths.push(path);
                }
                if paths.len() >= limit {
                    return (paths, true);
                }
                continue;
            }
            queue.push_back((edge.source, depth + 1));
        }
    }

    if paths.is_empty()
        && let Some(path) = build_error_path(graph, source, source, &parents, target_edge_index)
    {
        paths.push(path);
    }

    (paths, truncated)
}

fn build_error_path(
    graph: &CodeGraph,
    start: NodeId,
    source: NodeId,
    parents: &BTreeMap<NodeId, (NodeId, usize)>,
    target_edge_index: usize,
) -> Option<ErrorTracePath> {
    let mut node_ids = vec![start];
    let mut edge_indexes = Vec::new();
    let mut current = start;
    while current != source {
        let (next, edge_index) = parents.get(&current)?;
        edge_indexes.push(*edge_index);
        node_ids.push(*next);
        current = *next;
    }
    edge_indexes.push(target_edge_index);
    let target_edge = graph.edges.get(target_edge_index)?;
    if node_ids.last().copied() != Some(target_edge.source) {
        node_ids.push(target_edge.source);
    }
    node_ids.push(target_edge.target);

    let nodes = node_ids
        .into_iter()
        .filter_map(|id| graph.nodes.iter().find(|node| node.id == id).cloned())
        .collect();
    let edges = edge_indexes
        .iter()
        .filter_map(|index| graph.edges.get(*index).cloned())
        .collect();
    let reached_entrypoint = graph
        .nodes
        .iter()
        .find(|node| node.id == start)
        .is_some_and(|node| node.kind == NodeKind::Entrypoint);

    Some(ErrorTracePath {
        nodes,
        edges,
        edge_indexes,
        reached_entrypoint,
    })
}

fn is_upstream_flow_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls | EdgeKind::References | EdgeKind::Entrypoint
    )
}

fn text_matches(actual: &str, expected: &str) -> bool {
    actual
        .to_ascii_lowercase()
        .contains(&expected.to_ascii_lowercase())
}

fn increment_facet(facets: &mut BTreeMap<String, usize>, key: String) {
    *facets.entry(key).or_insert(0) += 1;
}

fn parse_node_id(value: &str) -> Result<NodeId, QueryError> {
    let value = value.trim().trim_start_matches('n');
    value
        .parse::<u64>()
        .map(NodeId)
        .map_err(|_| QueryError::new(format!("invalid node id `{value}`")))
}

fn parse_limit(value: &str) -> Result<usize, QueryError> {
    value
        .parse::<usize>()
        .map(|value| value.clamp(1, 1000))
        .map_err(|_| QueryError::new(format!("invalid limit `{value}`")))
}

fn split_query_tokens(expression: &str) -> Result<Vec<String>, QueryError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for character in expression.chars() {
        match quote {
            Some(current_quote) if character == current_quote => {
                quote = None;
            }
            Some(_) => current.push(character),
            None if character == '"' || character == '\'' => {
                quote = Some(character);
            }
            None if character.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(character),
        }
    }

    if let Some(open_quote) = quote {
        return Err(QueryError::new(format!(
            "unterminated quoted string starting with `{open_quote}`"
        )));
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

fn add_parse_error_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("skipped_reason")
            .is_some_and(|reason| reason == "max_file_size")
        {
            let file_size = node
                .metadata
                .get("file_size_bytes")
                .map(String::as_str)
                .unwrap_or("unknown");
            let max_file_size = node
                .metadata
                .get("max_file_size_bytes")
                .map(String::as_str)
                .unwrap_or("unknown");
            insights.push(Insight {
                kind: "skipped_large_file".to_string(),
                severity: InsightSeverity::Warning,
                message: format!(
                    "{} skipped because size {file_size} exceeds max file size {max_file_size}",
                    node.label
                ),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        } else if node.metadata.contains_key("parse_error") {
            insights.push(Insight {
                kind: "parse_error".to_string(),
                severity: InsightSeverity::Error,
                message: format!("{} failed to parse", node.label),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        } else if node
            .metadata
            .get("syntax_errors")
            .is_some_and(|value| value == "true")
        {
            insights.push(Insight {
                kind: "syntax_error".to_string(),
                severity: InsightSeverity::Warning,
                message: format!("{} contains syntax error nodes", node.label),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        }
    }
}

fn add_semantic_diagnostic_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "diagnostic")
        {
            continue;
        }
        if node
            .metadata
            .get("source")
            .is_none_or(|source| source != "lsp")
        {
            continue;
        }

        let diagnostic_severity = node
            .metadata
            .get("severity")
            .map(String::as_str)
            .unwrap_or("unknown");
        let severity = match diagnostic_severity {
            "error" => InsightSeverity::Error,
            "warning" => InsightSeverity::Warning,
            _ => InsightSeverity::Info,
        };
        let message = node
            .metadata
            .get("message")
            .map(String::as_str)
            .unwrap_or(node.label.as_str());
        let path = node
            .metadata
            .get("path")
            .map(String::as_str)
            .or_else(|| node.span.as_ref().map(|span| span.path.as_str()))
            .unwrap_or("unknown path");
        let line = node
            .metadata
            .get("line")
            .cloned()
            .or_else(|| node.span.as_ref().map(|span| span.start_line.to_string()))
            .unwrap_or_else(|| "?".to_string());
        let column = node
            .metadata
            .get("column")
            .cloned()
            .or_else(|| node.span.as_ref().map(|span| span.start_column.to_string()))
            .unwrap_or_else(|| "?".to_string());
        let diagnostic_source = node
            .metadata
            .get("diagnostic_source")
            .map(String::as_str)
            .unwrap_or("lsp");
        let diagnostic_code = node.metadata.get("diagnostic_code").map(String::as_str);
        let code = diagnostic_code
            .map(|value| format!(" [{value}]"))
            .unwrap_or_default();
        let diagnostic_edges = graph
            .edges
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| {
                (edge.target == node.id
                    && edge
                        .metadata
                        .get("relation")
                        .is_some_and(|relation| relation == "diagnostic"))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let mut nodes = vec![node.id];
        for edge_index in &diagnostic_edges {
            if let Some(edge) = graph.edges.get(*edge_index)
                && !nodes.contains(&edge.source)
            {
                nodes.push(edge.source);
            }
        }

        insights.push(Insight {
            kind: "semantic_diagnostic".to_string(),
            severity,
            message: format!(
                "{diagnostic_source} {diagnostic_severity} at {path}:{line}:{column}{code}: {message}"
            ),
            nodes,
            edges: diagnostic_edges,
        });
    }
}

fn insight_search_matches(insight: &Insight, expected: &str) -> bool {
    insight.kind.to_ascii_lowercase().contains(expected)
        || insight.message.to_ascii_lowercase().contains(expected)
        || insight
            .nodes
            .iter()
            .any(|node_id| node_id.0.to_string().contains(expected))
        || insight
            .edges
            .iter()
            .any(|edge_index| edge_index.to_string().contains(expected))
}

fn add_unresolved_call_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_some_and(|value| value == "call")
            && node
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "unresolved")
        {
            insights.push(Insight {
                kind: "unresolved_call".to_string(),
                severity: InsightSeverity::Warning,
                message: format!(
                    "Call target `{}` could not be resolved syntactically",
                    node.label
                ),
                nodes: vec![node.id],
                edges: incoming_edge_indexes(graph, node.id, EdgeKind::Calls),
            });
        }
    }
}

fn add_ambiguous_call_resolution_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<(NodeId, String), Vec<(usize, NodeId)>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::Calls {
            continue;
        }
        let Some(call_label) = edge
            .metadata
            .get("call_label")
            .map(|label| label.trim())
            .filter(|label| !label.is_empty())
        else {
            continue;
        };
        groups
            .entry((edge.source, call_label.to_string()))
            .or_default()
            .push((index, edge.target));
    }

    for ((caller_id, call_label), matches) in groups {
        let targets: BTreeSet<_> = matches.iter().map(|(_, target)| *target).collect();
        if targets.len() < 2 {
            continue;
        }

        let caller = node_label(graph, caller_id).unwrap_or("unknown");
        let target_labels = targets
            .iter()
            .filter_map(|target| node_label(graph, *target))
            .take(5)
            .map(|label| format!("`{label}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut nodes = Vec::with_capacity(targets.len() + 1);
        nodes.push(caller_id);
        nodes.extend(targets.iter().copied());
        let edges = matches.iter().map(|(index, _)| *index).collect();

        insights.push(Insight {
            kind: "ambiguous_call_resolution".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{caller}` calls `{call_label}` but it resolves to multiple targets: {target_labels}"
            ),
            nodes,
            edges,
        });
    }
}

fn add_unresolved_local_import_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::ExternalDependency
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "import")
            || node
                .metadata
                .get("import_scope")
                .is_none_or(|scope| scope != "local")
            || node
                .metadata
                .get("resolution")
                .is_none_or(|resolution| resolution != "unresolved")
        {
            continue;
        }
        let target = node
            .metadata
            .get("import_target")
            .map(String::as_str)
            .unwrap_or(node.label.as_str());
        let edges = incoming_edge_indexes(graph, node.id, EdgeKind::Imports);
        let source = edges
            .first()
            .and_then(|index| graph.edges.get(*index))
            .and_then(|edge| node_label(graph, edge.source))
            .unwrap_or("unknown");

        insights.push(Insight {
            kind: "unresolved_local_import".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{source}` imports local target `{target}` but no matching file was found"
            ),
            nodes: std::iter::once(node.id)
                .chain(
                    edges
                        .iter()
                        .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
                )
                .collect(),
            edges,
        });
    }
}

fn add_unresolved_sql_table_reference_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "app_sql_query")
        {
            continue;
        }
        let Some(tables) = node
            .metadata
            .get("unresolved_tables")
            .map(|tables| tables.trim())
            .filter(|tables| !tables.is_empty())
        else {
            continue;
        };

        let incoming = incoming_edge_indexes(graph, node.id, EdgeKind::References);
        let outgoing = outgoing_edge_indexes(graph, node.id, EdgeKind::References);
        let mut edges = incoming
            .iter()
            .chain(outgoing.iter())
            .copied()
            .collect::<Vec<_>>();
        edges.sort_unstable();
        edges.dedup();

        let mut nodes = std::iter::once(node.id)
            .chain(
                edges
                    .iter()
                    .filter_map(|index| graph.edges.get(*index))
                    .flat_map(|edge| [edge.source, edge.target]),
            )
            .collect::<Vec<_>>();
        nodes.sort_unstable();
        nodes.dedup();

        let operation = node
            .metadata
            .get("operation")
            .map(String::as_str)
            .unwrap_or("sql");
        let source = incoming
            .first()
            .and_then(|index| graph.edges.get(*index))
            .and_then(|edge| node_label(graph, edge.source))
            .unwrap_or("unknown source");

        insights.push(Insight {
            kind: "unresolved_sql_table_reference".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{source}` has {operation} SQL query `{}` referencing table(s) `{tables}` without a matching indexed schema table",
                node.label
            ),
            nodes,
            edges,
        });
    }
}

fn add_cross_language_heuristic_edge_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind)
            || !matches!(
                edge.confidence,
                codegraph_core::Confidence::Heuristic | codegraph_core::Confidence::Unknown
            )
        {
            continue;
        }
        let source_language = node_language(&nodes_by_id, edge.source);
        let target_language = node_language(&nodes_by_id, edge.target);
        if source_language == "unknown"
            || target_language == "unknown"
            || source_language == target_language
        {
            continue;
        }
        let source = nodes_by_id
            .get(&edge.source)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        let target = nodes_by_id
            .get(&edge.target)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        insights.push(Insight {
            kind: "cross_language_heuristic_edge".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{source}` ({source_language}) {} `{target}` ({target_language}) with {} confidence",
                edge_kind_name(&edge.kind),
                confidence_name(edge.confidence)
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![edge_index],
        });
    }
}

fn add_duplicate_function_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<&str, Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::Function {
            groups.entry(&node.label).or_default().push(node.id);
        }
    }

    for (label, nodes) in groups {
        if nodes.len() > 1 {
            insights.push(Insight {
                kind: "duplicate_function_label".to_string(),
                severity: InsightSeverity::Info,
                message: format!("Function label `{label}` appears {} times", nodes.len()),
                nodes,
                edges: Vec::new(),
            });
        }
    }
}

fn add_duplicate_compose_published_port_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<(String, String), Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "compose_port")
        {
            continue;
        }
        let Some(published) = node
            .metadata
            .get("published_port")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let protocol = node
            .metadata
            .get("protocol")
            .map(String::as_str)
            .unwrap_or("tcp")
            .to_ascii_lowercase();
        groups
            .entry((published.to_string(), protocol))
            .or_default()
            .push(node.id);
    }

    for ((published, protocol), nodes) in groups {
        if nodes.len() <= 1 {
            continue;
        }
        let services = nodes
            .iter()
            .filter_map(|node_id| graph.nodes.iter().find(|node| node.id == *node_id))
            .filter_map(|node| node.metadata.get("service").cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        let mut edges = Vec::new();
        for node_id in &nodes {
            edges.extend(incoming_edge_indexes(graph, *node_id, EdgeKind::References));
        }
        insights.push(Insight {
            kind: "duplicate_compose_published_port".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Docker Compose published port `{published}/{protocol}` is declared by multiple services: {services}"
            ),
            nodes,
            edges,
        });
    }
}

fn add_duplicate_entrypoint_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<&str, Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::Entrypoint {
            groups.entry(&node.label).or_default().push(node.id);
        }
    }

    for (label, nodes) in groups {
        if nodes.len() <= 1 {
            continue;
        }

        let edges = nodes
            .iter()
            .flat_map(|node_id| incoming_edge_indexes(graph, *node_id, EdgeKind::Entrypoint))
            .collect();

        insights.push(Insight {
            kind: "duplicate_entrypoint_label".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Entrypoint label `{label}` appears {} times and may make label-based traces ambiguous",
                nodes.len()
            ),
            nodes,
            edges,
        });
    }
}

fn add_ambiguous_entrypoint_target_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "manifest_entrypoint")
        {
            continue;
        }

        for relation in ["entrypoint_file", "entrypoint_function"] {
            let matches = graph
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| {
                    edge.source == node.id
                        && edge.kind == EdgeKind::References
                        && edge
                            .metadata
                            .get("relation")
                            .is_some_and(|value| value == relation)
                })
                .collect::<Vec<_>>();
            let targets = matches
                .iter()
                .map(|(_, edge)| edge.target)
                .collect::<BTreeSet<_>>();
            if targets.len() < 2 {
                continue;
            }

            let relation_label = if relation == "entrypoint_file" {
                "files"
            } else {
                "functions"
            };
            let target_labels = targets
                .iter()
                .filter_map(|target| node_label(graph, *target))
                .take(5)
                .map(|label| format!("`{label}`"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut nodes = Vec::with_capacity(targets.len() + 1);
            nodes.push(node.id);
            nodes.extend(targets.iter().copied());
            let edges = matches.iter().map(|(index, _)| *index).collect();

            insights.push(Insight {
                kind: "ambiguous_entrypoint_target".to_string(),
                severity: InsightSeverity::Warning,
                message: format!(
                    "Entrypoint `{}` resolves to multiple {relation_label}: {target_labels}",
                    node.label
                ),
                nodes,
                edges,
            });
        }
    }
}

fn add_orphan_function_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let entrypoints: BTreeSet<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Entrypoint)
        .map(|edge| edge.target)
        .collect();
    let called: BTreeSet<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Calls
                || (edge.kind == EdgeKind::References
                    && edge
                        .metadata
                        .get("relation")
                        .is_some_and(|relation| relation == "entrypoint_function"))
        })
        .map(|edge| edge.target)
        .collect();

    for node in &graph.nodes {
        if node.kind == NodeKind::Function
            && !entrypoints.contains(&node.id)
            && !called.contains(&node.id)
        {
            insights.push(Insight {
                kind: "orphan_function".to_string(),
                severity: InsightSeverity::Info,
                message: format!("Function `{}` has no incoming call edge", node.label),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        }
    }
}

fn add_error_flow_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::MayError {
            continue;
        }
        let source = graph
            .nodes
            .iter()
            .find(|node| node.id == edge.source)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        let target = graph
            .nodes
            .iter()
            .find(|node| node.id == edge.target)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        insights.push(Insight {
            kind: "potential_error_flow".to_string(),
            severity: InsightSeverity::Warning,
            message: format!("`{source}` may error via `{target}`"),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

fn add_unresolved_entrypoint_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "manifest_entrypoint")
        {
            continue;
        }
        let Some(target) = node
            .metadata
            .get("target")
            .map(|target| target.trim())
            .filter(|target| !target.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge.metadata.get("relation").is_some_and(|relation| {
                    matches!(relation.as_str(), "entrypoint_file" | "entrypoint_function")
                })
        });
        if resolved {
            continue;
        }

        insights.push(Insight {
            kind: "unresolved_entrypoint_target".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Entrypoint `{}` declares target `{target}` but no matching file or function was found",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
}

fn add_unresolved_dockerfile_command_path_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    add_unresolved_workflow_command_path_insights(
        graph,
        insights,
        "dockerfile_entrypoint",
        "docker_command_path",
        "unresolved_dockerfile_command_path",
        "Dockerfile instruction",
    );
}

fn add_unresolved_compose_command_path_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    add_unresolved_workflow_command_path_insights(
        graph,
        insights,
        "compose_service",
        "compose_command_path",
        "unresolved_compose_command_path",
        "Compose service",
    );
}

fn add_unresolved_compose_env_file_path_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "compose_env_file")
        {
            continue;
        }
        let Some(env_file_path) = node
            .metadata
            .get("env_file_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "compose_env_file_path")
        });
        if resolved {
            continue;
        }

        let service = node
            .metadata
            .get("service")
            .map(String::as_str)
            .unwrap_or("unknown");
        let mut nodes = vec![node.id];
        nodes.extend(compose_env_file_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_compose_env_file_path".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Compose service `{service}` references env_file `{env_file_path}` but the file was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::ReadsConfig),
        });
    }
}

fn compose_env_file_reader_ids(graph: &CodeGraph, config: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == config
                && edge.kind == EdgeKind::ReadsConfig
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "compose_env_file")
        })
        .map(|edge| edge.source)
        .collect()
}

fn add_unresolved_compose_volume_source_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "compose_volume")
        {
            continue;
        }
        let Some(source_path) = node
            .metadata
            .get("local_source_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "compose_volume_source_path")
        });
        if resolved {
            continue;
        }

        let service = node
            .metadata
            .get("service")
            .map(String::as_str)
            .unwrap_or("unknown");
        let target_path = node
            .metadata
            .get("target_path")
            .map(String::as_str)
            .unwrap_or("unknown");
        let mut nodes = vec![node.id];
        nodes.extend(compose_volume_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_compose_volume_source_path".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Compose service `{service}` mounts local source `{source_path}` to `{target_path}` but the source path was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

fn compose_volume_reader_ids(graph: &CodeGraph, volume: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == volume
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "compose_volume")
        })
        .map(|edge| edge.source)
        .collect()
}

fn add_unresolved_github_actions_job_need_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "github_actions_job")
        {
            continue;
        }
        let workflow = node
            .metadata
            .get("workflow")
            .map(String::as_str)
            .unwrap_or("workflow");
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        for dependency in metadata_list(node, "needs") {
            if github_actions_job_exists(graph, workflow, &dependency) {
                continue;
            }
            insights.push(Insight {
                kind: "unresolved_github_actions_job_need".to_string(),
                severity: InsightSeverity::Warning,
                message: format!(
                    "GitHub Actions job `{workflow}/{job}` declares need `{dependency}` but no matching job was found"
                ),
                nodes: vec![node.id],
                edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
            });
        }
    }
}

fn github_actions_job_exists(graph: &CodeGraph, workflow: &str, job: &str) -> bool {
    graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Entrypoint
            && node
                .metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "github_actions_job")
            && node
                .metadata
                .get("workflow")
                .is_some_and(|value| value == workflow)
            && node.metadata.get("job").is_some_and(|value| value == job)
    })
}

fn add_unresolved_github_actions_local_action_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "github_actions_local_action")
        {
            continue;
        }
        let Some(local_action_path) = node
            .metadata
            .get("local_action_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "github_actions_local_action_path")
        });
        if resolved {
            continue;
        }

        let workflow = node
            .metadata
            .get("workflow")
            .map(String::as_str)
            .unwrap_or("workflow");
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        let mut nodes = vec![node.id];
        nodes.extend(github_actions_local_action_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_github_actions_local_action".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "GitHub Actions job `{workflow}/{job}` uses local action `{local_action_path}` but no matching action directory, action.yml, action.yaml, or Dockerfile was found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::DependsOn),
        });
    }
}

fn github_actions_local_action_reader_ids(graph: &CodeGraph, action: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == action
                && edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "github_actions_uses")
        })
        .map(|edge| edge.source)
        .collect()
}

fn add_unresolved_github_actions_run_path_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "github_actions_run_step")
        {
            continue;
        }
        let Some(command_path) = node
            .metadata
            .get("command_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let reader_ids = github_actions_run_step_reader_ids(graph, node.id);
        let resolved = github_actions_run_path_is_resolved(graph, &reader_ids, command_path);
        if resolved {
            continue;
        }

        let workflow = node
            .metadata
            .get("workflow")
            .map(String::as_str)
            .unwrap_or("workflow");
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        let command = node
            .metadata
            .get("command")
            .map(String::as_str)
            .unwrap_or(command_path);
        let mut nodes = vec![node.id];
        nodes.extend(reader_ids);
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_github_actions_run_path".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "GitHub Actions job `{workflow}/{job}` runs `{command}` but command path `{command_path}` was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

fn github_actions_run_step_reader_ids(graph: &CodeGraph, step: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == step
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "github_actions_run")
        })
        .map(|edge| edge.source)
        .collect()
}

fn github_actions_run_path_is_resolved(
    graph: &CodeGraph,
    reader_ids: &[NodeId],
    command_path: &str,
) -> bool {
    graph.edges.iter().any(|edge| {
        reader_ids.contains(&edge.source)
            && edge.kind == EdgeKind::References
            && graph
                .nodes
                .iter()
                .find(|node| node.id == edge.target)
                .is_some_and(|node| node.label == command_path)
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "github_actions_run_command_path")
    })
}

fn add_unresolved_gitlab_ci_job_dependency_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "gitlab_ci_job")
        {
            continue;
        }
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        for (field, relation_label) in [("needs", "need"), ("dependencies", "dependency")] {
            for dependency in metadata_list(node, field) {
                if gitlab_ci_job_exists(graph, &dependency) {
                    continue;
                }
                insights.push(Insight {
                    kind: "unresolved_gitlab_ci_job_dependency".to_string(),
                    severity: InsightSeverity::Warning,
                    message: format!(
                        "GitLab CI job `{job}` declares {relation_label} `{dependency}` but no matching job was found"
                    ),
                    nodes: vec![node.id],
                    edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
                });
            }
        }
    }
}

fn gitlab_ci_job_exists(graph: &CodeGraph, job: &str) -> bool {
    graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Entrypoint
            && node
                .metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "gitlab_ci_job")
            && node.metadata.get("job").is_some_and(|value| value == job)
    })
}

fn metadata_list(node: &Node, key: &str) -> Vec<String> {
    node.metadata
        .get(key)
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn add_unresolved_gitlab_ci_script_path_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "gitlab_ci_script")
        {
            continue;
        }
        let Some(command_path) = node
            .metadata
            .get("command_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let reader_ids = gitlab_ci_script_reader_ids(graph, node.id);
        let resolved = gitlab_ci_script_path_is_resolved(graph, &reader_ids, command_path);
        if resolved {
            continue;
        }

        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        let command = node
            .metadata
            .get("command")
            .map(String::as_str)
            .unwrap_or(command_path);
        let mut nodes = vec![node.id];
        nodes.extend(reader_ids);
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_gitlab_ci_script_path".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "GitLab CI job `{job}` runs `{command}` but command path `{command_path}` was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

fn gitlab_ci_script_reader_ids(graph: &CodeGraph, script: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == script
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "gitlab_ci_script")
        })
        .map(|edge| edge.source)
        .collect()
}

fn gitlab_ci_script_path_is_resolved(
    graph: &CodeGraph,
    reader_ids: &[NodeId],
    command_path: &str,
) -> bool {
    graph.edges.iter().any(|edge| {
        reader_ids.contains(&edge.source)
            && edge.kind == EdgeKind::References
            && graph
                .nodes
                .iter()
                .find(|node| node.id == edge.target)
                .is_some_and(|node| node.label == command_path)
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "gitlab_ci_script_command_path")
    })
}

fn add_unresolved_kubernetes_config_ref_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "kubernetes_config_ref")
        {
            continue;
        }
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "kubernetes_config_ref")
        });
        if resolved {
            continue;
        }

        let config_kind = node
            .metadata
            .get("config_kind")
            .map(String::as_str)
            .unwrap_or("config");
        let name = node
            .metadata
            .get("name")
            .map(String::as_str)
            .unwrap_or("unknown");
        let namespace = node
            .metadata
            .get("namespace")
            .map(String::as_str)
            .unwrap_or("default");
        let workload = node
            .metadata
            .get("workload")
            .map(String::as_str)
            .unwrap_or("unknown");
        let workload_kind = node
            .metadata
            .get("workload_kind")
            .map(String::as_str)
            .unwrap_or("workload");
        let mut nodes = vec![node.id];
        nodes.extend(kubernetes_config_ref_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_kubernetes_config_ref".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Kubernetes {workload_kind} `{workload}` references {config_kind} `{namespace}/{name}` but no matching manifest was found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::ReadsConfig),
        });
    }
}

fn kubernetes_config_ref_reader_ids(graph: &CodeGraph, config_ref: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == config_ref
                && edge.kind == EdgeKind::ReadsConfig
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "kubernetes_config_ref")
        })
        .map(|edge| edge.source)
        .collect()
}

fn add_unresolved_kubernetes_ingress_backend_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "kubernetes_service_ref")
        {
            continue;
        }
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "kubernetes_service_ref")
        });
        if resolved {
            continue;
        }

        let name = node
            .metadata
            .get("name")
            .map(String::as_str)
            .unwrap_or("unknown");
        let namespace = node
            .metadata
            .get("namespace")
            .map(String::as_str)
            .unwrap_or("default");
        let ingress = node
            .metadata
            .get("ingress")
            .map(String::as_str)
            .unwrap_or("unknown");
        let route = kubernetes_ingress_route_label(node);
        let mut nodes = vec![node.id];
        nodes.extend(kubernetes_service_ref_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_kubernetes_ingress_backend".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Kubernetes Ingress `{ingress}` routes {route} to Service `{namespace}/{name}` but no matching Service manifest was found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

fn kubernetes_service_ref_reader_ids(graph: &CodeGraph, service_ref: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == service_ref
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "kubernetes_ingress_backend")
        })
        .map(|edge| edge.source)
        .collect()
}

fn kubernetes_ingress_route_label(node: &Node) -> String {
    let host = node.metadata.get("host").map(String::as_str).unwrap_or("*");
    let path = node.metadata.get("path").map(String::as_str).unwrap_or("/");
    format!("`{host}{path}`")
}

fn add_unresolved_kubernetes_service_selector_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "kubernetes_service")
        {
            continue;
        }
        let Some(selector) = node
            .metadata
            .get("selector")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if kubernetes_service_selector_is_resolved(graph, node.id) {
            continue;
        }

        let name = node
            .metadata
            .get("name")
            .map(String::as_str)
            .unwrap_or("unknown");
        let namespace = node
            .metadata
            .get("namespace")
            .map(String::as_str)
            .unwrap_or("default");
        insights.push(Insight {
            kind: "unresolved_kubernetes_service_selector".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Kubernetes Service `{namespace}/{name}` selector `{selector}` does not match any scanned workload"
            ),
            nodes: vec![node.id],
            edges: Vec::new(),
        });
    }
}

fn kubernetes_service_selector_is_resolved(graph: &CodeGraph, service: NodeId) -> bool {
    graph.edges.iter().any(|edge| {
        edge.source == service
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "kubernetes_service_selector")
    })
}

fn add_unresolved_makefile_command_path_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    add_unresolved_workflow_command_path_insights(
        graph,
        insights,
        "makefile_target",
        "make_command_path",
        "unresolved_makefile_command_path",
        "Makefile target",
    );
}

fn add_unresolved_workflow_command_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
    item_kind: &str,
    resolution: &str,
    insight_kind: &str,
    label_prefix: &str,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != item_kind)
        {
            continue;
        }
        let Some(command_path) = node
            .metadata
            .get("command_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == resolution)
        });
        if resolved {
            continue;
        }

        let command = node
            .metadata
            .get("command")
            .map(String::as_str)
            .unwrap_or(command_path);
        insights.push(Insight {
            kind: insight_kind.to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{label_prefix} `{}` runs `{command}` but command path `{command_path}` was not found",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
}

fn add_entrypoint_dead_end_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || entrypoint_has_outgoing_trace_edge(graph, node.id)
            || unresolved_manifest_entrypoint_target(graph, node)
            || unresolved_framework_route_handler_target(graph, node)
        {
            continue;
        }

        insights.push(Insight {
            kind: "entrypoint_dead_end".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Entrypoint `{}` has no outgoing code, config, dependency, or error flow",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
}

fn entrypoint_has_outgoing_trace_edge(graph: &CodeGraph, node_id: NodeId) -> bool {
    graph
        .edges
        .iter()
        .any(|edge| edge.source == node_id && is_trace_edge(&edge.kind))
}

fn unresolved_manifest_entrypoint_target(graph: &CodeGraph, node: &Node) -> bool {
    if node
        .metadata
        .get("item_kind")
        .is_none_or(|kind| kind != "manifest_entrypoint")
        || node
            .metadata
            .get("target")
            .map(|target| target.trim())
            .is_none_or(str::is_empty)
    {
        return false;
    }

    !graph.edges.iter().any(|edge| {
        edge.source == node.id
            && edge.kind == EdgeKind::References
            && edge.metadata.get("relation").is_some_and(|relation| {
                matches!(relation.as_str(), "entrypoint_file" | "entrypoint_function")
            })
    })
}

fn unresolved_framework_route_handler_target(graph: &CodeGraph, node: &Node) -> bool {
    if node
        .metadata
        .get("item_kind")
        .is_none_or(|kind| kind != "framework_route")
        || node
            .metadata
            .get("handler")
            .map(|handler| handler.trim())
            .is_none_or(str::is_empty)
    {
        return false;
    }

    !graph.edges.iter().any(|edge| {
        edge.source == node.id
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|resolution| resolution == "framework_route_handler")
    })
}

fn add_unreachable_config_read_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let reachable = entrypoint_reachable_nodes(graph);
    if reachable.is_empty() {
        return;
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if !matches!(
            edge.kind,
            EdgeKind::ReadsConfig | EdgeKind::ReadsEnvironment
        ) || reachable.contains(&edge.source)
        {
            continue;
        }

        let reader = node_label(graph, edge.source).unwrap_or("unknown");
        let target = node_label(graph, edge.target).unwrap_or("unknown");
        insights.push(Insight {
            kind: "unreachable_config_read".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{reader}` reads `{target}` but is not reachable from any entrypoint"
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

fn add_unreachable_error_flow_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let reachable = entrypoint_reachable_nodes(graph);
    if reachable.is_empty() {
        return;
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::MayError || reachable.contains(&edge.source) {
            continue;
        }

        let source = node_label(graph, edge.source).unwrap_or("unknown");
        let target = node_label(graph, edge.target).unwrap_or("unknown");
        insights.push(Insight {
            kind: "unreachable_error_flow".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{source}` may error via `{target}` but is not reachable from any entrypoint"
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

fn add_unreachable_source_file_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let reachable = entrypoint_reachable_nodes(graph);
    if reachable.is_empty() {
        return;
    }

    let source_files = graph
        .nodes
        .iter()
        .filter(|node| is_source_file_candidate(graph, node));
    for file in source_files {
        if reachable.contains(&file.id) || file_has_reachable_code(graph, file.id, &reachable) {
            continue;
        }

        let language = file
            .metadata
            .get("language")
            .map(String::as_str)
            .unwrap_or("unknown");
        insights.push(Insight {
            kind: "unreachable_source_file".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "`{}` contains {language} code but is not reachable from any entrypoint",
                file.label
            ),
            nodes: vec![file.id],
            edges: contained_code_edge_indexes(graph, file.id),
        });
    }
}

fn add_conflicting_config_default_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<(String, String), Vec<(NodeId, String)>> = BTreeMap::new();
    for node in &graph.nodes {
        if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
            continue;
        }
        let Some(default_value) = node
            .metadata
            .get("default_value")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        groups
            .entry((kind_name(&node.kind), node.label.clone()))
            .or_default()
            .push((node.id, default_value.to_string()));
    }

    for ((kind, label), node_defaults) in groups {
        let mut default_values = BTreeMap::<String, Vec<NodeId>>::new();
        for (node_id, default_value) in node_defaults {
            default_values
                .entry(default_value)
                .or_default()
                .push(node_id);
        }
        if default_values.len() < 2 {
            continue;
        }

        let nodes: Vec<_> = default_values
            .values()
            .flat_map(|ids| ids.iter().copied())
            .collect();
        let edge_kind = if kind == "environment" {
            EdgeKind::ReadsEnvironment
        } else {
            EdgeKind::ReadsConfig
        };
        let edges: Vec<_> = nodes
            .iter()
            .flat_map(|node_id| incoming_edge_indexes(graph, *node_id, edge_kind.clone()))
            .collect();
        let values = format_backtick_list(default_values.keys().map(String::as_str), 8);

        insights.push(Insight {
            kind: "conflicting_config_default".to_string(),
            severity: InsightSeverity::Warning,
            message: format!("{kind} `{label}` is read with multiple fallback values: {values}"),
            nodes,
            edges,
        });
    }
}

fn add_mixed_config_requirement_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    #[derive(Default)]
    struct ConfigRequirementGroup {
        required_nodes: Vec<NodeId>,
        default_nodes: BTreeMap<String, Vec<NodeId>>,
    }

    let mut groups: BTreeMap<(String, String), ConfigRequirementGroup> = BTreeMap::new();
    for node in &graph.nodes {
        if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
            continue;
        }
        let edge_kind = if node.kind == NodeKind::Environment {
            EdgeKind::ReadsEnvironment
        } else {
            EdgeKind::ReadsConfig
        };
        if incoming_edge_indexes(graph, node.id, edge_kind).is_empty() {
            continue;
        }

        let entry = groups
            .entry((kind_name(&node.kind), node.label.clone()))
            .or_default();
        if let Some(default_value) = node
            .metadata
            .get("default_value")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            entry
                .default_nodes
                .entry(default_value.to_string())
                .or_default()
                .push(node.id);
        } else {
            entry.required_nodes.push(node.id);
        }
    }

    for ((kind, label), group) in groups {
        if group.required_nodes.is_empty() || group.default_nodes.is_empty() {
            continue;
        }

        let edge_kind = if kind == "environment" {
            EdgeKind::ReadsEnvironment
        } else {
            EdgeKind::ReadsConfig
        };
        let mut nodes = group.required_nodes.clone();
        nodes.extend(
            group
                .default_nodes
                .values()
                .flat_map(|ids| ids.iter().copied()),
        );
        let edges: Vec<_> = nodes
            .iter()
            .flat_map(|node_id| incoming_edge_indexes(graph, *node_id, edge_kind.clone()))
            .collect();
        let values = format_backtick_list(group.default_nodes.keys().map(String::as_str), 8);

        insights.push(Insight {
            kind: "mixed_config_requirement".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{kind} `{label}` is read both as required and with fallback values: {values}"
            ),
            nodes,
            edges,
        });
    }
}

fn add_sensitive_config_default_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
            continue;
        }
        let Some(default_value) = node
            .metadata
            .get("default_value")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if !sensitive_config_default_candidate(&node.label, default_value) {
            continue;
        }

        let edge_kind = if node.kind == NodeKind::Environment {
            EdgeKind::ReadsEnvironment
        } else {
            EdgeKind::ReadsConfig
        };
        let edges = incoming_edge_indexes(graph, node.id, edge_kind);
        if edges.is_empty() {
            continue;
        }

        let kind = kind_name(&node.kind);
        insights.push(Insight {
            kind: "sensitive_config_default".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{kind} `{}` looks sensitive and has a non-empty fallback value",
                node.label
            ),
            nodes: std::iter::once(node.id)
                .chain(
                    edges
                        .iter()
                        .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
                )
                .collect(),
            edges,
        });
    }
}

fn add_undeclared_flutter_asset_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let declared_assets = flutter_declared_assets(graph);
    if declared_assets.is_empty() {
        return;
    }

    let mut reported = BTreeSet::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::ReadsConfig {
            continue;
        }
        let Some(target) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        let Some(asset_path) = flutter_asset_read_path(target) else {
            continue;
        };
        if flutter_asset_is_declared(&asset_path, &declared_assets) {
            continue;
        }
        if !reported.insert(asset_path.clone()) {
            continue;
        }
        let reader = node_label(graph, edge.source).unwrap_or("unknown");
        insights.push(Insight {
            kind: "undeclared_flutter_asset".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{reader}` reads Flutter asset `{asset_path}` but no matching `pubspec.yaml` asset declaration was found"
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![edge_index],
        });
    }
}

fn flutter_declared_assets(graph: &CodeGraph) -> Vec<String> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Config
                && node
                    .metadata
                    .get("item_kind")
                    .is_some_and(|value| value == "flutter_asset")
        })
        .filter_map(|node| {
            node.metadata.get("asset_path").cloned().or_else(|| {
                node.label
                    .strip_prefix("flutter asset:")
                    .map(str::to_string)
            })
        })
        .collect()
}

fn flutter_asset_read_path(node: &Node) -> Option<String> {
    if node.kind != NodeKind::Config {
        return None;
    }
    if node
        .metadata
        .get("config_kind")
        .is_some_and(|value| value == "flutter_asset_read")
    {
        return node.metadata.get("value").cloned().or_else(|| {
            node.label
                .strip_prefix("flutter asset read:")
                .map(str::to_string)
        });
    }
    let label = node.label.trim();
    (looks_like_flutter_asset_path(label)).then(|| label.to_string())
}

fn looks_like_flutter_asset_path(path: &str) -> bool {
    let path = path.trim();
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains("://")
        && (path.starts_with("assets/")
            || path.starts_with("asset/")
            || path.contains("/assets/")
            || path.contains("/asset/"))
}

fn flutter_asset_is_declared(asset_path: &str, declarations: &[String]) -> bool {
    declarations.iter().any(|declared| {
        let declared = declared.trim();
        !declared.is_empty()
            && (asset_path == declared
                || (declared.ends_with('/') && asset_path.starts_with(declared)))
    })
}

fn add_rationale_risk_comment_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "rationale_comment")
        {
            continue;
        }
        let Some(kind) = node.metadata.get("rationale_kind").map(String::as_str) else {
            continue;
        };
        let severity = match kind {
            "security" => InsightSeverity::Error,
            "fixme" | "hack" | "bug" | "xxx" => InsightSeverity::Warning,
            _ => continue,
        };
        let edges = incoming_edge_indexes(graph, node.id, EdgeKind::Contains);
        let location = node
            .span
            .as_ref()
            .map(|span| format!("{}:{}", span.path, span.start_line))
            .unwrap_or_else(|| "unknown location".to_string());
        insights.push(Insight {
            kind: "rationale_risk_comment".to_string(),
            severity,
            message: format!(
                "{} comment `{}` should be reviewed at {location}",
                kind.to_ascii_uppercase(),
                node.label
            ),
            nodes: std::iter::once(node.id)
                .chain(
                    edges
                        .iter()
                        .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
                )
                .collect(),
            edges,
        });
    }
}

fn add_sensitive_ci_environment_literal_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Environment
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "ci_environment")
            || node
                .metadata
                .get("value_kind")
                .is_none_or(|kind| kind != "literal")
            || !sensitive_config_label(&node.label)
        {
            continue;
        }
        let edges = incoming_edge_indexes(graph, node.id, EdgeKind::ReadsEnvironment);
        if edges.is_empty() {
            continue;
        }
        let source = node
            .metadata
            .get("source")
            .map(String::as_str)
            .unwrap_or("ci");
        let scope = node
            .metadata
            .get("scope")
            .map(String::as_str)
            .unwrap_or("job");
        insights.push(Insight {
            kind: "sensitive_ci_environment_literal".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{source} {scope} environment `{}` looks sensitive and is assigned a literal value",
                node.label
            ),
            nodes: std::iter::once(node.id)
                .chain(
                    edges
                        .iter()
                        .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
                )
                .collect(),
            edges,
        });
    }
}

fn sensitive_config_default_candidate(label: &str, default_value: &str) -> bool {
    sensitive_config_label(label) || credential_like_default(default_value)
}

fn sensitive_config_label(label: &str) -> bool {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("public_key")
        && !normalized.contains("private_key")
        && !normalized.contains("secret")
    {
        return false;
    }

    [
        "password",
        "passwd",
        "passphrase",
        "secret",
        "token",
        "credential",
        "private_key",
        "api_key",
        "access_key",
        "signing_key",
        "encryption_key",
        "jwt",
    ]
    .iter()
    .any(|indicator| normalized.contains(indicator))
}

fn credential_like_default(default_value: &str) -> bool {
    let normalized = default_value
        .trim()
        .trim_matches(|character| matches!(character, '"' | '\'' | '`'))
        .to_ascii_lowercase();
    (normalized.contains("://") && normalized.contains('@'))
        || normalized.contains("password=")
        || normalized.contains("passwd=")
        || normalized.contains("token=")
        || normalized.contains("secret=")
        || placeholder_credential_default(&normalized)
}

fn placeholder_credential_default(default_value: &str) -> bool {
    let tokens = default_value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let compact = tokens.join("");
    if matches!(
        compact.as_str(),
        "changeme"
            | "changeit"
            | "replaceme"
            | "replaceit"
            | "replacewithsecret"
            | "replacewithtoken"
            | "replacewithapikey"
            | "yourpassword"
            | "yoursecret"
            | "yourtoken"
            | "yourapikey"
            | "examplesecret"
            | "exampletoken"
            | "exampleapikey"
            | "dummysecret"
            | "dummytoken"
            | "dummyapikey"
            | "todosecret"
            | "todotoken"
            | "fixmesecret"
            | "fixmetoken"
    ) {
        return true;
    }

    let has_placeholder = tokens.iter().any(|token| {
        matches!(
            *token,
            "changeme"
                | "changeit"
                | "replace"
                | "replaceit"
                | "replaceme"
                | "todo"
                | "fixme"
                | "example"
                | "sample"
                | "dummy"
                | "placeholder"
                | "your"
        )
    });
    let has_credential = tokens.iter().any(|token| {
        matches!(
            *token,
            "password"
                | "passwd"
                | "passphrase"
                | "secret"
                | "token"
                | "credential"
                | "credentials"
                | "apikey"
                | "jwt"
        ) || *token == "key"
    });
    has_placeholder && has_credential
}

fn add_undeclared_import_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let declared = declared_package_ids(graph);
    let declared_ecosystems: BTreeSet<_> = declared
        .iter()
        .filter_map(|package_id| {
            package_id
                .split_once(':')
                .map(|(ecosystem, _)| ecosystem.to_string())
        })
        .collect();

    if declared_ecosystems.is_empty() {
        return;
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::Imports {
            continue;
        }

        let Some(source_node) = graph.nodes.iter().find(|node| node.id == edge.source) else {
            continue;
        };
        if is_dependency_manifest_source_path(&source_node.label) {
            continue;
        }
        let Some(import_node) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        if import_node
            .metadata
            .get("import_scope")
            .is_some_and(|scope| scope == "local")
        {
            continue;
        }
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        if matches!(language, "c" | "cpp") {
            continue;
        }
        let imports = import_package_candidates(language, &import_node.label, &declared_ecosystems);
        if imports.is_empty() {
            continue;
        }
        let Some(import) = imports
            .iter()
            .find(|import| declared_ecosystems.contains(import.ecosystem.as_str()))
        else {
            continue;
        };
        if imports
            .iter()
            .any(|import| is_declared_package(&declared, &import.ecosystem, &import.package))
        {
            continue;
        }

        let source = source_node.label.as_str();
        insights.push(Insight {
            kind: "undeclared_external_import".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{source}` imports `{}` but no matching {} dependency was found",
                import.package, import.ecosystem
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

fn add_unused_dependency_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let used_packages = dependency_usage_packages(graph);
    let used_ecosystems: BTreeSet<_> = used_packages
        .iter()
        .map(|(_, import)| import.ecosystem.as_str())
        .collect();
    if used_ecosystems.is_empty() {
        return;
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn
            || edge
                .metadata
                .get("dependency_kind")
                .is_none_or(|kind| kind != "runtime")
        {
            continue;
        }

        let Some(dependency) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        let Some(package_id) = dependency.metadata.get("package_id") else {
            continue;
        };
        let Some((ecosystem, _)) = package_id.split_once(':') else {
            continue;
        };
        if !used_ecosystems.contains(ecosystem) {
            continue;
        }
        if used_packages
            .iter()
            .any(|(_, import)| import_matches_package_id(package_id, import))
        {
            continue;
        }

        let source = graph
            .nodes
            .iter()
            .find(|node| node.id == edge.source)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        insights.push(Insight {
            kind: "unused_declared_dependency".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "`{source}` declares `{}` but no matching import was found",
                dependency.label
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

fn add_conflicting_dependency_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<NodeId, Vec<(usize, String)>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn {
            continue;
        }
        if edge
            .metadata
            .get("dependency_version_kind")
            .is_some_and(|kind| kind == "locked")
        {
            continue;
        }
        let Some(version) = edge
            .metadata
            .get("dependency_version")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        groups
            .entry(edge.target)
            .or_default()
            .push((index, version.to_string()));
    }

    for (target, declarations) in groups {
        let distinct_versions: BTreeSet<_> = declarations
            .iter()
            .map(|(_, version)| version.as_str())
            .collect();
        if distinct_versions.len() < 2 {
            continue;
        }

        let mut nodes = BTreeSet::from([target]);
        let edge_indexes: Vec<_> = declarations
            .iter()
            .map(|(index, _)| {
                if let Some(edge) = graph.edges.get(*index) {
                    nodes.insert(edge.source);
                }
                *index
            })
            .collect();
        let versions = distinct_versions
            .iter()
            .take(4)
            .map(|version| format!("`{version}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let package = node_label(graph, target).unwrap_or("unknown");
        insights.push(Insight {
            kind: "conflicting_dependency_declaration".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Dependency `{package}` is declared with multiple constraints: {versions}"
            ),
            nodes: nodes.into_iter().collect(),
            edges: edge_indexes,
        });
    }
}

fn add_mixed_dependency_scope_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<String, Vec<(usize, NodeId, NodeId, String)>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn {
            continue;
        }
        let Some(scope) = edge
            .metadata
            .get("dependency_kind")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(target) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        let key = target
            .metadata
            .get("package_id")
            .cloned()
            .unwrap_or_else(|| format!("node:{}", target.id.0));
        groups
            .entry(key)
            .or_default()
            .push((index, edge.source, edge.target, scope.to_string()));
    }

    for declarations in groups.into_values() {
        let scopes: BTreeSet<_> = declarations
            .iter()
            .map(|(_, _, _, scope)| scope.as_str())
            .collect();
        if scopes.len() < 2 {
            continue;
        }

        let mut nodes = BTreeSet::new();
        let mut edges = Vec::new();
        for (index, source, target, _) in &declarations {
            nodes.insert(*source);
            nodes.insert(*target);
            edges.push(*index);
        }
        let Some(package) = declarations
            .first()
            .and_then(|(_, _, target, _)| node_label(graph, *target))
        else {
            continue;
        };
        let scope_list = format_backtick_list(scopes.iter().copied(), 6);
        insights.push(Insight {
            kind: "mixed_dependency_scope".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Dependency `{package}` is declared in multiple dependency scopes: {scope_list}"
            ),
            nodes: nodes.into_iter().collect(),
            edges,
        });
    }
}

fn add_non_runtime_dependency_import_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let declarations = dependency_declarations_by_package(graph);
    if declarations.is_empty() {
        return;
    }
    let declared_ecosystems = declared_ecosystems_from_package_ids(declarations.keys());

    let path_index = node_path_index(graph);
    let mut reported = BTreeSet::new();
    for (import_edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::Imports {
            continue;
        }
        let Some(source) = graph.nodes.iter().find(|node| node.id == edge.source) else {
            continue;
        };
        if is_dependency_manifest_source_path(&source.label) {
            continue;
        }
        if node_path_matches(source, &path_index, "test")
            || path_index
                .get(&source.id)
                .is_some_and(|path| is_test_like_source_path(path))
            || is_test_like_source_path(&source.label)
        {
            continue;
        }

        let Some(import_node) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        if import_node
            .metadata
            .get("import_scope")
            .is_some_and(|scope| scope == "local")
        {
            continue;
        }
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        let imports = import_package_candidates(language, &import_node.label, &declared_ecosystems);
        let Some((package_id, package_declarations)) = imports.iter().find_map(|import| {
            declarations
                .iter()
                .find(|(package_id, _)| import_matches_package_id(package_id, import))
        }) else {
            continue;
        };
        let scopes: BTreeSet<_> = package_declarations
            .iter()
            .map(|declaration| declaration.kind.as_str())
            .collect();
        if scopes.contains("runtime") {
            continue;
        }
        if !reported.insert((edge.source, package_id.clone())) {
            continue;
        }

        let mut nodes = BTreeSet::from([edge.source, edge.target]);
        let mut edges = vec![import_edge_index];
        for declaration in package_declarations {
            nodes.insert(declaration.source);
            nodes.insert(declaration.target);
            edges.push(declaration.edge_index);
        }
        let scope_list = format_backtick_list(scopes.iter().copied(), 6);
        let source_label = node_label(graph, edge.source).unwrap_or("unknown");
        let package = package_declarations
            .first()
            .and_then(|declaration| node_label(graph, declaration.target))
            .unwrap_or(package_id.as_str());
        insights.push(Insight {
            kind: "non_runtime_dependency_import".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{source_label}` imports `{package}` from production-like code, but the package is declared only as {scope_list}"
            ),
            nodes: nodes.into_iter().collect(),
            edges,
        });
    }
}

fn add_test_only_runtime_dependency_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let declarations = dependency_declarations_by_package(graph);
    if declarations.is_empty() {
        return;
    }
    let usages = dependency_import_usages_by_package(graph);
    if usages.is_empty() {
        return;
    }

    for (package_id, package_declarations) in declarations {
        let runtime_declarations = package_declarations
            .iter()
            .filter(|declaration| declaration.kind == "runtime")
            .collect::<Vec<_>>();
        if runtime_declarations.is_empty() {
            continue;
        }
        let Some(package_usages) = usages.get(&package_id).filter(|usages| !usages.is_empty())
        else {
            continue;
        };
        if package_usages.iter().any(|usage| !usage.test_like) {
            continue;
        }

        let mut nodes = BTreeSet::new();
        let mut edges = Vec::new();
        for declaration in runtime_declarations {
            nodes.insert(declaration.source);
            nodes.insert(declaration.target);
            edges.push(declaration.edge_index);
        }
        for usage in package_usages {
            nodes.insert(usage.source);
            nodes.insert(usage.target);
            edges.push(usage.edge_index);
        }
        let package = package_declarations
            .first()
            .and_then(|declaration| node_label(graph, declaration.target))
            .unwrap_or(package_id.as_str());
        insights.push(Insight {
            kind: "test_only_runtime_dependency".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "Dependency `{package}` is declared as runtime but is only imported from test-like sources"
            ),
            nodes: nodes.into_iter().collect(),
            edges,
        });
    }
}

#[derive(Debug)]
struct DependencyDeclaration {
    edge_index: usize,
    source: NodeId,
    target: NodeId,
    kind: String,
}

fn dependency_declarations_by_package(
    graph: &CodeGraph,
) -> BTreeMap<String, Vec<DependencyDeclaration>> {
    let mut declarations: BTreeMap<String, Vec<DependencyDeclaration>> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn {
            continue;
        }
        let Some(kind) = edge
            .metadata
            .get("dependency_kind")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(target) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        let Some(package_id) = target.metadata.get("package_id") else {
            continue;
        };
        declarations
            .entry(package_id.clone())
            .or_default()
            .push(DependencyDeclaration {
                edge_index,
                source: edge.source,
                target: edge.target,
                kind: kind.to_string(),
            });
    }
    declarations
}

#[derive(Debug)]
struct DependencyImportUsage {
    edge_index: usize,
    source: NodeId,
    target: NodeId,
    test_like: bool,
}

fn dependency_import_usages_by_package(
    graph: &CodeGraph,
) -> BTreeMap<String, Vec<DependencyImportUsage>> {
    let path_index = node_path_index(graph);
    let declared = declared_package_ids(graph);
    let declared_ecosystems = declared_ecosystems_from_package_ids(declared.iter());
    let mut usages: BTreeMap<String, Vec<DependencyImportUsage>> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::Imports {
            continue;
        }
        let Some(source) = graph.nodes.iter().find(|node| node.id == edge.source) else {
            continue;
        };
        if is_dependency_manifest_source_path(&source.label) {
            continue;
        }
        let Some(import_node) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        if import_node
            .metadata
            .get("import_scope")
            .is_some_and(|scope| scope == "local")
        {
            continue;
        }
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        let imports = import_package_candidates(language, &import_node.label, &declared_ecosystems);
        let Some(package_id) = imports.iter().find_map(|import| {
            declared
                .iter()
                .find(|package_id| import_matches_package_id(package_id, import))
                .cloned()
        }) else {
            continue;
        };
        let test_like = path_index
            .get(&source.id)
            .is_some_and(|path| is_test_like_source_path(path))
            || is_test_like_source_path(&source.label);
        usages
            .entry(package_id)
            .or_default()
            .push(DependencyImportUsage {
                edge_index,
                source: edge.source,
                target: edge.target,
                test_like,
            });
    }
    usages
}

fn add_duplicate_framework_route_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<(String, String), Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "framework_route")
        {
            continue;
        }
        let Some(path) = node
            .metadata
            .get("path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let method = node
            .metadata
            .get("method")
            .map(|method| method.trim())
            .filter(|method| !method.is_empty())
            .unwrap_or("ROUTE")
            .to_ascii_uppercase();
        groups
            .entry((method, path.to_string()))
            .or_default()
            .push(node.id);
    }

    for ((method, path), nodes) in groups {
        if nodes.len() < 2 {
            continue;
        }

        let handlers = nodes
            .iter()
            .filter_map(|id| graph.nodes.iter().find(|node| node.id == *id))
            .filter_map(|node| node.metadata.get("handler").map(String::as_str))
            .collect::<BTreeSet<_>>();
        let handler_text = if handlers.is_empty() {
            "multiple handlers".to_string()
        } else {
            handlers
                .iter()
                .take(5)
                .map(|handler| format!("`{handler}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let edge_indexes = nodes
            .iter()
            .flat_map(|node| outgoing_edge_indexes(graph, *node, EdgeKind::References))
            .collect();

        insights.push(Insight {
            kind: "duplicate_framework_route".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Route `{method} {path}` is declared {} times ({handler_text})",
                nodes.len()
            ),
            nodes,
            edges: edge_indexes,
        });
    }
}

fn add_unresolved_framework_route_handler_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "framework_route")
        {
            continue;
        }
        let Some(handler) = node
            .metadata
            .get("handler")
            .map(|handler| handler.trim())
            .filter(|handler| !handler.is_empty())
        else {
            continue;
        };

        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|resolution| resolution == "framework_route_handler")
        });
        if resolved {
            continue;
        }

        let method = node
            .metadata
            .get("method")
            .map(|method| method.trim())
            .filter(|method| !method.is_empty())
            .unwrap_or("ROUTE");
        let path = node
            .metadata
            .get("path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
            .unwrap_or(&node.label);
        let framework = node
            .metadata
            .get("framework")
            .map(|framework| framework.trim())
            .filter(|framework| !framework.is_empty())
            .unwrap_or("framework");
        let mut edges = incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint);
        edges.extend(outgoing_edge_indexes(graph, node.id, EdgeKind::References));
        edges.sort_unstable();
        edges.dedup();

        insights.push(Insight {
            kind: "unresolved_framework_route_handler".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{framework} route `{method} {path}` references handler `{handler}` but no matching function was found"
            ),
            nodes: vec![node.id],
            edges,
        });
    }
}

fn add_custom_rule_violation_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "custom_rule_violation")
        {
            continue;
        }

        let rule_kind = node
            .metadata
            .get("rule_kind")
            .map(String::as_str)
            .unwrap_or("violation");
        let message = node
            .metadata
            .get("message")
            .cloned()
            .unwrap_or_else(|| node.label.clone());
        let severity = node
            .metadata
            .get("severity")
            .map(|value| insight_severity_from_str(value))
            .unwrap_or(InsightSeverity::Warning);
        let mut edges = outgoing_edge_indexes(graph, node.id, EdgeKind::References);
        if let Some(edge_index) = node
            .metadata
            .get("violated_edge_index")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| *index < graph.edges.len())
        {
            edges.push(edge_index);
            edges.sort_unstable();
            edges.dedup();
        }

        insights.push(Insight {
            kind: format!("custom_rule_{rule_kind}"),
            severity,
            message,
            nodes: vec![node.id],
            edges,
        });
    }
}

fn insight_severity_from_str(value: &str) -> InsightSeverity {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => InsightSeverity::Error,
        "info" => InsightSeverity::Info,
        _ => InsightSeverity::Warning,
    }
}

fn add_dependency_cycle_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    const MAX_CYCLE_INSIGHTS: usize = 50;

    let mut nodes = BTreeSet::new();
    let mut adjacency: BTreeMap<NodeId, Vec<(NodeId, usize)>> = BTreeMap::new();
    let mut reverse: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if !is_cycle_edge(&edge.kind) {
            continue;
        }
        nodes.insert(edge.source);
        nodes.insert(edge.target);
        adjacency
            .entry(edge.source)
            .or_default()
            .push((edge.target, index));
        reverse.entry(edge.target).or_default().push(edge.source);
    }

    let mut visited = BTreeSet::new();
    let mut order = Vec::new();
    for node in &nodes {
        if visited.contains(node) {
            continue;
        }
        fill_finish_order(*node, &adjacency, &mut visited, &mut order);
    }

    let mut assigned = BTreeSet::new();
    for node in order.into_iter().rev() {
        if assigned.contains(&node) {
            continue;
        }
        let component = reverse_component(node, &reverse, &mut assigned);
        let component_nodes: BTreeSet<_> = component.iter().copied().collect();
        let component_edges: Vec<_> = graph
            .edges
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| {
                if is_cycle_edge(&edge.kind)
                    && component_nodes.contains(&edge.source)
                    && component_nodes.contains(&edge.target)
                {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();

        if component.len() < 2 {
            continue;
        }

        let labels = component
            .iter()
            .filter_map(|id| node_label(graph, *id))
            .take(5)
            .map(|label| format!("`{label}`"))
            .collect::<Vec<_>>()
            .join(" -> ");
        let suffix = if component.len() > 5 { " -> ..." } else { "" };
        insights.push(Insight {
            kind: "dependency_cycle".to_string(),
            severity: InsightSeverity::Warning,
            message: format!("Directed dependency cycle involving {labels}{suffix}"),
            nodes: component,
            edges: component_edges,
        });

        if insights
            .iter()
            .filter(|insight| insight.kind == "dependency_cycle")
            .count()
            >= MAX_CYCLE_INSIGHTS
        {
            return;
        }
    }
}

fn fill_finish_order(
    start: NodeId,
    adjacency: &BTreeMap<NodeId, Vec<(NodeId, usize)>>,
    visited: &mut BTreeSet<NodeId>,
    order: &mut Vec<NodeId>,
) {
    let mut stack = vec![(start, false)];
    while let Some((node, finished)) = stack.pop() {
        if finished {
            order.push(node);
            continue;
        }
        if !visited.insert(node) {
            continue;
        }
        stack.push((node, true));
        if let Some(edges) = adjacency.get(&node) {
            for (target, _) in edges.iter().rev() {
                if !visited.contains(target) {
                    stack.push((*target, false));
                }
            }
        }
    }
}

fn reverse_component(
    start: NodeId,
    reverse: &BTreeMap<NodeId, Vec<NodeId>>,
    assigned: &mut BTreeSet<NodeId>,
) -> Vec<NodeId> {
    let mut component = Vec::new();
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
        if !assigned.insert(node) {
            continue;
        }
        component.push(node);
        if let Some(sources) = reverse.get(&node) {
            for source in sources.iter().rev() {
                if !assigned.contains(source) {
                    stack.push(*source);
                }
            }
        }
    }
    component.sort();
    component
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportPackage {
    ecosystem: String,
    package: String,
}

fn declared_package_ids(graph: &CodeGraph) -> BTreeSet<String> {
    graph
        .nodes
        .iter()
        .filter_map(|node| {
            if node
                .metadata
                .get("item_kind")
                .is_some_and(|value| value == "dependency")
            {
                node.metadata.get("package_id").cloned()
            } else {
                None
            }
        })
        .collect()
}

fn import_packages(graph: &CodeGraph) -> Vec<(usize, ImportPackage)> {
    let declared = declared_package_ids(graph);
    let declared_ecosystems = declared_ecosystems_from_package_ids(declared.iter());
    graph
        .edges
        .iter()
        .enumerate()
        .flat_map(|(index, edge)| {
            if edge.kind != EdgeKind::Imports {
                return Vec::new();
            }
            if graph
                .nodes
                .iter()
                .find(|node| node.id == edge.source)
                .is_some_and(|node| is_dependency_manifest_source_path(&node.label))
            {
                return Vec::new();
            }
            let Some(import_node) = graph.nodes.iter().find(|node| node.id == edge.target) else {
                return Vec::new();
            };
            let Some(language) = import_node.metadata.get("language") else {
                return Vec::new();
            };
            import_package_candidates(language, &import_node.label, &declared_ecosystems)
                .into_iter()
                .map(move |import| (index, import))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn dependency_usage_packages(graph: &CodeGraph) -> Vec<(usize, ImportPackage)> {
    let mut packages = import_packages(graph);
    for (index, node) in graph.nodes.iter().enumerate() {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "call")
            || node
                .metadata
                .get("language")
                .is_none_or(|language| language != "rust")
        {
            continue;
        }
        if let Some(package) = rust_path_package(&node.label) {
            packages.push((
                index,
                ImportPackage {
                    ecosystem: "cargo".to_string(),
                    package,
                },
            ));
        }
    }
    packages
}

fn import_package_candidate(language: &str, label: &str) -> Option<ImportPackage> {
    match language {
        "rust" => rust_import_package(label).map(|package| ImportPackage {
            ecosystem: "cargo".to_string(),
            package,
        }),
        "python" => python_import_package(label).map(|package| ImportPackage {
            ecosystem: "python".to_string(),
            package,
        }),
        "javascript" | "typescript" | "tsx" => {
            js_import_package(label).map(|package| ImportPackage {
                ecosystem: "npm".to_string(),
                package,
            })
        }
        "go" => go_import_package(label).map(|package| ImportPackage {
            ecosystem: "go".to_string(),
            package,
        }),
        "dart" => dart_import_package(label).map(|package| ImportPackage {
            ecosystem: "dart".to_string(),
            package,
        }),
        "php" => php_import_packages(label)
            .into_iter()
            .next()
            .map(|package| ImportPackage {
                ecosystem: "composer".to_string(),
                package,
            }),
        _ => None,
    }
}

fn import_package_candidates(
    language: &str,
    label: &str,
    declared_ecosystems: &BTreeSet<String>,
) -> Vec<ImportPackage> {
    if matches!(language, "c" | "cpp") {
        let Some(package) = c_family_include_package(label) else {
            return Vec::new();
        };
        return ["vcpkg", "conan", "cmake"]
            .into_iter()
            .filter(|ecosystem| declared_ecosystems.contains(*ecosystem))
            .map(|ecosystem| ImportPackage {
                ecosystem: ecosystem.to_string(),
                package: package.clone(),
            })
            .collect();
    }

    if language == "php" {
        if !declared_ecosystems.contains("composer") {
            return Vec::new();
        }
        return php_import_packages(label)
            .into_iter()
            .map(|package| ImportPackage {
                ecosystem: "composer".to_string(),
                package,
            })
            .collect();
    }

    import_package_candidate(language, label)
        .into_iter()
        .collect()
}

fn import_matches_package_id(package_id: &str, import: &ImportPackage) -> bool {
    let Some((ecosystem, package)) = package_id.split_once(':') else {
        return false;
    };
    if ecosystem != import.ecosystem {
        return false;
    }

    match ecosystem {
        "go" => import.package == package || import.package.starts_with(&format!("{package}/")),
        "cargo" => {
            let canonical = import.package.to_ascii_lowercase();
            let hyphenated = canonical.replace('_', "-");
            let underscored = canonical.replace('-', "_");
            package == canonical || package == hyphenated || package == underscored
        }
        "python" => package == canonical_python_package_name(&import.package),
        "npm" | "composer" | "dart" => package == import.package.to_ascii_lowercase(),
        "vcpkg" | "conan" | "cmake" => package == import.package.to_ascii_lowercase(),
        _ => package == import.package,
    }
}

fn rust_import_package(label: &str) -> Option<String> {
    let value = label.trim().strip_prefix("use ")?;
    let first = value
        .trim()
        .trim_start_matches("::")
        .split([':', ';', ',', '{', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())?;
    if matches!(first, "std" | "core" | "alloc" | "crate" | "self" | "super") {
        None
    } else {
        Some(first.to_ascii_lowercase())
    }
}

fn rust_path_package(label: &str) -> Option<String> {
    let first = label
        .trim()
        .trim_start_matches("::")
        .split("::")
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())?;
    if first.contains('.') || matches!(first, "std" | "core" | "alloc" | "crate" | "self" | "super")
    {
        None
    } else {
        Some(first.to_ascii_lowercase())
    }
}

fn python_import_package(label: &str) -> Option<String> {
    let value = label.trim();
    let package = if let Some(rest) = value.strip_prefix("import ") {
        rest.split([',', ' ', '\n', '\t'])
            .find(|part| !part.is_empty())
            .and_then(|part| part.split('.').next())
    } else if let Some(rest) = value.strip_prefix("from ") {
        rest.split_whitespace()
            .next()
            .and_then(|part| part.split('.').next())
    } else {
        None
    }?;

    let package = canonical_python_package_name(package);
    if is_python_stdlib_package(&package) || package.is_empty() {
        None
    } else {
        Some(package)
    }
}

fn js_import_package(label: &str) -> Option<String> {
    let module = first_quoted_string(label)?;
    if module.starts_with('.')
        || module.starts_with('/')
        || module.starts_with("node:")
        || is_node_builtin_module(&module)
    {
        return None;
    }

    if module.starts_with('@') {
        let mut parts = module.split('/');
        let scope = parts.next()?;
        let name = parts.next()?;
        Some(format!("{scope}/{name}").to_ascii_lowercase())
    } else {
        module
            .split('/')
            .next()
            .filter(|part| !part.is_empty())
            .map(|package| package.to_ascii_lowercase())
    }
}

fn go_import_package(label: &str) -> Option<String> {
    for module in quoted_strings(label) {
        if module.starts_with('.') || module.starts_with('/') {
            continue;
        }
        let first = module.split('/').next().unwrap_or("");
        if first.contains('.') {
            return Some(module);
        }
    }
    None
}

fn dart_import_package(label: &str) -> Option<String> {
    let uri = first_quoted_string(label)?;
    if uri.starts_with('.')
        || uri.starts_with('/')
        || uri.starts_with("dart:")
        || uri.contains("://")
    {
        return None;
    }
    let rest = uri.strip_prefix("package:")?;
    rest.split('/')
        .next()
        .map(str::trim)
        .filter(|package| !package.is_empty())
        .map(|package| package.to_ascii_lowercase())
}

fn php_import_packages(label: &str) -> Vec<String> {
    let mut packages = Vec::new();
    for namespace in php_import_namespaces(label) {
        for package in php_namespace_package_candidates(&namespace) {
            if !packages.contains(&package) {
                packages.push(package);
            }
        }
    }
    packages
}

fn php_import_namespaces(label: &str) -> Vec<String> {
    let mut value = label.trim().trim_end_matches(';').trim();
    if let Some(rest) = value.strip_prefix("use ") {
        value = rest.trim();
    }
    value = value
        .strip_prefix("function ")
        .or_else(|| value.strip_prefix("const "))
        .unwrap_or(value)
        .trim();

    if let Some((prefix, rest)) = value.split_once('{') {
        let prefix = prefix.trim().trim_end_matches('\\');
        let Some((group, _)) = rest.split_once('}') else {
            return Vec::new();
        };
        return group
            .split(',')
            .filter_map(|part| {
                let clause = php_namespace_without_alias(part);
                if clause.is_empty() {
                    None
                } else if prefix.is_empty() {
                    Some(clause.to_string())
                } else {
                    Some(format!("{prefix}\\{clause}"))
                }
            })
            .collect();
    }

    let namespace = php_namespace_without_alias(value);
    if namespace.is_empty() {
        Vec::new()
    } else {
        vec![namespace.to_string()]
    }
}

fn php_namespace_without_alias(value: &str) -> &str {
    value
        .split_once(" as ")
        .map(|(namespace, _)| namespace)
        .unwrap_or(value)
        .trim()
        .trim_start_matches('\\')
}

fn php_namespace_package_candidates(namespace: &str) -> Vec<String> {
    let parts = namespace
        .split('\\')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 || is_php_non_composer_namespace_root(parts[0]) {
        return Vec::new();
    }

    let vendor = composer_package_part(parts[0]);
    let mut packages = Vec::new();
    match parts.as_slice() {
        ["Monolog", ..] => packages.push("monolog/monolog".to_string()),
        ["PHPUnit", ..] => packages.push("phpunit/phpunit".to_string()),
        ["GuzzleHttp", ..] => packages.push("guzzlehttp/guzzle".to_string()),
        ["Symfony", "Component", component, ..] => {
            packages.push(format!("symfony/{}", composer_package_part(component)));
        }
        ["Psr", component, ..] => {
            packages.push(format!("psr/{}", composer_package_part(component)));
        }
        _ => {}
    }

    if let Some(component) = parts.get(1) {
        packages.push(format!("{vendor}/{}", composer_package_part(component)));
    }
    packages.push(format!("{vendor}/{vendor}"));
    packages.retain(|package| package.split('/').all(|part| !part.is_empty()));
    packages.dedup();
    packages
}

fn is_php_non_composer_namespace_root(root: &str) -> bool {
    matches!(
        root,
        "App"
            | "Tests"
            | "Test"
            | "Database"
            | "Config"
            | "DateTime"
            | "DateTimeImmutable"
            | "DateTimeInterface"
            | "DateInterval"
            | "DateTimeZone"
            | "Exception"
            | "RuntimeException"
            | "InvalidArgumentException"
            | "Throwable"
            | "Closure"
            | "ArrayObject"
            | "Iterator"
            | "IteratorAggregate"
            | "Traversable"
            | "Countable"
            | "JsonSerializable"
            | "PDO"
    )
}

fn composer_package_part(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_separator = false;
    for character in value.trim().chars() {
        if matches!(character, '_' | '-' | '.') {
            if !previous_separator && !normalized.is_empty() {
                normalized.push('-');
                previous_separator = true;
            }
            continue;
        }
        if character.is_ascii_uppercase() {
            if !normalized.is_empty() && !previous_separator {
                normalized.push('-');
            }
            normalized.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_separator = false;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn c_family_include_package(label: &str) -> Option<String> {
    let header = include_header_name(label)?;
    let package = header
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches(".hpp")
        .trim_end_matches(".hh")
        .trim_end_matches(".hxx")
        .trim_end_matches(".h")
        .to_ascii_lowercase();
    if package.is_empty()
        || matches!(
            package.as_str(),
            "assert"
                | "complex"
                | "ctype"
                | "errno"
                | "float"
                | "inttypes"
                | "iso646"
                | "limits"
                | "locale"
                | "math"
                | "setjmp"
                | "signal"
                | "stdalign"
                | "stdarg"
                | "stdatomic"
                | "stdbool"
                | "stddef"
                | "stdint"
                | "stdio"
                | "stdlib"
                | "stdnoreturn"
                | "string"
                | "tgmath"
                | "threads"
                | "time"
                | "uchar"
                | "wchar"
                | "wctype"
                | "algorithm"
                | "array"
                | "atomic"
                | "bit"
                | "chrono"
                | "concepts"
                | "coroutine"
                | "deque"
                | "exception"
                | "filesystem"
                | "format"
                | "fstream"
                | "functional"
                | "future"
                | "initializer_list"
                | "iostream"
                | "istream"
                | "iterator"
                | "map"
                | "memory"
                | "mutex"
                | "optional"
                | "ostream"
                | "queue"
                | "ranges"
                | "regex"
                | "set"
                | "span"
                | "sstream"
                | "stdexcept"
                | "string_view"
                | "thread"
                | "tuple"
                | "type_traits"
                | "unordered_map"
                | "unordered_set"
                | "utility"
                | "variant"
                | "vector"
        )
    {
        None
    } else {
        Some(package)
    }
}

fn include_header_name(label: &str) -> Option<String> {
    let value = label.trim();
    if let Some(start) = value.find('<') {
        let rest = &value[start + 1..];
        let end = rest.find('>')?;
        return Some(rest[..end].trim().to_string());
    }
    quoted_strings(value).into_iter().next()
}

fn declared_ecosystems_from_package_ids<'a>(
    package_ids: impl IntoIterator<Item = &'a String>,
) -> BTreeSet<String> {
    package_ids
        .into_iter()
        .filter_map(|package_id| {
            package_id
                .split_once(':')
                .map(|(ecosystem, _)| ecosystem.to_string())
        })
        .collect()
}

fn is_declared_package(declared: &BTreeSet<String>, ecosystem: &str, package: &str) -> bool {
    match ecosystem {
        "go" => declared.iter().any(|package_id| {
            package_id.strip_prefix("go:").is_some_and(|module| {
                package == module || package.starts_with(&format!("{module}/"))
            })
        }),
        "cargo" => {
            let canonical = package.to_ascii_lowercase();
            let hyphenated = canonical.replace('_', "-");
            let underscored = canonical.replace('-', "_");
            declared.contains(&format!("cargo:{canonical}"))
                || declared.contains(&format!("cargo:{hyphenated}"))
                || declared.contains(&format!("cargo:{underscored}"))
        }
        "python" => declared.contains(&format!(
            "python:{}",
            canonical_python_package_name(package)
        )),
        "npm" => declared.contains(&format!("npm:{}", package.to_ascii_lowercase())),
        "composer" => declared.contains(&format!("composer:{}", package.to_ascii_lowercase())),
        "vcpkg" | "conan" | "cmake" => {
            declared.contains(&format!("{ecosystem}:{}", package.to_ascii_lowercase()))
        }
        _ => declared.contains(&format!("{ecosystem}:{package}")),
    }
}

fn canonical_python_package_name(name: &str) -> String {
    let mut normalized = String::new();
    let mut previous_separator = false;
    for character in name.trim().chars() {
        if matches!(character, '-' | '_' | '.') {
            if !previous_separator {
                normalized.push('-');
            }
            previous_separator = true;
        } else {
            normalized.extend(character.to_lowercase());
            previous_separator = false;
        }
    }
    normalized
}

fn first_quoted_string(value: &str) -> Option<String> {
    quoted_strings(value).into_iter().next()
}

fn quoted_strings(value: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut quote = None;
    let mut start = 0;

    for (index, character) in value.char_indices() {
        match quote {
            Some(current_quote) if character == current_quote => {
                strings.push(value[start..index].to_string());
                quote = None;
            }
            None if character == '"' || character == '\'' || character == '`' => {
                quote = Some(character);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    strings
}

fn is_node_builtin_module(module: &str) -> bool {
    matches!(
        module,
        "assert"
            | "buffer"
            | "child_process"
            | "cluster"
            | "crypto"
            | "dgram"
            | "dns"
            | "events"
            | "fs"
            | "http"
            | "https"
            | "module"
            | "net"
            | "os"
            | "path"
            | "process"
            | "querystring"
            | "readline"
            | "stream"
            | "string_decoder"
            | "timers"
            | "tls"
            | "tty"
            | "url"
            | "util"
            | "vm"
            | "zlib"
    )
}

fn is_python_stdlib_package(package: &str) -> bool {
    matches!(
        package,
        "abc"
            | "argparse"
            | "asyncio"
            | "base64"
            | "collections"
            | "contextlib"
            | "csv"
            | "dataclasses"
            | "datetime"
            | "functools"
            | "glob"
            | "hashlib"
            | "http"
            | "importlib"
            | "inspect"
            | "io"
            | "itertools"
            | "json"
            | "logging"
            | "math"
            | "os"
            | "pathlib"
            | "pickle"
            | "random"
            | "re"
            | "shutil"
            | "sqlite3"
            | "statistics"
            | "string"
            | "subprocess"
            | "sys"
            | "tempfile"
            | "threading"
            | "time"
            | "typing"
            | "unittest"
            | "urllib"
            | "uuid"
            | "venv"
            | "warnings"
            | "xml"
    )
}

fn incoming_edge_indexes(graph: &CodeGraph, target: NodeId, kind: EdgeKind) -> Vec<usize> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            if edge.target == target && edge.kind == kind {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

fn outgoing_edge_indexes(graph: &CodeGraph, source: NodeId, kind: EdgeKind) -> Vec<usize> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            if edge.source == source && edge.kind == kind {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

fn is_cycle_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls | EdgeKind::References | EdgeKind::Imports | EdgeKind::DependsOn
    )
}

fn is_trace_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls
            | EdgeKind::References
            | EdgeKind::Imports
            | EdgeKind::ReadsConfig
            | EdgeKind::ReadsEnvironment
            | EdgeKind::MayError
            | EdgeKind::DependsOn
    )
}

fn normalize_workflow_filters(filters: WorkflowFilters) -> WorkflowFilters {
    WorkflowFilters {
        edge_kind: normalize_workflow_filter(filters.edge_kind),
        confidence: normalize_workflow_filter(filters.confidence),
        language: normalize_workflow_filter(filters.language),
        risk_severity: normalize_workflow_filter(filters.risk_severity),
        block_kind: normalize_workflow_filter(filters.block_kind),
    }
}

fn normalize_workflow_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn workflow_included_node_ids(
    start: &Node,
    blocks: &[WorkflowBlock],
    transitions: &[WorkflowTransition],
    filters: &WorkflowFilters,
) -> BTreeSet<NodeId> {
    let has_block_filters = filters.language.is_some()
        || filters.risk_severity.is_some()
        || filters.block_kind.is_some();
    if !has_block_filters {
        return blocks.iter().map(|block| block.node.id).collect();
    }

    let mut included = BTreeSet::from([start.id]);
    for block in blocks {
        if workflow_block_filter_matches(block, filters) {
            included.insert(block.node.id);
        }
    }
    if filters.risk_severity.is_some() {
        let block_by_node = blocks
            .iter()
            .map(|block| (block.node.id, block))
            .collect::<BTreeMap<_, _>>();
        for transition in transitions {
            if !workflow_transition_filter_matches(transition, filters) {
                continue;
            }
            let source_matches = block_by_node
                .get(&transition.source_node_id)
                .is_some_and(|block| workflow_block_non_risk_filters_match(block, filters));
            let target_matches = block_by_node
                .get(&transition.target_node_id)
                .is_some_and(|block| workflow_block_non_risk_filters_match(block, filters));
            if source_matches && target_matches {
                included.insert(transition.source_node_id);
                included.insert(transition.target_node_id);
            }
        }
    }
    included
}

fn workflow_edge_filter_matches(edge: &Edge, filters: &WorkflowFilters) -> bool {
    filters
        .edge_kind
        .as_deref()
        .is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
        && filters
            .confidence
            .as_deref()
            .is_none_or(|expected| text_matches(&confidence_name(edge.confidence), expected))
}

fn workflow_block_filter_matches(block: &WorkflowBlock, filters: &WorkflowFilters) -> bool {
    workflow_block_non_risk_filters_match(block, filters)
        && filters.risk_severity.as_deref().is_none_or(|expected| {
            block
                .risk_refs
                .iter()
                .any(|risk| text_matches(severity_name(risk.severity), expected))
        })
}

fn workflow_block_non_risk_filters_match(block: &WorkflowBlock, filters: &WorkflowFilters) -> bool {
    filters
        .language
        .as_deref()
        .is_none_or(|expected| workflow_node_language_matches(&block.node, expected))
        && filters.block_kind.as_deref().is_none_or(|expected| {
            text_matches(&workflow_block_kind_filter_name(&block.kind), expected)
                || text_matches(workflow_block_kind_label(&block.kind), expected)
        })
}

fn workflow_node_language_matches(node: &Node, expected: &str) -> bool {
    node.metadata
        .get("language")
        .is_some_and(|language| text_matches(language, expected))
}

fn workflow_transition_filter_matches(
    transition: &WorkflowTransition,
    filters: &WorkflowFilters,
) -> bool {
    workflow_edge_filter_matches(&transition.edge, filters)
        && filters.risk_severity.as_deref().is_none_or(|expected| {
            transition
                .risk_refs
                .iter()
                .any(|risk| text_matches(severity_name(risk.severity), expected))
        })
}

fn workflow_block_id(id: NodeId) -> String {
    format!("wb-{}", id.0)
}

fn workflow_block_kind(
    node: &Node,
    incoming_edge: Option<&Edge>,
    is_start: bool,
) -> WorkflowBlockKind {
    if is_start {
        return WorkflowBlockKind::Start;
    }
    if node.kind == NodeKind::ExternalDependency {
        return WorkflowBlockKind::ExternalBoundary;
    }
    match node.metadata.get("item_kind").map(String::as_str) {
        Some("branch") => return WorkflowBlockKind::Branch,
        Some("loop") => return WorkflowBlockKind::Loop,
        Some("async") => return WorkflowBlockKind::Async,
        _ => {}
    }
    match incoming_edge.map(|edge| &edge.kind) {
        Some(EdgeKind::Calls) => WorkflowBlockKind::Call,
        Some(EdgeKind::ReadsConfig) => WorkflowBlockKind::ConfigRead,
        Some(EdgeKind::ReadsEnvironment) => WorkflowBlockKind::EnvironmentRead,
        Some(EdgeKind::MayError) => WorkflowBlockKind::Error,
        Some(EdgeKind::DependsOn) => WorkflowBlockKind::Dependency,
        Some(EdgeKind::Imports) => WorkflowBlockKind::Import,
        Some(EdgeKind::References) => WorkflowBlockKind::Reference,
        _ => WorkflowBlockKind::Unknown,
    }
}

fn workflow_block_kind_label(kind: &WorkflowBlockKind) -> &'static str {
    match kind {
        WorkflowBlockKind::Start => "start",
        WorkflowBlockKind::Call => "call",
        WorkflowBlockKind::ConfigRead => "config",
        WorkflowBlockKind::EnvironmentRead => "env",
        WorkflowBlockKind::Dependency => "dependency",
        WorkflowBlockKind::Import => "import",
        WorkflowBlockKind::Branch => "branch",
        WorkflowBlockKind::Loop => "loop",
        WorkflowBlockKind::Async => "async",
        WorkflowBlockKind::Error => "error",
        WorkflowBlockKind::Reference => "reference",
        WorkflowBlockKind::ExternalBoundary => "external",
        WorkflowBlockKind::Unknown => "node",
    }
}

fn workflow_block_kind_filter_name(kind: &WorkflowBlockKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| workflow_block_kind_label(kind).to_string())
}

fn workflow_risk_refs_for_node(report: &InsightReport, node_id: NodeId) -> Vec<WorkflowRiskRef> {
    report
        .insights
        .iter()
        .enumerate()
        .filter(|(_, insight)| insight.nodes.contains(&node_id))
        .take(8)
        .map(|(insight_index, insight)| workflow_risk_ref(insight_index, insight))
        .collect()
}

fn workflow_risk_refs_for_edge(report: &InsightReport, edge_index: usize) -> Vec<WorkflowRiskRef> {
    report
        .insights
        .iter()
        .enumerate()
        .filter(|(_, insight)| insight.edges.contains(&edge_index))
        .take(8)
        .map(|(insight_index, insight)| workflow_risk_ref(insight_index, insight))
        .collect()
}

fn workflow_risk_ref(insight_index: usize, insight: &Insight) -> WorkflowRiskRef {
    WorkflowRiskRef {
        insight_index,
        kind: insight.kind.clone(),
        severity: insight.severity,
        message: insight.message.clone(),
        edge_indexes: insight.edges.clone(),
    }
}

fn mermaid_report_block_id(id: &str) -> String {
    let mut normalized = String::from("B");
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
        } else {
            normalized.push('_');
        }
    }
    normalized
}

fn mermaid_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('|', " ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TraceDirection {
    Outgoing,
    Incoming,
}

fn trace_edges_from_indexed(
    graph: &CodeGraph,
    node_id: NodeId,
    direction: TraceDirection,
) -> impl Iterator<Item = (usize, &Edge)> {
    graph.edges.iter().enumerate().filter(move |(_, edge)| {
        is_trace_edge(&edge.kind)
            && match direction {
                TraceDirection::Outgoing => edge.source == node_id,
                TraceDirection::Incoming => edge.target == node_id,
            }
    })
}

fn trace_edges_from(
    graph: &CodeGraph,
    node_id: NodeId,
    direction: TraceDirection,
) -> impl Iterator<Item = &Edge> {
    trace_edges_from_indexed(graph, node_id, direction).map(|(_, edge)| edge)
}

fn trace_next_node(edge: &Edge, node_id: NodeId, direction: TraceDirection) -> NodeId {
    match direction {
        TraceDirection::Outgoing => edge.target,
        TraceDirection::Incoming => {
            debug_assert_eq!(edge.target, node_id);
            edge.source
        }
    }
}

fn entrypoint_reachable_nodes(graph: &CodeGraph) -> BTreeSet<NodeId> {
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();

    for edge in &graph.edges {
        if edge.kind == EdgeKind::Entrypoint && reachable.insert(edge.target) {
            queue.push_back(edge.target);
        }
    }

    while let Some(node) = queue.pop_front() {
        for edge in graph
            .edges
            .iter()
            .filter(|edge| edge.source == node && is_trace_edge(&edge.kind))
        {
            if reachable.insert(edge.target) {
                queue.push_back(edge.target);
            }
        }
    }

    reachable
}

fn is_source_file_candidate(graph: &CodeGraph, node: &Node) -> bool {
    node.kind == NodeKind::File
        && node.metadata.contains_key("language")
        && !node.metadata.contains_key("skipped_reason")
        && !is_test_like_source_path(&node.label)
        && graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::Contains
                && graph
                    .nodes
                    .iter()
                    .any(|child| child.id == edge.target && is_code_symbol(&child.kind))
        })
}

fn file_has_reachable_code(
    graph: &CodeGraph,
    file_id: NodeId,
    reachable: &BTreeSet<NodeId>,
) -> bool {
    graph.edges.iter().any(|edge| {
        edge.source == file_id
            && edge.kind == EdgeKind::Contains
            && reachable.contains(&edge.target)
    })
}

fn contained_code_edge_indexes(graph: &CodeGraph, file_id: NodeId) -> Vec<usize> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            (edge.source == file_id && edge.kind == EdgeKind::Contains).then_some(index)
        })
        .collect()
}

fn is_code_symbol(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function | NodeKind::Type | NodeKind::Module | NodeKind::Entrypoint
    )
}

fn is_test_like_source_path(path: &str) -> bool {
    let normalized_original = path.replace('\\', "/");
    let normalized = normalized_original.to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let original_file_name = normalized_original
        .rsplit('/')
        .next()
        .unwrap_or(normalized_original.as_str());
    let stem = file_name
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_name);
    normalized.split('/').any(|part| {
        matches!(
            part,
            "test"
                | "tests"
                | "__test__"
                | "__tests__"
                | "spec"
                | "specs"
                | "testdata"
                | "testing"
                | "fixture"
                | "fixtures"
                | "example"
                | "examples"
                | "sample"
                | "samples"
                | "mock"
                | "mocks"
        )
    }) || stem == "test"
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("_spec")
        || stem.ends_with("_specs")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
        || file_name.ends_with(".bats")
        || original_file_name.ends_with("Test.php")
        || original_file_name.ends_with("Spec.php")
        || file_name.ends_with("_test.dart")
        || file_name.ends_with(".g.dart")
        || file_name.ends_with(".freezed.dart")
        || file_name.ends_with(".mocks.dart")
        || file_name.ends_with(".gen.dart")
        || normalized.contains("/.dart_tool/")
        || normalized.contains("/generated/")
}

fn is_dependency_manifest_source_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    matches!(
        normalized.rsplit('/').next().unwrap_or(normalized.as_str()),
        "setup.py"
    )
}

fn node_label(graph: &CodeGraph, id: NodeId) -> Option<&str> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| node.label.as_str())
}

fn kind_name(kind: &codegraph_core::NodeKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| format!("{kind:?}").to_ascii_lowercase())
}

fn architecture_group_for_path(path: &str) -> (String, String) {
    let normalized = path.trim_matches('/').replace('\\', "/");
    let Some((first, _)) = normalized.split_once('/') else {
        return (".".to_string(), "root".to_string());
    };
    let first = first.trim();
    if first.is_empty() {
        (".".to_string(), "root".to_string())
    } else {
        (first.to_string(), first.to_string())
    }
}

fn node_architecture_areas(
    graph: &CodeGraph,
    nodes_by_id: &BTreeMap<NodeId, &Node>,
) -> BTreeMap<NodeId, String> {
    let mut areas = BTreeMap::new();
    for node in nodes_by_id.values() {
        if node.kind == NodeKind::File {
            let (area, _) = architecture_group_for_path(&node.label);
            areas.insert(node.id, area);
        }
    }
    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Contains)
    {
        let Some(area) = areas.get(&edge.source).cloned() else {
            continue;
        };
        areas.entry(edge.target).or_insert(area);
    }
    areas
}

fn is_architecture_symbol(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Module
            | NodeKind::Function
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Config
            | NodeKind::Environment
    )
}

fn is_architecture_dependency_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Imports
            | EdgeKind::Calls
            | EdgeKind::References
            | EdgeKind::ReadsConfig
            | EdgeKind::ReadsEnvironment
            | EdgeKind::MayError
            | EdgeKind::Entrypoint
            | EdgeKind::DependsOn
    )
}

fn is_hotspot_candidate(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::File
            | NodeKind::Module
            | NodeKind::Function
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Config
            | NodeKind::Environment
    )
}

fn node_language(nodes_by_id: &BTreeMap<NodeId, &Node>, id: NodeId) -> String {
    nodes_by_id
        .get(&id)
        .and_then(|node| node.metadata.get("language"))
        .map(|language| language.trim())
        .filter(|language| !language.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn edge_kind_name(kind: &EdgeKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| format!("{kind:?}").to_ascii_lowercase())
}

fn confidence_name(confidence: codegraph_core::Confidence) -> String {
    serde_json_name(&confidence).unwrap_or_else(|| format!("{confidence:?}").to_ascii_lowercase())
}

fn severity_name(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Info => "info",
        InsightSeverity::Warning => "warning",
        InsightSeverity::Error => "error",
    }
}

fn dot_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn dot_color(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Repository => "#5cc8a7",
        NodeKind::Directory => "#7f9cff",
        NodeKind::File => "#67b7dc",
        NodeKind::Module => "#8ccf7e",
        NodeKind::Function => "#f2c14e",
        NodeKind::Entrypoint => "#5cc8a7",
        NodeKind::Type => "#df7e7e",
        NodeKind::Config => "#e5b454",
        NodeKind::Environment => "#d8a657",
        NodeKind::ExternalDependency => "#b88ee6",
        NodeKind::Unknown => "#a5adb3",
    }
}

fn serde_json_name<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeKind, SourceSpan};

    #[test]
    fn summary_counts_graph_facts() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let mut metadata = BTreeMap::new();
        metadata.insert("annotation.domain".to_string(), "payments".to_string());
        metadata.insert("annotation.owner".to_string(), "team-payments".to_string());
        graph.add_node_with_metadata(NodeKind::File, "src/payments.rs", None, metadata);
        graph.add_edge_with_metadata(
            graph.root,
            main,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
            BTreeMap::from([
                ("relation".to_string(), "entrypoint_function".to_string()),
                ("source".to_string(), "manifest".to_string()),
            ]),
        );

        let summary = summarize(&graph);

        assert_eq!(summary.nodes, 3);
        assert_eq!(summary.edges, 1);
        assert_eq!(summary.entrypoints, 1);
        assert_eq!(summary.node_kinds.get("function"), Some(&1));
        assert_eq!(summary.edge_confidences.get("syntactic"), Some(&1));
        assert_eq!(summary.edge_relations.get("entrypoint_function"), Some(&1));
        assert_eq!(summary.edge_sources.get("manifest"), Some(&1));
        assert_eq!(
            summary
                .annotation_facets
                .get("annotation.domain")
                .and_then(|values| values.get("payments")),
            Some(&1)
        );
        assert_eq!(
            summary
                .annotation_facets
                .get("annotation.owner")
                .and_then(|values| values.get("team-payments")),
            Some(&1)
        );
    }

    #[test]
    fn project_report_combines_summary_quality_and_limited_views() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/main.rs");
        graph.add_node_with_metadata(
            NodeKind::File,
            "src/broken.rs",
            None,
            BTreeMap::from([("parse_error".to_string(), "unexpected token".to_string())]),
        );
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_node(NodeKind::Function, "orphan");
        let config = graph.add_node(NodeKind::Config, "DATABASE_URL");
        let unresolved = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "missing",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "call".to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
            ]),
        );
        graph.add_edge(file, main, EdgeKind::Defines, Confidence::Exact);
        graph.add_edge(file, main, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(main, unresolved, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let report = project_report(
            &graph,
            ProjectReportLimits {
                architecture_group_limit: 5,
                architecture_edge_limit: 5,
                language_link_limit: 5,
                hotspot_limit: 1,
                community_limit: 5,
                insight_limit: 1,
                file_summary_limit: 1,
                node_summary_limit: 2,
                fail_on: InsightSeverity::Warning,
            },
        );

        assert_eq!(report.graph_schema_version, graph.schema_version);
        assert_eq!(report.summary.nodes, graph.nodes.len());
        assert_eq!(report.entrypoints.len(), 1);
        assert_eq!(report.hotspots.hotspots.len(), 1);
        assert_eq!(
            report.hotspots.total_architectural_hubs + report.hotspots.total_utility_hubs,
            report.hotspots.total_candidates
        );
        assert!(!report.surprising_links.links.is_empty());
        assert!(!report.communities.communities.is_empty());
        assert_eq!(report.quality_gate.fail_on, "warning");
        assert_eq!(report.insights.insights.len(), 1);
        assert_eq!(report.file_summaries.files.len(), 1);
        assert_eq!(report.file_summaries.total_files, 2);
        assert!(report.file_summaries.truncated);
        assert_eq!(report.node_summaries.nodes.len(), 2);
        assert!(report.node_summaries.total_nodes >= 3);
        assert!(report.node_summaries.truncated);
        assert!(
            report
                .node_summaries
                .nodes
                .iter()
                .any(|summary| summary.roles.iter().any(|role| role == "entrypoint"))
        );
        assert_eq!(report.insights.total, report.quality_gate.report.total);
        assert_eq!(report.risk_summary.total, report.quality_gate.report.total);
        assert_eq!(report.risk_summary.errors, 1);
        assert_eq!(report.risk_summary.warnings, 1);
        assert!(report.risk_summary.infos >= 1);
        assert_eq!(
            report.risk_summary.score,
            100 + 10 + report.risk_summary.infos
        );
        assert_eq!(report.risk_summary.grade, "high");
        assert!(!report.quality_gate.passed);
        assert_eq!(report.quality_gate.failing_insights, 2);
        assert!(
            report
                .risk_summary
                .top_kinds
                .iter()
                .any(|risk| risk.kind == "parse_error" && risk.severity == "error")
        );
    }

    #[test]
    fn project_report_markdown_includes_evidence_and_suggested_questions() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/main.rs");
        let main = graph.add_node_with_span(
            NodeKind::Function,
            "main",
            SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 3,
                start_column: 1,
                end_line: 5,
                end_column: 2,
            },
        );
        let config = graph.add_node(NodeKind::Config, "DATABASE_URL");
        let unresolved = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "missing_call",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "call".to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
            ]),
        );
        graph.add_edge(file, main, EdgeKind::Defines, Confidence::Exact);
        graph.add_edge(file, main, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(main, unresolved, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let report = project_report(
            &graph,
            ProjectReportLimits {
                architecture_group_limit: 5,
                architecture_edge_limit: 5,
                language_link_limit: 5,
                hotspot_limit: 5,
                community_limit: 5,
                insight_limit: 5,
                file_summary_limit: 5,
                node_summary_limit: 5,
                fail_on: InsightSeverity::Warning,
            },
        );
        let markdown = project_report_markdown(
            &report,
            &ProjectReportMarkdownOptions {
                title: "CodeGraph Project Report".to_string(),
                root: Some("repo".to_string()),
                generated_at_unix: Some(1_234),
            },
        );

        assert!(markdown.contains("# CodeGraph Project Report"));
        assert!(markdown.contains("- Root: `repo`"));
        assert!(markdown.contains("## Confidence Guide"));
        assert!(markdown.contains("## Compact Node Summaries"));
        assert!(markdown.contains("entrypoint"));
        assert!(markdown.contains("## Compact File Summaries"));
        assert!(markdown.contains("src/main.rs"));
        assert!(markdown.contains("| `exact` | extracted |"));
        assert!(markdown.contains("| `heuristic` | inferred |"));
        assert!(markdown.contains("| `unknown` | ambiguous |"));
        assert!(markdown.contains("## Key Concepts"));
        assert!(markdown.contains("## Communities"));
        assert!(markdown.contains("## Surprising Links"));
        assert!(markdown.contains("## Risks And Insights"));
        assert!(markdown.contains("### Insight Evidence"));
        assert!(markdown.contains("## Suggested Questions"));
        assert!(markdown.contains("missing_call"));
        assert!(markdown.contains("#2"));
        assert!(markdown.contains("What startup flow is reachable from main?"));
    }

    #[test]
    fn architecture_map_groups_files_and_cross_group_edges() {
        let mut graph = CodeGraph::new("repo");
        let api_file = graph.add_node(NodeKind::File, "api/main.rs");
        let core_file = graph.add_node(NodeKind::File, "core/lib.rs");
        let api_main = graph.add_node(NodeKind::Function, "main");
        let core_load = graph.add_node(NodeKind::Function, "load_config");
        graph.add_edge(graph.root, api_file, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(graph.root, core_file, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(
            api_file,
            api_main,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        graph.add_edge(
            core_file,
            core_load,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        graph.add_edge(api_main, core_load, EdgeKind::Calls, Confidence::Heuristic);

        let map = architecture_map(&graph, 10, 10);

        assert_eq!(map.total_groups, 2);
        assert_eq!(map.total_edges, 1);
        let api = map.groups.iter().find(|group| group.id == "api").unwrap();
        let core = map.groups.iter().find(|group| group.id == "core").unwrap();
        assert_eq!(api.files, 1);
        assert_eq!(api.symbols, 1);
        assert_eq!(core.files, 1);
        assert_eq!(core.symbols, 1);
        assert_eq!(map.edges[0].source, "api");
        assert_eq!(map.edges[0].target, "core");
        assert_eq!(map.edges[0].edge_kinds.get("calls"), Some(&1));
        assert_eq!(map.edges[0].edge_indexes, vec![4]);
    }

    #[test]
    fn surprising_links_rank_cross_area_language_and_heuristic_edges() {
        let mut graph = CodeGraph::new("repo");
        let api_file = graph.add_node_with_metadata(
            NodeKind::File,
            "api/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let scripts_file = graph.add_node_with_metadata(
            NodeKind::File,
            "scripts/deploy.py",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let handler = graph.add_node_with_metadata(
            NodeKind::Function,
            "handle_request",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let script = graph.add_node_with_metadata(
            NodeKind::Function,
            "deploy",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        graph.add_edge(api_file, handler, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(scripts_file, script, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(handler, script, EdgeKind::Calls, Confidence::Heuristic);

        let report = surprising_links(&graph, 10);

        assert_eq!(report.total_candidates, 1);
        let link = &report.links[0];
        assert_eq!(link.source.label, "handle_request");
        assert_eq!(link.target.label, "deploy");
        assert_eq!(link.source_area, "api");
        assert_eq!(link.target_area, "scripts");
        assert_eq!(link.source_language, "rust");
        assert_eq!(link.target_language, "python");
        assert_eq!(link.confidence, "heuristic");
        assert!(link.score >= 12);
        assert!(link.reasons.contains(&"cross_area".to_string()));
        assert!(link.reasons.contains(&"cross_language".to_string()));
        assert!(link.reasons.contains(&"heuristic_confidence".to_string()));
        assert_eq!(link.edge_index, 2);
    }

    #[test]
    fn language_dependencies_group_edges_by_node_languages() {
        let mut graph = CodeGraph::new("repo");
        let rust_main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let python_helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let python_config = graph.add_node_with_metadata(
            NodeKind::Config,
            "settings.yaml",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        graph.add_edge(
            rust_main,
            python_helper,
            EdgeKind::Calls,
            Confidence::Heuristic,
        );
        graph.add_edge(
            python_helper,
            python_config,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );
        graph.add_edge(graph.root, rust_main, EdgeKind::Contains, Confidence::Exact);

        let report = language_dependencies(&graph, 10);

        assert_eq!(report.total_links, 2);
        assert_eq!(report.total_edges, 2);
        assert_eq!(report.cross_language_edges, 1);
        let cross = report
            .links
            .iter()
            .find(|link| link.source_language == "rust" && link.target_language == "python")
            .unwrap();
        assert_eq!(cross.count, 1);
        assert_eq!(cross.edge_kinds.get("calls"), Some(&1));
        assert_eq!(cross.confidences.get("heuristic"), Some(&1));
        assert_eq!(cross.edge_indexes, vec![0]);
    }

    #[test]
    fn insights_report_cross_language_heuristic_edges() {
        let mut graph = CodeGraph::new("repo");
        let rust_main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let python_helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let rust_helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper_rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(
            rust_main,
            python_helper,
            EdgeKind::Calls,
            Confidence::Heuristic,
        );
        graph.add_edge(
            rust_main,
            rust_helper,
            EdgeKind::Calls,
            Confidence::Heuristic,
        );
        graph.add_edge(
            python_helper,
            rust_helper,
            EdgeKind::References,
            Confidence::Exact,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "cross_language_heuristic_edge")
            .expect("expected cross-language heuristic insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.edges, vec![0]);
        assert!(insight.nodes.contains(&rust_main));
        assert!(insight.nodes.contains(&python_helper));
        assert_eq!(
            report.by_kind.get("cross_language_heuristic_edge"),
            Some(&1)
        );
    }

    #[test]
    fn hotspots_rank_nodes_by_dependency_degree() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let load_config = graph.add_node(NodeKind::Function, "load_config");
        let settings = graph.add_node(NodeKind::Config, "settings.toml");
        let helper = graph.add_node(NodeKind::Function, "helper");
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            settings,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );

        let report = hotspots(&graph, 2);

        assert_eq!(report.total_candidates, 4);
        assert!(report.truncated);
        assert_eq!(report.hotspots[0].node.label, "load_config");
        assert_eq!(report.hotspots[0].score, 3);
        assert_eq!(report.hotspots[0].incoming, 2);
        assert_eq!(report.hotspots[0].outgoing, 1);
        assert_eq!(report.hotspots[0].hub_kind, "architectural");
        assert_eq!(report.architectural_hubs[0].node.label, "load_config");
        assert_eq!(report.total_architectural_hubs, 4);
        assert_eq!(report.total_utility_hubs, 0);
        assert_eq!(report.hotspots[0].edge_kinds.get("calls"), Some(&2));
        assert_eq!(report.hotspots[0].edge_kinds.get("reads_config"), Some(&1));
    }

    #[test]
    fn hotspots_separate_architectural_hubs_from_utility_hubs() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Entrypoint, "server main");
        let load_config = graph.add_node(NodeKind::Function, "load_config");
        let new_fn = graph.add_node(NodeKind::Function, "new");
        let default_fn = graph.add_node(NodeKind::Function, "default");
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, new_fn, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(load_config, new_fn, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(default_fn, new_fn, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, default_fn, EdgeKind::Calls, Confidence::Heuristic);

        let report = hotspots(&graph, 5);

        assert!(
            report
                .utility_hubs
                .iter()
                .any(|hotspot| hotspot.node.label == "new" && hotspot.hub_kind == "utility")
        );
        assert!(
            report
                .architectural_hubs
                .iter()
                .any(|hotspot| hotspot.node.label == "server main"
                    && hotspot.hub_kind == "architectural")
        );
        assert!(report.total_utility_hubs >= 2);
        assert!(report.total_architectural_hubs >= 2);
    }

    #[test]
    fn communities_group_related_files_symbols_and_external_edges() {
        let mut graph = CodeGraph::new("repo");
        let api_file = graph.add_node_with_metadata(
            NodeKind::File,
            "api/users.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let core_file = graph.add_node_with_metadata(
            NodeKind::File,
            "core/db.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let _docs_file = graph.add_node(NodeKind::File, "docs/adr.md");
        let route = graph.add_node(NodeKind::Entrypoint, "route GET /users");
        let handler = graph.add_node(NodeKind::Function, "list_users");
        let db = graph.add_node(NodeKind::Function, "load_users");
        graph.add_edge(api_file, route, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(api_file, handler, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(core_file, db, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(route, handler, EdgeKind::Calls, Confidence::Exact);
        graph.add_edge(handler, db, EdgeKind::Calls, Confidence::Heuristic);

        let report = communities(&graph, 1);

        assert_eq!(report.total_communities, 3);
        assert_eq!(report.total_nodes, 6);
        assert_eq!(report.total_external_edges, 1);
        assert!(report.truncated);
        let community = &report.communities[0];
        assert_eq!(community.label, "api");
        assert_eq!(community.node_count, 3);
        assert_eq!(community.files, 1);
        assert_eq!(community.entrypoints, 1);
        assert_eq!(community.internal_edges, 3);
        assert_eq!(community.incoming_external_edges, 0);
        assert_eq!(community.outgoing_external_edges, 1);
        assert_eq!(community.languages.get("rust"), Some(&1));
        assert_eq!(community.node_kinds.get("file"), Some(&1));
        assert!(
            community
                .sample_nodes
                .iter()
                .any(|node| node.label == "route GET /users")
        );
        assert_eq!(community.edge_indexes, vec![0, 1, 3, 4]);

        let full = communities(&graph, 10);
        let core = full
            .communities
            .iter()
            .find(|community| community.label == "core")
            .expect("core community should be present");
        assert_eq!(core.incoming_external_edges, 1);
        assert_eq!(core.outgoing_external_edges, 0);
        let docs = full
            .communities
            .iter()
            .find(|community| community.label == "docs")
            .expect("isolated docs file should remain visible as a community");
        assert_eq!(docs.node_count, 1);
        assert_eq!(docs.files, 1);
    }

    #[test]
    fn source_search_filters_limits_and_returns_context() {
        let root = temp_analysis_root();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(
            root.join("src").join("app.py"),
            "def main():\n    token = load_secret()\n    return token\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src").join("config.py"),
            "SECRET_NAME = 'token'\n",
        )
        .unwrap();
        std::fs::write(
            root.join("target").join("generated.py"),
            "token = 'ignored'\n",
        )
        .unwrap();

        let result = search_source(
            &root,
            &SourceSearchRequest {
                query: "TOKEN".to_string(),
                path_filter: Some("src/".to_string()),
                case_sensitive: false,
                limit: 2,
                context: 1,
                include_hidden: false,
                include_ignored: false,
                max_file_size: 1024,
                ignored_names: BTreeSet::from(["target".to_string()]),
                ignored_globs: BTreeSet::from(["fixtures/**".to_string()]),
            },
        );

        assert_eq!(result.total_matches, 3);
        assert_eq!(result.matches.len(), 2);
        assert!(result.truncated);
        assert!(
            result
                .matches
                .iter()
                .all(|item| item.path.starts_with("src/"))
        );
        assert!(
            !result
                .matches
                .iter()
                .any(|item| item.path.contains("target"))
        );
        let app_match = result
            .matches
            .iter()
            .find(|item| item.path == "src/app.py" && item.line == 2)
            .expect("missing app.py token match");
        assert_eq!(app_match.column, 5);
        assert!(
            app_match
                .context
                .iter()
                .any(|line| line.highlight && line.text.contains("token = load_secret()"))
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_search_respects_ignored_globs() {
        let root = temp_analysis_root();
        std::fs::create_dir_all(root.join("src").join("generated")).unwrap();
        std::fs::create_dir_all(root.join("src").join("domain")).unwrap();
        std::fs::write(
            root.join("src").join("generated").join("skip.py"),
            "token = 1\n",
        )
        .unwrap();
        std::fs::write(
            root.join("src").join("domain").join("keep.py"),
            "token = 2\n",
        )
        .unwrap();

        let result = search_source(
            &root,
            &SourceSearchRequest {
                query: "token".to_string(),
                path_filter: None,
                case_sensitive: false,
                limit: 10,
                context: 0,
                include_hidden: false,
                include_ignored: false,
                max_file_size: 1024,
                ignored_names: BTreeSet::new(),
                ignored_globs: BTreeSet::from(["src/generated/**".to_string()]),
            },
        );

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.matches[0].path, "src/domain/keep.py");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn node_card_includes_context_source_and_related_insights() {
        let root = temp_analysis_root();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src").join("main.rs"),
            "fn main() {\n    missing();\n}\n",
        )
        .unwrap();
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/main.rs");
        let function = graph.add_node_with_span(
            NodeKind::Function,
            "main",
            SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 3,
                end_column: 2,
            },
        );
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "call".to_string());
        metadata.insert("unresolved".to_string(), "true".to_string());
        let call = graph.add_node_with_metadata(NodeKind::Unknown, "missing", None, metadata);
        let env = graph.add_node_with_metadata(
            NodeKind::Environment,
            "DATABASE_URL",
            None,
            BTreeMap::from([(
                "default_value".to_string(),
                "postgres://demo:password@localhost/app".to_string(),
            )]),
        );
        let mut error_metadata = BTreeMap::new();
        error_metadata.insert("item_kind".to_string(), "error".to_string());
        let error = graph.add_node_with_metadata(NodeKind::Unknown, "panic", None, error_metadata);
        graph.add_edge(file, function, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(function, call, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            function,
            env,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(function, error, EdgeKind::MayError, Confidence::Heuristic);

        let card = node_card(&graph, Some(&root), function, 10, 1, 10)
            .unwrap()
            .expect("expected node card");

        assert_eq!(card.context.node.id, function);
        assert_eq!(card.context.edges.len(), 4);
        assert_eq!(card.dependency_summary.incoming, 1);
        assert_eq!(card.dependency_summary.outgoing, 3);
        assert_eq!(card.dependency_summary.edge_kinds.get("contains"), Some(&1));
        assert_eq!(card.dependency_summary.edge_kinds.get("calls"), Some(&1));
        assert_eq!(
            card.dependency_summary.edge_kinds.get("reads_environment"),
            Some(&1)
        );
        assert_eq!(
            card.dependency_summary.edge_kinds.get("may_error"),
            Some(&1)
        );
        assert_eq!(card.dependency_summary.confidences.get("exact"), Some(&1));
        assert_eq!(
            card.dependency_summary.confidences.get("heuristic"),
            Some(&3)
        );
        assert_eq!(card.dependency_summary.neighbor_kinds.get("file"), Some(&1));
        assert_eq!(
            card.dependency_summary.neighbor_kinds.get("unknown"),
            Some(&2)
        );
        assert_eq!(
            card.dependency_summary.neighbor_kinds.get("environment"),
            Some(&1)
        );
        assert_eq!(
            card.source.as_ref().map(|source| source.path.as_str()),
            Some("src/main.rs")
        );
        assert!(
            card.source
                .as_ref()
                .unwrap()
                .lines
                .iter()
                .any(|line| line.highlight && line.text.contains("missing"))
        );
        assert_eq!(card.total_insights, 3);
        assert_eq!(card.insight_summary.by_severity.get("warning"), Some(&2));
        assert_eq!(card.insight_summary.by_severity.get("info"), Some(&1));
        assert_eq!(
            card.insight_summary.by_kind.get("orphan_function"),
            Some(&1)
        );
        assert_eq!(
            card.insight_summary.by_kind.get("potential_error_flow"),
            Some(&1)
        );
        assert_eq!(
            card.insight_summary.by_kind.get("sensitive_config_default"),
            Some(&1)
        );
        assert!(
            card.insights
                .iter()
                .any(|insight| insight.kind == "orphan_function")
        );
        assert!(
            card.insights
                .iter()
                .any(|insight| insight.kind == "potential_error_flow")
        );
        assert!(
            card.insights
                .iter()
                .any(|insight| insight.kind == "sensitive_config_default")
        );
        assert!(card.actions.iter().any(|action| {
            action.kind == "symbol_graph"
                && action.query
                    == format!(
                        "symbols node_id:{} direction:out edge_limit:300",
                        function.0
                    )
        }));

        let file_card = node_card(&graph, Some(&root), file, 10, 1, 10)
            .unwrap()
            .expect("expected file node card");
        assert_eq!(file_card.context.node.id, file);
        assert_eq!(file_card.total_insights, 3);
        assert_eq!(
            file_card.insight_summary.by_kind.get("orphan_function"),
            Some(&1)
        );
        assert_eq!(
            file_card
                .insight_summary
                .by_kind
                .get("potential_error_flow"),
            Some(&1)
        );
        assert_eq!(
            file_card
                .insight_summary
                .by_kind
                .get("sensitive_config_default"),
            Some(&1)
        );
        assert!(
            file_card
                .insights
                .iter()
                .any(|insight| insight.kind == "orphan_function")
        );
        assert!(
            file_card
                .insights
                .iter()
                .any(|insight| insight.kind == "potential_error_flow")
        );
        assert!(
            file_card
                .insights
                .iter()
                .any(|insight| insight.kind == "sensitive_config_default")
        );
        assert_eq!(
            file_card.source.as_ref().map(|source| source.path.as_str()),
            Some("src/main.rs")
        );
        assert!(
            file_card
                .source
                .as_ref()
                .unwrap()
                .lines
                .iter()
                .any(|line| !line.highlight && line.text.contains("fn main"))
        );
        assert!(file_card.actions.iter().any(|action| {
            action.kind == "file_graph"
                && action.query == "files path:src/main.rs direction:out edge_limit:300"
        }));
        let file_summary = file_card
            .file_summary
            .as_ref()
            .expect("expected file summary");
        assert_eq!(file_summary.contained_nodes, 1);
        assert_eq!(file_summary.code_symbols, 1);
        assert_eq!(file_summary.trace_edges, 3);
        assert_eq!(file_summary.calls, 1);
        assert_eq!(file_summary.unresolved_calls, 1);
        assert_eq!(file_summary.environment_reads, 1);
        assert_eq!(file_summary.error_facts, 1);
        assert_eq!(file_summary.contained_kinds.get("function"), Some(&1));
        assert_eq!(file_summary.trace_edge_kinds.get("calls"), Some(&1));
        assert_eq!(
            file_summary.trace_edge_kinds.get("reads_environment"),
            Some(&1)
        );
        assert_eq!(file_summary.trace_edge_kinds.get("may_error"), Some(&1));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn node_card_suggests_focused_graph_actions() {
        let mut graph = CodeGraph::new("repo");
        let mut dependency_metadata = BTreeMap::new();
        dependency_metadata.insert("item_kind".to_string(), "dependency".to_string());
        dependency_metadata.insert("package_id".to_string(), "cargo:serde".to_string());
        let dependency = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "serde",
            None,
            dependency_metadata,
        );
        let config = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        let mut error_metadata = BTreeMap::new();
        error_metadata.insert("item_kind".to_string(), "error".to_string());
        let error = graph.add_node_with_metadata(NodeKind::Unknown, "panic", None, error_metadata);
        let document = graph.add_node_with_metadata(
            NodeKind::File,
            "docs/adr/0001-runtime.md",
            None,
            BTreeMap::from([
                ("language".to_string(), "markdown".to_string()),
                ("item_kind".to_string(), "document".to_string()),
                ("document_kind".to_string(), "adr".to_string()),
            ]),
        );

        let dependency_card = node_card(&graph, None, dependency, 10, 1, 10)
            .unwrap()
            .expect("expected dependency card");
        assert!(dependency_card.actions.iter().any(|action| {
            action.kind == "package_graph"
                && action.query == format!("packages node_id:{} edge_limit:300", dependency.0)
        }));

        let config_card = node_card(&graph, None, config, 10, 1, 10)
            .unwrap()
            .expect("expected config card");
        assert!(config_card.actions.iter().any(|action| {
            action.kind == "config_graph"
                && action.query == format!("configs node_id:{} depth:6", config.0)
        }));

        let error_card = node_card(&graph, None, error, 10, 1, 10)
            .unwrap()
            .expect("expected error card");
        assert!(error_card.actions.iter().any(|action| {
            action.kind == "error_graph"
                && action.query == format!("errors node_id:{} depth:6", error.0)
        }));

        let document_card = node_card(&graph, None, document, 10, 1, 10)
            .unwrap()
            .expect("expected document card");
        assert!(document_card.actions.iter().any(|action| {
            action.kind == "document_graph"
                && action.query == format!("docs node_id:{} edge_limit:300", document.0)
        }));
    }

    #[test]
    fn exports_dot_and_ndjson() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);

        let dot = export_dot(&graph);
        assert!(dot.starts_with("digraph CodeGraph"));
        assert!(dot.contains("main"));
        assert!(dot.contains("contains"));

        let ndjson = export_ndjson(&graph).unwrap();
        let records: Vec<_> = ndjson
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect();
        assert_eq!(records.len(), 4);
        assert_eq!(records[0]["record_type"], "graph");
        assert_eq!(records[1]["record_type"], "node");
        assert_eq!(records[3]["record_type"], "edge");
    }

    #[test]
    fn trace_follows_dependency_edges() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);

        let result = trace(
            &graph,
            TraceRequest {
                start: TraceStart::Label("main".to_string()),
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.edges.len(), 1);
        assert_eq!(
            result
                .nodes
                .iter()
                .find(|node| node.node.id == helper)
                .unwrap()
                .depth,
            1
        );
    }

    #[test]
    fn trace_dependents_follows_incoming_dependency_edges() {
        let mut graph = CodeGraph::new("repo");
        let caller = graph.add_node(NodeKind::Function, "caller");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        graph.add_edge(caller, main, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let result = trace_dependents(
            &graph,
            TraceRequest {
                start: TraceStart::Label("settings.toml".to_string()),
                max_depth: 3,
            },
        )
        .unwrap();

        assert_eq!(result.nodes.len(), 4);
        assert_eq!(result.edges.len(), 3);
        assert_eq!(
            result
                .nodes
                .iter()
                .find(|node| node.node.id == caller)
                .unwrap()
                .depth,
            3
        );
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == helper && edge.target == config)
        );
    }

    #[test]
    fn trace_follows_entrypoint_reference_edges() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Syntactic,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );

        let result = trace(
            &graph,
            TraceRequest {
                start: TraceStart::Label("cargo bin:demo".to_string()),
                max_depth: 1,
            },
        )
        .unwrap();

        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.edges.len(), 1);
        assert!(result.nodes.iter().any(|node| node.node.id == main));
    }

    #[test]
    fn trace_entrypoints_returns_filtered_entrypoint_flows() {
        let mut graph = CodeGraph::new("repo");
        let cli_entrypoint = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:codegraph-cli",
            None,
            BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
        );
        let server_entrypoint = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:codegraph-server",
            None,
            BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
        );
        let cli_main = graph.add_node(NodeKind::Function, "cli_main");
        let server_main = graph.add_node(NodeKind::Function, "server_main");
        graph.add_edge(
            graph.root,
            cli_entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            graph.root,
            server_entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            cli_entrypoint,
            cli_main,
            EdgeKind::References,
            Confidence::Syntactic,
        );
        graph.add_edge(
            server_entrypoint,
            server_main,
            EdgeKind::References,
            Confidence::Syntactic,
        );

        let report = trace_entrypoints(
            &graph,
            EntrypointTraceRequest {
                search: Some("server".to_string()),
                max_depth: 1,
                limit: 10,
            },
        );

        assert_eq!(report.total_entrypoints, 1);
        assert_eq!(report.traces.len(), 1);
        assert_eq!(report.traces[0].start.id, server_entrypoint);
        assert!(
            report.traces[0]
                .nodes
                .iter()
                .any(|node| node.node.id == server_main)
        );
        assert!(
            !report.traces[0]
                .nodes
                .iter()
                .any(|node| node.node.id == cli_main)
        );
    }

    #[test]
    fn workflow_builds_block_steps_with_risk_context() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
        let main = graph.add_node(NodeKind::Function, "main");
        let load_config = graph.add_node(NodeKind::Function, "load_config");
        let env = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        let config = graph.add_node(NodeKind::Config, "config/app.toml");
        let error = graph.add_node(NodeKind::Unknown, "panic: missing config");
        let package = graph.add_node(NodeKind::ExternalDependency, "serde");
        graph.add_edge(
            graph.root,
            entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Syntactic,
        );
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            env,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            load_config,
            config,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );
        graph.add_edge(
            load_config,
            error,
            EdgeKind::MayError,
            Confidence::Heuristic,
        );
        graph.add_edge(load_config, package, EdgeKind::DependsOn, Confidence::Exact);

        let report = workflow(
            &graph,
            WorkflowRequest {
                start: TraceStart::Label("cargo bin:api".to_string()),
                max_depth: 3,
                block_limit: 20,
                filters: WorkflowFilters::default(),
                compact: false,
            },
        )
        .expect("workflow report");

        assert_eq!(report.start.id, entrypoint);
        assert_eq!(report.total_blocks, 7);
        assert_eq!(report.total_transitions, 6);
        assert!(report.blocks.iter().any(|block| {
            block.node.id == entrypoint
                && block.id == format!("wb-{}", entrypoint.0)
                && block.kind == WorkflowBlockKind::Start
                && block.depth == 0
        }));
        assert!(report.blocks.iter().any(|block| {
            block.node.id == load_config && block.kind == WorkflowBlockKind::Call
        }));
        assert!(report.blocks.iter().any(|block| {
            block.node.id == env && block.kind == WorkflowBlockKind::EnvironmentRead
        }));
        assert!(report.blocks.iter().any(|block| {
            block.node.id == config && block.kind == WorkflowBlockKind::ConfigRead
        }));
        assert!(report.blocks.iter().any(|block| {
            block.node.id == error
                && block.kind == WorkflowBlockKind::Error
                && block
                    .risk_refs
                    .iter()
                    .any(|risk| risk.kind == "potential_error_flow")
        }));
        assert!(report.blocks.iter().any(|block| {
            block.node.id == package && block.kind == WorkflowBlockKind::ExternalBoundary
        }));
        assert!(report.transitions.iter().any(|transition| {
            transition.source_node_id == load_config
                && transition.target_node_id == error
                && transition.edge.metadata.get("edge_index").is_some()
                && transition
                    .risk_refs
                    .iter()
                    .any(|risk| risk.kind == "potential_error_flow")
        }));

        let mermaid = workflow_mermaid(&report);
        assert!(mermaid.starts_with("flowchart TD"));
        assert!(mermaid.contains("start: cargo bin:api"));
        assert!(mermaid.contains("reads_environment/heuristic"));
    }

    #[test]
    fn workflow_classifies_control_flow_blocks_from_item_kind() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
        let main = graph.add_node(NodeKind::Function, "main");
        let branch = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "branch: if",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "branch".to_string()),
                ("language".to_string(), "rust".to_string()),
                ("control_kind".to_string(), "if".to_string()),
            ]),
        );
        let loop_node = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "loop: for",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "loop".to_string()),
                ("language".to_string(), "rust".to_string()),
                ("control_kind".to_string(), "for".to_string()),
            ]),
        );
        let async_node = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "async: await",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "async".to_string()),
                ("language".to_string(), "rust".to_string()),
                ("control_kind".to_string(), "await".to_string()),
            ]),
        );
        graph.add_edge(
            graph.root,
            entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Syntactic,
        );
        graph.add_edge(main, branch, EdgeKind::References, Confidence::Heuristic);
        graph.add_edge(main, loop_node, EdgeKind::References, Confidence::Heuristic);
        graph.add_edge(
            main,
            async_node,
            EdgeKind::References,
            Confidence::Heuristic,
        );

        let report = workflow(
            &graph,
            WorkflowRequest {
                start: TraceStart::Label("cargo bin:api".to_string()),
                max_depth: 2,
                block_limit: 20,
                filters: WorkflowFilters::default(),
                compact: false,
            },
        )
        .expect("workflow report");

        assert!(
            report
                .blocks
                .iter()
                .any(|block| block.node.id == branch && block.kind == WorkflowBlockKind::Branch)
        );
        assert!(
            report
                .blocks
                .iter()
                .any(|block| block.node.id == loop_node && block.kind == WorkflowBlockKind::Loop)
        );
        assert!(
            report
                .blocks
                .iter()
                .any(|block| block.node.id == async_node && block.kind == WorkflowBlockKind::Async)
        );
    }

    #[test]
    fn workflow_compacts_repeated_low_signal_blocks() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper_a = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper_a",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper_b = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper_b",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(main, helper_a, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, helper_b, EdgeKind::Calls, Confidence::Heuristic);

        let report = workflow(
            &graph,
            WorkflowRequest {
                start: TraceStart::Label("main".to_string()),
                max_depth: 1,
                block_limit: 20,
                filters: WorkflowFilters::default(),
                compact: true,
            },
        )
        .expect("workflow report");

        assert!(report.compact);
        assert_eq!(report.raw_total_blocks, 3);
        assert_eq!(report.raw_total_transitions, 2);
        assert_eq!(report.total_blocks, 2);
        assert_eq!(report.total_transitions, 1);
        let compacted = report
            .blocks
            .iter()
            .find(|block| block.compacted)
            .expect("compacted block");
        assert_eq!(compacted.compacted_count, 2);
        assert_eq!(compacted.source_node_ids, vec![helper_a, helper_b]);
        assert!(
            compacted
                .node
                .label
                .contains("2 compacted rust call blocks")
        );
        let compacted_transition = report
            .transitions
            .iter()
            .find(|transition| transition.compacted)
            .expect("compacted transition");
        assert_eq!(compacted_transition.compacted_count, 2);

        let mermaid = workflow_mermaid(&report);
        assert!(mermaid.contains("2 compacted rust call blocks"));
    }

    #[test]
    fn workflow_filters_blocks_edges_language_and_risk() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
        let main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let load_config = graph.add_node_with_metadata(
            NodeKind::Function,
            "load_config",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let env = graph.add_node_with_metadata(
            NodeKind::Environment,
            "DATABASE_URL",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "panic: missing config",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(
            graph.root,
            entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(entrypoint, main, EdgeKind::References, Confidence::Exact);
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            env,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            load_config,
            error,
            EdgeKind::MayError,
            Confidence::Heuristic,
        );

        let env_only = workflow(
            &graph,
            WorkflowRequest {
                start: TraceStart::Label("main".to_string()),
                max_depth: 3,
                block_limit: 20,
                filters: WorkflowFilters {
                    language: Some("rust".to_string()),
                    block_kind: Some("environment_read".to_string()),
                    ..WorkflowFilters::default()
                },
                compact: false,
            },
        )
        .expect("environment workflow");
        assert_eq!(
            env_only.filters.block_kind.as_deref(),
            Some("environment_read")
        );
        assert!(
            env_only
                .blocks
                .iter()
                .any(|block| block.node.id == main && block.kind == WorkflowBlockKind::Start)
        );
        assert!(env_only.blocks.iter().any(|block| {
            block.node.id == env && block.kind == WorkflowBlockKind::EnvironmentRead
        }));
        assert!(!env_only.blocks.iter().any(|block| block.node.id == error));

        let risky_errors = workflow(
            &graph,
            WorkflowRequest {
                start: TraceStart::Label("load_config".to_string()),
                max_depth: 1,
                block_limit: 20,
                filters: WorkflowFilters {
                    edge_kind: Some("may_error".to_string()),
                    confidence: Some("heuristic".to_string()),
                    risk_severity: Some("warning".to_string()),
                    ..WorkflowFilters::default()
                },
                compact: false,
            },
        )
        .expect("risk workflow");
        assert_eq!(risky_errors.total_blocks, 2);
        assert_eq!(risky_errors.total_transitions, 1);
        assert!(risky_errors.blocks.iter().any(|block| {
            block.node.id == error
                && block.kind == WorkflowBlockKind::Error
                && block
                    .risk_refs
                    .iter()
                    .any(|risk| risk.severity == InsightSeverity::Warning)
        }));
        assert!(risky_errors.transitions.iter().all(|transition| {
            transition.edge.kind == EdgeKind::MayError
                && transition.edge.confidence == Confidence::Heuristic
                && transition
                    .risk_refs
                    .iter()
                    .any(|risk| risk.severity == InsightSeverity::Warning)
        }));
    }

    #[test]
    fn workflow_entrypoints_returns_filtered_block_reports() {
        let mut graph = CodeGraph::new("repo");
        let api_entrypoint = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:api",
            None,
            BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
        );
        let worker_entrypoint = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:worker",
            None,
            BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
        );
        let api_main = graph.add_node(NodeKind::Function, "api_main");
        let worker_main = graph.add_node(NodeKind::Function, "worker_main");
        graph.add_edge(
            graph.root,
            api_entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            graph.root,
            worker_entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            api_entrypoint,
            api_main,
            EdgeKind::References,
            Confidence::Syntactic,
        );
        graph.add_edge(
            worker_entrypoint,
            worker_main,
            EdgeKind::References,
            Confidence::Syntactic,
        );

        let report = workflow_entrypoints(
            &graph,
            EntrypointWorkflowRequest {
                search: Some("api".to_string()),
                max_depth: 2,
                block_limit: 10,
                limit: 10,
                filters: WorkflowFilters::default(),
                compact: false,
            },
        );

        assert_eq!(report.max_depth, 2);
        assert_eq!(report.block_limit, 10);
        assert_eq!(report.total_entrypoints, 1);
        assert_eq!(report.workflows.len(), 1);
        assert_eq!(report.workflows[0].start.id, api_entrypoint);
        assert!(
            report.workflows[0].blocks.iter().any(
                |block| block.node.id == api_main && block.kind == WorkflowBlockKind::Reference
            )
        );
        assert!(
            !report.workflows[0]
                .blocks
                .iter()
                .any(|block| block.node.id == worker_main)
        );
    }

    #[test]
    fn workflow_query_builds_reports_from_query_result_nodes() {
        let mut graph = CodeGraph::new("repo");
        let api = graph.add_node_with_metadata(
            NodeKind::Function,
            "api_main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let worker = graph.add_node_with_metadata(
            NodeKind::Function,
            "worker_main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let api_helper = graph.add_node(NodeKind::Function, "api_helper");
        let worker_helper = graph.add_node(NodeKind::Function, "worker_helper");
        graph.add_edge(api, api_helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            worker,
            worker_helper,
            EdgeKind::Calls,
            Confidence::Heuristic,
        );

        let report = workflow_query(
            &graph,
            WorkflowQueryRequest {
                query: "nodes kind:function search:main".to_string(),
                max_depth: 2,
                block_limit: 20,
                limit: 1,
                filters: WorkflowFilters {
                    edge_kind: Some("calls".to_string()),
                    confidence: Some("heuristic".to_string()),
                    ..WorkflowFilters::default()
                },
                compact: false,
            },
        )
        .expect("workflow query report");

        assert_eq!(report.query, "nodes kind:function search:main");
        assert_eq!(report.max_depth, 2);
        assert_eq!(report.block_limit, 20);
        assert_eq!(report.filters.edge_kind.as_deref(), Some("calls"));
        assert_eq!(report.total_query_nodes, 2);
        assert_eq!(report.total_candidates, 2);
        assert_eq!(report.workflows.len(), 1);
        assert!(report.truncated);
        assert_eq!(report.workflows[0].start.id, api);
        assert!(
            report.workflows[0]
                .blocks
                .iter()
                .any(|block| block.node.id == api_helper)
        );
    }

    #[test]
    fn query_filters_nodes_by_kind_label_and_metadata() {
        let mut graph = CodeGraph::new("repo");
        let mut metadata = BTreeMap::new();
        metadata.insert("language".to_string(), "rust".to_string());
        graph.add_node_with_metadata(NodeKind::Function, "load_config", None, metadata);
        graph.add_node(NodeKind::Function, "render");
        graph.add_node(NodeKind::File, "src/main.rs");

        let result = query_graph(
            &graph,
            "nodes kind:function label:load metadata.language:rust",
        )
        .unwrap();

        assert_eq!(result.total_nodes, 1);
        assert_eq!(result.nodes[0].label, "load_config");
        assert!(result.edges.is_empty());
    }

    #[test]
    fn query_annotations_returns_annotated_node_context() {
        let mut graph = CodeGraph::new("repo");
        let mut payment_metadata = BTreeMap::new();
        payment_metadata.insert("language".to_string(), "rust".to_string());
        payment_metadata.insert("annotation.domain".to_string(), "payments".to_string());
        payment_metadata.insert("annotation.layer".to_string(), "service".to_string());
        let payment =
            graph.add_node_with_metadata(NodeKind::Function, "charge_card", None, payment_metadata);
        let database = graph.add_node(NodeKind::Function, "write_payment");
        let mut billing_metadata = BTreeMap::new();
        billing_metadata.insert("annotation.domain".to_string(), "billing".to_string());
        billing_metadata.insert("annotation.team".to_string(), "payments".to_string());
        graph.add_node_with_metadata(NodeKind::Function, "invoice", None, billing_metadata);
        graph.add_edge(payment, database, EdgeKind::Calls, Confidence::Heuristic);

        let result = query_graph(
            &graph,
            "annotations key:domain value:payments direction:out edge_limit:10",
        )
        .unwrap();

        assert_eq!(result.total_edges, 1);
        assert!(result.nodes.iter().any(|node| node.id == payment));
        assert!(result.nodes.iter().any(|node| node.id == database));
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == payment && edge.target == database)
        );
        assert!(!result.nodes.iter().any(|node| node.label == "invoice"));

        let exact = query_graph(&graph, "annotations annotation.domain:payments").unwrap();
        assert_eq!(exact.total_nodes, 2);
        assert!(exact.nodes.iter().any(|node| node.id == payment));

        let error = query_graph(&graph, "annotations nope:value")
            .expect_err("invalid annotations term should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported annotations query term")
        );
    }

    #[test]
    fn query_filters_edges_and_supports_calls_alias() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let other = graph.add_node(NodeKind::Function, "other");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(other, helper, EdgeKind::Calls, Confidence::Heuristic);

        let result = query_graph(&graph, "calls(function:main)").unwrap();

        assert_eq!(result.total_edges, 1);
        assert_eq!(result.edges[0].source, main);
        assert_eq!(result.edges[0].target, helper);
        assert_eq!(
            result.edges[0].metadata.get("edge_index"),
            Some(&"0".to_string())
        );
        assert_eq!(result.nodes.len(), 2);

        let by_index = query_graph(&graph, "edges edge_index:1").unwrap();
        assert_eq!(by_index.total_edges, 1);
        assert_eq!(by_index.edges[0].source, other);
        assert_eq!(by_index.edges[0].target, helper);
        assert_eq!(
            by_index.edges[0].metadata.get("edge_index"),
            Some(&"1".to_string())
        );
    }

    #[test]
    fn query_filters_edges_by_confidence() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let manifest = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(manifest, main, EdgeKind::References, Confidence::Exact);

        let heuristic = query_graph(&graph, "edges confidence:heuristic").unwrap();
        assert_eq!(heuristic.total_edges, 1);
        assert_eq!(heuristic.edges[0].kind, EdgeKind::Calls);

        let exact_reference =
            query_graph(&graph, "edges kind:references confidence:exact").unwrap();
        assert_eq!(exact_reference.total_edges, 1);
        assert_eq!(exact_reference.edges[0].source, manifest);
    }

    #[test]
    fn explain_edge_returns_provenance_for_matching_edge() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Syntactic,
            BTreeMap::from([
                ("relation".to_string(), "entrypoint_function".to_string()),
                ("resolution".to_string(), "manifest_path".to_string()),
            ]),
        );

        let explanation = explain_edge(
            &graph,
            ExplainEdgeRequest {
                edge_index: None,
                source: Some("cargo bin".to_string()),
                target: Some("main".to_string()),
                kind: Some("references".to_string()),
            },
        )
        .unwrap()
        .expect("missing explanation");

        assert_eq!(explanation.edge_index, 0);
        assert_eq!(explanation.total_matches, 1);
        assert_eq!(explanation.source.id, entrypoint);
        assert_eq!(explanation.target.id, main);
        assert!(explanation.summary.contains("references"));
        assert!(
            explanation
                .evidence
                .iter()
                .any(|item| item == "metadata.relation=entrypoint_function")
        );
        assert!(
            explanation
                .evidence
                .iter()
                .any(|item| item == "confidence=syntactic")
        );
    }

    #[test]
    fn explain_edge_supports_edge_index_lookup() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let explanation = explain_edge(
            &graph,
            ExplainEdgeRequest {
                edge_index: Some(1),
                source: None,
                target: None,
                kind: None,
            },
        )
        .unwrap()
        .expect("missing explanation");

        assert_eq!(explanation.edge_index, 1);
        assert_eq!(explanation.edge.kind, EdgeKind::ReadsConfig);
        assert_eq!(explanation.target.label, "settings.toml");
    }

    #[test]
    fn explain_edge_includes_related_insights() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let service = graph.add_node(NodeKind::Function, "service");
        let repository = graph.add_node(NodeKind::Function, "repository");
        graph.add_edge(main, service, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(service, repository, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(repository, main, EdgeKind::Calls, Confidence::Heuristic);

        let explanation = explain_edge(
            &graph,
            ExplainEdgeRequest {
                edge_index: Some(0),
                source: None,
                target: None,
                kind: None,
            },
        )
        .unwrap()
        .expect("missing explanation");

        assert_eq!(explanation.total_insights, 1);
        assert_eq!(
            explanation.insight_summary.by_severity.get("warning"),
            Some(&1)
        );
        assert_eq!(
            explanation.insight_summary.by_kind.get("dependency_cycle"),
            Some(&1)
        );
        assert_eq!(explanation.insights[0].kind, "dependency_cycle");
        assert!(!explanation.truncated_insights);
    }

    #[test]
    fn query_trace_returns_focused_subgraph() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let dependency = graph.add_node(NodeKind::ExternalDependency, "serde");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, dependency, EdgeKind::DependsOn, Confidence::Exact);

        let result = query_graph(&graph, "trace label:main depth:2").unwrap();

        assert_eq!(result.total_nodes, 3);
        assert_eq!(result.total_edges, 2);
        assert!(result.nodes.iter().any(|node| node.label == "serde"));
    }

    #[test]
    fn query_dependents_returns_reverse_dependency_subgraph() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let result = query_graph(&graph, "dependents label:settings.toml depth:2").unwrap();

        assert_eq!(result.total_nodes, 3);
        assert_eq!(result.total_edges, 2);
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == helper && edge.target == config)
        );
    }

    #[test]
    fn query_path_returns_shortest_dependency_path() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let service = graph.add_node(NodeKind::Function, "service");
        let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        let unrelated = graph.add_node(NodeKind::Function, "unrelated");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, service, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            service,
            database_url,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            unrelated,
            database_url,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );

        let result = query_graph(&graph, "path from:main to:DATABASE_URL depth:4").unwrap();

        assert_eq!(result.total_nodes, 4);
        assert_eq!(result.total_edges, 3);
        assert_eq!(
            result
                .nodes
                .iter()
                .map(|node| node.label.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "helper", "service", "DATABASE_URL"]
        );
        assert_eq!(result.edges[0].source, main);
        assert_eq!(result.edges[2].target, database_url);
    }

    #[test]
    fn query_path_respects_depth_and_edge_kind() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let limited = query_graph(&graph, "path from:main to:settings.toml depth:1").unwrap();
        assert!(limited.nodes.is_empty());
        assert!(limited.truncated);

        let calls_only = query_graph(
            &graph,
            "path from:main to:settings.toml depth:3 edge_kind:calls",
        )
        .unwrap();
        assert!(calls_only.nodes.is_empty());
        assert!(!calls_only.truncated);
    }

    #[test]
    fn query_neighbors_returns_directional_neighborhoods() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let service = graph.add_node(NodeKind::Function, "service");
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        let caller = graph.add_node(NodeKind::Function, "caller");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, service, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);
        graph.add_edge(caller, main, EdgeKind::Calls, Confidence::Heuristic);

        let outgoing = query_graph(
            &graph,
            "neighbors label:main direction:out depth:2 edge_kind:calls",
        )
        .unwrap();
        assert_eq!(outgoing.total_edges, 2);
        assert!(outgoing.nodes.iter().any(|node| node.id == main));
        assert!(outgoing.nodes.iter().any(|node| node.id == helper));
        assert!(outgoing.nodes.iter().any(|node| node.id == service));
        assert!(!outgoing.nodes.iter().any(|node| node.id == config));
        assert!(!outgoing.nodes.iter().any(|node| node.id == caller));

        let incoming = query_graph(&graph, "neighbors main direction:in").unwrap();
        assert_eq!(incoming.total_edges, 1);
        assert!(incoming.nodes.iter().any(|node| node.id == caller));
        assert!(!incoming.nodes.iter().any(|node| node.id == helper));
    }

    #[test]
    fn query_symbols_returns_file_and_dependency_context() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/config.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let load_config = graph.add_node_with_metadata(
            NodeKind::Function,
            "load_config",
            None,
            BTreeMap::from([
                ("language".to_string(), "rust".to_string()),
                ("item_kind".to_string(), "function".to_string()),
            ]),
        );
        let helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "parse_config",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let caller = graph.add_node(NodeKind::Function, "main");
        let unrelated = graph.add_node(NodeKind::Function, "render");
        graph.add_edge(file, load_config, EdgeKind::Contains, Confidence::Syntactic);
        graph.add_edge(load_config, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(caller, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(caller, unrelated, EdgeKind::Calls, Confidence::Heuristic);

        let result = query_graph(&graph, "symbols load_config direction:out").unwrap();

        assert!(result.nodes.iter().any(|node| node.id == file));
        assert!(result.nodes.iter().any(|node| node.id == load_config));
        assert!(result.nodes.iter().any(|node| node.id == helper));
        assert!(!result.nodes.iter().any(|node| node.id == caller));
        assert!(!result.nodes.iter().any(|node| node.id == unrelated));
        assert!(result.edges.iter().any(|edge| {
            edge.source == file && edge.target == load_config && edge.kind == EdgeKind::Contains
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == load_config && edge.target == helper && edge.kind == EdgeKind::Calls
        }));
        assert_eq!(result.facets.node_kinds.get("function"), Some(&2));
        assert_eq!(result.facets.edge_kinds.get("calls"), Some(&1));

        let by_path = query_graph(&graph, "symbols path:src/config.rs").unwrap();
        assert!(by_path.nodes.iter().any(|node| node.id == load_config));

        let error = query_graph(&graph, "symbols nope:value")
            .expect_err("invalid symbols term should fail");
        assert!(error.to_string().contains("unsupported symbols query term"));
    }

    #[test]
    fn query_files_returns_structure_and_symbol_context() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/config.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let load_config = graph.add_node_with_metadata(
            NodeKind::Function,
            "load_config",
            None,
            BTreeMap::from([
                ("language".to_string(), "rust".to_string()),
                ("item_kind".to_string(), "function".to_string()),
            ]),
        );
        let helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "parse_config",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let import = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "serde::Deserialize",
            None,
            BTreeMap::from([
                ("language".to_string(), "rust".to_string()),
                ("item_kind".to_string(), "import".to_string()),
            ]),
        );
        let env = graph.add_node_with_metadata(
            NodeKind::Environment,
            "DATABASE_URL",
            None,
            BTreeMap::from([("item_kind".to_string(), "environment".to_string())]),
        );
        let caller = graph.add_node(NodeKind::Function, "main");
        let unrelated_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/render.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let unrelated = graph.add_node(NodeKind::Function, "render");
        graph.add_edge(file, load_config, EdgeKind::Contains, Confidence::Syntactic);
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(load_config, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            env,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(caller, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            unrelated_file,
            unrelated,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );

        let result = query_graph(
            &graph,
            "files path:src/config.rs direction:out edge_limit:20",
        )
        .unwrap();

        assert!(result.nodes.iter().any(|node| node.id == file));
        assert!(result.nodes.iter().any(|node| node.id == load_config));
        assert!(result.nodes.iter().any(|node| node.id == helper));
        assert!(result.nodes.iter().any(|node| node.id == import));
        assert!(result.nodes.iter().any(|node| node.id == env));
        assert!(!result.nodes.iter().any(|node| node.id == caller));
        assert!(!result.nodes.iter().any(|node| node.id == unrelated_file));
        assert!(!result.nodes.iter().any(|node| node.id == unrelated));
        assert!(result.edges.iter().any(|edge| {
            edge.source == file && edge.target == load_config && edge.kind == EdgeKind::Contains
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == file && edge.target == import && edge.kind == EdgeKind::Imports
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == load_config && edge.target == helper && edge.kind == EdgeKind::Calls
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == load_config
                && edge.target == env
                && edge.kind == EdgeKind::ReadsEnvironment
        }));
        assert_eq!(result.facets.node_kinds.get("file"), Some(&1));
        assert_eq!(result.facets.edge_kinds.get("contains"), Some(&1));
        assert_eq!(result.facets.edge_kinds.get("reads_environment"), Some(&1));

        let incoming = query_graph(&graph, "files path:src/config.rs direction:in").unwrap();
        assert!(incoming.nodes.iter().any(|node| node.id == caller));
        assert!(incoming.edges.iter().any(|edge| {
            edge.source == caller && edge.target == load_config && edge.kind == EdgeKind::Calls
        }));

        let by_language = query_graph(&graph, "files language:rust edge_limit:20").unwrap();
        assert!(by_language.nodes.iter().any(|node| node.id == file));

        let error =
            query_graph(&graph, "files nope:value").expect_err("invalid files term should fail");
        assert!(error.to_string().contains("unsupported files query term"));
    }

    #[test]
    fn query_documents_returns_sections_and_code_references() {
        let mut graph = CodeGraph::new("repo");
        let doc = graph.add_node_with_metadata(
            NodeKind::File,
            "docs/adr/0001-runtime.md",
            None,
            BTreeMap::from([
                ("language".to_string(), "markdown".to_string()),
                ("item_kind".to_string(), "document".to_string()),
                ("document_kind".to_string(), "adr".to_string()),
                ("source".to_string(), "markdown".to_string()),
            ]),
        );
        let section = graph.add_node_with_metadata(
            NodeKind::Module,
            "docs/adr/0001-runtime.md#Runtime Flow",
            Some(SourceSpan {
                path: "docs/adr/0001-runtime.md".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 15,
            }),
            BTreeMap::from([
                ("language".to_string(), "markdown".to_string()),
                ("item_kind".to_string(), "document_section".to_string()),
                ("document_kind".to_string(), "adr".to_string()),
                ("heading".to_string(), "Runtime Flow".to_string()),
                ("anchor".to_string(), "runtime-flow".to_string()),
                ("source".to_string(), "markdown".to_string()),
            ]),
        );
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let function = graph.add_node_with_metadata(
            NodeKind::Function,
            "load_config",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let other_doc = graph.add_node_with_metadata(
            NodeKind::File,
            "docs/readme.md",
            None,
            BTreeMap::from([
                ("language".to_string(), "markdown".to_string()),
                ("item_kind".to_string(), "document".to_string()),
                ("document_kind".to_string(), "markdown".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            doc,
            section,
            EdgeKind::Contains,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "document_section".to_string())]),
        );
        graph.add_edge(file, function, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge_with_metadata(
            section,
            file,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([
                ("relation".to_string(), "markdown_link".to_string()),
                ("source".to_string(), "markdown".to_string()),
                ("target".to_string(), "../../src/main.rs".to_string()),
                ("resolved_path".to_string(), "src/main.rs".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            section,
            function,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                (
                    "relation".to_string(),
                    "markdown_symbol_reference".to_string(),
                ),
                ("source".to_string(), "markdown".to_string()),
                ("symbol".to_string(), "load_config".to_string()),
            ]),
        );

        let result = query_graph(&graph, "docs document_kind:adr target:src/main.rs").unwrap();

        assert!(result.nodes.iter().any(|node| node.id == section));
        assert!(result.nodes.iter().any(|node| node.id == file));
        assert!(result.nodes.iter().any(|node| node.id == doc));
        assert!(!result.nodes.iter().any(|node| node.id == other_doc));
        assert!(result.edges.iter().any(|edge| {
            edge.source == doc
                && edge.target == section
                && edge.kind == EdgeKind::Contains
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "document_section")
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == section
                && edge.target == file
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|value| value == "markdown_link")
        }));
        assert_eq!(result.facets.languages.get("markdown"), Some(&2));
        assert_eq!(result.facets.languages.get("rust"), Some(&1));
        assert_eq!(result.facets.item_kinds.get("document_section"), Some(&1));

        let by_alias = query_graph(&graph, "adr heading:Runtime edge_limit:20").unwrap();
        assert!(by_alias.nodes.iter().any(|node| node.id == section));
        assert!(by_alias.edges.iter().any(|edge| edge.target == function));

        let error = query_graph(&graph, "docs unsupported:value")
            .expect_err("invalid docs term should fail");
        assert!(error.to_string().contains("unsupported docs query term"));
    }

    #[test]
    fn query_sql_returns_schema_and_source_query_context() {
        let mut graph = CodeGraph::new("repo");
        let schema = graph.add_node_with_metadata(
            NodeKind::File,
            "db/schema.sql",
            None,
            BTreeMap::from([
                ("language".to_string(), "sql".to_string()),
                ("item_kind".to_string(), "sql_schema".to_string()),
            ]),
        );
        let users = graph.add_node_with_metadata(
            NodeKind::Type,
            "sql table:users",
            None,
            BTreeMap::from([
                ("language".to_string(), "sql".to_string()),
                ("item_kind".to_string(), "sql_table".to_string()),
                ("table_name".to_string(), "users".to_string()),
                ("table_key".to_string(), "users".to_string()),
            ]),
        );
        let user_id = graph.add_node_with_metadata(
            NodeKind::Config,
            "sql column:users.id",
            None,
            BTreeMap::from([
                ("language".to_string(), "sql".to_string()),
                ("item_kind".to_string(), "sql_column".to_string()),
                ("table_name".to_string(), "users".to_string()),
                ("column_name".to_string(), "id".to_string()),
            ]),
        );
        let rust_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/repo.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let load_users = graph.add_node_with_metadata(
            NodeKind::Function,
            "load_users",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let query = graph.add_node_with_metadata(
            NodeKind::Config,
            "sql query:src/repo.rs:4",
            None,
            BTreeMap::from([
                ("language".to_string(), "rust".to_string()),
                ("item_kind".to_string(), "app_sql_query".to_string()),
                ("operation".to_string(), "select".to_string()),
                ("tables".to_string(), "audit_log,users".to_string()),
                ("unresolved_tables".to_string(), "audit_log".to_string()),
                ("resolution".to_string(), "partial".to_string()),
            ]),
        );

        graph.add_edge_with_metadata(
            schema,
            users,
            EdgeKind::Contains,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "sql_table".to_string())]),
        );
        graph.add_edge_with_metadata(
            users,
            user_id,
            EdgeKind::Contains,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "sql_column".to_string())]),
        );
        graph.add_edge(rust_file, load_users, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge_with_metadata(
            load_users,
            query,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([("relation".to_string(), "app_sql_query".to_string())]),
        );
        graph.add_edge_with_metadata(
            query,
            users,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                (
                    "relation".to_string(),
                    "app_sql_table_reference".to_string(),
                ),
                ("operation".to_string(), "select".to_string()),
                ("table".to_string(), "users".to_string()),
            ]),
        );

        let result = query_graph(&graph, "sql table:users edge_limit:20").unwrap();

        assert!(result.nodes.iter().any(|node| node.id == users));
        assert!(result.nodes.iter().any(|node| node.id == user_id));
        assert!(result.nodes.iter().any(|node| node.id == schema));
        assert!(result.nodes.iter().any(|node| node.id == query));
        assert!(result.nodes.iter().any(|node| node.id == load_users));
        assert!(!result.nodes.iter().any(|node| node.id == rust_file));
        assert!(result.edges.iter().any(|edge| {
            edge.source == schema
                && edge.target == users
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "sql_table")
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == users
                && edge.target == user_id
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "sql_column")
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == query
                && edge.target == users
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "app_sql_table_reference")
        }));
        assert_eq!(result.facets.item_kinds.get("sql_table"), Some(&1));
        assert_eq!(result.facets.item_kinds.get("app_sql_query"), Some(&1));

        let unresolved = query_graph(&graph, "sql unresolved:true").unwrap();
        assert!(unresolved.nodes.iter().any(|node| node.id == query));
        assert!(unresolved.nodes.iter().any(|node| node.id == users));
        assert!(unresolved.total_nodes >= 2);

        let by_operation = query_graph(&graph, "database operation:select").unwrap();
        assert!(by_operation.nodes.iter().any(|node| node.id == query));

        let error =
            query_graph(&graph, "sql unsupported:value").expect_err("invalid sql term should fail");
        assert!(error.to_string().contains("unsupported sql query term"));

        let card = node_card(&graph, None, query, 10, 1, 10)
            .expect("SQL query card should not error")
            .expect("expected SQL query card");
        assert!(card.actions.iter().any(|action| {
            action.kind == "sql_graph"
                && action.query == format!("sql node_id:{} edge_limit:300", query.0)
        }));
    }

    #[test]
    fn query_packages_returns_manifest_and_import_context() {
        let mut graph = CodeGraph::new("repo");
        let cargo_manifest = graph.add_node_with_metadata(
            NodeKind::File,
            "Cargo.toml",
            None,
            BTreeMap::from([("language".to_string(), "toml".to_string())]),
        );
        let rust_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let js_file = graph.add_node_with_metadata(
            NodeKind::File,
            "web/app.js",
            None,
            BTreeMap::from([("language".to_string(), "javascript".to_string())]),
        );
        let serde_dependency = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "serde",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "dependency".to_string()),
                ("ecosystem".to_string(), "cargo".to_string()),
                ("package_id".to_string(), "cargo:serde".to_string()),
                ("source".to_string(), "manifest".to_string()),
            ]),
        );
        let serde_import = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "use serde::Deserialize;",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "rust".to_string()),
            ]),
        );
        let express_import = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "import express from 'express';",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "javascript".to_string()),
            ]),
        );
        let serde_json_dependency = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "serde_json",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "dependency".to_string()),
                ("ecosystem".to_string(), "cargo".to_string()),
                ("package_id".to_string(), "cargo:serde_json".to_string()),
                ("source".to_string(), "manifest".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            cargo_manifest,
            serde_dependency,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "1".to_string()),
                ("source".to_string(), "manifest".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            cargo_manifest,
            serde_json_dependency,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "1".to_string()),
                ("source".to_string(), "manifest".to_string()),
            ]),
        );
        graph.add_edge(
            rust_file,
            serde_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            js_file,
            express_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );

        let result = query_graph(&graph, "packages serde ecosystem:cargo").unwrap();

        assert_eq!(result.returned_nodes, result.nodes.len());
        assert_eq!(result.returned_edges, result.edges.len());
        assert_eq!(
            result.facets.node_kinds.get("external_dependency"),
            Some(&2)
        );
        assert_eq!(result.facets.node_kinds.get("file"), Some(&2));
        assert_eq!(result.facets.edge_kinds.get("depends_on"), Some(&1));
        assert_eq!(result.facets.edge_kinds.get("imports"), Some(&1));
        assert_eq!(result.facets.languages.get("rust"), Some(&2));
        assert!(result.nodes.iter().any(|node| node.id == cargo_manifest));
        assert!(result.nodes.iter().any(|node| node.id == rust_file));
        assert!(result.nodes.iter().any(|node| node.id == serde_dependency));
        assert!(result.nodes.iter().any(|node| node.id == serde_import));
        assert!(
            !result
                .nodes
                .iter()
                .any(|node| node.id == serde_json_dependency)
        );
        assert!(!result.nodes.iter().any(|node| node.id == express_import));
        assert!(result.edges.iter().any(|edge| {
            edge.source == cargo_manifest
                && edge.target == serde_dependency
                && edge.kind == EdgeKind::DependsOn
        }));
        assert!(result.edges.iter().any(|edge| {
            edge.source == rust_file
                && edge.target == serde_import
                && edge.kind == EdgeKind::Imports
        }));

        let path_limited = query_graph(&graph, "packages package:serde path:src").unwrap();
        assert_eq!(path_limited.total_edges, 1);
        assert!(
            path_limited
                .edges
                .iter()
                .all(|edge| edge.kind == EdgeKind::Imports)
        );

        let error = query_graph(&graph, "packages unsupported:value")
            .expect_err("invalid packages term should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported packages query term")
        );
    }

    #[test]
    fn query_unreachable_returns_source_file_focus() {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let live_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let live_main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let legacy_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/legacy.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let legacy_fn = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_worker",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let test_file = graph.add_node_with_metadata(
            NodeKind::File,
            "tests/legacy_test.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let test_fn = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_test",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            live_file,
            live_main,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        graph.add_edge(entry, live_main, EdgeKind::References, Confidence::Exact);
        graph.add_edge(
            legacy_file,
            legacy_fn,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        graph.add_edge(
            test_file,
            test_fn,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );

        let result = query_graph(&graph, "unreachable language:rust").unwrap();

        assert!(result.nodes.iter().any(|node| node.id == legacy_file));
        assert!(result.nodes.iter().any(|node| node.id == legacy_fn));
        assert!(result.edges.iter().any(|edge| {
            edge.source == legacy_file
                && edge.target == legacy_fn
                && edge.kind == EdgeKind::Contains
        }));
        assert!(!result.nodes.iter().any(|node| node.id == live_file));
        assert!(!result.nodes.iter().any(|node| node.id == live_main));
        assert!(!result.nodes.iter().any(|node| node.id == test_file));
    }

    #[test]
    fn query_unreachable_supports_general_node_scope() {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let unused = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_worker",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);

        let result = query_graph(&graph, "unreachable kind:function label:legacy_worker").unwrap();

        assert_eq!(result.total_nodes, 1);
        assert_eq!(result.nodes[0].id, unused);
        assert!(result.edges.is_empty());

        let error =
            query_graph(&graph, "unreachable scope:maybe").expect_err("invalid scope should fail");
        assert!(error.to_string().contains("invalid unreachable scope"));
    }

    #[test]
    fn query_unreachable_returns_config_and_error_flow_scopes() {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let live_env = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        let live_error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "panic",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        let legacy_loader = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_loader",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let legacy_env = graph.add_node(NodeKind::Environment, "LEGACY_TOKEN");
        let legacy_worker = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_worker",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let legacy_error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "LegacyError",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
        graph.add_edge(
            main,
            live_env,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(main, live_error, EdgeKind::MayError, Confidence::Heuristic);
        graph.add_edge(
            legacy_loader,
            legacy_env,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            legacy_worker,
            legacy_error,
            EdgeKind::MayError,
            Confidence::Heuristic,
        );

        let configs = query_graph(&graph, "unreachable scope:config search:LEGACY_TOKEN").unwrap();
        assert!(configs.nodes.iter().any(|node| node.id == legacy_loader));
        assert!(configs.nodes.iter().any(|node| node.id == legacy_env));
        assert!(configs.edges.iter().any(|edge| {
            edge.source == legacy_loader
                && edge.target == legacy_env
                && edge.kind == EdgeKind::ReadsEnvironment
        }));
        assert!(!configs.nodes.iter().any(|node| node.id == main));
        assert!(!configs.nodes.iter().any(|node| node.id == live_env));

        let errors = query_graph(&graph, "unreachable scope:errors search:LegacyError").unwrap();
        assert!(errors.nodes.iter().any(|node| node.id == legacy_worker));
        assert!(errors.nodes.iter().any(|node| node.id == legacy_error));
        assert!(errors.edges.iter().any(|edge| {
            edge.source == legacy_worker
                && edge.target == legacy_error
                && edge.kind == EdgeKind::MayError
        }));
        assert!(!errors.nodes.iter().any(|node| node.id == main));
        assert!(!errors.nodes.iter().any(|node| node.id == live_error));
    }

    #[test]
    fn query_diagnostics_returns_diagnostic_context() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let diagnostic = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "error: semantic mismatch",
            Some(SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 3,
                start_column: 9,
                end_line: 3,
                end_column: 10,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "diagnostic".to_string()),
                ("source".to_string(), "lsp".to_string()),
                ("severity".to_string(), "error".to_string()),
                ("diagnostic_source".to_string(), "rustc".to_string()),
                ("diagnostic_code".to_string(), "E0001".to_string()),
                ("message".to_string(), "semantic mismatch".to_string()),
                ("path".to_string(), "src/main.rs".to_string()),
            ]),
        );
        let warning = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "warning: style issue",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "diagnostic".to_string()),
                ("source".to_string(), "lsp".to_string()),
                ("severity".to_string(), "warning".to_string()),
                ("diagnostic_source".to_string(), "rustc".to_string()),
                ("message".to_string(), "style issue".to_string()),
                ("path".to_string(), "src/main.rs".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            file,
            diagnostic,
            EdgeKind::MayError,
            Confidence::Semantic,
            BTreeMap::from([("relation".to_string(), "diagnostic".to_string())]),
        );
        graph.add_edge_with_metadata(
            file,
            warning,
            EdgeKind::MayError,
            Confidence::Semantic,
            BTreeMap::from([("relation".to_string(), "diagnostic".to_string())]),
        );

        let result = query_graph(
            &graph,
            "diagnostics severity:error language:rust code:E0001",
        )
        .unwrap();

        assert_eq!(result.total_edges, 1);
        assert!(result.nodes.iter().any(|node| node.id == file));
        assert!(result.nodes.iter().any(|node| node.id == diagnostic));
        assert!(!result.nodes.iter().any(|node| node.id == warning));
        assert_eq!(result.edges[0].source, file);
        assert_eq!(result.edges[0].target, diagnostic);

        let error = query_graph(&graph, "diagnostics nope:value")
            .expect_err("invalid diagnostics term should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported diagnostics query term")
        );
    }

    #[test]
    fn query_insights_returns_risk_context() {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let orphan = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_worker",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);

        let result =
            query_graph(&graph, "insights severity:info kind:orphan language:rust").unwrap();

        assert_eq!(result.total_nodes, 1);
        assert!(result.nodes.iter().any(|node| node.id == orphan));
        assert!(result.edges.is_empty());

        let by_node = query_graph(&graph, "risks node:legacy_worker").unwrap();
        assert!(by_node.nodes.iter().any(|node| node.id == orphan));

        let error = query_graph(&graph, "insights nope:value")
            .expect_err("invalid insights term should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported insights query term")
        );
    }

    #[test]
    fn query_insights_returns_sensitive_config_default_context() {
        let mut graph = CodeGraph::new("repo");
        let load_config = graph.add_node_with_metadata(
            NodeKind::Function,
            "load_config",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let token = graph.add_node_with_metadata(
            NodeKind::Environment,
            "API_TOKEN",
            None,
            BTreeMap::from([("default_value".to_string(), "local-token".to_string())]),
        );
        graph.add_edge(
            load_config,
            token,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );

        let result = query_graph(&graph, "insights kind:sensitive_config_default").unwrap();

        assert_eq!(result.total_nodes, 2);
        assert_eq!(result.total_edges, 1);
        assert!(result.nodes.iter().any(|node| node.id == load_config));
        assert!(result.nodes.iter().any(|node| node.id == token));
        assert_eq!(result.edges[0].source, load_config);
        assert_eq!(result.edges[0].target, token);
    }

    #[test]
    fn query_entrypoints_returns_start_context() {
        let mut graph = CodeGraph::new("repo");
        let cargo = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:api",
            None,
            BTreeMap::from([
                ("language".to_string(), "rust".to_string()),
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
                ("entrypoint_kind".to_string(), "binary".to_string()),
            ]),
        );
        let npm = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "npm script:start",
            None,
            BTreeMap::from([
                ("language".to_string(), "javascript".to_string()),
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ]),
        );
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_edge(graph.root, cargo, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(graph.root, npm, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge_with_metadata(
            cargo,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );

        let result = query_graph(&graph, "entrypoints language:rust").unwrap();

        assert!(result.nodes.iter().any(|node| node.id == cargo));
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(!result.nodes.iter().any(|node| node.id == npm));
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == graph.root && edge.target == cargo)
        );
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == cargo && edge.target == main)
        );

        let by_search = query_graph(&graph, "starts api").unwrap();
        assert!(by_search.nodes.iter().any(|node| node.id == cargo));

        let error = query_graph(&graph, "entrypoints nope:value")
            .expect_err("invalid entrypoints term should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported entrypoints query term")
        );
    }

    #[test]
    fn query_routes_returns_route_handler_context() {
        let mut graph = CodeGraph::new("repo");
        let route = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route GET /users",
            Some(SourceSpan {
                path: "src/server.js".to_string(),
                start_line: 7,
                start_column: 1,
                end_line: 7,
                end_column: 30,
            }),
            BTreeMap::from([
                ("language".to_string(), "javascript".to_string()),
                ("item_kind".to_string(), "framework_route".to_string()),
                ("entrypoint_kind".to_string(), "route".to_string()),
                ("framework".to_string(), "express".to_string()),
                ("method".to_string(), "GET".to_string()),
                ("path".to_string(), "/users".to_string()),
                ("handler".to_string(), "listUsers".to_string()),
            ]),
        );
        let other_route = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route POST /users",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("framework".to_string(), "express".to_string()),
                ("method".to_string(), "POST".to_string()),
                ("path".to_string(), "/users".to_string()),
                ("handler".to_string(), "createUser".to_string()),
            ]),
        );
        let file = graph.add_node(NodeKind::File, "src/server.js");
        let express_import = graph.add_node(NodeKind::ExternalDependency, "express");
        let handler = graph.add_node(NodeKind::Function, "listUsers");
        let load_config = graph.add_node(NodeKind::Function, "loadConfig");
        let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        graph.add_edge(graph.root, route, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            graph.root,
            other_route,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge_with_metadata(
            route,
            file,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "framework_route_file".to_string())]),
        );
        graph.add_edge(
            file,
            express_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge_with_metadata(
            route,
            handler,
            EdgeKind::References,
            Confidence::Syntactic,
            BTreeMap::from([(
                "resolution".to_string(),
                "framework_route_handler".to_string(),
            )]),
        );
        graph.add_edge(handler, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            database_url,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );

        let result = query_graph(
            &graph,
            "routes method:GET path:/users framework:express depth:3",
        )
        .unwrap();

        assert!(result.nodes.iter().any(|node| node.id == route));
        assert!(result.nodes.iter().any(|node| node.id == file));
        assert!(!result.nodes.iter().any(|node| node.id == express_import));
        assert!(result.nodes.iter().any(|node| node.id == handler));
        assert!(result.nodes.iter().any(|node| node.id == database_url));
        assert!(!result.nodes.iter().any(|node| node.id == other_route));
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == graph.root && edge.target == route)
        );
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == route && edge.target == handler)
        );

        let by_handler = query_graph(&graph, "endpoint handler:listUsers").unwrap();
        assert!(by_handler.nodes.iter().any(|node| node.id == route));

        let by_source = query_graph(&graph, "routes source_path:src/server.js").unwrap();
        assert!(by_source.nodes.iter().any(|node| node.id == route));

        let error =
            query_graph(&graph, "routes nope:value").expect_err("invalid routes term should fail");
        assert!(error.to_string().contains("unsupported routes query term"));
    }

    #[test]
    fn query_configs_returns_reader_and_entrypoint_context() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let load_config = graph.add_node(NodeKind::Function, "load_config");
        let database_url = graph.add_node_with_metadata(
            NodeKind::Environment,
            "DATABASE_URL",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper = graph.add_node(NodeKind::Function, "helper");
        let settings = graph.add_node(NodeKind::Config, "config/app.toml");
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            database_url,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            helper,
            settings,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );

        let result = query_graph(&graph, "configs target:DATABASE depth:4").unwrap();

        assert!(result.nodes.iter().any(|node| node.id == entrypoint));
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(result.nodes.iter().any(|node| node.id == load_config));
        assert!(result.nodes.iter().any(|node| node.id == database_url));
        assert!(!result.nodes.iter().any(|node| node.id == settings));
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == load_config && edge.target == database_url)
        );

        let all_configs = query_graph(&graph, "config").unwrap();
        assert!(all_configs.nodes.iter().any(|node| node.id == settings));
        assert!(
            all_configs
                .edges
                .iter()
                .any(|edge| edge.source == helper && edge.target == settings)
        );

        let by_search = query_graph(&graph, "env DATABASE").unwrap();
        assert!(by_search.nodes.iter().any(|node| node.id == database_url));

        let error = query_graph(&graph, "configs nope:value")
            .expect_err("invalid configs term should fail");
        assert!(error.to_string().contains("unsupported configs query term"));
    }

    #[test]
    fn natural_query_maps_config_question_to_bounded_query() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let load_config = graph.add_node(NodeKind::Function, "load_config");
        let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            database_url,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );

        let report = natural_query(
            &graph,
            NaturalQueryRequest {
                question: "Where is DATABASE_URL read from the environment?".to_string(),
                compact: false,
            },
        )
        .unwrap();

        assert_eq!(
            report.generated_query,
            "configs target:DATABASE_URL depth:6"
        );
        assert_eq!(report.rule, "config_or_environment");
        assert_eq!(report.confidence, "high");
        assert!(report.result.nodes.iter().any(|node| node.id == entrypoint));
        assert!(
            report
                .result
                .nodes
                .iter()
                .any(|node| node.id == load_config)
        );
        assert!(
            report
                .result
                .nodes
                .iter()
                .any(|node| node.id == database_url)
        );
    }

    #[test]
    fn natural_query_supports_russian_call_questions() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let worker = graph.add_node(NodeKind::Function, "worker");
        let load_config = graph.add_node(NodeKind::Function, "load_config");
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(worker, load_config, EdgeKind::Calls, Confidence::Heuristic);

        let report = natural_query(
            &graph,
            NaturalQueryRequest {
                question: "Кто вызывает load_config?".to_string(),
                compact: false,
            },
        )
        .unwrap();

        assert_eq!(
            report.generated_query,
            "neighbors label:load_config direction:in depth:2 edge_kind:calls"
        );
        assert_eq!(report.rule, "call_neighborhood");
        assert!(!report.result.compact);
        assert!(report.result.nodes.iter().any(|node| node.id == main));
        assert!(report.result.nodes.iter().any(|node| node.id == worker));
        assert!(
            report
                .result
                .nodes
                .iter()
                .any(|node| node.id == load_config)
        );
    }

    #[test]
    fn query_errors_returns_source_and_entrypoint_context() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "npm script:start");
        let main = graph.add_node(NodeKind::Function, "main");
        let load_data = graph.add_node(NodeKind::Function, "loadData");
        let error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "failed to load data",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        let helper = graph.add_node(NodeKind::Function, "helper");
        let panic = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "panic",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );
        graph.add_edge(main, load_data, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(load_data, error, EdgeKind::MayError, Confidence::Heuristic);
        graph.add_edge(helper, panic, EdgeKind::MayError, Confidence::Heuristic);

        let result = query_graph(&graph, "errors target:load depth:4").unwrap();

        assert!(result.nodes.iter().any(|node| node.id == entrypoint));
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(result.nodes.iter().any(|node| node.id == load_data));
        assert!(result.nodes.iter().any(|node| node.id == error));
        assert!(!result.nodes.iter().any(|node| node.id == panic));
        assert!(
            result
                .edges
                .iter()
                .any(|edge| edge.source == load_data && edge.target == error)
        );

        let all_errors = query_graph(&graph, "errors").unwrap();
        assert!(all_errors.nodes.iter().any(|node| node.id == panic));
        assert!(
            all_errors
                .edges
                .iter()
                .any(|edge| edge.source == helper && edge.target == panic)
        );

        let by_search = query_graph(&graph, "exceptions panic").unwrap();
        assert!(by_search.nodes.iter().any(|node| node.id == panic));

        let error_query =
            query_graph(&graph, "errors nope:value").expect_err("invalid errors term should fail");
        assert!(
            error_query
                .to_string()
                .contains("unsupported errors query term")
        );
    }

    #[test]
    fn query_cycles_returns_dependency_cycle_context() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let service = graph.add_node_with_metadata(
            NodeKind::Function,
            "service",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let repository = graph.add_node_with_metadata(
            NodeKind::Function,
            "repository",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper = graph.add_node(NodeKind::Function, "helper");
        graph.add_edge(main, service, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(service, repository, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(repository, main, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, service, EdgeKind::Calls, Confidence::Heuristic);

        let result = query_graph(&graph, "cycles language:rust").unwrap();

        assert_eq!(result.total_nodes, 3);
        assert_eq!(result.total_edges, 3);
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(result.nodes.iter().any(|node| node.id == service));
        assert!(result.nodes.iter().any(|node| node.id == repository));
        assert!(!result.nodes.iter().any(|node| node.id == helper));
        assert!(result.edges.iter().all(|edge| edge.kind == EdgeKind::Calls));

        let by_label = query_graph(&graph, "cycle label:repository").unwrap();
        assert!(by_label.nodes.iter().any(|node| node.id == repository));

        let by_edge_kind = query_graph(&graph, "cycles edge_kind:calls").unwrap();
        assert_eq!(by_edge_kind.total_edges, 3);

        let error =
            query_graph(&graph, "cycles nope:value").expect_err("invalid cycles term should fail");
        assert!(error.to_string().contains("unsupported cycles query term"));
    }

    #[test]
    fn query_hotspots_returns_high_degree_context() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let python_worker = graph.add_node_with_metadata(
            NodeKind::Function,
            "worker",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, main, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(python_worker, main, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let result = query_graph(
            &graph,
            "hotspots language:rust min_score:3 limit:1 edge_limit:3",
        )
        .unwrap();

        assert!(result.truncated);
        assert_eq!(result.total_edges, 4);
        assert_eq!(result.edges.len(), 3);
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(result.nodes.iter().any(|node| node.id == python_worker));

        let incoming = query_graph(&graph, "hotspots label:main direction:in").unwrap();
        assert_eq!(incoming.total_edges, 2);
        assert!(incoming.edges.iter().all(|edge| edge.target == main));

        let by_edge_kind = query_graph(&graph, "hotspots edge_kind:reads_config").unwrap();
        assert_eq!(by_edge_kind.total_edges, 1);
        assert!(
            by_edge_kind
                .edges
                .iter()
                .all(|edge| edge.kind == EdgeKind::ReadsConfig)
        );

        let error = query_graph(&graph, "hotspots nope:value")
            .expect_err("invalid hotspots term should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported hotspots query term")
        );
    }

    #[test]
    fn compact_query_result_collapses_low_signal_nodes() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper_a = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper_a",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper_b = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper_b",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let config = graph.add_node(NodeKind::Config, "DATABASE_URL");
        graph.add_edge(main, helper_a, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, helper_b, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let result = query_graph(&graph, "neighbors main direction:out").unwrap();
        let compacted = compact_query_result(result);

        assert!(compacted.compact);
        assert_eq!(compacted.raw_total_nodes, 4);
        assert_eq!(compacted.raw_total_edges, 3);
        assert_eq!(compacted.total_nodes, 3);
        assert_eq!(compacted.total_edges, 2);
        assert_eq!(compacted.compacted_nodes, 2);
        assert_eq!(compacted.compacted_edges, 1);
        assert!(compacted.nodes.iter().any(|node| node.id == config));
        let aggregate = compacted
            .nodes
            .iter()
            .find(|node| {
                node.metadata
                    .get("compacted")
                    .is_some_and(|value| value == "true")
            })
            .expect("compacted aggregate");
        assert_eq!(
            aggregate.metadata.get("compacted_count"),
            Some(&"2".to_string())
        );
        assert!(aggregate.label.contains("2 compacted rust function nodes"));
        assert!(compacted.edges.iter().any(|edge| {
            edge.metadata
                .get("compacted")
                .is_some_and(|value| value == "true")
        }));
    }

    #[test]
    fn trace_config_returns_readers_and_entrypoint_paths() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let load_config = graph.add_node(NodeKind::Function, "load_config");
        let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );
        graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            load_config,
            database_url,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );

        let result = trace_config(
            &graph,
            ConfigTraceRequest {
                target: "DATABASE".to_string(),
                max_depth: 4,
                limit: 10,
            },
        );

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.total_readers, 1);
        assert_eq!(result.total_paths, 1);
        assert!(!result.truncated);
        let matched = &result.matches[0];
        assert_eq!(matched.target.id, database_url);
        assert_eq!(matched.readers[0].node.id, load_config);
        assert_eq!(
            matched.paths[0]
                .nodes
                .iter()
                .map(|node| node.label.as_str())
                .collect::<Vec<_>>(),
            vec!["cargo bin:demo", "main", "load_config", "DATABASE_URL"]
        );
        assert!(matched.paths[0].reached_entrypoint);
    }

    #[test]
    fn trace_config_falls_back_to_direct_reader_path() {
        let mut graph = CodeGraph::new("repo");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let config = graph.add_node(NodeKind::Config, "config/app.toml");
        graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let result = trace_config(
            &graph,
            ConfigTraceRequest {
                target: "app.toml".to_string(),
                max_depth: 2,
                limit: 10,
            },
        );

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.matches[0].paths.len(), 1);
        assert_eq!(
            result.matches[0].paths[0]
                .nodes
                .iter()
                .map(|node| node.label.as_str())
                .collect::<Vec<_>>(),
            vec!["helper", "config/app.toml"]
        );
        assert!(!result.matches[0].paths[0].reached_entrypoint);
    }

    #[test]
    fn trace_errors_returns_sources_and_entrypoint_paths() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "npm script:start");
        let main = graph.add_node(NodeKind::Function, "main");
        let load_data = graph.add_node(NodeKind::Function, "loadData");
        let error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "failed to load data",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );
        graph.add_edge(main, load_data, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(load_data, error, EdgeKind::MayError, Confidence::Heuristic);

        let result = trace_errors(
            &graph,
            ErrorTraceRequest {
                target: "load data".to_string(),
                max_depth: 4,
                limit: 10,
            },
        );

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.total_sources, 1);
        assert_eq!(result.total_paths, 1);
        assert!(!result.truncated);
        let matched = &result.matches[0];
        assert_eq!(matched.error.id, error);
        assert_eq!(matched.sources[0].node.id, load_data);
        assert_eq!(
            matched.paths[0]
                .nodes
                .iter()
                .map(|node| node.label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "npm script:start",
                "main",
                "loadData",
                "failed to load data"
            ]
        );
        assert!(matched.paths[0].reached_entrypoint);
    }

    #[test]
    fn trace_errors_falls_back_to_direct_source_path() {
        let mut graph = CodeGraph::new("repo");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "panic",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        graph.add_edge(helper, error, EdgeKind::MayError, Confidence::Heuristic);

        let result = trace_errors(
            &graph,
            ErrorTraceRequest {
                target: "panic".to_string(),
                max_depth: 2,
                limit: 10,
            },
        );

        assert_eq!(result.total_matches, 1);
        assert_eq!(result.matches[0].paths.len(), 1);
        assert_eq!(
            result.matches[0].paths[0]
                .nodes
                .iter()
                .map(|node| node.label.as_str())
                .collect::<Vec<_>>(),
            vec!["helper", "panic"]
        );
        assert!(!result.matches[0].paths[0].reached_entrypoint);
    }

    #[test]
    fn focus_subgraph_returns_selected_nodes_and_edges() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let result = focus_subgraph(
            &graph,
            FocusRequest {
                node_ids: vec![main],
                edge_indexes: vec![1],
                edge_limit: 10,
            },
        );

        assert_eq!(result.query, "focus");
        assert_eq!(result.total_edges, 1);
        assert_eq!(result.edges[0].source, helper);
        assert_eq!(result.edges[0].target, config);
        assert_eq!(
            result.edges[0].metadata.get("edge_index"),
            Some(&"1".to_string())
        );
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(result.nodes.iter().any(|node| node.id == helper));
        assert!(result.nodes.iter().any(|node| node.id == config));
    }

    #[test]
    fn focus_subgraph_expands_node_only_focus_to_incident_edges() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let unrelated = graph.add_node(NodeKind::Function, "unrelated");
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([
                ("relation".to_string(), "entrypoint_function".to_string()),
                ("source".to_string(), "manifest".to_string()),
            ]),
        );
        graph.add_edge(unrelated, main, EdgeKind::Calls, Confidence::Heuristic);

        let result = focus_subgraph(
            &graph,
            FocusRequest {
                node_ids: vec![entrypoint],
                edge_indexes: Vec::new(),
                edge_limit: 10,
            },
        );

        assert_eq!(result.total_edges, 1);
        assert_eq!(result.edges[0].source, entrypoint);
        assert_eq!(result.edges[0].target, main);
        assert!(result.nodes.iter().any(|node| node.id == entrypoint));
        assert!(result.nodes.iter().any(|node| node.id == main));
        assert!(!result.nodes.iter().any(|node| node.id == unrelated));
    }

    #[test]
    fn graph_slice_filters_and_pages_nodes() {
        let mut graph = CodeGraph::new("repo");
        let mut metadata = BTreeMap::new();
        metadata.insert("language".to_string(), "rust".to_string());
        metadata.insert("item_kind".to_string(), "function".to_string());
        let main = graph.add_node_with_metadata(NodeKind::Function, "main", None, metadata);
        let helper = graph.add_node(NodeKind::Function, "helper");
        let file = graph.add_node(NodeKind::File, "src/main.rs");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(file, main, EdgeKind::Contains, Confidence::Syntactic);

        let result = slice_graph(
            &graph,
            GraphSliceRequest {
                node_offset: 0,
                node_limit: 1,
                edge_offset: 0,
                edge_limit: 10,
                path_prefix: None,
                kind: Some("function".to_string()),
                search: None,
                language: None,
                item_kind: None,
                edge_kind: None,
                confidence: None,
                edge_relation: None,
                edge_source: None,
            },
        );

        assert_eq!(result.total_nodes, 2);
        assert_eq!(result.nodes.len(), 1);
        assert!(result.truncated_nodes);
        assert!(result.edges.is_empty());

        let result = slice_graph(
            &graph,
            GraphSliceRequest {
                node_offset: 0,
                node_limit: 10,
                edge_offset: 0,
                edge_limit: 10,
                path_prefix: None,
                kind: Some("function".to_string()),
                search: Some("rust".to_string()),
                language: Some("rust".to_string()),
                item_kind: Some("function".to_string()),
                edge_kind: None,
                confidence: None,
                edge_relation: None,
                edge_source: None,
            },
        );

        assert_eq!(result.total_nodes, 1);
        assert_eq!(result.nodes[0].label, "main");
    }

    #[test]
    fn graph_slice_pages_edges_inside_returned_node_page() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let other = graph.add_node(NodeKind::Function, "other");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(helper, other, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, other, EdgeKind::References, Confidence::Heuristic);

        let result = slice_graph(
            &graph,
            GraphSliceRequest {
                node_offset: 0,
                node_limit: 10,
                edge_offset: 0,
                edge_limit: 1,
                path_prefix: None,
                kind: Some("function".to_string()),
                search: None,
                language: None,
                item_kind: None,
                edge_kind: Some("calls".to_string()),
                confidence: None,
                edge_relation: None,
                edge_source: None,
            },
        );

        assert_eq!(result.total_nodes, 3);
        assert_eq!(result.total_edges, 2);
        assert_eq!(result.edges.len(), 1);
        assert_eq!(result.edges[0].source, main);
        assert_eq!(
            result.edges[0].metadata.get("edge_index"),
            Some(&"0".to_string())
        );
        assert!(result.truncated_edges);
    }

    #[test]
    fn graph_slice_filters_nodes_by_path_prefix() {
        let mut graph = CodeGraph::new("repo");
        let api_file = graph.add_node(NodeKind::File, "api/main.rs");
        let core_file = graph.add_node(NodeKind::File, "core/lib.rs");
        let api_main = graph.add_node(NodeKind::Function, "main");
        let core_helper = graph.add_node(NodeKind::Function, "helper");
        graph.add_edge(
            api_file,
            api_main,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        graph.add_edge(
            core_file,
            core_helper,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );

        let result = slice_graph(
            &graph,
            GraphSliceRequest {
                node_offset: 0,
                node_limit: 10,
                edge_offset: 0,
                edge_limit: 10,
                path_prefix: Some("api".to_string()),
                kind: None,
                search: None,
                language: None,
                item_kind: None,
                edge_kind: None,
                confidence: None,
                edge_relation: None,
                edge_source: None,
            },
        );

        let labels: BTreeSet<_> = result
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect();
        assert_eq!(result.total_nodes, 2);
        assert!(labels.contains("api/main.rs"));
        assert!(labels.contains("main"));
        assert!(!labels.contains("core/lib.rs"));
        assert!(!labels.contains("helper"));
    }

    #[test]
    fn graph_slice_filters_edges_by_confidence() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge_with_metadata(
            entrypoint,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([
                ("relation".to_string(), "entrypoint_function".to_string()),
                ("source".to_string(), "manifest".to_string()),
            ]),
        );

        let result = slice_graph(
            &graph,
            GraphSliceRequest {
                node_offset: 0,
                node_limit: 10,
                edge_offset: 0,
                edge_limit: 10,
                path_prefix: None,
                kind: None,
                search: None,
                language: None,
                item_kind: None,
                edge_kind: None,
                confidence: Some("exact".to_string()),
                edge_relation: Some("entrypoint_function".to_string()),
                edge_source: Some("manifest".to_string()),
            },
        );

        assert_eq!(result.total_edges, 1);
        assert_eq!(result.edges[0].source, entrypoint);
        assert_eq!(result.edges[0].confidence, Confidence::Exact);
    }

    #[test]
    fn node_context_returns_limited_neighbor_edges() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/main.rs");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        let config = graph.add_node(NodeKind::Config, "config/app.toml");
        graph.add_edge(file, main, EdgeKind::Contains, Confidence::Syntactic);
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

        let context = node_context(&graph, main, 2).unwrap();

        assert_eq!(context.node.label, "main");
        assert_eq!(context.total_edges, 3);
        assert_eq!(context.edges.len(), 2);
        assert_eq!(
            context.edges[0].metadata.get("edge_index"),
            Some(&"0".to_string())
        );
        assert_eq!(
            context.edges[1].metadata.get("edge_index"),
            Some(&"1".to_string())
        );
        assert!(context.truncated_edges);
        assert!(context.nodes.iter().any(|node| node.id == main));
        assert!(context.nodes.iter().any(|node| node.id == file));
        assert!(context.nodes.iter().any(|node| node.id == helper));
    }

    #[test]
    fn node_context_returns_none_for_missing_node() {
        let graph = CodeGraph::new("repo");

        assert!(node_context(&graph, NodeId(999), 10).is_none());
    }

    #[test]
    fn insights_report_unresolved_calls_and_orphans() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let orphan = graph.add_node(NodeKind::Function, "orphan");
        let unresolved = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "missing",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "call".to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
            ]),
        );
        graph.add_edge(main, unresolved, EdgeKind::Calls, Confidence::Heuristic);

        let report = insights(&graph);

        assert!(
            report
                .insights
                .iter()
                .any(|insight| insight.kind == "unresolved_call")
        );
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "orphan_function" && insight.nodes.contains(&orphan)
        }));
    }

    #[test]
    fn insights_report_ambiguous_call_resolution() {
        let mut graph = CodeGraph::new("repo");
        let caller = graph.add_node(NodeKind::Function, "main");
        let left = graph.add_node(NodeKind::Function, "parse");
        let right = graph.add_node(NodeKind::Function, "parser::parse");
        let single = graph.add_node(NodeKind::Function, "load_config");
        graph.add_edge_with_metadata(
            caller,
            left,
            EdgeKind::Calls,
            Confidence::Heuristic,
            BTreeMap::from([
                ("call_label".to_string(), "parse".to_string()),
                ("resolution".to_string(), "ambiguous".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            caller,
            right,
            EdgeKind::Calls,
            Confidence::Heuristic,
            BTreeMap::from([
                ("call_label".to_string(), "parse".to_string()),
                ("resolution".to_string(), "ambiguous".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            caller,
            single,
            EdgeKind::Calls,
            Confidence::Heuristic,
            BTreeMap::from([
                ("call_label".to_string(), "load_config".to_string()),
                ("resolution".to_string(), "resolved".to_string()),
            ]),
        );

        let report = insights(&graph);
        let ambiguous = report
            .insights
            .iter()
            .find(|insight| insight.kind == "ambiguous_call_resolution")
            .expect("expected ambiguous call insight");

        assert_eq!(ambiguous.severity, InsightSeverity::Warning);
        assert!(ambiguous.message.contains("main"));
        assert!(ambiguous.message.contains("parse"));
        assert!(ambiguous.nodes.contains(&caller));
        assert!(ambiguous.nodes.contains(&left));
        assert!(ambiguous.nodes.contains(&right));
        assert!(!ambiguous.nodes.contains(&single));
        assert_eq!(ambiguous.edges.len(), 2);
    }

    #[test]
    fn insights_report_unresolved_local_imports() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/app.js");
        let import = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "import missing from './missing.js';",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "javascript".to_string()),
                ("import_scope".to_string(), "local".to_string()),
                ("import_target".to_string(), "./missing.js".to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
            ]),
        );
        let external = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "import express from 'express';",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "javascript".to_string()),
            ]),
        );
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(file, external, EdgeKind::Imports, Confidence::Syntactic);

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_local_import")
            .expect("expected unresolved local import insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("src/app.js"));
        assert!(insight.message.contains("./missing.js"));
        assert!(insight.nodes.contains(&file));
        assert!(insight.nodes.contains(&import));
        assert!(!insight.nodes.contains(&external));
        assert_eq!(insight.edges.len(), 1);
    }

    #[test]
    fn insights_report_unresolved_sql_table_references() {
        let mut graph = CodeGraph::new("repo");
        let load_users = graph.add_node(NodeKind::Function, "load_users");
        let users = graph.add_node_with_metadata(
            NodeKind::Type,
            "sql table:users",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "sql_table".to_string()),
                ("table_name".to_string(), "users".to_string()),
            ]),
        );
        let query = graph.add_node_with_metadata(
            NodeKind::Config,
            "sql query:src/repo.py:2",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "app_sql_query".to_string()),
                ("operation".to_string(), "select".to_string()),
                ("tables".to_string(), "audit_log,users".to_string()),
                ("unresolved_tables".to_string(), "audit_log".to_string()),
                ("resolution".to_string(), "partial".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            load_users,
            query,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([("relation".to_string(), "app_sql_query".to_string())]),
        );
        graph.add_edge_with_metadata(
            query,
            users,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([(
                "relation".to_string(),
                "app_sql_table_reference".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_sql_table_reference")
            .expect("expected unresolved SQL table insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("audit_log"));
        assert!(insight.message.contains("load_users"));
        assert!(insight.nodes.contains(&load_users));
        assert!(insight.nodes.contains(&query));
        assert!(insight.nodes.contains(&users));
        assert_eq!(
            report.by_kind.get("unresolved_sql_table_reference"),
            Some(&1)
        );
    }

    #[test]
    fn insights_do_not_report_manifest_referenced_functions_as_orphans() {
        let mut graph = CodeGraph::new("repo");
        let entrypoint = graph.add_node(NodeKind::Entrypoint, "python console_script:cg");
        let referenced = graph.add_node(NodeKind::Function, "main");
        let orphan = graph.add_node(NodeKind::Function, "unused");
        graph.add_edge_with_metadata(
            entrypoint,
            referenced,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );

        let report = insights(&graph);

        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "orphan_function" && insight.nodes.contains(&referenced)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "orphan_function" && insight.nodes.contains(&orphan)
        }));
    }

    #[test]
    fn insights_report_duplicate_entrypoints() {
        let mut graph = CodeGraph::new("repo");
        let left = graph.add_node(NodeKind::Entrypoint, "npm script:start");
        let right = graph.add_node(NodeKind::Entrypoint, "npm script:start");
        let unique = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
        graph.add_edge(graph.root, left, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(graph.root, right, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(graph.root, unique, EdgeKind::Entrypoint, Confidence::Exact);

        let report = insights(&graph);
        let duplicate = report
            .insights
            .iter()
            .find(|insight| insight.kind == "duplicate_entrypoint_label")
            .expect("expected duplicate entrypoint insight");

        assert_eq!(duplicate.severity, InsightSeverity::Warning);
        assert_eq!(duplicate.nodes, vec![left, right]);
        assert!(duplicate.message.contains("npm script:start"));
        assert_eq!(duplicate.edges.len(), 2);
        assert!(!duplicate.nodes.contains(&unique));
    }

    #[test]
    fn insights_report_ambiguous_manifest_entrypoint_targets() {
        let mut graph = CodeGraph::new("repo");
        let ambiguous = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "python console_script:serve",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
                ("target".to_string(), "app:serve".to_string()),
            ]),
        );
        let first = graph.add_node(NodeKind::Function, "app::serve");
        let second = graph.add_node(NodeKind::Function, "legacy::serve");
        let resolved = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:api",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
                ("target".to_string(), "src/main.rs".to_string()),
            ]),
        );
        let file = graph.add_node(NodeKind::File, "src/main.rs");
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_edge(
            graph.root,
            ambiguous,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            graph.root,
            resolved,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        for target in [first, second] {
            graph.add_edge_with_metadata(
                ambiguous,
                target,
                EdgeKind::References,
                Confidence::Heuristic,
                BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
            );
        }
        graph.add_edge_with_metadata(
            resolved,
            file,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_file".to_string())]),
        );
        graph.add_edge_with_metadata(
            resolved,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "ambiguous_entrypoint_target")
            .expect("expected ambiguous entrypoint target insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("python console_script:serve"));
        assert!(insight.message.contains("functions"));
        assert!(insight.nodes.contains(&ambiguous));
        assert!(insight.nodes.contains(&first));
        assert!(insight.nodes.contains(&second));
        assert!(!insight.nodes.contains(&resolved));
        assert_eq!(insight.edges.len(), 2);
    }

    #[test]
    fn insights_report_duplicate_functions_and_error_flow() {
        let mut graph = CodeGraph::new("repo");
        let left = graph.add_node(NodeKind::Function, "parse");
        let right = graph.add_node(NodeKind::Function, "parse");
        let error = graph.add_node(NodeKind::Unknown, "panic");
        graph.add_edge(left, error, EdgeKind::MayError, Confidence::Heuristic);

        let report = insights(&graph);

        assert!(report.insights.iter().any(|insight| {
            insight.kind == "duplicate_function_label"
                && insight.nodes.contains(&left)
                && insight.nodes.contains(&right)
        }));
        assert!(
            report
                .insights
                .iter()
                .any(|insight| insight.kind == "potential_error_flow")
        );
    }

    #[test]
    fn insights_report_unresolved_manifest_entrypoints() {
        let mut graph = CodeGraph::new("repo");
        let broken = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "npm script:start",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
                ("target".to_string(), "node missing.js".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:demo",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
                ("target".to_string(), "src/main.rs".to_string()),
            ]),
        );
        let targetless = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo package:repo",
            None,
            BTreeMap::from([("item_kind".to_string(), "manifest_entrypoint".to_string())]),
        );
        let main_file = graph.add_node(NodeKind::File, "src/main.rs");
        graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            graph.root,
            resolved,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            graph.root,
            targetless,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge_with_metadata(
            resolved,
            main_file,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_file".to_string())]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_entrypoint_target")
            .expect("expected unresolved entrypoint insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![broken]);
        assert!(insight.message.contains("missing.js"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_entrypoint_target"
                && (insight.nodes.contains(&resolved) || insight.nodes.contains(&targetless))
        }));
    }

    #[test]
    fn insights_report_unresolved_makefile_command_paths() {
        let mut graph = CodeGraph::new("repo");
        let broken = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "make target:deploy",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "makefile_target".to_string()),
                (
                    "command".to_string(),
                    "./scripts/deploy.sh --prod".to_string(),
                ),
                ("command_path".to_string(), "scripts/deploy.sh".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "make target:test",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "makefile_target".to_string()),
                ("command".to_string(), "./scripts/test.sh".to_string()),
                ("command_path".to_string(), "scripts/test.sh".to_string()),
            ]),
        );
        let shell_only = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "make target:build",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "makefile_target".to_string()),
                ("command".to_string(), "cargo test --workspace".to_string()),
            ]),
        );
        let test_script = graph.add_node(NodeKind::File, "scripts/test.sh");
        graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            graph.root,
            resolved,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge(
            graph.root,
            shell_only,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge_with_metadata(
            resolved,
            test_script,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                ("relation".to_string(), "entrypoint_file".to_string()),
                ("resolution".to_string(), "make_command_path".to_string()),
            ]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_makefile_command_path")
            .expect("expected unresolved Makefile command path insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![broken]);
        assert!(insight.message.contains("scripts/deploy.sh"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_makefile_command_path"
                && (insight.nodes.contains(&resolved) || insight.nodes.contains(&shell_only))
        }));
    }

    #[test]
    fn insights_report_unresolved_dockerfile_command_paths() {
        let mut graph = CodeGraph::new("repo");
        let broken = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "docker entrypoint:./docker/start.sh",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "dockerfile_entrypoint".to_string()),
                ("command".to_string(), "./docker/start.sh".to_string()),
                ("command_path".to_string(), "docker/start.sh".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "docker cmd:./docker/migrate.sh",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "dockerfile_entrypoint".to_string()),
                ("command".to_string(), "./docker/migrate.sh".to_string()),
                ("command_path".to_string(), "docker/migrate.sh".to_string()),
            ]),
        );
        let migrate_script = graph.add_node(NodeKind::File, "docker/migrate.sh");
        graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            graph.root,
            resolved,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge_with_metadata(
            resolved,
            migrate_script,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                ("relation".to_string(), "entrypoint_file".to_string()),
                ("resolution".to_string(), "docker_command_path".to_string()),
            ]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_dockerfile_command_path")
            .expect("expected unresolved Dockerfile command path insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![broken]);
        assert!(insight.message.contains("docker/start.sh"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_dockerfile_command_path"
                && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_compose_command_paths() {
        let mut graph = CodeGraph::new("repo");
        let broken = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "compose service:web",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_service".to_string()),
                ("command".to_string(), "./scripts/start.sh".to_string()),
                ("command_path".to_string(), "scripts/start.sh".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "compose service:worker",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_service".to_string()),
                ("command".to_string(), "./scripts/worker.sh".to_string()),
                ("command_path".to_string(), "scripts/worker.sh".to_string()),
            ]),
        );
        let worker_script = graph.add_node(NodeKind::File, "scripts/worker.sh");
        graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            graph.root,
            resolved,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge_with_metadata(
            resolved,
            worker_script,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([
                ("relation".to_string(), "entrypoint_file".to_string()),
                ("resolution".to_string(), "compose_command_path".to_string()),
            ]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_compose_command_path")
            .expect("expected unresolved Compose command path insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![broken]);
        assert!(insight.message.contains("scripts/start.sh"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_compose_command_path" && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_compose_env_file_paths() {
        let mut graph = CodeGraph::new("repo");
        let web = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "compose service:web",
            None,
            BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
        );
        let worker = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "compose service:worker",
            None,
            BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
        );
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "compose env file:config/missing.env",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_env_file".to_string()),
                ("service".to_string(), "web".to_string()),
                (
                    "env_file_path".to_string(),
                    "config/missing.env".to_string(),
                ),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "compose env file:worker.env",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_env_file".to_string()),
                ("service".to_string(), "worker".to_string()),
                ("env_file_path".to_string(), "worker.env".to_string()),
            ]),
        );
        let worker_env = graph.add_node(NodeKind::File, "worker.env");
        graph.add_edge(graph.root, web, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(graph.root, worker, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge_with_metadata(
            web,
            missing,
            EdgeKind::ReadsConfig,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_env_file".to_string())]),
        );
        graph.add_edge_with_metadata(
            worker,
            resolved,
            EdgeKind::ReadsConfig,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_env_file".to_string())]),
        );
        graph.add_edge_with_metadata(
            resolved,
            worker_env,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([(
                "resolution".to_string(),
                "compose_env_file_path".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_compose_env_file_path")
            .expect("expected unresolved Compose env_file path insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&missing));
        assert!(insight.nodes.contains(&web));
        assert!(insight.message.contains("config/missing.env"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_compose_env_file_path" && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_compose_volume_source_paths() {
        let mut graph = CodeGraph::new("repo");
        let web = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "compose service:web",
            None,
            BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
        );
        let worker = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "compose service:worker",
            None,
            BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
        );
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "compose volume:config/missing->/app/config",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_volume".to_string()),
                ("service".to_string(), "web".to_string()),
                (
                    "local_source_path".to_string(),
                    "config/missing".to_string(),
                ),
                ("target_path".to_string(), "/app/config".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "compose volume:config->/app/config",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_volume".to_string()),
                ("service".to_string(), "worker".to_string()),
                ("local_source_path".to_string(), "config".to_string()),
                ("target_path".to_string(), "/app/config".to_string()),
            ]),
        );
        let config_dir = graph.add_node(NodeKind::Directory, "config");
        graph.add_edge_with_metadata(
            web,
            missing,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_volume".to_string())]),
        );
        graph.add_edge_with_metadata(
            worker,
            resolved,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_volume".to_string())]),
        );
        graph.add_edge_with_metadata(
            resolved,
            config_dir,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([(
                "resolution".to_string(),
                "compose_volume_source_path".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_compose_volume_source_path")
            .expect("expected unresolved Compose volume source path insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&missing));
        assert!(insight.nodes.contains(&web));
        assert!(insight.message.contains("config/missing"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_compose_volume_source_path"
                && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_github_actions_local_actions() {
        let mut graph = CodeGraph::new("repo");
        let build = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "github workflow:CI/build",
            None,
            BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
        );
        let deploy = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "github workflow:CI/deploy",
            None,
            BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
        );
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "github action:.github/actions/missing",
            None,
            BTreeMap::from([
                (
                    "item_kind".to_string(),
                    "github_actions_local_action".to_string(),
                ),
                ("workflow".to_string(), "CI".to_string()),
                ("job".to_string(), "build".to_string()),
                (
                    "local_action_path".to_string(),
                    ".github/actions/missing".to_string(),
                ),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "github action:.github/actions/setup",
            None,
            BTreeMap::from([
                (
                    "item_kind".to_string(),
                    "github_actions_local_action".to_string(),
                ),
                ("workflow".to_string(), "CI".to_string()),
                ("job".to_string(), "deploy".to_string()),
                (
                    "local_action_path".to_string(),
                    ".github/actions/setup".to_string(),
                ),
            ]),
        );
        let setup_dir = graph.add_node(NodeKind::Directory, ".github/actions/setup");
        graph.add_edge_with_metadata(
            build,
            missing,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "github_actions_uses".to_string())]),
        );
        graph.add_edge_with_metadata(
            deploy,
            resolved,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "github_actions_uses".to_string())]),
        );
        graph.add_edge_with_metadata(
            resolved,
            setup_dir,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([(
                "resolution".to_string(),
                "github_actions_local_action_path".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_github_actions_local_action")
            .expect("expected unresolved GitHub Actions local action insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&missing));
        assert!(insight.nodes.contains(&build));
        assert!(insight.message.contains(".github/actions/missing"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_github_actions_local_action"
                && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_github_actions_job_needs() {
        let mut graph = CodeGraph::new("repo");
        let build = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "github workflow:CI/build",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "github_actions_job".to_string()),
                ("workflow".to_string(), "CI".to_string()),
                ("job".to_string(), "build".to_string()),
            ]),
        );
        let deploy = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "github workflow:CI/deploy",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "github_actions_job".to_string()),
                ("workflow".to_string(), "CI".to_string()),
                ("job".to_string(), "deploy".to_string()),
                ("needs".to_string(), "build,missing".to_string()),
            ]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_github_actions_job_need")
            .expect("expected unresolved GitHub Actions job need insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![deploy]);
        assert!(insight.message.contains("missing"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_github_actions_job_need"
                && insight.message.contains("build")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_github_actions_job_need" && insight.nodes.contains(&build)
        }));
    }

    #[test]
    fn insights_report_unresolved_github_actions_run_paths() {
        let mut graph = CodeGraph::new("repo");
        let build = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "github workflow:CI/build",
            None,
            BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
        );
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "github run:CI/build/10",
            None,
            BTreeMap::from([
                (
                    "item_kind".to_string(),
                    "github_actions_run_step".to_string(),
                ),
                ("workflow".to_string(), "CI".to_string()),
                ("job".to_string(), "build".to_string()),
                ("command".to_string(), "./scripts/missing.sh".to_string()),
                ("command_path".to_string(), "scripts/missing.sh".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "github run:CI/build/11",
            None,
            BTreeMap::from([
                (
                    "item_kind".to_string(),
                    "github_actions_run_step".to_string(),
                ),
                ("workflow".to_string(), "CI".to_string()),
                ("job".to_string(), "build".to_string()),
                ("command".to_string(), "./scripts/test.sh".to_string()),
                ("command_path".to_string(), "scripts/test.sh".to_string()),
            ]),
        );
        let test_script = graph.add_node(NodeKind::File, "scripts/test.sh");
        for step in [missing, resolved] {
            graph.add_edge_with_metadata(
                build,
                step,
                EdgeKind::References,
                Confidence::Exact,
                BTreeMap::from([("relation".to_string(), "github_actions_run".to_string())]),
            );
        }
        graph.add_edge_with_metadata(
            build,
            test_script,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([(
                "resolution".to_string(),
                "github_actions_run_command_path".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_github_actions_run_path")
            .expect("expected unresolved GitHub Actions run path insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&missing));
        assert!(insight.nodes.contains(&build));
        assert!(insight.message.contains("scripts/missing.sh"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_github_actions_run_path"
                && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_gitlab_ci_script_paths() {
        let mut graph = CodeGraph::new("repo");
        let build = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "gitlab job:build",
            None,
            BTreeMap::from([("item_kind".to_string(), "gitlab_ci_job".to_string())]),
        );
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "gitlab script:build/10",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "gitlab_ci_script".to_string()),
                ("job".to_string(), "build".to_string()),
                ("command".to_string(), "./scripts/missing.sh".to_string()),
                ("command_path".to_string(), "scripts/missing.sh".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "gitlab script:build/11",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "gitlab_ci_script".to_string()),
                ("job".to_string(), "build".to_string()),
                ("command".to_string(), "./scripts/test.sh".to_string()),
                ("command_path".to_string(), "scripts/test.sh".to_string()),
            ]),
        );
        let test_script = graph.add_node(NodeKind::File, "scripts/test.sh");
        for script in [missing, resolved] {
            graph.add_edge_with_metadata(
                build,
                script,
                EdgeKind::References,
                Confidence::Exact,
                BTreeMap::from([("relation".to_string(), "gitlab_ci_script".to_string())]),
            );
        }
        graph.add_edge_with_metadata(
            build,
            test_script,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([(
                "resolution".to_string(),
                "gitlab_ci_script_command_path".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_gitlab_ci_script_path")
            .expect("expected unresolved GitLab CI script path insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&missing));
        assert!(insight.nodes.contains(&build));
        assert!(insight.message.contains("scripts/missing.sh"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_gitlab_ci_script_path" && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_gitlab_ci_job_dependencies() {
        let mut graph = CodeGraph::new("repo");
        let build = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "gitlab job:build",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "gitlab_ci_job".to_string()),
                ("job".to_string(), "build".to_string()),
            ]),
        );
        let deploy = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "gitlab job:deploy",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "gitlab_ci_job".to_string()),
                ("job".to_string(), "deploy".to_string()),
                ("needs".to_string(), "build,missing-need".to_string()),
                (
                    "dependencies".to_string(),
                    "build,missing-artifacts".to_string(),
                ),
            ]),
        );

        let report = insights(&graph);
        let missing_need = report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "unresolved_gitlab_ci_job_dependency"
                    && insight.message.contains("missing-need")
            })
            .expect("expected unresolved GitLab CI need insight");
        let missing_artifacts = report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "unresolved_gitlab_ci_job_dependency"
                    && insight.message.contains("missing-artifacts")
            })
            .expect("expected unresolved GitLab CI dependency insight");

        assert_eq!(missing_need.severity, InsightSeverity::Warning);
        assert_eq!(missing_need.nodes, vec![deploy]);
        assert_eq!(missing_artifacts.nodes, vec![deploy]);
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_gitlab_ci_job_dependency"
                && insight.message.contains("`build`")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_gitlab_ci_job_dependency" && insight.nodes.contains(&build)
        }));
    }

    #[test]
    fn insights_report_unresolved_kubernetes_config_refs() {
        let mut graph = CodeGraph::new("repo");
        let web = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "k8s deployment:prod/web",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_workload".to_string()),
                ("kubernetes_kind".to_string(), "Deployment".to_string()),
                ("name".to_string(), "web".to_string()),
                ("namespace".to_string(), "prod".to_string()),
            ]),
        );
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s config ref:configmap prod/missing-config",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_config_ref".to_string()),
                ("config_kind".to_string(), "configmap".to_string()),
                ("name".to_string(), "missing-config".to_string()),
                ("namespace".to_string(), "prod".to_string()),
                ("workload".to_string(), "web".to_string()),
                ("workload_kind".to_string(), "Deployment".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s config ref:secret prod/app-secret",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_config_ref".to_string()),
                ("config_kind".to_string(), "secret".to_string()),
                ("name".to_string(), "app-secret".to_string()),
                ("namespace".to_string(), "prod".to_string()),
                ("workload".to_string(), "web".to_string()),
                ("workload_kind".to_string(), "Deployment".to_string()),
            ]),
        );
        let secret = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s secret:prod/app-secret",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_config".to_string()),
                ("config_kind".to_string(), "secret".to_string()),
                ("name".to_string(), "app-secret".to_string()),
                ("namespace".to_string(), "prod".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            web,
            missing,
            EdgeKind::ReadsConfig,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "kubernetes_config_ref".to_string())]),
        );
        graph.add_edge_with_metadata(
            web,
            resolved,
            EdgeKind::ReadsConfig,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "kubernetes_config_ref".to_string())]),
        );
        graph.add_edge_with_metadata(
            resolved,
            secret,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([(
                "resolution".to_string(),
                "kubernetes_config_ref".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_kubernetes_config_ref")
            .expect("expected unresolved Kubernetes config ref insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&missing));
        assert!(insight.nodes.contains(&web));
        assert!(insight.message.contains("prod/missing-config"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_kubernetes_config_ref" && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_kubernetes_ingress_backends() {
        let mut graph = CodeGraph::new("repo");
        let ingress = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "k8s ingress:prod/web",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_ingress".to_string()),
                ("name".to_string(), "web".to_string()),
                ("namespace".to_string(), "prod".to_string()),
            ]),
        );
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s service ref:prod/missing",
            None,
            BTreeMap::from([
                (
                    "item_kind".to_string(),
                    "kubernetes_service_ref".to_string(),
                ),
                ("name".to_string(), "missing".to_string()),
                ("namespace".to_string(), "prod".to_string()),
                ("ingress".to_string(), "web".to_string()),
                ("host".to_string(), "example.test".to_string()),
                ("path".to_string(), "/missing".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s service ref:prod/api",
            None,
            BTreeMap::from([
                (
                    "item_kind".to_string(),
                    "kubernetes_service_ref".to_string(),
                ),
                ("name".to_string(), "api".to_string()),
                ("namespace".to_string(), "prod".to_string()),
                ("ingress".to_string(), "web".to_string()),
                ("host".to_string(), "example.test".to_string()),
                ("path".to_string(), "/api".to_string()),
            ]),
        );
        let service = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s service:prod/api",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_service".to_string()),
                ("name".to_string(), "api".to_string()),
                ("namespace".to_string(), "prod".to_string()),
            ]),
        );
        for service_ref in [missing, resolved] {
            graph.add_edge_with_metadata(
                ingress,
                service_ref,
                EdgeKind::References,
                Confidence::Exact,
                BTreeMap::from([(
                    "relation".to_string(),
                    "kubernetes_ingress_backend".to_string(),
                )]),
            );
        }
        graph.add_edge_with_metadata(
            resolved,
            service,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([(
                "resolution".to_string(),
                "kubernetes_service_ref".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_kubernetes_ingress_backend")
            .expect("expected unresolved Kubernetes ingress backend insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&missing));
        assert!(insight.nodes.contains(&ingress));
        assert!(insight.message.contains("example.test/missing"));
        assert!(insight.message.contains("prod/missing"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_kubernetes_ingress_backend"
                && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_unresolved_kubernetes_service_selectors() {
        let mut graph = CodeGraph::new("repo");
        let missing = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s service:prod/orphan",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_service".to_string()),
                ("name".to_string(), "orphan".to_string()),
                ("namespace".to_string(), "prod".to_string()),
                ("selector".to_string(), "app=missing".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Config,
            "k8s service:prod/web",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_service".to_string()),
                ("name".to_string(), "web".to_string()),
                ("namespace".to_string(), "prod".to_string()),
                ("selector".to_string(), "app=web".to_string()),
            ]),
        );
        let web = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "k8s deployment:prod/web",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "kubernetes_workload".to_string()),
                ("name".to_string(), "web".to_string()),
                ("namespace".to_string(), "prod".to_string()),
                ("pod_labels".to_string(), "app=web".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            resolved,
            web,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([(
                "relation".to_string(),
                "kubernetes_service_selector".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_kubernetes_service_selector")
            .expect("expected unresolved Kubernetes service selector insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![missing]);
        assert!(insight.message.contains("prod/orphan"));
        assert!(insight.message.contains("app=missing"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unresolved_kubernetes_service_selector"
                && insight.nodes.contains(&resolved)
        }));
    }

    #[test]
    fn insights_report_duplicate_compose_published_ports() {
        let mut graph = CodeGraph::new("repo");
        let web = graph.add_node(NodeKind::Entrypoint, "compose service:web");
        let admin = graph.add_node(NodeKind::Entrypoint, "compose service:admin");
        let worker = graph.add_node(NodeKind::Entrypoint, "compose service:worker");
        let web_port = graph.add_node_with_metadata(
            NodeKind::Config,
            "compose port:8080->80/tcp",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_port".to_string()),
                ("service".to_string(), "web".to_string()),
                ("published_port".to_string(), "8080".to_string()),
                ("target_port".to_string(), "80".to_string()),
                ("protocol".to_string(), "tcp".to_string()),
            ]),
        );
        let admin_port = graph.add_node_with_metadata(
            NodeKind::Config,
            "compose port:8080->8080/tcp",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_port".to_string()),
                ("service".to_string(), "admin".to_string()),
                ("published_port".to_string(), "8080".to_string()),
                ("target_port".to_string(), "8080".to_string()),
                ("protocol".to_string(), "tcp".to_string()),
            ]),
        );
        let worker_port = graph.add_node_with_metadata(
            NodeKind::Config,
            "compose port:8080->9000/udp",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "compose_port".to_string()),
                ("service".to_string(), "worker".to_string()),
                ("published_port".to_string(), "8080".to_string()),
                ("target_port".to_string(), "9000".to_string()),
                ("protocol".to_string(), "udp".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            web,
            web_port,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_port".to_string())]),
        );
        graph.add_edge_with_metadata(
            admin,
            admin_port,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_port".to_string())]),
        );
        graph.add_edge_with_metadata(
            worker,
            worker_port,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_port".to_string())]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "duplicate_compose_published_port")
            .expect("expected duplicate Compose published port insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("8080/tcp"));
        assert!(insight.message.contains("web"));
        assert!(insight.message.contains("admin"));
        assert!(insight.nodes.contains(&web_port));
        assert!(insight.nodes.contains(&admin_port));
        assert!(!insight.nodes.contains(&worker_port));
    }

    #[test]
    fn insights_report_entrypoint_dead_ends() {
        let mut graph = CodeGraph::new("repo");
        let dead = graph.add_node(NodeKind::Entrypoint, "npm script:preview");
        let live = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
        let main = graph.add_node(NodeKind::Function, "main");
        let unresolved_manifest = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "cargo bin:missing",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
                ("target".to_string(), "src/missing.rs".to_string()),
            ]),
        );
        graph.add_edge(graph.root, dead, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(graph.root, live, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            graph.root,
            unresolved_manifest,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
        graph.add_edge_with_metadata(
            live,
            main,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );

        let report = insights(&graph);
        let dead_end = report
            .insights
            .iter()
            .find(|insight| insight.kind == "entrypoint_dead_end")
            .expect("expected dead-end entrypoint insight");

        assert_eq!(dead_end.severity, InsightSeverity::Warning);
        assert_eq!(dead_end.nodes, vec![dead]);
        assert!(dead_end.edges.iter().any(|index| {
            graph
                .edges
                .get(*index)
                .is_some_and(|edge| edge.source == graph.root && edge.target == dead)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "entrypoint_dead_end"
                && (insight.nodes.contains(&live) || insight.nodes.contains(&unresolved_manifest))
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "unresolved_entrypoint_target"
                && insight.nodes.contains(&unresolved_manifest)
        }));
    }

    #[test]
    fn insights_report_unreachable_config_reads() {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let live_config = graph.add_node(NodeKind::Environment, "DATABASE_URL");
        let unused_loader = graph.add_node(NodeKind::Function, "unused_loader");
        let unused_config = graph.add_node(NodeKind::Config, "config/legacy.toml");
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
        graph.add_edge(
            main,
            live_config,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            unused_loader,
            unused_config,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unreachable_config_read")
            .expect("expected unreachable config read insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![unused_loader, unused_config]);
        assert!(insight.message.contains("unused_loader"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unreachable_config_read" && insight.nodes.contains(&main)
        }));
    }

    #[test]
    fn insights_report_unreachable_error_flows() {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node(NodeKind::Function, "main");
        let live_error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "panic",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        let legacy_worker = graph.add_node(NodeKind::Function, "legacy_worker");
        let legacy_error = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "LegacyError",
            None,
            BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
        );
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
        graph.add_edge(main, live_error, EdgeKind::MayError, Confidence::Heuristic);
        graph.add_edge(
            legacy_worker,
            legacy_error,
            EdgeKind::MayError,
            Confidence::Heuristic,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unreachable_error_flow")
            .expect("expected unreachable error flow insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.nodes, vec![legacy_worker, legacy_error]);
        assert!(insight.message.contains("legacy_worker"));
        assert!(insight.message.contains("LegacyError"));
        assert_eq!(report.by_kind.get("unreachable_error_flow"), Some(&1));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unreachable_error_flow" && insight.nodes.contains(&main)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "potential_error_flow" && insight.nodes.contains(&main)
        }));
    }

    #[test]
    fn insights_report_unreachable_source_files() {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let live_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let live_main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let legacy_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/legacy.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let legacy_fn = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_worker",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let test_file = graph.add_node_with_metadata(
            NodeKind::File,
            "tests/legacy_test.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let test_fn = graph.add_node_with_metadata(
            NodeKind::Function,
            "legacy_test",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(
            live_file,
            live_main,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        graph.add_edge(entry, live_main, EdgeKind::References, Confidence::Exact);
        graph.add_edge(
            legacy_file,
            legacy_fn,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );
        graph.add_edge(
            test_file,
            test_fn,
            EdgeKind::Contains,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unreachable_source_file")
            .expect("expected unreachable source file insight");

        assert_eq!(insight.severity, InsightSeverity::Info);
        assert_eq!(insight.nodes, vec![legacy_file]);
        assert!(insight.message.contains("src/legacy.rs"));
        assert!(insight.message.contains("rust"));
        assert_eq!(insight.edges.len(), 1);
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unreachable_source_file"
                && (insight.nodes.contains(&live_file) || insight.nodes.contains(&test_file))
        }));
    }

    #[test]
    fn insights_report_conflicting_config_defaults() {
        let mut graph = CodeGraph::new("repo");
        let first_reader = graph.add_node(NodeKind::Function, "api_server");
        let second_reader = graph.add_node(NodeKind::Function, "worker");
        let first_env = graph.add_node_with_metadata(
            NodeKind::Environment,
            "PORT",
            None,
            BTreeMap::from([("default_value".to_string(), "8000".to_string())]),
        );
        let second_env = graph.add_node_with_metadata(
            NodeKind::Environment,
            "PORT",
            None,
            BTreeMap::from([("default_value".to_string(), "9000".to_string())]),
        );
        let extra_envs = ["3000", "5000", "7000", "8080", "9090", "9091", "9092"]
            .into_iter()
            .map(|default_value| {
                graph.add_node_with_metadata(
                    NodeKind::Environment,
                    "PORT",
                    None,
                    BTreeMap::from([("default_value".to_string(), default_value.to_string())]),
                )
            })
            .collect::<Vec<_>>();
        let stable_env = graph.add_node_with_metadata(
            NodeKind::Environment,
            "HOST",
            None,
            BTreeMap::from([("default_value".to_string(), "127.0.0.1".to_string())]),
        );
        graph.add_edge(
            first_reader,
            first_env,
            EdgeKind::ReadsEnvironment,
            codegraph_core::Confidence::Heuristic,
        );
        graph.add_edge(
            second_reader,
            second_env,
            EdgeKind::ReadsEnvironment,
            codegraph_core::Confidence::Heuristic,
        );
        for env in &extra_envs {
            graph.add_edge(
                second_reader,
                *env,
                EdgeKind::ReadsEnvironment,
                codegraph_core::Confidence::Heuristic,
            );
        }
        graph.add_edge(
            second_reader,
            stable_env,
            EdgeKind::ReadsEnvironment,
            codegraph_core::Confidence::Heuristic,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "conflicting_config_default")
            .expect("expected conflicting config default insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("PORT"));
        assert!(insight.message.contains("8000"));
        assert!(insight.message.contains("9000"));
        assert!(insight.message.contains("9091"));
        assert!(insight.message.contains("and 1 more"));
        assert!(insight.nodes.contains(&first_env));
        assert!(insight.nodes.contains(&second_env));
        assert!(extra_envs.iter().all(|env| insight.nodes.contains(env)));
        assert_eq!(insight.edges.len(), 9);
        assert!(!insight.nodes.contains(&stable_env));
    }

    #[test]
    fn insights_report_mixed_config_requirement_defaults() {
        let mut graph = CodeGraph::new("repo");
        let api = graph.add_node(NodeKind::Function, "api_server");
        let worker = graph.add_node(NodeKind::Function, "worker");
        let required_port = graph.add_node(NodeKind::Environment, "PORT");
        let default_port = graph.add_node_with_metadata(
            NodeKind::Environment,
            "PORT",
            None,
            BTreeMap::from([("default_value".to_string(), "8080".to_string())]),
        );
        let stable_host = graph.add_node_with_metadata(
            NodeKind::Environment,
            "HOST",
            None,
            BTreeMap::from([("default_value".to_string(), "127.0.0.1".to_string())]),
        );
        let unused_required_port = graph.add_node(NodeKind::Environment, "PORT");
        graph.add_edge(
            api,
            required_port,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            worker,
            default_port,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            api,
            stable_host,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "mixed_config_requirement")
            .expect("expected mixed config requirement insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("PORT"));
        assert!(insight.message.contains("required"));
        assert!(insight.message.contains("8080"));
        assert!(insight.nodes.contains(&required_port));
        assert!(insight.nodes.contains(&default_port));
        assert!(!insight.nodes.contains(&stable_host));
        assert!(!insight.nodes.contains(&unused_required_port));
        assert_eq!(insight.edges.len(), 2);
        assert_eq!(report.by_kind.get("mixed_config_requirement"), Some(&1));
    }

    #[test]
    fn insights_report_undeclared_flutter_asset_reads() {
        let mut graph = CodeGraph::new("repo");
        let pubspec = graph.add_node(NodeKind::File, "pubspec.yaml");
        let main = graph.add_node(NodeKind::Function, "main");
        let declared_file = graph.add_node_with_metadata(
            NodeKind::Config,
            "flutter asset:assets/config/app.json",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "flutter_asset".to_string()),
                (
                    "asset_path".to_string(),
                    "assets/config/app.json".to_string(),
                ),
            ]),
        );
        let declared_dir = graph.add_node_with_metadata(
            NodeKind::Config,
            "flutter asset:assets/images/",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "flutter_asset".to_string()),
                ("asset_path".to_string(), "assets/images/".to_string()),
            ]),
        );
        let declared_read = graph.add_node_with_metadata(
            NodeKind::Config,
            "flutter asset read:assets/config/app.json",
            None,
            BTreeMap::from([
                ("config_kind".to_string(), "flutter_asset_read".to_string()),
                ("value".to_string(), "assets/config/app.json".to_string()),
            ]),
        );
        let directory_read = graph.add_node_with_metadata(
            NodeKind::Config,
            "flutter asset read:assets/images/logo.png",
            None,
            BTreeMap::from([
                ("config_kind".to_string(), "flutter_asset_read".to_string()),
                ("value".to_string(), "assets/images/logo.png".to_string()),
            ]),
        );
        let missing_read = graph.add_node_with_metadata(
            NodeKind::Config,
            "flutter asset read:assets/missing/secret.json",
            None,
            BTreeMap::from([
                ("config_kind".to_string(), "flutter_asset_read".to_string()),
                (
                    "value".to_string(),
                    "assets/missing/secret.json".to_string(),
                ),
            ]),
        );
        graph.add_edge(
            pubspec,
            declared_file,
            EdgeKind::Contains,
            Confidence::Exact,
        );
        graph.add_edge(pubspec, declared_dir, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(
            main,
            declared_read,
            EdgeKind::ReadsConfig,
            Confidence::Syntactic,
        );
        graph.add_edge(
            main,
            directory_read,
            EdgeKind::ReadsConfig,
            Confidence::Syntactic,
        );
        graph.add_edge(
            main,
            missing_read,
            EdgeKind::ReadsConfig,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "undeclared_flutter_asset")
            .expect("expected undeclared Flutter asset insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("assets/missing/secret.json"));
        assert!(insight.nodes.contains(&main));
        assert!(insight.nodes.contains(&missing_read));
        assert!(!insight.nodes.contains(&declared_read));
        assert!(!insight.nodes.contains(&directory_read));
        assert_eq!(report.by_kind.get("undeclared_flutter_asset"), Some(&1));
    }

    #[test]
    fn insights_report_rationale_risk_comments() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/auth.rs");
        let security = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "SECURITY: verify token audience",
            Some(SourceSpan {
                path: "src/auth.rs".to_string(),
                start_line: 7,
                start_column: 1,
                end_line: 7,
                end_column: 35,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "rationale_comment".to_string()),
                ("rationale_kind".to_string(), "security".to_string()),
            ]),
        );
        let fixme = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "FIXME: handle retry backoff",
            Some(SourceSpan {
                path: "src/auth.rs".to_string(),
                start_line: 12,
                start_column: 5,
                end_line: 12,
                end_column: 33,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "rationale_comment".to_string()),
                ("rationale_kind".to_string(), "fixme".to_string()),
            ]),
        );
        let why = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "WHY: keep startup simple",
            Some(SourceSpan {
                path: "src/auth.rs".to_string(),
                start_line: 3,
                start_column: 1,
                end_line: 3,
                end_column: 27,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "rationale_comment".to_string()),
                ("rationale_kind".to_string(), "why".to_string()),
            ]),
        );
        for node in [security, fixme, why] {
            graph.add_edge_with_metadata(
                file,
                node,
                EdgeKind::Contains,
                Confidence::Exact,
                BTreeMap::from([("relation".to_string(), "rationale_comment".to_string())]),
            );
        }

        let report = insights(&graph);
        let rationale = report
            .insights
            .iter()
            .filter(|insight| insight.kind == "rationale_risk_comment")
            .collect::<Vec<_>>();

        assert_eq!(rationale.len(), 2);
        assert!(rationale.iter().any(|insight| {
            insight.severity == InsightSeverity::Error
                && insight.nodes.contains(&security)
                && insight.nodes.contains(&file)
                && insight.message.contains("SECURITY")
                && insight.message.contains("src/auth.rs:7")
        }));
        assert!(rationale.iter().any(|insight| {
            insight.severity == InsightSeverity::Warning
                && insight.nodes.contains(&fixme)
                && insight.nodes.contains(&file)
                && insight.message.contains("FIXME")
                && insight.message.contains("src/auth.rs:12")
        }));
        assert!(!rationale.iter().any(|insight| insight.nodes.contains(&why)));
        assert_eq!(report.by_kind.get("rationale_risk_comment"), Some(&2));
    }

    #[test]
    fn insights_report_sensitive_config_defaults_without_leaking_values() {
        let mut graph = CodeGraph::new("repo");
        let api = graph.add_node(NodeKind::Function, "api_server");
        let worker = graph.add_node(NodeKind::Function, "worker");
        let secret = graph.add_node_with_metadata(
            NodeKind::Environment,
            "API_TOKEN",
            None,
            BTreeMap::from([("default_value".to_string(), "dev-super-secret".to_string())]),
        );
        let database_url = graph.add_node_with_metadata(
            NodeKind::Environment,
            "DATABASE_URL",
            None,
            BTreeMap::from([(
                "default_value".to_string(),
                "postgres://demo:password@localhost/app".to_string(),
            )]),
        );
        let auth_header = graph.add_node_with_metadata(
            NodeKind::Config,
            "service.auth_header",
            None,
            BTreeMap::from([("default_value".to_string(), "replace-me-token".to_string())]),
        );
        let port = graph.add_node_with_metadata(
            NodeKind::Environment,
            "PORT",
            None,
            BTreeMap::from([("default_value".to_string(), "8080".to_string())]),
        );
        let public_key = graph.add_node_with_metadata(
            NodeKind::Environment,
            "PUBLIC_KEY",
            None,
            BTreeMap::from([("default_value".to_string(), "public-demo-key".to_string())]),
        );
        let callback_url = graph.add_node_with_metadata(
            NodeKind::Config,
            "CALLBACK_URL",
            None,
            BTreeMap::from([(
                "default_value".to_string(),
                "https://example.com/callback".to_string(),
            )]),
        );
        graph.add_edge(
            api,
            secret,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            api,
            database_url,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            api,
            auth_header,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );
        graph.add_edge(
            worker,
            port,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            worker,
            public_key,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
        );
        graph.add_edge(
            worker,
            callback_url,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );

        let report = insights(&graph);
        let sensitive = report
            .insights
            .iter()
            .filter(|insight| insight.kind == "sensitive_config_default")
            .collect::<Vec<_>>();

        assert_eq!(sensitive.len(), 3);
        assert!(sensitive.iter().any(|insight| {
            insight.nodes.contains(&secret)
                && insight.nodes.contains(&api)
                && insight.message.contains("API_TOKEN")
                && !insight.message.contains("dev-super-secret")
        }));
        assert!(sensitive.iter().any(|insight| {
            insight.nodes.contains(&database_url)
                && insight.message.contains("DATABASE_URL")
                && !insight.message.contains("postgres://")
        }));
        assert!(sensitive.iter().any(|insight| {
            insight.nodes.contains(&auth_header)
                && insight.nodes.contains(&api)
                && insight.message.contains("service.auth_header")
                && !insight.message.contains("replace-me-token")
        }));
        assert!(
            !sensitive.iter().any(|insight| insight.nodes.contains(&port)
                || insight.nodes.contains(&public_key)
                || insight.nodes.contains(&callback_url))
        );
        assert_eq!(report.by_kind.get("sensitive_config_default"), Some(&3));
        assert_eq!(report.by_severity.get("warning"), Some(&3));
    }

    #[test]
    fn insights_report_sensitive_ci_environment_literals_without_leaking_values() {
        let mut graph = CodeGraph::new("repo");
        let job = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "github workflow:CI/deploy",
            None,
            BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
        );
        let literal_secret = graph.add_node_with_metadata(
            NodeKind::Environment,
            "API_TOKEN",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "ci_environment".to_string()),
                ("source".to_string(), "github-actions".to_string()),
                ("scope".to_string(), "job".to_string()),
                ("value_kind".to_string(), "literal".to_string()),
            ]),
        );
        let secret_reference = graph.add_node_with_metadata(
            NodeKind::Environment,
            "DEPLOY_TOKEN",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "ci_environment".to_string()),
                ("source".to_string(), "github-actions".to_string()),
                ("scope".to_string(), "job".to_string()),
                ("value_kind".to_string(), "secret_reference".to_string()),
            ]),
        );
        let ordinary_literal = graph.add_node_with_metadata(
            NodeKind::Environment,
            "BUILD_MODE",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "ci_environment".to_string()),
                ("source".to_string(), "github-actions".to_string()),
                ("scope".to_string(), "job".to_string()),
                ("value_kind".to_string(), "literal".to_string()),
            ]),
        );
        for environment in [literal_secret, secret_reference, ordinary_literal] {
            graph.add_edge_with_metadata(
                job,
                environment,
                EdgeKind::ReadsEnvironment,
                Confidence::Exact,
                BTreeMap::from([("relation".to_string(), "ci_environment".to_string())]),
            );
        }

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "sensitive_ci_environment_literal")
            .expect("expected sensitive CI environment literal insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.nodes.contains(&literal_secret));
        assert!(insight.nodes.contains(&job));
        assert!(insight.message.contains("API_TOKEN"));
        assert!(!insight.message.contains("dev-super-secret"));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "sensitive_ci_environment_literal"
                && (insight.nodes.contains(&secret_reference)
                    || insight.nodes.contains(&ordinary_literal))
        }));
    }

    #[test]
    fn insights_report_dependency_cycles() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let service = graph.add_node(NodeKind::Function, "service");
        let repository = graph.add_node(NodeKind::Function, "repository");
        let config = graph.add_node(NodeKind::Config, "settings.toml");
        graph.add_edge(main, service, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(service, repository, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(repository, main, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge(
            service,
            config,
            EdgeKind::ReadsConfig,
            Confidence::Heuristic,
        );

        let report = insights(&graph);
        let cycle = report
            .insights
            .iter()
            .find(|insight| insight.kind == "dependency_cycle")
            .expect("expected dependency cycle insight");

        assert_eq!(cycle.severity, InsightSeverity::Warning);
        assert_eq!(cycle.nodes, vec![main, service, repository]);
        assert_eq!(cycle.edges.len(), 3);
        assert!(cycle.message.contains("main"));
        assert!(!cycle.nodes.contains(&config));
    }

    #[test]
    fn insights_report_undeclared_external_imports() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/main.ts");
        let react = dependency_node(&mut graph, "react", "npm:react");
        graph.add_edge(file, react, EdgeKind::DependsOn, Confidence::Exact);

        let react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
        let express_import =
            import_node(&mut graph, "import express from \"express\";", "typescript");
        let lodash_require = import_node(&mut graph, "require(\"lodash\")", "javascript");
        let local_python_import = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            "import service",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "python".to_string()),
                ("import_scope".to_string(), "local".to_string()),
                ("resolution".to_string(), "resolved".to_string()),
            ]),
        );
        let fs_import = import_node(&mut graph, "import fs from \"node:fs\";", "typescript");
        graph.add_edge(file, react_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(
            file,
            express_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            file,
            lodash_require,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            file,
            local_python_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(file, fs_import, EdgeKind::Imports, Confidence::Syntactic);

        let report = insights(&graph);
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("express")
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("lodash")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("react")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("service")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("node:fs")
        }));
    }

    #[test]
    fn insights_report_unused_declared_runtime_dependencies() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "package.json");
        let file = graph.add_node(NodeKind::File, "src/main.ts");
        let react = dependency_node(&mut graph, "react", "npm:react");
        let lodash = dependency_node(&mut graph, "lodash", "npm:lodash");
        let vite = dependency_node(&mut graph, "vite", "npm:vite");
        graph.add_edge_with_metadata(
            manifest,
            react,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            lodash,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            vite,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
        );

        let react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
        graph.add_edge(file, react_import, EdgeKind::Imports, Confidence::Syntactic);

        let report = insights(&graph);
        let unused = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unused_declared_dependency")
            .expect("expected unused declared dependency insight");

        assert_eq!(unused.severity, InsightSeverity::Info);
        assert!(unused.message.contains("lodash"));
        assert!(unused.nodes.contains(&manifest));
        assert!(unused.nodes.contains(&lodash));
        assert!(!unused.nodes.contains(&react));
        assert!(!unused.nodes.contains(&vite));
        assert_eq!(unused.edges.len(), 1);
    }

    #[test]
    fn unused_dependency_insights_follow_rust_direct_crate_paths() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "Cargo.toml");
        let function = graph.add_node(NodeKind::Function, "load_manifest");
        let toml = dependency_node(&mut graph, "toml", "cargo:toml");
        let serde = dependency_node(&mut graph, "serde", "cargo:serde");
        let call = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "toml::from_str",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "call".to_string()),
                ("language".to_string(), "rust".to_string()),
            ]),
        );
        graph.add_edge(function, call, EdgeKind::Calls, Confidence::Heuristic);
        graph.add_edge_with_metadata(
            manifest,
            toml,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            serde,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );

        let report = insights(&graph);
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&toml)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&serde)
        }));
    }

    #[test]
    fn dependency_insights_ignore_setup_py_manifest_imports() {
        let mut graph = CodeGraph::new("repo");
        let setup_file = graph.add_node_with_metadata(
            NodeKind::File,
            "setup.py",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let app_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/app.py",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let requests = dependency_node(&mut graph, "requests", "python:requests");
        let fastapi = dependency_node(&mut graph, "fastapi", "python:fastapi");
        graph.add_edge_with_metadata(
            setup_file,
            requests,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            setup_file,
            fastapi,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );

        let setup_import = import_node(&mut graph, "from setuptools import setup", "python");
        let setup_requests = import_node(&mut graph, "import requests", "python");
        let fastapi_import = import_node(&mut graph, "from fastapi import FastAPI", "python");
        graph.add_edge(
            setup_file,
            setup_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            setup_file,
            setup_requests,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            app_file,
            fastapi_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("setuptools")
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&requests)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&fastapi)
        }));
    }

    #[test]
    fn insights_match_c_family_package_manager_includes() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "vcpkg.json");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.cpp",
            None,
            BTreeMap::from([("language".to_string(), "cpp".to_string())]),
        );
        let fmt = dependency_node(&mut graph, "fmt", "vcpkg:fmt");
        let zlib = dependency_node(&mut graph, "zlib", "vcpkg:zlib");
        let curl = dependency_node(&mut graph, "curl", "vcpkg:curl");
        let spdlog = dependency_node(&mut graph, "spdlog", "conan:spdlog");
        let cmake = dependency_node(&mut graph, "cmake", "conan:cmake");
        let openssl = dependency_node(&mut graph, "openssl", "cmake:openssl");

        for dependency in [fmt, zlib, curl, spdlog, openssl] {
            graph.add_edge_with_metadata(
                manifest,
                dependency,
                EdgeKind::DependsOn,
                Confidence::Exact,
                BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
            );
        }
        graph.add_edge_with_metadata(
            manifest,
            cmake,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "build".to_string())]),
        );

        let fmt_include = import_node(&mut graph, "#include <fmt/core.h>", "cpp");
        let zlib_include = import_node(&mut graph, "#include <zlib.h>", "cpp");
        let spdlog_include = import_node(&mut graph, "#include <spdlog/spdlog.h>", "cpp");
        let cmake_include = import_node(&mut graph, "#include <cmake/tool.h>", "cpp");
        let openssl_include = import_node(&mut graph, "#include <openssl/ssl.h>", "cpp");
        graph.add_edge(file, fmt_include, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(file, zlib_include, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(
            file,
            spdlog_include,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            file,
            cmake_include,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            file,
            openssl_include,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&curl)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&fmt)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&zlib)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&spdlog)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&openssl)
        }));
        let non_runtime = report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "non_runtime_dependency_import" && insight.nodes.contains(&cmake)
            })
            .expect("expected C++ build dependency import insight");
        assert!(non_runtime.nodes.contains(&file));
        assert!(non_runtime.nodes.contains(&cmake_include));
    }

    #[test]
    fn insights_report_conflicting_dependency_declarations() {
        let mut graph = CodeGraph::new("repo");
        let root_manifest = graph.add_node(NodeKind::File, "Cargo.toml");
        let app_manifest = graph.add_node(NodeKind::File, "crates/app/Cargo.toml");
        let lockfile = graph.add_node(NodeKind::File, "Cargo.lock");
        let serde = dependency_node(&mut graph, "serde", "cargo:serde");
        let anyhow = dependency_node(&mut graph, "anyhow", "cargo:anyhow");
        graph.add_edge_with_metadata(
            root_manifest,
            serde,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "1".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            app_manifest,
            serde,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "2".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            lockfile,
            serde,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "1.2.3".to_string()),
                ("dependency_version_kind".to_string(), "locked".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            app_manifest,
            anyhow,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "1".to_string()),
            ]),
        );

        let report = insights(&graph);
        let conflict = report
            .insights
            .iter()
            .find(|insight| insight.kind == "conflicting_dependency_declaration")
            .expect("expected conflicting dependency declaration insight");

        assert_eq!(conflict.severity, InsightSeverity::Warning);
        assert!(conflict.message.contains("serde"));
        assert!(conflict.message.contains("`1`"));
        assert!(conflict.message.contains("`2`"));
        assert!(conflict.nodes.contains(&root_manifest));
        assert!(conflict.nodes.contains(&app_manifest));
        assert!(conflict.nodes.contains(&serde));
        assert!(!conflict.nodes.contains(&anyhow));
        assert_eq!(conflict.edges.len(), 2);
    }

    #[test]
    fn insights_ignore_locked_versions_as_conflicting_constraints() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "package.json");
        let lockfile = graph.add_node(NodeKind::File, "package-lock.json");
        let react = dependency_node(&mut graph, "react", "npm:react");
        graph.add_edge_with_metadata(
            manifest,
            react,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "^19.0.0".to_string()),
                (
                    "dependency_version_kind".to_string(),
                    "constraint".to_string(),
                ),
            ]),
        );
        graph.add_edge_with_metadata(
            lockfile,
            react,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "19.0.0".to_string()),
                ("dependency_version_kind".to_string(), "locked".to_string()),
            ]),
        );

        let report = insights(&graph);
        assert!(
            !report
                .insights
                .iter()
                .any(|insight| insight.kind == "conflicting_dependency_declaration")
        );
    }

    #[test]
    fn insights_report_mixed_dependency_scopes() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "package.json");
        let workspace_manifest = graph.add_node(NodeKind::File, "packages/app/package.json");
        let react = dependency_node(&mut graph, "react", "npm:react");
        let lodash = dependency_node(&mut graph, "lodash", "npm:lodash");
        graph.add_edge_with_metadata(
            manifest,
            react,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "^18".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            workspace_manifest,
            react,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "dev".to_string()),
                ("dependency_version".to_string(), "^18".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            workspace_manifest,
            lodash,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "^4".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            manifest,
            lodash,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), "^4".to_string()),
            ]),
        );

        let report = insights(&graph);
        let mixed = report
            .insights
            .iter()
            .find(|insight| insight.kind == "mixed_dependency_scope")
            .expect("expected mixed dependency scope insight");

        assert_eq!(mixed.severity, InsightSeverity::Warning);
        assert!(mixed.message.contains("react"));
        assert!(mixed.message.contains("`runtime`"));
        assert!(mixed.message.contains("`dev`"));
        assert!(mixed.nodes.contains(&manifest));
        assert!(mixed.nodes.contains(&workspace_manifest));
        assert!(mixed.nodes.contains(&react));
        assert!(!mixed.nodes.contains(&lodash));
        assert_eq!(mixed.edges.len(), 2);
        assert_eq!(report.by_kind.get("mixed_dependency_scope"), Some(&1));
        assert!(
            !report
                .insights
                .iter()
                .any(|insight| insight.kind == "conflicting_dependency_declaration")
        );
    }

    #[test]
    fn insights_report_non_runtime_dependency_imports_from_production_sources() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "package.json");
        let app = graph.add_node_with_metadata(
            NodeKind::File,
            "src/app.ts",
            None,
            BTreeMap::from([("language".to_string(), "typescript".to_string())]),
        );
        let test = graph.add_node_with_metadata(
            NodeKind::File,
            "tests/app.test.ts",
            None,
            BTreeMap::from([("language".to_string(), "typescript".to_string())]),
        );
        let spec = graph.add_node_with_metadata(
            NodeKind::File,
            "src/__tests__/setup.spec.tsx",
            None,
            BTreeMap::from([("language".to_string(), "typescript".to_string())]),
        );
        let react = dependency_node(&mut graph, "react", "npm:react");
        let vite = dependency_node(&mut graph, "vite", "npm:vite");
        graph.add_edge_with_metadata(
            manifest,
            react,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            vite,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
        );

        let app_vite_import = import_node(
            &mut graph,
            "import { defineConfig } from \"vite\";",
            "typescript",
        );
        let app_react_import =
            import_node(&mut graph, "import React from \"react\";", "typescript");
        let test_vite_import =
            import_node(&mut graph, "import { test } from \"vite\";", "typescript");
        let spec_vite_import = import_node(
            &mut graph,
            "import { defineConfig } from \"vite\";",
            "typescript",
        );
        graph.add_edge(
            app,
            app_vite_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            app,
            app_react_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            test,
            test_vite_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            spec,
            spec_vite_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "non_runtime_dependency_import")
            .expect("expected non-runtime dependency import insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("src/app.ts"));
        assert!(insight.message.contains("vite"));
        assert!(insight.message.contains("`dev`"));
        assert!(insight.nodes.contains(&app));
        assert!(insight.nodes.contains(&app_vite_import));
        assert!(insight.nodes.contains(&manifest));
        assert!(insight.nodes.contains(&vite));
        assert!(!insight.nodes.contains(&app_react_import));
        assert!(!insight.nodes.contains(&test));
        assert!(!insight.nodes.contains(&test_vite_import));
        assert!(!insight.nodes.contains(&spec));
        assert!(!insight.nodes.contains(&spec_vite_import));
        assert_eq!(
            report.by_kind.get("non_runtime_dependency_import"),
            Some(&1)
        );
    }

    #[test]
    fn insights_report_go_indirect_dependency_imports_from_production_sources() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "go.mod");
        let app = graph.add_node_with_metadata(
            NodeKind::File,
            "cmd/server/main.go",
            None,
            BTreeMap::from([("language".to_string(), "go".to_string())]),
        );
        let sys = dependency_node(&mut graph, "golang.org/x/sys", "go:golang.org/x/sys");
        graph.add_edge_with_metadata(
            manifest,
            sys,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "indirect".to_string())]),
        );

        let unix_import = import_node(&mut graph, "import \"golang.org/x/sys/unix\"", "go");
        graph.add_edge(app, unix_import, EdgeKind::Imports, Confidence::Syntactic);

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "non_runtime_dependency_import")
            .expect("expected direct import of indirect Go dependency insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("cmd/server/main.go"));
        assert!(insight.message.contains("golang.org/x/sys"));
        assert!(insight.message.contains("`indirect`"));
        assert!(insight.nodes.contains(&manifest));
        assert!(insight.nodes.contains(&app));
        assert!(insight.nodes.contains(&sys));
        assert!(insight.nodes.contains(&unix_import));
        assert_eq!(
            report.by_kind.get("non_runtime_dependency_import"),
            Some(&1)
        );
    }

    #[test]
    fn insights_report_runtime_dependencies_used_only_by_tests() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "package.json");
        let test = graph.add_node_with_metadata(
            NodeKind::File,
            "tests/app.test.ts",
            None,
            BTreeMap::from([("language".to_string(), "typescript".to_string())]),
        );
        let app = graph.add_node_with_metadata(
            NodeKind::File,
            "src/app.ts",
            None,
            BTreeMap::from([("language".to_string(), "typescript".to_string())]),
        );
        let jest = dependency_node(&mut graph, "jest", "npm:jest");
        let react = dependency_node(&mut graph, "react", "npm:react");
        let vite = dependency_node(&mut graph, "vite", "npm:vite");
        graph.add_edge_with_metadata(
            manifest,
            jest,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            react,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            vite,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
        );

        let jest_import = import_node(
            &mut graph,
            "import { describe } from \"jest\";",
            "typescript",
        );
        let react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
        let vite_import = import_node(&mut graph, "import { test } from \"vite\";", "typescript");
        graph.add_edge(test, jest_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(app, react_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(test, vite_import, EdgeKind::Imports, Confidence::Syntactic);

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "test_only_runtime_dependency")
            .expect("expected test-only runtime dependency insight");

        assert_eq!(insight.severity, InsightSeverity::Info);
        assert!(insight.message.contains("jest"));
        assert!(insight.nodes.contains(&manifest));
        assert!(insight.nodes.contains(&jest));
        assert!(insight.nodes.contains(&test));
        assert!(insight.nodes.contains(&jest_import));
        assert!(!insight.nodes.contains(&app));
        assert!(!insight.nodes.contains(&react));
        assert!(!insight.nodes.contains(&vite));
        assert_eq!(report.by_kind.get("test_only_runtime_dependency"), Some(&1));
    }

    #[test]
    fn insights_match_dart_pubspec_dependency_scopes() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "pubspec.yaml");
        let app = graph.add_node_with_metadata(
            NodeKind::File,
            "lib/main.dart",
            None,
            BTreeMap::from([("language".to_string(), "dart".to_string())]),
        );
        let test = graph.add_node_with_metadata(
            NodeKind::File,
            "test/widget_test.dart",
            None,
            BTreeMap::from([("language".to_string(), "dart".to_string())]),
        );
        let generated = graph.add_node_with_metadata(
            NodeKind::File,
            "lib/src/user.freezed.dart",
            None,
            BTreeMap::from([("language".to_string(), "dart".to_string())]),
        );
        let http = dependency_node(&mut graph, "http", "dart:http");
        let build_runner = dependency_node(&mut graph, "build_runner", "dart:build_runner");
        let test_dep = dependency_node(&mut graph, "test", "dart:test");
        let collection = dependency_node(&mut graph, "collection", "dart:collection");
        graph.add_edge_with_metadata(
            manifest,
            http,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            build_runner,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            test_dep,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
        graph.add_edge_with_metadata(
            manifest,
            collection,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );

        let http_import = import_node(&mut graph, "import 'package:http/http.dart';", "dart");
        let build_runner_import = import_node(
            &mut graph,
            "import 'package:build_runner/build_runner.dart';",
            "dart",
        );
        let test_import = import_node(&mut graph, "import 'package:test/test.dart';", "dart");
        let undeclared_import = import_node(
            &mut graph,
            "import 'package:riverpod/riverpod.dart';",
            "dart",
        );
        let sdk_import = import_node(&mut graph, "import 'dart:io';", "dart");
        let generated_build_import = import_node(
            &mut graph,
            "import 'package:build_runner/build_runner.dart';",
            "dart",
        );
        graph.add_edge(app, http_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(
            app,
            build_runner_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(test, test_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(
            app,
            undeclared_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(app, sdk_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(
            generated,
            generated_build_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("http")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("dart:io")
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import"
                && insight.message.contains("riverpod")
                && insight.nodes.contains(&undeclared_import)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&collection)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&http)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "non_runtime_dependency_import"
                && insight.nodes.contains(&build_runner_import)
                && !insight.nodes.contains(&generated_build_import)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "test_only_runtime_dependency"
                && insight.nodes.contains(&test_dep)
                && insight.nodes.contains(&test_import)
        }));
    }

    #[test]
    fn test_like_source_paths_cover_common_language_conventions() {
        for path in [
            "src/__tests__/app.spec.tsx",
            "web/components/Button.test.jsx",
            "internal/server/server_test.go",
            "tests/test_api.py",
            "tests/api_test.py",
            "src/FooTest.php",
            "src/FooSpec.php",
            "native/foo_test.cpp",
            "native/test_parser.cc",
            "scripts/deploy_test.sh",
            "scripts/deploy.bats",
            "pkg/testdata/input.go",
            "test/widget_test.dart",
            "integration_test/app_test.dart",
            "lib/src/user.g.dart",
            "lib/src/user.freezed.dart",
            "lib/generated/assets.gen.dart",
            ".dart_tool/build/generated/app/lib/main.dart",
        ] {
            assert!(
                is_test_like_source_path(path),
                "expected test-like path: {path}"
            );
        }

        for path in [
            "src/app.ts",
            "src/context.php",
            "src/contest.php",
            "cmd/server/main.go",
            "native/parser.cpp",
            "scripts/deploy.sh",
            "lib/main.dart",
            "lib/src/user.dart",
        ] {
            assert!(
                !is_test_like_source_path(path),
                "expected production-like path: {path}"
            );
        }
    }

    #[test]
    fn insights_report_duplicate_framework_routes() {
        let mut graph = CodeGraph::new("repo");
        let first = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route GET /users",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("method".to_string(), "GET".to_string()),
                ("path".to_string(), "/users".to_string()),
                ("handler".to_string(), "list_users".to_string()),
            ]),
        );
        let second = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route GET /users",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("method".to_string(), "GET".to_string()),
                ("path".to_string(), "/users".to_string()),
                ("handler".to_string(), "legacy_users".to_string()),
            ]),
        );
        let post = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route POST /users",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("method".to_string(), "POST".to_string()),
                ("path".to_string(), "/users".to_string()),
                ("handler".to_string(), "create_user".to_string()),
            ]),
        );
        let list_users = graph.add_node(NodeKind::Function, "list_users");
        let legacy_users = graph.add_node(NodeKind::Function, "legacy_users");
        graph.add_edge(
            first,
            list_users,
            EdgeKind::References,
            Confidence::Syntactic,
        );
        graph.add_edge(
            second,
            legacy_users,
            EdgeKind::References,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        let duplicate = report
            .insights
            .iter()
            .find(|insight| insight.kind == "duplicate_framework_route")
            .expect("expected duplicate route insight");

        assert_eq!(duplicate.severity, InsightSeverity::Warning);
        assert!(duplicate.message.contains("GET /users"));
        assert!(duplicate.message.contains("list_users"));
        assert!(duplicate.message.contains("legacy_users"));
        assert!(duplicate.nodes.contains(&first));
        assert!(duplicate.nodes.contains(&second));
        assert!(!duplicate.nodes.contains(&post));
        assert_eq!(duplicate.edges.len(), 2);
    }

    #[test]
    fn insights_report_unresolved_framework_route_handlers() {
        let mut graph = CodeGraph::new("repo");
        let unresolved = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route GET /missing",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("framework".to_string(), "fastapi".to_string()),
                ("method".to_string(), "GET".to_string()),
                ("path".to_string(), "/missing".to_string()),
                ("handler".to_string(), "missing_handler".to_string()),
            ]),
        );
        let resolved = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route POST /users",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("framework".to_string(), "fastapi".to_string()),
                ("method".to_string(), "POST".to_string()),
                ("path".to_string(), "/users".to_string()),
                ("handler".to_string(), "create_user".to_string()),
            ]),
        );
        let inline = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            "route GET /inline",
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("framework".to_string(), "express".to_string()),
                ("method".to_string(), "GET".to_string()),
                ("path".to_string(), "/inline".to_string()),
            ]),
        );
        let file = graph.add_node(NodeKind::File, "api.py");
        let handler = graph.add_node(NodeKind::Function, "create_user");
        graph.add_edge(
            graph.root,
            unresolved,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
        );
        graph.add_edge(
            graph.root,
            resolved,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
        );
        graph.add_edge(
            graph.root,
            inline,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
        );
        graph.add_edge_with_metadata(
            unresolved,
            file,
            EdgeKind::References,
            Confidence::Syntactic,
            BTreeMap::from([("resolution".to_string(), "framework_route_file".to_string())]),
        );
        graph.add_edge_with_metadata(
            resolved,
            handler,
            EdgeKind::References,
            Confidence::Syntactic,
            BTreeMap::from([(
                "resolution".to_string(),
                "framework_route_handler".to_string(),
            )]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "unresolved_framework_route_handler")
            .expect("expected unresolved route handler insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("GET /missing"));
        assert!(insight.message.contains("missing_handler"));
        assert!(insight.nodes.contains(&unresolved));
        assert!(!insight.nodes.contains(&resolved));
        assert!(!insight.nodes.contains(&inline));
        assert_eq!(insight.edges.len(), 2);
        assert_eq!(
            report.by_kind.get("unresolved_framework_route_handler"),
            Some(&1)
        );
    }

    #[test]
    fn insights_report_custom_rule_violations() {
        let mut graph = CodeGraph::new("repo");
        let caller = graph.add_node(NodeKind::Function, "render");
        let callee = graph.add_node(NodeKind::Function, "query_user");
        graph.add_edge(caller, callee, EdgeKind::Calls, Confidence::Heuristic);
        let violated_edge_index = graph.edges.len() - 1;
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "custom_rule_violation".to_string());
        metadata.insert("rule_id".to_string(), "ui-cannot-call-db".to_string());
        metadata.insert("rule_kind".to_string(), "forbidden_edge".to_string());
        metadata.insert("severity".to_string(), "error".to_string());
        metadata.insert(
            "message".to_string(),
            "UI layer must not call database layer directly".to_string(),
        );
        metadata.insert(
            "violated_edge_index".to_string(),
            violated_edge_index.to_string(),
        );
        let violation = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "custom rule violation:no-left-pad",
            None,
            metadata,
        );
        graph.add_edge(violation, caller, EdgeKind::References, Confidence::Exact);
        graph.add_edge(violation, callee, EdgeKind::References, Confidence::Exact);

        let report = insights(&graph);
        let custom = report
            .insights
            .iter()
            .find(|insight| insight.kind == "custom_rule_forbidden_edge")
            .expect("expected custom rule insight");

        assert_eq!(custom.severity, InsightSeverity::Error);
        assert_eq!(
            custom.message,
            "UI layer must not call database layer directly"
        );
        assert_eq!(custom.nodes, vec![violation]);
        assert!(custom.edges.contains(&violated_edge_index));
        assert_eq!(custom.edges.len(), 3);
    }

    #[test]
    fn filter_insight_report_filters_and_limits_results() {
        let report = InsightReport {
            total: 3,
            by_severity: BTreeMap::from([("error".to_string(), 1), ("warning".to_string(), 2)]),
            by_kind: BTreeMap::from([
                ("dependency_cycle".to_string(), 1),
                ("parse_error".to_string(), 1),
                ("undeclared_external_import".to_string(), 1),
            ]),
            insights: vec![
                Insight {
                    kind: "dependency_cycle".to_string(),
                    severity: InsightSeverity::Warning,
                    message: "cycle through service".to_string(),
                    nodes: vec![NodeId(1)],
                    edges: vec![10],
                },
                Insight {
                    kind: "undeclared_external_import".to_string(),
                    severity: InsightSeverity::Warning,
                    message: "imports express".to_string(),
                    nodes: vec![NodeId(2)],
                    edges: vec![11],
                },
                Insight {
                    kind: "parse_error".to_string(),
                    severity: InsightSeverity::Error,
                    message: "broken file".to_string(),
                    nodes: vec![NodeId(3)],
                    edges: Vec::new(),
                },
            ],
        };

        let filtered = filter_insight_report(
            report,
            &InsightFilter {
                severity: Some(InsightSeverity::Warning),
                kind: Some("dependency".to_string()),
                search: Some("cycle".to_string()),
                limit: 1,
            },
        );

        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.by_severity.get("error"), None);
        assert_eq!(filtered.by_severity.get("warning"), Some(&1));
        assert_eq!(filtered.by_kind.get("dependency_cycle"), Some(&1));
        assert_eq!(filtered.by_kind.get("parse_error"), None);
        assert_eq!(filtered.insights.len(), 1);
        assert_eq!(filtered.insights[0].kind, "dependency_cycle");
    }

    #[test]
    fn check_insights_respects_severity_thresholds() {
        let report = InsightReport {
            total: 6,
            by_severity: BTreeMap::from([
                ("info".to_string(), 3),
                ("warning".to_string(), 2),
                ("error".to_string(), 1),
            ]),
            by_kind: BTreeMap::new(),
            insights: Vec::new(),
        };

        let error_check = check_insights(report.clone(), InsightSeverity::Error);
        assert!(!error_check.passed);
        assert_eq!(error_check.fail_on, "error");
        assert_eq!(error_check.failing_insights, 1);

        let warning_check = check_insights(report.clone(), InsightSeverity::Warning);
        assert!(!warning_check.passed);
        assert_eq!(warning_check.failing_insights, 3);

        let clean_report = InsightReport {
            total: 3,
            by_severity: BTreeMap::from([("info".to_string(), 3)]),
            by_kind: BTreeMap::new(),
            insights: Vec::new(),
        };
        let clean_check = check_insights(clean_report, InsightSeverity::Warning);
        assert!(clean_check.passed);
        assert_eq!(clean_check.failing_insights, 0);
    }

    #[test]
    fn insights_report_skipped_large_files() {
        let mut graph = CodeGraph::new("repo");
        let mut metadata = BTreeMap::new();
        metadata.insert("skipped".to_string(), "true".to_string());
        metadata.insert("skipped_reason".to_string(), "max_file_size".to_string());
        metadata.insert("file_size_bytes".to_string(), "8192".to_string());
        metadata.insert("max_file_size_bytes".to_string(), "4096".to_string());
        let file = graph.add_node_with_metadata(NodeKind::File, "src/huge.rs", None, metadata);

        let summary = summarize(&graph);
        assert_eq!(summary.skipped_files, 1);

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "skipped_large_file")
            .expect("expected skipped large file insight");

        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert!(insight.message.contains("src/huge.rs"));
        assert!(insight.message.contains("8192"));
        assert!(insight.nodes.contains(&file));
    }

    #[test]
    fn insights_report_semantic_diagnostics() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/main.rs");
        let diagnostic = graph.add_node_with_metadata(
            NodeKind::Unknown,
            "error: semantic mismatch",
            Some(SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 3,
                start_column: 9,
                end_line: 3,
                end_column: 10,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "diagnostic".to_string()),
                ("source".to_string(), "lsp".to_string()),
                ("severity".to_string(), "error".to_string()),
                ("diagnostic_source".to_string(), "rustc".to_string()),
                ("diagnostic_code".to_string(), "E0001".to_string()),
                ("message".to_string(), "semantic mismatch".to_string()),
                ("path".to_string(), "src/main.rs".to_string()),
                ("line".to_string(), "3".to_string()),
                ("column".to_string(), "9".to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            file,
            diagnostic,
            EdgeKind::MayError,
            Confidence::Semantic,
            BTreeMap::from([("relation".to_string(), "diagnostic".to_string())]),
        );

        let report = insights(&graph);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.kind == "semantic_diagnostic")
            .expect("expected semantic diagnostic insight");

        assert_eq!(insight.severity, InsightSeverity::Error);
        assert_eq!(insight.nodes, vec![diagnostic, file]);
        assert_eq!(insight.edges, vec![0]);
        assert!(insight.message.contains("rustc error"));
        assert!(insight.message.contains("src/main.rs:3:9"));
        assert!(insight.message.contains("E0001"));
        assert_eq!(report.by_severity.get("error"), Some(&1));
        assert_eq!(report.by_kind.get("semantic_diagnostic"), Some(&1));

        let card = node_card(&graph, None, file, 10, 1, 10)
            .unwrap()
            .expect("expected file card");
        assert!(
            card.insights
                .iter()
                .any(|insight| insight.kind == "semantic_diagnostic")
        );
    }

    #[test]
    fn insights_match_php_composer_namespace_imports() {
        let mut graph = CodeGraph::new("repo");
        let manifest = graph.add_node(NodeKind::File, "composer.json");
        let app = graph.add_node_with_metadata(
            NodeKind::File,
            "src/App.php",
            None,
            BTreeMap::from([("language".to_string(), "php".to_string())]),
        );
        let test = graph.add_node_with_metadata(
            NodeKind::File,
            "tests/AppTest.php",
            None,
            BTreeMap::from([("language".to_string(), "php".to_string())]),
        );

        let monolog = dependency_node(&mut graph, "monolog/monolog", "composer:monolog/monolog");
        let symfony_console =
            dependency_node(&mut graph, "symfony/console", "composer:symfony/console");
        let phpunit = dependency_node(&mut graph, "phpunit/phpunit", "composer:phpunit/phpunit");
        let doctrine = dependency_node(&mut graph, "doctrine/orm", "composer:doctrine/orm");
        for dependency in [monolog, symfony_console, doctrine] {
            graph.add_edge_with_metadata(
                manifest,
                dependency,
                EdgeKind::DependsOn,
                Confidence::Exact,
                BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
            );
        }
        graph.add_edge_with_metadata(
            manifest,
            phpunit,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
        );

        let monolog_import = import_node(&mut graph, "use Monolog\\Logger;", "php");
        let symfony_import = import_node(
            &mut graph,
            "use Symfony\\Component\\Console\\Application;",
            "php",
        );
        let phpunit_app_import =
            import_node(&mut graph, "use PHPUnit\\Framework\\TestCase;", "php");
        let phpunit_test_import =
            import_node(&mut graph, "use PHPUnit\\Framework\\TestCase;", "php");
        let undeclared_import = import_node(
            &mut graph,
            "use Acme\\Missing\\Client as MissingClient;",
            "php",
        );
        let local_import = import_node(&mut graph, "use App\\Domain\\Service;", "php");
        let builtin_import = import_node(&mut graph, "use DateTimeImmutable;", "php");

        graph.add_edge(
            app,
            monolog_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            app,
            symfony_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            app,
            phpunit_app_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            test,
            phpunit_test_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(
            app,
            undeclared_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(app, local_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(
            app,
            builtin_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );

        let report = insights(&graph);
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&monolog)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&symfony_console)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "unused_declared_dependency" && insight.nodes.contains(&doctrine)
        }));
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import"
                && insight.message.contains("acme/missing")
                && insight.nodes.contains(&undeclared_import)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.nodes.contains(&local_import)
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.nodes.contains(&builtin_import)
        }));
        let non_runtime = report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "non_runtime_dependency_import" && insight.nodes.contains(&phpunit)
            })
            .expect("expected PHP production import of dev Composer dependency");
        assert!(non_runtime.message.contains("phpunit/phpunit"));
        assert!(non_runtime.nodes.contains(&app));
        assert!(non_runtime.nodes.contains(&phpunit_app_import));
        assert!(!non_runtime.nodes.contains(&test));
        assert!(!non_runtime.nodes.contains(&phpunit_test_import));
    }

    #[test]
    fn insights_match_cargo_python_and_go_import_conventions() {
        let mut graph = CodeGraph::new("repo");
        let rust_file = graph.add_node(NodeKind::File, "src/lib.rs");
        let python_file = graph.add_node(NodeKind::File, "app.py");
        let go_file = graph.add_node(NodeKind::File, "main.go");

        let serde_json = dependency_node(&mut graph, "serde-json", "cargo:serde-json");
        let fastapi = dependency_node(&mut graph, "fastapi", "python:fastapi");
        let gin = dependency_node(
            &mut graph,
            "github.com/gin-gonic/gin",
            "go:github.com/gin-gonic/gin",
        );
        graph.add_edge(
            rust_file,
            serde_json,
            EdgeKind::DependsOn,
            Confidence::Exact,
        );
        graph.add_edge(python_file, fastapi, EdgeKind::DependsOn, Confidence::Exact);
        graph.add_edge(go_file, gin, EdgeKind::DependsOn, Confidence::Exact);

        for (file, label, language) in [
            (rust_file, "use serde_json::Value;", "rust"),
            (rust_file, "use anyhow::Result;", "rust"),
            (rust_file, "use std::fs;", "rust"),
            (python_file, "from fastapi import FastAPI", "python"),
            (python_file, "import requests", "python"),
            (python_file, "import os", "python"),
            (go_file, "import \"github.com/gin-gonic/gin/binding\"", "go"),
            (go_file, "import \"github.com/pkg/errors\"", "go"),
            (go_file, "import \"fmt\"", "go"),
        ] {
            let import = import_node(&mut graph, label, language);
            graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
        }

        let report = insights(&graph);
        for expected in ["anyhow", "requests", "github.com/pkg/errors"] {
            assert!(
                report.insights.iter().any(|insight| {
                    insight.kind == "undeclared_external_import"
                        && insight.message.contains(expected)
                }),
                "missing undeclared import insight for {expected}"
            );
        }
        for ignored in [
            "serde_json",
            "fastapi",
            "github.com/gin-gonic/gin/binding",
            "std::fs",
            "os",
            "fmt",
        ] {
            assert!(
                !report.insights.iter().any(|insight| {
                    insight.kind == "undeclared_external_import"
                        && insight.message.contains(ignored)
                }),
                "unexpected undeclared import insight for {ignored}"
            );
        }
    }

    fn import_node(graph: &mut CodeGraph, label: &str, language: &str) -> NodeId {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "import".to_string());
        metadata.insert("language".to_string(), language.to_string());
        graph.add_node_with_metadata(NodeKind::ExternalDependency, label, None, metadata)
    }

    fn dependency_node(graph: &mut CodeGraph, label: &str, package_id: &str) -> NodeId {
        let mut metadata = BTreeMap::new();
        metadata.insert("item_kind".to_string(), "dependency".to_string());
        metadata.insert("package_id".to_string(), package_id.to_string());
        graph.add_node_with_metadata(NodeKind::ExternalDependency, label, None, metadata)
    }

    fn temp_analysis_root() -> std::path::PathBuf {
        static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let process_id = std::process::id();
        std::env::temp_dir().join(format!(
            "codegraph-analysis-test-{process_id}-{counter}-{nanos}"
        ))
    }
}
