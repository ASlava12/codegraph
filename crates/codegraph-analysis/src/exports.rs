//! Graph export targets: DOT, NDJSON, GraphML, Cypher, FalkorDB, and
//! Mermaid/HTML.

use codegraph_core::{CodeGraph, Confidence, Edge, Node, NodeId, NodeKind};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[allow(unused_imports)]
use crate::*;

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

/// Export the graph as GraphML for external graph tools (yEd, Gephi,
/// Cytoscape). Nodes carry kind/label/path/language attributes and edges
/// carry kind/confidence plus relation/source provenance where present.
pub fn export_graphml(graph: &CodeGraph) -> String {
    let mut output = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n\
         \x20 <key id=\"node_kind\" for=\"node\" attr.name=\"kind\" attr.type=\"string\"/>\n\
         \x20 <key id=\"node_label\" for=\"node\" attr.name=\"label\" attr.type=\"string\"/>\n\
         \x20 <key id=\"node_path\" for=\"node\" attr.name=\"path\" attr.type=\"string\"/>\n\
         \x20 <key id=\"node_language\" for=\"node\" attr.name=\"language\" attr.type=\"string\"/>\n\
         \x20 <key id=\"edge_kind\" for=\"edge\" attr.name=\"kind\" attr.type=\"string\"/>\n\
         \x20 <key id=\"edge_confidence\" for=\"edge\" attr.name=\"confidence\" attr.type=\"string\"/>\n\
         \x20 <key id=\"edge_relation\" for=\"edge\" attr.name=\"relation\" attr.type=\"string\"/>\n\
         \x20 <key id=\"edge_source\" for=\"edge\" attr.name=\"source\" attr.type=\"string\"/>\n\
         \x20 <graph id=\"codegraph\" edgedefault=\"directed\">\n",
    );

    for node in &graph.nodes {
        output.push_str(&format!("    <node id=\"{}\">\n", node.id));
        output.push_str(&format!(
            "      <data key=\"node_kind\">{}</data>\n",
            xml_escape(&kind_name(&node.kind))
        ));
        output.push_str(&format!(
            "      <data key=\"node_label\">{}</data>\n",
            xml_escape(&node.label)
        ));
        let path = node
            .span
            .as_ref()
            .map(|span| span.path.as_str())
            .or_else(|| node.metadata.get("path").map(String::as_str));
        if let Some(path) = path {
            output.push_str(&format!(
                "      <data key=\"node_path\">{}</data>\n",
                xml_escape(path)
            ));
        }
        if let Some(language) = node.metadata.get("language") {
            output.push_str(&format!(
                "      <data key=\"node_language\">{}</data>\n",
                xml_escape(language)
            ));
        }
        output.push_str("    </node>\n");
    }

    for edge in &graph.edges {
        output.push_str(&format!(
            "    <edge source=\"{}\" target=\"{}\">\n",
            edge.source, edge.target
        ));
        output.push_str(&format!(
            "      <data key=\"edge_kind\">{}</data>\n",
            xml_escape(&edge_kind_name(&edge.kind))
        ));
        output.push_str(&format!(
            "      <data key=\"edge_confidence\">{}</data>\n",
            xml_escape(&confidence_name(edge.confidence))
        ));
        if let Some(relation) = edge.metadata.get("relation") {
            output.push_str(&format!(
                "      <data key=\"edge_relation\">{}</data>\n",
                xml_escape(relation)
            ));
        }
        if let Some(source) = edge.metadata.get("source") {
            output.push_str(&format!(
                "      <data key=\"edge_source\">{}</data>\n",
                xml_escape(source)
            ));
        }
        output.push_str("    </edge>\n");
    }

    output.push_str("  </graph>\n</graphml>\n");
    output
}

pub(crate) fn xml_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Escape a string for a single-quoted Cypher literal.
pub(crate) fn cypher_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\'', "\\'")
}

