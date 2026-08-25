//! The graph query language: expression parsing, every query slice, node
//! reference resolution, and result assembly.

use codegraph_core::{
    CodeGraph, Confidence, Edge, EdgeKind, Node, NodeId, NodeKind, is_vendored_source_path,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[allow(unused_imports)]
use crate::*;

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

/// Validate query syntax and command selection without requiring a graph.
/// CLI callers use this before an expensive scan so typos fail immediately.
pub fn validate_query_expression(expression: &str) -> Result<(), QueryError> {
    let spec = QuerySpec::parse(expression)?;
    if matches!(
        spec.command.as_str(),
        "nodes"
            | "node"
            | "edges"
            | "edge"
            | "calls"
            | "call"
            | "dependencies"
            | "depends"
            | "trace"
            | "dependents"
            | "impact"
            | "incoming"
            | "neighbors"
            | "neighbor"
            | "neighborhood"
            | "symbols"
            | "symbol"
            | "defs"
            | "definitions"
            | "files"
            | "file"
            | "sources"
            | "source"
            | "docs"
            | "doc"
            | "documents"
            | "document"
            | "adr"
            | "adrs"
            | "rfc"
            | "rfcs"
            | "sql"
            | "schema"
            | "database"
            | "db"
            | "entrypoints"
            | "entrypoint"
            | "starts"
            | "startup"
            | "routes"
            | "route"
            | "endpoints"
            | "endpoint"
            | "packages"
            | "package"
            | "deps"
            | "external"
            | "externals"
            | "configs"
            | "config"
            | "environment"
            | "env"
            | "errors"
            | "error"
            | "exceptions"
            | "exception"
            | "cycles"
            | "cycle"
            | "hotspots"
            | "hotspot"
            | "central"
            | "hubs"
            | "unreachable"
            | "dead"
            | "diagnostics"
            | "diagnostic"
            | "annotations"
            | "annotation"
            | "tags"
            | "tag"
            | "insights"
            | "insight"
            | "risks"
            | "risk"
            | "findings"
            | "finding"
            | "path"
            | "paths"
    ) {
        Ok(())
    } else {
        Err(QueryError::new(format!(
            "unknown query command `{}`; validate with `codegraph query --help`",
            spec.command
        )))
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
        notes: result.notes,
    }
}

pub(crate) fn query_neighbors(
    graph: &CodeGraph,
    spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
    // Which name the caller gave, so an answer about one of several
    // definitions says so, as a trace does.
    let mut start_label: Option<String> = None;
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
        start_label = Some(label.clone());
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

    let result = QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        visited_nodes.len(),
        total_edges,
        truncated,
    );
    Ok(
        match note_for_shared_label(graph, start_label.as_deref(), start) {
            Some(note) => result.with_note(note),
            None => result,
        },
    )
}

/// The note a query owes its reader when the name it was given belongs to
/// several definitions: kong has two functions called `load`, and "nothing
/// calls load" is only true of the one that was picked.
fn note_for_shared_label(graph: &CodeGraph, label: Option<&str>, chosen: NodeId) -> Option<String> {
    let label = label?;
    let node = graph.nodes.iter().find(|node| node.id == chosen)?;
    shared_name_note(graph, label, node)
}

/// Both ends of a path can name several definitions, and the answer is
/// about the pair that was picked.
fn with_endpoint_notes(
    graph: &CodeGraph,
    from: &str,
    to: &str,
    start: NodeId,
    target: NodeId,
    result: QueryResult,
) -> QueryResult {
    [(from, start), (to, target)]
        .into_iter()
        .filter_map(|(label, chosen)| note_for_shared_label(graph, Some(label), chosen))
        .fold(result, |result, note| result.with_note(note))
}

pub(crate) fn query_symbols(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &returned_node_ids);

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

pub(crate) fn query_files(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &returned_node_ids);

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

pub(crate) fn query_documents(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);
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

pub(crate) fn query_sql(graph: &CodeGraph, mut spec: QuerySpec) -> Result<QueryResult, QueryError> {
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
    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);
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

pub(crate) fn query_entrypoints(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
    if let Some(first) = spec.positional.first() {
        spec.terms
            .entry("search".to_string())
            .or_insert(first.clone());
    }
    validate_entrypoint_terms(&spec)?;

    let path_index = node_path_index(graph);
    // A program the parser recognised is a Function node an `Entrypoint`
    // edge points at -- Rust's `main`, a C# file written as top-level
    // statements -- and asking for the node kind alone answered "where
    // does the program start" with everything except the programs.
    let entrypoints = entrypoint_node_ids(graph);
    let mut matched: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            (node.kind == NodeKind::Entrypoint || entrypoints.contains(&node.id))
                && entrypoint_query_matches(node, &spec, &path_index)
        })
        .cloned()
        .collect();
    // A reader asking where the program starts wants the program: koel's
    // 151 entrypoints opened with eight GitHub Actions jobs, which say how
    // it is built and tested rather than how it runs. The overview ranks
    // them for the same reason, and one ranking is enough.
    matched.sort_by_key(|node| {
        (
            entrypoint_rank(node),
            node.span
                .as_ref()
                .map_or(0, |span| span.path.matches('/').count()),
            node.id,
        )
    });
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
    let mut node_ids = selected_ids.clone();
    for edge in &edges {
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }
    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);

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

pub(crate) fn query_routes(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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

    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);
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

pub(crate) fn query_packages(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);

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

