//! Shared graph helpers: edge indexes, workflow filters, path/area
//! classification, and confidence naming.

use codegraph_core::{CodeGraph, Edge, EdgeKind, Node, NodeId, NodeKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn incoming_edge_indexes(
    graph: &CodeGraph,
    target: NodeId,
    kind: EdgeKind,
) -> Vec<usize> {
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

pub(crate) fn outgoing_edge_indexes(
    graph: &CodeGraph,
    source: NodeId,
    kind: EdgeKind,
) -> Vec<usize> {
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

pub(crate) fn is_cycle_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls | EdgeKind::References | EdgeKind::Imports | EdgeKind::DependsOn
    )
}

pub(crate) fn is_trace_edge(kind: &EdgeKind) -> bool {
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

pub(crate) fn normalize_workflow_filters(filters: WorkflowFilters) -> WorkflowFilters {
    WorkflowFilters {
        edge_kind: normalize_workflow_filter(filters.edge_kind),
        confidence: normalize_workflow_filter(filters.confidence),
        language: normalize_workflow_filter(filters.language),
        risk_severity: normalize_workflow_filter(filters.risk_severity),
        block_kind: normalize_workflow_filter(filters.block_kind),
    }
}

pub(crate) fn normalize_workflow_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn workflow_included_node_ids(
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

pub(crate) fn workflow_edge_filter_matches(edge: &Edge, filters: &WorkflowFilters) -> bool {
    filters
        .edge_kind
        .as_deref()
        .is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
        && filters
            .confidence
            .as_deref()
            .is_none_or(|expected| text_matches(&confidence_name(edge.confidence), expected))
}

pub(crate) fn workflow_block_filter_matches(
    block: &WorkflowBlock,
    filters: &WorkflowFilters,
) -> bool {
    workflow_block_non_risk_filters_match(block, filters)
        && filters.risk_severity.as_deref().is_none_or(|expected| {
            block
                .risk_refs
                .iter()
                .any(|risk| text_matches(severity_name(risk.severity), expected))
        })
}

pub(crate) fn workflow_block_non_risk_filters_match(
    block: &WorkflowBlock,
    filters: &WorkflowFilters,
) -> bool {
    filters
        .language
        .as_deref()
        .is_none_or(|expected| workflow_node_language_matches(&block.node, expected))
        && filters.block_kind.as_deref().is_none_or(|expected| {
            text_matches(&workflow_block_kind_filter_name(&block.kind), expected)
                || text_matches(workflow_block_kind_label(&block.kind), expected)
        })
}

pub(crate) fn workflow_node_language_matches(node: &Node, expected: &str) -> bool {
    node.metadata
        .get("language")
        .is_some_and(|language| text_matches(language, expected))
}

pub(crate) fn workflow_transition_filter_matches(
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

pub(crate) fn workflow_block_id(id: NodeId) -> String {
    format!("wb-{}", id.0)
}

pub(crate) fn workflow_block_kind(
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
        Some("return") => return WorkflowBlockKind::Return,
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

pub(crate) fn workflow_block_kind_label(kind: &WorkflowBlockKind) -> &'static str {
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
        WorkflowBlockKind::Return => "return",
        WorkflowBlockKind::Error => "error",
        WorkflowBlockKind::Reference => "reference",
        WorkflowBlockKind::ExternalBoundary => "external",
        WorkflowBlockKind::Unknown => "node",
    }
}

pub(crate) fn workflow_block_kind_filter_name(kind: &WorkflowBlockKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| workflow_block_kind_label(kind).to_string())
}

pub(crate) fn workflow_risk_refs_for_node(
    report: &InsightReport,
    node_id: NodeId,
) -> Vec<WorkflowRiskRef> {
    report
        .insights
        .iter()
        .enumerate()
        .filter(|(_, insight)| insight.nodes.contains(&node_id))
        .take(8)
        .map(|(insight_index, insight)| workflow_risk_ref(insight_index, insight))
        .collect()
}

pub(crate) fn workflow_risk_refs_for_edge(
    report: &InsightReport,
    edge_index: usize,
) -> Vec<WorkflowRiskRef> {
    report
        .insights
        .iter()
        .enumerate()
        .filter(|(_, insight)| insight.edges.contains(&edge_index))
        .take(8)
        .map(|(insight_index, insight)| workflow_risk_ref(insight_index, insight))
        .collect()
}

pub(crate) fn workflow_risk_ref(insight_index: usize, insight: &Insight) -> WorkflowRiskRef {
    WorkflowRiskRef {
        insight_index,
        kind: insight.kind.clone(),
        severity: insight.severity,
        message: insight.message.clone(),
        edge_indexes: insight.edges.clone(),
    }
}

pub(crate) fn mermaid_report_block_id(id: &str) -> String {
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

pub(crate) fn mermaid_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '|'], " ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraceDirection {
    Outgoing,
    Incoming,
}

pub(crate) fn trace_edges_from_indexed(
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

pub(crate) fn trace_edges_from(
    graph: &CodeGraph,
    node_id: NodeId,
    direction: TraceDirection,
) -> impl Iterator<Item = &Edge> {
    trace_edges_from_indexed(graph, node_id, direction).map(|(_, edge)| edge)
}

pub(crate) fn trace_next_node(edge: &Edge, node_id: NodeId, direction: TraceDirection) -> NodeId {
    match direction {
        TraceDirection::Outgoing => edge.target,
        TraceDirection::Incoming => {
            debug_assert_eq!(edge.target, node_id);
            edge.source
        }
    }
}

pub(crate) fn entrypoint_reachable_nodes(graph: &CodeGraph) -> BTreeSet<NodeId> {
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();

    // One adjacency pass keeps the BFS O(V + E) instead of rescanning every
    // edge per visited node (audit F11).
    let mut outgoing: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.kind == EdgeKind::Entrypoint && reachable.insert(edge.target) {
            queue.push_back(edge.target);
        }
        if is_trace_edge(&edge.kind) {
            outgoing.entry(edge.source).or_default().push(edge.target);
        }
    }

    while let Some(node) = queue.pop_front() {
        for target in outgoing.get(&node).map(Vec::as_slice).unwrap_or(&[]) {
            if reachable.insert(*target) {
                queue.push_back(*target);
            }
        }
    }

    reachable
}

pub(crate) fn is_source_file_candidate(graph: &CodeGraph, node: &Node) -> bool {
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

pub(crate) fn file_has_reachable_code(
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

pub(crate) fn contained_code_edge_indexes(graph: &CodeGraph, file_id: NodeId) -> Vec<usize> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            (edge.source == file_id && edge.kind == EdgeKind::Contains).then_some(index)
        })
        .collect()
}

