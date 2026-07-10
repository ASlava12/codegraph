//! Node context and investigation cards with source previews.

use codegraph_core::{CodeGraph, Edge, EdgeKind, Node, NodeId, NodeKind};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

#[allow(unused_imports)]
use crate::*;

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
    request: NodeCardRequest,
) -> io::Result<Option<NodeCard>> {
    let NodeCardRequest {
        node_id,
        edge_limit,
        source_context,
        insight_limit,
    } = request;
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

pub(crate) fn node_insight_summary(insights: &[Insight]) -> NodeInsightSummary {
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

pub(crate) fn node_card_related_insights(graph: &CodeGraph, node: &Node) -> Vec<Insight> {
    let path_index = (node.kind == NodeKind::File).then(|| node_path_index(graph));
    let nodes_by_id = nodes_by_id_index(graph);
    insights(graph)
        .insights
        .into_iter()
        .filter(|insight| {
            node_card_insight_matches(&nodes_by_id, node, path_index.as_ref(), insight)
        })
        .collect()
}

pub(crate) fn node_card_insight_matches(
    nodes_by_id: &BTreeMap<NodeId, &Node>,
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
        nodes_by_id
            .get(node_id)
            .is_some_and(|candidate| node_path_matches(candidate, path_index, &node.label))
    })
}

/// Build the `NodeId -> &Node` lookup that the indexed report helpers reuse
/// across every file/node instead of rescanning `graph.nodes` per call.
pub(crate) fn nodes_by_id_index(graph: &CodeGraph) -> BTreeMap<NodeId, &Node> {
    graph.nodes.iter().map(|node| (node.id, node)).collect()
}

/// Group edges by their source node so file summaries can look up a node's
/// outgoing edges directly instead of rescanning `graph.edges` per file.
pub(crate) fn outgoing_edge_index(graph: &CodeGraph) -> BTreeMap<NodeId, Vec<&Edge>> {
    let mut index: BTreeMap<NodeId, Vec<&Edge>> = BTreeMap::new();
    for edge in &graph.edges {
        index.entry(edge.source).or_default().push(edge);
    }
    index
}

/// Group edges by both endpoints so dependency summaries can look up a node's
/// incident edges directly instead of rescanning `graph.edges` per node. A
/// self-loop is stored once so it is not double-counted.
pub(crate) fn incident_edge_index(graph: &CodeGraph) -> BTreeMap<NodeId, Vec<&Edge>> {
    let mut index: BTreeMap<NodeId, Vec<&Edge>> = BTreeMap::new();
    for edge in &graph.edges {
        index.entry(edge.source).or_default().push(edge);
        if edge.target != edge.source {
            index.entry(edge.target).or_default().push(edge);
        }
    }
    index
}

pub(crate) fn file_node_summary(graph: &CodeGraph, node: &Node) -> Option<FileNodeSummary> {
    let nodes_by_id = nodes_by_id_index(graph);
    let outgoing = outgoing_edge_index(graph);
    file_node_summary_indexed(&nodes_by_id, &outgoing, node)
}

pub(crate) fn file_node_summary_indexed(
    nodes_by_id: &BTreeMap<NodeId, &Node>,
    outgoing_edges: &BTreeMap<NodeId, Vec<&Edge>>,
    node: &Node,
) -> Option<FileNodeSummary> {
    if node.kind != NodeKind::File {
        return None;
    }

    let mut summary = FileNodeSummary::default();
    let mut contained_code_ids = BTreeSet::new();
    let no_edges: Vec<&Edge> = Vec::new();

    for edge in outgoing_edges.get(&node.id).unwrap_or(&no_edges) {
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

    for code_id in &contained_code_ids {
        for edge in outgoing_edges.get(code_id).unwrap_or(&no_edges) {
            if !is_trace_edge(&edge.kind) {
                continue;
            }
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
    }

    Some(summary)
}

pub(crate) fn node_dependency_summary(graph: &CodeGraph, node_id: NodeId) -> NodeDependencySummary {
    let nodes_by_id = nodes_by_id_index(graph);
    let incident: Vec<&Edge> = graph
        .edges
        .iter()
        .filter(|edge| edge.source == node_id || edge.target == node_id)
        .collect();
    node_dependency_summary_indexed(&nodes_by_id, &incident, node_id)
}

pub(crate) fn node_dependency_summary_indexed(
    nodes_by_id: &BTreeMap<NodeId, &Node>,
    incident_edges: &[&Edge],
    node_id: NodeId,
) -> NodeDependencySummary {
    let mut summary = NodeDependencySummary::default();

    for edge in incident_edges {
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
                node_language(nodes_by_id, neighbor_id),
            );
        }
    }

    summary
}

pub(crate) fn node_card_actions(node: &Node) -> Vec<NodeCardAction> {
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

pub(crate) fn quote_query_value(value: &str) -> String {
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

pub(crate) fn node_source_preview(
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

pub(crate) fn read_file_source_preview(root: &Path, path: &Path) -> io::Result<SourcePreview> {
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
pub(crate) struct QuerySpec {
    pub(crate) original: String,
    pub(crate) command: String,
    pub(crate) terms: BTreeMap<String, String>,
    pub(crate) positional: Vec<String>,
    pub(crate) limit: usize,
}

impl QuerySpec {
    pub(crate) fn parse(expression: &str) -> Result<Self, QueryError> {
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

pub(crate) fn parse_call_expression(expression: &str) -> Result<Option<QuerySpec>, QueryError> {
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

pub(crate) fn query_nodes(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
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

pub(crate) fn query_edges(
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

pub(crate) fn query_trace(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
    query_trace_with(graph, spec, trace)
}

pub(crate) fn query_dependents(
    graph: &CodeGraph,
    spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
    query_trace_with(graph, spec, trace_dependents)
}

pub(crate) fn query_trace_with(
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