pub(crate) fn query_configs(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let mut matched_targets: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            matches!(node.kind, NodeKind::Config | NodeKind::Environment)
                && config_query_matches(node, &spec, &path_index)
        })
        .cloned()
        .collect();
    // "What configuration does it read?" is answered by this query, and
    // koel's answer opened with twelve GitHub Actions run steps -- how the
    // project is linted, not how it is configured. What the program reads
    // comes first, in graph order inside a tier so the answer stays the
    // same between runs.
    matched_targets.sort_by_key(|node| {
        (
            u8::from(
                node.span
                    .as_ref()
                    .is_some_and(|span| is_repository_tooling_source_path(&span.path)),
            ),
            node.id,
        )
    });

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

    let matched_ids: Vec<NodeId> = matched_targets
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    let total_edges = edges.len();
    let unmatched = named_but_unmatched(&spec, &matched_targets).map(str::to_string);
    let result = QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        truncated,
    );
    Ok(match unmatched {
        Some(value) => result.with_note(nothing_matched_note(
            graph,
            "configuration key",
            &value,
            |node| matches!(node.kind, NodeKind::Config | NodeKind::Environment),
        )),
        None => result,
    })
}

/// What a query asked for by name when nothing answered to it.
fn named_but_unmatched<'a>(spec: &'a QuerySpec, matched: &[Node]) -> Option<&'a str> {
    if !matched.is_empty() {
        return None;
    }
    spec.terms
        .get("target")
        .or_else(|| spec.terms.get("search"))
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
}

pub(crate) fn query_errors(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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

    let matched_ids: Vec<NodeId> = matched_errors
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);
    let edges = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(index, _)| edge_indexes.contains(index))
        .map(|(_, edge)| edge.clone())
        .collect::<Vec<_>>();

    let total_nodes = nodes.len();
    let total_edges = edges.len();
    let unmatched = named_but_unmatched(&spec, &matched_errors).map(str::to_string);
    let result = QueryResult::new(
        graph,
        spec.original,
        nodes,
        edges,
        total_nodes,
        total_edges,
        truncated,
    );
    Ok(match unmatched {
        Some(value) => result.with_note(nothing_matched_note(graph, "error", &value, |node| {
            node.metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "error")
        })),
        None => result,
    })
}

pub(crate) fn query_cycles(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
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

    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .flat_map(|insight| insight.nodes.iter().copied())
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &node_ids);
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

/// The nodes a query matched, in the order that chose them, followed by the
/// context its edges reach. A result read top-down opens with the answer:
/// filtering the whole graph by an id set returned the repository node
/// first and the answer somewhere in the hundreds that followed.
fn matched_nodes_first(
    graph: &CodeGraph,
    matched: &[NodeId],
    node_ids: &BTreeSet<NodeId>,
) -> Vec<Node> {
    let selected: BTreeSet<NodeId> = matched.iter().copied().collect();
    let by_id: BTreeMap<NodeId, &Node> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut nodes: Vec<Node> = matched
        .iter()
        .filter_map(|id| by_id.get(id).map(|node| (*node).clone()))
        .collect();
    nodes.extend(
        graph
            .nodes
            .iter()
            .filter(|node| node_ids.contains(&node.id) && !selected.contains(&node.id))
            .cloned(),
    );
    nodes
}

