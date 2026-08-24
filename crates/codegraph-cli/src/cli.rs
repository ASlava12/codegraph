//! Command-line surface: the clap parser, every subcommand, and the shared
//! argument groups. Execution lives in `main.rs`; this module only declares
//! what the CLI accepts.

use clap::{Args, Parser, Subcommand, ValueEnum};
use codegraph_analysis::memory;
use codegraph_analysis::{
    DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT, DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT,
    DEFAULT_REPORT_COMMUNITY_LIMIT, DEFAULT_REPORT_FILE_SUMMARY_LIMIT,
    DEFAULT_REPORT_HOTSPOT_LIMIT, DEFAULT_REPORT_INSIGHT_LIMIT, DEFAULT_REPORT_LANGUAGE_LINK_LIMIT,
    DEFAULT_REPORT_NODE_SUMMARY_LIMIT,
};
use codegraph_lsp::{DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS, DEFAULT_SEMANTIC_WORK_ITEM_LIMIT};
use std::path::PathBuf;

use crate::install;
use crate::parse_cli_node_id;

#[derive(Debug, Parser)]
#[command(name = "codegraph")]
#[command(about = "Build and inspect code knowledge graphs")]
pub(crate) struct Cli {
    /// Maximum bytes to read from any single file during scans.
    #[arg(long, global = true)]
    pub(crate) max_file_size: Option<u64>,

    /// Log query/ask/journey commands to .codegraph/query-log.jsonl for this run,
    /// even when [query_log] is not enabled in .codegraph/config.toml.
    #[arg(long, global = true)]
    pub(crate) log_queries: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
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
    #[command(
        after_help = "Examples:\n  codegraph registry-add ../backend --name backend\n  codegraph registry-add . --registry-path ./registry.json"
    )]
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
    #[command(
        after_help = "Examples:\n  codegraph registry-query 'configs target:DATABASE_URL'\n  codegraph registry-query 'nodes kind:function limit:5' --project backend"
    )]
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
    #[command(
        after_help = "Examples:\n  codegraph merge api.json docs.json --output merged.json\n  codegraph merge --project backend --project frontend --output merged.json"
    )]
    Merge {
        /// Graph JSON files to merge (as produced by scan/export).
        inputs: Vec<PathBuf>,

        /// Also merge registered projects by name, scanned through the cache
        /// as syntactic graphs — the semantic pass belongs to a single
        /// project's own scan (repeatable).
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
    PrImpact(PrImpactArgs),

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
    Query(QueryArgs),

    /// Emit a step-numbered execution journey between two graph labels or node ids.
    #[command(
        after_help = "Examples:\n  codegraph journey --from main --to load_config .\n  codegraph journey --from 'cargo bin:api' --to n42 . --depth 8 --paths 5"
    )]
    Journey(JourneyArgs),

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

        /// Also write assistant hook configuration snippets under
        /// .codegraph/hooks/ nudging agents toward CodeGraph before
        /// grep-heavy workflows.
        #[arg(long)]
        hooks: bool,
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

        /// Linked graph node ids (numeric or n-prefixed, e.g. 42 or n42);
        /// repeat for multiple nodes.
        #[arg(long = "node-id", value_parser = parse_cli_node_id)]
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
    RefactorContext(RefactorContextArgs),

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
        #[arg(long, default_value_t = 40)]
        limit: usize,

        /// Include repository-wide risks (slower; refactor-context also includes them).
        #[arg(long)]
        include_risks: bool,

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
    #[command(
        after_help = "Examples:\n  codegraph component-contract . --source docs --target crates/codegraph-analysis\n  Area names come from `codegraph architecture .` and must match exactly."
    )]
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
    Ask(AskArgs),

    /// Emit an investigation card for one graph node as JSON.
    NodeCard(NodeCardArgs),

    /// Search source text and emit compact matching snippets as JSON.
    SourceSearch(SourceSearchArgs),

    /// Explain why an edge exists and show confidence/provenance evidence.
    #[command(
        after_help = "Examples:\n  codegraph explain-edge . --edge-index 42\n  codegraph explain-edge . --source main --target load_config --kind calls"
    )]
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
    Workflow(WorkflowArgs),

    /// Emit block-style workflows from entrypoint candidates.
    WorkflowEntrypoints(WorkflowEntrypointsArgs),

    /// Emit block-style workflows from graph query result nodes.
    WorkflowQuery(WorkflowQueryArgs),

    /// Trace config files and environment variables back to readers and entrypoints.
    #[command(
        after_help = "Examples:\n  codegraph trace-config DATABASE_URL .\n  codegraph trace-config config/settings.toml . --depth 8"
    )]
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
        #[arg(long, default_value_t = 8)]
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
pub(crate) struct ScanArgs {
    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct ReportArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Output format.
    #[arg(long, value_enum, default_value_t = ReportFormat::Json)]
    pub(crate) format: ReportFormat,

