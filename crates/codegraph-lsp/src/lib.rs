use codegraph_core::{CodeGraph, Confidence, Edge, EdgeKind, Node, NodeId, NodeKind, SourceSpan};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};

pub const DEFAULT_SEMANTIC_WORK_ITEM_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspDiscoveryReport {
    pub servers: Vec<LspServerStatus>,
    pub total_servers: usize,
    pub available_servers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LspServerStatus {
    pub id: &'static str,
    pub languages: &'static [&'static str],
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub capabilities: &'static [&'static str],
    pub installed: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticReadinessReport {
    pub languages: Vec<LanguageReadiness>,
    pub total_languages: usize,
    pub covered_languages: usize,
    pub missing_languages: usize,
    pub semantic_candidate_nodes: usize,
    pub required_servers: Vec<&'static str>,
    pub missing_servers: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageReadiness {
    pub language: String,
    pub nodes: usize,
    pub server: Option<&'static str>,
    pub installed: bool,
    pub capabilities: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticEnrichmentPlan {
    pub languages: Vec<LanguageEnrichmentPlan>,
    pub total_languages: usize,
    pub ready_languages: usize,
    pub blocked_languages: usize,
    pub unsupported_languages: usize,
    pub semantic_candidate_nodes: usize,
    pub heuristic_edges_to_upgrade: usize,
    pub planned_requests: SemanticRequestCounts,
    pub total_work_items: usize,
    pub work_item_limit: usize,
    pub truncated_work_items: bool,
    pub work_items: Vec<SemanticWorkItem>,
    pub missing_servers: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LanguageEnrichmentPlan {
    pub language: String,
    pub status: &'static str,
    pub nodes: usize,
    pub files: usize,
    pub symbol_nodes: usize,
    pub heuristic_edges_to_upgrade: usize,
    pub server: Option<&'static str>,
    pub command: Option<&'static str>,
    pub installed: bool,
    pub capabilities: Vec<&'static str>,
    pub planned_requests: SemanticRequestCounts,
    pub blocked_reason: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SemanticRequestCounts {
    pub document_symbols: usize,
    pub definitions: usize,
    pub references: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticWorkItem {
    pub id: String,
    pub kind: &'static str,
    pub capability: &'static str,
    pub priority: usize,
    pub reason: &'static str,
    pub language: String,
    pub status: &'static str,
    pub server: Option<&'static str>,
    pub blocked_reason: Option<&'static str>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub node: Option<SemanticNodeRef>,
    pub target: Option<SemanticNodeRef>,
    pub edge_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticNodeRef {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LspServerSpec {
    id: &'static str,
    languages: &'static [&'static str],
    command: &'static str,
    args: &'static [&'static str],
    capabilities: &'static [&'static str],
}

const LSP_SERVER_SPECS: &[LspServerSpec] = &[
    LspServerSpec {
        id: "rust-analyzer",
        languages: &["rust"],
        command: "rust-analyzer",
        args: &[],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "gopls",
        languages: &["go"],
        command: "gopls",
        args: &[],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "typescript-language-server",
        languages: &["javascript", "typescript", "tsx"],
        command: "typescript-language-server",
        args: &["--stdio"],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "pyright-langserver",
        languages: &["python"],
        command: "pyright-langserver",
        args: &["--stdio"],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "clangd",
        languages: &["c", "cpp"],
        command: "clangd",
        args: &[],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "intelephense",
        languages: &["php"],
        command: "intelephense",
        args: &["--stdio"],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "bash-language-server",
        languages: &["bash"],
        command: "bash-language-server",
        args: &["start"],
        capabilities: &["document_symbols", "diagnostics"],
    },
];

pub fn discover_lsp_servers() -> LspDiscoveryReport {
    let servers: Vec<_> = LSP_SERVER_SPECS
        .iter()
        .map(|spec| {
            let path = find_executable(spec.command);
            LspServerStatus {
                id: spec.id,
                languages: spec.languages,
                command: spec.command,
                args: spec.args,
                capabilities: spec.capabilities,
                installed: path.is_some(),
                path: path.map(|path| path.display().to_string()),
            }
        })
        .collect();
    let available_servers = servers.iter().filter(|server| server.installed).count();

    LspDiscoveryReport {
        total_servers: servers.len(),
        available_servers,
        servers,
    }
}

pub fn semantic_readiness(languages: &BTreeMap<String, usize>) -> SemanticReadinessReport {
    let discovery = discover_lsp_servers();
    semantic_readiness_with_discovery(languages, &discovery)
}

pub fn semantic_enrichment_plan(graph: &CodeGraph) -> SemanticEnrichmentPlan {
    semantic_enrichment_plan_with_limit(graph, DEFAULT_SEMANTIC_WORK_ITEM_LIMIT)
}

pub fn semantic_enrichment_plan_with_limit(
    graph: &CodeGraph,
    work_item_limit: usize,
) -> SemanticEnrichmentPlan {
    let discovery = discover_lsp_servers();
    semantic_enrichment_plan_with_discovery_and_limit(graph, &discovery, work_item_limit)
}

pub fn semantic_readiness_with_discovery(
    languages: &BTreeMap<String, usize>,
    discovery: &LspDiscoveryReport,
) -> SemanticReadinessReport {
    let mut readiness = Vec::new();
    let mut required_servers = BTreeSet::new();
    let mut missing_servers = BTreeSet::new();
    let mut semantic_candidate_nodes = 0;

    for (language, nodes) in languages {
        let server = discovery
            .servers
            .iter()
            .find(|server| server.languages.contains(&language.as_str()));
        let installed = server.is_some_and(|server| server.installed);
        if let Some(server) = server {
            required_servers.insert(server.id);
            if !server.installed {
                missing_servers.insert(server.id);
            }
        }
        if server.is_some() {
            semantic_candidate_nodes += *nodes;
        }
        readiness.push(LanguageReadiness {
            language: language.clone(),
            nodes: *nodes,
            server: server.map(|server| server.id),
            installed,
            capabilities: server
                .map(|server| server.capabilities)
                .unwrap_or(&[] as &[&str]),
        });
    }

    readiness.sort_by(|left, right| {
        right
            .nodes
            .cmp(&left.nodes)
            .then_with(|| left.language.cmp(&right.language))
    });
    let total_languages = readiness.len();
    let covered_languages = readiness.iter().filter(|item| item.installed).count();

    SemanticReadinessReport {
        languages: readiness,
        total_languages,
        covered_languages,
        missing_languages: total_languages.saturating_sub(covered_languages),
        semantic_candidate_nodes,
        required_servers: required_servers.into_iter().collect(),
        missing_servers: missing_servers.into_iter().collect(),
    }
}

pub fn semantic_enrichment_plan_with_discovery(
    graph: &CodeGraph,
    discovery: &LspDiscoveryReport,
) -> SemanticEnrichmentPlan {
    semantic_enrichment_plan_with_discovery_and_limit(
        graph,
        discovery,
        DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
    )
}

pub fn semantic_enrichment_plan_with_discovery_and_limit(
    graph: &CodeGraph,
    discovery: &LspDiscoveryReport,
    work_item_limit: usize,
) -> SemanticEnrichmentPlan {
    let mut languages: BTreeMap<String, LanguagePlanAccumulator> = BTreeMap::new();

    for node in &graph.nodes {
        let Some(language) = node_language(node.metadata.get("language").map(String::as_str))
        else {
            continue;
        };
        let plan = languages.entry(language.to_string()).or_default();
        plan.nodes += 1;
        if node.kind == NodeKind::File {
            plan.files += 1;
        }
        if is_symbol_candidate(&node.kind) && node.span.is_some() {
            plan.symbol_nodes += 1;
        }
    }

    let nodes_by_id: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    for edge in &graph.edges {
        if !matches!(edge.confidence, Confidence::Heuristic | Confidence::Unknown)
            || !matches!(
                edge.kind,
                EdgeKind::Calls | EdgeKind::Imports | EdgeKind::References
            )
        {
            continue;
        }
        if let Some(source) = nodes_by_id.get(&edge.source) {
            let Some(language) = node_language(source.metadata.get("language").map(String::as_str))
            else {
                continue;
            };
            languages
                .entry(language.to_string())
                .or_default()
                .heuristic_edges_to_upgrade += 1;
        }
    }

    let mut missing_servers = BTreeSet::new();
    let mut plans = Vec::new();
    let mut totals = SemanticRequestCounts::default();
    let mut semantic_candidate_nodes = 0;
    let mut heuristic_edges_to_upgrade = 0;

    for (language, counts) in languages {
        let server = discovery
            .servers
            .iter()
            .find(|server| server.languages.contains(&language.as_str()));
        let installed = server.is_some_and(|server| server.installed);
        let mut requests = SemanticRequestCounts::default();
        let capabilities = server
            .map(|server| server.capabilities.to_vec())
            .unwrap_or_default();
        if let Some(server) = server {
            semantic_candidate_nodes += counts.nodes;
            heuristic_edges_to_upgrade += counts.heuristic_edges_to_upgrade;
            if !server.installed {
                missing_servers.insert(server.id);
            }
            if server.installed {
                if has_capability(server, "document_symbols") {
                    requests.document_symbols = counts.files;
                }
                if has_capability(server, "definitions") {
                    requests.definitions = counts.heuristic_edges_to_upgrade;
                }
                if has_capability(server, "references") {
                    requests.references = counts.symbol_nodes;
                }
                if has_capability(server, "diagnostics") {
                    requests.diagnostics = counts.files;
                }
            }
        }

        totals.add(requests);
        let status = if server.is_none() {
            "unsupported_language"
        } else if !installed {
            "missing_server"
        } else {
            "ready"
        };
        let blocked_reason = match status {
            "unsupported_language" => Some("no_known_language_server"),
            "missing_server" => Some("language_server_not_installed"),
            _ => None,
        };

        plans.push(LanguageEnrichmentPlan {
            language,
            status,
            nodes: counts.nodes,
            files: counts.files,
            symbol_nodes: counts.symbol_nodes,
            heuristic_edges_to_upgrade: counts.heuristic_edges_to_upgrade,
            server: server.map(|server| server.id),
            command: server.map(|server| server.command),
            installed,
            capabilities,
            planned_requests: requests,
            blocked_reason,
        });
    }

    plans.sort_by(|left, right| {
        status_rank(left.status)
            .cmp(&status_rank(right.status))
            .then_with(|| right.nodes.cmp(&left.nodes))
            .then_with(|| left.language.cmp(&right.language))
    });
    let total_languages = plans.len();
    let ready_languages = plans.iter().filter(|plan| plan.status == "ready").count();
    let blocked_languages = plans
        .iter()
        .filter(|plan| plan.status == "missing_server")
        .count();
    let unsupported_languages = plans
        .iter()
        .filter(|plan| plan.status == "unsupported_language")
        .count();
    let (work_items, total_work_items) = semantic_work_items(graph, &plans, work_item_limit);

    SemanticEnrichmentPlan {
        languages: plans,
        total_languages,
        ready_languages,
        blocked_languages,
        unsupported_languages,
        semantic_candidate_nodes,
        heuristic_edges_to_upgrade,
        planned_requests: totals,
        total_work_items,
        work_item_limit,
        truncated_work_items: total_work_items > work_items.len(),
        work_items,
        missing_servers: missing_servers.into_iter().collect(),
    }
}

pub fn server_specs() -> &'static [&'static str] {
    const IDS: &[&str] = &[
        "rust-analyzer",
        "gopls",
        "typescript-language-server",
        "pyright-langserver",
        "clangd",
        "intelephense",
        "bash-language-server",
    ];
    IDS
}

#[derive(Debug, Default)]
struct LanguagePlanAccumulator {
    nodes: usize,
    files: usize,
    symbol_nodes: usize,
    heuristic_edges_to_upgrade: usize,
}

impl SemanticRequestCounts {
    fn add(&mut self, other: Self) {
        self.document_symbols += other.document_symbols;
        self.definitions += other.definitions;
        self.references += other.references;
        self.diagnostics += other.diagnostics;
    }
}

fn semantic_work_items(
    graph: &CodeGraph,
    plans: &[LanguageEnrichmentPlan],
    work_item_limit: usize,
) -> (Vec<SemanticWorkItem>, usize) {
    let mut work_items = Vec::new();
    let mut total_work_items = 0;
    let nodes_by_id: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();

    for plan in plans {
        if plan.status == "unsupported_language" {
            push_semantic_work_item(
                &mut work_items,
                &mut total_work_items,
                work_item_limit,
                SemanticWorkItem {
                    id: format!("language_support:{}", plan.language),
                    kind: "language_support",
                    capability: "language_server",
                    priority: work_item_priority("language_server"),
                    reason: "no language server is configured for this source language",
                    language: plan.language.clone(),
                    status: plan.status,
                    server: None,
                    blocked_reason: plan.blocked_reason,
                    path: None,
                    line: None,
                    column: None,
                    node: None,
                    target: None,
                    edge_index: None,
                },
            );
            continue;
        }

        if plan.capabilities.contains(&"definitions") {
            for (edge_index, edge) in language_heuristic_edges(graph, &nodes_by_id, &plan.language)
            {
                let source = nodes_by_id.get(&edge.source).copied();
                let target = nodes_by_id.get(&edge.target).copied();
                push_semantic_work_item(
                    &mut work_items,
                    &mut total_work_items,
                    work_item_limit,
                    edge_work_item("definitions", plan, edge_index, source, target),
                );
            }
        }
        if plan.capabilities.contains(&"diagnostics") {
            for node in language_file_nodes(graph, &plan.language) {
                push_semantic_work_item(
                    &mut work_items,
                    &mut total_work_items,
                    work_item_limit,
                    file_work_item("diagnostics", plan, node),
                );
            }
        }
        if plan.capabilities.contains(&"document_symbols") {
            for node in language_file_nodes(graph, &plan.language) {
                push_semantic_work_item(
                    &mut work_items,
                    &mut total_work_items,
                    work_item_limit,
                    file_work_item("document_symbols", plan, node),
                );
            }
        }
        if plan.capabilities.contains(&"references") {
            for node in language_symbol_nodes(graph, &plan.language) {
                push_semantic_work_item(
                    &mut work_items,
                    &mut total_work_items,
                    work_item_limit,
                    node_work_item("references", plan, node),
                );
            }
        }
    }

    (work_items, total_work_items)
}

fn language_file_nodes<'a>(graph: &'a CodeGraph, language: &str) -> impl Iterator<Item = &'a Node> {
    graph.nodes.iter().filter(move |node| {
        node.kind == NodeKind::File
            && node_language(node.metadata.get("language").map(String::as_str)) == Some(language)
    })
}

fn language_symbol_nodes<'a>(
    graph: &'a CodeGraph,
    language: &str,
) -> impl Iterator<Item = &'a Node> {
    graph.nodes.iter().filter(move |node| {
        is_symbol_candidate(&node.kind)
            && node.span.is_some()
            && node_language(node.metadata.get("language").map(String::as_str)) == Some(language)
    })
}

