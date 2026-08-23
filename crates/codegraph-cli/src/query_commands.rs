//! Handlers for the query commands: the graph query language and the
//! natural-language `ask` front end.

use anyhow::Result;

use crate::*;

/// `codegraph query`
pub(crate) fn run_query(
    args: QueryArgs,
    max_file_size: Option<u64>,
    log_queries: bool,
) -> Result<()> {
    let QueryArgs {
        expression,
        path,
        include_hidden,
        include_ignored,
        compact,
        cache,
    } = args;

    validate_query_expression(&expression)?;
    let graph = scan_with_options(
        path.clone(),
        include_hidden,
        include_ignored,
        max_file_size,
        &cache,
    )?;
    let started = Instant::now();
    let result = query_graph(&graph, &expression);
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(result) => {
            let result = if compact {
                compact_query_result(result)
            } else {
                result
            };
            let output = serde_json::to_string_pretty(&result)?;
            log_cli_query(
                &path,
                log_queries,
                "query",
                &expression,
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
                "query",
                &expression,
                "error",
                duration_ms,
                None,
            );
            return Err(error.into());
        }
    }
    Ok(())
}

/// `codegraph ask`
pub(crate) fn run_ask(args: AskArgs, max_file_size: Option<u64>, log_queries: bool) -> Result<()> {
    let AskArgs {
        question,
        path,
        include_hidden,
        include_ignored,
        compact,
        cache,
    } = args;

    let graph = scan_with_options(
        path.clone(),
        include_hidden,
        include_ignored,
        max_file_size,
        &cache,
    )?;
    let query_text = question.clone();
    let started = Instant::now();
    let result = natural_query(&graph, NaturalQueryRequest { question, compact });
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(report) => {
            let output = serde_json::to_string_pretty(&report)?;
            log_cli_query(
                &path,
                log_queries,
                "ask",
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
                "ask",
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
