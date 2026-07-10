//! The `codegraph` command-line interface: scan, query, trace, workflow,
//! journey, refactoring, insight, report, export, cache, registry, watch,
//! memory, and MCP commands over the shared analysis crates.

mod bench_context;
mod hooks;
mod install;
mod mcp;
use codegraph_analysis::memory;
mod merge;
use codegraph_analysis::pr_impact;
mod query_log;
mod registry;
mod watch;
mod wiki;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use codegraph_analysis::{
    ComponentContractRequest, ComponentDependencyRequest, ConfigTraceRequest,
    DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT, DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
    DEFAULT_REPORT_COMMUNITY_LIMIT, DEFAULT_REPORT_FILE_SUMMARY_LIMIT,
    DEFAULT_REPORT_HOTSPOT_LIMIT, DEFAULT_REPORT_INSIGHT_LIMIT, DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
    DEFAULT_REPORT_NODE_SUMMARY_LIMIT, EntrypointTraceRequest, EntrypointWorkflowRequest,
    ErrorTraceRequest, ExplainEdgeRequest, ImpactRequest, InsightFilter, InsightSeverity,
    JourneyRequest, NaturalQueryRequest, NodeCardRequest, ProjectReport, ProjectReportLimits,
    ProjectReportMarkdownOptions, RefactorContextRequest, SeamRequest, SourceSearchRequest,
    TraceRequest, TraceStart, WorkflowFilters, WorkflowQueryRequest, WorkflowRequest,
    architecture_map, check_insights, communities, compact_query_result, component_contract,
    component_dependencies, entrypoints, explain_edge, filter_insight_report, hotspots, impact,
    insights, journey, language_dependencies, natural_query, project_report,
    project_report_markdown, query_graph, read_source_preview, refactor_context, seams,
    search_source, summarize, surprising_links, trace, trace_config, trace_dependents,
    trace_entrypoints, trace_errors, workflow, workflow_entrypoints, workflow_mermaid,
    workflow_query,
};
use codegraph_analysis::{
    DEFAULT_MERMAID_EDGE_LIMIT, DEFAULT_MERMAID_NODE_LIMIT, MermaidSection, export_cypher,
    export_dot, export_falkordb, export_graph_mermaid_html, export_graphml, export_mermaid_html,
    export_ndjson, node_card,
};
use codegraph_core::NodeId;
use codegraph_indexer::{
    IndexOptionOverrides, configured_index_options, scan_coverage, scan_project,
};
use codegraph_lsp::{
    DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS, DEFAULT_SEMANTIC_WORK_ITEM_LIMIT, SemanticLspCache,
    SemanticLspResponse, SemanticLspRunOptions, SemanticWorkItemFilter, apply_semantic_graph_patch,
    discover_lsp_servers, normalize_semantic_request_timeout_ms,
    normalize_semantic_work_item_limit, run_semantic_execution_batch_cached,
    semantic_enrichment_plan_with_filter, semantic_execution_batch,
    semantic_graph_patch_from_responses, semantic_readiness,
};
use codegraph_parser::language_adapters;
use codegraph_storage::{GraphCache, default_cache_dir, scan_project_cached};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Parser)]
#[command(name = "codegraph")]
#[command(about = "Build and inspect code knowledge graphs")]
struct Cli {
    /// Maximum bytes to read from any single file during scans.
    #[arg(long, global = true)]
    max_file_size: Option<u64>,

    /// Log query/ask/journey commands to .codegraph/query-log.jsonl for this run,
    /// even when [query_log] is not enabled in .codegraph/config.toml.
    #[arg(long, global = true)]
    log_queries: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List built-in language adapters and detection patterns as JSON.
    Languages,

    /// Report available semantic language servers for LSP enrichment.
    Lsp,

    /// Report project language coverage by available semantic language servers.
    SemanticReadiness(ScanArgs),

    /// Plan semantic LSP enrichment work for the scanned graph.
    SemanticPlan(SemanticPlanArgs),

    /// Group semantic LSP work into executable server batches.
    SemanticBatch(SemanticPlanArgs),

    /// Execute ready semantic LSP batches and emit response JSON for semantic-patch/apply.
    SemanticRun(SemanticRunArgs),

    /// Convert semantic LSP responses into a graph patch.
    SemanticPatch(SemanticPatchArgs),

    /// Apply semantic LSP responses and emit an enriched graph plus report.
    SemanticApply(SemanticPatchArgs),

