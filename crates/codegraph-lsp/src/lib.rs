//! Semantic enrichment through language servers: LSP discovery,
//! readiness and planning reports, capped work queues, executable
//! request batches over stdio, response-to-patch mapping, and patch
//! application with persistent semantic caches.

use codegraph_core::{CodeGraph, Confidence, Edge, EdgeKind, Node, NodeId, NodeKind, SourceSpan};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const DEFAULT_SEMANTIC_WORK_ITEM_LIMIT: usize = 100;
pub const MAX_SEMANTIC_WORK_ITEM_LIMIT: usize = 1_000;
pub const DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS: u64 = 30_000;
/// How long to let a language server finish loading before asking it
/// anything. rust-analyzer takes tens of seconds on a workspace of any size
/// and answers with nothing until it is done; the wait ends as soon as the
/// server goes quiet, so this is the ceiling rather than the cost.
pub const DEFAULT_SEMANTIC_SETTLE_TIMEOUT_MS: u64 = 120_000;
pub const MAX_SEMANTIC_REQUEST_TIMEOUT_MS: u64 = 300_000;
const SEMANTIC_CACHE_SCHEMA_VERSION: u32 = 1;
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

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
    pub work_item_filter: SemanticWorkItemFilter,
    pub truncated_work_items: bool,
    pub work_items: Vec<SemanticWorkItem>,
    pub missing_servers: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticExecutionBatch {
    pub workspace_root: String,
    pub work_item_limit: usize,
    pub work_item_filter: SemanticWorkItemFilter,
    pub total_work_items: usize,
    pub truncated_work_items: bool,
    pub server_batches: Vec<SemanticServerBatch>,
    pub blocked_items: Vec<SemanticWorkItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticServerBatch {
    pub server: String,
    pub command: String,
    pub args: Vec<String>,
    pub installed: bool,
    pub path: Option<String>,
    pub status: &'static str,
    pub languages: Vec<String>,
    pub work_items: Vec<SemanticWorkItem>,
    pub requests: Vec<SemanticLspRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticLspRequest {
    pub id: String,
    pub work_item_id: Option<String>,
    pub request_kind: &'static str,
    pub method: &'static str,
    pub params: Value,
    pub document_uri: Option<String>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub expected_result: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticLspResponse {
    pub request_id: String,
    pub method: String,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub error: Option<SemanticLspError>,
}

#[derive(Debug, Clone)]
pub struct SemanticLspCache {
    dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SemanticCachedRun {
    pub responses: Vec<SemanticLspResponse>,
    pub cache: SemanticLspCacheInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticLspCacheInfo {
    pub status: SemanticLspCacheStatus,
    pub dir: Option<String>,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticLspCacheStatus {
    Disabled,
    Hit,
    Miss,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SemanticLspCacheRecord {
    cache_schema_version: u32,
    batch_hash: String,
    responses: Vec<SemanticLspResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticLspError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticLspRunOptions {
    pub request_timeout: Duration,
    /// How long to let a server finish loading before asking it anything.
    /// Indexing a workspace takes far longer than answering about one
    /// position in it, so this is its own budget rather than the request's.
    pub settle_timeout: Duration,
}

impl Default for SemanticLspRunOptions {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_millis(DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS),
            settle_timeout: Duration::from_millis(DEFAULT_SEMANTIC_SETTLE_TIMEOUT_MS),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticLspRunError {
    message: String,
}

impl SemanticLspRunError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SemanticLspRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SemanticLspRunError {}

impl SemanticLspCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn load(
        &self,
        batch: &SemanticExecutionBatch,
    ) -> Result<Option<Vec<SemanticLspResponse>>, SemanticLspRunError> {
        let batch_hash = semantic_batch_hash(batch)?;
        let path = self.cache_path(&batch_hash);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(SemanticLspRunError::new(format!(
                    "failed to read semantic LSP cache `{}`: {error}",
                    path.display()
                )));
            }
        };
        // A malformed record (truncated write, tampering) is a cache miss, not
        // an error — the batch reruns and the entry is rewritten.
        let Ok(record) = serde_json::from_slice::<SemanticLspCacheRecord>(&bytes) else {
            return Ok(None);
        };
        if record.cache_schema_version != SEMANTIC_CACHE_SCHEMA_VERSION
            || record.batch_hash != batch_hash
        {
            return Ok(None);
        }
        Ok(Some(record.responses))
    }

    pub fn store(
        &self,
        batch: &SemanticExecutionBatch,
        responses: &[SemanticLspResponse],
    ) -> Result<String, SemanticLspRunError> {
        fs::create_dir_all(&self.dir).map_err(|error| {
            SemanticLspRunError::new(format!(
                "failed to create semantic LSP cache `{}`: {error}",
                self.dir.display()
            ))
        })?;
        let batch_hash = semantic_batch_hash(batch)?;
        let path = self.cache_path(&batch_hash);
        let temporary_path = path.with_extension(format!(
            "json.tmp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let record = SemanticLspCacheRecord {
            cache_schema_version: SEMANTIC_CACHE_SCHEMA_VERSION,
            batch_hash: batch_hash.clone(),
            responses: responses.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&record).map_err(|error| {
            SemanticLspRunError::new(format!("failed to encode semantic LSP cache: {error}"))
        })?;
        fs::write(&temporary_path, bytes).map_err(|error| {
            SemanticLspRunError::new(format!(
                "failed to write semantic LSP cache `{}`: {error}",
                temporary_path.display()
            ))
        })?;
        fs::rename(&temporary_path, &path).map_err(|error| {
            SemanticLspRunError::new(format!(
                "failed to publish semantic LSP cache `{}`: {error}",
                path.display()
            ))
        })?;
        Ok(batch_hash)
    }

    fn cache_path(&self, batch_hash: &str) -> PathBuf {
        self.dir.join(format!("semantic-lsp-{batch_hash}.json"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticGraphPatch {
    pub workspace_root: String,
    pub responses: usize,
    pub matched_responses: usize,
    pub semantic_edges: Vec<SemanticEdgePatch>,
    pub diagnostics: Vec<SemanticDiagnosticPatch>,
    pub unmatched_locations: Vec<SemanticUnmatchedLocation>,
    pub response_errors: Vec<SemanticResponseError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticEdgePatch {
    pub request_id: String,
    pub work_item_id: String,
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    pub replaced_edge_index: Option<usize>,
    pub original_target: Option<NodeId>,
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub evidence: &'static str,
    /// What the replaced edge said was called, and in which language. A
    /// semantic edge is the strongest one in the graph and carried neither,
    /// so every count grouped by resolution lost it and nothing could say
    /// what the call named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Where the call is written. Every other call edge records that under
    /// `file`/`line`, and a semantic edge recorded the answer's location
    /// instead -- for a definition in the standard library an absolute path
    /// on the machine that ran the pass, which no other span in the graph
    /// is and which cannot mean anything on another machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticDiagnosticPatch {
    pub request_id: String,
    pub work_item_id: String,
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub severity: Option<u64>,
    pub code: Option<String>,
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticUnmatchedLocation {
    pub request_id: String,
    pub work_item_id: Option<String>,
    pub method: String,
    pub uri: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticResponseError {
    pub request_id: String,
    pub method: String,
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticGraphApplyResult {
    pub graph: CodeGraph,
    pub report: SemanticGraphApplyReport,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SemanticGraphApplyReport {
    pub semantic_edges: usize,
    pub replaced_edges: usize,
    pub added_edges: usize,
    pub skipped_edges: usize,
    pub diagnostic_nodes: usize,
    pub diagnostic_edges: usize,
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
    pub workspace_symbols: usize,
    pub definitions: usize,
    pub references: usize,
    pub diagnostics: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SemanticWorkItemFilter {
    pub language: Option<String>,
    pub status: Option<String>,
    pub capability: Option<String>,
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
            "workspace_symbols",
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
            "workspace_symbols",
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
            "workspace_symbols",
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
            "workspace_symbols",
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
            "workspace_symbols",
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
            "workspace_symbols",
            "diagnostics",
        ],
    },
    LspServerSpec {
        id: "dart-analysis-server",
        languages: &["dart"],
        command: "dart",
        args: &["language-server", "--protocol=lsp"],
        capabilities: &[
            "definitions",
            "references",
            "document_symbols",
            "workspace_symbols",
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
            let path = find_lsp_server_executable(spec);
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

pub fn semantic_enrichment_plan_with_filter(
    graph: &CodeGraph,
    work_item_limit: usize,
    work_item_filter: SemanticWorkItemFilter,
) -> SemanticEnrichmentPlan {
    let discovery = discover_lsp_servers();
    semantic_enrichment_plan_with_discovery_and_filter(
        graph,
        &discovery,
        work_item_limit,
        work_item_filter,
    )
}

pub fn semantic_execution_batch(
    workspace_root: &Path,
    graph: &CodeGraph,
    work_item_limit: usize,
    work_item_filter: SemanticWorkItemFilter,
) -> SemanticExecutionBatch {
    let discovery = discover_lsp_servers();
    semantic_execution_batch_with_discovery(
        workspace_root,
        graph,
        &discovery,
        work_item_limit,
        work_item_filter,
    )
}

pub(crate) fn run_semantic_execution_batch(
    batch: &SemanticExecutionBatch,
    options: &SemanticLspRunOptions,
) -> Result<Vec<SemanticLspResponse>, SemanticLspRunError> {
    let workspace_root = Path::new(&batch.workspace_root);
    let mut responses = Vec::new();
    let mut ready_batches = 0;

    for server_batch in &batch.server_batches {
        if server_batch.status != "ready" || !server_batch.installed {
            continue;
        }
        ready_batches += 1;
        responses.extend(run_semantic_server_batch(
            workspace_root,
            server_batch,
            options,
        )?);
    }

    if ready_batches == 0 {
        return Err(SemanticLspRunError::new(
            "semantic run has no ready language-server batches",
        ));
    }

    Ok(responses)
}

pub fn run_semantic_execution_batch_cached(
    cache: Option<&SemanticLspCache>,
    batch: &SemanticExecutionBatch,
    options: &SemanticLspRunOptions,
) -> Result<SemanticCachedRun, SemanticLspRunError> {
    let Some(cache) = cache else {
        let responses = run_semantic_execution_batch(batch, options)?;
        return Ok(SemanticCachedRun {
            responses,
            cache: SemanticLspCacheInfo {
                status: SemanticLspCacheStatus::Disabled,
                dir: None,
                key: None,
            },
        });
    };

    let key = semantic_batch_hash(batch)?;
    if let Some(responses) = cache.load(batch)? {
        return Ok(SemanticCachedRun {
            responses,
            cache: SemanticLspCacheInfo {
                status: SemanticLspCacheStatus::Hit,
                dir: Some(cache.dir().display().to_string()),
                key: Some(key),
            },
        });
    }

    let responses = run_semantic_execution_batch(batch, options)?;
    let stored_key = cache.store(batch, &responses)?;
    Ok(SemanticCachedRun {
        responses,
        cache: SemanticLspCacheInfo {
            status: SemanticLspCacheStatus::Miss,
            dir: Some(cache.dir().display().to_string()),
            key: Some(stored_key),
        },
    })
}

pub(crate) fn semantic_readiness_with_discovery(
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

pub(crate) fn semantic_enrichment_plan_with_discovery_and_filter(
    graph: &CodeGraph,
    discovery: &LspDiscoveryReport,
    work_item_limit: usize,
    work_item_filter: SemanticWorkItemFilter,
) -> SemanticEnrichmentPlan {
    let work_item_limit = normalize_semantic_work_item_limit(work_item_limit);
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
                if has_capability(server, "workspace_symbols") {
                    requests.workspace_symbols = 1;
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
    let (work_items, total_work_items) =
        semantic_work_items(graph, &plans, work_item_limit, &work_item_filter);

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
        work_item_filter,
        truncated_work_items: total_work_items > work_items.len(),
        work_items,
        missing_servers: missing_servers.into_iter().collect(),
    }
}

pub fn normalize_semantic_work_item_limit(work_item_limit: usize) -> usize {
    work_item_limit.clamp(1, MAX_SEMANTIC_WORK_ITEM_LIMIT)
}

pub fn normalize_semantic_request_timeout_ms(request_timeout_ms: u64) -> u64 {
    request_timeout_ms.clamp(1, MAX_SEMANTIC_REQUEST_TIMEOUT_MS)
}

pub(crate) fn semantic_execution_batch_with_discovery(
    workspace_root: &Path,
    graph: &CodeGraph,
    discovery: &LspDiscoveryReport,
    work_item_limit: usize,
    work_item_filter: SemanticWorkItemFilter,
) -> SemanticExecutionBatch {
    let plan = semantic_enrichment_plan_with_discovery_and_filter(
        graph,
        discovery,
        work_item_limit,
        work_item_filter,
    );
    let mut grouped: BTreeMap<String, SemanticServerBatchBuilder> = BTreeMap::new();
    let mut blocked_items = Vec::new();

    for item in &plan.work_items {
        let Some(server_id) = item.server else {
            blocked_items.push(item.clone());
            continue;
        };
        let server = discovery
            .servers
            .iter()
            .find(|server| server.id == server_id);
        let entry = grouped
            .entry(server_id.to_string())
            .or_insert_with(|| SemanticServerBatchBuilder::new(server_id, server, workspace_root));
        entry.languages.insert(item.language.clone());
        entry.work_items.push(item.clone());
    }

    let server_batches = grouped
        .into_values()
        .map(SemanticServerBatchBuilder::finish)
        .collect();

    SemanticExecutionBatch {
        workspace_root: workspace_root.display().to_string(),
        work_item_limit: plan.work_item_limit,
        work_item_filter: plan.work_item_filter,
        total_work_items: plan.total_work_items,
        truncated_work_items: plan.truncated_work_items,
        server_batches,
        blocked_items,
    }
}

pub fn semantic_graph_patch_from_responses(
    workspace_root: &Path,
    graph: &CodeGraph,
    batch: &SemanticExecutionBatch,
    responses: &[SemanticLspResponse],
) -> SemanticGraphPatch {
    let mut request_by_id = BTreeMap::new();
    let mut work_item_by_id = BTreeMap::new();
    for server_batch in &batch.server_batches {
        for request in &server_batch.requests {
            request_by_id.insert(request.id.as_str(), request);
        }
        for item in &server_batch.work_items {
            work_item_by_id.insert(item.id.as_str(), item);
        }
    }

    let mut matched_responses = 0;
    let mut semantic_edges = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unmatched_locations = Vec::new();
    let mut response_errors = Vec::new();

    for response in responses {
        let Some(request) = request_by_id.get(response.request_id.as_str()).copied() else {
            unmatched_locations.push(SemanticUnmatchedLocation {
                request_id: response.request_id.clone(),
                work_item_id: None,
                method: response.method.clone(),
                uri: String::new(),
                path: None,
                line: None,
                column: None,
                reason: "unknown_request",
            });
            continue;
        };
        matched_responses += 1;

        if let Some(error) = &response.error {
            response_errors.push(SemanticResponseError {
                request_id: response.request_id.clone(),
                method: request.method.to_string(),
                code: error.code,
                message: error.message.clone(),
            });
            continue;
        }

        let Some(work_item_id) = request.work_item_id.as_deref() else {
            continue;
        };
        let Some(work_item) = work_item_by_id.get(work_item_id).copied() else {
            unmatched_locations.push(SemanticUnmatchedLocation {
                request_id: response.request_id.clone(),
                work_item_id: Some(work_item_id.to_string()),
                method: request.method.to_string(),
                uri: request.document_uri.clone().unwrap_or_default(),
                path: request.path.clone(),
                line: request.line,
                column: request.column,
                reason: "unknown_work_item",
            });
            continue;
        };

        match request.method {
            "textDocument/definition" => collect_definition_edges(
                workspace_root,
                graph,
                request,
                work_item,
                &response.result,
                &mut semantic_edges,
                &mut unmatched_locations,
            ),
            "textDocument/references" => collect_reference_edges(
                workspace_root,
                graph,
                request,
                work_item,
                &response.result,
                &mut semantic_edges,
                &mut unmatched_locations,
            ),
            "textDocument/diagnostic" => collect_diagnostics(
                workspace_root,
                request,
                work_item,
                &response.result,
                &mut diagnostics,
                &mut unmatched_locations,
            ),
            _ => {}
        }
    }

    SemanticGraphPatch {
        workspace_root: workspace_root.display().to_string(),
        responses: responses.len(),
        matched_responses,
        semantic_edges,
        diagnostics,
        unmatched_locations,
        response_errors,
    }
}

pub fn apply_semantic_graph_patch(
    graph: &CodeGraph,
    patch: &SemanticGraphPatch,
) -> SemanticGraphApplyResult {
    let mut graph = graph.clone();
    let mut report = SemanticGraphApplyReport::default();
    let mut replaced_indexes = BTreeSet::new();

    for edge_patch in &patch.semantic_edges {
        report.semantic_edges += 1;
        let edge = semantic_edge_from_patch(edge_patch);
        // A patch can be serialized and applied later, possibly to a graph
        // that has changed shape; only replace by index when the edge at that
        // index is provably the one the patch was built from — otherwise fall
        // through to the add path instead of overwriting a stranger.
        if let Some(index) = edge_patch.replaced_edge_index
            && graph.edges.get(index).is_some_and(|existing| {
                existing.source == edge_patch.source
                    && edge_patch
                        .original_target
                        .is_none_or(|original| existing.target == original)
            })
            && replaced_indexes.insert(index)
        {
            graph.edges[index] = edge;
            report.replaced_edges += 1;
            continue;
        }

        if graph
            .edges
            .iter()
            .any(|existing| semantic_edge_matches(existing, &edge))
        {
            report.skipped_edges += 1;
        } else {
            graph.edges.push(edge);
            report.added_edges += 1;
        }
    }

    for diagnostic in &patch.diagnostics {
        let source = node_at_location(
            &graph,
            &diagnostic.path,
            Some(diagnostic.line),
            Some(diagnostic.column),
        )
        .map(|node| node.id);
        let diagnostic_id = add_diagnostic_node(&mut graph, diagnostic);
        report.diagnostic_nodes += 1;
        if let Some(source) = source {
            graph.add_edge_with_metadata(
                source,
                diagnostic_id,
                EdgeKind::MayError,
                Confidence::Semantic,
                diagnostic_edge_metadata(diagnostic),
            );
            report.diagnostic_edges += 1;
        }
    }

    SemanticGraphApplyResult { graph, report }
}

/// Owns a spawned language-server process and kills+reaps it on drop unless
/// disarmed. Every error path after spawn (a hung server hitting the read
/// timeout is the likely case) would otherwise leak the child and its stdout
/// reader thread.
struct ChildGuard {
    child: Child,
    armed: bool,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn run_semantic_server_batch(
    workspace_root: &Path,
    server_batch: &SemanticServerBatch,
    options: &SemanticLspRunOptions,
) -> Result<Vec<SemanticLspResponse>, SemanticLspRunError> {
    let executable = server_batch
        .path
        .as_deref()
        .unwrap_or(server_batch.command.as_str());
    let child = Command::new(executable)
        .args(&server_batch.args)
        .current_dir(workspace_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| {
            SemanticLspRunError::new(format!(
                "failed to start language server `{}`: {error}",
                server_batch.server
            ))
        })?;
    let mut child = ChildGuard::new(child);

    let mut stdin = child.child.stdin.take().ok_or_else(|| {
        SemanticLspRunError::new(format!(
            "language server `{}` did not expose stdin",
            server_batch.server
        ))
    })?;
    let stdout = child.child.stdout.take().ok_or_else(|| {
        SemanticLspRunError::new(format!(
            "language server `{}` did not expose stdout",
            server_batch.server
        ))
    })?;
    let receiver = spawn_lsp_reader(server_batch.server.clone(), stdout);
    let mut opened_documents = BTreeSet::new();
    let mut responses = Vec::new();

    for request in &server_batch.requests {
        if request.request_kind == "request" {
            open_request_document(
                workspace_root,
                server_batch,
                request,
                &mut opened_documents,
                &mut stdin,
            )?;
            write_lsp_request(&mut stdin, request)?;
            let response = read_lsp_response(
                server_batch,
                &receiver,
                &mut stdin,
                request,
                options.request_timeout,
            )?;
            if request.work_item_id.is_some() {
                responses.push(response);
            }
        } else if request.request_kind == "notification" {
            write_lsp_notification(&mut stdin, request.method, request.params.clone())?;
            if request.method == "initialized" {
                wait_for_server_to_settle(&receiver, &mut stdin, options.settle_timeout);
            }
        }
    }

    shutdown_lsp_server(server_batch, &receiver, &mut stdin, options.request_timeout)?;
    wait_for_lsp_exit(
        &mut child.child,
        &server_batch.server,
        options.request_timeout,
    )?;
    child.disarm();
    Ok(responses)
}

fn spawn_lsp_reader(
    server: String,
    stdout: impl Read + Send + 'static,
) -> Receiver<Result<Value, SemanticLspRunError>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_lsp_message(&mut reader) {
                Ok(Some(message)) => {
                    if sender.send(Ok(message)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = sender.send(Err(SemanticLspRunError::new(format!(
                        "failed to read language server `{server}` response: {error}"
                    ))));
                    break;
                }
            }
        }
    });
    receiver
}

fn open_request_document(
    workspace_root: &Path,
    server_batch: &SemanticServerBatch,
    request: &SemanticLspRequest,
    opened_documents: &mut BTreeSet<String>,
    stdin: &mut ChildStdin,
) -> Result<(), SemanticLspRunError> {
    let Some(uri) = request.document_uri.as_ref() else {
        return Ok(());
    };
    if !opened_documents.insert(uri.clone()) {
        return Ok(());
    }
    let Some(path) = request.path.as_deref() else {
        return Ok(());
    };
    let absolute_path = workspace_root.join(path);
    let bytes = fs::read(&absolute_path).map_err(|error| {
        SemanticLspRunError::new(format!(
            "failed to read `{}` for LSP didOpen: {error}",
            absolute_path.display()
        ))
    })?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    write_lsp_notification(
        stdin,
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": lsp_language_id(server_batch, path),
                "version": 1,
                "text": text,
            },
        }),
    )
}

fn read_lsp_response(
    server_batch: &SemanticServerBatch,
    receiver: &Receiver<Result<Value, SemanticLspRunError>>,
    stdin: &mut ChildStdin,
    request: &SemanticLspRequest,
    timeout: Duration,
) -> Result<SemanticLspResponse, SemanticLspRunError> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(SemanticLspRunError::new(format!(
                "timed out waiting for `{}` response from `{}`",
                request.method, server_batch.server
            )));
        }
        let message = receiver.recv_timeout(remaining).map_err(|error| {
            SemanticLspRunError::new(format!(
                "language server `{}` closed before `{}` response: {error}",
                server_batch.server, request.method
            ))
        })??;

        if is_server_request(&message) {
            write_lsp_server_request_response(stdin, &message)?;
            continue;
        }
        if !lsp_response_id_matches(&message, &request.id) {
            continue;
        }
        return Ok(semantic_lsp_response_from_message(request, &message));
    }
}

fn shutdown_lsp_server(
    server_batch: &SemanticServerBatch,
    receiver: &Receiver<Result<Value, SemanticLspRunError>>,
    stdin: &mut ChildStdin,
    timeout: Duration,
) -> Result<(), SemanticLspRunError> {
    let shutdown = SemanticLspRequest {
        id: format!("lsp:{}:shutdown", server_batch.server),
        work_item_id: None,
        request_kind: "request",
        method: "shutdown",
        params: Value::Null,
        document_uri: None,
        path: None,
        line: None,
        column: None,
        expected_result: "null",
    };
    write_lsp_request(stdin, &shutdown)?;
    let _ = read_lsp_response(server_batch, receiver, stdin, &shutdown, timeout)?;
    write_lsp_notification(stdin, "exit", Value::Null)
}

fn wait_for_lsp_exit(
    child: &mut Child,
    server: &str,
    timeout: Duration,
) -> Result<(), SemanticLspRunError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            SemanticLspRunError::new(format!("failed to poll `{server}` exit: {error}"))
        })? {
            if !status.success() {
                // Some servers exit non-zero after a clean shutdown request.
                // Every response has already been collected at this point;
                // failing here threw away the whole batch for nothing.
                eprintln!("warning: language server `{server}` exited with {status}");
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            child.kill().map_err(|error| {
                SemanticLspRunError::new(format!(
                    "failed to stop `{server}` after shutdown timeout: {error}"
                ))
            })?;
            let _ = child.wait();
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn semantic_batch_hash(batch: &SemanticExecutionBatch) -> Result<String, SemanticLspRunError> {
    let bytes = serde_json::to_vec(batch).map_err(|error| {
        SemanticLspRunError::new(format!("failed to hash semantic LSP batch: {error}"))
    })?;
    let mut hash = FNV_OFFSET;
    fnv_update(&mut hash, &bytes);
    // Mix in a stamp (size + mtime) of every file the batch touches. The batch
    // itself is derived from the syntactic graph, so an edit that doesn't
    // change the graph (e.g. a changed literal) would otherwise hash the same
    // and replay stale diagnostics from the cache forever.
    let root = Path::new(&batch.workspace_root);
    let mut paths = BTreeSet::new();
    for server_batch in &batch.server_batches {
        for request in &server_batch.requests {
            if let Some(path) = request.path.as_deref() {
                paths.insert(path);
            }
        }
    }
    for path in paths {
        fnv_update(&mut hash, path.as_bytes());
        let stamp = fs::metadata(root.join(path))
            .map(|metadata| {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos())
                    .unwrap_or(0);
                (metadata.len(), modified)
            })
            .unwrap_or((u64::MAX, u128::MAX));
        fnv_update(&mut hash, &stamp.0.to_le_bytes());
        fnv_update(&mut hash, &stamp.1.to_le_bytes());
    }
    Ok(format!("{hash:016x}"))
}

fn fnv_update(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn write_lsp_request<W: Write>(
    writer: &mut W,
    request: &SemanticLspRequest,
) -> Result<(), SemanticLspRunError> {
    write_lsp_json(
        writer,
        &json!({
            "jsonrpc": "2.0",
            "id": request.id.clone(),
            "method": request.method,
            "params": request.params.clone(),
        }),
    )
}

fn write_lsp_notification<W: Write>(
    writer: &mut W,
    method: &str,
    params: Value,
) -> Result<(), SemanticLspRunError> {
    write_lsp_json(
        writer,
        &json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }),
    )
}

fn write_lsp_server_request_response<W: Write>(
    writer: &mut W,
    request: &Value,
) -> Result<(), SemanticLspRunError> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    write_lsp_json(
        writer,
        &json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": Value::Null,
        }),
    )
}

fn write_lsp_json<W: Write>(writer: &mut W, message: &Value) -> Result<(), SemanticLspRunError> {
    let bytes = serde_json::to_vec(message).map_err(|error| {
        SemanticLspRunError::new(format!("failed to encode LSP message: {error}"))
    })?;
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len()).map_err(|error| {
        SemanticLspRunError::new(format!("failed to write LSP header: {error}"))
    })?;
    writer
        .write_all(&bytes)
        .map_err(|error| SemanticLspRunError::new(format!("failed to write LSP body: {error}")))?;
    writer
        .flush()
        .map_err(|error| SemanticLspRunError::new(format!("failed to flush LSP message: {error}")))
}

fn read_lsp_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(content_length) = content_length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing LSP Content-Length header",
        ));
    };
    // A broken server could declare an absurd length and drive an unbounded
    // allocation; no real LSP response approaches this bound.
    const MAX_LSP_MESSAGE_BYTES: usize = 256 * 1024 * 1024;
    if content_length > MAX_LSP_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "LSP message of {content_length} bytes exceeds the {MAX_LSP_MESSAGE_BYTES}-byte limit"
            ),
        ));
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid LSP JSON message: {error}"),
        )
    })
}

