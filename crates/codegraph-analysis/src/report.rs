//! The bounded project report snapshot: risk summary, quality gate,
//! topology, compact file/node summaries, and Markdown rendering.

use codegraph_core::{CodeGraph, Edge, EdgeKind, Node, NodeId, NodeKind, is_vendored_source_path};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

#[allow(unused_imports)]
use crate::*;

/// The durable id of every node the kept insights cite. Only those: a
/// report on a large project holds tens of thousands of findings before the
/// limit, and a table of all their nodes would dwarf the report.
fn durable_node_ids(graph: &CodeGraph, report: &InsightReport) -> BTreeMap<String, String> {
    let cited: BTreeSet<NodeId> = report
        .insights
        .iter()
        .flat_map(|insight| insight.nodes.iter().copied())
        .collect();
    graph
        .nodes
        .iter()
        .filter(|node| cited.contains(&node.id))
        .filter_map(|node| {
            node.metadata
                .get("stable_id")
                .map(|stable_id| (node.id.to_string(), stable_id.clone()))
        })
        .collect()
}

pub fn project_report(graph: &CodeGraph, limits: ProjectReportLimits) -> ProjectReport {
    let limits = normalize_project_report_limits(limits);
    let full_insight_report = insights(graph);
    let risk_summary = project_risk_summary(&full_insight_report);
    let quality_gate = bounded_quality_gate(
        check_insights(full_insight_report.clone(), limits.fail_on),
        REPORT_QUALITY_GATE_SAMPLE_LIMIT,
    );
    let file_summaries =
        compact_file_summaries(graph, &full_insight_report, limits.file_summary_limit);
    let node_summaries =
        compact_node_summaries(graph, &full_insight_report, limits.node_summary_limit);
    let insight_report = filter_insight_report(
        full_insight_report,
        &InsightFilter {
            severity: None,
            kind: None,
            search: None,
            limit: limits.insight_limit,
        },
    );

    let durable_node_ids = durable_node_ids(graph, &insight_report);

    ProjectReport {
        graph_schema_version: graph.schema_version,
        summary: summarize(graph),
        entrypoints: entrypoints(graph),
        insights: insight_report,
        risk_summary,
        quality_gate,
        architecture: architecture_map(
            graph,
            limits.architecture_group_limit,
            limits.architecture_edge_limit,
        ),
        language_dependencies: language_dependencies(graph, limits.language_link_limit),
        surprising_links: surprising_links(graph, limits.architecture_edge_limit),
        hotspots: hotspots(graph, limits.hotspot_limit),
        communities: communities(graph, limits.community_limit),
        file_summaries,
        durable_node_ids,
        node_summaries,
    }
}

/// How a committed report cites a node: by the durable id when the scan
/// stamped one, because the positional `n42` moves when a file is edited
/// above the node and this document is read weeks later.
fn durable_or_positional(report: &ProjectReport, node: NodeId) -> String {
    let positional = node.to_string();
    report
        .durable_node_ids
        .get(&positional)
        .cloned()
        .unwrap_or(positional)
}

/// The durable id a node carries, for the tables that hold whole nodes.
fn node_reference(node: &Node) -> String {
    node.metadata
        .get("stable_id")
        .cloned()
        .unwrap_or_else(|| node.id.to_string())
}

