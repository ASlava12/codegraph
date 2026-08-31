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
            settle_timeout: std::time::Duration::from_millis(
                codegraph_lsp::DEFAULT_SEMANTIC_SETTLE_TIMEOUT_MS,
            ),
        },
    )?;
    // The array is the contract `semantic-patch` reads, so the count goes to
    // stderr beside it. A server that answers every request with nothing --
    // rust-analyzer does exactly that until it has finished loading a
    // workspace -- otherwise looks the same as one that found nothing to
    // say, and 297 empty answers scroll past as 297 answers.
    let answered = run
        .responses
        .iter()
        .filter(|response| response.error.is_none() && !result_is_empty(&response.result))
        .count();
    let failed = run
        .responses
        .iter()
        .filter(|response| response.error.is_some())
        .count();
    eprintln!(
        "{} responses: {answered} answered, {} empty, {failed} failed",
        run.responses.len(),
        run.responses.len() - answered - failed
    );
    println!("{}", serde_json::to_string_pretty(&run.responses)?);
    Ok(())
}

/// Whether a language server said nothing. `null`, `[]` and `{}` are all the
/// same answer: it has nothing for this position.
fn result_is_empty(result: &serde_json::Value) -> bool {
    match result {
        serde_json::Value::Null => true,
        serde_json::Value::Array(items) => items.is_empty(),
        serde_json::Value::Object(fields) => fields.is_empty(),
        _ => false,
    }
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

#[cfg(test)]
mod tests {
    use super::result_is_empty;
    use serde_json::json;

    #[test]
    fn a_server_that_says_nothing_is_not_a_server_that_answered() {
        // rust-analyzer replies `[]` to every request until it has finished
        // loading a workspace, and 297 of those scrolled past as 297
        // answers. All three spellings of nothing are nothing.
        assert!(result_is_empty(&json!(null)));
        assert!(result_is_empty(&json!([])));
        assert!(result_is_empty(&json!({})));
        assert!(!result_is_empty(&json!([{"uri": "file:///x.rs"}])));
        assert!(!result_is_empty(&json!({"uri": "file:///x.rs"})));
    }
}
