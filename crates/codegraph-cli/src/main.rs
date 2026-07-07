use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use codegraph_analysis::{
    ConfigTraceRequest, EntrypointTraceRequest, ErrorTraceRequest, ExplainEdgeRequest,
    InsightFilter, InsightSeverity, SourceSearchRequest, TraceRequest, TraceStart,
    architecture_map, check_insights, entrypoints, explain_edge, filter_insight_report, hotspots,
    insights, language_dependencies, query_graph, search_source, summarize, trace, trace_config,
    trace_dependents, trace_entrypoints, trace_errors,
};
use codegraph_analysis::{export_dot, export_ndjson};
use codegraph_indexer::{
    IndexOptionOverrides, configured_index_options, scan_coverage, scan_project,
};
use codegraph_lsp::{discover_lsp_servers, semantic_readiness};
use codegraph_parser::language_adapters;
use codegraph_storage::{GraphCache, default_cache_dir, scan_project_cached};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Instant;

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