pub(crate) fn is_code_symbol(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function | NodeKind::Type | NodeKind::Module | NodeKind::Entrypoint
    )
}

pub(crate) fn is_test_like_source_path(path: &str) -> bool {
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
        || stem == "tests"
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

pub(crate) fn is_dependency_manifest_source_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    matches!(
        normalized.rsplit('/').next().unwrap_or(normalized.as_str()),
        "setup.py"
    )
}

pub(crate) fn node_label(graph: &CodeGraph, id: NodeId) -> Option<&str> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| node.label.as_str())
}

pub(crate) fn kind_name(kind: &codegraph_core::NodeKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| format!("{kind:?}").to_ascii_lowercase())
}

/// Stable community id for a repository-relative file path, matching the
/// ids used by [`communities`] (`area:<top-level-directory>`).
pub fn community_id_for_path(path: &str) -> String {
    format!("area:{}", architecture_group_for_path(path).0)
}

pub(crate) fn architecture_group_for_path(path: &str) -> (String, String) {
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

pub(crate) fn node_architecture_areas(
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

pub(crate) fn is_architecture_symbol(kind: &NodeKind) -> bool {
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

pub(crate) fn is_architecture_dependency_edge(kind: &EdgeKind) -> bool {
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

pub(crate) fn is_hotspot_candidate(kind: &NodeKind) -> bool {
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

pub(crate) fn node_language(nodes_by_id: &BTreeMap<NodeId, &Node>, id: NodeId) -> String {
    nodes_by_id
        .get(&id)
        .and_then(|node| node.metadata.get("language"))
        .map(|language| language.trim())
        .filter(|language| !language.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) fn edge_kind_name(kind: &EdgeKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| format!("{kind:?}").to_ascii_lowercase())
}

pub(crate) fn confidence_name(confidence: codegraph_core::Confidence) -> String {
    serde_json_name(&confidence).unwrap_or_else(|| format!("{confidence:?}").to_ascii_lowercase())
}

pub(crate) fn severity_name(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Info => "info",
        InsightSeverity::Warning => "warning",
        InsightSeverity::Error => "error",
    }
}

pub(crate) fn dot_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub(crate) fn dot_color(kind: &NodeKind) -> &'static str {
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
        NodeKind::ControlFlow => "#9aa7d8",
        NodeKind::Unknown => "#a5adb3",
    }
}

pub(crate) fn serde_json_name<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
}