fn semantic_lsp_response_from_message(
    request: &SemanticLspRequest,
    message: &Value,
) -> SemanticLspResponse {
    SemanticLspResponse {
        request_id: request.id.clone(),
        method: request.method.to_string(),
        result: message.get("result").cloned().unwrap_or(Value::Null),
        error: message.get("error").map(lsp_error_from_message),
    }
}

fn lsp_error_from_message(value: &Value) -> SemanticLspError {
    SemanticLspError {
        code: value.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("language server error")
            .to_string(),
    }
}

/// Give a server that reports progress time to finish its first pass.
///
/// rust-analyzer answers `textDocument/definition` with `[]` and
/// `textDocument/references` with `-32801 content modified` until it has
/// built the crate graph, and both of those read as answers rather than as
/// "ask me later" -- a run of 297 requests came back 297 times empty. It
/// says when it is done: a `$/progress` whose value is `{"kind": "end"}`.
///
/// Loading has phases, so one `end` is not the end. After the first the wait
/// keeps draining until the server goes quiet, and the whole thing is
/// bounded: a server that reports no progress at all costs one idle gap and
/// nothing else. It never fails -- a server that will not settle is still
/// worth asking, and its answers are what they are.
fn wait_for_server_to_settle(
    receiver: &Receiver<Result<Value, SemanticLspRunError>>,
    stdin: &mut ChildStdin,
    timeout: Duration,
) {
    /// How long a settled server stays silent before the wait believes it.
    const QUIET: Duration = Duration::from_millis(750);
    let deadline = Instant::now() + timeout;
    let mut ended = false;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        let wait = if ended {
            QUIET.min(remaining)
        } else {
            remaining
        };
        let Ok(message) = receiver.recv_timeout(wait) else {
            // Silence: either the server is done talking, or it never
            // started. Both mean there is nothing more to wait for.
            return;
        };
        let Ok(message) = message else {
            return;
        };
        if is_server_request(&message) {
            let _ = write_lsp_server_request_response(stdin, &message);
            continue;
        }
        if progress_ended(&message) {
            ended = true;
        }
    }
}

