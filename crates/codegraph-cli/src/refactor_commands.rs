//! Handlers for change-safety commands: execution journeys, refactor context
//! bundles, and pull-request impact.

use anyhow::Result;

use crate::*;

/// `codegraph journey`
pub(crate) fn run_journey(
    args: JourneyArgs,
    max_file_size: Option<u64>,
    log_queries: bool,
) -> Result<()> {
    let JourneyArgs {
        from,
        to,
        path,
        depth,
        paths,
        include_hidden,
        include_ignored,
        cache,
    } = args;

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
    Ok(())
}

/// `codegraph refactor-context`
pub(crate) fn run_refactor_context(
    args: RefactorContextArgs,
    max_file_size: Option<u64>,
) -> Result<()> {
    let RefactorContextArgs {
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
    } = args;

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
    Ok(())
}

/// `codegraph pr-impact`
pub(crate) fn run_pr_impact(args: PrImpactArgs, max_file_size: Option<u64>) -> Result<()> {
    let PrImpactArgs {
        path,
        base,
        files,
        ci_state,
        review_state,
        include_hidden,
        include_ignored,
        cache,
    } = args;

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
    Ok(())
}