/// PascalCase node label from snake_case kind names.
pub(crate) fn cypher_label(kind_name: &str) -> String {
    kind_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

pub(crate) fn cypher_statements(graph: &CodeGraph, if_not_exists: bool) -> Vec<String> {
    let mut statements = Vec::with_capacity(graph.nodes.len() + graph.edges.len() + 1);
    statements.push(if if_not_exists {
        "CREATE INDEX codegraph_node_id IF NOT EXISTS FOR (n:CodeNode) ON (n.id)".to_string()
    } else {
        "CREATE INDEX FOR (n:CodeNode) ON (n.id)".to_string()
    });

    for node in &graph.nodes {
        let mut properties = vec![
            format!("id: {}", node.id.0),
            format!("kind: '{}'", cypher_escape(&kind_name(&node.kind))),
            format!("label: '{}'", cypher_escape(&node.label)),
        ];
        if let Some(span) = &node.span {
            properties.push(format!("path: '{}'", cypher_escape(&span.path)));
            properties.push(format!("line: {}", span.start_line));
        } else if let Some(path) = node.metadata.get("path") {
            properties.push(format!("path: '{}'", cypher_escape(path)));
        }
        if let Some(language) = node.metadata.get("language") {
            properties.push(format!("language: '{}'", cypher_escape(language)));
        }
        statements.push(format!(
            "CREATE (:CodeNode:{} {{{}}})",
            cypher_label(&kind_name(&node.kind)),
            properties.join(", ")
        ));
    }

    for edge in &graph.edges {
        let mut properties = vec![format!(
            "confidence: '{}'",
            cypher_escape(&confidence_name(edge.confidence))
        )];
        if let Some(relation) = edge.metadata.get("relation") {
            properties.push(format!("relation: '{}'", cypher_escape(relation)));
        }
        if let Some(source) = edge.metadata.get("source") {
            properties.push(format!("source: '{}'", cypher_escape(source)));
        }
        statements.push(format!(
            "MATCH (a:CodeNode {{id: {}}}), (b:CodeNode {{id: {}}}) CREATE (a)-[:{} {{{}}}]->(b)",
            edge.source.0,
            edge.target.0,
            edge_kind_name(&edge.kind).to_uppercase(),
            properties.join(", ")
        ));
    }
    statements
}

/// Export the graph as a Neo4j Cypher script: pipe into `cypher-shell` to
/// load nodes (labelled `CodeNode` plus their kind) and typed relationships
/// with confidence/provenance properties.
pub fn export_cypher(graph: &CodeGraph) -> String {
    let mut output = String::from("// CodeGraph export for Neo4j: cypher-shell -f graph.cypher\n");
    for statement in cypher_statements(graph, true) {
        output.push_str(&statement);
        output.push_str(";\n");
    }
    output
}

/// Export the graph as a FalkorDB load script: each Cypher statement wrapped
/// in `GRAPH.QUERY` so the file pipes straight into `redis-cli`.
pub fn export_falkordb(graph: &CodeGraph, graph_key: &str) -> String {
    let key = graph_key.replace(char::is_whitespace, "-");
    let mut output = String::from("# CodeGraph export for FalkorDB: redis-cli < graph.falkordb\n");
    for statement in cypher_statements(graph, false) {
        let escaped = statement.replace('\\', "\\\\").replace('"', "\\\"");
        output.push_str(&format!("GRAPH.QUERY {key} \"{escaped}\"\n"));
    }
    output
}

pub const DEFAULT_SVG_NODE_LIMIT: usize = 500;
pub const DEFAULT_SVG_EDGE_LIMIT: usize = 1500;

/// Render the graph as a self-contained SVG: nodes on a deterministic
/// circle (sorted by kind, then label, then id, so equal graphs render
/// byte-identical images), edges as confidence-colored chords, node
/// `<title>` tooltips, and a kind legend. Large graphs truncate to the
/// given limits and record the truncation in an SVG comment.
pub fn export_svg(graph: &CodeGraph, node_limit: usize, edge_limit: usize) -> String {
    let node_limit = node_limit.max(1);
    let edge_limit = edge_limit.max(1);

    // Pick the highest-degree nodes first so a truncated render keeps its
    // edges (a kind/label slice of a large graph is mostly disconnected),
    // then lay the selection out in kind/label order for stable grouping.
    let mut degree: BTreeMap<NodeId, usize> = BTreeMap::new();
    for edge in &graph.edges {
        *degree.entry(edge.source).or_default() += 1;
        *degree.entry(edge.target).or_default() += 1;
    }
    let mut nodes: Vec<&Node> = graph.nodes.iter().collect();
    let total_nodes = nodes.len();
    if nodes.len() > node_limit {
        nodes.sort_by(|left, right| {
            degree
                .get(&right.id)
                .unwrap_or(&0)
                .cmp(degree.get(&left.id).unwrap_or(&0))
                .then_with(|| left.id.0.cmp(&right.id.0))
        });
        nodes.truncate(node_limit);
    }
    nodes.sort_by(|left, right| {
        kind_name(&left.kind)
            .cmp(&kind_name(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.0.cmp(&right.id.0))
    });

    let mut position_by_id = BTreeMap::new();
    let count = nodes.len().max(1);
    let radius = 240.0_f64.max(count as f64 * 14.0 / (2.0 * std::f64::consts::PI));
    let padding = 80.0;
    let center = radius + padding;
    let size = center * 2.0;
    for (index, node) in nodes.iter().enumerate() {
        let angle = index as f64 / count as f64 * 2.0 * std::f64::consts::PI;
        let x = center + radius * angle.cos();
        let y = center + radius * angle.sin();
        position_by_id.insert(node.id, (x, y));
    }

    let mut edges: Vec<&Edge> = graph
        .edges
        .iter()
        .filter(|edge| {
            position_by_id.contains_key(&edge.source) && position_by_id.contains_key(&edge.target)
        })
        .collect();
    let total_edges = edges.len();
    edges.truncate(edge_limit);

    let mut output = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {size:.0} {size:.0}\" \
         width=\"{size:.0}\" height=\"{size:.0}\" font-family=\"Inter, sans-serif\">\n"
    );
    output.push_str(&format!(
        "<!-- codegraph svg export: {} of {total_nodes} nodes, {} of {total_edges} edges -->\n",
        nodes.len(),
        edges.len()
    ));
    output.push_str(&format!(
        "  <rect width=\"{size:.0}\" height=\"{size:.0}\" fill=\"#101418\"/>\n"
    ));

    for edge in &edges {
        let (x1, y1) = position_by_id[&edge.source];
        let (x2, y2) = position_by_id[&edge.target];
        output.push_str(&format!(
            "  <line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" \
             stroke=\"{}\" stroke-width=\"0.7\" stroke-opacity=\"0.35\"/>\n",
            svg_confidence_color(edge.confidence)
        ));
    }

    let draw_labels = nodes.len() <= 80;
    for node in &nodes {
        let (x, y) = position_by_id[&node.id];
        output.push_str(&format!(
            "  <circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"5\" fill=\"{}\"><title>{} ({})</title></circle>\n",
            svg_kind_color(&node.kind),
            xml_escape(&node.label),
            xml_escape(&kind_name(&node.kind))
        ));
        if draw_labels {
            output.push_str(&format!(
                "  <text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" fill=\"#c9d2da\">{}</text>\n",
                x + 8.0,
                y + 3.0,
                xml_escape(truncate_svg_label(&node.label))
            ));
        }
    }

    let mut kinds: Vec<String> = nodes
        .iter()
        .map(|node| kind_name(&node.kind))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    kinds.sort();
    for (index, kind) in kinds.iter().enumerate() {
        let y = 20.0 + index as f64 * 16.0;
        output.push_str(&format!(
            "  <circle cx=\"16\" cy=\"{:.1}\" r=\"5\" fill=\"{}\"/>\n  <text x=\"26\" y=\"{:.1}\" font-size=\"11\" fill=\"#c9d2da\">{}</text>\n",
            y,
            svg_kind_color_by_name(kind),
            y + 4.0,
            xml_escape(kind)
        ));
    }

    output.push_str("</svg>\n");
    output
}

fn truncate_svg_label(label: &str) -> &str {
    let mut end = label.len().min(40);
    while !label.is_char_boundary(end) {
        end -= 1;
    }
    &label[..end]
}

fn svg_kind_color(kind: &NodeKind) -> &'static str {
    svg_kind_color_by_name(&kind_name(kind))
}

