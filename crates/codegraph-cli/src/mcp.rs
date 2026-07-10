//! MCP stdio transport over the shared analysis engine.
//!
//! The transport is newline-delimited JSON-RPC 2.0 on stdin/stdout as defined
//! by the Model Context Protocol stdio transport. The server scans the target
//! project once at startup and serves bounded graph tools over that snapshot
//! through [`codegraph_analysis::McpEngine`]; the same engine backs the
//! authenticated HTTP transport in `codegraph-server`. Tool calls feed the
//! local query audit log when the repository opts in.

use anyhow::Result;
use codegraph_analysis::{McpEngine, McpToolAudit};
use codegraph_core::CodeGraph;
use std::io::{BufRead, Write};
use std::path::PathBuf;

pub struct McpServer {
    graph: CodeGraph,
    root: PathBuf,
    fingerprint: Option<String>,
    log_settings: crate::query_log::QueryLogSettings,
}

impl McpServer {
    pub fn new(graph: CodeGraph, root: PathBuf, fingerprint: Option<String>) -> Self {
        let log_settings = crate::query_log::load_settings(&root).unwrap_or_default();
        Self {
            graph,
            root,
            fingerprint,
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
        let engine = McpEngine {
            graph: &self.graph,
            root: Some(&self.root),
            fingerprint: self.fingerprint.as_deref(),
        };
        let (response, audit) = engine.handle_line(line);
        if let Some(audit) = audit {
            self.log_tool_call(&audit);
        }
        response
    }

    /// Best-effort local query audit: never fails the tool call, only warns.
    fn log_tool_call(&self, audit: &McpToolAudit) {
        if !self.log_settings.enabled {
            return;
        }
        let action = format!("mcp:{}", audit.tool);
        let query = audit.arguments.to_string();
        let event = crate::query_log::QueryLogEvent {
            surface: "mcp",
            action: &action,
            query: &query,
            outcome: if audit.ok { "ok" } else { "error" },
            duration_ms: audit.duration_ms,
            response: audit.response_json.as_deref(),
            recorded_at_unix: crate::query_log::unix_now(),
        };
        if let Err(error) = crate::query_log::log_query(&self.root, &self.log_settings, event) {
            eprintln!("warning: failed to write query log: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{Confidence, EdgeKind, NodeKind};
    use serde_json::Value;

    fn test_server() -> McpServer {
        let mut graph = CodeGraph::new("repo");
        let main = graph.add_node(NodeKind::Function, "main");
        let helper = graph.add_node(NodeKind::Function, "helper");
        graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(graph.root, helper, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
        McpServer::new(graph, PathBuf::from("."), Some("fp-test".to_string()))
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
        let server = McpServer::new(graph, root.clone(), Some("fp-test".to_string()));

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
