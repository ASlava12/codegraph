//! Focused subgraphs: focus sets, edge explanation, and canvas slices.

use codegraph_core::{CodeGraph, Edge};
use std::collections::BTreeSet;

#[allow(unused_imports)]
use crate::*;

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

pub(crate) fn edge_related_insights(graph: &CodeGraph, edge_index: usize) -> Vec<Insight> {
    insights(graph)
        .insights
        .into_iter()
        .filter(|insight| insight.edges.contains(&edge_index))
        .collect()
}

pub(crate) fn edge_with_index(edge_index: usize, edge: &Edge) -> Edge {
    let mut indexed = edge.clone();
    indexed
        .metadata
        .insert(EDGE_INDEX_METADATA_KEY.to_string(), edge_index.to_string());
    indexed
}

pub(crate) fn edges_with_indexes(graph: &CodeGraph, edges: Vec<Edge>) -> Vec<Edge> {
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