    /// Write the report to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,

    /// Maximum architecture groups to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_ARCHITECTURE_GROUP_LIMIT)]
    pub(crate) architecture_group_limit: usize,

    /// Maximum architecture edges to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_ARCHITECTURE_EDGE_LIMIT)]
    pub(crate) architecture_edge_limit: usize,

    /// Maximum language dependency links to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_LANGUAGE_LINK_LIMIT)]
    pub(crate) language_link_limit: usize,

    /// Maximum hotspots to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_HOTSPOT_LIMIT)]
    pub(crate) hotspot_limit: usize,

    /// Maximum graph communities to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_COMMUNITY_LIMIT)]
    pub(crate) community_limit: usize,

    /// Maximum insights to include while keeping full insight counts.
    #[arg(long, default_value_t = DEFAULT_REPORT_INSIGHT_LIMIT)]
    pub(crate) insight_limit: usize,

    /// Maximum compact file summaries to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_FILE_SUMMARY_LIMIT)]
    pub(crate) file_summary_limit: usize,

    /// Maximum compact node summaries to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_NODE_SUMMARY_LIMIT)]
    pub(crate) node_summary_limit: usize,

    /// Mark the quality gate as failed when an insight has this severity or higher.
    #[arg(long, value_enum, default_value = "error")]
    pub(crate) fail_on: InsightSeverityArg,
}