fn language_heuristic_edges<'a>(
    graph: &'a CodeGraph,
    nodes_by_id: &'a BTreeMap<NodeId, &'a Node>,
    language: &str,
) -> impl Iterator<Item = (usize, &'a Edge)> {
    graph.edges.iter().enumerate().filter(move |(_, edge)| {
        matches!(edge.confidence, Confidence::Heuristic | Confidence::Unknown)
            && matches!(
                edge.kind,
                EdgeKind::Calls | EdgeKind::Imports | EdgeKind::References
            )
            && nodes_by_id
                .get(&edge.source)
                .and_then(|node| node_language(node.metadata.get("language").map(String::as_str)))
                == Some(language)
    })
}

fn push_semantic_work_item(
    work_items: &mut Vec<SemanticWorkItem>,
    total_work_items: &mut usize,
    work_item_limit: usize,
    item: SemanticWorkItem,
) {
    *total_work_items += 1;
    if work_items.len() < work_item_limit {
        work_items.push(item);
    }
}

fn file_work_item(
    capability: &'static str,
    plan: &LanguageEnrichmentPlan,
    node: &Node,
) -> SemanticWorkItem {
    let (path, line, column) = node_location(node);
    let id = work_item_id(
        capability,
        &plan.language,
        path.as_deref(),
        Some(node),
        None,
    );
    SemanticWorkItem {
        id,
        kind: "file",
        capability,
        priority: work_item_priority(capability),
        reason: work_item_reason(capability),
        language: plan.language.clone(),
        status: plan.status,
        server: plan.server,
        blocked_reason: plan.blocked_reason,
        path,
        line,
        column,
        node: Some(node_ref(node)),
        target: None,
        edge_index: None,
    }
}

