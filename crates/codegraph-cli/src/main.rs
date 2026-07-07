use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use codegraph_analysis::{
    ConfigTraceRequest, EntrypointTraceRequest, ErrorTraceRequest, ExplainEdgeRequest,
    InsightFilter, InsightSeverity, ProjectReport, ProjectReportLimits, SourceSearchRequest,
    TraceRequest, TraceStart, architecture_map, check_insights, entrypoints, explain_edge,
    filter_insight_report, hotspots, insights, language_dependencies, project_report, query_graph,
    search_source, summarize, trace, trace_config, trace_dependents, trace_entrypoints,
    trace_errors,
};
use codegraph_analysis::{export_dot, export_ndjson, node_card};
use codegraph_core::NodeId;
use codegraph_indexer::{
    IndexOptionOverrides, configured_index_options, scan_coverage, scan_project,
};
use codegraph_lsp::{
    DEFAULT_SEMANTIC_WORK_ITEM_LIMIT, SemanticLspResponse, SemanticLspRunOptions,
    SemanticWorkItemFilter, apply_semantic_graph_patch, discover_lsp_servers,
    run_semantic_execution_batch, semantic_enrichment_plan_with_filter, semantic_execution_batch,
    semantic_graph_patch_from_responses, semantic_readiness,
};
use codegraph_parser::language_adapters;
use codegraph_storage::{GraphCache, default_cache_dir, scan_project_cached};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Parser)]
#[command(name = "codegraph")]
#[command(about = "Build and inspect code knowledge graphs")]
struct Cli {
    /// Maximum bytes to read from any single file during scans.
    #[arg(long, global = true)]
    max_file_size: Option<u64>,

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

    /// Emit a production-oriented project report snapshot as JSON.
    Report(ReportArgs),

    /// Emit a top-level architecture map grouped by project area.
    Architecture(ArchitectureArgs),

    /// Emit language-to-language dependency links as JSON.
    LanguageDependencies(LanguageDependencyArgs),

    /// Emit high-degree graph hotspots as JSON.
    Hotspots(HotspotArgs),

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
    IncrementalMergePreview(CacheDiffArgs),

    /// Emit entrypoint candidate nodes as JSON.
    Entrypoints(ScanArgs),

    /// Emit investigation insights such as unresolved calls and error flows.
    Insights(InsightArgs),

    /// Run insight checks and exit non-zero when findings meet a severity threshold.
    Check(CheckArgs),

    /// Query focused graph slices as JSON.
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

    /// Trace config files and environment variables back to readers and entrypoints.
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

    /// Maximum architecture groups to include.
    #[arg(long, default_value_t = 50)]
    architecture_group_limit: usize,

    /// Maximum architecture edges to include.
    #[arg(long, default_value_t = 200)]
    architecture_edge_limit: usize,

    /// Maximum language dependency links to include.
    #[arg(long, default_value_t = 50)]
    language_link_limit: usize,

    /// Maximum hotspots to include.
    #[arg(long, default_value_t = 25)]
    hotspot_limit: usize,

    /// Maximum insights to include while keeping full insight counts.
    #[arg(long, default_value_t = 50)]
    insight_limit: usize,

    /// Mark the quality gate as failed when an insight has this severity or higher.
    #[arg(long, value_enum, default_value = "error")]
    fail_on: InsightSeverityArg,
}

#[derive(Debug, Args)]
struct SemanticPlanArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum concrete semantic work items to include.
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

    /// Milliseconds to wait for each language-server response.
    #[arg(long, default_value_t = 30_000)]
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
struct HotspotArgs {
    #[command(flatten)]
    scan: ScanArgs,

    /// Maximum hotspots to include.
    #[arg(long, default_value_t = 25)]
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

    /// Numeric graph node id to inspect.
    #[arg(long)]
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
                    args.work_item_limit,
                    filter
                ))?
            );
        }
        Command::SemanticBatch(args) => {
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
                    args.work_item_limit,
                    filter
                ))?
            );
        }
        Command::SemanticRun(args) => {
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
            let batch = semantic_execution_batch(
                &workspace_root,
                &graph,
                args.plan.work_item_limit,
                filter,
            );
            let responses = run_semantic_execution_batch(
                &batch,
                &SemanticLspRunOptions {
                    request_timeout: std::time::Duration::from_millis(args.request_timeout_ms),
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&responses)?);
        }
        Command::SemanticPatch(args) => {
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
            let batch = semantic_execution_batch(
                &workspace_root,
                &graph,
                args.plan.work_item_limit,
                filter,
            );
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
            let batch = semantic_execution_batch(
                &workspace_root,
                &graph,
                args.plan.work_item_limit,
                filter,
            );
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
            let snapshot = build_project_report_snapshot(args, max_file_size)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
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
            let options = configured_index_options(
                &args.path,
                &scan_overrides(args.include_hidden, args.include_ignored, max_file_size),
            )?;
            let cache = GraphCache::new(args.cache_dir.unwrap_or_else(default_cache_dir));
            let preview = cache.incremental_merge_preview(&args.path, &options, args.limit)?;
            println!("{}", serde_json::to_string_pretty(&preview)?);
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
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&query_graph(&graph, &expression)?)?
            );
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
                NodeId(args.node_id),
                args.edge_limit,
                args.source_context,
                args.insight_limit,
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

fn report_limits_from_args(args: &ReportArgs) -> ProjectReportLimits {
    ProjectReportLimits {
        architecture_group_limit: args.architecture_group_limit,
        architecture_edge_limit: args.architecture_edge_limit,
        language_link_limit: args.language_link_limit,
        hotspot_limit: args.hotspot_limit,
        insight_limit: args.insight_limit,
        fail_on: InsightSeverity::from(args.fail_on),
    }
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