/// Whether a message is a server saying one of its jobs has finished.
fn progress_ended(message: &Value) -> bool {
    message.get("method").and_then(Value::as_str) == Some("$/progress")
        && message
            .get("params")
            .and_then(|params| params.get("value"))
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str)
            == Some("end")
}

fn is_server_request(message: &Value) -> bool {
    message.get("method").and_then(Value::as_str).is_some()
        && message.get("id").is_some()
        && message.get("result").is_none()
        && message.get("error").is_none()
}

fn lsp_response_id_matches(message: &Value, expected: &str) -> bool {
    message
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| id == expected)
}

fn lsp_language_id(server_batch: &SemanticServerBatch, path: &str) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let language = match extension.as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "go" => "go",
        "c" => "c",
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => "cpp",
        "php" => "php",
        "sh" | "bash" => "shellscript",
        _ => server_batch
            .languages
            .first()
            .map(String::as_str)
            .unwrap_or("text"),
    };
    if language == "bash" {
        "shellscript".to_string()
    } else {
        language.to_string()
    }
}

fn semantic_edge_from_patch(edge_patch: &SemanticEdgePatch) -> Edge {
    Edge {
        source: edge_patch.source,
        target: edge_patch.target,
        kind: edge_patch.kind,
        confidence: Confidence::Semantic,
        metadata: semantic_edge_metadata(edge_patch),
    }
}

fn semantic_edge_matches(left: &Edge, right: &Edge) -> bool {
    left.source == right.source
        && left.target == right.target
        && left.kind == right.kind
        && left.confidence == right.confidence
}

fn semantic_edge_metadata(edge_patch: &SemanticEdgePatch) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::from([
        ("source".to_string(), "lsp".to_string()),
        ("relation".to_string(), "semantic_resolution".to_string()),
        ("request_id".to_string(), edge_patch.request_id.clone()),
        ("work_item_id".to_string(), edge_patch.work_item_id.clone()),
        ("evidence".to_string(), edge_patch.evidence.to_string()),
    ]);
    // Where the call is written, the way every other call edge says it. The
    // answer's own location went here before, which for a definition in the
    // standard library is an absolute path on the machine that ran the
    // pass: no other span in the graph is one, and it means nothing
    // anywhere else. A reference edge has no call site and its location is
    // the site, so it keeps what it had.
    let (file, line, column) = match &edge_patch.call_file {
        Some(file) => (
            file.clone(),
            edge_patch.call_line.unwrap_or(edge_patch.line),
            edge_patch.call_column.unwrap_or(edge_patch.column),
        ),
        None => (edge_patch.path.clone(), edge_patch.line, edge_patch.column),
    };
    metadata.insert("file".to_string(), file);
    metadata.insert("line".to_string(), line.to_string());
    metadata.insert("column".to_string(), column.to_string());
    // Where the definition turned out to be, when that is a place this
    // project contains. Outside it the evidence and the target already say
    // which dependency or library answered, and the path would only be
    // this machine's.
    if !edge_patch.path.starts_with('/') && !edge_patch.path.contains(":\\") {
        metadata.insert("definition_file".to_string(), edge_patch.path.clone());
        metadata.insert("definition_line".to_string(), edge_patch.line.to_string());
    }
    // A semantic edge is a resolved call, and saying so is what lets every
    // count and filter that groups by resolution see the strongest edges in
    // the graph. Applying the pass to ripgrep had been making its reported
    // resolution rate worse, because 337 calls left every category at once.
    if edge_patch.kind == EdgeKind::Calls {
        let resolution = match edge_patch.evidence {
            "lsp_definition_in_standard_library" => "builtin",
            _ => "resolved",
        };
        metadata.insert("resolution".to_string(), resolution.to_string());
        metadata.insert("resolution_basis".to_string(), "semantic".to_string());
    }
    if let Some(label) = &edge_patch.call_label {
        metadata.insert("call_label".to_string(), label.clone());
    }
    if let Some(language) = &edge_patch.language {
        metadata.insert("language".to_string(), language.clone());
    }
    if let Some(index) = edge_patch.replaced_edge_index {
        metadata.insert("replaced_edge_index".to_string(), index.to_string());
    }
    if let Some(target) = edge_patch.original_target {
        metadata.insert("original_target".to_string(), target.to_string());
    }
    metadata
}

fn add_diagnostic_node(graph: &mut CodeGraph, diagnostic: &SemanticDiagnosticPatch) -> NodeId {
    let severity = diagnostic
        .severity
        .map(diagnostic_severity_label)
        .unwrap_or("unknown");
    let label = format!("{}: {}", severity, truncate_label(&diagnostic.message, 96));
    graph.add_node_with_metadata(
        NodeKind::Unknown,
        label,
        Some(SourceSpan {
            path: diagnostic.path.clone(),
            start_line: diagnostic.line,
            start_column: diagnostic.column,
            end_line: diagnostic.line,
            end_column: diagnostic.column.saturating_add(1),
        }),
        diagnostic_node_metadata(diagnostic),
    )
}

fn diagnostic_node_metadata(diagnostic: &SemanticDiagnosticPatch) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::from([
        ("item_kind".to_string(), "diagnostic".to_string()),
        ("source".to_string(), "lsp".to_string()),
        ("request_id".to_string(), diagnostic.request_id.clone()),
        ("work_item_id".to_string(), diagnostic.work_item_id.clone()),
        ("message".to_string(), diagnostic.message.clone()),
        ("path".to_string(), diagnostic.path.clone()),
        ("line".to_string(), diagnostic.line.to_string()),
        ("column".to_string(), diagnostic.column.to_string()),
    ]);
    if let Some(severity) = diagnostic.severity {
        metadata.insert(
            "severity".to_string(),
            diagnostic_severity_label(severity).to_string(),
        );
        metadata.insert("severity_code".to_string(), severity.to_string());
    }
    if let Some(code) = &diagnostic.code {
        metadata.insert("diagnostic_code".to_string(), code.clone());
    }
    if let Some(source) = &diagnostic.source {
        metadata.insert("diagnostic_source".to_string(), source.clone());
    }
    metadata
}

fn diagnostic_edge_metadata(diagnostic: &SemanticDiagnosticPatch) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::from([
        ("source".to_string(), "lsp".to_string()),
        ("relation".to_string(), "diagnostic".to_string()),
        ("request_id".to_string(), diagnostic.request_id.clone()),
        ("work_item_id".to_string(), diagnostic.work_item_id.clone()),
        ("path".to_string(), diagnostic.path.clone()),
        ("line".to_string(), diagnostic.line.to_string()),
        ("column".to_string(), diagnostic.column.to_string()),
    ]);
    if let Some(severity) = diagnostic.severity {
        metadata.insert(
            "severity".to_string(),
            diagnostic_severity_label(severity).to_string(),
        );
    }
    metadata
}

fn diagnostic_severity_label(severity: u64) -> &'static str {
    match severity {
        1 => "error",
        2 => "warning",
        3 => "info",
        4 => "hint",
        _ => "unknown",
    }
}

fn truncate_label(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(3)).collect();
    truncated.push_str("...");
    truncated
}

fn collect_definition_edges(
    workspace_root: &Path,
    graph: &CodeGraph,
    request: &SemanticLspRequest,
    work_item: &SemanticWorkItem,
    result: &Value,
    semantic_edges: &mut Vec<SemanticEdgePatch>,
    unmatched_locations: &mut Vec<SemanticUnmatchedLocation>,
) {
    let Some(source) = work_item.node.as_ref() else {
        return;
    };
    let replaced = work_item
        .edge_index
        .and_then(|index| graph.edges.get(index));
    let edge_kind = replaced
        .map(|edge| edge.kind)
        .unwrap_or(EdgeKind::References);
    let replaced_label = replaced.and_then(|edge| edge.metadata.get("call_label").cloned());
    let replaced_language = replaced.and_then(|edge| edge.metadata.get("language").cloned());
    let call_file = replaced.and_then(|edge| edge.metadata.get("file").cloned());
    let call_line = replaced.and_then(|edge| numeric_metadata(edge, "line"));
    let call_column = replaced.and_then(|edge| numeric_metadata(edge, "column"));
    let original_target = work_item.target.as_ref().map(|target| target.id);

    for location in lsp_locations(result) {
        match graph_node_for_location(workspace_root, graph, &location) {
            // A definition that lands back on the asking node says nothing:
            // it happens when the position falls inside the caller itself (a
            // local variable, a recursive call). Recording it would add an
            // edge from a node to itself and call it semantic knowledge.
            Some((_, target)) if target.id == source.id => {
                unmatched_locations.push(unmatched_location(
                    request,
                    work_item,
                    &location,
                    workspace_root,
                    "definition_resolves_to_the_asking_node",
                ))
            }
            Some((path, target)) => semantic_edges.push(SemanticEdgePatch {
                request_id: request.id.clone(),
                work_item_id: work_item.id.clone(),
                source: source.id,
                target: target.id,
                kind: edge_kind,
                confidence: Confidence::Semantic,
                replaced_edge_index: work_item.edge_index,
                original_target,
                path,
                line: location.line.unwrap_or(1),
                column: location.column.unwrap_or(1),
                evidence: "lsp_definition",
                call_label: replaced_label.clone(),
                language: replaced_language.clone(),
                call_file: call_file.clone(),
                call_line,
                call_column,
            }),
            // The definition is not this project's, but the path still says
            // whose it is, and a dependency the project declares is a
            // better answer than none: 12 of ripgrep's first 100 answers
            // name `log`, `anyhow`, `regex`, `memchr` and the like.
            None => match dependency_for_location(graph, workspace_root, &location) {
                Some(dependency) => semantic_edges.push(SemanticEdgePatch {
                    request_id: request.id.clone(),
                    work_item_id: work_item.id.clone(),
                    source: source.id,
                    target: dependency.id,
                    kind: edge_kind,
                    confidence: Confidence::Semantic,
                    replaced_edge_index: work_item.edge_index,
                    original_target,
                    path: path_from_file_uri(workspace_root, &location.uri)
                        .unwrap_or_else(|| location.uri.clone()),
                    line: location.line.unwrap_or(1),
                    column: location.column.unwrap_or(1),
                    evidence: "lsp_definition_in_dependency",
                    call_label: replaced_label.clone(),
                    language: replaced_language.clone(),
                    call_file: call_file.clone(),
                    call_line,
                    call_column,
                }),
                // The language server has placed the definition in the
                // language's own library, which is a better answer than
                // "unresolved" and one no hand-written list can give:
                // `unwrap` and `clone` were left out of the builtin lists
                // precisely because a project may declare them, and here
                // the compiler has said it did not. 595 of ripgrep's first
                // 1000 answers are this, every one of them.
                None => match standard_library_edge(
                    workspace_root,
                    &location,
                    request,
                    work_item,
                    source,
                    edge_kind,
                    original_target,
                    replaced_label.clone(),
                    replaced_language.clone(),
                    call_file.clone(),
                    call_line,
                    call_column,
                ) {
                    Some(edge) => semantic_edges.push(edge),
                    None => unmatched_locations.push(unmatched_location(
                        request,
                        work_item,
                        &location,
                        workspace_root,
                        missing_node_reason(workspace_root, &location.uri),
                    )),
                },
            },
        }
    }
}

/// A number an edge records in its metadata.
fn numeric_metadata(edge: &Edge, key: &str) -> Option<u32> {
    edge.metadata.get(key)?.parse().ok()
}

/// A call whose definition the server placed in the language's own
/// library. It keeps the target it had -- the placeholder that already
/// stands for the call -- and says `builtin` with the evidence that
/// settled it, so the graph stops reporting a resolver failure where the
/// compiler has given a plain answer.
#[allow(clippy::too_many_arguments)]
fn standard_library_edge(
    workspace_root: &Path,
    location: &LspLocation,
    request: &SemanticLspRequest,
    work_item: &SemanticWorkItem,
    source: &SemanticNodeRef,
    edge_kind: EdgeKind,
    original_target: Option<NodeId>,
    call_label: Option<String>,
    language: Option<String>,
    call_file: Option<String>,
    call_line: Option<u32>,
    call_column: Option<u32>,
) -> Option<SemanticEdgePatch> {
    if !location_is_outside_the_project(workspace_root, &location.uri) {
        return None;
    }
    let path = path_from_file_uri(workspace_root, &location.uri)?;
    if !path_is_in_a_standard_library(&path) {
        return None;
    }
    let target = original_target?;
    Some(SemanticEdgePatch {
        request_id: request.id.clone(),
        work_item_id: work_item.id.clone(),
        source: source.id,
        target,
        kind: edge_kind,
        confidence: Confidence::Semantic,
        replaced_edge_index: work_item.edge_index,
        original_target,
        path,
        line: location.line.unwrap_or(1),
        column: location.column.unwrap_or(1),
        evidence: "lsp_definition_in_standard_library",
        call_label,
        language,
        call_file,
        call_line,
        call_column,
    })
}

