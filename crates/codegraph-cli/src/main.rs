//! The `codegraph` command-line interface: scan, query, trace, workflow,
//! journey, refactoring, insight, report, export, cache, registry, watch,
//! memory, and MCP commands over the shared analysis crates.

mod bench_context;
mod cli;
mod hooks;
mod install;
mod mcp;
use codegraph_analysis::memory;
mod merge;
use codegraph_analysis::pr_impact;
mod query_commands;
mod query_log;
mod refactor_commands;
mod registry;
mod repository;
mod semantic_commands;
mod watch;
mod wiki;
mod workflow_commands;

use anyhow::Result;
use clap::Parser;
use cli::*;
use codegraph_analysis::{
    ComponentContractRequest, ComponentDependencyRequest, ConfigTraceRequest,
    EntrypointTraceRequest, ErrorTraceRequest, ExplainEdgeRequest, ImpactRequest, InsightFilter,
    InsightSeverity, JourneyRequest, NaturalQueryRequest, NodeCardRequest, ProjectReport,
    ProjectReportLimits, ProjectReportMarkdownOptions, RefactorContextRequest, SeamRequest,
    SourceSearchRequest, TraceRequest, TraceStart, architecture_map, check_insights, communities,
    compact_query_result, component_contract, component_dependencies, entrypoints, explain_edge,
    filter_insight_report, graph_health, hotspots, impact, impact_fast, insights, journey,
    language_dependencies, missing_node_error, natural_query, project_report,
    project_report_markdown, query_graph, read_source_preview, refactor_context, seams,
    search_source, suggested_questions, summarize, surprising_links, trace, trace_config,
    trace_dependents, trace_entrypoints, trace_errors, validate_query_expression,
};
use codegraph_analysis::{
    DEFAULT_MERMAID_EDGE_LIMIT, DEFAULT_MERMAID_NODE_LIMIT, DEFAULT_SVG_EDGE_LIMIT,
    DEFAULT_SVG_NODE_LIMIT, export_cypher, export_dot, export_falkordb, export_graph_mermaid_html,
    export_graphml, export_ndjson, export_svg, node_card, node_card_fast,
};
use codegraph_core::NodeId;
use codegraph_indexer::{
    IndexOptionOverrides, configured_index_options, scan_coverage, scan_project,
};
use codegraph_lsp::{
    AutoEnrichmentOptions, AutoEnrichmentReport, SemanticLspCache, SemanticLspCacheStatus,
    SemanticLspResponse, SemanticLspRunOptions, SemanticWorkItemFilter, apply_semantic_graph_patch,
    auto_enrich_graph, discover_lsp_servers, normalize_semantic_request_timeout_ms,
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
            semantic_commands::run_semantic_readiness(args, max_file_size)?
        }
        Command::SemanticPlan(args) => semantic_commands::run_semantic_plan(args, max_file_size)?,
        Command::SemanticBatch(args) => semantic_commands::run_semantic_batch(args, max_file_size)?,
        Command::SemanticRun(args) => semantic_commands::run_semantic_run(args, max_file_size)?,
        Command::SemanticPatch(args) => semantic_commands::run_semantic_patch(args, max_file_size)?,
        Command::SemanticApply(args) => semantic_commands::run_semantic_apply(args, max_file_size)?,
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
        Command::Doctor(args) => {
            let graph = scan_with_options(
                args.path,
                args.include_hidden,
                args.include_ignored,
                max_file_size,
                &args.cache,
            )?;
            println!("{}", serde_json::to_string_pretty(&graph_health(&graph))?);
        }
        Command::Questions(args) => {
            let graph = scan_with_options(
                args.path,
                args.include_hidden,
                args.include_ignored,
                max_file_size,
                &args.cache,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&suggested_questions(&graph))?
            );
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
        Command::PrImpact(args) => refactor_commands::run_pr_impact(args, max_file_size)?,
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
        Command::Query(args) => query_commands::run_query(args, max_file_size, log_queries)?,
        Command::Journey(args) => refactor_commands::run_journey(args, max_file_size, log_queries)?,
        Command::InstallAgent {
            path,
            platform,
            force,
            hooks,
        } => {
            let report = install::install_agent(&path, platform, force, hooks)?;
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
            // A memory may hold either form, so both answer to the same node.
            let node_labels = graph
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
        Command::RefactorContext(args) => {
            refactor_commands::run_refactor_context(args, max_file_size)?
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
            include_risks,
            include_hidden,
            include_ignored,
            cache,
        } => {
            let graph =
                scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
            let impact_builder = if include_risks { impact } else { impact_fast };
            let report = impact_builder(
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
        Command::Ask(args) => query_commands::run_ask(args, max_file_size, log_queries)?,
        Command::NodeCard(args) => {
            let graph = scan_with_options(
                args.scan.path.clone(),
                args.scan.include_hidden,
                args.scan.include_ignored,
                max_file_size,
                &args.scan.cache,
            )?;
            let card_builder = if args.include_insights {
                node_card
            } else {
                node_card_fast
            };
            // The durable `cg-*` id is what an agent saved; resolving it
            // needs the graph, so it happens here rather than in the flag.
            let (node_id, note) = resolve_node_id(&graph, &args.node_id)?;
            let mut card = card_builder(
                &graph,
                Some(&args.scan.path),
                NodeCardRequest {
                    node_id,
                    edge_limit: args.edge_limit,
                    source_context: args.source_context,
                    insight_limit: args.insight_limit,
                },
            )?
            .ok_or_else(|| anyhow::anyhow!("node {} not found", args.node_id))?;
            card.notes.extend(note);
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
            let target = label.clone();
            let result = trace(
                &graph,
                TraceRequest {
                    start: TraceStart::Label(label),
                    max_depth: depth,
                },
            );
            // A name that matched nothing is not a trace with no steps.
            let result = result.ok_or_else(|| {
                anyhow::anyhow!("{}", missing_node_error(&graph, "trace start", &target))
            })?;
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
            let target = label.clone();
            let result = trace_dependents(
                &graph,
                TraceRequest {
                    start: TraceStart::Label(label),
                    max_depth: depth,
                },
            );
            let result = result.ok_or_else(|| {
                anyhow::anyhow!("{}", missing_node_error(&graph, "trace start", &target))
            })?;
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
        Command::Workflow(args) => workflow_commands::run_workflow(args, max_file_size)?,
        Command::WorkflowEntrypoints(args) => {
            workflow_commands::run_workflow_entrypoints(args, max_file_size)?
        }
        Command::WorkflowQuery(args) => workflow_commands::run_workflow_query(args, max_file_size)?,
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
        OutputFormat::Svg => print!(
            "{}",
            export_svg(graph, DEFAULT_SVG_NODE_LIMIT, DEFAULT_SVG_EDGE_LIMIT)
        ),
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
/// The node a `--node-id` names: a numeric or n-prefixed id, or the
/// durable `cg-*` one, which only the graph can resolve.
/// The node a `--node-id` names, and what choosing it decided. `impact` and
/// `journey` both take a label; this took an id and said `invalid node id
/// \`main\`` to one, including the labels its own suggested commands hand
/// back.
fn resolve_node_id(
    graph: &codegraph_core::CodeGraph,
    value: &str,
) -> Result<(NodeId, Option<String>)> {
    codegraph_analysis::resolve_node_reference_with_note(graph, value).ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            codegraph_analysis::missing_node_error(graph, "node", value)
        )
    })
}

/// A node reference to store: the durable `cg-*` id the scan stamps, or the
/// positional form. Kept as written, because a memory outlives the scan and
/// only the durable id still points at the same definition.
pub(crate) fn parse_cli_node_reference(value: &str) -> Result<String, String> {
    let value = value.trim();
    if codegraph_analysis::parse_node_id(value).is_ok()
        || (value.starts_with("cg-") && value.len() > 3)
    {
        return Ok(value.to_string());
    }
    Err(format!(
        "invalid node id `{value}`; expected a durable cg-* id, `n42`, or `42`"
    ))
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

pub(crate) fn scan_with_options(
    path: PathBuf,
    include_hidden: bool,
    include_ignored: bool,
    max_file_size: Option<u64>,
    cache_args: &CacheArgs,
) -> Result<codegraph_core::CodeGraph> {
    // Every command takes a path, so resolving a URL here gives all of them
    // a repository they do not have yet: `codegraph summary
    // https://github.com/owner/repo` clones it once under the cache and
    // reads it from there afterwards.
    let path = match path.to_str() {
        Some(target) if repository::is_remote(target) => repository::repository_path(target)?,
        _ => path,
    };
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
    scan_with_cache_status(path, options, cache_args, cache).map(|(graph, _)| graph)
}

/// The graph a command answers from, with the cache status the scan
/// reported. `report` needs both, and reaching past this for the cache
/// status is how it came to answer from a graph the semantic pass never
/// touched while `insights` beside it answered from one that had.
fn scan_with_cache_status(
    path: PathBuf,
    options: codegraph_indexer::IndexOptions,
    cache_args: &CacheArgs,
    cache: Option<GraphCache>,
) -> Result<(codegraph_core::CodeGraph, codegraph_storage::CacheInfo)> {
    let scanned = scan_project_cached(&path, &options, cache.as_ref())?;
    let (graph, enrichment) = auto_enrich_graph(
        &path,
        scanned.graph,
        semantic_cache_from_args(cache_args).as_ref(),
        &AutoEnrichmentOptions {
            enabled: !cache_args.no_semantic,
            ..AutoEnrichmentOptions::default()
        },
    );
    report_semantic_pass(&enrichment);
    Ok((graph, scanned.cache))
}

/// Say what the automatic semantic pass did. It is bounded so a scan stays
/// fast, and the bound is small next to what it could ask -- 100 of 49649
/// on this repository -- so a graph it touched is a sampled graph and the
/// line says so. The report goes to stderr, since stdout is the graph.
fn report_semantic_pass(report: &AutoEnrichmentReport) {
    if !report.applied {
        return;
    }
    let servers = if report.servers.is_empty() {
        String::from("no server")
    } else {
        report.servers.join(", ")
    };
    let cache = match report.cache {
        SemanticLspCacheStatus::Hit => ", cache hit",
        SemanticLspCacheStatus::Miss => ", asked the server",
        SemanticLspCacheStatus::Disabled => "",
    };
    eprintln!(
        "semantic: {} edges, {} upgraded, from {} of {} candidates ({servers}{cache})",
        report.semantic_edges,
        report.replaced_edges,
        report.requested_work_items,
        report.total_work_items
    );
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
    let coverage = scan_coverage(&args.scan.path, &options)?;
    let (graph, cache_info) =
        scan_with_cache_status(args.scan.path.clone(), options, &args.scan.cache, cache)?;
    let report = project_report(&graph, report_limits_from_args(&args));

    Ok(ProjectReportSnapshot {
        root: args.scan.path.display().to_string(),
        generated_at_unix: unix_seconds(),
        cache: cache_info,
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