/// Mirrors the web canvas palette in `static/js/03-state.js`.
fn svg_kind_color_by_name(kind: &str) -> &'static str {
    match kind {
        "repository" | "entrypoint" => "#5cc8a7",
        "directory" => "#7f9cff",
        "file" => "#67b7dc",
        "module" => "#8ccf7e",
        "function" => "#f2c14e",
        "type" => "#df7e7e",
        "external_dependency" => "#b88ee6",
        "config" => "#e5b454",
        "environment" => "#d8a657",
        "control_flow" => "#9aa7d8",
        _ => "#a5adb3",
    }
}

fn svg_confidence_color(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Exact => "#5cc8a7",
        Confidence::Semantic => "#67b7dc",
        Confidence::Syntactic => "#c9d2da",
        Confidence::Heuristic => "#e5b454",
        Confidence::Unknown => "#a5adb3",
    }
}

pub const DEFAULT_MERMAID_NODE_LIMIT: usize = 300;
pub const DEFAULT_MERMAID_EDGE_LIMIT: usize = 600;

/// Render the graph as a bounded Mermaid flowchart. Mermaid rendering
/// degrades on very large diagrams, so nodes and edges are capped and the
/// truncation is stated in a `%%` comment instead of failing silently.
pub(crate) fn graph_mermaid(graph: &CodeGraph, node_limit: usize, edge_limit: usize) -> String {
    let node_limit = node_limit.max(1);
    let edge_limit = edge_limit.max(1);
    let mut lines = vec!["flowchart LR".to_string()];
    let included: BTreeSet<NodeId> = graph
        .nodes
        .iter()
        .take(node_limit)
        .map(|node| node.id)
        .collect();
    for node in graph.nodes.iter().take(node_limit) {
        lines.push(format!(
            "  {}[\"{}\"]",
            node.id,
            mermaid_escape(&format!("{} ({})", node.label, kind_name(&node.kind)))
        ));
    }
    let mut rendered_edges = 0usize;
    for edge in &graph.edges {
        if rendered_edges >= edge_limit {
            break;
        }
        if !included.contains(&edge.source) || !included.contains(&edge.target) {
            continue;
        }
        lines.push(format!(
            "  {} -->|{}| {}",
            edge.source,
            mermaid_escape(&format!(
                "{}/{}",
                edge_kind_name(&edge.kind),
                confidence_name(edge.confidence)
            )),
            edge.target
        ));
        rendered_edges += 1;
    }
    if graph.nodes.len() > node_limit || graph.edges.len() > rendered_edges {
        lines.push(format!(
            "  %% truncated: showing {} of {} nodes and {} of {} edges",
            included.len(),
            graph.nodes.len(),
            rendered_edges,
            graph.edges.len()
        ));
    }
    lines.join("\n")
}