    /// Scan a project and emit the initial graph as JSON.
    #[command(
        after_help = "Examples:\n  codegraph scan .\n  codegraph scan . --format dot > graph.dot\n  codegraph scan ../other-repo --include-hidden"
    )]
    Scan {
        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Emit graph summary counts as JSON.
    Summary(ScanArgs),

    /// Emit a production-oriented project report snapshot as JSON or Markdown.
    Report(ReportArgs),

    /// Emit a top-level architecture map grouped by project area.
    Architecture(ArchitectureArgs),

    /// Emit language-to-language dependency links as JSON.
    LanguageDependencies(LanguageDependencyArgs),

    /// Emit ranked surprising dependency links as JSON.
    SurprisingLinks(SurprisingLinkArgs),

    /// Emit high-degree graph hotspots as JSON.
    Hotspots(HotspotArgs),

    /// Emit graph communities/subsystems as JSON.
    Communities(CommunityArgs),

    /// Explain scan coverage, ignored paths, and file-size skips as JSON.
    Coverage(CoverageArgs),

    /// Benchmark project scans and emit timing plus graph size metrics as JSON.
    #[command(visible_alias = "bench")]
    Benchmark(BenchmarkArgs),

    /// Explain graph cache fingerprint changes without scanning the full graph.
    CacheDiff(CacheDiffArgs),

    /// List persistent per-file graph chunks from the graph cache.
    CacheChunks(CacheDiffArgs),

    /// Plan incremental scan work from the persistent graph cache fingerprint.
    IncrementalPlan(CacheDiffArgs),

    /// Scan only the changed current files described by the incremental cache plan.
    IncrementalScan(CacheDiffArgs),

    /// Preview a graph assembled from cached unchanged files plus changed-file rescans.
    IncrementalMergePreview(IncrementalOutputArgs),

    /// Update the persistent graph cache when the incremental result is complete.
    IncrementalUpdate(IncrementalOutputArgs),

    /// Register a repository in the global graph registry.
    RegistryAdd {
        /// Repository root to register.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Short project name; defaults to the directory name.
        #[arg(long)]
        name: Option<String>,

        /// Registry file override; defaults to <cache-dir>/registry.json.
        #[arg(long)]
        registry_path: Option<PathBuf>,
    },

    /// List repositories in the global graph registry.
    RegistryList {
        /// Registry file override; defaults to <cache-dir>/registry.json.
        #[arg(long)]
        registry_path: Option<PathBuf>,
    },

    /// Remove a repository from the global graph registry by name.
    RegistryRemove {
        /// Registered project name to remove.
        name: String,

        /// Registry file override; defaults to <cache-dir>/registry.json.
        #[arg(long)]
        registry_path: Option<PathBuf>,
    },

    /// Run one graph query expression across all (or selected) registered repositories.
    RegistryQuery {
        /// Query expression, for example: nodes kind:function or path from:main to:load_config.
        expression: String,

        /// Restrict the run to these registered project names (repeatable).
        #[arg(long = "project")]
        projects: Vec<String>,

        /// Collapse repeated low-signal nodes in each query result.
        #[arg(long)]
        compact: bool,

        /// Registry file override; defaults to <cache-dir>/registry.json.
        #[arg(long)]
        registry_path: Option<PathBuf>,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Merge graph JSON artifacts and/or registered projects into one graph with source provenance.
    Merge {
        /// Graph JSON files to merge (as produced by scan/export).
        inputs: Vec<PathBuf>,

        /// Also merge registered projects by name, scanned through the cache (repeatable).
        #[arg(long = "project")]
        projects: Vec<String>,

        /// Write the merged graph JSON here and print the merge report to stdout;
        /// without --output the merged graph itself goes to stdout.
        #[arg(long)]
        output: Option<PathBuf>,

        /// Root label for the merged graph.
        #[arg(long, default_value = "merged")]
        label: String,

        /// Registry file override; defaults to <cache-dir>/registry.json.
        #[arg(long)]
        registry_path: Option<PathBuf>,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Benchmark token/context savings and graph-query recall against text-scan oracles.
    BenchContext {
        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum sampled symbols/config keys per savings task.
        #[arg(long, default_value_t = 20)]
        samples: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// PR impact dashboard: map changed files onto communities, hotspots, blast radius, and risks.
    #[command(
        after_help = "Examples:\n  codegraph pr-impact . --base origin/main --ci-state passing\n  codegraph pr-impact . --file src/util.rs --file src/main.rs"
    )]
    PrImpact {
        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Git base ref to diff against for the changed-file list (default: working tree changes).
        #[arg(long, default_value = "HEAD")]
        base: String,

        /// Explicit changed file overrides (repeatable); skips git entirely.
        #[arg(long = "file")]
        files: Vec<String>,

        /// CI state string recorded verbatim in the report, for example: passing or pending.
        #[arg(long)]
        ci_state: Option<String>,

        /// Review state string recorded verbatim in the report, for example: approved.
        #[arg(long)]
        review_state: Option<String>,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Export an Obsidian-compatible Markdown wiki: communities, entrypoints, hotspots, config flows, and risks.
    ExportWiki {
        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output directory for the vault (created if missing, pages overwritten).
        #[arg(long, default_value = "codegraph-wiki")]
        output: PathBuf,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Install git post-commit/post-checkout hooks that refresh the graph cache automatically.
    InstallHooks {
        /// Repository root containing .git.
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Run the git-hook action: incremental refresh plus optional exports (used by installed hooks).
    HookRun {
        /// Hook kind recorded in the result, for example post-commit or post-checkout.
        kind: String,

        /// Project root to refresh.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        /// Maximum changed files listed per refresh plan.
        #[arg(long, default_value_t = 100)]
        limit: usize,

        /// Directory for persistent graph cache records.
        #[arg(long)]
        cache_dir: Option<PathBuf>,
    },

    /// Watch the project and refresh the graph cache automatically on changes, emitting NDJSON events.
    Watch {
        #[command(flatten)]
        args: CacheDiffArgs,

        /// Poll interval in milliseconds between project fingerprint checks.
        #[arg(long, default_value_t = 2000)]
        interval_ms: u64,

        /// Exit after this many refresh events; 0 keeps watching until interrupted.
        #[arg(long, default_value_t = 0)]
        max_refreshes: usize,
    },

    /// Emit entrypoint candidate nodes as JSON.
    Entrypoints(ScanArgs),

    /// Emit investigation insights such as unresolved calls and error flows.
    Insights(InsightArgs),

    /// Run insight checks and exit non-zero when findings meet a severity threshold.
    Check(CheckArgs),

    /// Query focused graph slices as JSON.
    #[command(
        after_help = "Examples:\n  codegraph query 'nodes kind:function label:main' .\n  codegraph query 'path from:main to:load_config depth:6' .\n  codegraph query 'docs owner:platform-team' .\n  codegraph query 'edges confidence:heuristic limit:20' ."
    )]
    Query {
        /// Query expression, for example: nodes kind:function label:main or path from:main to:init.
        expression: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        /// Collapse repeated low-signal nodes in the query result.
        #[arg(long)]
        compact: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Emit a step-numbered execution journey between two graph labels or node ids.
    #[command(
        after_help = "Examples:\n  codegraph journey --from main --to load_config .\n  codegraph journey --from 'cargo bin:api' --to n42 . --depth 8 --paths 5"
    )]
    Journey {
        /// Journey start label or node id, for example: main or n12.
        #[arg(long)]
        from: String,

        /// Journey target label or node id, for example: load_config or n42.
        #[arg(long)]
        to: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum path search depth between the endpoints.
        #[arg(long, default_value_t = 8)]
        depth: usize,

        /// Maximum ranked alternative paths to return.
        #[arg(long, default_value_t = 3)]
        paths: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Install project-scoped agent guidance: .mcp.json entry plus CLAUDE.md/AGENTS.md snippets.
    InstallAgent {
        /// Project root to install into.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Target assistant platform.
        #[arg(long, value_enum, default_value_t = install::AgentPlatform::All)]
        platform: install::AgentPlatform,

        /// Overwrite an existing conflicting codegraph entry in .mcp.json.
        #[arg(long)]
        force: bool,
    },

    /// Save an investigation outcome to repository memory (.codegraph/memory.jsonl).
    #[command(
        after_help = "Examples:\n  codegraph memory-save 'configs target:DATABASE_URL' . --outcome useful --note 'reader lives in server config'\n  codegraph memory-save 'calls(function:legacy)' . --outcome dead_end"
    )]
    MemorySave {
        /// The query, ask question, or journey expression that was investigated.
        query: String,

        /// Project root the investigation ran against.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Investigation outcome: useful, dead_end, or corrected.
        #[arg(long, value_parser = clap::builder::ValueParser::new(str::parse::<memory::MemoryOutcome>))]
        outcome: memory::MemoryOutcome,

        /// Free-text lesson or correction.
        #[arg(long)]
        note: Option<String>,

        /// Linked graph node ids; repeat for multiple nodes.
        #[arg(long = "node-id")]
        node_ids: Vec<u64>,

        /// Include hidden files and directories in the fingerprint.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories in the fingerprint.
        #[arg(long)]
        include_ignored: bool,
    },

    /// List saved investigation memory with staleness against the current project fingerprint.
    MemoryList {
        /// Project root the memory belongs to.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Filter records by outcome: useful, dead_end, or corrected.
        #[arg(long, value_parser = clap::builder::ValueParser::new(str::parse::<memory::MemoryOutcome>))]
        outcome: Option<memory::MemoryOutcome>,

        /// Return only records whose source fingerprint no longer matches.
        #[arg(long)]
        only_stale: bool,

        /// Include hidden files and directories in the fingerprint.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories in the fingerprint.
        #[arg(long)]
        include_ignored: bool,
    },

    /// Aggregate saved investigation memory into repository lessons with stale-source warnings.
    MemoryReflect {
        /// Project root the memory belongs to.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Serve CodeGraph analysis tools to assistants over the MCP stdio transport.
    #[command(
        after_help = "Examples:\n  codegraph mcp .\n  # .mcp.json entry:\n  #   {\"mcpServers\": {\"codegraph\": {\"command\": \"codegraph\", \"args\": [\"mcp\", \".\"]}}}"
    )]
    Mcp {
        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// List local query audit records from .codegraph/query-log.jsonl.
    QueryLog {
        /// Project root containing the query log.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Only include records with this action, for example: query, ask, journey, mcp:query_graph.
        #[arg(long)]
        action: Option<String>,

        /// Maximum number of most recent records to include.
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },

    /// Emit a one-shot refactor context bundle: impact, dependencies, optional journey, risks, and source.
    #[command(
        after_help = "Examples:\n  codegraph refactor-context load_config .\n  codegraph refactor-context scan_project . --from main --depth 6"
    )]
    RefactorContext {
        /// Refactor target label or node id, for example: load_config or n42.
        target: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Optional journey start label or node id, for example an entrypoint.
        #[arg(long)]
        from: Option<String>,

        /// Maximum traversal depth for impact and journey.
        #[arg(long, default_value_t = 8)]
        depth: usize,

        /// Maximum ranked journey paths.
        #[arg(long, default_value_t = 3)]
        paths: usize,

        /// Maximum listed dependents.
        #[arg(long, default_value_t = 100)]
        dependent_limit: usize,

        /// Maximum bundled risks.
        #[arg(long, default_value_t = 50)]
        risk_limit: usize,

        /// Source preview context lines around the target span.
        #[arg(long, default_value_t = 6)]
        source_context: u32,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Rank cross-area boundaries by coupling friction: safest seams to extract and most tangled ones.
    Seams {
        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum ranked boundaries per list.
        #[arg(long, default_value_t = 25)]
        limit: usize,

        /// Maximum sample edge indexes per boundary.
        #[arg(long, default_value_t = 10)]
        edge_limit: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Report the blast radius of changing a node: dependents, entrypoints, tests, and impact score.
    #[command(
        after_help = "Examples:\n  codegraph impact load_config .\n  codegraph impact n42 . --depth 8 --limit 200"
    )]
    Impact {
        /// Impact target label or node id, for example: load_config or n42.
        target: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum reverse dependency depth.
        #[arg(long, default_value_t = 6)]
        depth: usize,

        /// Maximum listed dependents.
        #[arg(long, default_value_t = 100)]
        limit: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Group a node's incoming/outgoing dependencies by architecture area, package, and language.
    #[command(visible_alias = "component")]
    ComponentDependencies {
        /// Component target label or node id, for example: load_config or n42.
        target: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum groups per facet.
        #[arg(long, default_value_t = 25)]
        group_limit: usize,

        /// Maximum sample edge indexes per group.
        #[arg(long, default_value_t = 10)]
        edge_limit: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// List the exact dependency edges between two architecture areas with confidence and risks.
    #[command(visible_alias = "contract")]
    ComponentContract {
        /// Source architecture area, for example: crates or web.
        #[arg(long)]
        source: String,

        /// Target architecture area.
        #[arg(long)]
        target: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum listed contract edges.
        #[arg(long, default_value_t = 100)]
        edge_limit: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Map a natural-language investigation question to a bounded graph query and run it.
    #[command(
        after_help = "Examples:\n  codegraph ask \"Where is DATABASE_URL read?\" .\n  codegraph ask \"Who calls load_config?\" .\n  codegraph ask \"Какие точки входа есть в проекте?\" ."
    )]
    Ask {
        /// Question, for example: "Where is DATABASE_URL read?".
        question: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        /// Collapse repeated low-signal nodes in the query result.
        #[arg(long)]
        compact: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Emit an investigation card for one graph node as JSON.
    NodeCard(NodeCardArgs),

    /// Search source text and emit compact matching snippets as JSON.
    SourceSearch(SourceSearchArgs),

    /// Explain why an edge exists and show confidence/provenance evidence.
    ExplainEdge {
        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Exact edge index from graph/query/focus results.
        #[arg(long)]
        edge_index: Option<usize>,

        /// Source node id or label substring.
        #[arg(long)]
        source: Option<String>,

        /// Target node id or label substring.
        #[arg(long)]
        target: Option<String>,

        /// Edge kind substring such as calls, imports, or references.
        #[arg(long)]
        kind: Option<String>,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Trace outgoing code-flow dependencies from a node label.
    Trace {
        /// Function/node label to trace from.
        label: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum outgoing dependency depth.
        #[arg(long, default_value_t = 2)]
        depth: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Trace incoming dependents that can reach a node label.
    TraceDependents {
        /// Function/node label to trace back from.
        label: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum incoming dependency depth.
        #[arg(long, default_value_t = 3)]
        depth: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Trace outgoing code-flow dependencies from entrypoint candidates.
    TraceEntrypoints {
        /// Filter entrypoints by label, kind, language, or metadata.
        #[arg(long)]
        search: Option<String>,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum outgoing dependency depth.
        #[arg(long, default_value_t = 3)]
        depth: usize,

        /// Maximum entrypoint traces to return.
        #[arg(long, default_value_t = 25)]
        limit: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Emit a block-style workflow from an entrypoint or node label.
    Workflow {
        /// Entrypoint/function/node label to convert into workflow blocks.
        label: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum outgoing dependency depth.
        #[arg(long, default_value_t = 4)]
        depth: usize,

        /// Maximum workflow blocks to return.
        #[arg(long, default_value_t = 200)]
        block_limit: usize,

        /// Restrict traversal to an edge kind such as calls, reads_environment, may_error, or depends_on.
        #[arg(long)]
        edge_kind: Option<String>,

        /// Restrict traversal to an edge confidence such as exact, semantic, syntactic, or heuristic.
        #[arg(long)]
        confidence: Option<String>,

        /// Restrict returned blocks to a source language metadata value.
        #[arg(long)]
        language: Option<String>,

        /// Restrict returned blocks/transitions to risk severity: info, warning, or error.
        #[arg(long)]
        risk_severity: Option<String>,

        /// Restrict returned blocks to a workflow kind such as call, branch, loop, async, return, config_read, environment_read, or error.
        #[arg(long)]
        block_kind: Option<String>,

        /// Collapse repeated low-signal workflow blocks into compact aggregate blocks.
        #[arg(long)]
        compact: bool,

        /// Output format.
        #[arg(long, value_enum, default_value_t = WorkflowFormat::Json)]
        format: WorkflowFormat,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Emit block-style workflows from entrypoint candidates.
    WorkflowEntrypoints {
        /// Filter entrypoints by label, kind, language, or metadata.
        #[arg(long)]
        search: Option<String>,

        /// Restrict entrypoints to a matching entrypoint_kind metadata value such as route, workflow_job, pipeline_job, make_target, service, or cmd.
        #[arg(long)]
        entrypoint_kind: Option<String>,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum outgoing dependency depth.
        #[arg(long, default_value_t = 4)]
        depth: usize,

        /// Maximum workflow blocks per entrypoint.
        #[arg(long, default_value_t = 200)]
        block_limit: usize,

        /// Maximum entrypoint workflows to return.
        #[arg(long, default_value_t = 25)]
        limit: usize,

        /// Restrict traversal to an edge kind such as calls, reads_environment, may_error, or depends_on.
        #[arg(long)]
        edge_kind: Option<String>,

        /// Restrict traversal to an edge confidence such as exact, semantic, syntactic, or heuristic.
        #[arg(long)]
        confidence: Option<String>,

        /// Restrict returned blocks to a source language metadata value.
        #[arg(long)]
        language: Option<String>,

        /// Restrict returned blocks/transitions to risk severity: info, warning, or error.
        #[arg(long)]
        risk_severity: Option<String>,

        /// Restrict returned blocks to a workflow kind such as call, branch, loop, async, return, config_read, environment_read, or error.
        #[arg(long)]
        block_kind: Option<String>,

        /// Collapse repeated low-signal workflow blocks into compact aggregate blocks.
        #[arg(long)]
        compact: bool,

        /// Output format.
        #[arg(long, value_enum, default_value_t = WorkflowFormat::Json)]
        format: WorkflowFormat,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Emit block-style workflows from graph query result nodes.
    WorkflowQuery {
        /// Graph query expression whose returned nodes become workflow starts.
        query: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum outgoing dependency depth.
        #[arg(long, default_value_t = 4)]
        depth: usize,

        /// Maximum workflow blocks per query node.
        #[arg(long, default_value_t = 200)]
        block_limit: usize,

        /// Maximum query-node workflows to return.
        #[arg(long, default_value_t = 25)]
        limit: usize,

        /// Restrict traversal to an edge kind such as calls, reads_environment, may_error, or depends_on.
        #[arg(long)]
        edge_kind: Option<String>,

        /// Restrict traversal to an edge confidence such as exact, semantic, syntactic, or heuristic.
        #[arg(long)]
        confidence: Option<String>,

        /// Restrict returned blocks to a source language metadata value.
        #[arg(long)]
        language: Option<String>,

        /// Restrict returned blocks/transitions to risk severity: info, warning, or error.
        #[arg(long)]
        risk_severity: Option<String>,

        /// Restrict returned blocks to a workflow kind such as call, branch, loop, async, return, config_read, environment_read, or error.
        #[arg(long)]
        block_kind: Option<String>,

        /// Collapse repeated low-signal workflow blocks into compact aggregate blocks.
        #[arg(long)]
        compact: bool,

        /// Output format.
        #[arg(long, value_enum, default_value_t = WorkflowFormat::Json)]
        format: WorkflowFormat,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Trace config files and environment variables back to readers and entrypoints.
    #[command(
        after_help = "Examples:\n  codegraph trace-config DATABASE_URL .\n  codegraph trace-config config/settings.toml . --depth 8"
    )]
    TraceConfig {
        /// Config file or environment variable label to trace.
        target: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum upstream dependency depth, including the final config read edge.
        #[arg(long, default_value_t = 6)]
        depth: usize,

        /// Maximum trace paths to return.
        #[arg(long, default_value_t = 50)]
        limit: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },

    /// Trace potential error/exception constructs back to sources and entrypoints.
    TraceErrors {
        /// Error label or metadata substring to trace.
        target: String,

        /// Project root to scan.
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Maximum upstream dependency depth, including the final error edge.
        #[arg(long, default_value_t = 6)]
        depth: usize,

        /// Maximum trace paths to return.
        #[arg(long, default_value_t = 50)]
        limit: usize,

        /// Include hidden files and directories.
        #[arg(long)]
        include_hidden: bool,

        /// Include default ignored directories such as target and node_modules.
        #[arg(long)]
        include_ignored: bool,

        #[command(flatten)]
        cache: CacheArgs,
    },
}

#[derive(Debug, Args)]
struct ScanArgs {
    /// Project root to scan.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,

    #[command(flatten)]
    cache: CacheArgs,
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Output format.
    #[arg(long, value_enum, default_value_t = ReportFormat::Json)]
    format: ReportFormat,

    /// Write the report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Maximum architecture groups to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT)]
    architecture_group_limit: usize,

    /// Maximum architecture edges to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT)]
    architecture_edge_limit: usize,

    /// Maximum language dependency links to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_LANGUAGE_LINK_LIMIT)]
    language_link_limit: usize,

    /// Maximum hotspots to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_HOTSPOT_LIMIT)]
    hotspot_limit: usize,

    /// Maximum graph communities to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_COMMUNITY_LIMIT)]
    community_limit: usize,

    /// Maximum insights to include while keeping full insight counts.
    #[arg(long, default_value_t = DEFAULT_REPORT_INSIGHT_LIMIT)]
    insight_limit: usize,

    /// Maximum compact file summaries to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_FILE_SUMMARY_LIMIT)]
    file_summary_limit: usize,

    /// Maximum compact node summaries to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_NODE_SUMMARY_LIMIT)]
    node_summary_limit: usize,

    /// Mark the quality gate as failed when an insight has this severity or higher.
    #[arg(long, value_enum, default_value = "error")]
    fail_on: InsightSeverityArg,
}

#[derive(Debug, Args)]
struct SemanticPlanArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum concrete semantic work items to include; larger values are capped.
    #[arg(long, default_value_t = DEFAULT_SEMANTIC_WORK_ITEM_LIMIT)]
    work_item_limit: usize,

    /// Restrict semantic work items to a source language such as rust or python.
    #[arg(long)]
    work_language: Option<String>,

    /// Restrict semantic work items to a status such as ready or missing_server.
    #[arg(long)]
    work_status: Option<String>,

    /// Restrict semantic work items to an LSP capability such as definitions.
    #[arg(long)]
    work_capability: Option<String>,
}

#[derive(Debug, Args)]
struct SemanticPatchArgs {
    #[command(flatten)]
    plan: SemanticPlanArgs,

    /// JSON file containing an array of semantic LSP responses.
    #[arg(long)]
    responses: PathBuf,
}

#[derive(Debug, Args)]
struct SemanticRunArgs {
    #[command(flatten)]
    plan: SemanticPlanArgs,

    /// Milliseconds to wait for each language-server response; larger values are capped.
    #[arg(long, default_value_t = DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS)]
    request_timeout_ms: u64,
}

#[derive(Debug, Args)]
struct CoverageArgs {
    /// Project root to inspect.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,
}

#[derive(Debug, Args)]
struct ArchitectureArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum groups to include.
    #[arg(long, default_value_t = 50)]
    group_limit: usize,

    /// Maximum inter-group edges to include.
    #[arg(long, default_value_t = 200)]
    edge_limit: usize,
}

#[derive(Debug, Args)]
struct LanguageDependencyArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum language dependency links to include.
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Args)]
struct SurprisingLinkArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum surprising links to include.
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Args)]
struct HotspotArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum hotspots to include.
    #[arg(long, default_value_t = 25)]
    limit: usize,
}

