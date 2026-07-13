//! Outgoing traces and workflow block reports from labels, entrypoints,
//! and query results.

use codegraph_core::CodeGraph;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[allow(unused_imports)]
use crate::*;

pub fn trace(graph: &CodeGraph, request: TraceRequest) -> Option<TraceResult> {
    trace_with_direction(graph, request, TraceDirection::Outgoing)
}

pub fn trace_dependents(graph: &CodeGraph, request: TraceRequest) -> Option<TraceResult> {
    trace_with_direction(graph, request, TraceDirection::Incoming)
}

pub(crate) fn trace_with_direction(
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
    let entrypoint_kind = request
        .entrypoint_kind
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let matched = entrypoints(graph)
        .into_iter()
        .filter(|node| search.is_none_or(|expected| node_search_matches(node, expected)))
        .filter(|node| {
            entrypoint_kind
                .as_deref()
                .is_none_or(|expected| entrypoint_kind_matches(node, expected))
        })
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
                    max_fanout: request.max_fanout,
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
        entrypoint_kind,
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
                    max_fanout: None,
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