fn node_work_item(
    capability: &'static str,
    plan: &LanguageEnrichmentPlan,
    node: &Node,
) -> SemanticWorkItem {
    let (path, line, column) = node_location(node);
    let id = work_item_id(
        capability,
        &plan.language,
        path.as_deref(),
        Some(node),
        None,
    );
    SemanticWorkItem {
        id,
        kind: "symbol",
        capability,
        priority: work_item_priority(capability),
        reason: work_item_reason(capability),
        language: plan.language.clone(),
        status: plan.status,
        server: plan.server,
        blocked_reason: plan.blocked_reason,
        path,
        line,
        column,
        node: Some(node_ref(node)),
        target: None,
        edge_index: None,
    }
}

fn edge_work_item(
    capability: &'static str,
    plan: &LanguageEnrichmentPlan,
    edge_index: usize,
    source: Option<&Node>,
    target: Option<&Node>,
) -> SemanticWorkItem {
    let (path, line, column) = source.map(node_location).unwrap_or((None, None, None));
    let id = work_item_id(
        capability,
        &plan.language,
        path.as_deref(),
        source,
        Some(edge_index),
    );
    SemanticWorkItem {
        id,
        kind: "edge",
        capability,
        priority: work_item_priority(capability),
        reason: work_item_reason(capability),
        language: plan.language.clone(),
        status: plan.status,
        server: plan.server,
        blocked_reason: plan.blocked_reason,
        path,
        line,
        column,
        node: source.map(node_ref),
        target: target.map(node_ref),
        edge_index: Some(edge_index),
    }
}

