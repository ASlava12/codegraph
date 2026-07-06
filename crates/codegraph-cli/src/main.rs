use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use codegraph_analysis::{
    ConfigTraceRequest, EntrypointTraceRequest, ErrorTraceRequest, InsightFilter, InsightSeverity,
    TraceRequest, TraceStart, entrypoints, filter_insight_report, insights, query_graph, summarize,
    trace, trace_config, trace_entrypoints, trace_errors,
};
use codegraph_analysis::{export_dot, export_ndjson};
use codegraph_indexer::{IndexOptions, scan_project};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Parser)]
#[command(name = "codegraph")]
#[command(about = "Build and inspect code knowledge graphs")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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
    },

    /// Emit graph summary counts as JSON.
    Summary(ScanArgs),

    /// Benchmark project scans and emit timing plus graph size metrics as JSON.
    #[command(visible_alias = "bench")]
    Benchmark(BenchmarkArgs),

    /// Emit entrypoint candidate nodes as JSON.
    Entrypoints(ScanArgs),

    /// Emit investigation insights such as unresolved calls and error flows.
    Insights(InsightArgs),

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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan {
            path,
            include_hidden,
            include_ignored,
            format,
        } => {
            let graph = scan_with_options(path, include_hidden, include_ignored)?;
            print_graph(&graph, format)?;
        }
        Command::Summary(args) => {
            let graph = scan_with_options(args.path, args.include_hidden, args.include_ignored)?;
            println!("{}", serde_json::to_string_pretty(&summarize(&graph))?);
        }
        Command::Benchmark(args) => {
            let report = benchmark_scans(args)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::Entrypoints(args) => {
            let graph = scan_with_options(args.path, args.include_hidden, args.include_ignored)?;
            println!("{}", serde_json::to_string_pretty(&entrypoints(&graph))?);
        }
        Command::Insights(args) => {
            let graph = scan_with_options(
                args.scan.path,
                args.scan.include_hidden,
                args.scan.include_ignored,
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
        Command::Query {
            expression,
            path,
            include_hidden,
            include_ignored,
        } => {
            let graph = scan_with_options(path, include_hidden, include_ignored)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&query_graph(&graph, &expression)?)?
            );
        }
        Command::Trace {
            label,
            path,
            depth,
            include_hidden,
            include_ignored,
        } => {
            let graph = scan_with_options(path, include_hidden, include_ignored)?;
            let result = trace(
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
        } => {
            let graph = scan_with_options(path, include_hidden, include_ignored)?;
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
        } => {
            let graph = scan_with_options(path, include_hidden, include_ignored)?;
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
        } => {
            let graph = scan_with_options(path, include_hidden, include_ignored)?;
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

fn print_graph(graph: &codegraph_core::CodeGraph, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(graph)?),
        OutputFormat::Dot => print!("{}", export_dot(graph)),
        OutputFormat::Ndjson => print!("{}", export_ndjson(graph)?),
    }
    Ok(())
}

fn scan_with_options(
    path: PathBuf,
    include_hidden: bool,
    include_ignored: bool,
) -> Result<codegraph_core::CodeGraph> {
    let options = IndexOptions {
        include_hidden,
        include_ignored,
        ..IndexOptions::default()
    };
    Ok(scan_project(path, &options)?)
}

fn benchmark_scans(args: BenchmarkArgs) -> Result<BenchmarkReport> {
    let runs = args.runs.clamp(1, 100);
    let options = IndexOptions {
        include_hidden: args.include_hidden,
        include_ignored: args.include_ignored,
        ..IndexOptions::default()
    };
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
        fastest_ms,
        slowest_ms,
        average_ms,
        measurements,
        summary: summarize(&graph),
    })
}