pub(crate) fn query_hotspots(
    graph: &CodeGraph,
    spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let mut node_ids = selected_ids.clone();
    for edge in &edges {
        node_ids.insert(edge.source);
        node_ids.insert(edge.target);
    }
    // The hotspots themselves first and in the order that ranks them, each
    // carrying the score that put it there: everything else in the result
    // is a neighbour reached through their edges, and asking "what is
    // central here" answered with a repository node and a benchmark script
    // because the subgraph came out in graph order.
    let mut nodes = matched
        .iter()
        .take(spec.limit)
        .map(|hotspot| {
            let mut node = hotspot.node.clone();
            node.metadata
                .insert("hotspot_score".to_string(), hotspot.score.to_string());
            node
        })
        .collect::<Vec<_>>();
    nodes.extend(
        graph
            .nodes
            .iter()
            .filter(|node| node_ids.contains(&node.id) && !selected_ids.contains(&node.id))
            .cloned(),
    );

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

pub(crate) fn query_unreachable(
    graph: &CodeGraph,
    spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let mut matched: Vec<&Node> = graph
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
        .collect();
    // "Which code is unused?" is answered by this query, and a document
    // was never going to be reached by running the program: koel's answer
    // opened with `.github/copilot-instructions.md` and four of its
    // headings. Code first, then what sits around it, in graph order
    // inside a tier so the answer stays the same between runs.
    matched.sort_by_key(|node| (unreachable_rank(node), node.id));
    let matched: Vec<NodeId> = matched.into_iter().map(|node| node.id).collect();
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

/// What a reader asking "which code is unused" wants first. Code that
/// nothing reaches is the answer; a document, a configuration value or a
/// dependency was never reached by running the program, so it is context
/// rather than a finding.
fn unreachable_rank(node: &Node) -> u8 {
    if node
        .metadata
        .get("item_kind")
        .is_some_and(|kind| kind.starts_with("document"))
        || node
            .metadata
            .get("language")
            .is_some_and(|language| language == "markdown")
    {
        return 3;
    }
    match node.kind {
        NodeKind::Function | NodeKind::Type => 0,
        NodeKind::File | NodeKind::Module | NodeKind::Entrypoint => 1,
        NodeKind::Directory | NodeKind::Config | NodeKind::Environment => 2,
        _ => 3,
    }
}

pub(crate) fn query_unreachable_flow_scope(
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

pub(crate) fn unreachable_flow_matches(
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

pub(crate) fn query_diagnostics(
    graph: &CodeGraph,
    spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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

pub(crate) fn query_annotations(
    graph: &CodeGraph,
    mut spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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
    let matched_ids: Vec<NodeId> = matched
        .iter()
        .take(spec.limit)
        .map(|node| node.id)
        .collect();
    let nodes = matched_nodes_first(graph, &matched_ids, &returned_node_ids);

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

pub(crate) fn query_insights(
    graph: &CodeGraph,
    spec: QuerySpec,
) -> Result<QueryResult, QueryError> {
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

pub(crate) fn query_path(graph: &CodeGraph, spec: QuerySpec) -> Result<QueryResult, QueryError> {
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
    let (from, to) = (from.clone(), to.clone());
    let edge_kind = spec
        .terms
        .get("edge_kind")
        .or_else(|| spec.terms.get("kind"));

    if start == target {
        let node = graph.nodes.iter().find(|node| node.id == start).cloned();
        return Ok(with_endpoint_notes(
            graph,
            &from,
            &to,
            start,
            target,
            QueryResult::new(
                graph,
                spec.original,
                node.into_iter().collect(),
                Vec::new(),
                1,
                0,
                false,
            ),
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
                return Ok(with_endpoint_notes(
                    graph,
                    &from,
                    &to,
                    start,
                    target,
                    QueryResult::new(
                        graph,
                        spec.original,
                        nodes,
                        edges,
                        total_nodes,
                        total_edges,
                        false,
                    ),
                ));
            }
            queue.push_back((edge.target, depth + 1));
        }
    }

    Ok(with_endpoint_notes(
        graph,
        &from,
        &to,
        start,
        target,
        QueryResult::new(
            graph,
            spec.original,
            Vec::new(),
            Vec::new(),
            0,
            0,
            truncated,
        ),
    ))
}

pub(crate) fn validate_node_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if is_node_term(key) {
            continue;
        }
        // Name what is accepted: the unknown-command error does, and an agent
        // that gets only "unsupported" has to go read the docs to recover.
        return Err(QueryError::new(format!(
            "unsupported node query term `{key}`; expected id, stable_id, kind, label, search, language, item_kind, package_id, or metadata.<key>"
        )));
    }
    Ok(())
}

pub(crate) fn validate_edge_terms(spec: &QuerySpec) -> Result<(), QueryError> {
    for key in spec.terms.keys() {
        if is_edge_term(key) {
            continue;
        }
        return Err(QueryError::new(format!(
            "unsupported edge query term `{key}`; expected kind, source, target, confidence, edge, edge_index, or metadata.<key>"
        )));
    }
    Ok(())
}

pub(crate) fn validate_path_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_neighbor_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_symbol_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_file_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_document_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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
                | "owner"
                | "doc_owner"
                | "status"
                | "doc_status"
                | "tag"
                | "tags"
                | "doc_tags"
                | "title"
                | "doc_title"
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

pub(crate) fn validate_sql_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_entrypoint_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_route_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_package_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_config_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_error_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_cycle_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_hotspot_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_unreachable_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_diagnostic_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_annotation_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn validate_insight_terms(spec: &QuerySpec) -> Result<(), QueryError> {
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

pub(crate) fn is_node_term(key: &str) -> bool {
    matches!(
        key,
        "id" | "stable_id" | "kind" | "label" | "search" | "language" | "item_kind" | "package_id"
    ) || key.starts_with("metadata.")
}

pub(crate) fn is_edge_term(key: &str) -> bool {
    matches!(
        key,
        "kind" | "source" | "target" | "confidence" | "edge" | "edge_index"
    ) || key.starts_with("metadata.")
}

pub(crate) fn is_lsp_diagnostic_node(node: &Node) -> bool {
    node.metadata
        .get("item_kind")
        .is_some_and(|kind| kind == "diagnostic")
        && node
            .metadata
            .get("source")
            .is_some_and(|source| source == "lsp")
}

pub(crate) fn diagnostic_query_matches(
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

pub(crate) fn diagnostic_source_nodes(graph: &CodeGraph, diagnostic_id: NodeId) -> Vec<NodeId> {
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

pub(crate) fn annotation_query_matches(
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

pub(crate) fn annotation_key_filter(spec: &QuerySpec) -> Option<&str> {
    spec.terms
        .get("key")
        .or_else(|| spec.terms.get("annotation"))
        .or_else(|| spec.terms.get("annotation_key"))
        .map(String::as_str)
}

pub(crate) fn annotation_value_filter(spec: &QuerySpec) -> Option<&str> {
    spec.terms
        .get("value")
        .or_else(|| spec.terms.get("annotation_value"))
        .map(String::as_str)
}

pub(crate) fn node_has_annotation(node: &Node) -> bool {
    node.metadata
        .keys()
        .any(|key| key.starts_with("annotation."))
}

pub(crate) fn annotation_matches(node: &Node, expected: &str) -> bool {
    node.metadata.iter().any(|(key, value)| {
        key.starts_with("annotation.")
            && (text_matches(key.trim_start_matches("annotation."), expected)
                || text_matches(key, expected)
                || text_matches(value, expected))
    })
}

pub(crate) fn annotation_key_matches(node: &Node, expected: &str) -> bool {
    node.metadata.keys().any(|key| {
        key.starts_with("annotation.")
            && (text_matches(key.trim_start_matches("annotation."), expected)
                || text_matches(key, expected))
    })
}

pub(crate) fn annotation_value_matches(node: &Node, expected: &str) -> bool {
    node.metadata
        .iter()
        .any(|(key, value)| key.starts_with("annotation.") && text_matches(value, expected))
}

pub(crate) fn annotation_pair_matches(
    node: &Node,
    key_expected: &str,
    value_expected: &str,
) -> bool {
    node.metadata.iter().any(|(key, value)| {
        key.starts_with("annotation.")
            && (text_matches(key.trim_start_matches("annotation."), key_expected)
                || text_matches(key, key_expected))
            && text_matches(value, value_expected)
    })
}

pub(crate) fn is_framework_route_node(node: &Node) -> bool {
    node.kind == NodeKind::Entrypoint
        && node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| kind == "framework_route")
}

pub(crate) fn insight_query_matches(
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

pub(crate) fn entrypoint_query_matches(
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

pub(crate) fn symbol_query_matches(
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

pub(crate) fn symbol_definition_edge_matches(
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    edge_kind: Option<&str>,
) -> bool {
    selected_ids.contains(&edge.target)
        && matches!(edge.kind, EdgeKind::Contains | EdgeKind::Defines)
        && edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
}

pub(crate) fn file_query_matches(
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

pub(crate) fn file_structural_edge_matches(
    edge: &Edge,
    selected_ids: &BTreeSet<NodeId>,
    edge_kind: Option<&str>,
) -> bool {
    selected_ids.contains(&edge.source)
        && matches!(edge.kind, EdgeKind::Contains | EdgeKind::Defines)
        && edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
}

pub(crate) fn file_trace_edge_touches_selected(
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

pub(crate) fn is_document_query_node(node: &Node) -> bool {
    node.metadata
        .get("item_kind")
        .is_some_and(|kind| matches!(kind.as_str(), "document" | "document_section"))
        || node
            .metadata
            .get("language")
            .is_some_and(|language| language == "markdown")
}

pub(crate) fn document_query_matches(
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
        "owner" | "doc_owner" => metadata_matches(node, "doc_owner", expected),
        "status" | "doc_status" => metadata_matches(node, "doc_status", expected),
        "tag" | "tags" | "doc_tags" => metadata_matches(node, "doc_tags", expected),
        "title" | "doc_title" => metadata_matches(node, "doc_title", expected),
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

pub(crate) fn document_edge_matches(
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

pub(crate) fn document_relevant_edge(graph: &CodeGraph, edge: &Edge) -> bool {
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
    matches!(edge.kind, EdgeKind::Contains | EdgeKind::References)
        && (source_is_doc || target_is_doc)
}

pub(crate) fn document_edges_search(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
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

pub(crate) fn document_node_has_relation(
    graph: &CodeGraph,
    node_id: NodeId,
    expected: &str,
) -> bool {
    graph.edges.iter().any(|edge| {
        (edge.source == node_id || edge.target == node_id)
            && document_relevant_edge(graph, edge)
            && edge_metadata_matches(edge, "relation", expected)
    })
}

pub(crate) fn document_node_references_target(
    graph: &CodeGraph,
    node_id: NodeId,
    expected: &str,
) -> bool {
    graph.edges.iter().any(|edge| {
        edge.source == node_id
            && document_relevant_edge(graph, edge)
            && document_edge_target_matches(graph, edge, expected)
    })
}

pub(crate) fn document_edge_target_matches(graph: &CodeGraph, edge: &Edge, expected: &str) -> bool {
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

pub(crate) fn is_sql_query_node(node: &Node) -> bool {
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

pub(crate) fn sql_query_matches(
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

pub(crate) fn sql_edge_matches(
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

pub(crate) fn sql_relevant_edge(graph: &CodeGraph, edge: &Edge) -> bool {
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

pub(crate) fn sql_edges_search(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
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

pub(crate) fn sql_node_has_relation(graph: &CodeGraph, node_id: NodeId, expected: &str) -> bool {
    graph.edges.iter().any(|edge| {
        (edge.source == node_id || edge.target == node_id)
            && sql_relevant_edge(graph, edge)
            && edge_metadata_matches(edge, "relation", expected)
    })
}

pub(crate) fn sql_edge_target_matches(graph: &CodeGraph, edge: &Edge, expected: &str) -> bool {
    edge.metadata
        .get("target")
        .is_some_and(|value| text_matches(value, expected))
        || graph
            .nodes
            .iter()
            .find(|node| node.id == edge.target)
            .is_some_and(|node| node_search_matches(node, expected))
}

pub(crate) fn sql_edge_endpoint_matches(graph: &CodeGraph, edge: &Edge, expected: &str) -> bool {
    graph
        .nodes
        .iter()
        .filter(|node| node.id == edge.source || node.id == edge.target)
        .any(|node| node_search_matches(node, expected) || sql_table_filter_matches(node, expected))
}

pub(crate) fn sql_table_filter_matches(node: &Node, expected: &str) -> bool {
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

pub(crate) fn sql_column_filter_matches(node: &Node, expected: &str) -> bool {
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

pub(crate) fn sql_unresolved_filter_matches(node: &Node, expected: &str) -> bool {
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

pub(crate) fn comma_list_matches(value: &str, expected: &str) -> bool {
    value
        .split(',')
        .map(str::trim)
        .any(|item| !item.is_empty() && text_matches(item, expected))
}

pub(crate) fn sql_source_nodes(graph: &CodeGraph, node_id: NodeId) -> Vec<&Node> {
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

pub(crate) fn route_query_matches(
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

pub(crate) fn route_trace_should_expand(edge: &Edge) -> bool {
    !edge
        .metadata
        .values()
        .any(|value| matches!(value.as_str(), "framework_route_file" | "entrypoint_file"))
}

pub(crate) fn is_package_query_node(node: &Node) -> bool {
    node.kind == NodeKind::ExternalDependency
        && node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| matches!(kind.as_str(), "dependency" | "import"))
}

pub(crate) fn package_query_matches(
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

pub(crate) fn package_edge_query_matches(
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

pub(crate) fn package_search_matches(node: &Node, expected: &str) -> bool {
    text_matches(&node.label, expected)
        || node
            .metadata
            .values()
            .any(|value| text_matches(value, expected))
        || package_node_key(node).is_some_and(|key| text_matches(&key, expected))
}

pub(crate) fn package_identifier_matches(node: &Node, expected: &str) -> bool {
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

pub(crate) fn package_key_matches(key: &str, expected: &str) -> bool {
    key.eq_ignore_ascii_case(expected)
        || key
            .split_once(':')
            .is_some_and(|(_, package)| package.eq_ignore_ascii_case(expected))
}

pub(crate) fn package_node_key(node: &Node) -> Option<String> {
    if let Some(package_id) = node.metadata.get("package_id") {
        return Some(package_id.clone());
    }
    let language = node.metadata.get("language")?;
    let package = import_package_candidate(language, &node.label)?;
    Some(package_id(&package.ecosystem, &package.package))
}

pub(crate) fn package_ecosystem(node: &Node) -> Option<String> {
    node.metadata.get("ecosystem").cloned().or_else(|| {
        package_node_key(node)
            .and_then(|key| key.split_once(':').map(|(value, _)| value.to_string()))
    })
}

pub(crate) fn package_id(ecosystem: &str, package: &str) -> String {
    format!("{}:{}", ecosystem.trim(), package.trim())
}

pub(crate) fn package_incoming_edges(graph: &CodeGraph, node_id: NodeId) -> Vec<&Edge> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == node_id && matches!(edge.kind, EdgeKind::Imports | EdgeKind::DependsOn)
        })
        .collect()
}

pub(crate) fn package_source_nodes(graph: &CodeGraph, node_id: NodeId) -> Vec<&Node> {
    package_incoming_edges(graph, node_id)
        .iter()
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.source))
        .collect()
}

pub(crate) fn config_query_matches(
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

pub(crate) fn error_query_matches(
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

pub(crate) fn cycle_query_matches(
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

pub(crate) fn hotspot_query_matches(
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

pub(crate) fn hotspot_edge_touches_selected(
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

pub(crate) fn insight_node_matches(graph: &CodeGraph, insight: &Insight, expected: &str) -> bool {
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

pub(crate) fn diagnostic_path_matches(node: &Node, expected: &str) -> bool {
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

pub(crate) fn unreachable_node_terms(spec: &QuerySpec) -> BTreeMap<String, String> {
    spec.terms
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "path_prefix" | "scope"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnreachableScope {
    SourceFiles,
    ConfigReads,
    ErrorFlows,
    AnyNode,
}

pub(crate) fn unreachable_scope(spec: &QuerySpec) -> Result<UnreachableScope, QueryError> {
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

pub(crate) fn node_matches(node: &Node, terms: &BTreeMap<String, String>) -> bool {
    terms.iter().all(|(key, expected)| match key.as_str() {
        "id" => parse_node_id(expected).is_ok_and(|id| node.id == id),
        "stable_id" => node
            .metadata
            .get("stable_id")
            .is_some_and(|value| value == expected),
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

pub(crate) fn slice_node_matches(
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

pub(crate) fn node_path_index(graph: &CodeGraph) -> BTreeMap<NodeId, String> {
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

pub(crate) fn node_path_matches(
    node: &Node,
    path_index: &BTreeMap<NodeId, String>,
    expected: &str,
) -> bool {
    let expected = normalize_path_prefix(expected);
    if expected.is_empty() {
        return true;
    }
    let expected_slash = format!("{expected}/");
    node_path_matches_prepared(node, path_index, &expected, &expected_slash)
}

/// `node_path_matches` with the normalized, non-empty prefix precomputed. Hot
/// callers that test many nodes against the same file path (e.g. compact file
/// summaries) normalize once and reuse it here instead of re-normalizing and
/// re-allocating `"{expected}/"` on every call.
pub(crate) fn node_path_matches_prepared(
    node: &Node,
    path_index: &BTreeMap<NodeId, String>,
    expected: &str,
    expected_slash: &str,
) -> bool {
    path_index
        .get(&node.id)
        .is_some_and(|path| path == expected || path.starts_with(expected_slash))
        || node
            .span
            .as_ref()
            .map(|span| normalize_graph_path(&span.path))
            .is_some_and(|path| path == expected || path.starts_with(expected_slash))
}

pub(crate) fn normalize_path_prefix(value: &str) -> String {
    normalize_graph_path(value)
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn normalize_graph_path(value: &str) -> String {
    let mut normalized = value.trim().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }
    normalized
}

pub(crate) fn metadata_matches(node: &Node, key: &str, expected: &str) -> bool {
    node.metadata
        .get(key)
        .is_some_and(|value| text_matches(value, expected))
}

pub(crate) fn edge_metadata_matches(edge: &Edge, key: &str, expected: &str) -> bool {
    edge.metadata
        .get(key)
        .is_some_and(|value| text_matches(value, expected))
}

pub(crate) fn node_search_matches(node: &Node, expected: &str) -> bool {
    text_matches(&node.label, expected)
        || text_matches(&kind_name(&node.kind), expected)
        || node
            .metadata
            .iter()
            .any(|(key, value)| text_matches(key, expected) || text_matches(value, expected))
}

pub(crate) fn entrypoint_kind_matches(node: &Node, expected: &str) -> bool {
    node.metadata
        .get("entrypoint_kind")
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

pub(crate) fn edge_matches(
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

pub(crate) fn matching_edge_indexes(
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

pub(crate) fn edge_evidence(
    edge_index: usize,
    source: &Node,
    target: &Node,
    edge: &Edge,
) -> Vec<String> {
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

    // A call edge knows what narrowed it down; saying so turns "syntax-level
    // fact" into the fact itself.
    if let Some(note) = edge
        .metadata
        .get("resolution_basis")
        .map(String::as_str)
        .and_then(resolution_basis_evidence)
    {
        evidence.push(note.to_string());
    }

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

/// What a call's `resolution_basis` means, in the reader's terms.
pub(crate) fn resolution_basis_evidence(basis: &str) -> Option<&'static str> {
    Some(match basis {
        "same_file" => "resolution_note=the target is defined in the calling file",
        "import" => "resolution_note=an import in the calling file names the target's module",
        "package" => "resolution_note=an unqualified call resolves inside its own package",
        "module_file" => "resolution_note=the module named in the call is the file that defines it",
        "lexical_scope" => "resolution_note=the target is visible from the calling definition",
        "module_export" => {
            "resolution_note=the target's module exports it, so another file can name it"
        }
        "receiver_type" => "resolution_note=the receiver's declared type owns the target",
        "owner_type" => "resolution_note=the call names the type that owns the target",
        "overload" => {
            "resolution_note=the candidates are one method's overloads, so the call reaches all of them"
        }
        "name" => "resolution_note=nothing but the name matched, across the whole project",
        _ => return None,
    })
}

pub(crate) fn confidence_evidence(confidence: codegraph_core::Confidence) -> &'static str {
    match confidence {
        codegraph_core::Confidence::Exact => "confidence_note=declared or directly resolved fact",
        codegraph_core::Confidence::Semantic => "confidence_note=semantic tooling fact",
        codegraph_core::Confidence::Syntactic => "confidence_note=syntax-level fact",
        codegraph_core::Confidence::Heuristic => "confidence_note=pattern or name based inference",
        codegraph_core::Confidence::Unknown => "confidence_note=unknown provenance",
    }
}

pub(crate) fn endpoint_matches(graph: &CodeGraph, id: NodeId, expected: &str) -> bool {
    parse_node_id(expected).is_ok_and(|expected_id| expected_id == id)
        || graph
            .nodes
            .iter()
            .find(|node| node.id == id)
            .is_some_and(|node| {
                // The durable id names one node exactly, which is what an
                // agent saved; the label is matched as text.
                node.metadata
                    .get("stable_id")
                    .is_some_and(|stable_id| stable_id == expected)
                    || text_matches(&node.label, expected)
            })
}

pub(crate) fn endpoint_nodes(graph: &CodeGraph, edges: &[Edge]) -> Vec<Node> {
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

/// Bounded Levenshtein distance with early exit above `max`.
pub(crate) fn edit_distance_within(left: &str, right: &str, max: usize) -> Option<usize> {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.len().abs_diff(right.len()) > max {
        return None;
    }
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (row, left_char) in left.iter().enumerate() {
        let mut current = vec![row + 1];
        let mut row_min = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let cost = usize::from(left_char != right_char);
            let value = (previous[column] + cost)
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
            row_min = row_min.min(value);
            current.push(value);
        }
        if row_min > max {
            return None;
        }
        previous = current;
    }
    (previous[right.len()] <= max).then_some(previous[right.len()])
}

/// Actionable node-not-found error: appends up to three near-matches
/// (bounded edit distance or meaningful substring overlap) so a mistyped
/// label points at real candidates.
/// The labels close enough to what was asked for to be worth naming: two
/// edits away, or one containing the other.
fn near_label_matches<'a>(
    graph: &'a CodeGraph,
    value: &str,
    accept: impl Fn(&Node) -> bool,
) -> Vec<&'a str> {
    let needle = value.trim().to_ascii_lowercase();
    let mut ranked: Vec<(usize, &str)> = Vec::new();
    if needle.len() >= 3 {
        for node in graph.nodes.iter().filter(|node| accept(node)) {
            let label = node.label.as_str();
            if ranked.iter().any(|(_, seen)| *seen == label) {
                continue;
            }
            let lower = label.to_ascii_lowercase();
            let substring = (needle.len() >= 4 && lower.contains(&needle))
                || (lower.len() >= 4 && needle.contains(&lower));
            let distance = edit_distance_within(&lower, &needle, 2);
            if let Some(distance) = distance {
                ranked.push((distance, label));
            } else if substring {
                ranked.push((3, label));
            } else if needle.len() >= 4
                // An error is labelled by the line that raises it, so the
                // name being asked for is one word inside a sentence.
                && lower
                    .split(|character: char| !character.is_alphanumeric() && character != '_')
                    .filter(|word| word.len() >= 4)
                    .any(|word| edit_distance_within(word, &needle, 2).is_some())
            {
                ranked.push((4, label));
            }
        }
    }
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.len().cmp(&b.1.len())));
    ranked.into_iter().take(3).map(|(_, label)| label).collect()
}

/// What to say when a query names something the graph does not hold. An
/// empty answer on its own reads as "this project has no such thing",
/// which is a claim the scan cannot make: it may simply not have seen it.
fn nothing_matched_note(
    graph: &CodeGraph,
    role: &str,
    value: &str,
    accept: impl Fn(&Node) -> bool,
) -> String {
    let suggestions = near_label_matches(graph, value, accept);
    if suggestions.is_empty() {
        format!("no {role} named `{value}` is in the graph; the scan may not read that form")
    } else {
        format!(
            "no {role} named `{value}` is in the graph; it holds {}",
            suggestions
                .iter()
                .map(|label| format!("`{label}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// What a surface owes a reader when the name it was given matched
/// nothing: the same sentence everywhere, with the nearest labels when
/// there are any. Public because the CLI and the API answer for entries
/// that return an `Option` rather than a `Result`.
pub fn missing_node_error(graph: &CodeGraph, role: &str, value: &str) -> QueryError {
    node_not_found_error(graph, role, value)
}

pub(crate) fn node_not_found_error(graph: &CodeGraph, role: &str, value: &str) -> QueryError {
    // Only names a reader can ask about again: an import statement and an
    // error construct carry a label, but "did you mean `use
    // codegraph_indexer::{IndexOptions,`?" is not a suggestion.
    let suggestions = near_label_matches(graph, value, |node| {
        declares_its_name(node) && node.kind != NodeKind::ExternalDependency
    });
    if suggestions.is_empty() {
        QueryError::new(format!(
            "{role} `{value}` did not match a node; try a label from `entrypoints`/`query 'nodes search:…'` or an id such as n42"
        ))
    } else {
        QueryError::new(format!(
            "{role} `{value}` did not match a node; did you mean {}?",
            suggestions
                .iter()
                .map(|label| format!("`{label}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

pub(crate) fn resolve_node_reference(graph: &CodeGraph, value: &str) -> Option<NodeId> {
    if let Ok(id) = parse_node_id(value) {
        return graph.nodes.iter().any(|node| node.id == id).then_some(id);
    }

    graph
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("stable_id")
                .is_some_and(|stable_id| stable_id == value)
        })
        // An exact label can name many nodes (`main` names 15 on terraform);
        // rank them instead of taking whichever the file walk reached first.
        .or_else(|| best_labelled_node(graph, value))
        .or_else(|| {
            // Substring fallback only when it is unambiguous: with several
            // candidates the winner would be whichever node happens to come
            // first, silently binding the query to the wrong symbol.
            let mut matches = graph
                .nodes
                .iter()
                .filter(|node| text_matches(&node.label, value));
            let first = matches.next()?;
            matches.next().is_none().then_some(first)
        })
        .map(|node| node.id)
}

pub(crate) fn path_edge_matches(edge: &Edge, edge_kind: Option<&str>) -> bool {
    is_trace_edge(&edge.kind)
        && edge_kind.is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NeighborDirection {
    In,
    Out,
    Both,
}

pub(crate) fn parse_neighbor_direction(
    value: &str,
    query: &str,
) -> Result<NeighborDirection, QueryError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "in" | "incoming" => Ok(NeighborDirection::In),
        "out" | "outgoing" => Ok(NeighborDirection::Out),
        "both" | "any" | "all" => Ok(NeighborDirection::Both),
        other => Err(QueryError::new(format!(
            "invalid {query} direction `{other}`; expected in, out, or both"
        ))),
    }
}

pub(crate) fn neighbor_edge_matches(
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

pub(crate) fn reconstruct_path_edges(
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

pub(crate) fn path_nodes(graph: &CodeGraph, start: NodeId, edges: &[Edge]) -> Vec<Node> {
    let mut ids = Vec::with_capacity(edges.len() + 1);
    ids.push(start);
    for edge in edges {
        ids.push(edge.target);
    }

    ids.into_iter()
        .filter_map(|id| graph.nodes.iter().find(|node| node.id == id).cloned())
        .collect()
}

pub(crate) fn config_target_matches(node: &Node, target: &str) -> bool {
    target.is_empty()
        || text_matches(&node.label, target)
        || node
            .metadata
            .iter()
            .any(|(key, value)| text_matches(key, target) || text_matches(value, target))
}

pub(crate) fn config_reader_paths(
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

pub(crate) fn build_config_path(
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

pub(crate) fn error_target_matches(node: &Node, target: &str) -> bool {
    target.is_empty()
        || text_matches(&node.label, target)
        || node
            .metadata
            .iter()
            .any(|(key, value)| text_matches(key, target) || text_matches(value, target))
}

pub(crate) fn error_source_paths(
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

pub(crate) fn build_error_path(
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

pub(crate) fn is_upstream_flow_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls | EdgeKind::References | EdgeKind::Entrypoint
    )
}

pub(crate) fn text_matches(actual: &str, expected: &str) -> bool {
    actual
        .to_ascii_lowercase()
        .contains(&expected.to_ascii_lowercase())
}

pub(crate) fn increment_facet(facets: &mut BTreeMap<String, usize>, key: String) {
    *facets.entry(key).or_insert(0) += 1;
}

/// Parse a node id in either bare numeric (`42`) or n-prefixed (`n42`)
/// form — the format printed by query results and web deep links.
pub fn parse_node_id(value: &str) -> Result<NodeId, QueryError> {
    let value = value.trim().trim_start_matches('n');
    value
        .parse::<u64>()
        .map(NodeId)
        .map_err(|_| QueryError::new(format!("invalid node id `{value}`")))
}

pub(crate) fn parse_limit(value: &str) -> Result<usize, QueryError> {
    value
        .parse::<usize>()
        .map(|value| value.clamp(1, 1000))
        .map_err(|_| QueryError::new(format!("invalid limit `{value}`")))
}

pub(crate) fn split_query_tokens(expression: &str) -> Result<Vec<String>, QueryError> {
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

pub(crate) fn add_parse_error_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
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
            // A file the scan would have read for facts is missing from
            // the graph, and a reader can raise the budget. A data file is
            // not: pytudes skips thirteen text corpora and one 20 MB list
            // of sudokus, and holding them would add nothing.
            let holds_facts = node.metadata.contains_key("language")
                || node.metadata.get("item_kind").map(String::as_str) == Some("notebook");
            insights.push(Insight {
                kind: "skipped_large_file".to_string(),
                severity: if holds_facts {
                    InsightSeverity::Warning
                } else {
                    InsightSeverity::Info
                },
                message: format!(
                    "{} skipped because size {file_size} exceeds max file size {max_file_size}",
                    node.label
                ),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        } else if let Some(reason) = node.metadata.get("manifest_parse_error") {
            // A manifest nobody can parse declares nothing, so every
            // dependency finding about the project is missing what it says.
            let vendored = is_vendored_source_path(&node.label);
            let fixture = is_test_like_source_path(&node.label);
            insights.push(Insight {
                kind: "malformed_manifest".to_string(),
                severity: if vendored || fixture {
                    InsightSeverity::Info
                } else {
                    InsightSeverity::Warning
                },
                message: format!(
                    "{} could not be parsed, so its dependencies are missing: {reason}",
                    node.label
                ),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        } else if let Some(reason) = node.metadata.get("read_error") {
            // A file the scan could not open holds no facts, and without
            // this the graph shows it as a file with nothing in it.
            insights.push(Insight {
                kind: "unreadable_file".to_string(),
                severity: InsightSeverity::Error,
                message: format!("{} could not be read: {reason}", node.label),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        } else if let Some(reason) = node.metadata.get("parse_error") {
            // The reason is the difference between a grammar this scan
            // cannot read and a file nothing can: redis's `life.lua` is
            // Latin-1, which is what "source is not valid utf-8" says.
            insights.push(Insight {
                kind: "parse_error".to_string(),
                severity: InsightSeverity::Error,
                message: format!("{} failed to parse: {reason}", node.label),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        } else if node
            .metadata
            .get("syntax_errors")
            .is_some_and(|value| value == "true")
        {
            insights.push(Insight {
                // An error node says the grammar did not cover something,
                // not that the code is broken: redis's `src/bio.c` compiles
                // and tree-sitter-c still stumbles over its macros. This is
                // a limit of the extraction, so it is reported as one — 686
                // of the corpus warnings were the parser's own reach.
                kind: "syntax_error".to_string(),
                severity: InsightSeverity::Info,
                message: match node.metadata.get("syntax_error_line") {
                    Some(line) => format!(
                        "{} contains syntax error nodes, first at line {line}",
                        node.label
                    ),
                    None => format!("{} contains syntax error nodes", node.label),
                },
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        }
    }
}

pub(crate) fn add_semantic_diagnostic_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
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

pub(crate) fn insight_search_matches(insight: &Insight, expected: &str) -> bool {
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

/// Heuristic-resolution findings read as info on syntactic-only scans and
/// escalate to warnings once semantic (LSP) enrichment has run — the same
/// calibration rule for unresolved calls, ambiguous calls, and heuristic
/// cross-language edges (audit F3; Phase 9 dogfooding).
pub(crate) fn heuristic_scan_severity(graph: &CodeGraph) -> InsightSeverity {
    let semantically_enriched = graph
        .edges
        .iter()
        .any(|edge| edge.confidence == Confidence::Semantic);
    if semantically_enriched {
        InsightSeverity::Warning
    } else {
        InsightSeverity::Info
    }
}

pub(crate) fn add_unresolved_call_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    // On syntactic-only scans an unresolved call is the expected default and
    // reads as info; once semantic enrichment has run, a target that still
    // cannot be resolved is a real warning.
    let severity = heuristic_scan_severity(graph);

    let mut by_label: BTreeMap<&str, Vec<NodeId>> = BTreeMap::new();
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
            by_label
                .entry(node.label.as_str())
                .or_default()
                .push(node.id);
        }
    }

    // One adjacency pass instead of an O(edges) scan per label (audit F11).
    let placeholder_ids: BTreeSet<NodeId> = by_label.values().flatten().copied().collect();
    let mut incoming_calls: BTreeMap<NodeId, Vec<usize>> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind == EdgeKind::Calls && placeholder_ids.contains(&edge.target) {
            incoming_calls
                .entry(edge.target)
                .or_default()
                .push(edge_index);
        }
    }

    for (label, node_ids) in by_label {
        let edges: Vec<usize> = node_ids
            .iter()
            .flat_map(|node_id| {
                incoming_calls
                    .get(node_id)
                    .map(Vec::as_slice)
                    .unwrap_or(&[])
                    .iter()
                    .copied()
            })
            .collect();
        // A call through a value the body binds has nothing to find:
        // terraform's `done()` comes from `runningCtx, done :=
        // context.WithCancel(…)`. Reporting it as a call the resolver
        // failed on made 1483 of its calls look like extraction gaps.
        if !edges.is_empty()
            && edges.iter().all(|edge_index| {
                graph.edges.get(*edge_index).is_some_and(|edge| {
                    edge.metadata.get("unresolved_reason").map(String::as_str)
                        == Some("local_value")
                })
            })
        {
            continue;
        }

        let message = if node_ids.len() > 1 {
            format!(
                "Call target `{label}` could not be resolved syntactically ({} placeholder nodes)",
                node_ids.len()
            )
        } else {
            format!("Call target `{label}` could not be resolved syntactically")
        };
        insights.push(Insight {
            kind: "unresolved_call".to_string(),
            severity,
            message,
            nodes: node_ids,
            edges,
        });
    }
}

pub(crate) fn add_ambiguous_call_resolution_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    // Severity and the callers of a placeholder were both recomputed by
    // scanning every edge, once per placeholder: on terraform that is 6700
    // placeholders times 223000 edges, twice over. One pass builds what the
    // loops need.
    let severity = heuristic_scan_severity(graph);
    let mut callers: BTreeMap<NodeId, Vec<(usize, NodeId)>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind == EdgeKind::Calls {
            callers
                .entry(edge.target)
                .or_default()
                .push((index, edge.source));
        }
    }

    for placeholder in graph.nodes.iter().filter(|node| {
        node.metadata
            .get("item_kind")
            .is_some_and(|kind| kind == "call")
            && node
                .metadata
                .get("resolution")
                .is_some_and(|resolution| resolution == "ambiguous")
    }) {
        let matches = callers
            .get(&placeholder.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut nodes = vec![placeholder.id];
        nodes.extend(matches.iter().map(|(_, source)| *source));
        nodes.sort_unstable();
        nodes.dedup();
        let count = placeholder
            .metadata
            .get("candidate_count")
            .map(String::as_str)
            .unwrap_or("multiple");
        let sample = placeholder
            .metadata
            .get("candidate_sample")
            .filter(|sample| !sample.is_empty())
            .map(|sample| format!("; sample: {sample}"))
            .unwrap_or_default();
        insights.push(Insight {
            kind: "ambiguous_call_resolution".to_string(),
            severity,
            message: format!(
                "Call `{}` has {count} same-language candidates and was kept as one bounded ambiguity{sample}",
                placeholder.label
            ),
            nodes,
            edges: matches.iter().map(|(index, _)| *index).collect(),
        });
    }

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
            severity,
            message: format!(
                "`{caller}` calls `{call_label}` but it resolves to multiple targets: {target_labels}"
            ),
            nodes,
            edges,
        });
    }
}
