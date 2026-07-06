use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use codegraph_analysis::{
    TraceRequest, TraceStart, entrypoints, insights, query_graph, summarize, trace,
};
use codegraph_analysis::{export_dot, export_ndjson};
use codegraph_indexer::{IndexOptions, scan_project};
use std::path::PathBuf;

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

    /// Emit entrypoint candidate nodes as JSON.
    Entrypoints(ScanArgs),

    /// Emit investigation insights such as unresolved calls and error flows.
    Insights(ScanArgs),

    /// Query focused graph slices as JSON.
    Query {
        /// Query expression, for example: nodes kind:function label:main.
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Dot,
    Ndjson,
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
        Command::Entrypoints(args) => {
            let graph = scan_with_options(args.path, args.include_hidden, args.include_ignored)?;
            println!("{}", serde_json::to_string_pretty(&entrypoints(&graph))?);
        }
        Command::Insights(args) => {
            let graph = scan_with_options(args.path, args.include_hidden, args.include_ignored)?;
            println!("{}", serde_json::to_string_pretty(&insights(&graph))?);
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
    }

    Ok(())
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
