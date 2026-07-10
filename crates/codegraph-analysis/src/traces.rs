//! Workflow Mermaid rendering plus config and error trace reports.

use codegraph_core::{CodeGraph, EdgeKind, NodeKind};

#[allow(unused_imports)]
use crate::*;

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