fn work_item_id(
    capability: &str,
    language: &str,
    path: Option<&str>,
    node: Option<&Node>,
    edge_index: Option<usize>,
) -> String {
    if let Some(edge_index) = edge_index {
        return format!("{capability}:{language}:edge:{edge_index}");
    }
    if let Some(node) = node {
        return format!("{capability}:{language}:node:{}", node.id.0);
    }
    if let Some(path) = path {
        return format!("{capability}:{language}:path:{path}");
    }
    format!("{capability}:{language}")
}

fn work_item_priority(capability: &str) -> usize {
    match capability {
        "definitions" => 10,
        "diagnostics" => 20,
        "document_symbols" => 30,
        "references" => 40,
        "language_server" => 90,
        _ => 100,
    }
}

fn work_item_reason(capability: &str) -> &'static str {
    match capability {
        "definitions" => "upgrade heuristic graph edges to semantic definitions",
        "diagnostics" => "collect language-server diagnostics for source files",
        "document_symbols" => "compare parser symbols with language-server document symbols",
        "references" => "find semantic references for known graph symbols",
        "language_server" => "enable language-server support before semantic enrichment",
        _ => "semantic enrichment work item",
    }
}

fn node_ref(node: &Node) -> SemanticNodeRef {
    let (path, line, column) = node_location(node);
    SemanticNodeRef {
        id: node.id,
        kind: node.kind.clone(),
        label: node.label.clone(),
        path,
        line,
        column,
    }
}