#[derive(Debug, Args)]
struct CommunityArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum graph communities to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_COMMUNITY_LIMIT)]
    limit: usize,
}

#[derive(Debug, Clone, Args)]
struct CacheArgs {
    /// Disable persistent graph cache for this command.
    #[arg(long)]
    no_cache: bool,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    cache_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct BenchmarkArgs {
    /// Project root to scan.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Number of measured scan runs.
    #[arg(long, default_value_t = 3)]
    runs: usize,

    /// Include hidden files and directories.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,
}

#[derive(Debug, Args)]
struct CacheDiffArgs {
    /// Project root to inspect.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,

    /// Maximum changed files per list.
    #[arg(long, default_value_t = 100)]
    limit: usize,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    cache_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct IncrementalOutputArgs {
    #[command(flatten)]
    cache: CacheDiffArgs,

    /// Include the full merged graph JSON instead of the compact summary.
    #[arg(long)]
    full_graph: bool,
}

#[derive(Debug, Args)]
struct InsightArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Filter insights by severity.
    #[arg(long, value_enum)]
    severity: Option<InsightSeverityArg>,

    /// Filter insights by kind substring.
    #[arg(long)]
    kind: Option<String>,

    /// Filter insights by kind, message, node id, or edge index substring.
    #[arg(long)]
    search: Option<String>,