/// Whether a path is inside a language's own library. Only the layouts a
/// path states outright are recognised: rust ships its sources under
/// `rustlib/src/rust/library`, and python keeps its own modules in `lib/
/// python3.x` with everything installed beside them in `site-packages`.
/// A path this cannot name stays unmatched rather than being guessed at.
fn path_is_in_a_standard_library(path: &str) -> bool {
    let path = path.replace('\\', "/");
    if path.contains("/rustlib/src/rust/library/") {
        return true;
    }
    if path.contains("/site-packages/") || path.contains("/dist-packages/") {
        return false;
    }
    path.contains("/lib/python3") || path.contains("/lib/python2")
}

/// The dependency node a location outside the project belongs to./// The dependency node a location outside the project belongs to. Every
/// ecosystem keeps its packages in a directory that names them -- cargo's
/// `registry/src/<index>/regex-1.10.2`, npm's `node_modules/regex`,
/// python's `site-packages/regex`, composer's `vendor/<org>/<name>`,
/// rubygems' `gems/<name>-1.0` -- so a definition the server places there
/// says which dependency the call reaches. Only one the project declares
/// counts: the standard library is in none of these directories, and a
/// package nothing declares has no node to point at.
fn dependency_for_location<'a>(
    graph: &'a CodeGraph,
    workspace_root: &Path,
    location: &LspLocation,
) -> Option<&'a Node> {
    if !location_is_outside_the_project(workspace_root, &location.uri) {
        return None;
    }
    let path = path_from_file_uri(workspace_root, &location.uri)?;
    let name = dependency_named_by_path(&path)?;
    graph.nodes.iter().find(|node| {
        node.kind == NodeKind::ExternalDependency
            && node.label == name
            && node.metadata.get("item_kind").map(String::as_str) == Some("dependency")
    })
}

/// The package a path inside a dependency directory names.
fn dependency_named_by_path(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if let Some(index) = path.find("/registry/src/") {
        // cargo unpacks `regex-1.10.2` beside the index it came from.
        let rest = &path[index + "/registry/src/".len()..];
        let directory = rest.split('/').nth(1)?;
        let (name, version) = directory.rsplit_once('-')?;
        if version.starts_with(|character: char| character.is_ascii_digit()) {
            return Some(name.to_string());
        }
    }
    if let Some(index) = path.rfind("/node_modules/") {
        let rest = &path[index + "/node_modules/".len()..];
        let mut parts = rest.split('/');
        let first = parts.next()?;
        return Some(match first.starts_with('@') {
            true => format!("{first}/{}", parts.next()?),
            false => first.to_string(),
        });
    }
    if let Some(index) = path.find("/site-packages/") {
        let rest = &path[index + "/site-packages/".len()..];
        return rest.split('/').next().map(str::to_string);
    }
    if let Some(index) = path.rfind("/vendor/") {
        let rest = &path[index + "/vendor/".len()..];
        let mut parts = rest.split('/');
        let organisation = parts.next()?;
        let name = parts.next()?;
        return Some(format!("{organisation}/{name}"));
    }
    if let Some(index) = path.rfind("/gems/") {
        let rest = &path[index + "/gems/".len()..];
        let directory = rest.split('/').next()?;
        let (name, version) = directory.rsplit_once('-')?;
        if version.starts_with(|character: char| character.is_ascii_digit()) {
            return Some(name.to_string());
        }
    }
    None
}

fn collect_reference_edges(
    workspace_root: &Path,
    graph: &CodeGraph,
    request: &SemanticLspRequest,
    work_item: &SemanticWorkItem,
    result: &Value,
    semantic_edges: &mut Vec<SemanticEdgePatch>,
    unmatched_locations: &mut Vec<SemanticUnmatchedLocation>,
) {
    let Some(target) = work_item.node.as_ref() else {
        return;
    };

    for location in lsp_locations(result) {
        match graph_node_for_location(workspace_root, graph, &location) {
            Some((path, source)) => {
                if source.id == target.id {
                    continue;
                }
                semantic_edges.push(SemanticEdgePatch {
                    request_id: request.id.clone(),
                    work_item_id: work_item.id.clone(),
                    source: source.id,
                    target: target.id,
                    kind: EdgeKind::References,
                    confidence: Confidence::Semantic,
                    replaced_edge_index: None,
                    original_target: None,
                    path,
                    line: location.line.unwrap_or(1),
                    column: location.column.unwrap_or(1),
                    evidence: "lsp_references",
                    call_label: None,
                    language: None,
                    call_file: None,
                    call_line: None,
                    call_column: None,
                });
            }
            None => unmatched_locations.push(unmatched_location(
                request,
                work_item,
                &location,
                workspace_root,
                missing_node_reason(workspace_root, &location.uri),
            )),
        }
    }
}

fn collect_diagnostics(
    workspace_root: &Path,
    request: &SemanticLspRequest,
    work_item: &SemanticWorkItem,
    result: &Value,
    diagnostics: &mut Vec<SemanticDiagnosticPatch>,
    unmatched_locations: &mut Vec<SemanticUnmatchedLocation>,
) {
    for diagnostic in lsp_diagnostics(result) {
        let path =
            path_from_file_uri(workspace_root, &diagnostic.uri).or_else(|| request.path.clone());
        let Some(path) = path else {
            unmatched_locations.push(SemanticUnmatchedLocation {
                request_id: request.id.clone(),
                work_item_id: Some(work_item.id.clone()),
                method: request.method.to_string(),
                uri: diagnostic.uri,
                path: None,
                line: diagnostic.line,
                column: diagnostic.column,
                reason: "diagnostic_path_not_in_workspace",
            });
            continue;
        };
        diagnostics.push(SemanticDiagnosticPatch {
            request_id: request.id.clone(),
            work_item_id: work_item.id.clone(),
            path,
            line: diagnostic.line.unwrap_or(1),
            column: diagnostic.column.unwrap_or(1),
            severity: diagnostic.severity,
            code: diagnostic.code,
            source: diagnostic.source,
            message: diagnostic.message,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LspLocation {
    uri: String,
    line: Option<u32>,
    column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LspDiagnostic {
    uri: String,
    line: Option<u32>,
    column: Option<u32>,
    severity: Option<u64>,
    code: Option<String>,
    source: Option<String>,
    message: String,
}

fn lsp_locations(result: &Value) -> Vec<LspLocation> {
    if result.is_null() {
        return Vec::new();
    }
    if let Some(items) = result.as_array() {
        return items.iter().filter_map(lsp_location).collect();
    }
    lsp_location(result).into_iter().collect()
}

fn lsp_location(value: &Value) -> Option<LspLocation> {
    let uri = value
        .get("targetUri")
        .or_else(|| value.get("uri"))?
        .as_str()?
        .to_string();
    let range = value
        .get("targetSelectionRange")
        .or_else(|| value.get("targetRange"))
        .or_else(|| value.get("range"));
    let (line, column) = range
        .and_then(|range| range.get("start"))
        .map(lsp_position)
        .unwrap_or((None, None));
    Some(LspLocation { uri, line, column })
}

fn lsp_diagnostics(result: &Value) -> Vec<LspDiagnostic> {
    let items = result
        .get("items")
        .or_else(|| result.get("diagnostics"))
        .unwrap_or(result);
    let Some(items) = items.as_array() else {
        return Vec::new();
    };

    items.iter().filter_map(lsp_diagnostic).collect()
}

fn lsp_diagnostic(value: &Value) -> Option<LspDiagnostic> {
    let message = value.get("message")?.as_str()?.to_string();
    let uri = value
        .get("uri")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default();
    let (line, column) = value
        .get("range")
        .and_then(|range| range.get("start"))
        .map(lsp_position)
        .unwrap_or((None, None));
    Some(LspDiagnostic {
        uri,
        line,
        column,
        severity: value.get("severity").and_then(Value::as_u64),
        code: diagnostic_code(value.get("code")),
        source: value
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_string),
        message,
    })
}

fn lsp_position(value: &Value) -> (Option<u32>, Option<u32>) {
    let line = value
        .get("line")
        .and_then(Value::as_u64)
        .and_then(|line| u32::try_from(line + 1).ok());
    let column = value
        .get("character")
        .and_then(Value::as_u64)
        .and_then(|column| u32::try_from(column + 1).ok());
    (line, column)
}

fn diagnostic_code(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Object(object) => object
            .get("value")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn graph_node_for_location<'a>(
    workspace_root: &Path,
    graph: &'a CodeGraph,
    location: &LspLocation,
) -> Option<(String, &'a Node)> {
    let path = path_from_file_uri(workspace_root, &location.uri)?;
    let node = node_at_location(graph, &path, location.line, location.column)?;
    Some((path, node))
}

fn node_at_location<'a>(
    graph: &'a CodeGraph,
    path: &str,
    line: Option<u32>,
    column: Option<u32>,
) -> Option<&'a Node> {
    let normalized_path = normalize_graph_path(path);
    let mut candidates: Vec<_> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.span.as_ref().is_some_and(|span| {
                normalize_graph_path(&span.path) == normalized_path
                    && span_contains(span, line, column)
            })
        })
        .collect();
    candidates.sort_by_key(|node| {
        (
            node_span_size(node.span.as_ref()),
            node_kind_rank(&node.kind),
            node.id.0,
        )
    });
    candidates.into_iter().next().or_else(|| {
        graph.nodes.iter().find(|node| {
            node.kind == NodeKind::File && normalize_graph_path(&node.label) == normalized_path
        })
    })
}

fn span_contains(span: &SourceSpan, line: Option<u32>, column: Option<u32>) -> bool {
    let Some(line) = line else {
        return true;
    };
    if line < span.start_line || line > span.end_line {
        return false;
    }
    let Some(column) = column else {
        return true;
    };
    if line == span.start_line && column < span.start_column {
        return false;
    }
    if line == span.end_line && column > span.end_column {
        return false;
    }
    true
}

fn node_span_size(span: Option<&SourceSpan>) -> u32 {
    span.map(|span| {
        span.end_line
            .saturating_sub(span.start_line)
            .saturating_mul(100_000)
            + span.end_column.saturating_sub(span.start_column)
    })
    .unwrap_or(u32::MAX)
}

fn node_kind_rank(kind: &NodeKind) -> usize {
    match kind {
        NodeKind::Function | NodeKind::Entrypoint => 0,
        NodeKind::Type | NodeKind::Module => 1,
        NodeKind::Config | NodeKind::Environment => 2,
        NodeKind::File => 3,
        _ => 4,
    }
}

fn unmatched_location(
    request: &SemanticLspRequest,
    work_item: &SemanticWorkItem,
    location: &LspLocation,
    workspace_root: &Path,
    reason: &'static str,
) -> SemanticUnmatchedLocation {
    SemanticUnmatchedLocation {
        request_id: request.id.clone(),
        work_item_id: Some(work_item.id.clone()),
        method: request.method.to_string(),
        uri: location.uri.clone(),
        path: path_from_file_uri(workspace_root, &location.uri),
        line: location.line,
        column: location.column,
        reason,
    }
}

/// A location the language server reports can fall outside this project: a
/// definition in a dependency, or one in the standard library. The graph holds
/// this project's files and nothing else, so finding no node there is the
/// expected answer. Calling it a missing graph node would report a gap in what
/// the parser saw and send the reader hunting for a defect that is not there.
fn missing_node_reason(workspace_root: &Path, uri: &str) -> &'static str {
    if location_is_outside_the_project(workspace_root, uri) {
        "location_is_outside_the_project"
    } else {
        "no_graph_node_at_lsp_location"
    }
}

fn location_is_outside_the_project(workspace_root: &Path, uri: &str) -> bool {
    let Some(encoded) = uri.strip_prefix("file://") else {
        return false;
    };
    let Some(decoded) = percent_decode_path(encoded) else {
        return false;
    };
    !PathBuf::from(decoded).starts_with(workspace_root)
}

fn path_from_file_uri(workspace_root: &Path, uri: &str) -> Option<String> {
    let encoded = uri.strip_prefix("file://")?;
    let decoded = percent_decode_path(encoded)?;
    let absolute = PathBuf::from(decoded);
    let relative = absolute.strip_prefix(workspace_root).unwrap_or(&absolute);
    Some(path_to_graph_path(relative))
}

fn path_to_graph_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn normalize_graph_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn percent_decode_path(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            decoded.push(hex_value(high)? * 16 + hex_value(low)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Default)]
struct LanguagePlanAccumulator {
    nodes: usize,
    files: usize,
    symbol_nodes: usize,
    heuristic_edges_to_upgrade: usize,
}

#[derive(Debug)]
struct SemanticServerBatchBuilder {
    server: String,
    command: String,
    args: Vec<String>,
    installed: bool,
    path: Option<String>,
    workspace_root: PathBuf,
    languages: BTreeSet<String>,
    work_items: Vec<SemanticWorkItem>,
}

impl SemanticServerBatchBuilder {
    fn new(server_id: &str, server: Option<&LspServerStatus>, workspace_root: &Path) -> Self {
        Self {
            server: server_id.to_string(),
            command: server
                .map(|server| server.command.to_string())
                .unwrap_or_else(|| server_id.to_string()),
            args: server
                .map(|server| server.args.iter().map(|arg| (*arg).to_string()).collect())
                .unwrap_or_default(),
            installed: server.is_some_and(|server| server.installed),
            path: server.and_then(|server| server.path.clone()),
            workspace_root: workspace_root.to_path_buf(),
            languages: BTreeSet::new(),
            work_items: Vec::new(),
        }
    }

    fn finish(self) -> SemanticServerBatch {
        let requests = lsp_requests_for_batch(&self.workspace_root, &self.server, &self.work_items);
        SemanticServerBatch {
            status: if self.installed {
                "ready"
            } else {
                "missing_server"
            },
            server: self.server,
            command: self.command,
            args: self.args,
            installed: self.installed,
            path: self.path,
            languages: self.languages.into_iter().collect(),
            work_items: self.work_items,
            requests,
        }
    }
}

