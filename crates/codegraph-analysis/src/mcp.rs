//! Transport-agnostic MCP message engine over the analysis APIs.
//!
//! The engine handles JSON-RPC 2.0 messages (`initialize`, `ping`,
//! `tools/list`, `tools/call`) against a borrowed graph snapshot so any
//! transport — the CLI stdio loop or the HTTP server — can serve the same
//! bounded graph tools. Tool calls return an [`McpToolAudit`] alongside the
//! response so transports can feed their own audit logging.

use crate::{
    ImpactRequest, InsightFilter, InsightSeverity, JourneyRequest, NaturalQueryRequest,
    NodeCardRequest, ProjectReportLimits, RefactorContextRequest, SourceSearchRequest, TraceStart,
    WorkflowFilters, WorkflowRequest, compact_query_result, filter_insight_report, impact,
    impact_fast, insights, journey, memory, missing_node_error, natural_query, node_card,
    node_card_fast, project_report, query_graph, refactor_context, resolve_node_reference,
    search_source, workflow,
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
    /// Project root for source previews, source search, and memory records.
    pub root: Option<&'a Path>,
    /// Current project fingerprint hash for memory staleness marking.
    pub fingerprint: Option<&'a str>,
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
                    // Which project this server answers about. It scans one
                    // root at startup, and a caller that believes otherwise
                    // reads answers about the wrong repository.
                    "instructions": match self.root {
                        Some(root) => format!(
                            "Every tool answers about the project at {}, scanned when this server started. To ask about another project, start a server with that path.",
                            root.display()
                        ),
                        None => "Every tool answers about the project this server scanned when it started.".to_string(),
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
        // No tool takes a project path: this server scanned one root when it
        // started. A call that names another project would otherwise be
        // answered from the wrong graph without a word.
        let named_project = ["path", "root", "project"]
            .into_iter()
            .find(|key| args.get(*key).is_some_and(|value| !value.is_null()));

        let started = Instant::now();
        let payload = if let Some(named) = named_project {
            let root = self
                .root
                .map(|root| root.display().to_string())
                .unwrap_or_else(|| "the project it scanned".to_string());
            Err(format!(
                "`{named}` is not a parameter of `{name}`: this server answers about {root}, scanned when it started. Start a server with another path to ask about another project."
            ))
        } else {
            match name {
                "query_graph" => self.tool_query_graph(&args),
                "get_node_card" => self.tool_node_card(&args),
                "get_neighbors" => self.tool_neighbors(&args),
                "shortest_path" => self.tool_shortest_path(&args),
                "workflow" => self.tool_workflow(&args),
                "insights" => self.tool_insights(&args),
                "impact" => self.tool_impact(&args),
                "report" => self.tool_report(&args),
                "refactor_context" => self.tool_refactor_context(&args),
                "ask" => self.tool_ask(&args),
                "source_search" => self.tool_source_search(&args),
                "memory_save" => self.tool_memory_save(&args),
                "memory_list" => self.tool_memory_list(&args),
                "memory_reflect" => self.tool_memory_reflect(&args),
                _ => Err(format!("unknown tool `{name}`")),
            }
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
        // A name that was given and did not resolve is a different answer
        // from a name that was never given.
        let node_id = match args.get("node_id") {
            Some(Value::Number(number)) => number
                .as_u64()
                .ok_or_else(|| format!("node_id `{number}` is not a node id")),
            Some(Value::String(value)) => self
                .resolve_target(value)
                .map(|id| id.0)
                .ok_or_else(|| missing_node_error(self.graph, "node_id", value).to_string()),
            // `resolve_target` already took a label here while the CLI and
            // `/api/node-card` refused one; what it did not do is say which
            // definition it picked.
            _ => Err(
                "get_node_card requires a `node_id`: a label, an integer, an n-prefixed id, or a stable cg-* id"
                    .to_string(),
            ),
        }?;
        let card_builder = if bool_arg(args, "include_insights", false) {
            node_card
        } else {
            node_card_fast
        };
        let card = card_builder(
            self.graph,
            self.root,
            NodeCardRequest {
                node_id: NodeId(node_id),
                edge_limit: usize_arg(args, "edge_limit", 24),
                source_context: u32_arg(args, "source_context", 5),
                insight_limit: usize_arg(args, "insight_limit", 8),
            },
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
        let start = self.resolve_target(target).ok_or_else(|| {
            missing_node_error(self.graph, "neighbors target", target).to_string()
        })?;
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
                max_fanout: args
                    .get("max_fanout")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize),
            },
        )
        .ok_or_else(|| missing_node_error(self.graph, "workflow target", target).to_string())?;
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
        let impact_builder = if bool_arg(args, "include_risks", false) {
            impact
        } else {
            impact_fast
        };
        let report = impact_builder(
            self.graph,
            ImpactRequest {
                target: required_str(args, "target")?.to_string(),
                max_depth: usize_arg(args, "depth", 6),
                limit: usize_arg(args, "limit", 40),
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

    fn tool_refactor_context(&self, args: &Value) -> Result<Value, String> {
        let bundle = refactor_context(
            self.graph,
            RefactorContextRequest {
                target: required_str(args, "target")?.to_string(),
                from: args
                    .get("from")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                max_depth: usize_arg(args, "depth", 6),
                path_limit: usize_arg(args, "paths", 3),
                dependent_limit: usize_arg(args, "dependent_limit", 50),
                risk_limit: usize_arg(args, "risk_limit", 50),
            },
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(bundle).map_err(|error| error.to_string())
    }

    fn tool_ask(&self, args: &Value) -> Result<Value, String> {
        let report = natural_query(
            self.graph,
            NaturalQueryRequest {
                question: required_str(args, "question")?.to_string(),
                compact: bool_arg(args, "compact", false),
            },
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn tool_source_search(&self, args: &Value) -> Result<Value, String> {
        let root = self
            .root
            .ok_or_else(|| "source_search requires a project root".to_string())?;
        let result = search_source(
            root,
            &SourceSearchRequest {
                query: required_str(args, "query")?.to_string(),
                path_filter: args
                    .get("path")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                case_sensitive: bool_arg(args, "case_sensitive", false),
                limit: usize_arg(args, "limit", 50),
                context: usize_arg(args, "context", 2),
                include_hidden: false,
                include_ignored: false,
                ignored_names: std::collections::BTreeSet::new(),
                ignored_globs: std::collections::BTreeSet::new(),
                max_file_size: 2 * 1024 * 1024,
            },
        );
        serde_json::to_value(result).map_err(|error| error.to_string())
    }

    fn memory_root(&self) -> Result<&Path, String> {
        self.root
            .ok_or_else(|| "memory tools require a project root".to_string())
    }

    fn memory_fingerprint(&self) -> Result<&str, String> {
        self.fingerprint.ok_or_else(|| {
            "memory tools require a project fingerprint; this transport did not provide one"
                .to_string()
        })
    }

    fn tool_memory_save(&self, args: &Value) -> Result<Value, String> {
        let root = self.memory_root()?;
        let fingerprint = self.memory_fingerprint()?;
        let outcome: memory::MemoryOutcome = required_str(args, "outcome")?.parse()?;
        // A durable `cg-*` id is the reference worth storing, and older
        // callers send numbers.
        let node_ids = args
            .get("node_ids")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| match value {
                        Value::String(text) => Some(text.clone()),
                        Value::Number(number) => number.as_u64().map(|id| format!("n{id}")),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let recorded_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        let record = memory::save_memory(
            root,
            memory::MemorySaveRequest {
                query: required_str(args, "query")?.to_string(),
                outcome,
                note: args
                    .get("note")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                node_ids,
                fingerprint: fingerprint.to_string(),
                recorded_at_unix,
            },
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(record).map_err(|error| error.to_string())
    }

    fn tool_memory_list(&self, args: &Value) -> Result<Value, String> {
        let root = self.memory_root()?;
        let fingerprint = self.memory_fingerprint()?;
        let outcome = match args.get("outcome").and_then(Value::as_str) {
            Some(value) => Some(value.parse::<memory::MemoryOutcome>()?),
            None => None,
        };
        let report = memory::list_memory(
            root,
            fingerprint,
            outcome,
            bool_arg(args, "only_stale", false),
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn tool_memory_reflect(&self, _args: &Value) -> Result<Value, String> {
        let root = self.memory_root()?;
        let fingerprint = self.memory_fingerprint()?;
        // A memory may hold either form, so both answer to the same node.
        let node_labels = self
            .graph
            .nodes
            .iter()
            .flat_map(|node| {
                let label = node.label.clone();
                let durable = node
                    .metadata
                    .get("stable_id")
                    .map(|stable_id| (stable_id.clone(), label.clone()));
                [Some((node.id.to_string(), label)), durable]
            })
            .flatten()
            .collect();
        let report =
            memory::reflect(root, fingerprint, &node_labels).map_err(|error| error.to_string())?;
        serde_json::to_value(report).map_err(|error| error.to_string())
    }

    fn resolve_target(&self, target: &str) -> Option<NodeId> {
        // The same ranking the queries use: an exact label can name many
        // nodes — `main` names ripgrep's program and its Python benchmark
        // script — and taking whichever the file walk reached first
        // answered `workflow main` with 107 blocks of benchsuite.
        resolve_node_reference(self.graph, target.trim())
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("missing required string argument `{key}`"))
}

// Out-of-range or non-integer JSON values saturate to the nearest bound
// instead of silently truncating (`as` wraps modulo) or silently falling back
// to the default; downstream clamps then bring them into the documented range.
fn usize_arg(args: &Value, key: &str, default: usize) -> usize {
    args.get(key)
        .and_then(Value::as_u64)
        .map(|value| usize::try_from(value).unwrap_or(usize::MAX))
        .unwrap_or(default)
}

fn u32_arg(args: &Value, key: &str, default: u32) -> u32 {
    args.get(key)
        .and_then(Value::as_u64)
        .map(|value| u32::try_from(value).unwrap_or(u32::MAX))
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
                    "node_id": {"type": ["integer", "string"], "description": "A label such as `main`, or a graph node id: numeric, n-prefixed, or the durable cg-* id."},
                    "edge_limit": {"type": "integer", "description": "Maximum neighbor edges (default 24)."},
                    "source_context": {"type": "integer", "description": "Source preview context lines (default 5)."},
                    "insight_limit": {"type": "integer", "description": "Maximum related risks (default 8)."},
                    "include_insights": {"type": "boolean", "description": "Run repository-wide insight analysis (default false)."}
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
                    "compact": {"type": "boolean", "description": "Collapse repeated low-signal blocks."},
                    "max_fanout": {"type": "integer", "description": "Cap outgoing edges expanded per node (calls first) so the flow follows the call chain into depth instead of one wide node. Try 8."}
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
                    "limit": {"type": "integer", "description": "Maximum listed dependents (default 40)."},
                    "include_risks": {"type": "boolean", "description": "Run repository-wide risk analysis (default false; refactor_context includes risks)."}
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "refactor_context",
            "description": "Return a one-shot refactor context bundle for a node: blast radius, grouped dependencies, optional journey from an entrypoint, related risks, and source spans.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "Refactor target label or node id."},
                    "from": {"type": "string", "description": "Optional journey start label or node id (an entrypoint)."},
                    "depth": {"type": "integer", "description": "Maximum traversal depth (default 6)."},
                    "paths": {"type": "integer", "description": "Maximum ranked journey paths (default 3)."},
                    "dependent_limit": {"type": "integer", "description": "Maximum listed dependents (default 50)."},
                    "risk_limit": {"type": "integer", "description": "Maximum related risks (default 50)."}
                },
                "required": ["target"]
            }
        }),
        json!({
            "name": "ask",
            "description": "Map an English or Russian investigation question to a bounded deterministic graph query and run it, returning the generated query, matched rule, confidence, alternatives, and the result slice.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": {"type": "string", "description": "Question, e.g. `Where is DATABASE_URL read?`"},
                    "compact": {"type": "boolean", "description": "Collapse repeated low-signal nodes."}
                },
                "required": ["question"]
            }
        }),
        json!({
            "name": "source_search",
            "description": "Search source text across the project and return compact matching snippets with line/column positions and context lines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Text to search for."},
                    "path": {"type": "string", "description": "Restrict matches to paths containing this substring."},
                    "case_sensitive": {"type": "boolean", "description": "Match case exactly (default false)."},
                    "limit": {"type": "integer", "description": "Maximum matches (default 50)."},
                    "context": {"type": "integer", "description": "Context lines around each match (default 2)."}
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "memory_save",
            "description": "Save an investigation outcome to repository memory (.codegraph/memory.jsonl) stamped with the current project fingerprint so it goes stale safely when sources change.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "The query, question, or journey that was investigated."},
                    "outcome": {"type": "string", "enum": ["useful", "dead_end", "corrected"], "description": "Investigation outcome."},
                    "note": {"type": "string", "description": "Free-text lesson or correction."},
                    "node_ids": {"type": "array", "items": {"type": ["string", "integer"]}, "description": "Linked graph node ids: the durable cg-* id the scan stamps, or a numeric/n-prefixed id."}
                },
                "required": ["query", "outcome"]
            }
        }),
        json!({
            "name": "memory_list",
            "description": "List saved investigation memory with staleness marks against the current project fingerprint.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "outcome": {"type": "string", "enum": ["useful", "dead_end", "corrected"], "description": "Filter records by outcome."},
                    "only_stale": {"type": "boolean", "description": "Return only records whose fingerprint no longer matches."}
                }
            }
        }),
        json!({
            "name": "memory_reflect",
            "description": "Aggregate saved investigation memory into repository lessons: dead ends, corrections, per-node lessons with provenance, and stale-source warnings.",
            "inputSchema": {"type": "object", "properties": {}}
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
            fingerprint: None,
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
            fingerprint: None,
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
            14
        );
    }

    fn call(engine: &McpEngine, name: &str, arguments: Value) -> Value {
        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        });
        let (response, _) = engine.handle_message(&message);
        let response = response.expect("tool response");
        assert_eq!(
            response["result"]["isError"], false,
            "tool {name} failed: {response}"
        );
        serde_json::from_str(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("text payload"),
        )
        .expect("payload json")
    }

    #[test]
    fn ask_refactor_context_and_source_search_tools_return_payloads() {
        let dir = std::env::temp_dir().join(format!(
            "codegraph-mcp-tools-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("main.rs"), "fn main() {\n    helper();\n}\n").expect("source");
        let graph = test_graph();
        let engine = McpEngine {
            graph: &graph,
            root: Some(&dir),
            fingerprint: Some("fp-current"),
        };

        let ask = call(&engine, "ask", json!({"question": "Who calls helper?"}));
        assert!(ask.get("generated_query").is_some());
        assert!(ask.get("rule").is_some());

        let bundle = call(&engine, "refactor_context", json!({"target": "helper"}));
        assert!(bundle.get("impact").is_some());

        // Node ids are accepted in both integer and n-prefixed string form.
        let helper_id = graph
            .nodes
            .iter()
            .find(|node| node.label == "helper")
            .expect("helper node")
            .id;
        let by_string = call(
            &engine,
            "get_node_card",
            json!({"node_id": format!("n{}", helper_id.0)}),
        );
        assert_eq!(by_string["context"]["node"]["id"], helper_id.0);

        let search = call(&engine, "source_search", json!({"query": "helper"}));
        assert_eq!(search["total_matches"], 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn memory_tools_roundtrip_over_mcp() {
        let dir = std::env::temp_dir().join(format!(
            "codegraph-mcp-memory-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let graph = test_graph();
        let engine = McpEngine {
            graph: &graph,
            root: Some(&dir),
            fingerprint: Some("fp-current"),
        };

        let saved = call(
            &engine,
            "memory_save",
            json!({
                "query": "calls(function:helper)",
                "outcome": "useful",
                "note": "helper is called from main only",
                "node_ids": [2],
            }),
        );
        assert_eq!(saved["id"], "mem-1");
        assert_eq!(saved["fingerprint"], "fp-current");

        let listed = call(&engine, "memory_list", json!({}));
        assert_eq!(listed["total"], 1);
        assert_eq!(listed["stale"], 0);

        let reflection = call(&engine, "memory_reflect", json!({}));
        assert_eq!(reflection["total_records"], 1);
        assert_eq!(reflection["outcome_counts"]["useful"], 1);

        // Without a fingerprint the memory tools fail with a clear message.
        let no_fingerprint = McpEngine {
            graph: &graph,
            root: Some(&dir),
            fingerprint: None,
        };
        let message = json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {"name": "memory_list", "arguments": {}},
        });
        let (response, _) = no_fingerprint.handle_message(&message);
        assert_eq!(response.expect("error response")["result"]["isError"], true);

        std::fs::remove_dir_all(&dir).ok();
    }
}