#[derive(Debug, Args)]
pub(crate) struct SemanticPlanArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Maximum concrete semantic work items to include; larger values are capped.
    #[arg(long, default_value_t = DEFAULT_SEMANTIC_WORK_ITEM_LIMIT)]
    pub(crate) work_item_limit: usize,

    /// Restrict semantic work items to a source language such as rust or python.
    #[arg(long)]
    pub(crate) work_language: Option<String>,

    /// Restrict semantic work items to a status such as ready or missing_server.
    #[arg(long)]
    pub(crate) work_status: Option<String>,

    /// Restrict semantic work items to an LSP capability such as definitions.
    #[arg(long)]
    pub(crate) work_capability: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct SemanticPatchArgs {
    #[command(flatten)]
    pub(crate) plan: SemanticPlanArgs,

    /// JSON file containing an array of semantic LSP responses.
    #[arg(long)]
    pub(crate) responses: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct SemanticRunArgs {
    #[command(flatten)]
    pub(crate) plan: SemanticPlanArgs,

    /// Milliseconds to wait for each language-server response; larger values are capped.
    #[arg(long, default_value_t = DEFAULT_SEMANTIC_REQUEST_TIMEOUT_MS)]
    pub(crate) request_timeout_ms: u64,
}

#[derive(Debug, Args)]
pub(crate) struct CoverageArgs {
    /// Project root to inspect.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ArchitectureArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Maximum groups to include.
    #[arg(long, default_value_t = 50)]
    pub(crate) group_limit: usize,

    /// Maximum inter-group edges to include.
    #[arg(long, default_value_t = 200)]
    pub(crate) edge_limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct LanguageDependencyArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Maximum language dependency links to include.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct SurprisingLinkArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Maximum surprising links to include.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct HotspotArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Maximum hotspots to include.
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct CommunityArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Maximum graph communities to include.
    #[arg(long, default_value_t = DEFAULT_REPORT_COMMUNITY_LIMIT)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct CacheArgs {
    /// Disable persistent graph cache for this command.
    #[arg(long)]
    pub(crate) no_cache: bool,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    pub(crate) cache_dir: Option<PathBuf>,

    /// Skip the automatic semantic pass and keep the scan syntax-only.
    /// Use this whenever the graph must be reproducible — CI gates above all —
    /// since the automatic pass depends on which language servers are
    /// installed on the machine.
    #[arg(long)]
    pub(crate) no_semantic: bool,
}

#[derive(Debug, Args)]
pub(crate) struct BenchmarkArgs {
    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Number of measured scan runs.
    #[arg(long, default_value_t = 3)]
    pub(crate) runs: usize,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,
}

#[derive(Debug, Args)]
pub(crate) struct CacheDiffArgs {
    /// Project root to inspect.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    /// Maximum changed files per list.
    #[arg(long, default_value_t = 100)]
    pub(crate) limit: usize,

    /// Directory for persistent graph cache records.
    #[arg(long)]
    pub(crate) cache_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct IncrementalOutputArgs {
    #[command(flatten)]
    pub(crate) cache: CacheDiffArgs,

    /// Include the full merged graph JSON instead of the compact summary.
    #[arg(long)]
    pub(crate) full_graph: bool,
}

#[derive(Debug, Args)]
pub(crate) struct InsightArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Filter insights by severity.
    #[arg(long, value_enum)]
    pub(crate) severity: Option<InsightSeverityArg>,

    /// Filter insights by kind substring.
    #[arg(long)]
    pub(crate) kind: Option<String>,

    /// Filter insights by kind, message, node id, or edge index substring.
    #[arg(long)]
    pub(crate) search: Option<String>,

    /// Maximum insights to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Fail when an insight has this severity or higher.
    #[arg(long, value_enum, default_value = "error")]
    pub(crate) fail_on: InsightSeverityArg,

    /// Restrict checks to insight kinds containing this substring.
    #[arg(long)]
    pub(crate) kind: Option<String>,

    /// Restrict checks by kind, message, node id, or edge index substring.
    #[arg(long)]
    pub(crate) search: Option<String>,

    /// Maximum insights to include in the JSON report.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Args)]
pub(crate) struct NodeCardArgs {
    #[command(flatten)]
    pub(crate) scan: ScanArgs,

    /// Graph node id to inspect, numeric or n-prefixed (42 or n42).
    #[arg(long, value_parser = parse_cli_node_id)]
    pub(crate) node_id: u64,

    /// Maximum neighboring edges to include.
    #[arg(long, default_value_t = 24)]
    pub(crate) edge_limit: usize,

    /// Source context lines around the node span.
    #[arg(long, default_value_t = 5)]
    pub(crate) source_context: u32,

    /// Maximum related insights to include.
    #[arg(long, default_value_t = 8)]
    pub(crate) insight_limit: usize,

    /// Include repository-wide insights related to this node (slower).
    #[arg(long)]
    pub(crate) include_insights: bool,
}

#[derive(Debug, Args)]
pub(crate) struct SourceSearchArgs {
    /// Text to search in source files.
    pub(crate) query: String,

    /// Project root to search.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Restrict results to paths containing this substring.
    #[arg(long)]
    pub(crate) path_filter: Option<String>,

    /// Match case exactly.
    #[arg(long)]
    pub(crate) case_sensitive: bool,

    /// Maximum matches to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,

    /// Context lines before and after each match.
    #[arg(long, default_value_t = 2)]
    pub(crate) context: usize,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Dot,
    Ndjson,
    Graphml,
    /// The API publishes this one as `mermaid_html`; take either spelling
    /// so a value read from `/api/schema` works here too.
    #[value(alias = "mermaid_html")]
    MermaidHtml,
    Cypher,
    Falkordb,
    Svg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum WorkflowFormat {
    Json,
    Mermaid,
    Html,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ReportFormat {
    Json,
    Markdown,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum InsightSeverityArg {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Args)]
pub(crate) struct WorkflowArgs {
    /// Entrypoint/function/node label to convert into workflow blocks.
    pub(crate) label: String,

    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Maximum outgoing dependency depth.
    #[arg(long, default_value_t = 4)]
    pub(crate) depth: usize,

    /// Maximum workflow blocks to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) block_limit: usize,

    /// Restrict traversal to an edge kind such as calls, reads_environment, may_error, or depends_on.
    #[arg(long)]
    pub(crate) edge_kind: Option<String>,

    /// Restrict traversal to an edge confidence such as exact, semantic, syntactic, or heuristic.
    #[arg(long)]
    pub(crate) confidence: Option<String>,

    /// Restrict returned blocks to a source language metadata value.
    #[arg(long)]
    pub(crate) language: Option<String>,

    /// Restrict returned blocks/transitions to risk severity: info, warning, or error.
    #[arg(long)]
    pub(crate) risk_severity: Option<String>,

    /// Restrict returned blocks to a workflow kind such as call, branch, loop, async, return, config_read, environment_read, or error.
    #[arg(long)]
    pub(crate) block_kind: Option<String>,

    /// Collapse repeated low-signal workflow blocks into compact aggregate blocks.
    #[arg(long)]
    pub(crate) compact: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = WorkflowFormat::Json)]
    pub(crate) format: WorkflowFormat,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    /// Cap outgoing edges expanded per node (calls first), so the block
    /// budget follows the call chain into depth instead of one wide node.
    #[arg(long)]
    pub(crate) max_fanout: Option<usize>,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct WorkflowEntrypointsArgs {
    /// Filter entrypoints by label, kind, language, or metadata.
    #[arg(long)]
    pub(crate) search: Option<String>,

    /// Restrict entrypoints to a matching entrypoint_kind metadata value such as route, workflow_job, pipeline_job, make_target, service, or cmd.
    #[arg(long)]
    pub(crate) entrypoint_kind: Option<String>,

    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Maximum outgoing dependency depth.
    #[arg(long, default_value_t = 4)]
    pub(crate) depth: usize,

    /// Maximum workflow blocks per entrypoint.
    #[arg(long, default_value_t = 200)]
    pub(crate) block_limit: usize,

    /// Cap outgoing edges expanded per node (calls first) so the block
    /// budget follows the call chain into depth. Unset means unbounded.
    #[arg(long)]
    pub(crate) max_fanout: Option<usize>,

    /// Maximum entrypoint workflows to return.
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: usize,

    /// Restrict traversal to an edge kind such as calls, reads_environment, may_error, or depends_on.
    #[arg(long)]
    pub(crate) edge_kind: Option<String>,

    /// Restrict traversal to an edge confidence such as exact, semantic, syntactic, or heuristic.
    #[arg(long)]
    pub(crate) confidence: Option<String>,

    /// Restrict returned blocks to a source language metadata value.
    #[arg(long)]
    pub(crate) language: Option<String>,

    /// Restrict returned blocks/transitions to risk severity: info, warning, or error.
    #[arg(long)]
    pub(crate) risk_severity: Option<String>,

    /// Restrict returned blocks to a workflow kind such as call, branch, loop, async, return, config_read, environment_read, or error.
    #[arg(long)]
    pub(crate) block_kind: Option<String>,

    /// Collapse repeated low-signal workflow blocks into compact aggregate blocks.
    #[arg(long)]
    pub(crate) compact: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = WorkflowFormat::Json)]
    pub(crate) format: WorkflowFormat,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct WorkflowQueryArgs {
    /// Graph query expression whose returned nodes become workflow starts.
    pub(crate) query: String,

    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Maximum outgoing dependency depth.
    #[arg(long, default_value_t = 4)]
    pub(crate) depth: usize,

    /// Maximum workflow blocks per query node.
    #[arg(long, default_value_t = 200)]
    pub(crate) block_limit: usize,

    /// Cap outgoing edges expanded per node (calls first) so the block
    /// budget follows the call chain into depth. Unset means unbounded.
    #[arg(long)]
    pub(crate) max_fanout: Option<usize>,

    /// Maximum query-node workflows to return.
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: usize,

    /// Restrict traversal to an edge kind such as calls, reads_environment, may_error, or depends_on.
    #[arg(long)]
    pub(crate) edge_kind: Option<String>,

    /// Restrict traversal to an edge confidence such as exact, semantic, syntactic, or heuristic.
    #[arg(long)]
    pub(crate) confidence: Option<String>,

    /// Restrict returned blocks to a source language metadata value.
    #[arg(long)]
    pub(crate) language: Option<String>,

    /// Restrict returned blocks/transitions to risk severity: info, warning, or error.
    #[arg(long)]
    pub(crate) risk_severity: Option<String>,

    /// Restrict returned blocks to a workflow kind such as call, branch, loop, async, return, config_read, environment_read, or error.
    #[arg(long)]
    pub(crate) block_kind: Option<String>,

    /// Collapse repeated low-signal workflow blocks into compact aggregate blocks.
    #[arg(long)]
    pub(crate) compact: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = WorkflowFormat::Json)]
    pub(crate) format: WorkflowFormat,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct JourneyArgs {
    /// Journey start label or node id, for example: main or n12.
    #[arg(long)]
    pub(crate) from: String,

    /// Journey target label or node id, for example: load_config or n42.
    #[arg(long)]
    pub(crate) to: String,

    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Maximum path search depth between the endpoints.
    #[arg(long, default_value_t = 8)]
    pub(crate) depth: usize,

    /// Maximum ranked alternative paths to return.
    #[arg(long, default_value_t = 3)]
    pub(crate) paths: usize,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct RefactorContextArgs {
    /// Refactor target label or node id, for example: load_config or n42.
    pub(crate) target: String,

    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Optional journey start label or node id, for example an entrypoint.
    #[arg(long)]
    pub(crate) from: Option<String>,

    /// Maximum traversal depth for impact and journey.
    #[arg(long, default_value_t = 8)]
    pub(crate) depth: usize,

    /// Maximum ranked journey paths.
    #[arg(long, default_value_t = 3)]
    pub(crate) paths: usize,

    /// Maximum listed dependents.
    #[arg(long, default_value_t = 100)]
    pub(crate) dependent_limit: usize,

    /// Maximum bundled risks.
    #[arg(long, default_value_t = 50)]
    pub(crate) risk_limit: usize,

    /// Source preview context lines around the target span.
    #[arg(long, default_value_t = 6)]
    pub(crate) source_context: u32,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct PrImpactArgs {
    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Git base ref to diff against for the changed-file list (default: working tree changes).
    #[arg(long, default_value = "HEAD")]
    pub(crate) base: String,

    /// Explicit changed file overrides (repeatable); skips git entirely.
    #[arg(long = "file")]
    pub(crate) files: Vec<String>,

    /// CI state string recorded verbatim in the report, for example: passing or pending.
    #[arg(long)]
    pub(crate) ci_state: Option<String>,

    /// Review state string recorded verbatim in the report, for example: approved.
    #[arg(long)]
    pub(crate) review_state: Option<String>,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct QueryArgs {
    /// Query expression, for example: nodes kind:function label:main or path from:main to:init.
    pub(crate) expression: String,

    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    /// Collapse repeated low-signal nodes in the query result.
    #[arg(long)]
    pub(crate) compact: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}

#[derive(Debug, Args)]
pub(crate) struct AskArgs {
    /// Question, for example: "Where is DATABASE_URL read?".
    pub(crate) question: String,

    /// Project root to scan.
    #[arg(default_value = ".")]
    pub(crate) path: PathBuf,

    /// Include hidden files and directories.
    #[arg(long)]
    pub(crate) include_hidden: bool,

    /// Include default ignored directories such as target and node_modules.
    #[arg(long)]
    pub(crate) include_ignored: bool,

    /// Collapse repeated low-signal nodes in the query result.
    #[arg(long)]
    pub(crate) compact: bool,

    #[command(flatten)]
    pub(crate) cache: CacheArgs,
}
