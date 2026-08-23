//! Handlers for the semantic (LSP) commands: readiness, planning, batch
//! construction, running a batch, and turning responses into a graph patch.

use anyhow::Result;

use crate::*;

/// `codegraph semantic-readiness`
pub(crate) fn run_semantic_readiness(args: ScanArgs, max_file_size: Option<u64>) -> Result<()> {
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
    Ok(())
}

/// `codegraph semantic-plan`
pub(crate) fn run_semantic_plan(args: SemanticPlanArgs, max_file_size: Option<u64>) -> Result<()> {
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
    Ok(())
}

/// `codegraph semantic-batch`
pub(crate) fn run_semantic_batch(args: SemanticPlanArgs, max_file_size: Option<u64>) -> Result<()> {
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
    Ok(())
}

/// `codegraph semantic-run`
pub(crate) fn run_semantic_run(args: SemanticRunArgs, max_file_size: Option<u64>) -> Result<()> {
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
    Ok(())
}

/// `codegraph semantic-patch`
pub(crate) fn run_semantic_patch(
    args: SemanticPatchArgs,
    max_file_size: Option<u64>,
) -> Result<()> {
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
    Ok(())
}

/// `codegraph semantic-apply`
pub(crate) fn run_semantic_apply(
    args: SemanticPatchArgs,
    max_file_size: Option<u64>,
) -> Result<()> {
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
    let patch = semantic_graph_patch_from_responses(&workspace_root, &graph, &batch, &responses);
    println!(
        "{}",
        serde_json::to_string_pretty(&apply_semantic_graph_patch(&graph, &patch))?
    );
    Ok(())
}