    /// Maximum insights to return.
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Fail when an insight has this severity or higher.
    #[arg(long, value_enum, default_value = "error")]
    fail_on: InsightSeverityArg,

    /// Restrict checks to insight kinds containing this substring.
    #[arg(long)]
    kind: Option<String>,

    /// Restrict checks by kind, message, node id, or edge index substring.
    #[arg(long)]
    search: Option<String>,

    /// Maximum insights to include in the JSON report.
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Args)]
struct NodeCardArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Graph node id to inspect, numeric or n-prefixed (42 or n42).
    #[arg(long, value_parser = parse_cli_node_id)]
    node_id: u64,

    /// Maximum neighboring edges to include.
    #[arg(long, default_value_t = 80)]
    edge_limit: usize,

    /// Source context lines around the node span.
    #[arg(long, default_value_t = 5)]
    source_context: u32,

    /// Maximum related insights to include.
    #[arg(long, default_value_t = 8)]
    insight_limit: usize,
}

#[derive(Debug, Args)]
struct SourceSearchArgs {
    /// Text to search in source files.
    query: String,

    /// Project root to search.
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Restrict results to paths containing this substring.
    #[arg(long)]
    path_filter: Option<String>,

    /// Match case exactly.
    #[arg(long)]
    case_sensitive: bool,

    /// Maximum matches to return.
    #[arg(long, default_value_t = 50)]
    limit: usize,

    /// Context lines before and after each match.
    #[arg(long, default_value_t = 2)]
    context: usize,

