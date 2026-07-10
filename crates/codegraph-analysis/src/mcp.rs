//! Transport-agnostic MCP message engine over the analysis APIs.
//!
//! The engine handles JSON-RPC 2.0 messages (`initialize`, `ping`,
//! `tools/list`, `tools/call`) against a borrowed graph snapshot so any
//! transport — the CLI stdio loop or the HTTP server — can serve the same
//! bounded graph tools. Tool calls return an [`McpToolAudit`] alongside the
//! response so transports can feed their own audit logging.

use crate::{
    ImpactRequest, InsightFilter, InsightSeverity, JourneyRequest, ProjectReportLimits, TraceStart,
    WorkflowFilters, WorkflowRequest, compact_query_result, filter_insight_report, impact,
    insights, journey, node_card, project_report, query_graph, workflow,
};
use codegraph_core::{CodeGraph, NodeId};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Instant;

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// One handled `tools/call`, reported back to the transport for audit logging.
#[derive(Debug, Clone)]
pub struct McpToolAudit {
    pub tool: String,
    pub arguments: Value,
    pub ok: bool,
    pub duration_ms: u64,
    /// Serialized tool payload when the call succeeded, for opt-in response logging.
    pub response_json: Option<String>,
}

pub struct McpEngine<'a> {
    pub graph: &'a CodeGraph,
    /// Project root for source previews in node cards.
    pub root: Option<&'a Path>,
}