fn lsp_requests_for_batch(
    workspace_root: &Path,
    server: &str,
    work_items: &[SemanticWorkItem],
) -> Vec<SemanticLspRequest> {
    let root_uri = file_uri(workspace_root);
    let workspace_folder_uri = root_uri.clone();
    let mut requests = vec![
        SemanticLspRequest {
            id: format!("lsp:{server}:initialize"),
            work_item_id: None,
            request_kind: "request",
            method: "initialize",
            params: json!({
                "processId": null,
                "rootUri": root_uri,
                "workspaceFolders": [{
                    "uri": workspace_folder_uri,
                    "name": workspace_name(workspace_root),
                }],
                "capabilities": {
                    "textDocument": {
                        "definition": { "linkSupport": true },
                        "references": {},
                        "documentSymbol": { "hierarchicalDocumentSymbolSupport": true },
                        "diagnostic": {},
                    },
                    "workspace": {
                        "symbol": { "resolveSupport": { "properties": ["location.range"] } },
                    },
                },
            }),
            document_uri: None,
            path: None,
            line: None,
            column: None,
            expected_result: "InitializeResult",
        },
        SemanticLspRequest {
            id: format!("lsp:{server}:initialized"),
            work_item_id: None,
            request_kind: "notification",
            method: "initialized",
            params: json!({}),
            document_uri: None,
            path: None,
            line: None,
            column: None,
            expected_result: "none",
        },
    ];

    requests.extend(
        work_items
            .iter()
            .filter_map(|item| lsp_request_for_work_item(workspace_root, server, item)),
    );
    requests
}

fn lsp_request_for_work_item(
    workspace_root: &Path,
    server: &str,
    item: &SemanticWorkItem,
) -> Option<SemanticLspRequest> {
    match item.capability {
        "definitions" => positioned_text_document_request(
            workspace_root,
            server,
            item,
            "textDocument/definition",
            "Definition | LocationLink[]",
            |uri, line, character| {
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                })
            },
        ),
        "references" => positioned_text_document_request(
            workspace_root,
            server,
            item,
            "textDocument/references",
            "Location[]",
            |uri, line, character| {
                json!({
                    "textDocument": { "uri": uri },
                    "position": { "line": line, "character": character },
                    "context": { "includeDeclaration": true },
                })
            },
        ),
        "document_symbols" => text_document_request(
            workspace_root,
            server,
            item,
            "textDocument/documentSymbol",
            "DocumentSymbol[] | SymbolInformation[]",
            |uri| json!({ "textDocument": { "uri": uri } }),
        ),
        "diagnostics" => text_document_request(
            workspace_root,
            server,
            item,
            "textDocument/diagnostic",
            "DocumentDiagnosticReport",
            |uri| json!({ "textDocument": { "uri": uri } }),
        ),
        "workspace_symbols" => Some(SemanticLspRequest {
            id: format!("lsp:{server}:{}", item.id),
            work_item_id: Some(item.id.clone()),
            request_kind: "request",
            method: "workspace/symbol",
            params: json!({ "query": "" }),
            document_uri: None,
            path: None,
            line: None,
            column: None,
            expected_result: "SymbolInformation[] | WorkspaceSymbol[]",
        }),
        _ => None,
    }
}

fn positioned_text_document_request<F>(
    workspace_root: &Path,
    server: &str,
    item: &SemanticWorkItem,
    method: &'static str,
    expected_result: &'static str,
    params: F,
) -> Option<SemanticLspRequest>
where
    F: FnOnce(String, u32, u32) -> Value,
{
    let path = item.path.as_deref()?;
    let line = item.line?;
    let column = item.column?;
    let uri = source_uri(workspace_root, path);
    let lsp_line = line.saturating_sub(1);
    let lsp_character = column.saturating_sub(1);
    Some(SemanticLspRequest {
        id: format!("lsp:{server}:{}", item.id),
        work_item_id: Some(item.id.clone()),
        request_kind: "request",
        method,
        params: params(uri.clone(), lsp_line, lsp_character),
        document_uri: Some(uri),
        path: Some(path.to_string()),
        line: Some(line),
        column: Some(column),
        expected_result,
    })
}

fn text_document_request<F>(
    workspace_root: &Path,
    server: &str,
    item: &SemanticWorkItem,
    method: &'static str,
    expected_result: &'static str,
    params: F,
) -> Option<SemanticLspRequest>
where
    F: FnOnce(String) -> Value,
{
    let path = item.path.as_deref()?;
    let uri = source_uri(workspace_root, path);
    Some(SemanticLspRequest {
        id: format!("lsp:{server}:{}", item.id),
        work_item_id: Some(item.id.clone()),
        request_kind: "request",
        method,
        params: params(uri.clone()),
        document_uri: Some(uri),
        path: Some(path.to_string()),
        line: item.line,
        column: item.column,
        expected_result,
    })
}

fn source_uri(workspace_root: &Path, path: &str) -> String {
    let source_path = Path::new(path);
    let absolute = if source_path.is_absolute() {
        source_path.to_path_buf()
    } else {
        workspace_root.join(source_path)
    };
    file_uri(&absolute)
}