    /// Include hidden files and directories.
    #[arg(long)]
    include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    include_ignored: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Dot,
    Ndjson,
    Graphml,
    MermaidHtml,
    Cypher,
    Falkordb,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum WorkflowFormat {
    Json,
    Mermaid,
    Html,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ReportFormat {
    Json,
    Markdown,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InsightSeverityArg {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    path: String,
    runs: usize,
    include_hidden: bool,
    include_ignored: bool,
    max_file_size: u64,
    fastest_ms: f64,
    slowest_ms: f64,
    average_ms: f64,
    measurements: Vec<BenchmarkMeasurement>,
    summary: codegraph_analysis::GraphSummary,
}

#[derive(Debug, Serialize)]
struct BenchmarkMeasurement {
    run: usize,
    duration_ms: f64,
    nodes: usize,
    edges: usize,
}

#[derive(Debug, Serialize)]
struct ProjectReportSnapshot {
    root: String,
    generated_at_unix: u64,
    cache: codegraph_storage::CacheInfo,
    coverage: codegraph_indexer::ScanCoverageReport,
    report: ProjectReport,
}

#[derive(Debug, Serialize)]
struct LanguageInfo {
    language: &'static str,
    parser: &'static str,
    extensions: &'static [&'static str],
    file_names: &'static [&'static str],
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let max_file_size = cli.max_file_size;
    let log_queries = cli.log_queries;

    match cli.command {
        Command::Languages => {
            println!("{}", serde_json::to_string_pretty(&language_report())?);
        }
        Command::Lsp => {
            println!("{}", serde_json::to_string_pretty(&discover_lsp_servers())?);
        }
        Command::SemanticReadiness(args) => {
            let graph = scan_with_options(
                args.path,
                args.include_hidden,
                args.include_ignored,
                max_file_size,
                &args.cache,
            )?;
            let summary = summarize(&graph);
            println!(
                "{}",
                serde_json::to_string_pretty(&semantic_readiness(&summary.languages))?
            );
        }
        Command::SemanticPlan(args) => {
            let work_item_limit = normalize_semantic_work_item_limit(args.work_item_limit);
            let filter = SemanticWorkItemFilter {
                language: args.work_language,
                status: args.work_status,
                capability: args.work_capability,
            };
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&semantic_enrichment_plan_with_filter(
                    &graph,
                    work_item_limit,
                    filter
                ))?
            );
        }
        Command::SemanticBatch(args) => {
            let work_item_limit = normalize_semantic_work_item_limit(args.work_item_limit);
            let path = args.scan.path;
            let workspace_root = canonical_workspace_root(&path);
            let filter = SemanticWorkItemFilter {
                language: args.work_language,
                status: args.work_status,
                capability: args.work_capability,
            };
            let graph = scan_with_options(
                path.clone(),
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&semantic_execution_batch(
                    &workspace_root,
                    &graph,
                    work_item_limit,
                    filter
                ))?
            );
        }
        Command::SemanticRun(args) => {
            let work_item_limit = normalize_semantic_work_item_limit(args.plan.work_item_limit);
            let path = args.plan.scan.path;
            let workspace_root = canonical_workspace_root(&path);
            let filter = SemanticWorkItemFilter {
                language: args.plan.work_language,
                status: args.plan.work_status,
                capability: args.plan.work_capability,
            };
            let graph = scan_with_options(
                path,
                args.plan.scan.include_hidden,
                args.plan.scan.include_ignored,
                max_file_size,
                &args.plan.scan.cache,
            )?;
            let batch = semantic_execution_batch(&workspace_root, &graph, work_item_limit, filter);
            let semantic_cache = semantic_cache_from_args(&args.plan.scan.cache);
            let run = run_semantic_execution_batch_cached(
                semantic_cache.as_ref(),
                &batch,
                &SemanticLspRunOptions {
                    request_timeout: std::time::Duration::from_millis(
                        normalize_semantic_request_timeout_ms(args.request_timeout_ms),
                    ),
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&run.responses)?);
        }
        Command::SemanticPatch(args) => {
            let work_item_limit = normalize_semantic_work_item_limit(args.plan.work_item_limit);
            let path = args.plan.scan.path;
            let workspace_root = canonical_workspace_root(&path);
            let filter = SemanticWorkItemFilter {
                language: args.plan.work_language,
                status: args.plan.work_status,
                capability: args.plan.work_capability,
            };
            let graph = scan_with_options(
                path,
                args.plan.scan.include_hidden,
                args.plan.scan.include_ignored,
                max_file_size,
                &args.plan.scan.cache,
            )?;
            let batch = semantic_execution_batch(&workspace_root, &graph, work_item_limit, filter);
            let response_text = std::fs::read_to_string(&args.responses)?;
            let responses: Vec<SemanticLspResponse> = serde_json::from_str(&response_text)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&semantic_graph_patch_from_responses(
                    &workspace_root,
                    &graph,
                    &batch,
                    &responses
                ))?
            );
        }
        Command::SemanticApply(args) => {
            let work_item_limit = normalize_semantic_work_item_limit(args.plan.work_item_limit);
            let path = args.plan.scan.path;
            let workspace_root = canonical_workspace_root(&path);
            let filter = SemanticWorkItemFilter {
                language: args.plan.work_language,
                status: args.plan.work_status,
                capability: args.plan.work_capability,
            };
            let graph = scan_with_options(
                path,
                args.plan.scan.include_hidden,
                args.plan.scan.include_ignored,
                max_file_size,
                &args.plan.scan.cache,
            )?;
            let batch = semantic_execution_batch(&workspace_root, &graph, work_item_limit, filter);
            let response_text = std::fs::read_to_string(&args.responses)?;
            let responses: Vec<SemanticLspResponse> = serde_json::from_str(&response_text)?;
            let patch =
                semantic_graph_patch_from_responses(&workspace_root, &graph, &batch, &responses);
            println!(
                "{}",
                serde_json::to_string_pretty(&apply_semantic_graph_patch(&graph, &patch))?
            );
        }
        Command::Scan {
            path,
            include_hidden,
            include_ignored,
            format,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            print_graph(&graph, format)?;
        }
        Command::Summary(args) => {
            let graph = scan_with_options(
                args.path,
                args.include_hidden,
                args.include_ignored,
                max_file_size,
                &args.cache,
            )?;
            println!("{}", serde_json::to_string_pretty(&summarize(&graph))?);
        }
        Command::Report(args) => {
            let format = args.format;
            let output = args.output.clone();
            let snapshot = build_project_report_snapshot(args, max_file_size)?;
            let rendered = match format {
                ReportFormat::Json => serde_json::to_string_pretty(&snapshot)?,
                ReportFormat::Markdown => project_report_markdown(
                    &snapshot.report,
                    &ProjectReportMarkdownOptions {
                        title: "CodeGraph Project Report".to_string(),
                        root: Some(snapshot.root.clone()),
                        generated_at_unix: Some(snapshot.generated_at_unix),
                    },
                ),
            };
            if let Some(output) = output {
                fs::write(output, rendered)?;
            } else {
                println!("{rendered}");
            }
        }
        Command::Coverage(args) => {
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&scan_coverage(&args.path, &options)?)?
            );
        }
        Command::Architecture(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&architecture_map(
                    &graph,
                    args.group_limit,
                    args.edge_limit,
                ))?
            );
        }
        Command::LanguageDependencies(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&language_dependencies(&graph, args.limit))?
            );
        }
        Command::SurprisingLinks(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&surprising_links(&graph, args.limit))?
            );
        }
        Command::Hotspots(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&hotspots(&graph, args.limit))?
            );
        }
        Command::Communities(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&communities(&graph, args.limit))?
            );
        }
        Command::Benchmark(args) => {
            let report = benchmark_scans(args, max_file_size)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::CacheDiff(args) => {
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            let cache = GraphCache::new(args.cache_dir.unwrap_or_else(default_cache_dir));
            let report = cache.diff(&args.path, &options, args.limit)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::CacheChunks(args) => {
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            let cache = GraphCache::new(args.cache_dir.unwrap_or_else(default_cache_dir));
            let report = cache.chunks(&args.path, &options, args.limit)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::IncrementalPlan(args) => {
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            let cache = GraphCache::new(args.cache_dir.unwrap_or_else(default_cache_dir));
            let plan = cache.incremental_plan(&args.path, &options, args.limit)?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        Command::IncrementalScan(args) => {
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            let cache = GraphCache::new(args.cache_dir.unwrap_or_else(default_cache_dir));
            let scan = cache.incremental_scan(&args.path, &options, args.limit)?;
            println!("{}", serde_json::to_string_pretty(&scan)?);
        }
        Command::IncrementalMergePreview(args) => {
            let cache_args = args.cache;
            let options = configured_index_options(
                &cache_args.path,
                &scan_overrides(
                    cache_args.include_hidden,
                    cache_args.include_ignored,
                    max_file_size,
                ),
            )?;
            let cache = GraphCache::new(cache_args.cache_dir.unwrap_or_else(default_cache_dir));
            let preview =
                cache.incremental_merge_preview(&cache_args.path, &options, cache_args.limit)?;
            if args.full_graph {
                println!("{}", serde_json::to_string_pretty(&preview)?);
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&merge_preview_compact(&preview))?
                );
            }
        }
        Command::IncrementalUpdate(args) => {
            let cache_args = args.cache;
            let options = configured_index_options(
                &cache_args.path,
                &scan_overrides(
                    cache_args.include_hidden,
                    cache_args.include_ignored,
                    max_file_size,
                ),
            )?;
            let cache = GraphCache::new(cache_args.cache_dir.unwrap_or_else(default_cache_dir));
            let update = cache.incremental_update(&cache_args.path, &options, cache_args.limit)?;
            if args.full_graph {
                println!("{}", serde_json::to_string_pretty(&update)?);
            } else {
                let mut compact = merge_preview_compact(&update.preview);
                compact["cache"] = serde_json::to_value(&update.cache)?;
                println!("{}", serde_json::to_string_pretty(&compact)?);
            }
        }
        Command::RegistryAdd {
            path,
            name,
            registry_path,
        } => {
            let registry_path = registry_path.unwrap_or_else(registry::default_registry_path);
            let project = registry::add(&registry_path, &path, name)?;
            println!("{}", serde_json::to_string_pretty(&project)?);
        }
        Command::RegistryList { registry_path } => {
            let registry_path = registry_path.unwrap_or_else(registry::default_registry_path);
            let report = registry::list(&registry_path)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::RegistryRemove {
            name,
            registry_path,
        } => {
            let registry_path = registry_path.unwrap_or_else(registry::default_registry_path);
            let removed = registry::remove(&registry_path, &name)?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::RegistryQuery {
            expression,
            projects,
            compact,
            registry_path,
            cache,
        } => {
            let registry_path = registry_path.unwrap_or_else(registry::default_registry_path);
            let graph_cache = (!cache.no_cache).then(|| {
                GraphCache::new(cache.cache_dir.clone().unwrap_or_else(default_cache_dir))
            });
            let report = registry::run_query(
                &registry_path,
                graph_cache.as_ref(),
                &expression,
                &projects,
                compact,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Merge {
            inputs,
            projects,
            output,
            label,
            registry_path,
            cache,
        } => {
            let graph_cache = (!cache.no_cache).then(|| {
                GraphCache::new(cache.cache_dir.clone().unwrap_or_else(default_cache_dir))
            });
            let mut taken = Vec::new();
            let mut merge_inputs = Vec::new();
            for path in &inputs {
                let graph = merge::load_graph_file(path)?;
                let name = merge::unique_input_name(path, &mut taken);
                merge_inputs.push(merge::MergeInput {
                    name,
                    origin: path.display().to_string(),
                    graph,
                });
            }
            if !projects.is_empty() {
                let registry_path = registry_path.unwrap_or_else(registry::default_registry_path);
                let registry = registry::load(&registry_path)?;
                for name in &projects {
                    let project = registry
                        .projects
                        .iter()
                        .find(|project| &project.name == name)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "project `{name}` is not registered in {}",
                                registry_path.display()
                            )
                        })?;
                    if taken.contains(name) {
                        return Err(anyhow::anyhow!(
                            "input name `{name}` is used by both a file and a project; rename one"
                        ));
                    }
                    let root = PathBuf::from(&project.root);
                    let options =
                        configured_index_options(&root, &IndexOptionOverrides::default())?;
                    let scanned = scan_project_cached(&root, &options, graph_cache.as_ref())?;
                    taken.push(name.clone());
                    merge_inputs.push(merge::MergeInput {
                        name: name.clone(),
                        origin: project.root.clone(),
                        graph: scanned.graph,
                    });
                }
            }
            let (merged, report) = merge::merge_graphs(merge_inputs, &label)?;
            match output {
                Some(path) => {
                    fs::write(
                        &path,
                        format!("{}\n", serde_json::to_string_pretty(&merged)?),
                    )?;
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                None => {
                    println!("{}", serde_json::to_string_pretty(&merged)?);
                }
            }
        }
        Command::BenchContext {
            path,
            samples,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let report = bench_context::run_benchmark(&graph, &path, samples)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::PrImpact {
            path,
            base,
            files,
            ci_state,
            review_state,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let (changed, base_used) = if files.is_empty() {
                (pr_impact::git_changed_files(&path, &base)?, Some(base))
            } else {
                (files, None)
            };
            let branch = pr_impact::git_current_branch(&path);
            let report = pr_impact::pr_impact(
                &graph,
                &changed,
                pr_impact::PrImpactContext {
                    base: base_used,
                    branch,
                    ci_state,
                    review_state,
                },
            );
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ExportWiki {
            path,
            output,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let label = path
                .canonicalize()
                .ok()
                .and_then(|root| {
                    root.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "CodeGraph".to_string());
            let report = wiki::export_wiki(&graph, &output, &label)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::InstallHooks { path } => {
            let report = hooks::install_hooks(&path)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::HookRun {
            kind,
            path,
            include_hidden,
            include_ignored,
            limit,
            cache_dir,
        } => {
            let options = configured_index_options(
                &path,
                &scan_overrides(include_hidden, include_ignored, max_file_size),
            )?;
            let cache = GraphCache::new(cache_dir.unwrap_or_else(default_cache_dir));
            let report =
                hooks::hook_run(&cache, &path, &options, &kind, limit, query_log::unix_now())?;
            println!("{}", serde_json::to_string(&report)?);
        }
        Command::Watch {
            args,
            interval_ms,
            max_refreshes,
        } => {
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            let cache = GraphCache::new(args.cache_dir.unwrap_or_else(default_cache_dir));
            watch::run_watch(
                &cache,
                &args.path,
                &options,
                &watch::WatchOptions {
                    interval_ms: interval_ms.max(100),
                    max_refreshes: (max_refreshes > 0).then_some(max_refreshes),
                    limit: args.limit,
                },
            )?;
        }
        Command::Entrypoints(args) => {
            let graph = scan_with_options(
                args.path,
                args.include_hidden,
                args.include_ignored,
                max_file_size,
                &args.cache,
            )?;
            println!("{}", serde_json::to_string_pretty(&entrypoints(&graph))?);
        }
        Command::Insights(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            let report = filter_insight_report(
                insights(&graph),
                &InsightFilter {
                    severity: args.severity.map(InsightSeverity::from),
                    kind: args.kind,
                    search: args.search,
                    limit: args.limit,
                },
            );
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Check(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            let fail_on = InsightSeverity::from(args.fail_on);
            let report = filter_insight_report(
                insights(&graph),
                &InsightFilter {
                    severity: None,
                    kind: args.kind,
                    search: args.search,
                    limit: args.limit,
                },
            );
            let check = check_insights(report, fail_on);
            println!("{}", serde_json::to_string_pretty(&check)?);
            if !check.passed {
                std::process::exit(2);
            }
        }
        Command::Query {
            expression,
            path,
            include_hidden,
            include_ignored,
            compact,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let started = Instant::now();
            let result = query_graph(&graph, &expression);
            let duration_ms = started.elapsed().as_millis() as u64;
            match result {
                Ok(result) => {
                    let result = if compact {
                        compact_query_result(result)
                    } else {
                        result
                    };
                    let output = serde_json::to_string_pretty(&result)?;
                    log_cli_query(
                        &path,
                        log_queries,
                        "query",
                        &expression,
                        "ok",
                        duration_ms,
                        Some(&output),
                    );
                    println!("{output}");
                }
                Err(error) => {
                    log_cli_query(
                        &path,
                        log_queries,
                        "query",
                        &expression,
                        "error",
                        duration_ms,
                        None,
                    );
                    return Err(error.into());
                }
            }
        }
        Command::Journey {
            from,
            to,
            path,
            depth,
            paths,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let query_text = format!("--from {from} --to {to}");
            let started = Instant::now();
            let result = journey(
                &graph,
                JourneyRequest {
                    from,
                    to,
                    max_depth: depth,
                    path_limit: paths,
                },
            );
            let duration_ms = started.elapsed().as_millis() as u64;
            match result {
                Ok(report) => {
                    let output = serde_json::to_string_pretty(&report)?;
                    log_cli_query(
                        &path,
                        log_queries,
                        "journey",
                        &query_text,
                        "ok",
                        duration_ms,
                        Some(&output),
                    );
                    println!("{output}");
                }
                Err(error) => {
                    log_cli_query(
                        &path,
                        log_queries,
                        "journey",
                        &query_text,
                        "error",
                        duration_ms,
                        None,
                    );
                    return Err(error.into());
                }
            }
        }
        Command::InstallAgent {
            path,
            platform,
            force,
        } => {
            let report = install::install_agent(&path, platform, force)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::MemorySave {
            query,
            path,
            outcome,
            note,
            node_ids,
            include_hidden,
            include_ignored,
        } => {
            let fingerprint =
                project_fingerprint_hash(&path, include_hidden, include_ignored, max_file_size)?;
            let recorded_at_unix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or_default();
            let record = memory::save_memory(
                &path,
                memory::MemorySaveRequest {
                    query,
                    outcome,
                    note,
                    node_ids,
                    fingerprint,
                    recorded_at_unix,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&record)?);
        }
        Command::MemoryList {
            path,
            outcome,
            only_stale,
            include_hidden,
            include_ignored,
        } => {
            let fingerprint =
                project_fingerprint_hash(&path, include_hidden, include_ignored, max_file_size)?;
            let report = memory::list_memory(&path, &fingerprint, outcome, only_stale)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::MemoryReflect {
            path,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let fingerprint =
                project_fingerprint_hash(&path, include_hidden, include_ignored, max_file_size)?;
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let node_labels = graph
                .nodes
                .iter()
                .map(|node| (node.id.0, node.label.clone()))
                .collect::<std::collections::BTreeMap<_, _>>();
            let report = memory::reflect(&path, &fingerprint, &node_labels)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Mcp {
            path,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let fingerprint =
                project_fingerprint_hash(&path, include_hidden, include_ignored, max_file_size)
                    .ok();
            mcp::McpServer::new(graph, path, fingerprint).run()?;
        }
        Command::QueryLog {
            path,
            action,
            limit,
        } => {
            let report = query_log::list_query_log(&path, action.as_deref(), limit)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::RefactorContext {
            target,
            path,
            from,
            depth,
            paths,
            dependent_limit,
            risk_limit,
            source_context,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let mut bundle = refactor_context(
                &graph,
                RefactorContextRequest {
                    target,
                    from,
                    max_depth: depth,
                    path_limit: paths,
                    dependent_limit,
                    risk_limit,
                },
            )?;
            if let Some(span) = bundle.target.span.clone() {
                bundle.target_source = read_source_preview(
                    &path,
                    Path::new(&span.path),
                    span.start_line,
                    span.end_line,
                    source_context,
                )
                .ok();
            }
            println!("{}", serde_json::to_string_pretty(&bundle)?);
        }
        Command::Seams {
            path,
            limit,
            edge_limit,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = seams(&graph, SeamRequest { limit, edge_limit });
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Impact {
            target,
            path,
            depth,
            limit,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = impact(
                &graph,
                ImpactRequest {
                    target,
                    max_depth: depth,
                    limit,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ComponentDependencies {
            target,
            path,
            group_limit,
            edge_limit,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = component_dependencies(
                &graph,
                ComponentDependencyRequest {
                    target,
                    group_limit,
                    edge_limit,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::ComponentContract {
            source,
            target,
            path,
            edge_limit,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = component_contract(
                &graph,
                ComponentContractRequest {
                    source,
                    target,
                    edge_limit,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Ask {
            question,
            path,
            include_hidden,
            include_ignored,
            compact,
            cache,
        } => {
            let graph = scan_with_options(
                path.clone(),
                include_hidden,
                include_ignored,
                max_file_size,
                &cache,
            )?;
            let query_text = question.clone();
            let started = Instant::now();
            let result = natural_query(&graph, NaturalQueryRequest { question, compact });
            let duration_ms = started.elapsed().as_millis() as u64;
            match result {
                Ok(report) => {
                    let output = serde_json::to_string_pretty(&report)?;
                    log_cli_query(
                        &path,
                        log_queries,
                        "ask",
                        &query_text,
                        "ok",
                        duration_ms,
                        Some(&output),
                    );
                    println!("{output}");
                }
                Err(error) => {
                    log_cli_query(
                        &path,
                        log_queries,
                        "ask",
                        &query_text,
                        "error",
                        duration_ms,
                        None,
                    );
                    return Err(error.into());
                }
            }
        }
        Command::NodeCard(args) => {
            let graph = scan_with_options(
                args.scan.path.clone(),
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            let card = node_card(
                &graph,
                Some(&args.scan.path),
                NodeCardRequest {
                    node_id: NodeId(args.node_id),
                    edge_limit: args.edge_limit,
                    source_context: args.source_context,
                    insight_limit: args.insight_limit,
                },
            )?
            .ok_or_else(|| anyhow::anyhow!("node {} not found", args.node_id))?;
            println!("{}", serde_json::to_string_pretty(&card)?);
        }
        Command::SourceSearch(args) => {
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            let result = search_source(
                &args.path,
                &SourceSearchRequest {
                    query: args.query,
                    path_filter: args.path_filter,
                    case_sensitive: args.case_sensitive,
                    limit: args.limit,
                    context: args.context,
                    include_hidden: options.include_hidden,
                    include_ignored: options.include_ignored,
                    max_file_size: options.max_file_size,
                    ignored_names: options.ignored_names,
                    ignored_globs: options.ignored_globs,
                },
            );
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::ExplainEdge {
            path,
            edge_index,
            source,
            target,
            kind,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let result = explain_edge(
                &graph,
                ExplainEdgeRequest {
                    edge_index,
                    source,
                    target,
                    kind,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::Trace {
            label,
            path,
            depth,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let result = trace(
                &graph,
                TraceRequest {
                    start: TraceStart::Label(label),
                    max_depth: depth,
                },
            );
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::TraceDependents {
            label,
            path,
            depth,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let result = trace_dependents(
                &graph,
                TraceRequest {
                    start: TraceStart::Label(label),
                    max_depth: depth,
                },
            );
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::TraceEntrypoints {
            search,
            path,
            depth,
            limit,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = trace_entrypoints(
                &graph,
                EntrypointTraceRequest {
                    search,
                    max_depth: depth,
                    limit,
                },
            );
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Workflow {
            label,
            path,
            depth,
            block_limit,
            edge_kind,
            confidence,
            language,
            risk_severity,
            block_kind,
            compact,
            format,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = workflow(
                &graph,
                WorkflowRequest {
                    start: TraceStart::Label(label),
                    max_depth: depth,
                    block_limit,
                    filters: WorkflowFilters::from_parts(
                        edge_kind,
                        confidence,
                        language,
                        risk_severity,
                        block_kind,
                    ),
                    compact,
                },
            );
            match (format, report) {
                (WorkflowFormat::Json, report) => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                (WorkflowFormat::Mermaid, Some(report)) => {
                    println!("{}", workflow_mermaid(&report));
                }
                (WorkflowFormat::Mermaid, None) => {
                    println!("flowchart TD");
                }
                (WorkflowFormat::Html, report) => {
                    let sections = report
                        .iter()
                        .map(|report| MermaidSection {
                            title: report.start.label.clone(),
                            mermaid: workflow_mermaid(report),
                        })
                        .collect::<Vec<_>>();
                    print!("{}", export_mermaid_html("CodeGraph workflow", &sections));
                }
            }
        }
        Command::WorkflowEntrypoints {
            search,
            entrypoint_kind,
            path,
            depth,
            block_limit,
            limit,
            edge_kind,
            confidence,
            language,
            risk_severity,
            block_kind,
            compact,
            format,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = workflow_entrypoints(
                &graph,
                EntrypointWorkflowRequest {
                    search,
                    entrypoint_kind,
                    max_depth: depth,
                    block_limit,
                    limit,
                    filters: WorkflowFilters::from_parts(
                        edge_kind,
                        confidence,
                        language,
                        risk_severity,
                        block_kind,
                    ),
                    compact,
                },
            );
            match format {
                WorkflowFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                WorkflowFormat::Mermaid => {
                    let rendered = report
                        .workflows
                        .iter()
                        .map(|workflow| {
                            format!(
                                "%% {}\n{}",
                                workflow.start.label,
                                workflow_mermaid(workflow)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    println!("{rendered}");
                }
                WorkflowFormat::Html => {
                    let sections = report
                        .workflows
                        .iter()
                        .map(|workflow| MermaidSection {
                            title: workflow.start.label.clone(),
                            mermaid: workflow_mermaid(workflow),
                        })
                        .collect::<Vec<_>>();
                    print!("{}", export_mermaid_html("CodeGraph workflows", &sections));
                }
            }
        }
        Command::WorkflowQuery {
            query,
            path,
            depth,
            block_limit,
            limit,
            edge_kind,
            confidence,
            language,
            risk_severity,
            block_kind,
            compact,
            format,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let report = workflow_query(
                &graph,
                WorkflowQueryRequest {
                    query,
                    max_depth: depth,
                    block_limit,
                    limit,
                    filters: WorkflowFilters::from_parts(
                        edge_kind,
                        confidence,
                        language,
                        risk_severity,
                        block_kind,
                    ),
                    compact,
                },
            )?;
            match format {
                WorkflowFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                WorkflowFormat::Mermaid => {
                    let rendered = report
                        .workflows
                        .iter()
                        .map(|workflow| {
                            format!(
                                "%% {}\n{}",
                                workflow.start.label,
                                workflow_mermaid(workflow)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    println!("{rendered}");
                }
                WorkflowFormat::Html => {
                    let sections = report
                        .workflows
                        .iter()
                        .map(|workflow| MermaidSection {
                            title: workflow.start.label.clone(),
                            mermaid: workflow_mermaid(workflow),
                        })
                        .collect::<Vec<_>>();
                    print!("{}", export_mermaid_html("CodeGraph workflows", &sections));
                }
            }
        }
        Command::TraceConfig {
            target,
            path,
            depth,
            limit,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let result = trace_config(
                &graph,
                ConfigTraceRequest {
                    target,
                    max_depth: depth,
                    limit,
                },
            );
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::TraceErrors {
            target,
            path,
            depth,
            limit,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let result = trace_errors(
                &graph,
                ErrorTraceRequest {
                    target,
                    max_depth: depth,
                    limit,
                },
            );
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}

impl From<InsightSeverityArg> for InsightSeverity {
    fn from(value: InsightSeverityArg) -> Self {
        match value {
            InsightSeverityArg::Info => Self::Info,
            InsightSeverityArg::Warning => Self::Warning,
            InsightSeverityArg::Error => Self::Error,
        }
    }
}

fn scan_overrides(
    include_hidden: bool,
    include_ignored: bool,
    max_file_size: Option<u64>,
) -> IndexOptionOverrides {
    IndexOptionOverrides {
        include_hidden,
        include_ignored,
        max_file_size,
    }
}

fn print_graph(graph: &codegraph_core::CodeGraph, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(graph)?),
        OutputFormat::Dot => print!("{}", export_dot(graph)),
        OutputFormat::Ndjson => print!("{}", export_ndjson(graph)?),
        OutputFormat::Graphml => print!("{}", export_graphml(graph)),
        OutputFormat::MermaidHtml => print!(
            "{}",
            export_graph_mermaid_html(
                graph,
                DEFAULT_MERMAID_NODE_LIMIT,
                DEFAULT_MERMAID_EDGE_LIMIT
            )
        ),
        OutputFormat::Cypher => print!("{}", export_cypher(graph)),
        OutputFormat::Falkordb => print!("{}", export_falkordb(graph, "codegraph")),
    }
    Ok(())
}

fn language_report() -> Vec<LanguageInfo> {
    language_adapters()
        .iter()
        .map(|adapter| {
            let info = adapter.info();
            LanguageInfo {
                language: info.language,
                parser: info.parser,
                extensions: info.extensions,
                file_names: info.file_names,
            }
        })
        .collect()
}

fn canonical_workspace_root(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Changed files from `git diff --name-only <base>` in the project root.
/// Best-effort local query audit: never fails the command, only warns.
fn log_cli_query(
    root: &Path,
    force_enabled: bool,
    action: &str,
    query: &str,
    outcome: &str,
    duration_ms: u64,
    response: Option<&str>,
) {
    let mut settings = match query_log::load_settings(root) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("warning: failed to load query log settings: {error:#}");
            return;
        }
    };
    if force_enabled {
        settings.enabled = true;
    }
    if !settings.enabled {
        return;
    }
    let event = query_log::QueryLogEvent {
        surface: "cli",
        action,
        query,
        outcome,
        duration_ms,
        response,
        recorded_at_unix: query_log::unix_now(),
    };
    if let Err(error) = query_log::log_query(root, &settings, event) {
        eprintln!("warning: failed to write query log: {error:#}");
    }
}

/// Compact incremental output: plan and merge stats stay, the merged graph
/// collapses to node/edge counts so hooks, logs, and agent pipelines are not
/// flooded with tens of megabytes of graph JSON (audit F10).
fn merge_preview_compact(
    preview: &codegraph_storage::IncrementalMergePreview,
) -> serde_json::Value {
    serde_json::json!({
        "plan": preview.plan,
        "merge": preview.merge,
        "graph_summary": {
            "nodes": preview.graph.nodes.len(),
            "edges": preview.graph.edges.len(),
        },
        "note": "compact output; pass --full-graph to include the merged graph JSON",
    })
}

/// Node ids are printed as `n42` in query results and web deep links;
/// accept both that form and the bare numeric id (audit F8).
fn parse_cli_node_id(value: &str) -> Result<u64, String> {
    codegraph_analysis::parse_node_id(value)
        .map(|id| id.0)
        .map_err(|error| error.to_string())
}

fn project_fingerprint_hash(
    path: &Path,
    include_hidden: bool,
    include_ignored: bool,
    max_file_size: Option<u64>,
) -> Result<String> {
    let options = configured_index_options(
        path,
        &scan_overrides(include_hidden, include_ignored, max_file_size),
    )?;
    Ok(GraphCache::fingerprint_project(path, &options)?.hash)
}

fn scan_with_options(
    path: PathBuf,
    include_hidden: bool,
    include_ignored: bool,
    max_file_size: Option<u64>,
    cache_args: &CacheArgs,
) -> Result<codegraph_core::CodeGraph> {
    let options = configured_index_options(
        &path,
        &scan_overrides(include_hidden, include_ignored, max_file_size),
    )?;
    let cache = (!cache_args.no_cache).then(|| {
        GraphCache::new(
            cache_args
                .cache_dir
                .clone()
                .unwrap_or_else(default_cache_dir),
        )
    });
    Ok(scan_project_cached(path, &options, cache.as_ref())?.graph)
}

fn semantic_cache_from_args(cache_args: &CacheArgs) -> Option<SemanticLspCache> {
    if cache_args.no_cache {
        return None;
    }
    Some(SemanticLspCache::new(
        cache_args
            .cache_dir
            .clone()
            .unwrap_or_else(default_cache_dir)
            .join("semantic-lsp"),
    ))
}

fn build_project_report_snapshot(
    args: ReportArgs,
    max_file_size: Option<u64>,
) -> Result<ProjectReportSnapshot> {
    let options = configured_index_options(
        &args.scan.path,
        &scan_overrides(
            args.scan.include_hidden,
            args.scan.include_ignored,
            max_file_size,
        ),
    )?;
    let cache = (!args.scan.cache.no_cache).then(|| {
        GraphCache::new(
            args.scan
                .cache
                .cache_dir
                .clone()
                .unwrap_or_else(default_cache_dir),
        )
    });
    let output = scan_project_cached(args.scan.path.clone(), &options, cache.as_ref())?;
    let coverage = scan_coverage(&args.scan.path, &options)?;
    let report = project_report(&output.graph, report_limits_from_args(&args));

    Ok(ProjectReportSnapshot {
        root: args.scan.path.display().to_string(),
        generated_at_unix: unix_seconds(),
        cache: output.cache,
        coverage,
        report,
    })
}

/// CLI report limits obey the same published `[1, MAX_*]` bounds as the API
/// (`ProjectReportLimits::clamped`), so both surfaces stay in contract.
fn report_limits_from_args(args: &ReportArgs) -> ProjectReportLimits {
    ProjectReportLimits {
        architecture_group_limit: args.architecture_group_limit,
        architecture_edge_limit: args.architecture_edge_limit,
        language_link_limit: args.language_link_limit,
        hotspot_limit: args.hotspot_limit,
        community_limit: args.community_limit,
        insight_limit: args.insight_limit,
        file_summary_limit: args.file_summary_limit,
        node_summary_limit: args.node_summary_limit,
        fail_on: InsightSeverity::from(args.fail_on),
    }
    .clamped()
}

fn benchmark_scans(args: BenchmarkArgs, max_file_size: Option<u64>) -> Result<BenchmarkReport> {
    let runs = args.runs.clamp(1, 100);
    let options = configured_index_options(
        &args.path,
        &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
    )?;
    let mut measurements = Vec::with_capacity(runs);
    let mut last_graph = None;

    for run in 1..=runs {
        let started = Instant::now();
        let graph = scan_project(&args.path, &options)?;
        let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
        measurements.push(BenchmarkMeasurement {
            run,
            duration_ms,
            nodes: graph.nodes.len(),
            edges: graph.edges.len(),
        });
        last_graph = Some(graph);
    }

    let fastest_ms = measurements
        .iter()
        .map(|measurement| measurement.duration_ms)
        .fold(f64::INFINITY, f64::min);
    let slowest_ms = measurements
        .iter()
        .map(|measurement| measurement.duration_ms)
        .fold(0.0, f64::max);
    let average_ms = measurements
        .iter()
        .map(|measurement| measurement.duration_ms)
        .sum::<f64>()
        / measurements.len() as f64;
    let graph = last_graph.expect("runs is clamped to at least one");

    Ok(BenchmarkReport {
        path: args.path.display().to_string(),
        runs,
        include_hidden: args.include_hidden,
        include_ignored: args.include_ignored,
        max_file_size: options.max_file_size,
        fastest_ms,
        slowest_ms,
        average_ms,
        measurements,
        summary: summarize(&graph),
    })
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{CodeGraph, NodeKind};
    use codegraph_storage::{
        CacheRecordStatus, IncrementalMergePreview, IncrementalMergeReport, IncrementalPlanAction,
        IncrementalScanPlan,
    };

    #[test]
    fn merge_preview_compact_keeps_plan_and_merge_but_summarizes_the_graph() {
        let mut graph = CodeGraph::new("repo");
        graph.add_node(NodeKind::Function, "main");
        let preview = IncrementalMergePreview {
            plan: IncrementalScanPlan {
                cache_dir: "/tmp/cache".to_string(),
                cache_record: CacheRecordStatus::Present,
                action: IncrementalPlanAction::Noop,
                reason: "cached fingerprint matches current files".to_string(),
                previous_hash: Some("abc".to_string()),
                current_hash: "abc".to_string(),
                previous_files: Some(2),
                current_files: 2,
                changed_files: 0,
                rescan_files: 0,
                removed_files: 0,
                reusable_files: 2,
                changed_current_bytes: 0,
                reusable_bytes: 10,
                reuse_file_ratio_basis_points: 10_000,
                reuse_byte_ratio_basis_points: 10_000,
                scan_paths: Vec::new(),
                removed_paths: Vec::new(),
                reusable_paths: vec!["src/main.rs".to_string()],
                impacted_nodes: 0,
                impacted_edges: 0,
                impacted_node_ids: Vec::new(),
                impacted_edge_indexes: Vec::new(),
                limit: 100,
                truncated: false,
            },
            merge: IncrementalMergeReport {
                complete_graph: true,
                reused_nodes: 2,
                reused_edges: 1,
                removed_cached_nodes: 0,
                removed_cached_edges: 0,
                chunk_removed_nodes: 0,
                chunk_removed_edges: 0,
                incoming_cross_file_edges: 0,
                graph_surface_added: 0,
                graph_surface_removed: 0,
                removed_paths_blocking: 0,
                completeness_blockers: Vec::new(),
                replaced_paths: 0,
                scanned_nodes: 0,
                scanned_edges: 0,
                merged_nodes: 2,
                merged_edges: 1,
                warning: None,
            },
            graph,
        };

        let compact = merge_preview_compact(&preview);
        assert_eq!(compact["graph_summary"]["nodes"], 2);
        assert_eq!(compact["graph_summary"]["edges"], 0);
        assert_eq!(compact["plan"]["action"], "noop");
        assert_eq!(compact["merge"]["merged_nodes"], 2);
        assert!(compact.get("graph").is_none(), "full graph must be omitted");
        assert!(
            compact["note"]
                .as_str()
                .is_some_and(|note| note.contains("--full-graph"))
        );
    }
}