impl McpEngine<'_> {
    /// Handle one newline-delimited JSON-RPC message; parse errors produce a
    /// JSON-RPC error response, notifications produce no response.
    pub fn handle_line(&self, line: &str) -> (Option<String>, Option<McpToolAudit>) {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return (
                    Some(
                        json!({
                            "jsonrpc": "2.0",
                            "id": Value::Null,
                            "error": {"code": -32700, "message": format!("parse error: {error}")},
                        })
                        .to_string(),
                    ),
                    None,
                );
            }
        };
        let (response, audit) = self.handle_message(&message);
        (response.map(|value| value.to_string()), audit)
    }

    /// Handle one parsed JSON-RPC message. Notifications return no response.
    pub fn handle_message(&self, message: &Value) -> (Option<Value>, Option<McpToolAudit>) {
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        // Notifications never get a response.
        if id.is_none() || method.starts_with("notifications/") {
            return (None, None);
        }
        let id = id.unwrap_or(Value::Null);
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        let (body, audit) = match method {
            "initialize" => (
                Ok(json!({
                    "protocolVersion": params
                        .get("protocolVersion")
                        .and_then(Value::as_str)
                        .unwrap_or(MCP_PROTOCOL_VERSION),
                    "capabilities": {"tools": {}},
                    "serverInfo": {
                        "name": "codegraph",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                })),
                None,
            ),
            "ping" => (Ok(json!({})), None),
            "tools/list" => (Ok(json!({"tools": mcp_tool_definitions()})), None),
            "tools/call" => self.handle_tool_call(&params),
            _ => (Err(format!("method `{method}` is not supported")), None),
        };

        let response = match body {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(message) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": message},
            }),
        };
        (Some(response), audit)
    }

    fn handle_tool_call(&self, params: &Value) -> (Result<Value, String>, Option<McpToolAudit>) {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return (Err("tools/call requires a `name`".to_string()), None);
        };
        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        let started = Instant::now();
        let payload = match name {
            "query_graph" => self.tool_query_graph(&args),
            "get_node_card" => self.tool_node_card(&args),
            "get_neighbors" => self.tool_neighbors(&args),
            "shortest_path" => self.tool_shortest_path(&args),
            "workflow" => self.tool_workflow(&args),
            "insights" => self.tool_insights(&args),
            "impact" => self.tool_impact(&args),
            "report" => self.tool_report(&args),
            _ => Err(format!("unknown tool `{name}`")),
        };
        let audit = McpToolAudit {
            tool: name.to_string(),
            arguments: args,
            ok: payload.is_ok(),
            duration_ms: started.elapsed().as_millis() as u64,
            response_json: payload.as_ref().ok().map(Value::to_string),
        };

        let body = match payload {
            Ok(value) => Ok(json!({
                "content": [{"type": "text", "text": value.to_string()}],
                "isError": false,
            })),
            Err(message) => Ok(json!({
                "content": [{"type": "text", "text": message}],
                "isError": true,
            })),
        };
        (body, Some(audit))
    }

    fn tool_query_graph(&self, args: &Value) -> Result<Value, String> {
        let query = required_str(args, "query")?;
        let result = query_graph(self.graph, query).map_err(|error| error.to_string())?;
        let result = if bool_arg(args, "compact", false) {
            compact_query_result(result)
        } else {
            result
        };
        serde_json::to_value(result).map_err(|error| error.to_string())
    }

    fn tool_node_card(&self, args: &Value) -> Result<Value, String> {
        let node_id = args
            .get("node_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| "get_node_card requires a numeric `node_id`".to_string())?;
        let card = node_card(
            self.graph,
            self.root,
            NodeId(node_id),
            usize_arg(args, "edge_limit", 80),
            u32_arg(args, "source_context", 5),
            usize_arg(args, "insight_limit", 8),
        )
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("node {node_id} was not found"))?;
        serde_json::to_value(card).map_err(|error| error.to_string())
    }

    fn tool_neighbors(&self, args: &Value) -> Result<Value, String> {
        let target = required_str(args, "target")?;
        let direction = args
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("out");
        let depth = usize_arg(args, "depth", 1).clamp(1, 8);
        let start = self
            .resolve_target(target)
            .ok_or_else(|| format!("neighbors target `{target}` did not match a node"))?;
        let mut expression = format!(
            "neighbors id:{} direction:{direction} depth:{depth}",
            start.0
        );
        if let Some(edge_kind) = args.get("edge_kind").and_then(Value::as_str) {
            expression.push_str(&format!(" edge_kind:{edge_kind}"));
        }
        let result = query_graph(self.graph, &expression).map_err(|error| error.to_string())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    }

    fn tool_shortest_path(&self, args: &Value) -> Result<Value, String> {
        let report = journey(
            self.graph,
            JourneyRequest {
                from: required_str(args, "from")?.to_string(),
                to: required_str(args, "to")?.to_string(),
                max_depth: usize_arg(args, "depth", 8),
                path_limit: usize_arg(args, "paths", 3),
            },
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn tool_workflow(&self, args: &Value) -> Result<Value, String> {
        let target = required_str(args, "target")?;
        let start = self
            .resolve_target(target)
            .map(TraceStart::NodeId)
            .unwrap_or_else(|| TraceStart::Label(target.to_string()));
        let report = workflow(
            self.graph,
            WorkflowRequest {
                start,
                max_depth: usize_arg(args, "depth", 4),
                block_limit: usize_arg(args, "block_limit", 200),
                filters: WorkflowFilters::default(),
                compact: bool_arg(args, "compact", false),
            },
        )
        .ok_or_else(|| format!("workflow target `{target}` did not match a node"))?;
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn tool_insights(&self, args: &Value) -> Result<Value, String> {
        let severity = match args.get("severity").and_then(Value::as_str) {
            None => None,
            Some("info") => Some(InsightSeverity::Info),
            Some("warning") => Some(InsightSeverity::Warning),
            Some("error") => Some(InsightSeverity::Error),
            Some(other) => {
                return Err(format!(
                    "unknown severity `{other}`; expected info, warning, or error"
                ));
            }
        };
        let report = filter_insight_report(
            insights(self.graph),
            &InsightFilter {
                severity,
                kind: args
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                search: args
                    .get("search")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                limit: usize_arg(args, "limit", 50),
            },
        );
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn tool_impact(&self, args: &Value) -> Result<Value, String> {
        let report = impact(
            self.graph,
            ImpactRequest {
                target: required_str(args, "target")?.to_string(),
                max_depth: usize_arg(args, "depth", 6),
                limit: usize_arg(args, "limit", 100),
            },
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn tool_report(&self, args: &Value) -> Result<Value, String> {
        let report = project_report(
            self.graph,
            ProjectReportLimits {
                insight_limit: usize_arg(args, "insight_limit", 100),
                ..ProjectReportLimits::default()
            },
        );
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn resolve_target(&self, target: &str) -> Option<NodeId> {
        let trimmed = target.trim();
        let id_text = trimmed.strip_prefix('n').unwrap_or(trimmed);
        if let Ok(id) = id_text.parse::<u64>() {
            let node_id = NodeId(id);
            if self.graph.nodes.iter().any(|node| node.id == node_id) {
                return Some(node_id);
            }
        }
        self.graph
            .nodes
            .iter()
            .find(|node| node.label == trimmed)
            .map(|node| node.id)
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing required string argument `{key}`"))
}

fn usize_arg(args: &Value, key: &str, default: usize) -> usize {
    args.get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(default)
}

fn u32_arg(args: &Value, key: &str, default: u32) -> u32 {
    args.get(key)
        .and_then(Value::as_u64)
        .map(|value| value as u32)
        .unwrap_or(default)
}

fn bool_arg(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(default)
}

pub fn mcp_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "query_graph",
            "description": "Run a CodeGraph query expression (nodes, edges, calls, trace, neighbors, path, configs, errors, insights, unreachable, sql, docs, ...) and return a bounded graph slice with facets.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Query expression, e.g. `nodes kind:function label:main` or `path from:main to:load_config depth:6`."},
                    "compact": {"type": "boolean", "description": "Collapse repeated low-signal nodes."}
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "get_node_card",
            "description": "Return an investigation card for one node: summary, source preview, dependencies, and related risks.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "node_id": {"type": "integer", "description": "Graph node id."},
                    "edge_limit": {"type": "integer", "description": "Maximum neighbor edges (default 80)."},
                    "source_context": {"type": "integer", "description": "Source preview context lines (default 5)."},
                    "insight_limit": {"type": "integer", "description": "Maximum related risks (default 8)."}
                },
                "required": ["node_id"]
            }
        }),
        json!({
            "name": "get_neighbors",
            "description": "Return the dependency neighborhood of a node by label or id, with direction and depth control.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "Node label or id such as `load_config` or `n42`."},
                    "direction": {"type": "string", "enum": ["in", "out", "both"], "description": "Traversal direction (default out)."},
                    "depth": {"type": "integer", "description": "Traversal depth 1-8 (default 1)."},
                    "edge_kind": {"type": "string", "description": "Restrict to one edge kind such as calls or imports."}
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "shortest_path",
            "description": "Return ranked execution journeys between two nodes: step-numbered chains with confidence ranking, fragile-hop flags, per-hop explanations, and risk summaries.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": {"type": "string", "description": "Start label or node id."},
                    "to": {"type": "string", "description": "Target label or node id."},
                    "depth": {"type": "integer", "description": "Maximum path depth (default 8)."},
                    "paths": {"type": "integer", "description": "Maximum ranked alternative paths (default 3)."}
                },
                "required": ["from", "to"]
            }
        }),
        json!({
            "name": "workflow",
            "description": "Return a block-style workflow (start/call/branch/loop/async/return/error blocks with transitions and risks) from a node label or id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "Start label or node id."},
                    "depth": {"type": "integer", "description": "Traversal depth (default 4)."},
                    "block_limit": {"type": "integer", "description": "Maximum blocks (default 200)."},
                    "compact": {"type": "boolean", "description": "Collapse repeated low-signal blocks."}
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "insights",
            "description": "Return investigation findings (unresolved calls, dependency issues, sensitive defaults, risky comments, ...) with severity/kind/search filters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "severity": {"type": "string", "enum": ["info", "warning", "error"]},
                    "kind": {"type": "string", "description": "Insight kind substring filter."},
                    "search": {"type": "string", "description": "Message/node/edge substring filter."},
                    "limit": {"type": "integer", "description": "Maximum findings (default 50)."}
                }
            }
        }),
        json!({
            "name": "impact",
            "description": "Return the blast radius of changing a node: transitive dependents, affected entrypoints/routes/tests, and a risk-weighted impact score.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "Node label or id."},
                    "depth": {"type": "integer", "description": "Reverse dependency depth (default 6)."},
                    "limit": {"type": "integer", "description": "Maximum listed dependents (default 100)."}
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "report",
            "description": "Return the project knowledge report: summary, key concepts, communities, hotspots, surprising links, risk summary, and quality gate.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "insight_limit": {"type": "integer", "description": "Maximum returned insights (default 100)."}
                }
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Confidence, EdgeKind, NodeKind};

    fn test_graph() -> CodeGraph {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(graph.root, helper, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        graph
    }

    #[test]
    fn handle_message_returns_tool_payload_and_audit() {
        let graph = test_graph();
        let engine = McpEngine {
            graph: &graph,
            root: None,
        };
        let message: Value = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_graph","arguments":{"query":"nodes kind:function label:main"}}}"#,
        )
        .unwrap();
        let (response, audit) = engine.handle_message(&message);
        let response = response.expect("tool call response");
        assert_eq!(response["result"]["isError"], false);
        let audit = audit.expect("tool call audit");
        assert_eq!(audit.tool, "query_graph");
        assert!(audit.ok);
        assert!(
            audit
                .response_json
                .as_deref()
                .is_some_and(|payload| payload.contains("total_nodes"))
        );

        let unknown: Value = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ghost","arguments":{}}}"#,
        )
        .unwrap();
        let (response, audit) = engine.handle_message(&unknown);
        assert_eq!(response.expect("error response")["result"]["isError"], true);
        let audit = audit.expect("audit for failed call");
        assert!(!audit.ok);
        assert!(audit.response_json.is_none());
    }

    #[test]
    fn notifications_and_non_tool_methods_have_no_audit() {
        let graph = test_graph();
        let engine = McpEngine {
            graph: &graph,
            root: None,
        };
        let notification: Value =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .unwrap();
        assert!(matches!(engine.handle_message(&notification), (None, None)));

        let list: Value =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).unwrap();
        let (response, audit) = engine.handle_message(&list);
        assert!(audit.is_none());
        assert_eq!(
            response.expect("list response")["result"]["tools"]
                .as_array()
                .expect("tools array")
                .len(),
            8
        );
    }
}