#[derive(Debug, Clone)]
pub struct MermaidSection {
    pub title: String,
    pub mermaid: String,
}

/// Wrap Mermaid diagrams in a self-describing HTML page. Rendering uses the
/// Mermaid CDN when online; each section also carries its raw Mermaid source
/// in a `<details>` block so the artifact stays useful offline.
pub fn export_mermaid_html(title: &str, sections: &[MermaidSection]) -> String {
    let mut body = String::new();
    for section in sections {
        body.push_str(&format!(
            "  <section>\n    <h2>{}</h2>\n    <pre class=\"mermaid\">\n{}\n    </pre>\n    <details>\n      <summary>Mermaid source</summary>\n      <pre class=\"mermaid-source\">{}</pre>\n    </details>\n  </section>\n",
            xml_escape(&section.title),
            xml_escape(&section.mermaid),
            xml_escape(&section.mermaid)
        ));
    }
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<style>\nbody {{ font-family: -apple-system, 'Segoe UI', Roboto, sans-serif; margin: 2rem; color: #1a202c; }}\nsection {{ margin-bottom: 2.5rem; }}\npre.mermaid {{ background: #f7fafc; border: 1px solid #e2e8f0; border-radius: 8px; padding: 1rem; overflow-x: auto; }}\npre.mermaid-source {{ background: #edf2f7; border-radius: 6px; padding: 0.75rem; overflow-x: auto; font-size: 0.85rem; }}\n.note {{ color: #4a5568; font-size: 0.9rem; }}\n</style>\n</head>\n<body>\n<h1>{title}</h1>\n<p class=\"note\">Diagrams render via the Mermaid CDN when online; the raw Mermaid source under each diagram works offline and in wikis.</p>\n{body}<script type=\"module\">\nimport mermaid from \"https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs\";\nmermaid.initialize({{ startOnLoad: true, securityLevel: \"strict\" }});\n</script>\n</body>\n</html>\n",
        title = xml_escape(title),
        body = body
    )
}

/// Export the full graph as one Mermaid flowchart wrapped in HTML.
pub fn export_graph_mermaid_html(
    graph: &CodeGraph,
    node_limit: usize,
    edge_limit: usize,
) -> String {
    let root_label = graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .map(|node| node.label.as_str())
        .unwrap_or("graph");
    export_mermaid_html(
        &format!("CodeGraph: {root_label}"),
        &[MermaidSection {
            title: "Graph".to_string(),
            mermaid: graph_mermaid(graph, node_limit, edge_limit),
        }],
    )
}
