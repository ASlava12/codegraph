//! Minimal MCP stdio server exposing CodeGraph analysis tools.
//!
//! The transport is newline-delimited JSON-RPC 2.0 on stdin/stdout as defined
//! by the Model Context Protocol stdio transport. The server scans the target
//! project once at startup and serves bounded graph tools over that snapshot,
//! so assistants can query the repository graph instead of reading raw files.

use anyhow::Result;
use codegraph_analysis::{
    ImpactRequest, InsightFilter, InsightSeverity, JourneyRequest, ProjectReportLimits, TraceStart,
    WorkflowFilters, WorkflowRequest, compact_query_result, filter_insight_report, impact,
    insights, journey, node_card, project_report, query_graph, workflow,
};
use codegraph_core::{CodeGraph, NodeId};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

pub struct McpServer {
    graph: CodeGraph,
    root: PathBuf,
    log_settings: crate::query_log::QueryLogSettings,
}

impl McpServer {
    pub fn new(graph: CodeGraph, root: PathBuf) -> Self {
        let log_settings = crate::query_log::load_settings(&root).unwrap_or_default();
        Self {
            graph,
            root,
            log_settings,
        }
    }

    pub fn run(&self) -> Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(response) = self.handle_line(&line) {
                let mut out = stdout.lock();
                out.write_all(response.as_bytes())?;
                out.write_all(b"\n")?;
                out.flush()?;
            }
        }
        Ok(())
    }

    pub fn handle_line(&self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(
                    json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": {"code": -32700, "message": format!("parse error: {error}")},
                    })
                    .to_string(),
                );
            }
        };
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        // Notifications never get a response.
        if id.is_none() || method.starts_with("notifications/") {
            return None;
        }
        let id = id.unwrap_or(Value::Null);
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        let body = match method {
            "initialize" => Ok(json!({
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
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => self.handle_tool_call(&params),
            _ => Err(format!("method `{method}` is not supported")),
        };

        Some(match body {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
            Err(message) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": message},
            })
            .to_string(),
        })
    }

    fn handle_tool_call(&self, params: &Value) -> Result<Value, String> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "tools/call requires a `name`".to_string())?;
        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        let started = std::time::Instant::now();
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
        self.log_tool_call(name, &args, &payload, started.elapsed().as_millis() as u64);

        match payload {
            Ok(value) => Ok(json!({
                "content": [{"type": "text", "text": value.to_string()}],
                "isError": false,
            })),
            Err(message) => Ok(json!({
                "content": [{"type": "text", "text": message}],
                "isError": true,
            })),
        }
    }

    /// Best-effort local query audit: never fails the tool call, only warns.
    fn log_tool_call(
        &self,
        name: &str,
        args: &Value,
        payload: &Result<Value, String>,
        duration_ms: u64,
    ) {
        if !self.log_settings.enabled {
            return;
        }
        let action = format!("mcp:{name}");
        let query = args.to_string();
        let (outcome, response) = match payload {
            Ok(value) => ("ok", Some(value.to_string())),
            Err(_) => ("error", None),
        };
        let event = crate::query_log::QueryLogEvent {
            surface: "mcp",
            action: &action,
            query: &query,
            outcome,
            duration_ms,
            response: response.as_deref(),
            recorded_at_unix: crate::query_log::unix_now(),
        };
        if let Err(error) = crate::query_log::log_query(&self.root, &self.log_settings, event) {
            eprintln!("warning: failed to write query log: {error:#}");
        }
    }

    fn tool_query_graph(&self, args: &Value) -> Result<Value, String> {
        let query = required_str(args, "query")?;
        let result = query_graph(&self.graph, query).map_err(|error| error.to_string())?;
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
            &self.graph,
            Some(&self.root),
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
        let result = query_graph(&self.graph, &expression).map_err(|error| error.to_string())?;
        serde_json::to_value(result).map_err(|error| error.to_string())
    }

    fn tool_shortest_path(&self, args: &Value) -> Result<Value, String> {
        let report = journey(
            &self.graph,
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
            &self.graph,
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
            insights(&self.graph),
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
            &self.graph,
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
            &self.graph,
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

fn tool_definitions() -> Vec<Value> {
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

    fn test_server() -> McpServer {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(graph.root, helper, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        McpServer::new(graph, PathBuf::from("."))
    }

    fn parse(response: Option<String>) -> Value {
        serde_json::from_str(&response.expect("response")).expect("valid json")
    }

    #[test]
    fn initialize_lists_capabilities_and_server_info() {
        let server = test_server();
        let response = parse(server.handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        ));
        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(response["result"]["serverInfo"]["name"], "codegraph");
        assert!(response["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn notifications_get_no_response() {
        let server = test_server();
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
    }

    #[test]
    fn tools_list_exposes_expected_tools() {
        let server = test_server();
        let response =
            parse(server.handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#));
        let tools = response["result"]["tools"].as_array().expect("tools");
        let names = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap_or_default())
            .collect::<Vec<_>>();
        for expected in [
            "query_graph",
            "get_node_card",
            "get_neighbors",
            "shortest_path",
            "workflow",
            "insights",
            "impact",
            "report",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
        assert!(tools.iter().all(|tool| tool["inputSchema"].is_object()));
    }

    #[test]
    fn tool_calls_return_graph_payloads() {
        let server = test_server();
        let response = parse(server.handle_line(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"query_graph","arguments":{"query":"nodes kind:function label:main"}}}"#,
        ));
        assert_eq!(response["result"]["isError"], false);
        let payload: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .expect("payload json");
        assert_eq!(payload["total_nodes"], 1);

        let path = parse(server.handle_line(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"shortest_path","arguments":{"from":"main","to":"helper"}}}"#,
        ));
        let journey: Value =
            serde_json::from_str(path["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(journey["total_paths"], 1);

        let neighbors = parse(server.handle_line(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"get_neighbors","arguments":{"target":"main","direction":"out"}}}"#,
        ));
        assert_eq!(neighbors["result"]["isError"], false);
    }

    #[test]
    fn tool_calls_are_audited_when_query_log_is_enabled() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .subsec_nanos();
        let root =
            std::env::temp_dir().join(format!("codegraph-mcp-log-{}-{nanos}", std::process::id()));
        fs::create_dir_all(root.join(".codegraph")).expect("config dir");
        fs::write(
            root.join(".codegraph").join("config.toml"),
            "[query_log]\nenabled = true\n",
        )
        .expect("config write");

        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);
        let server = McpServer::new(graph, root.clone());

        parse(server.handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_graph","arguments":{"query":"nodes kind:function"}}}"#,
        ));
        parse(server.handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ghost","arguments":{}}}"#,
        ));

        let report = crate::query_log::list_query_log(&root, None, 50).expect("query log report");
        assert_eq!(report.total, 2);
        assert_eq!(report.records[0].surface, "mcp");
        assert_eq!(report.records[0].action, "mcp:query_graph");
        assert!(report.records[0].query.contains("nodes kind:function"));
        assert_eq!(report.records[0].outcome, "ok");
        assert!(
            report.records[0].response_preview.is_none(),
            "responses stay out of the audit log unless opted in"
        );
        assert_eq!(report.records[1].action, "mcp:ghost");
        assert_eq!(report.records[1].outcome, "error");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn unknown_methods_and_tools_report_errors() {
        let server = test_server();
        let response =
            parse(server.handle_line(r#"{"jsonrpc":"2.0","id":6,"method":"resources/list"}"#));
        assert_eq!(response["error"]["code"], -32601);

        let tool = parse(server.handle_line(
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"ghost","arguments":{}}}"#,
        ));
        assert_eq!(tool["result"]["isError"], true);
    }
}