fn file_uri(path: &Path) -> String {
    let mut path_text = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !path_text.starts_with('/') {
        path_text = format!("/{path_text}");
    }
    format!("file://{}", percent_encode_path(&path_text))
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn workspace_name(workspace_root: &Path) -> String {
    workspace_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

impl SemanticRequestCounts {
    fn add(&mut self, other: Self) {
        self.document_symbols += other.document_symbols;
        self.workspace_symbols += other.workspace_symbols;
        self.definitions += other.definitions;
        self.references += other.references;
        self.diagnostics += other.diagnostics;
    }
}

fn semantic_work_items(
    graph: &CodeGraph,
    plans: &[LanguageEnrichmentPlan],
    work_item_limit: usize,
    work_item_filter: &SemanticWorkItemFilter,
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
                work_item_filter,
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
            // The budget is small next to the number of candidate edges, so
            // spend it where syntax gave up: a call the scanner could not
            // resolve, or resolved to several look-alike declarations. Asking
            // about an edge that already points at one declaration mostly
            // confirms what is known. Ties keep graph order, so the selection
            // stays deterministic.
            let mut candidates: Vec<_> =
                language_heuristic_edges(graph, &nodes_by_id, &plan.language)
                    .map(|(edge_index, edge)| {
                        let unresolved = nodes_by_id
                            .get(&edge.target)
                            .and_then(|node| node.metadata.get("resolution"))
                            .is_some_and(|resolution| {
                                matches!(resolution.as_str(), "unresolved" | "ambiguous")
                            });
                        (u8::from(!unresolved), edge_index, edge)
                    })
                    .collect();
            candidates.sort_by_key(|(rank, edge_index, _)| (*rank, *edge_index));
            let candidates = spread_over_directories(candidates, |(_, _, edge)| {
                nodes_by_id
                    .get(&edge.source)
                    .and_then(|node| node.span.as_ref())
                    .map(|span| span.path.as_str())
                    .unwrap_or_default()
            });
            for (_, edge_index, edge) in candidates {
                let source = nodes_by_id.get(&edge.source).copied();
                let target = nodes_by_id.get(&edge.target).copied();
                push_semantic_work_item(
                    &mut work_items,
                    &mut total_work_items,
                    work_item_limit,
                    work_item_filter,
                    edge_work_item("definitions", plan, edge_index, edge, source, target),
                );
            }
        }
        if plan.capabilities.contains(&"workspace_symbols") {
            push_semantic_work_item(
                &mut work_items,
                &mut total_work_items,
                work_item_limit,
                work_item_filter,
                workspace_work_item("workspace_symbols", plan),
            );
        }
        if plan.capabilities.contains(&"diagnostics") {
            for node in language_file_nodes(graph, &plan.language) {
                push_semantic_work_item(
                    &mut work_items,
                    &mut total_work_items,
                    work_item_limit,
                    work_item_filter,
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
                    work_item_filter,
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
                    work_item_filter,
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

/// Interleave candidates so no single directory can spend the whole budget.
///
/// Candidates arrive in graph order, which is the alphabetical file walk, so a
/// vendored `deps/` directory took every request on redis and the project's own
/// `src/` never got asked. Taking one candidate per directory in turn samples
/// the project instead of its first folder, without the scanner having to guess
/// which directories are "third-party". Order within a directory is preserved,
/// and directories are visited in sorted order, so the selection is
/// deterministic.
fn spread_over_directories<T>(candidates: Vec<T>, path_of: impl Fn(&T) -> &str) -> Vec<T> {
    let mut by_directory: BTreeMap<String, VecDeque<T>> = BTreeMap::new();
    for candidate in candidates {
        let path = path_of(&candidate);
        let directory = path
            .rsplit_once('/')
            .map(|(directory, _)| directory)
            .unwrap_or("")
            .to_string();
        by_directory
            .entry(directory)
            .or_default()
            .push_back(candidate);
    }

    let mut spread = Vec::new();
    while !by_directory.is_empty() {
        by_directory.retain(|_, queue| {
            if let Some(candidate) = queue.pop_front() {
                spread.push(candidate);
            }
            !queue.is_empty()
        });
    }
    spread
}

fn push_semantic_work_item(
    work_items: &mut Vec<SemanticWorkItem>,
    total_work_items: &mut usize,
    work_item_limit: usize,
    work_item_filter: &SemanticWorkItemFilter,
    item: SemanticWorkItem,
) {
    if !semantic_work_item_matches(&item, work_item_filter) {
        return;
    }
    *total_work_items += 1;
    if work_items.len() < work_item_limit {
        work_items.push(item);
    }
}

fn semantic_work_item_matches(
    item: &SemanticWorkItem,
    work_item_filter: &SemanticWorkItemFilter,
) -> bool {
    filter_matches(&work_item_filter.language, &item.language)
        && filter_matches(&work_item_filter.status, item.status)
        && filter_matches(&work_item_filter.capability, item.capability)
}

fn filter_matches(filter: &Option<String>, value: &str) -> bool {
    filter
        .as_deref()
        .is_none_or(|filter| filter.is_empty() || filter.eq_ignore_ascii_case(value))
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
    edge: &Edge,
    source: Option<&Node>,
    target: Option<&Node>,
) -> SemanticWorkItem {
    // Ask where the fact actually is. A call edge records its call site, and
    // asking there is the whole point — asking at the caller's own declaration
    // just makes the server answer with the caller, which is how the pass used
    // to spend its budget producing self-referential edges.
    let (path, line, column) = source.map(node_location).unwrap_or((None, None, None));
    let (line, column) = match edge_site(edge) {
        Some(site) => site,
        None => (line, column),
    };
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

fn workspace_work_item(
    capability: &'static str,
    plan: &LanguageEnrichmentPlan,
) -> SemanticWorkItem {
    SemanticWorkItem {
        id: work_item_id(capability, &plan.language, None, None, None),
        kind: "workspace",
        capability,
        priority: work_item_priority(capability),
        reason: work_item_reason(capability),
        language: plan.language.clone(),
        status: plan.status,
        server: plan.server,
        blocked_reason: plan.blocked_reason,
        path: None,
        line: None,
        column: None,
        node: None,
        target: None,
        edge_index: None,
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
        "workspace_symbols" => 50,
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
        "workspace_symbols" => "collect workspace symbols for semantic graph reconciliation",
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

/// The position an edge was recorded at, when it carries one. Call edges do:
/// the indexer stamps the call site on them.
fn edge_site(edge: &Edge) -> Option<(Option<u32>, Option<u32>)> {
    let line = edge.metadata.get("line")?.parse::<u32>().ok()?;
    let column = edge
        .metadata
        .get("column")
        .and_then(|column| column.parse::<u32>().ok())
        .map(|column| column + method_offset_in(edge.metadata.get("call_label")));
    Some((Some(line), column))
}

/// How far past the start of a call its method name begins. A dotted label
/// names a method on a receiver and the span starts at the receiver, so
/// asking there answers with the local variable rather than the method:
/// `builder.add` is ripgrep's own `GlobSetBuilder::add`, and 29 of the
/// first 100 answers came back pointing at the caller because the question
/// was about `builder`. No arithmetic on the source is needed -- the parser
/// rejects a label holding a space or a newline, so a label that survives is
/// written contiguously and the method starts exactly as far along as
/// everything before it is long.
fn method_offset_in(label: Option<&String>) -> u32 {
    let Some(label) = label else {
        return 0;
    };
    if label.contains([' ', '\n', '\t']) {
        return 0;
    }
    let Some(index) = label.rfind(['.', ':']) else {
        return 0;
    };
    // A char boundary is what the LSP counts, and a label can hold one that
    // is not a byte: `crate::Ünicode::read` is a legal Rust path.
    label[..=index].chars().count() as u32
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

fn find_lsp_server_executable(spec: &LspServerSpec) -> Option<PathBuf> {
    let path = find_executable(spec.command)?;
    if lsp_server_executable_is_usable(spec, &path) {
        Some(path)
    } else {
        None
    }
}

fn lsp_server_executable_is_usable(spec: &LspServerSpec, path: &Path) -> bool {
    if spec.id != "rust-analyzer" {
        return true;
    }
    Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
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
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    #[cfg(unix)]
    #[test]
    fn child_guard_kills_process_on_drop_unless_disarmed() {
        // A hung language server must not leak: dropping the guard kills+reaps it.
        let child = Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleeper");
        let pid = child.id();
        let guard = ChildGuard::new(child);
        drop(guard);
        // kill+wait ran synchronously in drop, so the pid is gone (reaped).
        let alive = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("run kill -0")
            .success();
        assert!(!alive, "guard must kill the child on drop");
    }

    #[test]
    fn discovery_report_lists_target_language_servers() {
        let report = discover_lsp_servers();
        let ids: Vec<_> = report.servers.iter().map(|server| server.id).collect();

        assert_eq!(report.total_servers, 8);
        assert!(ids.contains(&"rust-analyzer"));
        assert!(ids.contains(&"gopls"));
        assert!(ids.contains(&"typescript-language-server"));
        assert!(ids.contains(&"pyright-langserver"));
        assert!(ids.contains(&"clangd"));
        assert!(ids.contains(&"intelephense"));
        assert!(ids.contains(&"dart-analysis-server"));
        assert!(ids.contains(&"bash-language-server"));
        assert!(report.available_servers <= report.total_servers);
    }

    #[test]
    fn discovery_report_preserves_primary_semantic_server_contracts() {
        let report = discover_lsp_servers();
        let required_capabilities = [
            "definitions",
            "references",
            "document_symbols",
            "workspace_symbols",
            "diagnostics",
        ];

        assert_lsp_server_contract(
            &report,
            "rust-analyzer",
            &["rust"],
            "rust-analyzer",
            &[],
            &required_capabilities,
        );
        assert_lsp_server_contract(
            &report,
            "gopls",
            &["go"],
            "gopls",
            &[],
            &required_capabilities,
        );
        assert_lsp_server_contract(
            &report,
            "typescript-language-server",
            &["javascript", "typescript", "tsx"],
            "typescript-language-server",
            &["--stdio"],
            &required_capabilities,
        );
        assert_lsp_server_contract(
            &report,
            "dart-analysis-server",
            &["dart"],
            "dart",
            &["language-server", "--protocol=lsp"],
            &required_capabilities,
        );
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
    fn semantic_lsp_cache_returns_cached_responses_without_running_lsp() {
        let dir = temp_dir();
        let cache = SemanticLspCache::new(&dir);
        let batch = SemanticExecutionBatch {
            workspace_root: "/workspace".to_string(),
            work_item_limit: 10,
            work_item_filter: SemanticWorkItemFilter::default(),
            total_work_items: 0,
            truncated_work_items: false,
            server_batches: Vec::new(),
            blocked_items: Vec::new(),
        };
        let responses = vec![SemanticLspResponse {
            request_id: "r1".to_string(),
            method: "textDocument/definition".to_string(),
            result: json!({"uri": "file:///workspace/src/main.rs"}),
            error: None,
        }];
        cache.store(&batch, &responses).unwrap();

        let run = run_semantic_execution_batch_cached(
            Some(&cache),
            &batch,
            &SemanticLspRunOptions::default(),
        )
        .unwrap();

        assert_eq!(run.cache.status, SemanticLspCacheStatus::Hit);
        assert_eq!(run.responses, responses);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn semantic_cache_misses_after_touched_file_changes() {
        // The batch shape is graph-derived, so an edit that keeps the graph
        // identical must still invalidate via the file stamp in the hash.
        let workspace = temp_dir();
        fs::create_dir_all(workspace.join("src")).unwrap();
        fs::write(
            workspace.join("src").join("main.rs"),
            "fn main() { let x = 1; }\n",
        )
        .unwrap();
        let dir = temp_dir();
        let cache = SemanticLspCache::new(&dir);
        let batch = SemanticExecutionBatch {
            workspace_root: workspace.display().to_string(),
            work_item_limit: 10,
            work_item_filter: SemanticWorkItemFilter::default(),
            total_work_items: 1,
            truncated_work_items: false,
            server_batches: vec![SemanticServerBatch {
                server: "rust-analyzer".to_string(),
                command: "rust-analyzer".to_string(),
                args: Vec::new(),
                installed: true,
                path: None,
                status: "ready",
                languages: vec!["rust".to_string()],
                work_items: Vec::new(),
                requests: vec![SemanticLspRequest {
                    id: "r1".to_string(),
                    work_item_id: Some("w1".to_string()),
                    request_kind: "request",
                    method: "textDocument/definition",
                    params: json!({}),
                    document_uri: None,
                    path: Some("src/main.rs".to_string()),
                    line: Some(1),
                    column: Some(4),
                    expected_result: "definition",
                }],
            }],
            blocked_items: Vec::new(),
        };
        let responses = vec![SemanticLspResponse {
            request_id: "r1".to_string(),
            method: "textDocument/definition".to_string(),
            result: json!({}),
            error: None,
        }];
        cache.store(&batch, &responses).unwrap();
        assert!(cache.load(&batch).unwrap().is_some(), "warm cache hits");

        // Same batch, edited file (different size → stamp differs even on
        // coarse-mtime filesystems): must miss.
        fs::write(
            workspace.join("src").join("main.rs"),
            "fn main() { let x = 100; }\n",
        )
        .unwrap();
        assert!(
            cache.load(&batch).unwrap().is_none(),
            "an edited file invalidates the semantic cache"
        );

        fs::remove_dir_all(workspace).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lsp_wire_round_trips_content_length_messages() {
        let message = json!({
            "jsonrpc": "2.0",
            "id": "request-1",
            "result": { "ok": true },
        });
        let mut bytes = Vec::new();
        write_lsp_json(&mut bytes, &message).unwrap();

        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("Content-Length: "));
        assert!(text.contains("\r\n\r\n"));

        let mut reader = BufReader::new(Cursor::new(bytes));
        let parsed = read_lsp_message(&mut reader).unwrap().unwrap();
        assert_eq!(parsed, message);
    }

    #[test]
    fn lsp_server_requests_are_acknowledged_with_null_result() {
        let request = json!({
            "jsonrpc": "2.0",
            "id": "server-request-1",
            "method": "client/registerCapability",
            "params": {},
        });
        let mut bytes = Vec::new();
        write_lsp_server_request_response(&mut bytes, &request).unwrap();

        let mut reader = BufReader::new(Cursor::new(bytes));
        let parsed = read_lsp_message(&mut reader).unwrap().unwrap();
        assert_eq!(parsed["id"], "server-request-1");
        assert!(parsed["result"].is_null());
    }

    #[test]
    fn lsp_error_messages_map_to_semantic_responses() {
        let request = SemanticLspRequest {
            id: "lsp:rust-analyzer:definitions:rust:edge:1".to_string(),
            work_item_id: Some("definitions:rust:edge:1".to_string()),
            request_kind: "request",
            method: "textDocument/definition",
            params: json!({}),
            document_uri: None,
            path: None,
            line: None,
            column: None,
            expected_result: "Definition | LocationLink[]",
        };

        let response = semantic_lsp_response_from_message(
            &request,
            &json!({
                "jsonrpc": "2.0",
                "id": "lsp:rust-analyzer:definitions:rust:edge:1",
                "error": { "code": -32601, "message": "not supported" },
            }),
        );

        assert_eq!(response.request_id, request.id);
        assert_eq!(response.method, "textDocument/definition");
        assert!(response.result.is_null());
        assert_eq!(response.error.unwrap().code, -32601);
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
                    capabilities: &[
                        "definitions",
                        "references",
                        "document_symbols",
                        "workspace_symbols",
                    ],
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

        let plan = semantic_enrichment_plan_with_discovery_and_filter(
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter::default(),
        );
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
        assert_eq!(plan.planned_requests.workspace_symbols, 1);
        assert_eq!(plan.planned_requests.definitions, 1);
        assert_eq!(plan.planned_requests.references, 2);
        assert_eq!(plan.planned_requests.diagnostics, 0);
        assert_eq!(plan.total_work_items, 8);
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
        assert!(plan.work_items.iter().any(|item| {
            item.kind == "workspace"
                && item.capability == "workspace_symbols"
                && item.language == "rust"
                && item.id == "workspace_symbols:rust"
                && item.priority == 50
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

        let filtered = semantic_enrichment_plan_with_discovery_and_filter(
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("definitions".to_string()),
            },
        );

        assert_eq!(filtered.total_work_items, 1);
        assert_eq!(filtered.work_items.len(), 1);
        assert_eq!(filtered.work_items[0].id, "definitions:rust:edge:1");
        assert_eq!(
            filtered.work_item_filter,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("definitions".to_string()),
            }
        );

        let minimum_limited = semantic_enrichment_plan_with_discovery_and_filter(
            &graph,
            &discovery,
            0,
            SemanticWorkItemFilter::default(),
        );
        assert_eq!(minimum_limited.work_item_limit, 1);
        assert_eq!(minimum_limited.work_items.len(), 1);
        assert!(minimum_limited.truncated_work_items);

        let maximum_limited = semantic_enrichment_plan_with_discovery_and_filter(
            &graph,
            &discovery,
            usize::MAX,
            SemanticWorkItemFilter::default(),
        );
        assert_eq!(
            maximum_limited.work_item_limit,
            MAX_SEMANTIC_WORK_ITEM_LIMIT
        );
        assert_eq!(maximum_limited.total_work_items, 8);
        assert_eq!(maximum_limited.work_items.len(), 8);

        let batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("definitions".to_string()),
            },
        );

        assert_eq!(batch.workspace_root, "/workspace/repo");
        assert_eq!(batch.total_work_items, 1);
        assert_eq!(batch.blocked_items, Vec::<SemanticWorkItem>::new());
        assert_eq!(batch.server_batches.len(), 1);
        assert_eq!(batch.server_batches[0].server, "rust-analyzer");
        assert_eq!(batch.server_batches[0].command, "rust-analyzer");
        assert!(batch.server_batches[0].installed);
        assert_eq!(batch.server_batches[0].languages, vec!["rust"]);
        assert_eq!(
            batch.server_batches[0].work_items[0].id,
            "definitions:rust:edge:1"
        );
        let requests = &batch.server_batches[0].requests;
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].id, "lsp:rust-analyzer:initialize");
        assert_eq!(requests[0].request_kind, "request");
        assert_eq!(requests[0].method, "initialize");
        assert_eq!(requests[0].params["rootUri"], "file:///workspace/repo");
        assert_eq!(requests[1].request_kind, "notification");
        assert_eq!(requests[1].method, "initialized");
        assert_eq!(requests[2].id, "lsp:rust-analyzer:definitions:rust:edge:1");
        assert_eq!(
            requests[2].work_item_id.as_deref(),
            Some("definitions:rust:edge:1")
        );
        assert_eq!(requests[2].method, "textDocument/definition");
        assert_eq!(
            requests[2].document_uri.as_deref(),
            Some("file:///workspace/repo/src/main.rs")
        );
        assert_eq!(requests[2].params["position"]["line"], 0);
        assert_eq!(requests[2].params["position"]["character"], 0);

        let workspace_batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("workspace_symbols".to_string()),
            },
        );
        assert_eq!(workspace_batch.server_batches[0].requests.len(), 3);
        assert_eq!(
            workspace_batch.server_batches[0].requests[2].method,
            "workspace/symbol"
        );
        assert_eq!(
            workspace_batch.server_batches[0].requests[2]
                .work_item_id
                .as_deref(),
            Some("workspace_symbols:rust")
        );
        assert_eq!(
            workspace_batch.server_batches[0].requests[2].params["query"],
            ""
        );

        let blocked_batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("markdown".to_string()),
                status: Some("unsupported_language".to_string()),
                capability: None,
            },
        );

        assert_eq!(
            blocked_batch.server_batches,
            Vec::<SemanticServerBatch>::new()
        );
        assert_eq!(blocked_batch.blocked_items.len(), 1);
        assert_eq!(
            blocked_batch.blocked_items[0].id,
            "language_support:markdown"
        );
    }

    #[test]
    fn semantic_request_timeout_is_clamped_to_runtime_bounds() {
        assert_eq!(normalize_semantic_request_timeout_ms(0), 1);
        assert_eq!(
            normalize_semantic_request_timeout_ms(DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS),
            DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS
        );
        assert_eq!(
            normalize_semantic_request_timeout_ms(u64::MAX),
            MAX_SEMANTIC_REQUEST_TIMEOUT_MS
        );
        assert_eq!(
            SemanticLspRunOptions::default().request_timeout,
            Duration::from_millis(DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS)
        );
    }

    #[test]
    fn definition_requests_ask_at_the_call_site() {
        let (mut graph, caller, _) = semantic_patch_graph();
        // The call happens deep inside the caller, not at its declaration.
        if let Some(edge) = graph
            .edges
            .iter_mut()
            .find(|edge| edge.kind == EdgeKind::Calls)
        {
            edge.metadata.insert("line".to_string(), "3".to_string());
            edge.metadata.insert("column".to_string(), "17".to_string());
        }
        let discovery = semantic_patch_discovery(&["definitions"]);
        let batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("definitions".to_string()),
            },
        );
        let request = definition_request(&batch);

        assert_eq!(request.line, Some(3), "the request must target the call");
        assert_eq!(request.column, Some(17));
        assert_ne!(
            request.line,
            graph
                .nodes
                .iter()
                .find(|node| node.id == caller)
                .and_then(|node| node.span.as_ref())
                .map(|span| span.start_line),
            "asking at the caller's declaration is what produced self-referential edges"
        );
    }

    #[test]
    fn a_definition_resolving_to_the_asking_node_is_not_an_edge() {
        let (graph, caller, _) = semantic_patch_graph();
        let discovery = semantic_patch_discovery(&["definitions"]);
        let batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("definitions".to_string()),
            },
        );
        let request = definition_request(&batch);

        // The server answers with a position inside the caller itself.
        let patch = semantic_graph_patch_from_responses(
            Path::new("/workspace/repo"),
            &graph,
            &batch,
            &[SemanticLspResponse {
                request_id: request.id.clone(),
                method: request.method.to_string(),
                result: json!({
                    "uri": "file:///workspace/repo/src/main.rs",
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 10 }
                    }
                }),
                error: None,
            }],
        );

        assert!(
            patch.semantic_edges.is_empty(),
            "a node cannot semantically resolve to itself: {:?}",
            patch.semantic_edges
        );
        assert_eq!(
            patch
                .unmatched_locations
                .first()
                .map(|location| location.reason),
            Some("definition_resolves_to_the_asking_node")
        );
        let _ = caller;
    }

    #[test]
    fn a_question_about_a_call_is_about_its_method() {
        // The span of a dotted call starts at the receiver, and asking
        // there answers with the local variable: 29 of ripgrep's first 100
        // answers came back pointing at the caller. `builder` is 8
        // characters wide, so `add` begins 8 further along.
        assert_eq!(method_offset_in(Some(&"builder.add".to_string())), 8);
        assert_eq!(
            method_offset_in(Some(&"GlobSetBuilder::new".to_string())),
            16
        );
        assert_eq!(method_offset_in(Some(&"a.b.c".to_string())), 4);

        // A bare name is already the method.
        assert_eq!(method_offset_in(Some(&"parse".to_string())), 0);
        assert_eq!(method_offset_in(None), 0);

        // The offset counts characters, which is what an LSP position
        // counts, not bytes.
        assert_eq!(method_offset_in(Some(&"Ünicöde.read".to_string())), 8);
    }

    #[test]
    fn a_language_ships_its_own_library_somewhere_it_says() {
        // 595 of ripgrep's first 1000 answers put the definition in rust's
        // own library, and `unwrap` and `clone` were kept out of the
        // hand-written builtin lists precisely because a project may
        // declare them. Here the compiler has said it did not.
        assert!(path_is_in_a_standard_library(
            "/home/u/.rustup/toolchains/stable/lib/rustlib/src/rust/library/core/src/cmp.rs"
        ));
        assert!(path_is_in_a_standard_library(
            "/usr/lib/python3.12/dataclasses.py"
        ));

        // What a project installs beside it is a dependency, not the
        // library, and a path that says neither stays unmatched.
        assert!(!path_is_in_a_standard_library(
            "/venv/lib/python3.12/site-packages/click/core.py"
        ));
        assert!(!path_is_in_a_standard_library(
            "/home/u/.cargo/registry/src/index.crates.io-1/regex-1.10.2/src/lib.rs"
        ));
        assert!(!path_is_in_a_standard_library("/home/u/project/src/lib.rs"));
    }

    #[test]
    fn a_path_inside_a_dependency_names_it() {
        // Every ecosystem keeps its packages in a directory that names
        // them, and 12 of ripgrep's first 100 answers land in one.
        assert_eq!(
            dependency_named_by_path(
                "/home/u/.cargo/registry/src/index.crates.io-6f17d22bba15001f/regex-1.10.2/src/lib.rs"
            )
            .as_deref(),
            Some("regex")
        );
        assert_eq!(
            dependency_named_by_path("/app/node_modules/@babel/parser/lib/index.js").as_deref(),
            Some("@babel/parser")
        );
        assert_eq!(
            dependency_named_by_path("/app/node_modules/lodash/index.js").as_deref(),
            Some("lodash")
        );
        assert_eq!(
            dependency_named_by_path("/venv/lib/python3.12/site-packages/click/core.py").as_deref(),
            Some("click")
        );
        assert_eq!(
            dependency_named_by_path("/app/vendor/guzzlehttp/psr7/src/Uri.php").as_deref(),
            Some("guzzlehttp/psr7")
        );
        assert_eq!(
            dependency_named_by_path("/usr/lib/ruby/gems/3.2.0/gems/rspec-core-3.13.0/lib/x.rb")
                .as_deref(),
            Some("rspec-core")
        );

        // The standard library lives in none of them, and a directory
        // whose last segment is not `name-version` is not a package.
        assert_eq!(
            dependency_named_by_path(
                "/home/u/.rustup/toolchains/stable/lib/rustlib/src/rust/library/core/src/cmp.rs"
            ),
            None
        );
        assert_eq!(dependency_named_by_path("/home/u/project/src/lib.rs"), None);
    }

    #[test]
    fn a_definition_in_a_dependency_is_not_a_gap_in_the_graph() {
        let root = Path::new("/workspace/project");
        assert_eq!(
            missing_node_reason(root, "file:///workspace/project/src/lib.rs"),
            "no_graph_node_at_lsp_location",
            "a file this project owns is one the graph is expected to know"
        );
        assert_eq!(
            missing_node_reason(root, "file:///home/user/.rustup/lib/std/src/time.rs"),
            "location_is_outside_the_project",
            "the standard library is not this project's to have parsed"
        );
    }

    #[test]
    fn work_items_are_spread_over_directories() {
        let candidates = vec![
            ("deps/a.c", 0),
            ("deps/b.c", 1),
            ("deps/c.c", 2),
            ("src/main.c", 3),
            ("modules/mod.c", 4),
        ];
        let spread = spread_over_directories(candidates, |(path, _)| *path);
        // One per directory in sorted order, then the next round: the first
        // directory alphabetically cannot take the whole budget.
        assert_eq!(
            spread.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
            vec![
                "deps/a.c",
                "modules/mod.c",
                "src/main.c",
                "deps/b.c",
                "deps/c.c"
            ]
        );
    }

    #[test]
    fn semantic_graph_patch_maps_lsp_location_definitions_to_graph_edges() {
        let (graph, caller, helper) = semantic_patch_graph();
        let discovery = semantic_patch_discovery(&["definitions"]);
        let batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("definitions".to_string()),
            },
        );
        let request = definition_request(&batch);

        let patch = semantic_graph_patch_from_responses(
            Path::new("/workspace/repo"),
            &graph,
            &batch,
            &[SemanticLspResponse {
                request_id: request.id.clone(),
                method: request.method.to_string(),
                result: json!({
                    "uri": "file:///workspace/repo/src/main.rs",
                    "range": {
                        "start": { "line": 9, "character": 4 },
                        "end": { "line": 9, "character": 10 }
                    }
                }),
                error: None,
            }],
        );

        assert_eq!(patch.workspace_root, "/workspace/repo");
        assert_eq!(patch.responses, 1);
        assert_eq!(patch.matched_responses, 1);
        assert!(patch.unmatched_locations.is_empty());
        assert!(patch.response_errors.is_empty());
        assert_eq!(patch.semantic_edges.len(), 1);
        assert_eq!(patch.semantic_edges[0].source, caller);
        assert_eq!(patch.semantic_edges[0].target, helper);
        assert_eq!(patch.semantic_edges[0].kind, EdgeKind::Calls);
        assert_eq!(patch.semantic_edges[0].confidence, Confidence::Semantic);
        assert_eq!(patch.semantic_edges[0].path, "src/main.rs");
        assert_eq!(patch.semantic_edges[0].line, 10);
        assert_eq!(patch.semantic_edges[0].column, 5);
        assert_eq!(patch.semantic_edges[0].evidence, "lsp_definition");
    }

    #[test]
    fn semantic_graph_patch_maps_lsp_location_links_to_graph_edges() {
        let (graph, caller, helper) = semantic_patch_graph();
        let discovery = semantic_patch_discovery(&["definitions"]);
        let batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("definitions".to_string()),
            },
        );
        let request = definition_request(&batch);

        let patch = semantic_graph_patch_from_responses(
            Path::new("/workspace/repo"),
            &graph,
            &batch,
            &[SemanticLspResponse {
                request_id: request.id.clone(),
                method: request.method.to_string(),
                result: json!([
                    {
                        "targetUri": "file:///workspace/repo/src/main.rs",
                        "targetSelectionRange": {
                            "start": { "line": 9, "character": 4 },
                            "end": { "line": 9, "character": 10 }
                        },
                        "targetRange": {
                            "start": { "line": 8, "character": 0 },
                            "end": { "line": 11, "character": 1 }
                        }
                    }
                ]),
                error: None,
            }],
        );

        assert_eq!(patch.semantic_edges.len(), 1);
        assert_eq!(patch.semantic_edges[0].source, caller);
        assert_eq!(patch.semantic_edges[0].target, helper);
        assert_eq!(patch.semantic_edges[0].replaced_edge_index, Some(1));
        assert_eq!(patch.semantic_edges[0].original_target, Some(helper));
    }

    #[test]
    fn semantic_graph_patch_collects_lsp_diagnostics() {
        let (graph, _, _) = semantic_patch_graph();
        let discovery = semantic_patch_discovery(&["diagnostics"]);
        let batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("rust".to_string()),
                status: Some("ready".to_string()),
                capability: Some("diagnostics".to_string()),
            },
        );
        let request = batch.server_batches[0]
            .requests
            .iter()
            .find(|request| request.method == "textDocument/diagnostic")
            .expect("diagnostic request");

        let patch = semantic_graph_patch_from_responses(
            Path::new("/workspace/repo"),
            &graph,
            &batch,
            &[SemanticLspResponse {
                request_id: request.id.clone(),
                method: request.method.to_string(),
                result: json!({
                    "kind": "full",
                    "items": [
                        {
                            "range": {
                                "start": { "line": 2, "character": 8 },
                                "end": { "line": 2, "character": 14 }
                            },
                            "severity": 1,
                            "code": "E0001",
                            "source": "rustc",
                            "message": "semantic mismatch"
                        }
                    ]
                }),
                error: None,
            }],
        );

        assert!(patch.semantic_edges.is_empty());
        assert_eq!(patch.diagnostics.len(), 1);
        assert_eq!(patch.diagnostics[0].path, "src/main.rs");
        assert_eq!(patch.diagnostics[0].line, 3);
        assert_eq!(patch.diagnostics[0].column, 9);
        assert_eq!(patch.diagnostics[0].severity, Some(1));
        assert_eq!(patch.diagnostics[0].code.as_deref(), Some("E0001"));
        assert_eq!(patch.diagnostics[0].source.as_deref(), Some("rustc"));
        assert_eq!(patch.diagnostics[0].message, "semantic mismatch");
    }

    #[test]
    fn apply_semantic_graph_patch_replaces_edges_and_adds_diagnostics() {
        let (graph, caller, helper) = semantic_patch_graph();
        let patch = SemanticGraphPatch {
            workspace_root: "/workspace/repo".to_string(),
            responses: 2,
            matched_responses: 2,
            semantic_edges: vec![SemanticEdgePatch {
                request_id: "lsp:rust-analyzer:definitions:rust:edge:1".to_string(),
                work_item_id: "definitions:rust:edge:1".to_string(),
                source: caller,
                target: helper,
                kind: EdgeKind::Calls,
                confidence: Confidence::Semantic,
                replaced_edge_index: Some(1),
                original_target: Some(helper),
                path: "src/main.rs".to_string(),
                line: 10,
                column: 5,
                evidence: "lsp_definition",
                call_label: Some("helper".to_string()),
                language: Some("rust".to_string()),
                call_file: Some("src/caller.rs".to_string()),
                call_line: Some(42),
                call_column: Some(9),
            }],
            diagnostics: vec![SemanticDiagnosticPatch {
                request_id: "lsp:rust-analyzer:diagnostics:rust:node:2".to_string(),
                work_item_id: "diagnostics:rust:node:2".to_string(),
                path: "src/main.rs".to_string(),
                line: 3,
                column: 9,
                severity: Some(1),
                code: Some("E0001".to_string()),
                source: Some("rustc".to_string()),
                message: "semantic mismatch".to_string(),
            }],
            unmatched_locations: Vec::new(),
            response_errors: Vec::new(),
        };

        let result = apply_semantic_graph_patch(&graph, &patch);

        assert_eq!(result.report.semantic_edges, 1);
        assert_eq!(result.report.replaced_edges, 1);
        assert_eq!(result.report.added_edges, 0);
        assert_eq!(result.report.diagnostic_nodes, 1);
        assert_eq!(result.report.diagnostic_edges, 1);
        assert_eq!(result.graph.edges[1].source, caller);
        assert_eq!(result.graph.edges[1].target, helper);
        assert_eq!(result.graph.edges[1].confidence, Confidence::Semantic);
        // A semantic edge is a resolved call and carries what every other
        // call edge carries. Without this, applying the pass to ripgrep
        // took 337 calls out of every category at once and made the
        // reported resolution rate worse rather than better.
        let applied = &result.graph.edges[1].metadata;
        assert_eq!(
            applied.get("resolution").map(String::as_str),
            Some("resolved")
        );
        assert_eq!(
            applied.get("resolution_basis").map(String::as_str),
            Some("semantic")
        );
        assert_eq!(
            applied.get("call_label").map(String::as_str),
            Some("helper")
        );
        assert_eq!(applied.get("language").map(String::as_str), Some("rust"));
        // Where the call is written, the way every other call edge says it.
        // The answer's own location went here before, and for a definition
        // in the standard library that is an absolute path on whichever
        // machine ran the pass: all 919 of ripgrep's spans were one.
        assert_eq!(
            applied.get("file").map(String::as_str),
            Some("src/caller.rs")
        );
        assert_eq!(applied.get("line").map(String::as_str), Some("42"));
        assert_eq!(applied.get("column").map(String::as_str), Some("9"));
        // The definition's own place, kept because this one is inside the
        // project.
        assert_eq!(
            applied.get("definition_file").map(String::as_str),
            Some("src/main.rs")
        );
        assert_eq!(
            applied.get("definition_line").map(String::as_str),
            Some("10")
        );
        assert_eq!(
            result.graph.edges[1]
                .metadata
                .get("evidence")
                .map(String::as_str),
            Some("lsp_definition")
        );
        let diagnostic_node = result.graph.nodes.last().expect("diagnostic node");
        assert_eq!(diagnostic_node.kind, NodeKind::Unknown);
        assert_eq!(
            diagnostic_node
                .metadata
                .get("item_kind")
                .map(String::as_str),
            Some("diagnostic")
        );
        assert_eq!(
            diagnostic_node.metadata.get("severity").map(String::as_str),
            Some("error")
        );
        let diagnostic_edge = result.graph.edges.last().expect("diagnostic edge");
        assert_eq!(diagnostic_edge.source, caller);
        assert_eq!(diagnostic_edge.target, diagnostic_node.id);
        assert_eq!(diagnostic_edge.kind, EdgeKind::MayError);
        assert_eq!(diagnostic_edge.confidence, Confidence::Semantic);
        assert_eq!(
            diagnostic_edge.metadata.get("relation").map(String::as_str),
            Some("diagnostic")
        );
    }

    #[test]
    fn dart_analysis_server_semantic_patches_validate_end_to_end() {
        // A Dart graph with a heuristic call edge, as the scanner produces it.
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "lib/main.dart",
            None,
            BTreeMap::from([("language".to_string(), "dart".to_string())]),
        );
        let caller = graph.add_node_with_metadata(
            NodeKind::Function,
            "caller",
            Some(SourceSpan {
                path: "lib/main.dart".to_string(),
                start_line: 2,
                start_column: 1,
                end_line: 4,
                end_column: 1,
            }),
            BTreeMap::from([("language".to_string(), "dart".to_string())]),
        );
        let helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper",
            Some(SourceSpan {
                path: "lib/main.dart".to_string(),
                start_line: 9,
                start_column: 1,
                end_line: 12,
                end_column: 1,
            }),
            BTreeMap::from([("language".to_string(), "dart".to_string())]),
        );
        graph.add_edge(file, caller, EdgeKind::Defines, Confidence::Syntactic);
        graph.add_edge(caller, helper, EdgeKind::Calls, Confidence::Heuristic);

        // Discovery advertises the real dart-analysis-server contract.
        let discovery = LspDiscoveryReport {
            total_servers: 1,
            available_servers: 1,
            servers: vec![LspServerStatus {
                id: "dart-analysis-server",
                languages: &["dart"],
                command: "dart",
                args: &["language-server", "--protocol=lsp"],
                capabilities: &["definitions", "diagnostics"],
                installed: true,
                path: Some("/sdk/bin/dart".to_string()),
            }],
        };
        let batch = semantic_execution_batch_with_discovery(
            Path::new("/workspace/repo"),
            &graph,
            &discovery,
            DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            SemanticWorkItemFilter {
                language: Some("dart".to_string()),
                status: Some("ready".to_string()),
                capability: None,
            },
        );
        let definition = definition_request(&batch);
        let diagnostic = batch.server_batches[0]
            .requests
            .iter()
            .find(|request| request.method == "textDocument/diagnostic")
            .expect("diagnostic request for dart file");

        // Dart analysis server responses: a definition Location and a full
        // document diagnostic report, both against dart file URIs.
        let patch = semantic_graph_patch_from_responses(
            Path::new("/workspace/repo"),
            &graph,
            &batch,
            &[
                SemanticLspResponse {
                    request_id: definition.id.clone(),
                    method: definition.method.to_string(),
                    result: json!({
                        "uri": "file:///workspace/repo/lib/main.dart",
                        "range": {
                            "start": { "line": 9, "character": 4 },
                            "end": { "line": 9, "character": 10 }
                        }
                    }),
                    error: None,
                },
                SemanticLspResponse {
                    request_id: diagnostic.id.clone(),
                    method: diagnostic.method.to_string(),
                    result: json!({
                        "kind": "full",
                        "items": [
                            {
                                "range": {
                                    "start": { "line": 2, "character": 2 },
                                    "end": { "line": 2, "character": 8 }
                                },
                                "severity": 2,
                                "code": "unused_element",
                                "source": "dart",
                                "message": "The declaration isn't referenced."
                            }
                        ]
                    }),
                    error: None,
                },
            ],
        );

        assert_eq!(patch.responses, 2);
        assert_eq!(patch.matched_responses, 2);
        assert!(patch.response_errors.is_empty());
        assert_eq!(patch.semantic_edges.len(), 1);
        assert_eq!(patch.semantic_edges[0].source, caller);
        assert_eq!(patch.semantic_edges[0].target, helper);
        assert_eq!(patch.semantic_edges[0].confidence, Confidence::Semantic);
        assert_eq!(patch.semantic_edges[0].path, "lib/main.dart");
        assert_eq!(patch.diagnostics.len(), 1);
        assert_eq!(patch.diagnostics[0].source.as_deref(), Some("dart"));
        assert_eq!(patch.diagnostics[0].code.as_deref(), Some("unused_element"));

        // Applying the patch upgrades the heuristic call to semantic and
        // records the dart diagnostic in the enriched graph.
        let result = apply_semantic_graph_patch(&graph, &patch);
        assert_eq!(result.report.semantic_edges, 1);
        assert_eq!(result.report.replaced_edges, 1);
        assert_eq!(result.report.diagnostic_nodes, 1);
        assert_eq!(result.graph.edges[1].source, caller);
        assert_eq!(result.graph.edges[1].target, helper);
        assert_eq!(result.graph.edges[1].confidence, Confidence::Semantic);
    }

    fn semantic_patch_graph() -> (CodeGraph, NodeId, NodeId) {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            None,
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let caller = graph.add_node_with_metadata(
            NodeKind::Function,
            "caller",
            Some(SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 2,
                start_column: 1,
                end_line: 4,
                end_column: 1,
            }),
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        let helper = graph.add_node_with_metadata(
            NodeKind::Function,
            "helper",
            Some(SourceSpan {
                path: "src/main.rs".to_string(),
                start_line: 9,
                start_column: 1,
                end_line: 12,
                end_column: 1,
            }),
            BTreeMap::from([("language".to_string(), "rust".to_string())]),
        );
        graph.add_edge(file, caller, EdgeKind::Defines, Confidence::Syntactic);
        graph.add_edge(caller, helper, EdgeKind::Calls, Confidence::Heuristic);
        (graph, caller, helper)
    }

    fn assert_lsp_server_contract(
        report: &LspDiscoveryReport,
        id: &str,
        languages: &[&str],
        command: &str,
        args: &[&str],
        capabilities: &[&str],
    ) {
        let server = report
            .servers
            .iter()
            .find(|server| server.id == id)
            .unwrap_or_else(|| panic!("missing LSP server `{id}`"));

        assert_eq!(server.languages, languages);
        assert_eq!(server.command, command);
        assert_eq!(server.args, args);
        for capability in capabilities {
            assert!(
                server.capabilities.contains(capability),
                "server `{id}` should expose `{capability}`"
            );
        }
    }

    fn semantic_patch_discovery(capabilities: &'static [&'static str]) -> LspDiscoveryReport {
        LspDiscoveryReport {
            total_servers: 1,
            available_servers: 1,
            servers: vec![LspServerStatus {
                id: "rust-analyzer",
                languages: &["rust"],
                command: "rust-analyzer",
                args: &[],
                capabilities,
                installed: true,
                path: Some("/bin/rust-analyzer".to_string()),
            }],
        }
    }

    fn definition_request(batch: &SemanticExecutionBatch) -> &SemanticLspRequest {
        batch.server_batches[0]
            .requests
            .iter()
            .find(|request| request.method == "textDocument/definition")
            .expect("definition request")
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

/// Settings for the automatic semantic pass that runs after a scan when the
/// matching language servers happen to be installed.
#[derive(Debug, Clone)]
pub struct AutoEnrichmentOptions {
    pub enabled: bool,
    pub work_item_limit: usize,
    pub request_timeout: Duration,
}

impl Default for AutoEnrichmentOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            work_item_limit: DEFAULT_SEMANTIC_WORK_ITEM_LIMIT,
            request_timeout: Duration::from_millis(DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS),
        }
    }
}