pub fn project_report_markdown(
    report: &ProjectReport,
    options: &ProjectReportMarkdownOptions,
) -> String {
    let mut output = String::new();
    writeln!(output, "# {}", markdown_text(&options.title)).unwrap();
    writeln!(output).unwrap();
    if let Some(root) = options.root.as_deref().filter(|value| !value.is_empty()) {
        writeln!(output, "- Root: `{}`", markdown_code(root)).unwrap();
    }
    if let Some(generated_at_unix) = options.generated_at_unix {
        writeln!(output, "- Generated at unix: `{generated_at_unix}`").unwrap();
    }
    writeln!(
        output,
        "- Graph schema version: `{}`",
        report.graph_schema_version
    )
    .unwrap();
    writeln!(
        output,
        "- Quality gate: **{}** (`fail_on={}`, failing_insights={})",
        if report.quality_gate.passed {
            "passed"
        } else {
            "failed"
        },
        markdown_code(&report.quality_gate.fail_on),
        report.quality_gate.failing_insights
    )
    .unwrap();
    writeln!(
        output,
        "- Risk: **{}** (score {}, total {}, errors {}, warnings {}, infos {})",
        markdown_text(&report.risk_summary.grade),
        report.risk_summary.score,
        report.risk_summary.total,
        report.risk_summary.errors,
        report.risk_summary.warnings,
        report.risk_summary.infos
    )
    .unwrap();

    writeln!(output, "\n## Summary").unwrap();
    writeln!(output, "| Metric | Count |").unwrap();
    writeln!(output, "| --- | ---: |").unwrap();
    writeln!(output, "| Nodes | {} |", report.summary.nodes).unwrap();
    writeln!(output, "| Edges | {} |", report.summary.edges).unwrap();
    writeln!(output, "| Entrypoints | {} |", report.summary.entrypoints).unwrap();
    writeln!(
        output,
        "| Skipped files | {} |",
        report.summary.skipped_files
    )
    .unwrap();

    write_count_table(
        &mut output,
        "Languages",
        "Language",
        &report.summary.languages,
        12,
    );
    write_count_table(
        &mut output,
        "Node Kinds",
        "Kind",
        &report.summary.node_kinds,
        12,
    );
    write_count_table(
        &mut output,
        "Edge Confidence",
        "Confidence",
        &report.summary.edge_confidences,
        12,
    );

    writeln!(output, "\n## Compact Node Summaries").unwrap();
    if report.node_summaries.nodes.is_empty() {
        writeln!(output, "No node summaries were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | Node | Kind | Roles | In | Out | Risks | Edge kinds | Source |"
        )
        .unwrap();
        writeln!(
            output,
            "| ---: | --- | --- | --- | ---: | ---: | --- | --- | --- |"
        )
        .unwrap();
        for node in report.node_summaries.nodes.iter().take(20) {
            writeln!(
                output,
                "| {} | {} | `{}` | {} | {} | {} | {} | {} | {} |",
                node.score,
                node_ref(&node.node),
                markdown_code(&kind_name(&node.node.kind)),
                markdown_table_cell(&node.roles.join(", ")),
                node.dependency_summary.incoming,
                node.dependency_summary.outgoing,
                count_map_inline(&node.insight_summary.by_severity, 4),
                count_map_inline(&node.dependency_summary.edge_kinds, 5),
                node_span_ref(&node.node)
            )
            .unwrap();
        }
        if report.node_summaries.truncated {
            writeln!(
                output,
                "\nNode summaries are truncated: showing {} of {} important nodes.",
                report.node_summaries.nodes.len(),
                report.node_summaries.total_nodes
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Compact File Summaries").unwrap();
    if report.file_summaries.files.is_empty() {
        writeln!(output, "No file summaries were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | File | Symbols | Trace | Imports | Config | Env | Errors | Unresolved | Risks | Trace kinds |"
        )
        .unwrap();
        writeln!(
            output,
            "| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |"
        )
        .unwrap();
        for file in report.file_summaries.files.iter().take(20) {
            writeln!(
                output,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                file.score,
                node_ref(&file.node),
                file.summary.code_symbols,
                file.summary.trace_edges,
                file.summary.imports,
                file.summary.config_reads,
                file.summary.environment_reads,
                file.summary.error_facts,
                file.summary.unresolved_calls,
                count_map_inline(&file.insight_summary.by_severity, 4),
                count_map_inline(&file.summary.trace_edge_kinds, 5)
            )
            .unwrap();
        }
        if report.file_summaries.truncated {
            writeln!(
                output,
                "\nFile summaries are truncated: showing {} of {} files.",
                report.file_summaries.files.len(),
                report.file_summaries.total_files
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Confidence Guide").unwrap();
    writeln!(
        output,
        "| CodeGraph confidence | Report wording | How to read it |"
    )
    .unwrap();
    writeln!(output, "| --- | --- | --- |").unwrap();
    writeln!(
        output,
        "| `exact` | extracted | Exact project metadata, deterministic manifests, or compiler-like facts. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `semantic` | resolved | Resolved through a semantic analyzer or language server. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `syntactic` | extracted from syntax | Extracted directly from source syntax. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `heuristic` | inferred | Inferred by a named rule and worth reviewing at architectural or runtime boundaries. |"
    )
    .unwrap();
    writeln!(
        output,
        "| `unknown` | ambiguous | Legacy, imported, or ambiguous evidence. |"
    )
    .unwrap();

    writeln!(output, "\n## Key Concepts").unwrap();
    let key_hotspots = if report.hotspots.architectural_hubs.is_empty() {
        &report.hotspots.hotspots
    } else {
        &report.hotspots.architectural_hubs
    };
    if key_hotspots.is_empty() {
        writeln!(output, "No architectural hub candidates were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | Node | Kind | Hub kind | In | Out | Edge kinds |"
        )
        .unwrap();
        writeln!(output, "| ---: | --- | --- | --- | ---: | ---: | --- |").unwrap();
        for hotspot in key_hotspots.iter().take(15) {
            writeln!(
                output,
                "| {} | {} | `{}` | `{}` | {} | {} | {} |",
                hotspot.score,
                node_ref(&hotspot.node),
                markdown_code(&kind_name(&hotspot.node.kind)),
                markdown_code(&hotspot.hub_kind),
                hotspot.incoming,
                hotspot.outgoing,
                count_map_inline(&hotspot.edge_kinds, 6)
            )
            .unwrap();
        }
        if report.hotspots.truncated {
            writeln!(
                output,
                "\nHotspots are truncated: showing {} key hubs out of {} candidates ({} architectural, {} utility).",
                key_hotspots.len(),
                report.hotspots.total_candidates,
                report.hotspots.total_architectural_hubs,
                report.hotspots.total_utility_hubs
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Communities").unwrap();
    if report.communities.communities.is_empty() {
        writeln!(output, "No graph communities were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Community | Nodes | Files | Entrypoints | Internal edges | External edges | Languages | Evidence |"
        )
        .unwrap();
        writeln!(
            output,
            "| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |"
        )
        .unwrap();
        for community in report.communities.communities.iter().take(12) {
            let external_edges =
                community.incoming_external_edges + community.outgoing_external_edges;
            writeln!(
                output,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
                markdown_code(&community.label),
                community.node_count,
                community.files,
                community.entrypoints,
                community.internal_edges,
                external_edges,
                count_map_inline(&community.languages, 5),
                edge_index_refs(&community.edge_indexes, 5)
            )
            .unwrap();
        }
        if report.communities.truncated {
            writeln!(
                output,
                "\nCommunities are truncated: showing {} of {} communities.",
                report.communities.communities.len(),
                report.communities.total_communities
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Entrypoints").unwrap();
    if report.entrypoints.is_empty() {
        writeln!(output, "No entrypoint candidates were found.").unwrap();
    } else {
        writeln!(output, "| Node | Kind | Source |").unwrap();
        writeln!(output, "| --- | --- | --- |").unwrap();
        for node in report.entrypoints.iter().take(20) {
            writeln!(
                output,
                "| {} | `{}` | {} |",
                node_ref(node),
                markdown_code(&kind_name(&node.kind)),
                node_span_ref(node)
            )
            .unwrap();
        }
        if report.entrypoints.len() > 20 {
            writeln!(
                output,
                "\nEntrypoints are truncated: showing 20 of {}.",
                report.entrypoints.len()
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Architecture Links").unwrap();
    if report.architecture.edges.is_empty() {
        writeln!(output, "No cross-area architecture links were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Source | Target | Count | Edge kinds | Confidence | Evidence |"
        )
        .unwrap();
        writeln!(output, "| --- | --- | ---: | --- | --- | --- |").unwrap();
        for edge in report.architecture.edges.iter().take(15) {
            writeln!(
                output,
                "| `{}` | `{}` | {} | {} | {} | {} |",
                markdown_code(&edge.source),
                markdown_code(&edge.target),
                edge.count,
                count_map_inline(&edge.edge_kinds, 5),
                count_map_inline(&edge.confidences, 5),
                edge_index_refs(&edge.edge_indexes, 5)
            )
            .unwrap();
        }
        if report.architecture.truncated_edges {
            writeln!(
                output,
                "\nArchitecture links are truncated: showing {} of {} links.",
                report.architecture.edges.len(),
                report.architecture.total_edges
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Surprising Links").unwrap();
    if report.surprising_links.links.is_empty() {
        writeln!(output, "No surprising dependency links were found.").unwrap();
    } else {
        writeln!(
            output,
            "| Score | Source | Target | Areas | Languages | Edge | Confidence | Reasons | Evidence |"
        )
        .unwrap();
        writeln!(
            output,
            "| ---: | --- | --- | --- | --- | --- | --- | --- | --- |"
        )
        .unwrap();
        for link in report.surprising_links.links.iter().take(15) {
            writeln!(
                output,
                "| {} | {} | {} | `{}` -> `{}` | `{}` -> `{}` | `{}` | `{}` | {} | #{} |",
                link.score,
                node_ref(&link.source),
                node_ref(&link.target),
                markdown_code(&link.source_area),
                markdown_code(&link.target_area),
                markdown_code(&link.source_language),
                markdown_code(&link.target_language),
                markdown_code(&link.edge_kind),
                markdown_code(&link.confidence),
                markdown_table_cell(&link.reasons.join(", ")),
                link.edge_index
            )
            .unwrap();
        }
        if report.surprising_links.truncated {
            writeln!(
                output,
                "\nSurprising links are truncated: showing {} of {} candidates.",
                report.surprising_links.links.len(),
                report.surprising_links.total_candidates
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Risks And Insights").unwrap();
    if report.risk_summary.top_kinds.is_empty() {
        writeln!(output, "No investigation insights were reported.").unwrap();
    } else {
        writeln!(output, "| Kind | Severity | Count |").unwrap();
        writeln!(output, "| --- | --- | ---: |").unwrap();
        for risk in &report.risk_summary.top_kinds {
            writeln!(
                output,
                "| `{}` | `{}` | {} |",
                markdown_code(&risk.kind),
                markdown_code(&risk.severity),
                risk.count
            )
            .unwrap();
        }
    }
    if !report.insights.insights.is_empty() {
        writeln!(output, "\n### Insight Evidence").unwrap();
        writeln!(output, "| Severity | Kind | Message | Evidence |").unwrap();
        writeln!(output, "| --- | --- | --- | --- |").unwrap();
        for insight in report.insights.insights.iter().take(20) {
            let node_refs = insight
                .nodes
                .iter()
                .map(|node| durable_or_positional(report, *node))
                .collect::<Vec<_>>()
                .join(", ");
            let evidence = if insight.edges.is_empty() {
                markdown_table_cell(&node_refs)
            } else if node_refs.is_empty() {
                edge_index_refs(&insight.edges, 8)
            } else {
                markdown_table_cell(&format!(
                    "nodes: {node_refs}; edges: {}",
                    edge_index_refs(&insight.edges, 8)
                ))
            };
            writeln!(
                output,
                "| `{}` | `{}` | {} | {} |",
                markdown_code(severity_name(insight.severity)),
                markdown_code(&insight.kind),
                markdown_table_cell(&insight.message),
                evidence
            )
            .unwrap();
        }
        if report.insights.insights.len() < report.insights.total {
            writeln!(
                output,
                "\nInsights are truncated: showing {} of {}.",
                report.insights.insights.len(),
                report.insights.total
            )
            .unwrap();
        }
    }

    writeln!(output, "\n## Suggested Questions").unwrap();
    for question in project_report_suggested_questions(report).iter().take(6) {
        writeln!(output, "- {}", markdown_text(question)).unwrap();
    }

    output
}

pub(crate) fn write_count_table(
    output: &mut String,
    title: &str,
    label: &str,
    values: &BTreeMap<String, usize>,
    limit: usize,
) {
    writeln!(output, "\n### {}", markdown_text(title)).unwrap();
    if values.is_empty() {
        writeln!(output, "No {} values were found.", title.to_lowercase()).unwrap();
        return;
    }
    writeln!(output, "| {} | Count |", markdown_table_cell(label)).unwrap();
    writeln!(output, "| --- | ---: |").unwrap();
    for (key, count) in sorted_count_entries(values).into_iter().take(limit) {
        writeln!(output, "| `{}` | {} |", markdown_code(key), count).unwrap();
    }
    if values.len() > limit {
        writeln!(output, "\nShowing {} of {} values.", limit, values.len()).unwrap();
    }
}

pub(crate) fn sorted_count_entries(values: &BTreeMap<String, usize>) -> Vec<(&String, &usize)> {
    let mut entries: Vec<_> = values.iter().collect();
    entries.sort_by(|(left_key, left_count), (right_key, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_key.cmp(right_key))
    });
    entries
}

pub(crate) fn count_map_inline(values: &BTreeMap<String, usize>, limit: usize) -> String {
    if values.is_empty() {
        return "-".to_string();
    }
    let mut parts: Vec<_> = sorted_count_entries(values)
        .into_iter()
        .take(limit)
        .map(|(key, count)| format!("`{}`={count}", markdown_code(key)))
        .collect();
    if values.len() > limit {
        parts.push(format!("+{} more", values.len() - limit));
    }
    markdown_table_cell(&parts.join(", "))
}

pub(crate) fn edge_index_refs(indexes: &[usize], limit: usize) -> String {
    if indexes.is_empty() {
        return "-".to_string();
    }
    let mut refs: Vec<_> = indexes
        .iter()
        .take(limit)
        .map(|index| format!("#{index}"))
        .collect();
    if indexes.len() > limit {
        refs.push(format!("+{} more", indexes.len() - limit));
    }
    markdown_table_cell(&refs.join(", "))
}

pub(crate) fn node_ref(node: &Node) -> String {
    markdown_table_cell(&format!(
        "`{}` `{}`",
        node_reference(node),
        markdown_code(&node.label)
    ))
}

pub(crate) fn node_span_ref(node: &Node) -> String {
    node.span
        .as_ref()
        .map(|span| {
            markdown_table_cell(&format!(
                "`{}:{}-{}`",
                markdown_code(&span.path),
                span.start_line,
                span.end_line
            ))
        })
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn project_report_suggested_questions(report: &ProjectReport) -> Vec<String> {
    let mut questions = Vec::new();
    if let Some(entrypoint) = report.entrypoints.first() {
        questions.push(format!(
            "What startup flow is reachable from {}?",
            entrypoint.label
        ));
    }
    if let Some(hotspot) = report
        .hotspots
        .architectural_hubs
        .first()
        .or_else(|| report.hotspots.hotspots.first())
    {
        questions.push(format!(
            "Why is {} a central graph hotspot?",
            hotspot.node.label
        ));
    }
    if let Some(community) = report.communities.communities.first() {
        questions.push(format!(
            "What responsibilities and external dependencies does the {} community have?",
            community.label
        ));
    }
    if let Some(edge) = report.architecture.edges.first() {
        questions.push(format!(
            "What evidence explains the architecture link from {} to {}?",
            edge.source, edge.target
        ));
    }
    if let Some(link) = report.surprising_links.links.first() {
        questions.push(format!(
            "Why is the {} edge from {} to {} surprising?",
            link.edge_kind, link.source.label, link.target.label
        ));
    }
    if let Some(risk) = report.risk_summary.top_kinds.first() {
        questions.push(format!(
            "Which code paths are involved in {} findings?",
            risk.kind
        ));
    }
    questions.push(
        "Which low-confidence or heuristic edges should be reviewed before changing shared code?"
            .to_string(),
    );
    questions
}

pub(crate) fn markdown_text(value: &str) -> String {
    value.replace(['\n', '\r'], " ").trim().to_string()
}

pub(crate) fn markdown_table_cell(value: &str) -> String {
    let value = markdown_text(value);
    let value = value.replace('|', "\\|");
    if value.is_empty() {
        "-".to_string()
    } else {
        value
    }
}

pub(crate) fn markdown_code(value: &str) -> String {
    markdown_table_cell(&value.replace('`', "'"))
}

pub(crate) fn project_risk_summary(report: &InsightReport) -> ProjectRiskSummary {
    let errors = severity_count(report, InsightSeverity::Error);
    let warnings = severity_count(report, InsightSeverity::Warning);
    let infos = severity_count(report, InsightSeverity::Info);
    // The score weighs actionable findings only: info counts scale with
    // repository size, and folding them in graded every large healthy
    // repository critical (Phase 9 dogfooding).
    let score = errors * 100 + warnings * 10;
    // One row per kind and severity. Pairing a kind's worst severity with
    // its total said "undeclared external import, warning, 9" where one of
    // the nine was a warning and eight were notes about a test's imports.
    // This summary is built from the untruncated report, so the counts are
    // every finding rather than the sample a limit left.
    let mut counts: BTreeMap<(&str, InsightSeverity), usize> = BTreeMap::new();
    for insight in &report.insights {
        *counts
            .entry((insight.kind.as_str(), insight.severity))
            .or_insert(0) += 1;
    }
    let mut top_kinds: Vec<_> = counts
        .into_iter()
        .map(|((kind, severity), count)| ProjectRiskKindSummary {
            kind: kind.to_string(),
            severity: severity_name(severity).to_string(),
            count,
        })
        .collect();
    top_kinds.sort_by(|left, right| {
        parse_report_severity(&right.severity)
            .cmp(&parse_report_severity(&left.severity))
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    top_kinds.truncate(10);

    ProjectRiskSummary {
        score,
        grade: risk_grade(score).to_string(),
        total: report.total,
        errors,
        warnings,
        infos,
        top_kinds,
    }
}

pub(crate) fn severity_count(report: &InsightReport, severity: InsightSeverity) -> usize {
    report
        .by_severity
        .get(severity_name(severity))
        .copied()
        .unwrap_or(0)
}

pub(crate) fn risk_grade(score: usize) -> &'static str {
    // Thresholds sized for the actionable score (errors x100, warnings x10):
    // critical means errors or a flood of warnings, not repository size
    // (Phase 9 dogfooding).
    match score {
        0 => "clean",
        1..=199 => "low",
        200..=999 => "medium",
        1000..=4999 => "high",
        _ => "critical",
    }
}

pub(crate) fn normalize_project_report_limits(limits: ProjectReportLimits) -> ProjectReportLimits {
    ProjectReportLimits {
        architecture_group_limit: limits
            .architecture_group_limit
            .clamp(1, MAX_REPORT_ARCHITECTURE_GROUP_LIMIT),
        architecture_edge_limit: limits
            .architecture_edge_limit
            .clamp(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT),
        language_link_limit: limits
            .language_link_limit
            .clamp(1, MAX_REPORT_LANGUAGE_LINK_LIMIT),
        hotspot_limit: limits.hotspot_limit.clamp(1, MAX_REPORT_HOTSPOT_LIMIT),
        community_limit: limits.community_limit.clamp(1, MAX_REPORT_COMMUNITY_LIMIT),
        insight_limit: limits.insight_limit.clamp(1, MAX_REPORT_INSIGHT_LIMIT),
        file_summary_limit: limits
            .file_summary_limit
            .clamp(1, MAX_REPORT_FILE_SUMMARY_LIMIT),
        node_summary_limit: limits
            .node_summary_limit
            .clamp(1, MAX_REPORT_NODE_SUMMARY_LIMIT),
        fail_on: limits.fail_on,
    }
}

/// Whether a summarised node or file is the program rather than its tests,
/// examples or vendored code.
///
/// kong's two highest-scoring files were test fixture plugins, ahead of
/// `kong/db/schema/init.lua`, and its highest-scoring symbol was a fixture's
/// `CtxTests:log`. Both lists answer "where is the weight of this project",
/// so the project's own code comes first and the rest keeps its place below.
fn summary_is_the_projects_own(node: &Node) -> bool {
    // A package is not this project's code however many files import it:
    // koel's report opened with `vite-plus` at 417 incoming edges and
    // `@testing-library/vue` at 242, above every function it declares.
    // The label of such a node is a package name, so the path check below
    // reads it as an ordinary file and answered yes.
    if node.kind == NodeKind::ExternalDependency && node.span.is_none() {
        return false;
    }
    let path = node
        .span
        .as_ref()
        .map(|span| span.path.as_str())
        .unwrap_or(node.label.as_str());
    !is_test_like_source_path(path) && !is_vendored_source_path(path)
}

pub(crate) fn compact_file_summaries(
    graph: &CodeGraph,
    insight_report: &InsightReport,
    limit: usize,
) -> ProjectCompactFileSummaryReport {
    let path_index = node_path_index(graph);
    let nodes_by_id = nodes_by_id_index(graph);
    let outgoing_edges = outgoing_edge_index(graph);
    // Which insights touch which path, built in one pass. The per-file filter
    // below used to walk every insight for every file — 5243 files by 32504
    // insights on terraform, which was 63 of the report's 65 seconds.
    let mut insights_by_path: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, insight) in insight_report.insights.iter().enumerate() {
        let mut touched: BTreeSet<String> = BTreeSet::new();
        for node_id in &insight.nodes {
            if let Some(path) = path_index.get(node_id) {
                touched.insert(path.clone());
            }
            if let Some(path) = nodes_by_id
                .get(node_id)
                .and_then(|node| node.span.as_ref())
                .map(|span| normalize_graph_path(&span.path))
            {
                touched.insert(path);
            }
        }
        for path in touched {
            insights_by_path.entry(path).or_default().push(index);
        }
    }
    let mut files: Vec<ProjectCompactFileSummary> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .filter_map(|node| {
            let summary = file_node_summary_indexed(&nodes_by_id, &outgoing_edges, node)?;
            // Precompute the file's normalized path once; the insight loop below
            // otherwise re-normalizes this same label for every insight/node
            // (114 files x 14k insights on this repository).
            let expected = normalize_path_prefix(&node.label);
            let related_insights: Vec<Insight> = if expected.is_empty() {
                // A file with no path at all matches anything the graph knows;
                // rare enough to keep the original scan.
                insight_report
                    .insights
                    .iter()
                    .filter(|insight| {
                        insight.nodes.contains(&node.id)
                            || insight
                                .nodes
                                .iter()
                                .any(|node_id| nodes_by_id.contains_key(node_id))
                    })
                    .cloned()
                    .collect()
            } else {
                insights_by_path
                    .get(&expected)
                    .map(|indexes| {
                        indexes
                            .iter()
                            .filter_map(|index| insight_report.insights.get(*index))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let insight_summary = node_insight_summary(&related_insights);
            let score = compact_file_summary_score(&summary, &insight_summary);
            Some(ProjectCompactFileSummary {
                node: node.clone(),
                summary,
                insight_summary,
                score,
            })
        })
        .collect();

    files.sort_by(|left, right| {
        summary_is_the_projects_own(&right.node)
            .cmp(&summary_is_the_projects_own(&left.node))
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| left.node.label.cmp(&right.node.label))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    let total_files = files.len();
    let truncated = files.len() > limit;
    files.truncate(limit);

    ProjectCompactFileSummaryReport {
        files,
        total_files,
        truncated,
    }
}

pub(crate) fn compact_file_summary_score(
    summary: &FileNodeSummary,
    insight_summary: &NodeInsightSummary,
) -> usize {
    let risk_score: usize = insight_summary
        .by_severity
        .iter()
        .map(|(severity, count)| match severity.as_str() {
            "error" => *count * 100,
            "warning" => *count * 40,
            "info" => *count * 10,
            _ => *count,
        })
        .sum();

    risk_score
        + summary.trace_edges * 10
        + summary.unresolved_calls * 20
        + summary.error_facts * 20
        + summary.config_reads * 8
        + summary.environment_reads * 8
        + summary.code_symbols * 5
        + summary.direct_dependencies * 3
        + summary.imports
}

/// Invert the insight list into a `NodeId -> referencing insights` map so
/// per-node compact summaries look up only their own findings. Without this the
/// summary loop rescans every insight for every candidate node — O(nodes ×
/// insights), which stalls for tens of seconds on repositories with thousands
/// of insights. Insight order is preserved and each insight appears once per
/// node even if it lists that node id more than once.
pub(crate) fn insights_by_referenced_node(
    insight_report: &InsightReport,
) -> BTreeMap<NodeId, Vec<Insight>> {
    let mut index: BTreeMap<NodeId, Vec<Insight>> = BTreeMap::new();
    for insight in &insight_report.insights {
        let mut seen: BTreeSet<NodeId> = BTreeSet::new();
        for node_id in &insight.nodes {
            if seen.insert(*node_id) {
                index.entry(*node_id).or_default().push(insight.clone());
            }
        }
    }
    index
}

pub(crate) fn compact_node_summaries(
    graph: &CodeGraph,
    insight_report: &InsightReport,
    limit: usize,
) -> ProjectCompactNodeSummaryReport {
    let nodes_by_id = nodes_by_id_index(graph);
    let incident_edges = incident_edge_index(graph);
    let insights_by_node = insights_by_referenced_node(insight_report);
    let no_edges: Vec<&Edge> = Vec::new();
    let mut nodes: Vec<ProjectCompactNodeSummary> = graph
        .nodes
        .iter()
        .filter(|node| compact_node_summary_candidate(&node.kind))
        // A placeholder stands in for a name nothing resolved -- `to_string`,
        // `map`, `Some` -- and its span is whichever call site came first.
        // Ranked by incoming edges they took fourteen of the report's top
        // twenty rows, none of which a reader can open.
        .filter(|node| !is_call_placeholder(node))
        .filter_map(|node| {
            let dependency_summary = node_dependency_summary_indexed(
                &nodes_by_id,
                incident_edges.get(&node.id).unwrap_or(&no_edges),
                node.id,
            );
            let related_insights: Vec<Insight> =
                insights_by_node.get(&node.id).cloned().unwrap_or_default();
            let insight_summary = node_insight_summary(&related_insights);
            let roles = compact_node_summary_roles(node, &dependency_summary, &insight_summary);
            if roles.is_empty()
                && dependency_summary.incoming == 0
                && dependency_summary.outgoing == 0
                && insight_summary.by_severity.is_empty()
            {
                return None;
            }
            let score = compact_node_summary_score(node, &dependency_summary, &insight_summary);
            Some(ProjectCompactNodeSummary {
                node: node.clone(),
                dependency_summary,
                insight_summary,
                roles,
                score,
            })
        })
        .collect();

    nodes.sort_by(|left, right| {
        summary_is_the_projects_own(&right.node)
            .cmp(&summary_is_the_projects_own(&left.node))
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| node_rank(&left.node.kind).cmp(&node_rank(&right.node.kind)))
            .then_with(|| left.node.label.cmp(&right.node.label))
            .then_with(|| left.node.id.cmp(&right.node.id))
    });

    let total_nodes = nodes.len();
    let truncated = nodes.len() > limit;
    nodes.truncate(limit);

    ProjectCompactNodeSummaryReport {
        nodes,
        total_nodes,
        truncated,
    }
}

pub(crate) fn compact_node_summary_candidate(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Module
            | NodeKind::Function
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Config
            | NodeKind::Environment
            | NodeKind::ExternalDependency
    )
}

pub(crate) fn compact_node_summary_roles(
    node: &Node,
    dependency_summary: &NodeDependencySummary,
    insight_summary: &NodeInsightSummary,
) -> Vec<String> {
    let mut roles = Vec::new();
    if node.kind == NodeKind::Entrypoint
        || dependency_summary
            .incoming_edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Entrypoint))
        || dependency_summary
            .outgoing_edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Entrypoint))
    {
        roles.push("entrypoint".to_string());
    }
    if !insight_summary.by_severity.is_empty() {
        roles.push("risk".to_string());
    }
    if node.kind == NodeKind::Config
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::ReadsConfig))
    {
        roles.push("config".to_string());
    }
    if node.kind == NodeKind::Environment
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::ReadsEnvironment))
    {
        roles.push("environment".to_string());
    }
    if node.kind == NodeKind::ExternalDependency
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::DependsOn))
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Imports))
    {
        roles.push("external_boundary".to_string());
    }
    if dependency_summary
        .edge_kinds
        .contains_key(&edge_kind_name(&EdgeKind::MayError))
    {
        roles.push("error_flow".to_string());
    }
    if dependency_summary.incoming + dependency_summary.outgoing >= 3 {
        roles.push("hub".to_string());
    }
    if roles.is_empty() && is_code_symbol(&node.kind) {
        roles.push("code_symbol".to_string());
    }
    roles
}

pub(crate) fn compact_node_summary_score(
    node: &Node,
    dependency_summary: &NodeDependencySummary,
    insight_summary: &NodeInsightSummary,
) -> usize {
    let risk_score: usize = insight_summary
        .by_severity
        .iter()
        .map(|(severity, count)| match severity.as_str() {
            "error" => *count * 100,
            "warning" => *count * 40,
            "info" => *count * 10,
            _ => *count,
        })
        .sum();
    let entrypoint_score = if node.kind == NodeKind::Entrypoint
        || dependency_summary
            .edge_kinds
            .contains_key(&edge_kind_name(&EdgeKind::Entrypoint))
    {
        50
    } else {
        0
    };
    let boundary_score = if matches!(
        node.kind,
        NodeKind::Config | NodeKind::Environment | NodeKind::ExternalDependency
    ) {
        15
    } else {
        0
    };

    risk_score
        + entrypoint_score
        + boundary_score
        + dependency_summary.incoming * 4
        + dependency_summary.outgoing * 6
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::Calls))
            .copied()
            .unwrap_or(0)
            * 5
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::MayError))
            .copied()
            .unwrap_or(0)
            * 10
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::ReadsConfig))
            .copied()
            .unwrap_or(0)
            * 8
        + dependency_summary
            .edge_kinds
            .get(&edge_kind_name(&EdgeKind::ReadsEnvironment))
            .copied()
            .unwrap_or(0)
            * 8
}

pub(crate) fn failing_insight_count(report: &InsightReport, fail_on: InsightSeverity) -> usize {
    report
        .by_severity
        .iter()
        .filter_map(|(severity, count)| {
            parse_report_severity(severity)
                .filter(|severity| *severity >= fail_on)
                .map(|_| *count)
        })
        .sum()
}

pub(crate) fn parse_report_severity(value: &str) -> Option<InsightSeverity> {
    match value {
        "info" => Some(InsightSeverity::Info),
        "warning" => Some(InsightSeverity::Warning),
        "error" => Some(InsightSeverity::Error),
        _ => None,
    }
}

pub(crate) fn insight_breakdowns(
    insights: &[Insight],
) -> (BTreeMap<String, usize>, BTreeMap<String, usize>) {
    let mut by_severity = BTreeMap::new();
    let mut by_kind = BTreeMap::new();
    for insight in insights {
        *by_severity
            .entry(severity_name(insight.severity).to_string())
            .or_insert(0) += 1;
        *by_kind.entry(insight.kind.clone()).or_insert(0) += 1;
    }
    (by_severity, by_kind)
}

pub(crate) fn format_backtick_list<'a>(
    values: impl Iterator<Item = &'a str>,
    limit: usize,
) -> String {
    let values = values.collect::<Vec<_>>();
    let rendered = values
        .iter()
        .take(limit)
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let remaining = values.len().saturating_sub(limit);
    if remaining == 0 {
        rendered
    } else if rendered.is_empty() {
        format!("{remaining} more")
    } else {
        format!("{rendered}, and {remaining} more")
    }
}
