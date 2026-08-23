//! Handlers for the workflow command family: a single workflow, workflows for
//! every entrypoint, and workflows for a query's result nodes.

use anyhow::Result;
use codegraph_analysis::{
    EntrypointWorkflowRequest, MermaidSection, TraceStart, WorkflowFilters, WorkflowQueryRequest,
    WorkflowRequest, export_mermaid_html, workflow, workflow_entrypoints, workflow_mermaid,
    workflow_query,
};

use crate::cli::{WorkflowArgs, WorkflowEntrypointsArgs, WorkflowFormat, WorkflowQueryArgs};
use crate::scan_with_options;

/// `codegraph workflow`
pub(crate) fn run_workflow(args: WorkflowArgs, max_file_size: Option<u64>) -> Result<()> {
    let WorkflowArgs {
        label,
        path,
        depth,
        block_limit,
        edge_kind,
        confidence,
        language,
        risk_severity,
        block_kind,
        compact,
        max_fanout,
        format,
        include_hidden,
        include_ignored,
        cache,
    } = args;

    let graph = scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label(label),
            max_depth: depth,
            block_limit,
            filters: WorkflowFilters::from_parts(
                edge_kind,
                confidence,
                language,
                risk_severity,
                block_kind,
            ),
            compact,
            max_fanout,
        },
    );
    match (format, report) {
        (WorkflowFormat::Json, report) => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        (WorkflowFormat::Mermaid, Some(report)) => {
            println!("{}", workflow_mermaid(&report));
        }
        (WorkflowFormat::Mermaid, None) => {
            println!("flowchart TD");
        }
        (WorkflowFormat::Html, report) => {
            let sections = report
                .iter()
                .map(|report| MermaidSection {
                    title: report.start.label.clone(),
                    mermaid: workflow_mermaid(report),
                })
                .collect::<Vec<_>>();
            print!("{}", export_mermaid_html("CodeGraph workflow", &sections));
        }
    }
    Ok(())
}

/// `codegraph workflow-entrypoints`
pub(crate) fn run_workflow_entrypoints(
    args: WorkflowEntrypointsArgs,
    max_file_size: Option<u64>,
) -> Result<()> {
    let WorkflowEntrypointsArgs {
        search,
        entrypoint_kind,
        path,
        depth,
        block_limit,
        max_fanout,
        limit,
        edge_kind,
        confidence,
        language,
        risk_severity,
        block_kind,
        compact,
        format,
        include_hidden,
        include_ignored,
        cache,
    } = args;

    let graph = scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
    let report = workflow_entrypoints(
        &graph,
        EntrypointWorkflowRequest {
            search,
            entrypoint_kind,
            max_depth: depth,
            block_limit,
            limit,
            filters: WorkflowFilters::from_parts(
                edge_kind,
                confidence,
                language,
                risk_severity,
                block_kind,
            ),
            compact,
            max_fanout,
        },
    );
    match format {
        WorkflowFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        WorkflowFormat::Mermaid => {
            let rendered = report
                .workflows
                .iter()
                .map(|workflow| {
                    format!(
                        "%% {}\n{}",
                        workflow.start.label,
                        workflow_mermaid(workflow)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            println!("{rendered}");
        }
        WorkflowFormat::Html => {
            let sections = report
                .workflows
                .iter()
                .map(|workflow| MermaidSection {
                    title: workflow.start.label.clone(),
                    mermaid: workflow_mermaid(workflow),
                })
                .collect::<Vec<_>>();
            print!("{}", export_mermaid_html("CodeGraph workflows", &sections));
        }
    }
    Ok(())
}

/// `codegraph workflow-query`
pub(crate) fn run_workflow_query(
    args: WorkflowQueryArgs,
    max_file_size: Option<u64>,
) -> Result<()> {
    let WorkflowQueryArgs {
        query,
        path,
        depth,
        block_limit,
        max_fanout,
        limit,
        edge_kind,
        confidence,
        language,
        risk_severity,
        block_kind,
        compact,
        format,
        include_hidden,
        include_ignored,
        cache,
    } = args;

    let graph = scan_with_options(path, include_hidden, include_ignored, max_file_size, &cache)?;
    let report = workflow_query(
        &graph,
        WorkflowQueryRequest {
            query,
            max_depth: depth,
            block_limit,
            limit,
            filters: WorkflowFilters::from_parts(
                edge_kind,
                confidence,
                language,
                risk_severity,
                block_kind,
            ),
            compact,
            max_fanout,
        },
    )?;
    match format {
        WorkflowFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        WorkflowFormat::Mermaid => {
            let rendered = report
                .workflows
                .iter()
                .map(|workflow| {
                    format!(
                        "%% {}\n{}",
                        workflow.start.label,
                        workflow_mermaid(workflow)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            println!("{rendered}");
        }
        WorkflowFormat::Html => {
            let sections = report
                .workflows
                .iter()
                .map(|workflow| MermaidSection {
                    title: workflow.start.label.clone(),
                    mermaid: workflow_mermaid(workflow),
                })
                .collect::<Vec<_>>();
            print!("{}", export_mermaid_html("CodeGraph workflows", &sections));
        }
    }
    Ok(())
}
