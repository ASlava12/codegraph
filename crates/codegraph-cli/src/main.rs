use anyhow::Result;
use clap::{Parser, Subcommand};
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
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Scan {
            path,
            include_hidden,
            include_ignored,
        } => {
            let options = IndexOptions {
                include_hidden,
                include_ignored,
                ..IndexOptions::default()
            };
            let graph = scan_project(path, &options)?;
            println!("{}", serde_json::to_string_pretty(&graph)?);
        }
    }

    Ok(())
}
