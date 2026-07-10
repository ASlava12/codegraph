//! Graph export targets: DOT, NDJSON, GraphML, Cypher, FalkorDB, and
//! Mermaid/HTML.

use codegraph_core::{CodeGraph, NodeId};
use serde_json::json;
use std::collections::BTreeSet;

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