/// What the automatic pass actually did, so callers (and the graph itself) can
/// report whether a result is syntax-only or semantically enriched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutoEnrichmentReport {
    pub applied: bool,
    pub servers: Vec<String>,
    pub semantic_edges: usize,
    pub replaced_edges: usize,
    /// Work items the pass actually asked about, and how many it could have
    /// asked about. The automatic pass is bounded so a scan stays fast, so
    /// these two numbers are usually far apart — an enriched graph is a
    /// sampled graph, and it should say so.
    pub requested_work_items: usize,
    pub total_work_items: usize,
    pub skipped_reason: Option<String>,
}

impl AutoEnrichmentReport {
    fn skipped(reason: &str) -> Self {
        Self {
            applied: false,
            servers: Vec::new(),
            semantic_edges: 0,
            replaced_edges: 0,
            requested_work_items: 0,
            total_work_items: 0,
            skipped_reason: Some(reason.to_string()),
        }
    }
}

/// Languages that actually appear in the graph, by node metadata.
fn graph_languages(graph: &CodeGraph) -> BTreeSet<String> {
    graph
        .nodes
        .iter()
        .filter_map(|node| node.metadata.get("language").cloned())
        .collect()
}

/// Enrich a freshly scanned graph with language-server facts when servers for
/// its languages are installed, and record on the root node what happened.
///
/// This deliberately trades reproducibility for accuracy: the same sources can
/// produce different graphs on machines with different servers installed, so
/// the result is always labeled (`semantic_enrichment` on the root node) and
/// callers that need a reproducible graph — CI gates above all — must disable
/// it. Server responses are cached, so only the first run pays for the server.
/// Run the automatic semantic pass and stamp the outcome onto the graph.
///
/// Whether a graph is syntax-only matters: the pass depends on which language
/// servers the machine has, so two scans of the same commit can legitimately
/// differ. A graph that stayed syntactic said nothing about why — no server,
/// a failing server, and an explicit `--no-semantic` all looked alike — so the
/// artifact could not answer the first question asked of it. Now every outcome
/// is recorded on the root node.
pub fn auto_enrich_graph(
    root: &Path,
    graph: CodeGraph,
    cache: Option<&SemanticLspCache>,
    options: &AutoEnrichmentOptions,
) -> (CodeGraph, AutoEnrichmentReport) {
    let (mut graph, report) = run_auto_enrichment(root, graph, cache, options);
    let root_id = graph.root;
    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == root_id) {
        if report.applied {
            node.metadata
                .insert("semantic_enrichment".to_string(), "applied".to_string());
            node.metadata
                .insert("semantic_servers".to_string(), report.servers.join(","));
            // The pass samples; say how much of the possible work it did so a
            // sampled graph is not mistaken for a fully resolved one.
            node.metadata.insert(
                "semantic_work_items".to_string(),
                format!(
                    "{}/{}",
                    report.requested_work_items, report.total_work_items
                ),
            );
        } else {
            node.metadata
                .insert("semantic_enrichment".to_string(), "skipped".to_string());
            if let Some(reason) = report.skipped_reason.as_deref() {
                node.metadata
                    .insert("semantic_skip_reason".to_string(), reason.to_string());
            }
        }
    }
    (graph, report)
}

