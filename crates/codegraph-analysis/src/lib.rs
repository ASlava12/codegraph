use codegraph_core::{CodeGraph, Edge, EdgeKind, Node, NodeId, NodeKind};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
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
pub struct HotspotReport {
    pub hotspots: Vec<Hotspot>,
    pub total_candidates: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hotspot {
    pub node: Node,
    pub score: usize,
    pub incoming: usize,
    pub outgoing: usize,
    pub edge_kinds: BTreeMap<String, usize>,
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
    pub returned_nodes: usize,
    #[serde(default)]
    pub returned_edges: usize,
    pub truncated: bool,
    #[serde(default)]
    pub facets: QueryFacets,
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
    "semantic_diagnostic",
    "sensitive_config_default",
    "skipped_large_file",
    "syntax_error",
    "test_only_runtime_dependency",
    "undeclared_external_import",
    "unreachable_config_read",
    "unreachable_error_flow",
    "unreachable_source_file",
    "unresolved_call",
    "unresolved_entrypoint_target",
    "unresolved_framework_route_handler",
    "unresolved_local_import",
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
pub const DEFAULT_REPORT_INSIGHT_LIMIT: usize = 50;
pub const MAX_REPORT_INSIGHT_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReportLimits {
    pub architecture_group_limit: usize,
    pub architecture_edge_limit: usize,
    pub language_link_limit: usize,
    pub hotspot_limit: usize,
    pub insight_limit: usize,
    pub fail_on: InsightSeverity,
}

impl Default for ProjectReportLimits {
    fn default() -> Self {
        Self {
            architecture_group_limit: DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
            architecture_edge_limit: DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT,
            language_link_limit: DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
            hotspot_limit: DEFAULT_REPORT_HOTSPOT_LIMIT,
            insight_limit: DEFAULT_REPORT_INSIGHT_LIMIT,
            fail_on: InsightSeverity::Error,
        }
    }
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
    pub hotspots: HotspotReport,
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

pub fn hotspots(graph: &CodeGraph, limit: usize) -> HotspotReport {
    let limit = limit.clamp(1, 500);
    let mut hotspots = hotspot_stats(graph, |_| true, NeighborDirection::Both);
    let total_candidates = hotspots.len();
    hotspots.truncate(limit);

    HotspotReport {
        hotspots,
        total_candidates,
        truncated: total_candidates > limit,
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
    add_cross_language_heuristic_edge_insights(graph, &mut insights);
    add_duplicate_function_insights(graph, &mut insights);
    add_duplicate_entrypoint_insights(graph, &mut insights);
    add_ambiguous_entrypoint_target_insights(graph, &mut insights);
    add_orphan_function_insights(graph, &mut insights);
    add_error_flow_insights(graph, &mut insights);
    add_unresolved_entrypoint_insights(graph, &mut insights);
    add_entrypoint_dead_end_insights(graph, &mut insights);
    add_unreachable_config_read_insights(graph, &mut insights);
    add_unreachable_error_flow_insights(graph, &mut insights);
    add_unreachable_source_file_insights(graph, &mut insights);
    add_conflicting_config_default_insights(graph, &mut insights);
    add_mixed_config_requirement_insights(graph, &mut insights);
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
        hotspots: hotspots(graph, limits.hotspot_limit),
    }
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
        insight_limit: limits.insight_limit.clamp(1, MAX_REPORT_INSIGHT_LIMIT),
        fail_on: limits.fail_on,
    }
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
            "unknown query command `{other}`; expected nodes, edges, calls, dependencies, trace, dependents, neighbors, symbols, files, entrypoints, routes, packages, configs, errors, cycles, hotspots, unreachable, diagnostics, annotations, insights, or path"
        ))),
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
        .filter_map(|package_id| package_id.split_once(':').map(|(ecosystem, _)| ecosystem))
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
        let Some(import) = import_package_candidate(language, &import_node.label) else {
            continue;
        };
        if !declared_ecosystems.contains(import.ecosystem.as_str()) {
            continue;
        }
        if is_declared_package(&declared, &import.ecosystem, &import.package) {
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
        "npm" => package == import.package.to_ascii_lowercase(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TraceDirection {
    Outgoing,
    Incoming,
}

fn trace_edges_from(
    graph: &CodeGraph,
    node_id: NodeId,
    direction: TraceDirection,
) -> impl Iterator<Item = &Edge> {
    graph.edges.iter().filter(move |edge| {
        is_trace_edge(&edge.kind)
            && match direction {
                TraceDirection::Outgoing => edge.source == node_id,
                TraceDirection::Incoming => edge.target == node_id,
            }
    })
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
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    normalized.split('/').any(|part| {
        matches!(
            part,
            "test"
                | "tests"
                | "__tests__"
                | "spec"
                | "specs"
                | "fixture"
                | "fixtures"
                | "example"
                | "examples"
                | "sample"
                | "samples"
                | "mock"
                | "mocks"
        )
    }) || file_name.ends_with("_test.go")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_test.py")
        || file_name.ends_with(".test.js")
        || file_name.ends_with(".test.ts")
        || file_name.ends_with(".spec.js")
        || file_name.ends_with(".spec.ts")
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
                insight_limit: 1,
                fail_on: InsightSeverity::Warning,
            },
        );

        assert_eq!(report.graph_schema_version, graph.schema_version);
        assert_eq!(report.summary.nodes, graph.nodes.len());
        assert_eq!(report.entrypoints.len(), 1);
        assert_eq!(report.hotspots.hotspots.len(), 1);
        assert_eq!(report.quality_gate.fail_on, "warning");
        assert_eq!(report.insights.insights.len(), 1);
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
        assert_eq!(report.hotspots[0].edge_kinds.get("calls"), Some(&2));
        assert_eq!(report.hotspots[0].edge_kinds.get("reads_config"), Some(&1));
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
        let env = graph.add_node(NodeKind::Environment, "DATABASE_URL");
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
        assert_eq!(card.total_insights, 2);
        assert_eq!(card.insight_summary.by_severity.get("warning"), Some(&1));
        assert_eq!(card.insight_summary.by_severity.get("info"), Some(&1));
        assert_eq!(
            card.insight_summary.by_kind.get("orphan_function"),
            Some(&1)
        );
        assert_eq!(
            card.insight_summary.by_kind.get("potential_error_flow"),
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
        assert_eq!(file_card.total_insights, 2);
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