fn node_location(node: &Node) -> (Option<String>, Option<u32>, Option<u32>) {
    if let Some(span) = &node.span {
        return span_location(span);
    }
    if node.kind == NodeKind::File {
        return (Some(node.label.clone()), None, None);
    }
    (None, None, None)
}

fn span_location(span: &SourceSpan) -> (Option<String>, Option<u32>, Option<u32>) {
    (
        Some(span.path.clone()),
        Some(span.start_line),
        Some(span.start_column),
    )
}

fn node_language(language: Option<&str>) -> Option<&str> {
    language.filter(|language| !language.is_empty())
}

fn is_symbol_candidate(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function | NodeKind::Type | NodeKind::Module | NodeKind::Entrypoint
    )
}

fn has_capability(server: &LspServerStatus, capability: &str) -> bool {
    server.capabilities.contains(&capability)
}

fn status_rank(status: &str) -> usize {
    match status {
        "ready" => 0,
        "missing_server" => 1,
        "unsupported_language" => 2,
        _ => 3,
    }
}

fn find_executable(command: &str) -> Option<PathBuf> {
    find_executable_with_path(command, env::var_os("PATH").as_deref())
}

fn find_executable_with_path(command: &str, path_var: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let path_var = path_var?;
    for dir in env::split_paths(path_var) {
        for candidate in executable_candidates(&dir, command) {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_candidates(dir: &Path, command: &str) -> Vec<PathBuf> {
    if cfg!(windows) {
        let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        pathext
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| dir.join(format!("{command}{extension}")))
            .chain(std::iter::once(dir.join(command)))
            .collect()
    } else {
        vec![dir.join(command)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeKind, SourceSpan};
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn discovery_report_lists_target_language_servers() {
        let report = discover_lsp_servers();
        let ids: Vec<_> = report.servers.iter().map(|server| server.id).collect();

        assert_eq!(report.total_servers, 7);
        assert!(ids.contains(&"rust-analyzer"));
        assert!(ids.contains(&"gopls"));
        assert!(ids.contains(&"typescript-language-server"));
        assert!(ids.contains(&"pyright-langserver"));
        assert!(ids.contains(&"clangd"));
        assert!(ids.contains(&"intelephense"));
        assert!(ids.contains(&"bash-language-server"));
        assert!(report.available_servers <= report.total_servers);
    }

    #[test]
    fn executable_lookup_uses_path_entries() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("rust-analyzer"), "").unwrap();
        let path_var = env::join_paths([dir.as_path()]).unwrap();

        let found = find_executable_with_path("rust-analyzer", Some(path_var.as_os_str()));

        assert_eq!(found, Some(dir.join("rust-analyzer")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn semantic_readiness_maps_project_languages_to_servers() {
        let languages = BTreeMap::from([
            ("rust".to_string(), 4),
            ("python".to_string(), 2),
            ("unknown".to_string(), 1),
        ]);
        let discovery = LspDiscoveryReport {
            total_servers: 2,
            available_servers: 1,
            servers: vec![
                LspServerStatus {
                    id: "rust-analyzer",
                    languages: &["rust"],
                    command: "rust-analyzer",
                    args: &[],
                    capabilities: &["definitions"],
                    installed: true,
                    path: Some("/bin/rust-analyzer".to_string()),
                },
                LspServerStatus {
                    id: "pyright-langserver",
                    languages: &["python"],
                    command: "pyright-langserver",
                    args: &["--stdio"],
                    capabilities: &["definitions"],
                    installed: false,
                    path: None,
                },
            ],
        };

        let report = semantic_readiness_with_discovery(&languages, &discovery);

        assert_eq!(report.total_languages, 3);
        assert_eq!(report.covered_languages, 1);
        assert_eq!(report.missing_languages, 2);
        assert_eq!(report.semantic_candidate_nodes, 6);
        assert_eq!(
            report.required_servers,
            vec!["pyright-langserver", "rust-analyzer"]
        );
        assert_eq!(report.missing_servers, vec!["pyright-langserver"]);
        assert!(
            report
                .languages
                .iter()
                .any(|language| language.language == "unknown" && language.server.is_none())
        );
    }

    #[test]
    fn semantic_enrichment_plan_counts_ready_and_blocked_work() {
        let mut graph = CodeGraph::new("repo");
        let rust_file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let rust_main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            Some(SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 12,
            }),
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let rust_helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper",
            Some(SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 2,
                start_column: 1,
                end_line: 2,
                end_column: 14,
            }),
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let python_file = graph.add_node_with_metadata(
            NodeKind::File,
            "app.py",
            None,
            BTreeMap::from([("language".to_string(), "python".to_string())]),
        );
        let unknown_file = graph.add_node_with_metadata(
            NodeKind::File,
            "README.md",
            None,
            BTreeMap::from([("language".to_string(), "markdown".to_string())]),
        );
        graph.add_edge(
            rust_file,
            rust_main,
            EdgeKind::Defines,
            Confidence::Syntactic,
        );
        graph.add_edge(
            rust_main,
            rust_helper,
            EdgeKind::Calls,
            Confidence::Heuristic,
        );
        graph.add_edge(
            python_file,
            unknown_file,
            EdgeKind::Imports,
            Confidence::Unknown,
        );

        let discovery = LspDiscoveryReport {
            total_servers: 2,
            available_servers: 1,
            servers: vec![
                LspServerStatus {
                    id: "rust-analyzer",
                    languages: &["rust"],
                    command: "rust-analyzer",
                    args: &[],
                    capabilities: &["definitions", "references", "document_symbols"],
                    installed: true,
                    path: Some("/bin/rust-analyzer".to_string()),
                },
                LspServerStatus {
                    id: "pyright-langserver",
                    languages: &["python"],
                    command: "pyright-langserver",
                    args: &["--stdio"],
                    capabilities: &["definitions", "diagnostics"],
                    installed: false,
                    path: None,
                },
            ],
        };

        let plan = semantic_enrichment_plan_with_discovery(&graph, &discovery);
        let rust = plan
            .languages
            .iter()
            .find(|language| language.language == "rust")
            .expect("rust plan");
        let python = plan
            .languages
            .iter()
            .find(|language| language.language == "python")
            .expect("python plan");
        let markdown = plan
            .languages
            .iter()
            .find(|language| language.language == "markdown")
            .expect("markdown plan");

        assert_eq!(plan.total_languages, 3);
        assert_eq!(plan.ready_languages, 1);
        assert_eq!(plan.blocked_languages, 1);
        assert_eq!(plan.unsupported_languages, 1);
        assert_eq!(plan.semantic_candidate_nodes, 4);
        assert_eq!(plan.heuristic_edges_to_upgrade, 2);
        assert_eq!(plan.missing_servers, vec!["pyright-langserver"]);
        assert_eq!(plan.planned_requests.document_symbols, 1);
        assert_eq!(plan.planned_requests.definitions, 1);
        assert_eq!(plan.planned_requests.references, 2);
        assert_eq!(plan.planned_requests.diagnostics, 0);
        assert_eq!(plan.total_work_items, 7);
        assert_eq!(plan.work_item_limit, DEFAULT_SEMANTIC_WORK_ITEM_LIMIT);
        assert!(!plan.truncated_work_items);
        let definition_item = plan
            .work_items
            .iter()
            .find(|item| {
                item.kind == "edge"
                    && item.capability == "definitions"
                    && item.language == "rust"
                    && item.status == "ready"
                    && item.edge_index == Some(1)
            })
            .expect("definition work item");
        assert_eq!(definition_item.id, "definitions:rust:edge:1");
        assert_eq!(definition_item.priority, 10);
        assert_eq!(
            definition_item.reason,
            "upgrade heuristic graph edges to semantic definitions"
        );
        assert_eq!(plan.work_items.first(), Some(definition_item));
        assert!(plan.work_items.iter().any(|item| {
            item.kind == "language_support"
                && item.language == "markdown"
                && item.status == "unsupported_language"
                && item.id == "language_support:markdown"
                && item.priority == 90
        }));

        assert_eq!(rust.status, "ready");
        assert_eq!(rust.files, 1);
        assert_eq!(rust.symbol_nodes, 2);
        assert_eq!(rust.heuristic_edges_to_upgrade, 1);
        assert_eq!(rust.planned_requests.definitions, 1);
        assert_eq!(python.status, "missing_server");
        assert_eq!(python.blocked_reason, Some("language_server_not_installed"));
        assert_eq!(python.planned_requests.definitions, 0);
        assert_eq!(markdown.status, "unsupported_language");
        assert_eq!(markdown.blocked_reason, Some("no_known_language_server"));
    }

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!(
            "codegraph-lsp-test-{}-{nanos}-{id}",
            std::process::id()
        ))
    }
}