fn run_auto_enrichment(
    root: &Path,
    graph: CodeGraph,
    cache: Option<&SemanticLspCache>,
    options: &AutoEnrichmentOptions,
) -> (CodeGraph, AutoEnrichmentReport) {
    if !options.enabled {
        return (graph, AutoEnrichmentReport::skipped("disabled"));
    }

    let languages = graph_languages(&graph);
    if languages.is_empty() {
        return (
            graph,
            AutoEnrichmentReport::skipped("no languages in graph"),
        );
    }

    let discovery = discover_lsp_servers();
    let servers: Vec<String> = discovery
        .servers
        .iter()
        .filter(|server| {
            server.installed
                && server
                    .languages
                    .iter()
                    .any(|language| languages.contains(*language))
        })
        .map(|server| server.id.to_string())
        .collect();
    if servers.is_empty() {
        return (
            graph,
            AutoEnrichmentReport::skipped("no language server installed for these languages"),
        );
    }

    let batch = semantic_execution_batch(
        root,
        &graph,
        options.work_item_limit,
        SemanticWorkItemFilter::default(),
    );
    let run = match run_semantic_execution_batch_cached(
        cache,
        &batch,
        &SemanticLspRunOptions {
            request_timeout: options.request_timeout,
            settle_timeout: Duration::from_millis(DEFAULT_SEMANTIC_SETTLE_TIMEOUT_MS),
        },
    ) {
        Ok(run) => run,
        // A failing server must never fail the scan: fall back to the
        // syntactic graph and say why.
        Err(error) => {
            return (graph, AutoEnrichmentReport::skipped(&error.to_string()));
        }
    };

    let patch = semantic_graph_patch_from_responses(root, &graph, &batch, &run.responses);
    let applied = apply_semantic_graph_patch(&graph, &patch);
    let enriched = applied.graph;
    let requested_work_items = batch
        .server_batches
        .iter()
        .map(|server_batch| server_batch.work_items.len())
        .sum();
    let report = AutoEnrichmentReport {
        applied: true,
        servers: servers.clone(),
        semantic_edges: applied.report.added_edges,
        replaced_edges: applied.report.replaced_edges,
        requested_work_items,
        total_work_items: batch.total_work_items,
        skipped_reason: None,
    };
    (enriched, report)
}

#[cfg(test)]
mod auto_enrichment_tests {
    use super::*;
    use codegraph_core::NodeKind;

    /// The graph minus the provenance stamp, so a test can assert that the
    /// facts are untouched while the stamp itself is checked separately.
    fn without_provenance(graph: &CodeGraph) -> CodeGraph {
        let mut graph = graph.clone();
        let root_id = graph.root;
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == root_id) {
            node.metadata.remove("semantic_enrichment");
            node.metadata.remove("semantic_skip_reason");
            node.metadata.remove("semantic_servers");
        }
        graph
    }

    fn skip_stamp(graph: &CodeGraph) -> (Option<String>, Option<String>) {
        let root = graph
            .nodes
            .iter()
            .find(|node| node.id == graph.root)
            .expect("missing root");
        (
            root.metadata.get("semantic_enrichment").cloned(),
            root.metadata.get("semantic_skip_reason").cloned(),
        )
    }

    #[test]
    fn a_server_says_when_it_has_finished_loading() {
        // rust-analyzer answers `textDocument/definition` with `[]` and
        // `references` with `-32801 content modified` until it has built the
        // crate graph, so a run of 297 requests came back 297 times empty.
        // The `end` is what says the wait is over.
        assert!(progress_ended(&json!({
            "method": "$/progress",
            "params": {"token": "rustAnalyzer/Indexing", "value": {"kind": "end"}}
        })));
        assert!(!progress_ended(&json!({
            "method": "$/progress",
            "params": {"token": "rustAnalyzer/Indexing", "value": {"kind": "begin"}}
        })));
        assert!(!progress_ended(&json!({
            "method": "$/progress",
            "params": {"value": {"kind": "report", "percentage": 40}}
        })));
        // Anything that is not progress says nothing about readiness.
        assert!(!progress_ended(&json!({"method": "window/logMessage"})));
        assert!(!progress_ended(&json!({"id": 3, "result": []})));
    }

    #[test]
    fn disabled_enrichment_returns_the_graph_untouched() {
        let mut graph = CodeGraph::new("repo");
        graph.add_node(NodeKind::Function, "main");
        let before = graph.clone();
        let (after, report) = auto_enrich_graph(
            Path::new("."),
            graph,
            None,
            &AutoEnrichmentOptions {
                enabled: false,
                ..AutoEnrichmentOptions::default()
            },
        );
        assert_eq!(
            without_provenance(&after),
            before,
            "a disabled pass must not touch the facts"
        );
        assert!(!report.applied);
        assert_eq!(report.skipped_reason.as_deref(), Some("disabled"));
        // The graph says for itself that it is syntax-only, and why.
        assert_eq!(
            skip_stamp(&after),
            (Some("skipped".to_string()), Some("disabled".to_string()))
        );
    }

    #[test]
    fn a_graph_without_languages_is_skipped_not_enriched() {
        // Nodes carry no language metadata, so no server could apply.
        let mut graph = CodeGraph::new("repo");
        graph.add_node(NodeKind::File, "notes.txt");
        let before = graph.clone();
        let (after, report) = auto_enrich_graph(
            Path::new("."),
            graph,
            None,
            &AutoEnrichmentOptions::default(),
        );
        assert_eq!(without_provenance(&after), before);
        assert!(!report.applied);
        assert_eq!(
            report.skipped_reason.as_deref(),
            Some("no languages in graph")
        );
        assert_eq!(
            skip_stamp(&after),
            (
                Some("skipped".to_string()),
                Some("no languages in graph".to_string())
            )
        );
    }

    #[test]
    fn enrichment_is_skipped_when_no_server_matches_the_languages() {
        // A language no bundled server covers can never be enriched, whatever
        // is installed on the machine — so this stays deterministic in CI.
        let mut graph = CodeGraph::new("repo");
        let id = graph.add_node(NodeKind::Function, "handler");
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == id) {
            node.metadata
                .insert("language".to_string(), "cobol".to_string());
        }
        let before = graph.clone();
        let (after, report) = auto_enrich_graph(
            Path::new("."),
            graph,
            None,
            &AutoEnrichmentOptions::default(),
        );
        assert_eq!(without_provenance(&after), before);
        assert!(!report.applied);
        assert!(
            skip_stamp(&after)
                .1
                .is_some_and(|reason| reason.contains("no language server")),
            "the graph must record why it stayed syntactic"
        );
        assert!(
            report
                .skipped_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("no language server")),
            "unexpected reason: {:?}",
            report.skipped_reason
        );
    }
}
