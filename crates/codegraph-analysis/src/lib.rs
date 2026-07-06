use codegraph_core::{CodeGraph, Edge, EdgeKind, Node, NodeId, NodeKind};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSummary {
    pub nodes: usize,
    pub edges: usize,
    pub node_kinds: BTreeMap<String, usize>,
    pub edge_kinds: BTreeMap<String, usize>,
    pub languages: BTreeMap<String, usize>,
    pub entrypoints: usize,
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
pub struct TraceNode {
    pub node: Node,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsightReport {
    pub total: usize,
    pub by_severity: BTreeMap<String, usize>,
    pub insights: Vec<Insight>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Insight {
    pub kind: String,
    pub severity: InsightSeverity,
    pub message: String,
    pub nodes: Vec<NodeId>,
    pub edges: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightSeverity {
    Info,
    Warning,
    Error,
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
    let mut languages = BTreeMap::new();

    for node in &graph.nodes {
        *node_kinds.entry(kind_name(&node.kind)).or_insert(0) += 1;
        if let Some(language) = node.metadata.get("language") {
            *languages.entry(language.clone()).or_insert(0) += 1;
        }
    }

    for edge in &graph.edges {
        *edge_kinds.entry(edge_kind_name(&edge.kind)).or_insert(0) += 1;
    }

    GraphSummary {
        nodes: graph.nodes.len(),
        edges: graph.edges.len(),
        node_kinds,
        edge_kinds,
        languages,
        entrypoints: graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Entrypoint)
            .count(),
    }
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
    add_unresolved_call_insights(graph, &mut insights);
    add_duplicate_function_insights(graph, &mut insights);
    add_orphan_function_insights(graph, &mut insights);
    add_error_flow_insights(graph, &mut insights);
    add_undeclared_import_insights(graph, &mut insights);
    insights.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.message.cmp(&right.message))
    });

    let mut by_severity = BTreeMap::new();
    for insight in &insights {
        *by_severity
            .entry(severity_name(insight.severity).to_string())
            .or_insert(0) += 1;
    }

    InsightReport {
        total: insights.len(),
        by_severity,
        insights,
    }
}

pub fn trace(graph: &CodeGraph, request: TraceRequest) -> Option<TraceResult> {
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
            if graph
                .edges
                .iter()
                .any(|edge| edge.source == node_id && is_trace_edge(&edge.kind))
            {
                truncated = true;
            }
            continue;
        }

        for edge in graph
            .edges
            .iter()
            .filter(|edge| edge.source == node_id && is_trace_edge(&edge.kind))
        {
            edges.push(edge.clone());
            if visited.insert(edge.target) {
                depths.insert(edge.target, depth + 1);
                queue.push_back((edge.target, depth + 1));
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

fn add_parse_error_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.metadata.contains_key("parse_error") {
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
        .filter(|edge| edge.kind == EdgeKind::Calls)
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

        let Some(import_node) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        let Some(import) = import_package_candidate(language, &import_node.label) else {
            continue;
        };
        if !declared_ecosystems.contains(import.ecosystem.as_str()) {
            continue;
        }
        if is_declared_package(&declared, &import.ecosystem, &import.package) {
            continue;
        }

        let source = graph
            .nodes
            .iter()
            .find(|node| node.id == edge.source)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
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

fn is_trace_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls
            | EdgeKind::Imports
            | EdgeKind::ReadsConfig
            | EdgeKind::ReadsEnvironment
            | EdgeKind::MayError
            | EdgeKind::DependsOn
    )
}

fn kind_name(kind: &codegraph_core::NodeKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| format!("{kind:?}").to_ascii_lowercase())
}

fn edge_kind_name(kind: &EdgeKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| format!("{kind:?}").to_ascii_lowercase())
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
    use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeKind};

    #[test]
    fn summary_counts_graph_facts() {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_edge(
            graph.root,
            main,
            EdgeKind::Entrypoint,
            Confidence::Syntactic,
        );

        let summary = summarize(&graph);

        assert_eq!(summary.nodes, 2);
        assert_eq!(summary.edges, 1);
        assert_eq!(summary.entrypoints, 1);
        assert_eq!(summary.node_kinds.get("function"), Some(&1));
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
    fn insights_report_undeclared_external_imports() {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node(NodeKind::File, "src/main.ts");
        let react = dependency_node(&mut graph, "react", "npm:react");
        graph.add_edge(file, react, EdgeKind::DependsOn, Confidence::Exact);

        let react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
        let express_import =
            import_node(&mut graph, "import express from \"express\";", "typescript");
        let fs_import = import_node(&mut graph, "import fs from \"node:fs\";", "typescript");
        graph.add_edge(file, react_import, EdgeKind::Imports, Confidence::Syntactic);
        graph.add_edge(
            file,
            express_import,
            EdgeKind::Imports,
            Confidence::Syntactic,
        );
        graph.add_edge(file, fs_import, EdgeKind::Imports, Confidence::Syntactic);

        let report = insights(&graph);
        assert!(report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("express")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("react")
        }));
        assert!(!report.insights.iter().any(|insight| {
            insight.kind == "undeclared_external_import" && insight.message.contains("node:fs")
        }));
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
}
