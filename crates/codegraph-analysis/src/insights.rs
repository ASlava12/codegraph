//! Investigation insights: the public insight API, quality gate, and
//! every insight generator with severity calibration.

use codegraph_core::{
    COMPUTED_ENVIRONMENT_KEY, CodeGraph, EdgeKind, Node, NodeId, NodeKind,
    is_python_stdlib_package, is_vendored_source_path,
};
use std::collections::{BTreeMap, BTreeSet};

#[allow(unused_imports)]
use crate::*;

pub fn insights(graph: &CodeGraph) -> InsightReport {
    let mut insights = Vec::new();
    add_parse_error_insights(graph, &mut insights);
    add_semantic_diagnostic_insights(graph, &mut insights);
    add_unresolved_call_insights(graph, &mut insights);
    add_ambiguous_call_resolution_insights(graph, &mut insights);
    add_unresolved_local_import_insights(graph, &mut insights);
    add_unresolved_sql_table_reference_insights(graph, &mut insights);
    add_sql_schema_consistency_insights(graph, &mut insights);
    add_cross_language_heuristic_edge_insights(graph, &mut insights);
    add_duplicate_function_insights(graph, &mut insights);
    add_duplicate_compose_published_port_insights(graph, &mut insights);
    add_duplicate_entrypoint_insights(graph, &mut insights);
    add_ambiguous_entrypoint_target_insights(graph, &mut insights);
    add_orphan_function_insights(graph, &mut insights);
    add_error_flow_insights(graph, &mut insights);
    add_unresolved_entrypoint_insights(graph, &mut insights);
    add_unresolved_compose_command_path_insights(graph, &mut insights);
    add_unresolved_compose_env_file_path_insights(graph, &mut insights);
    add_unresolved_compose_volume_source_path_insights(graph, &mut insights);
    add_unresolved_github_actions_job_need_insights(graph, &mut insights);
    add_unresolved_github_actions_local_action_insights(graph, &mut insights);
    add_unresolved_github_actions_run_path_insights(graph, &mut insights);
    add_unresolved_gitlab_ci_job_dependency_insights(graph, &mut insights);
    add_unresolved_gitlab_ci_script_path_insights(graph, &mut insights);
    add_unresolved_kubernetes_config_ref_insights(graph, &mut insights);
    add_unresolved_kubernetes_ingress_backend_insights(graph, &mut insights);
    add_unresolved_kubernetes_service_selector_insights(graph, &mut insights);
    add_unresolved_dockerfile_command_path_insights(graph, &mut insights);
    add_unresolved_makefile_command_path_insights(graph, &mut insights);
    add_entrypoint_dead_end_insights(graph, &mut insights);
    // One walk feeds every reachability finding: recomputing it per insight
    // meant four BFS passes over the whole graph.
    let reachable = entrypoint_reachable_nodes(graph);
    add_entrypoint_coverage_insights(graph, &reachable, &mut insights);
    add_unreachable_config_read_insights(graph, &reachable, &mut insights);
    add_unreachable_error_flow_insights(graph, &reachable, &mut insights);
    add_unreachable_source_file_insights(graph, &reachable, &mut insights);
    add_conflicting_config_default_insights(graph, &mut insights);
    add_mixed_config_requirement_insights(graph, &mut insights);
    add_undeclared_flutter_asset_insights(graph, &mut insights);
    add_unmatched_platform_channel_insights(graph, &mut insights);
    add_rationale_risk_comment_insights(graph, &mut insights);
    add_sensitive_ci_environment_literal_insights(graph, &mut insights);
    add_sensitive_config_default_insights(graph, &mut insights);
    add_undeclared_import_insights(graph, &mut insights);
    add_unused_dependency_insights(graph, &mut insights);
    add_conflicting_dependency_insights(graph, &mut insights);
    add_mixed_dependency_scope_insights(graph, &mut insights);
    add_non_runtime_dependency_import_insights(graph, &mut insights);
    add_test_only_runtime_dependency_insights(graph, &mut insights);
    add_unresolved_framework_route_handler_insights(graph, &mut insights);
    add_duplicate_framework_route_insights(graph, &mut insights);
    add_custom_rule_violation_insights(graph, &mut insights);
    add_dependency_cycle_insights(graph, &mut insights);
    insights.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.message.cmp(&right.message))
    });

    let mut by_severity = BTreeMap::new();
    let mut by_kind = BTreeMap::new();
    for insight in &insights {
        *by_severity
            .entry(severity_name(insight.severity).to_string())
            .or_insert(0) += 1;
        *by_kind.entry(insight.kind.clone()).or_insert(0) += 1;
    }

    InsightReport {
        total: insights.len(),
        by_severity,
        by_kind,
        insights,
    }
}

pub fn filter_insight_report(report: InsightReport, filter: &InsightFilter) -> InsightReport {
    let kind = filter.kind.as_ref().map(|value| value.to_ascii_lowercase());
    let search = filter
        .search
        .as_ref()
        .map(|value| value.to_ascii_lowercase());
    let mut insights: Vec<_> = report
        .insights
        .into_iter()
        .filter(|insight| {
            filter
                .severity
                .is_none_or(|expected| insight.severity == expected)
                && kind
                    .as_deref()
                    .is_none_or(|expected| insight.kind.to_ascii_lowercase().contains(expected))
                && search
                    .as_deref()
                    .is_none_or(|expected| insight_search_matches(insight, expected))
        })
        .collect();
    let total = insights.len();
    let (by_severity, by_kind) = insight_breakdowns(&insights);
    insights.truncate(filter.limit.clamp(1, 500));

    InsightReport {
        total,
        by_severity,
        by_kind,
        insights,
    }
}

pub fn check_insights(report: InsightReport, fail_on: InsightSeverity) -> CheckReport {
    let failing_insights = failing_insight_count(&report, fail_on);
    CheckReport {
        passed: failing_insights == 0,
        fail_on: severity_name(fail_on).to_string(),
        failing_insights,
        report,
    }
}

/// Cap the insight list embedded in a gate report to a failing-first sample.
/// Totals, breakdowns, and the pass/fail verdict stay limit-independent.
pub(crate) fn bounded_quality_gate(mut check: CheckReport, sample_limit: usize) -> CheckReport {
    check
        .report
        .insights
        .sort_by_key(|insight| std::cmp::Reverse(insight.severity));
    check.report.insights.truncate(sample_limit.max(1));
    check
}

/// Where one unresolved import points, as far as anything can tell. A
/// relative target means a different file from each directory it is
/// written in, so it is resolved against the file that wrote it; anything
/// else is looked up on a search path and names the same missing file
/// wherever it appears.
fn missing_import_key(source: &str, target: &str) -> String {
    if !target.starts_with('.') {
        return target.to_string();
    }
    let mut parts: Vec<&str> = Vec::new();
    let directory = source.rsplit_once('/').map(|(head, _)| head).unwrap_or("");
    for part in directory.split('/').chain(target.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}

struct MissingImport {
    target: String,
    sources: BTreeSet<String>,
    production_source: bool,
    /// Whether every file that names it wrote a C-family include whose
    /// first directory this repository does not hold — a library the
    /// machine installs rather than a file of this project.
    from_an_include_path: bool,
    nodes: BTreeSet<NodeId>,
    edges: Vec<usize>,
}

pub(crate) fn add_unresolved_local_import_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let directories = scanned_directory_labels(graph);
    let published = published_paths(graph);
    let mut missing: BTreeMap<String, MissingImport> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::ExternalDependency
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "import")
            || node
                .metadata
                .get("import_scope")
                .is_none_or(|scope| scope != "local")
            || node
                .metadata
                .get("resolution")
                .is_none_or(|resolution| resolution != "unresolved")
        {
            continue;
        }
        let target = node
            .metadata
            .get("import_target")
            .map(String::as_str)
            .unwrap_or(node.label.as_str());
        // `. ./$cache_file` names whatever the script put in that variable,
        // and `@HOME_MANAGER_LIB@` is filled in when the package is built,
        // so there is no file to go looking for. `./dist/vue.cjs.js` is
        // written by a build into a directory no scan walks.
        if target.contains('$')
            || (target.starts_with('@') && target.ends_with('@'))
            || command_path_is_installed_or_unscanned(target)
        {
            continue;
        }
        // `#include "openssl/ssl.h"` is written with quotes and comes from
        // the include path all the same: redis holds no `openssl/`
        // directory and spdlog no `benchmark/` one, so what is missing is a
        // library the machine installs rather than a file of this project.
        let from_an_include_path = matches!(
            node.metadata.get("language").map(String::as_str),
            Some("c") | Some("cpp") | Some("objc")
        ) && !target.starts_with("./")
            && !target.starts_with("../")
            && target.split_once('/').is_some_and(|(head, _)| {
                !directories
                    .iter()
                    .any(|directory| *directory == head || directory.ends_with(&format!("/{head}")))
            });
        let edges = incoming_edge_indexes(graph, node.id, EdgeKind::Imports);
        let source = edges
            .first()
            .and_then(|index| graph.edges.get(*index))
            .and_then(|edge| node_label(graph, edge.source))
            .unwrap_or("unknown");
        // Imports inside inline test modules or test-convention files are
        // fixture wiring, not production dead links, mirroring the
        // benchmark-oracle test exclusions (Phase 9 dogfooding).
        // And openzeppelin's formal-verification harnesses import
        // `../patched/...`, a copy of the contracts that `make` writes
        // beside them. The package publishes the contracts and not the
        // harnesses, so this is the verification setup rather than the
        // program.
        let production = !node
            .metadata
            .get("test_context")
            .is_some_and(|value| value == "true")
            && !is_test_like_source_path(source)
            && !is_vendored_source_path(source)
            && !published.excludes(source);

        let entry = missing
            .entry(missing_import_key(source, target))
            .or_insert_with(|| MissingImport {
                target: target.to_string(),
                sources: BTreeSet::new(),
                production_source: false,
                from_an_include_path: true,
                nodes: BTreeSet::new(),
                edges: Vec::new(),
            });
        entry.sources.insert(source.to_string());
        entry.production_source |= production;
        entry.from_an_include_path &= from_an_include_path;
        entry.nodes.insert(node.id);
        entry.nodes.extend(
            edges
                .iter()
                .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
        );
        entry.edges.extend(edges);
    }

    // 63 of redis's files include the same generated jemalloc header, and
    // saying so 63 times says nothing the first one did not.
    for entry in missing.into_values() {
        let sources = format_backtick_list(entry.sources.iter().map(String::as_str), 3);
        let verb = if entry.sources.len() == 1 {
            "imports"
        } else {
            "import"
        };
        let target = entry.target;
        insights.push(Insight {
            kind: "unresolved_local_import".to_string(),
            severity: if entry.production_source && !entry.from_an_include_path {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "{sources} {verb} local target `{target}` but no matching file was found"
            ),
            nodes: entry.nodes.into_iter().collect(),
            edges: entry.edges,
        });
    }
}

/// Broader schema-consistency findings from indexer-recorded metadata:
/// ALTER/DROP statements whose table was never defined in the indexed
/// schema, and migration files sharing one sequence number.
pub(crate) fn add_sql_schema_consistency_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if let Some(tables) = node.metadata.get("unresolved_sql_alter_tables") {
            for entry in tables.split(',').filter(|entry| !entry.is_empty()) {
                let (operation, table) = entry.split_once(':').unwrap_or(("alter", entry));
                insights.push(Insight {
                    kind: "unresolved_sql_alter_target".to_string(),
                    severity: InsightSeverity::Warning,
                    message: format!(
                        "`{}` runs {} TABLE on `{table}`, which is not defined in the indexed schema",
                        node.label,
                        operation.to_uppercase()
                    ),
                    nodes: vec![node.id],
                    edges: Vec::new(),
                });
            }
        }
        if let Some(other) = node.metadata.get("duplicate_migration_sequence") {
            insights.push(Insight {
                kind: "duplicate_migration_sequence".to_string(),
                severity: InsightSeverity::Warning,
                message: format!(
                    "Migration `{}` shares its sequence number with `{other}`; apply order is ambiguous",
                    node.label
                ),
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        }
    }
}

pub(crate) fn add_unresolved_sql_table_reference_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "app_sql_query")
        {
            continue;
        }
        let Some(tables) = node
            .metadata
            .get("unresolved_tables")
            .map(|tables| tables.trim())
            .filter(|tables| !tables.is_empty())
        else {
            continue;
        };
        let named: Vec<&str> = tables
            .split(',')
            .map(str::trim)
            .filter(|table| !table.is_empty() && names_a_project_table(table))
            .collect();
        if named.is_empty() {
            continue;
        }
        let tables = named.join(", ");

        let incoming = incoming_edge_indexes(graph, node.id, EdgeKind::References);
        let outgoing = outgoing_edge_indexes(graph, node.id, EdgeKind::References);
        let mut edges = incoming
            .iter()
            .chain(outgoing.iter())
            .copied()
            .collect::<Vec<_>>();
        edges.sort_unstable();
        edges.dedup();

        let mut nodes = std::iter::once(node.id)
            .chain(
                edges
                    .iter()
                    .filter_map(|index| graph.edges.get(*index))
                    .flat_map(|edge| [edge.source, edge.target]),
            )
            .collect::<Vec<_>>();
        nodes.sort_unstable();
        nodes.dedup();

        let operation = node
            .metadata
            .get("operation")
            .map(String::as_str)
            .unwrap_or("sql");
        let source = incoming
            .first()
            .and_then(|index| graph.edges.get(*index))
            .and_then(|edge| node_label(graph, edge.source))
            .unwrap_or("unknown source");
        // SQL strings in inline test modules or test-convention files are
        // fixtures, not production queries against the indexed schema,
        // mirroring the benchmark-oracle test exclusions (Phase 9 dogfooding).
        let severity = if node
            .metadata
            .get("test_context")
            .is_some_and(|value| value == "true")
            || node
                .span
                .as_ref()
                .is_some_and(|span| is_test_like_source_path(&span.path))
        {
            InsightSeverity::Info
        } else {
            InsightSeverity::Warning
        };

        insights.push(Insight {
            kind: "unresolved_sql_table_reference".to_string(),
            severity,
            message: format!(
                "`{source}` has {operation} SQL query `{}` referencing table(s) `{tables}` without a matching indexed schema table",
                node.label
            ),
            nodes,
            edges,
        });
    }
}

/// Whether a name a query reads from could be a table this project
/// defines. `fmt.Sprintf("... FROM %s.%s", schema, table)` fills the name
/// in when the query runs, and `information_schema` and `pg_catalog`
/// belong to the database rather than to anybody's migrations.
fn names_a_project_table(table: &str) -> bool {
    let lowered = table.to_ascii_lowercase();
    !table.contains('%')
        && !table.contains('$')
        && !lowered.starts_with("information_schema.")
        && !lowered.starts_with("pg_catalog.")
        && !lowered.starts_with("sys.")
        && !matches!(
            lowered.as_str(),
            "sqlite_master" | "sqlite_sequence" | "current_timestamp" | "dual"
        )
}

pub(crate) fn add_cross_language_heuristic_edge_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind)
            || !matches!(
                edge.confidence,
                codegraph_core::Confidence::Heuristic | codegraph_core::Confidence::Unknown
            )
        {
            continue;
        }
        let source_language = node_language(&nodes_by_id, edge.source);
        let target_language = node_language(&nodes_by_id, edge.target);
        if source_language == "unknown"
            || target_language == "unknown"
            || source_language == target_language
        {
            continue;
        }
        let source = nodes_by_id
            .get(&edge.source)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        let target = nodes_by_id
            .get(&edge.target)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        insights.push(Insight {
            kind: "cross_language_heuristic_edge".to_string(),
            severity: heuristic_scan_severity(graph),
            message: format!(
                "`{source}` ({source_language}) {} `{target}` ({target_language}) with {} confidence",
                edge_kind_name(&edge.kind),
                confidence_name(edge.confidence)
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![edge_index],
        });
    }
}

pub(crate) fn add_duplicate_function_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<&str, Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::Function {
            groups.entry(&node.label).or_default().push(node.id);
        }
    }

    for (label, nodes) in groups {
        if nodes.len() > 1 {
            insights.push(Insight {
                kind: "duplicate_function_label".to_string(),
                severity: InsightSeverity::Info,
                message: format!("Function label `{label}` appears {} times", nodes.len()),
                nodes,
                edges: Vec::new(),
            });
        }
    }
}

pub(crate) fn add_duplicate_compose_published_port_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let mut groups: BTreeMap<(String, String), Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "compose_port")
        {
            continue;
        }
        let Some(published) = node
            .metadata
            .get("published_port")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let protocol = node
            .metadata
            .get("protocol")
            .map(String::as_str)
            .unwrap_or("tcp")
            .to_ascii_lowercase();
        groups
            .entry((published.to_string(), protocol))
            .or_default()
            .push(node.id);
    }

    for ((published, protocol), nodes) in groups {
        if nodes.len() <= 1 {
            continue;
        }
        let services = nodes
            .iter()
            .filter_map(|node_id| graph.nodes.iter().find(|node| node.id == *node_id))
            .filter_map(|node| node.metadata.get("service").cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        let mut edges = Vec::new();
        for node_id in &nodes {
            edges.extend(incoming_edge_indexes(graph, *node_id, EdgeKind::References));
        }
        insights.push(Insight {
            kind: "duplicate_compose_published_port".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Docker Compose published port `{published}/{protocol}` is declared by multiple services: {services}"
            ),
            nodes,
            edges,
        });
    }
}

pub(crate) fn add_duplicate_entrypoint_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<&str, Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::Entrypoint {
            groups.entry(&node.label).or_default().push(node.id);
        }
    }

    for (label, nodes) in groups {
        if nodes.len() <= 1 {
            continue;
        }

        let edges = nodes
            .iter()
            .flat_map(|node_id| incoming_edge_indexes(graph, *node_id, EdgeKind::Entrypoint))
            .collect();

        insights.push(Insight {
            // A C project gives nine of its Makefiles an `all` target and
            // eleven test files a `/` route, which is how those projects
            // are built rather than something wrong with them. A query for
            // an ambiguous label already comes back saying which nodes
            // matched, so this is a note about the labels, not a defect.
            kind: "duplicate_entrypoint_label".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "Entrypoint label `{label}` appears {} times, so a trace by that label has to say which one",
                nodes.len()
            ),
            nodes,
            edges,
        });
    }
}

pub(crate) fn add_ambiguous_entrypoint_target_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "manifest_entrypoint")
        {
            continue;
        }

        for relation in ["entrypoint_file", "entrypoint_function"] {
            let matches = graph
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| {
                    edge.source == node.id
                        && edge.kind == EdgeKind::References
                        && edge
                            .metadata
                            .get("relation")
                            .is_some_and(|value| value == relation)
                })
                .collect::<Vec<_>>();
            let targets = matches
                .iter()
                .map(|(_, edge)| edge.target)
                .collect::<BTreeSet<_>>();
            if targets.len() < 2 {
                continue;
            }

            let relation_label = if relation == "entrypoint_file" {
                "files"
            } else {
                "functions"
            };
            let target_labels = targets
                .iter()
                .filter_map(|target| node_label(graph, *target))
                .take(5)
                .map(|label| format!("`{label}`"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut nodes = Vec::with_capacity(targets.len() + 1);
            nodes.push(node.id);
            nodes.extend(targets.iter().copied());
            let edges = matches.iter().map(|(index, _)| *index).collect();

            insights.push(Insight {
                kind: "ambiguous_entrypoint_target".to_string(),
                severity: InsightSeverity::Warning,
                message: format!(
                    "Entrypoint `{}` resolves to multiple {relation_label}: {target_labels}",
                    node.label
                ),
                nodes,
                edges,
            });
        }
    }
}

pub(crate) fn add_orphan_function_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let entrypoints: BTreeSet<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Entrypoint)
        .map(|edge| edge.target)
        .collect();
    let called: BTreeSet<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| {
            edge.kind == EdgeKind::Calls
                || (edge.kind == EdgeKind::References
                    && edge
                        .metadata
                        .get("relation")
                        .is_some_and(|relation| relation == "entrypoint_function"))
        })
        .map(|edge| edge.target)
        .collect();

    for node in &graph.nodes {
        // A definition written inside another is reached through the one
        // that holds it: shellcheck names 167 `where` bindings `f`, and
        // "nothing calls f" is not news about a local helper.
        if node.metadata.contains_key("enclosing_function") {
            continue;
        }
        if node.kind == NodeKind::Function
            && !entrypoints.contains(&node.id)
            && !called.contains(&node.id)
        {
            // A function nobody in the repository calls is either dead or
            // the API: terraform has 11406 exported functions with no
            // in-repo caller against 592 unexported ones, and one number
            // for both says nothing a reader can act on.
            let exported = node
                .metadata
                .get("visibility")
                .is_some_and(|visibility| visibility == "public");
            let message = if exported {
                format!(
                    "Function `{}` has no incoming call edge; it is exported, so its callers may be outside this repository",
                    node.label
                )
            } else {
                format!("Function `{}` has no incoming call edge", node.label)
            };
            insights.push(Insight {
                kind: "orphan_function".to_string(),
                severity: InsightSeverity::Info,
                message,
                nodes: vec![node.id],
                edges: Vec::new(),
            });
        }
    }
}

pub(crate) fn add_error_flow_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let labels: BTreeMap<NodeId, &str> = graph
        .nodes
        .iter()
        .map(|node| (node.id, node.label.as_str()))
        .collect();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::MayError {
            continue;
        }
        let source = labels.get(&edge.source).copied().unwrap_or("unknown");
        let target = labels.get(&edge.target).copied().unwrap_or("unknown");
        insights.push(Insight {
            // Error constructs are normal control flow in Result-idiomatic
            // code; the fact is informational (Phase 9 dogfooding).
            kind: "potential_error_flow".to_string(),
            severity: InsightSeverity::Info,
            message: format!("`{source}` may error via `{target}`"),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

/// Whether a manifest entrypoint's target mentions a file at all. A
/// target is often a whole shell command, and only a token carrying a
/// directory separator or a script extension can name something in the
/// repository — a flag or a program name cannot.
/// The value of a `NAME=value` word, when the word is one: a shell sets
/// variables that way before the program it runs.
fn shell_assignment_value(token: &str) -> Option<&str> {
    let (name, value) = token.split_once('=')?;
    let names_a_variable = !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    (names_a_variable && !value.is_empty()).then_some(value)
}

pub(crate) fn names_a_path(target: &str) -> bool {
    path_like_tokens(target).next().is_some()
}

/// The words of a command that could name something the repository holds.
fn path_like_tokens(target: &str) -> impl Iterator<Item = &str> {
    target.split_whitespace().filter_map(|token| {
        let token = token.trim_matches(['"', '\'']);
        // A flag names nothing, `lib/**/*.js` is a pattern rather than a
        // file the repository has to contain, and
        // `http://localhost:3000/x.html` is somewhere else entirely.
        if token.starts_with('-')
            || token.contains('*')
            || token.contains('?')
            || token.contains("://")
        {
            return None;
        }
        // `@zod/source` is a package specifier, not a directory: zod runs
        // `tsx --conditions @zod/source` and the graph went looking for a
        // file.
        if token.starts_with('@') && token.matches('/').count() == 1 {
            return None;
        }
        // `env SRC=./fv/harnesses hardhat build` sets a variable before it
        // names the program: the path is the value, and openzeppelin's
        // script was reported unresolved because the whole assignment was
        // read as one.
        let token = shell_assignment_value(token).unwrap_or(token);
        if token.contains('/') {
            return Some(token);
        }
        // `app.main` after `python -m` names a module the repository can
        // hold, and so does anything with a source extension.
        let segments: Vec<&str> = token.split('.').collect();
        (segments.len() >= 2
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && segment
                        .chars()
                        .all(|character| character.is_alphanumeric() || character == '_')
            }))
        .then_some(token)
    })
}

/// Whether a path sits in a hidden directory the scan never opened. A scan
/// skips hidden directories unless asked for them, apart from `.github`,
/// so `node --test .vitepress/search.test.js` names a file nobody looked
/// for. If the scan did hold something under that directory -- because
/// hidden files were included, or because it is `.github` -- then it did
/// look, and a missing file there is a real one.
fn is_unscanned_hidden_path(scanned_paths: &BTreeSet<&str>, token: &str) -> bool {
    let Some(directory) = token
        .split('/')
        .next()
        .filter(|segment| segment.starts_with('.') && *segment != "." && *segment != "..")
    else {
        return false;
    };
    let prefix = format!("{directory}/");
    !scanned_paths
        .iter()
        .any(|path| path.starts_with(&prefix) || *path == directory)
}

pub(crate) fn add_unresolved_entrypoint_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let scanned_paths: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::File | NodeKind::Directory))
        .map(|node| node.label.as_str())
        .collect();
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "manifest_entrypoint")
        {
            continue;
        }
        let Some(target) = node
            .metadata
            .get("target")
            .map(|target| target.trim())
            .filter(|target| !target.is_empty())
        else {
            continue;
        };
        // `vitepress dev`, `patch-package --exclude nothing`, `npm run a &&
        // npm run b`: a script that runs a program names no file in the
        // repository, so there is nothing here that failed to resolve. 44
        // of the 59 unresolved targets across the corpora are of this kind.
        if !names_a_path(target) {
            continue;
        }
        // `vite packages-private/sfc-playground --host` and `npm --prefix
        // tests/module/cjs run test` name directories that are right
        // there, and `conventional-changelog -i CHANGELOG.md` a file the
        // scan holds. None of them failed to resolve; the entrypoint
        // simply points at something other than a function.
        // An npm script runs where its manifest is: zod declares
        // `tsc -p tsconfig.bench.json` in `packages/tsc/package.json`, and
        // that file sits beside it rather than at the repository root.
        let declared_in = node
            .span
            .as_ref()
            .map(|span| span.path.clone())
            .or_else(|| declaring_file_label(graph, node.id));
        let base_dir = declared_in
            .as_deref()
            .and_then(|path| path.rsplit_once('/'))
            .map(|(directory, _)| directory.to_string());
        if path_like_tokens(target).any(|token| {
            let token = token
                .trim_start_matches("./")
                .trim_end_matches('/')
                .trim_end_matches('\\');
            let beside_the_manifest = base_dir
                .as_deref()
                .map(|directory| format!("{directory}/{token}"));
            scanned_paths.contains(token)
                || beside_the_manifest
                    .as_deref()
                    .is_some_and(|path| scanned_paths.contains(path))
                || is_unscanned_hidden_path(&scanned_paths, token)
                // `@php vendor/bin/phpunit` runs what composer installs, as
                // monolog's `composer script:test` does.
                || command_path_is_installed_or_unscanned(token)
        }) {
            continue;
        }
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge.metadata.get("relation").is_some_and(|relation| {
                    matches!(relation.as_str(), "entrypoint_file" | "entrypoint_function")
                })
        });
        if resolved {
            continue;
        }

        insights.push(Insight {
            kind: "unresolved_entrypoint_target".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Entrypoint `{}` declares target `{target}` but no matching file or function was found",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
}

pub(crate) fn add_unresolved_dockerfile_command_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    add_unresolved_workflow_command_path_insights(
        graph,
        insights,
        "dockerfile_entrypoint",
        "docker_command_path",
        "unresolved_dockerfile_command_path",
        "Dockerfile instruction",
    );
}

pub(crate) fn add_unresolved_compose_command_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    add_unresolved_workflow_command_path_insights(
        graph,
        insights,
        "compose_service",
        "compose_command_path",
        "unresolved_compose_command_path",
        "Compose service",
    );
}

pub(crate) fn add_unresolved_compose_env_file_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    // One missing file is one fact, however many services name it:
    // gqlgen's compose file gives the same `.env` to five services and
    // said so five times.
    let mut missing: BTreeMap<String, MissingEnvFile> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "compose_env_file")
        {
            continue;
        }
        let Some(env_file_path) = node
            .metadata
            .get("env_file_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "compose_env_file_path")
        });
        if resolved {
            continue;
        }

        let service = node
            .metadata
            .get("service")
            .map(String::as_str)
            .unwrap_or("unknown");
        let entry = missing
            .entry(env_file_path.to_string())
            .or_insert_with(|| MissingEnvFile {
                services: BTreeSet::new(),
                nodes: BTreeSet::new(),
                edges: Vec::new(),
                // A compose file among the examples describes how to run an
                // example, not how to run the project.
                production: true,
            });
        entry.services.insert(service.to_string());
        entry.nodes.insert(node.id);
        entry
            .nodes
            .extend(compose_env_file_reader_ids(graph, node.id));
        entry
            .edges
            .extend(incoming_edge_indexes(graph, node.id, EdgeKind::ReadsConfig));
        entry.production &= !node
            .span
            .as_ref()
            .is_some_and(|span| is_test_like_source_path(&span.path))
            && !is_test_like_source_path(env_file_path);
    }

    for (env_file_path, entry) in missing {
        let services = format_backtick_list(entry.services.iter().map(String::as_str), 3);
        let verb = if entry.services.len() == 1 {
            "references"
        } else {
            "reference"
        };
        let mut edges = entry.edges;
        edges.sort_unstable();
        edges.dedup();
        insights.push(Insight {
            kind: "unresolved_compose_env_file_path".to_string(),
            severity: if entry.production {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "Compose service {services} {verb} env_file `{env_file_path}` but the file was not found"
            ),
            nodes: entry.nodes.into_iter().collect(),
            edges,
        });
    }
}

struct MissingEnvFile {
    services: BTreeSet<String>,
    nodes: BTreeSet<NodeId>,
    edges: Vec<usize>,
    production: bool,
}

pub(crate) fn compose_env_file_reader_ids(graph: &CodeGraph, config: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == config
                && edge.kind == EdgeKind::ReadsConfig
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "compose_env_file")
        })
        .map(|edge| edge.source)
        .collect()
}

pub(crate) fn add_unresolved_compose_volume_source_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "compose_volume")
        {
            continue;
        }
        let Some(source_path) = node
            .metadata
            .get("local_source_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "compose_volume_source_path")
        });
        if resolved || path_holds_an_unexpanded_variable(source_path) {
            continue;
        }

        let service = node
            .metadata
            .get("service")
            .map(String::as_str)
            .unwrap_or("unknown");
        let target_path = node
            .metadata
            .get("target_path")
            .map(String::as_str)
            .unwrap_or("unknown");
        let mut nodes = vec![node.id];
        nodes.extend(compose_volume_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_compose_volume_source_path".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Compose service `{service}` mounts local source `{source_path}` to `{target_path}` but the source path was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

pub(crate) fn compose_volume_reader_ids(graph: &CodeGraph, volume: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == volume
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "compose_volume")
        })
        .map(|edge| edge.source)
        .collect()
}

pub(crate) fn add_unresolved_github_actions_job_need_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "github_actions_job")
        {
            continue;
        }
        let workflow = node
            .metadata
            .get("workflow")
            .map(String::as_str)
            .unwrap_or("workflow");
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        for dependency in metadata_list(node, "needs") {
            if github_actions_job_exists(graph, workflow, &dependency) {
                continue;
            }
            insights.push(Insight {
                kind: "unresolved_github_actions_job_need".to_string(),
                severity: InsightSeverity::Warning,
                message: format!(
                    "GitHub Actions job `{workflow}/{job}` declares need `{dependency}` but no matching job was found"
                ),
                nodes: vec![node.id],
                edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
            });
        }
    }
}

pub(crate) fn github_actions_job_exists(graph: &CodeGraph, workflow: &str, job: &str) -> bool {
    graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Entrypoint
            && node
                .metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "github_actions_job")
            && node
                .metadata
                .get("workflow")
                .is_some_and(|value| value == workflow)
            && node.metadata.get("job").is_some_and(|value| value == job)
    })
}

pub(crate) fn add_unresolved_github_actions_local_action_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "github_actions_local_action")
        {
            continue;
        }
        let Some(local_action_path) = node
            .metadata
            .get("local_action_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "github_actions_local_action_path")
        });
        if resolved {
            continue;
        }

        let workflow = node
            .metadata
            .get("workflow")
            .map(String::as_str)
            .unwrap_or("workflow");
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        let mut nodes = vec![node.id];
        nodes.extend(github_actions_local_action_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_github_actions_local_action".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "GitHub Actions job `{workflow}/{job}` uses local action `{local_action_path}` but no matching action directory, action.yml, action.yaml, or Dockerfile was found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::DependsOn),
        });
    }
}

pub(crate) fn github_actions_local_action_reader_ids(
    graph: &CodeGraph,
    action: NodeId,
) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == action
                && edge.kind == EdgeKind::DependsOn
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "github_actions_uses")
        })
        .map(|edge| edge.source)
        .collect()
}

pub(crate) fn add_unresolved_github_actions_run_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "github_actions_run_step")
        {
            continue;
        }
        let Some(command_path) = node
            .metadata
            .get("command_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        if scanned_directory_labels(graph).contains(command_path)
            || command_path_is_installed_or_unscanned(command_path)
            || command_writes_its_path(node)
        {
            continue;
        }
        let reader_ids = github_actions_run_step_reader_ids(graph, node.id);
        let resolved = github_actions_run_path_is_resolved(graph, &reader_ids, command_path);
        if resolved {
            continue;
        }

        let workflow = node
            .metadata
            .get("workflow")
            .map(String::as_str)
            .unwrap_or("workflow");
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        let command = node
            .metadata
            .get("command")
            .map(String::as_str)
            .unwrap_or(command_path);
        let mut nodes = vec![node.id];
        nodes.extend(reader_ids);
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_github_actions_run_path".to_string(),
            // The same reading as everywhere else: a missing script is a
            // broken reference, and `_boot/dune.exe` or `./src/redis-server`
            // is missing only until the build runs.
            severity: if names_a_source_file(command_path) {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "GitHub Actions job `{workflow}/{job}` runs `{command}` but command path `{command_path}` was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

pub(crate) fn github_actions_run_step_reader_ids(graph: &CodeGraph, step: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == step
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "github_actions_run")
        })
        .map(|edge| edge.source)
        .collect()
}

pub(crate) fn github_actions_run_path_is_resolved(
    graph: &CodeGraph,
    reader_ids: &[NodeId],
    command_path: &str,
) -> bool {
    graph.edges.iter().any(|edge| {
        reader_ids.contains(&edge.source)
            && edge.kind == EdgeKind::References
            && graph
                .nodes
                .iter()
                .find(|node| node.id == edge.target)
                .is_some_and(|node| node.label == command_path)
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "github_actions_run_command_path")
    })
}

pub(crate) fn add_unresolved_gitlab_ci_job_dependency_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "gitlab_ci_job")
        {
            continue;
        }
        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        for (field, relation_label) in [("needs", "need"), ("dependencies", "dependency")] {
            for dependency in metadata_list(node, field) {
                if gitlab_ci_job_exists(graph, &dependency) {
                    continue;
                }
                insights.push(Insight {
                    kind: "unresolved_gitlab_ci_job_dependency".to_string(),
                    severity: InsightSeverity::Warning,
                    message: format!(
                        "GitLab CI job `{job}` declares {relation_label} `{dependency}` but no matching job was found"
                    ),
                    nodes: vec![node.id],
                    edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
                });
            }
        }
    }
}

pub(crate) fn gitlab_ci_job_exists(graph: &CodeGraph, job: &str) -> bool {
    graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Entrypoint
            && node
                .metadata
                .get("item_kind")
                .is_some_and(|kind| kind == "gitlab_ci_job")
            && node.metadata.get("job").is_some_and(|value| value == job)
    })
}

pub(crate) fn metadata_list(node: &Node, key: &str) -> Vec<String> {
    node.metadata
        .get(key)
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn add_unresolved_gitlab_ci_script_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "gitlab_ci_script")
        {
            continue;
        }
        let Some(command_path) = node
            .metadata
            .get("command_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        if scanned_directory_labels(graph).contains(command_path)
            || command_path_is_installed_or_unscanned(command_path)
            || command_writes_its_path(node)
        {
            continue;
        }
        let reader_ids = gitlab_ci_script_reader_ids(graph, node.id);
        let resolved = gitlab_ci_script_path_is_resolved(graph, &reader_ids, command_path);
        if resolved {
            continue;
        }

        let job = node
            .metadata
            .get("job")
            .map(String::as_str)
            .unwrap_or("job");
        let command = node
            .metadata
            .get("command")
            .map(String::as_str)
            .unwrap_or(command_path);
        let mut nodes = vec![node.id];
        nodes.extend(reader_ids);
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_gitlab_ci_script_path".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "GitLab CI job `{job}` runs `{command}` but command path `{command_path}` was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

pub(crate) fn gitlab_ci_script_reader_ids(graph: &CodeGraph, script: NodeId) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == script
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "gitlab_ci_script")
        })
        .map(|edge| edge.source)
        .collect()
}

pub(crate) fn gitlab_ci_script_path_is_resolved(
    graph: &CodeGraph,
    reader_ids: &[NodeId],
    command_path: &str,
) -> bool {
    graph.edges.iter().any(|edge| {
        reader_ids.contains(&edge.source)
            && edge.kind == EdgeKind::References
            && graph
                .nodes
                .iter()
                .find(|node| node.id == edge.target)
                .is_some_and(|node| node.label == command_path)
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|value| value == "gitlab_ci_script_command_path")
    })
}

pub(crate) fn add_unresolved_kubernetes_config_ref_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "kubernetes_config_ref")
        {
            continue;
        }
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "kubernetes_config_ref")
        });
        if resolved {
            continue;
        }

        let config_kind = node
            .metadata
            .get("config_kind")
            .map(String::as_str)
            .unwrap_or("config");
        let name = node
            .metadata
            .get("name")
            .map(String::as_str)
            .unwrap_or("unknown");
        let namespace = node
            .metadata
            .get("namespace")
            .map(String::as_str)
            .unwrap_or("default");
        let workload = node
            .metadata
            .get("workload")
            .map(String::as_str)
            .unwrap_or("unknown");
        let workload_kind = node
            .metadata
            .get("workload_kind")
            .map(String::as_str)
            .unwrap_or("workload");
        let mut nodes = vec![node.id];
        nodes.extend(kubernetes_config_ref_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_kubernetes_config_ref".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Kubernetes {workload_kind} `{workload}` references {config_kind} `{namespace}/{name}` but no matching manifest was found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::ReadsConfig),
        });
    }
}

pub(crate) fn kubernetes_config_ref_reader_ids(
    graph: &CodeGraph,
    config_ref: NodeId,
) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == config_ref
                && edge.kind == EdgeKind::ReadsConfig
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "kubernetes_config_ref")
        })
        .map(|edge| edge.source)
        .collect()
}

pub(crate) fn add_unresolved_kubernetes_ingress_backend_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "kubernetes_service_ref")
        {
            continue;
        }
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == "kubernetes_service_ref")
        });
        if resolved {
            continue;
        }

        let name = node
            .metadata
            .get("name")
            .map(String::as_str)
            .unwrap_or("unknown");
        let namespace = node
            .metadata
            .get("namespace")
            .map(String::as_str)
            .unwrap_or("default");
        let ingress = node
            .metadata
            .get("ingress")
            .map(String::as_str)
            .unwrap_or("unknown");
        let route = kubernetes_ingress_route_label(node);
        let mut nodes = vec![node.id];
        nodes.extend(kubernetes_service_ref_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_kubernetes_ingress_backend".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Kubernetes Ingress `{ingress}` routes {route} to Service `{namespace}/{name}` but no matching Service manifest was found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::References),
        });
    }
}

pub(crate) fn kubernetes_service_ref_reader_ids(
    graph: &CodeGraph,
    service_ref: NodeId,
) -> Vec<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| {
            edge.target == service_ref
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("relation")
                    .is_some_and(|relation| relation == "kubernetes_ingress_backend")
        })
        .map(|edge| edge.source)
        .collect()
}

pub(crate) fn kubernetes_ingress_route_label(node: &Node) -> String {
    let host = node.metadata.get("host").map(String::as_str).unwrap_or("*");
    let path = node.metadata.get("path").map(String::as_str).unwrap_or("/");
    format!("`{host}{path}`")
}

pub(crate) fn add_unresolved_kubernetes_service_selector_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Config
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "kubernetes_service")
        {
            continue;
        }
        let Some(selector) = node
            .metadata
            .get("selector")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if kubernetes_service_selector_is_resolved(graph, node.id) {
            continue;
        }

        let name = node
            .metadata
            .get("name")
            .map(String::as_str)
            .unwrap_or("unknown");
        let namespace = node
            .metadata
            .get("namespace")
            .map(String::as_str)
            .unwrap_or("default");
        insights.push(Insight {
            kind: "unresolved_kubernetes_service_selector".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Kubernetes Service `{namespace}/{name}` selector `{selector}` does not match any scanned workload"
            ),
            nodes: vec![node.id],
            edges: Vec::new(),
        });
    }
}

pub(crate) fn kubernetes_service_selector_is_resolved(graph: &CodeGraph, service: NodeId) -> bool {
    graph.edges.iter().any(|edge| {
        edge.source == service
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "kubernetes_service_selector")
    })
}

pub(crate) fn add_unresolved_makefile_command_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    add_unresolved_workflow_command_path_insights(
        graph,
        insights,
        "makefile_target",
        "make_command_path",
        "unresolved_makefile_command_path",
        "Makefile target",
    );
}

/// The directories the scan walked, by their repository-relative label.
///
/// `(cd ../deps && $(MAKE) distclean)` names a directory, and `make -C
/// docs/mkdocs` runs in one. Neither is a file, and neither is missing.
pub(crate) fn scanned_directory_labels(graph: &CodeGraph) -> BTreeSet<&str> {
    graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Directory)
        .map(|node| node.label.as_str())
        .collect()
}

/// A path a package manager fills in, or one no scan walks.
///
/// `vendor/bin/phpunit` arrives with composer, `node_modules/...` with npm,
/// `venv/bin/mkdocs` with a virtualenv, and `build/`, `dist/` and `target/`
/// hold what a build wrote. A repository is not missing them; whoever runs
/// the job installs or builds them first. Hidden directories are skipped by
/// every default scan, so their contents were never looked for either.
/// A command that writes or deletes a path names it all the same, so the node
/// keeps the path; nothing is missing when the path is not there yet.
pub(crate) fn command_writes_its_path(node: &Node) -> bool {
    node.metadata
        .get("command_path_role")
        .is_some_and(|role| role == "written")
}

pub(crate) fn command_path_is_installed_or_unscanned(command_path: &str) -> bool {
    command_path.split('/').any(|segment| {
        matches!(
            segment,
            "vendor"
                | "node_modules"
                | "venv"
                | "target"
                | "build"
                | "dist"
                | "_build"
                // What a test run wrote: express's CI reads `./coverage`
                // for the lcov file its own tests produced.
                | "coverage"
                | "htmlcov"
        ) || (segment.starts_with('.') && segment != "." && segment != "..")
    })
}

/// `worktree/${OLD_KONG_VERSION}` is whatever the variable holds when the
/// stack runs, so nothing can be said about whether it exists.
pub(crate) fn path_holds_an_unexpanded_variable(path: &str) -> bool {
    path.contains('$')
}

pub(crate) fn add_unresolved_workflow_command_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
    item_kind: &str,
    resolution: &str,
    insight_kind: &str,
    label_prefix: &str,
) {
    let directories = scanned_directory_labels(graph);
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != item_kind)
        {
            continue;
        }
        let Some(command_path) = node
            .metadata
            .get("command_path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|value| value == resolution)
        });
        if resolved {
            continue;
        }
        if directories.contains(command_path)
            || command_path_is_installed_or_unscanned(command_path)
            || command_writes_its_path(node)
        {
            continue;
        }

        let command = node
            .metadata
            .get("command")
            .map(String::as_str)
            .unwrap_or(command_path);
        // A command naming a source file this repository does not hold is a
        // broken reference somebody can fix. One naming what the build
        // produces — `src/redis-server`, `_boot/dune.exe`, `ca/ca.crt` — is
        // missing only until the build runs, which is most of them.
        let severity = if names_a_source_file(command_path) {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Info
        };
        insights.push(Insight {
            kind: insight_kind.to_string(),
            severity,
            message: format!(
                "{label_prefix} `{}` runs `{command}` but command path `{command_path}` was not found",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
}

/// Whether a path names a file a person writes rather than one a build
/// produces. A script, a program's source, a configuration a project keeps
/// — those are held in the repository, and a missing one is a broken
/// reference.
fn names_a_source_file(path: &str) -> bool {
    let extension = path.rsplit('/').next().unwrap_or(path).rsplit_once('.');
    extension.is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "sh" | "bash"
                | "zsh"
                | "fish"
                | "ps1"
                | "py"
                | "rb"
                | "pl"
                | "js"
                | "mjs"
                | "cjs"
                | "ts"
                | "lua"
                | "php"
                | "r"
                | "jl"
                | "sql"
                | "yml"
                | "yaml"
                | "toml"
                | "json"
                | "cfg"
                | "ini"
                | "conf"
                | "mk"
                | "make"
                | "cmake"
                | "dockerfile"
        )
    })
}

pub(crate) fn add_entrypoint_dead_end_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || entrypoint_has_outgoing_trace_edge(graph, node.id)
            || unresolved_manifest_entrypoint_target(graph, node)
            || unresolved_framework_route_handler_target(graph, node)
        {
            continue;
        }

        // A fuzzer target in `tests/thirdparty/Fuzzer` and a CUDA example
        // under `tests/cuda_example` are declared where the project keeps
        // what it does not ship, so a dead end there is a note. A manifest
        // entrypoint carries no span, so the file that contains it is the
        // one to ask about.
        let declared_in = node
            .span
            .as_ref()
            .map(|span| span.path.clone())
            .or_else(|| declaring_file_label(graph, node.id));
        let declared_by_the_project = declared_in
            .as_deref()
            .is_none_or(manifest_is_the_projects_own);
        insights.push(Insight {
            kind: "entrypoint_dead_end".to_string(),
            severity: if declared_by_the_project {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "Entrypoint `{}` has no outgoing code, config, dependency, or error flow",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
}

/// The file that declares a node, for the facts that carry no span of their
/// own: a manifest entrypoint is contained by the manifest that declares it.
fn declaring_file_label(graph: &CodeGraph, node: NodeId) -> Option<String> {
    graph
        .edges
        .iter()
        .filter(|edge| edge.target == node && edge.kind == EdgeKind::Contains)
        .find_map(|edge| {
            graph
                .nodes
                .iter()
                .find(|source| source.id == edge.source && source.kind == NodeKind::File)
                .map(|source| source.label.clone())
        })
}

pub(crate) fn entrypoint_has_outgoing_trace_edge(graph: &CodeGraph, node_id: NodeId) -> bool {
    graph
        .edges
        .iter()
        .any(|edge| edge.source == node_id && is_trace_edge(&edge.kind))
}

pub(crate) fn unresolved_manifest_entrypoint_target(graph: &CodeGraph, node: &Node) -> bool {
    if node
        .metadata
        .get("item_kind")
        .is_none_or(|kind| kind != "manifest_entrypoint")
        || node
            .metadata
            .get("target")
            .map(|target| target.trim())
            .is_none_or(str::is_empty)
    {
        return false;
    }

    !graph.edges.iter().any(|edge| {
        edge.source == node.id
            && edge.kind == EdgeKind::References
            && edge.metadata.get("relation").is_some_and(|relation| {
                matches!(relation.as_str(), "entrypoint_file" | "entrypoint_function")
            })
    })
}

pub(crate) fn unresolved_framework_route_handler_target(graph: &CodeGraph, node: &Node) -> bool {
    if node
        .metadata
        .get("item_kind")
        .is_none_or(|kind| kind != "framework_route")
        || node
            .metadata
            .get("handler")
            .map(|handler| handler.trim())
            .is_none_or(str::is_empty)
    {
        return false;
    }

    !graph.edges.iter().any(|edge| {
        edge.source == node.id
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("resolution")
                .is_some_and(|resolution| resolution == "framework_route_handler")
    })
}

/// Below this share of functions reachable from an entrypoint, the
/// `unreachable_*` findings describe the call graph's blind spots rather than
/// dead code, and the reader deserves to be told which one they are looking at.
const LOW_ENTRYPOINT_COVERAGE_NUMERATOR: usize = 1;
const LOW_ENTRYPOINT_COVERAGE_DENOMINATOR: usize = 2;

/// How many functions a project needs before its coverage is worth judging;
/// a handful of functions says nothing either way.
const MIN_FUNCTIONS_FOR_COVERAGE: usize = 20;

/// State plainly how much of the project the entrypoints actually reach.
///
/// Reachability walks resolved calls, so a project whose calls mostly go
/// through values (`client.Do(...)`) or outside the repository has few
/// reachable functions no matter how alive its code is. Without this line the
/// thousands of `unreachable_*` findings read as "this code is dead".
pub(crate) fn add_entrypoint_coverage_insights(
    graph: &CodeGraph,
    reachable: &BTreeSet<NodeId>,
    insights: &mut Vec<Insight>,
) {
    if reachable.is_empty() {
        return;
    }

    let functions: Vec<&Node> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function)
        .collect();
    if !entrypoint_coverage_is_low(graph, reachable) {
        return;
    }
    let reached = functions
        .iter()
        .filter(|function| reachable.contains(&function.id))
        .count();

    let function_ids: BTreeSet<NodeId> = functions.iter().map(|function| function.id).collect();
    let (calls, resolved_calls) = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Calls)
        .fold((0usize, 0usize), |(total, resolved), edge| {
            (
                total + 1,
                resolved + usize::from(function_ids.contains(&edge.target)),
            )
        });

    let entrypoints: Vec<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Entrypoint)
        .map(|edge| edge.target)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let coverage = percentage(reached, functions.len());
    let resolution = percentage(resolved_calls, calls);
    // A library has no `main`: its code is reached by whoever imports it, so
    // "unreachable from an entrypoint" is a statement about this repository
    // running alone, not about dead code. Saying which of the two the reader
    // is looking at costs one more walk.
    let exported: BTreeSet<NodeId> = functions
        .iter()
        .filter(|function| {
            function
                .metadata
                .get("visibility")
                .is_some_and(|visibility| visibility == "public")
        })
        .map(|function| function.id)
        .collect();
    let exported_note = if exported.is_empty() {
        String::new()
    } else {
        let mut roots: BTreeSet<NodeId> = reachable.clone();
        roots.extend(exported.iter().copied());
        let with_api = entrypoint_reachable_nodes_from(graph, &roots);
        let api_reached = functions
            .iter()
            .filter(|function| with_api.contains(&function.id))
            .count();
        format!(
            "; counting the {} exported functions as starting points reaches {}%",
            exported.len(),
            percentage(api_reached, functions.len())
        )
    };
    // Alamofire, dplyr and ecto have no `main` and no route into their own
    // code: their functions run when somebody else imports them. Coverage
    // cannot be low where there is nothing to start, so that reads as
    // context rather than as something to fix.
    let started_in_code = starts_in_its_own_code(graph);
    let opening = if started_in_code {
        ""
    } else {
        "no program entrypoint starts this project's own code, so "
    };
    insights.push(Insight {
        kind: "low_entrypoint_coverage".to_string(),
        severity: if started_in_code {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Info
        },
        message: format!(
            "{opening}entrypoints reach {reached} of {} functions ({coverage}%), and {resolution}% of calls resolve to a scanned function — the rest name a dependency, the standard library, or a method the syntax cannot type{exported_note} — treat `unreachable_*` findings as gaps in call resolution, or as a library reached through its API, before reading them as dead code",
            functions.len()
        ),
        nodes: entrypoints.into_iter().take(8).collect(),
        edges: Vec::new(),
    });
}

/// Whether anything starts this project's own code: a `main` the parser
/// recognised, or an entrypoint a manifest, framework or CI file resolved
/// onto a function.
fn starts_in_its_own_code(graph: &CodeGraph) -> bool {
    let nodes_by_id = nodes_by_id_index(graph);
    // A `main` in `extras/`, `metrics/` or `Snippets/Docs` is a demonstration
    // of the library, not the library being started: gson has five such,
    // Polly one, and reading them as programs made every library in the
    // corpus report that its own code is unreachable.
    let is_the_projects_own_program = |node: &codegraph_core::Node| {
        node.span.as_ref().is_none_or(|span| {
            !is_test_like_source_path(&span.path)
                && !is_vendored_source_path(&span.path)
                && !is_repository_tooling_source_path(&span.path)
                // A Cargo build script builds the crate; it is not the
                // crate running. serde has three.
                && !span.path.ends_with("build.rs")
        })
    };
    graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Function
            && node
                .metadata
                .get("entrypoint_kind")
                .is_some_and(|kind| kind == "program")
            && is_the_projects_own_program(node)
    }) || graph.edges.iter().any(|edge| {
        edge.metadata
            .get("relation")
            .is_some_and(|relation| relation == "entrypoint_function")
            && nodes_by_id
                .get(&edge.target)
                .is_some_and(|node| is_the_projects_own_program(node))
    })
}

/// Whether entrypoints reach enough of the code for "unreachable" to be a
/// claim about the code. Below half, [`add_entrypoint_coverage_insights`]
/// says so once, and what every other unreachability finding describes is
/// that gap rather than dead code.
fn entrypoint_coverage_is_low(graph: &CodeGraph, reachable: &BTreeSet<NodeId>) -> bool {
    let functions = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function);
    let (total, reached) = functions.fold((0usize, 0usize), |(total, reached), function| {
        (
            total + 1,
            reached + usize::from(reachable.contains(&function.id)),
        )
    });
    total >= MIN_FUNCTIONS_FOR_COVERAGE
        && reached * LOW_ENTRYPOINT_COVERAGE_DENOMINATOR < total * LOW_ENTRYPOINT_COVERAGE_NUMERATOR
}

/// Whole-percent share, reported as 0 when there is nothing to divide.
fn percentage(part: usize, whole: usize) -> usize {
    (part * 100).checked_div(whole).unwrap_or_default()
}

pub(crate) fn add_unreachable_config_read_insights(
    graph: &CodeGraph,
    reachable: &BTreeSet<NodeId>,
    insights: &mut Vec<Insight>,
) {
    if reachable.is_empty() {
        return;
    }

    // When entrypoints reach less than half the code, the coverage finding
    // has already said so, and repeating it once per configuration read
    // states the gap rather than anything about the read.
    let severity = if entrypoint_coverage_is_low(graph, reachable) {
        InsightSeverity::Info
    } else {
        InsightSeverity::Warning
    };
    let path_index = node_path_index(graph);
    let mut reads: BTreeMap<(NodeId, NodeId), Vec<usize>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if !matches!(
            edge.kind,
            EdgeKind::ReadsConfig | EdgeKind::ReadsEnvironment
        ) || reachable.contains(&edge.source)
        {
            continue;
        }
        // A test is run by a test runner and a build config by a build
        // tool: neither is reachable from a program's entrypoint, and
        // saying so describes the tooling rather than the code. 65% of
        // terraform's findings of this kind were tests, 81% of kong's, and
        // Vue's were its vite and rollup configs.
        let reader_path = path_index
            .get(&edge.source)
            .map(String::as_str)
            .or_else(|| {
                graph
                    .nodes
                    .iter()
                    .find(|node| node.id == edge.source)
                    .and_then(|node| node.span.as_ref())
                    .map(|span| span.path.as_str())
            });
        if reader_path.is_some_and(|path| {
            is_test_like_source_path(path) || is_tool_configuration_source_path(path)
        }) {
            continue;
        }

        // A key assembled at runtime names nothing to go and look at, and
        // `reads <computed name>` reads as a hole in the report.
        if node_label(graph, edge.target).is_some_and(|label| label == COMPUTED_ENVIRONMENT_KEY) {
            continue;
        }

        // Reading the same variable on two lines of one function is one
        // fact about that function, not two.
        reads
            .entry((edge.source, edge.target))
            .or_default()
            .push(index);
    }

    for ((source, target), edges) in reads {
        let reader = node_label(graph, source).unwrap_or("unknown");
        let target_label = node_label(graph, target).unwrap_or("unknown");
        insights.push(Insight {
            kind: "unreachable_config_read".to_string(),
            severity,
            message: format!(
                "`{reader}` reads `{target_label}` but is not reachable from any entrypoint"
            ),
            nodes: vec![source, target],
            edges,
        });
    }
}

pub(crate) fn add_unreachable_error_flow_insights(
    graph: &CodeGraph,
    reachable: &BTreeSet<NodeId>,
    insights: &mut Vec<Insight>,
) {
    if reachable.is_empty() {
        return;
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::MayError || reachable.contains(&edge.source) {
            continue;
        }

        let source = node_label(graph, edge.source).unwrap_or("unknown");
        let target = node_label(graph, edge.target).unwrap_or("unknown");
        insights.push(Insight {
            // Reachability is heuristic on syntactic scans and error
            // constructs are ubiquitous; keep as informational context
            // (Phase 9 dogfooding).
            kind: "unreachable_error_flow".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "`{source}` may error via `{target}` but is not reachable from any entrypoint"
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

/// How many of a file's functions it offers outwards.
fn exported_function_count(graph: &CodeGraph, file: NodeId) -> usize {
    graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Contains && edge.source == file)
        .filter_map(|edge| graph.nodes.iter().find(|node| node.id == edge.target))
        .filter(|node| {
            node.kind == NodeKind::Function
                && node
                    .metadata
                    .get("visibility")
                    .is_some_and(|visibility| visibility == "public")
        })
        .count()
}

pub(crate) fn add_unreachable_source_file_insights(
    graph: &CodeGraph,
    reachable: &BTreeSet<NodeId>,
    insights: &mut Vec<Insight>,
) {
    if reachable.is_empty() {
        return;
    }

    let source_files = graph
        .nodes
        .iter()
        .filter(|node| is_source_file_candidate(graph, node));
    for file in source_files {
        if reachable.contains(&file.id) || file_has_reachable_code(graph, file.id, reachable) {
            continue;
        }

        let language = file
            .metadata
            .get("language")
            .map(String::as_str)
            .unwrap_or("unknown");
        // Most files a library never reaches from an entrypoint are its
        // API: 129 of okio's 176, 383 of terraform's 500. Saying how many
        // of its functions are exported is the difference between "dead"
        // and "reached from outside".
        let exported = exported_function_count(graph, file.id);
        let offered = match exported {
            0 => String::new(),
            1 => "; one of its functions is exported, so callers may be outside this repository"
                .to_string(),
            _ => format!(
                "; {exported} of its functions are exported, so callers may be outside this repository"
            ),
        };
        insights.push(Insight {
            kind: "unreachable_source_file".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "`{}` contains {language} code but is not reachable from any entrypoint{offered}",
                file.label
            ),
            nodes: vec![file.id],
            edges: contained_code_edge_indexes(graph, file.id),
        });
    }
}

/// One environment variable or config key, with every read of it that the
/// scan recorded. The key is one node, so the per-read facts — above all the
/// fallback value — live on the reading edges; older graphs (and declaration
/// nodes minted by the compose/CI passes) still carry a `default_value` on the
/// node, which stands in when an edge does not name one.
#[derive(Default)]
struct ConfigKeyReads {
    /// Fallback value -> the key nodes and reading edges that use it.
    defaults: BTreeMap<String, (BTreeSet<NodeId>, BTreeSet<usize>)>,
    /// Reads with no fallback at all: the key is required there.
    required_nodes: BTreeSet<NodeId>,
    required_edges: BTreeSet<usize>,
}

fn trimmed_default(metadata: &BTreeMap<String, String>) -> Option<String> {
    metadata
        .get("default_value")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Group every environment/config read in the graph by `(kind, key)`.
fn config_key_reads(graph: &CodeGraph) -> BTreeMap<(String, String), ConfigKeyReads> {
    let mut groups: BTreeMap<(String, String), ConfigKeyReads> = BTreeMap::new();
    for node in &graph.nodes {
        if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
            continue;
        }
        let edge_kind = if node.kind == NodeKind::Environment {
            EdgeKind::ReadsEnvironment
        } else {
            EdgeKind::ReadsConfig
        };
        let node_default = trimmed_default(&node.metadata);
        let reads = incoming_edge_indexes(graph, node.id, edge_kind);
        if reads.is_empty() {
            // Nothing reads the key; only a node-level fallback can still say
            // something about it.
            if let Some(default_value) = node_default {
                groups
                    .entry((kind_name(&node.kind), node.label.clone()))
                    .or_default()
                    .defaults
                    .entry(default_value)
                    .or_default()
                    .0
                    .insert(node.id);
            }
            continue;
        }

        let entry = groups
            .entry((kind_name(&node.kind), node.label.clone()))
            .or_default();
        for index in reads {
            let edge_default = graph
                .edges
                .get(index)
                .and_then(|edge| trimmed_default(&edge.metadata));
            // A CI workflow's `env:` block with a value states what the
            // job runs with: ripgrep writes `TARGET_DIR: ./target` there
            // and every step below reads what the workflow set. An entry
            // with no value does ask the runner for one.
            if graph.edges.get(index).is_some_and(|edge| {
                edge.metadata.get("relation").map(String::as_str) == Some("ci_environment")
                    && edge.metadata.get("value_present").map(String::as_str) == Some("true")
            }) {
                continue;
            }
            match edge_default.or_else(|| node_default.clone()) {
                Some(default_value) => {
                    let slot = entry.defaults.entry(default_value).or_default();
                    slot.0.insert(node.id);
                    slot.1.insert(index);
                }
                None => {
                    entry.required_nodes.insert(node.id);
                    entry.required_edges.insert(index);
                }
            }
        }
    }
    groups
}

/// Whether every read of a key happens in vendored code: jemalloc's
/// generated `configure` reads `as_lineno` three ways, which is autoconf's
/// business rather than redis's.
fn every_read_is_vendored(graph: &CodeGraph, edges: impl IntoIterator<Item = usize>) -> bool {
    let mut files = edges.into_iter().filter_map(|index| {
        graph
            .edges
            .get(index)
            .and_then(|edge| edge.metadata.get("file"))
    });
    let mut any = false;
    files.all(|file| {
        any = true;
        is_vendored_source_path(file)
    }) && any
}

/// Whether every default the key is read with is a step of one chain: a
/// script assigning the variable to itself, again, in the same file.
fn states_one_fallback_chain(graph: &CodeGraph, reads: &ConfigKeyReads) -> bool {
    let mut files = BTreeSet::new();
    for index in reads.defaults.values().flat_map(|(_, edges)| edges.iter()) {
        let Some(edge) = graph.edges.get(*index) else {
            return false;
        };
        if edge.metadata.get("defaults_variable").map(String::as_str) != Some("true") {
            return false;
        }
        let Some(file) = edge.metadata.get("file") else {
            return false;
        };
        files.insert(file.clone());
    }
    !files.is_empty()
}

/// Whether every read of a key happens in a test: dune's blackbox setup
/// script falls back to `$PWD` in one place and `.` in another, which is
/// the suite's business rather than the program's.
fn every_read_is_test_like(graph: &CodeGraph, edges: impl IntoIterator<Item = usize>) -> bool {
    let mut files = edges.into_iter().filter_map(|index| {
        graph
            .edges
            .get(index)
            .and_then(|edge| edge.metadata.get("file"))
    });
    let mut any = false;
    files.all(|file| {
        any = true;
        is_test_like_source_path(file)
    }) && any
}

pub(crate) fn add_conflicting_config_default_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for ((kind, label), reads) in config_key_reads(graph) {
        if reads.defaults.len() < 2 {
            continue;
        }
        // `X=${X:-$(git config ...)}` followed by `X=${X:-origin}` is one
        // chain of fallbacks, not two answers to the same question: dune
        // writes exactly that in three release scripts.
        if states_one_fallback_chain(graph, &reads) {
            continue;
        }

        let nodes: BTreeSet<NodeId> = reads
            .defaults
            .values()
            .flat_map(|(nodes, _)| nodes.iter().copied())
            .collect();
        let edges: BTreeSet<usize> = reads
            .defaults
            .values()
            .flat_map(|(_, edges)| edges.iter().copied())
            .collect();
        let values = format_backtick_list(reads.defaults.keys().map(String::as_str), 8);

        let vendored = every_read_is_vendored(graph, edges.iter().copied())
            || every_read_is_test_like(graph, edges.iter().copied());
        insights.push(Insight {
            kind: "conflicting_config_default".to_string(),
            severity: if vendored {
                InsightSeverity::Info
            } else {
                InsightSeverity::Warning
            },
            message: format!("{kind} `{label}` is read with multiple fallback values: {values}"),
            nodes: nodes.into_iter().collect(),
            edges: edges.into_iter().collect(),
        });
    }
}

/// What each shell script sources, by file label. kong keeps its release
/// defaults in `scripts/release-lib.sh` and `scripts/make-release` opens
/// with `source "$(dirname "$0")/release-lib.sh"`: the reads below that
/// line are answered by the file it pulled in, and the path is written
/// with a shell expansion no scan can resolve, so the name it ends with
/// is what identifies it.
fn sourced_shell_files(graph: &CodeGraph) -> BTreeMap<String, BTreeSet<String>> {
    let files: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .map(|node| node.label.as_str())
        .collect();
    let nodes_by_id = node_index(graph);
    let mut sourced: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.kind != EdgeKind::Imports {
            continue;
        }
        let (Some(source), Some(import)) =
            (nodes_by_id.get(&edge.source), nodes_by_id.get(&edge.target))
        else {
            continue;
        };
        if source.kind != NodeKind::File
            || import.metadata.get("language").map(String::as_str) != Some("bash")
        {
            continue;
        }
        let statement = import.label.trim();
        let Some(rest) = statement
            .strip_prefix("source ")
            .or_else(|| statement.strip_prefix(". "))
        else {
            continue;
        };
        let Some(name) = rest
            .trim()
            .trim_matches(['"', '\''])
            .rsplit('/')
            .next()
            .map(str::trim)
            .filter(|name| !name.is_empty() && !name.contains(['$', '*', '"']))
        else {
            continue;
        };
        let entry = sourced.entry(source.label.clone()).or_default();
        entry.extend(
            files
                .iter()
                .filter(|file| {
                    file.rsplit('/')
                        .next()
                        .is_some_and(|candidate| candidate == name)
                })
                .map(|file| file.to_string()),
        );
    }
    sourced
}

/// The first line in each file where a script gives the key a default by
/// assigning it to itself.
fn defaulting_assignment_lines(graph: &CodeGraph, reads: &ConfigKeyReads) -> BTreeMap<String, u32> {
    let mut guarded: BTreeMap<String, u32> = BTreeMap::new();
    let all_edges = reads
        .defaults
        .values()
        .flat_map(|(_, edges)| edges.iter())
        .chain(reads.required_edges.iter());
    for index in all_edges {
        let Some(edge) = graph.edges.get(*index) else {
            continue;
        };
        let Some(file) = edge.metadata.get("file") else {
            continue;
        };
        // Either the read hands the variable its own default, or the file
        // assigns the name outright somewhere above.
        let line = if edge.metadata.get("defaults_variable").map(String::as_str) == Some("true") {
            edge.metadata.get("line").and_then(|line| line.parse().ok())
        } else {
            edge.metadata
                .get("assigned_at_line")
                .and_then(|line| line.parse().ok())
        };
        let Some(line) = line else {
            continue;
        };
        let slot = guarded.entry(file.clone()).or_insert(line);
        *slot = (*slot).min(line);
    }
    guarded
}

/// Whether a read comes after the line that gave the key a default in the
/// same file. A read inside a function is not ordered against that line at
/// all: dune declares `confirm ()` above the assignments and calls it
/// below, so the body reads what the script put there however the file is
/// laid out.
fn read_is_guarded(
    graph: &CodeGraph,
    index: usize,
    guarded: &BTreeMap<String, u32>,
    sourced: &BTreeMap<String, BTreeSet<String>>,
    nodes_by_id: &BTreeMap<NodeId, &Node>,
) -> bool {
    let Some(edge) = graph.edges.get(index) else {
        return false;
    };
    let (Some(file), Some(line)) = (
        edge.metadata.get("file"),
        edge.metadata
            .get("line")
            .and_then(|line| line.parse::<u32>().ok()),
    ) else {
        return false;
    };
    let Some(guard) = guarded.get(file) else {
        // A script the file sources may have answered for the key before
        // this line ran.
        return sourced
            .get(file)
            .is_some_and(|sourced| sourced.iter().any(|file| guarded.contains_key(file)));
    };
    let inside_a_function = nodes_by_id
        .get(&edge.source)
        .is_some_and(|node| node.kind == NodeKind::Function);
    inside_a_function || line > *guard
}

pub(crate) fn add_mixed_config_requirement_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let sourced = sourced_shell_files(graph);
    let nodes_by_id = node_index(graph);
    for ((kind, label), reads) in config_key_reads(graph) {
        if reads.required_edges.is_empty() || reads.defaults.is_empty() {
            continue;
        }
        // A script that opens with `GOPATH=${GOPATH:-$(go env GOPATH)}`
        // has given the variable a value, and every `$GOPATH` below that
        // line reads what the script itself put there. 24 of the corpus's
        // 25 same-file findings were this one shell idiom.
        let guarded = defaulting_assignment_lines(graph, &reads);
        let required_edges: BTreeSet<usize> = reads
            .required_edges
            .iter()
            .copied()
            .filter(|index| !read_is_guarded(graph, *index, &guarded, &sourced, &nodes_by_id))
            .collect();
        if required_edges.is_empty() {
            continue;
        }

        let mut nodes = reads.required_nodes.clone();
        nodes.extend(
            reads
                .defaults
                .values()
                .flat_map(|(default_nodes, _)| default_nodes.iter().copied()),
        );
        let mut edges = required_edges.clone();
        edges.extend(
            reads
                .defaults
                .values()
                .flat_map(|(_, default_edges)| default_edges.iter().copied()),
        );
        let values = format_backtick_list(reads.defaults.keys().map(String::as_str), 8);

        let vendored = every_read_is_vendored(graph, edges.iter().copied())
            || every_read_is_test_like(graph, edges.iter().copied());
        insights.push(Insight {
            kind: "mixed_config_requirement".to_string(),
            severity: if vendored {
                InsightSeverity::Info
            } else {
                InsightSeverity::Warning
            },
            message: format!(
                "{kind} `{label}` is read both as required and with fallback values: {values}"
            ),
            nodes: nodes.into_iter().collect(),
            edges: edges.into_iter().collect(),
        });
    }
}

pub(crate) fn add_sensitive_config_default_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
            continue;
        }
        let edge_kind = if node.kind == NodeKind::Environment {
            EdgeKind::ReadsEnvironment
        } else {
            EdgeKind::ReadsConfig
        };
        let node_default = trimmed_default(&node.metadata);
        let edges: Vec<usize> = incoming_edge_indexes(graph, node.id, edge_kind)
            .into_iter()
            .filter(|index| {
                graph
                    .edges
                    .get(*index)
                    .and_then(|edge| trimmed_default(&edge.metadata))
                    .or_else(|| node_default.clone())
                    .is_some_and(|default_value| {
                        sensitive_config_default_candidate(&node.label, &default_value)
                    })
            })
            .collect();
        if edges.is_empty() {
            continue;
        }

        let kind = kind_name(&node.kind);
        insights.push(Insight {
            kind: "sensitive_config_default".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{kind} `{}` looks sensitive and has a non-empty fallback value",
                node.label
            ),
            nodes: std::iter::once(node.id)
                .chain(
                    edges
                        .iter()
                        .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
                )
                .collect(),
            edges,
        });
    }
}

/// Warn about Dart platform channels with no matching native handler — but
/// only when the repository actually contains native host sources, so pure
/// Dart packages and plugin consumers stay quiet.
pub(crate) fn add_unmatched_platform_channel_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let has_native_sources = graph.nodes.iter().any(|node| {
        node.kind == NodeKind::File
            && node.label.rsplit('.').next().is_some_and(|extension| {
                matches!(extension, "kt" | "kts" | "java" | "swift" | "m" | "mm")
            })
    });
    if !has_native_sources {
        return;
    }
    let mut reported = BTreeSet::new();
    for node in &graph.nodes {
        if node.metadata.get("item_kind").map(String::as_str) != Some("platform_channel")
            || node.metadata.get("source").map(String::as_str) != Some("dart")
        {
            continue;
        }
        if node
            .metadata
            .keys()
            .any(|key| key.starts_with("native_handler_"))
        {
            continue;
        }
        let Some(name) = node.metadata.get("channel_name") else {
            continue;
        };
        let kind = node
            .metadata
            .get("channel_kind")
            .map(String::as_str)
            .unwrap_or("method");
        if !reported.insert((name.clone(), kind.to_string())) {
            continue;
        }
        insights.push(Insight {
            kind: "unmatched_platform_channel".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Flutter {kind} channel `{name}` has no matching native Android/iOS handler registration"
            ),
            nodes: vec![node.id],
            edges: Vec::new(),
        });
    }
}

pub(crate) fn add_undeclared_flutter_asset_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let nodes_by_id = node_index(graph);
    let declared_assets = flutter_declared_assets(graph);
    if declared_assets.is_empty() {
        return;
    }

    let mut reported = BTreeSet::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::ReadsConfig {
            continue;
        }
        let Some(target) = nodes_by_id.get(&edge.target).copied() else {
            continue;
        };
        let Some(asset_path) = flutter_asset_read_path(target) else {
            continue;
        };
        if flutter_asset_is_declared(&asset_path, &declared_assets) {
            continue;
        }
        if !reported.insert(asset_path.clone()) {
            continue;
        }
        let reader = node_label(graph, edge.source).unwrap_or("unknown");
        insights.push(Insight {
            kind: "undeclared_flutter_asset".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "`{reader}` reads Flutter asset `{asset_path}` but no matching `pubspec.yaml` asset declaration was found"
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![edge_index],
        });
    }
}

pub(crate) fn flutter_declared_assets(graph: &CodeGraph) -> Vec<String> {
    graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Config
                && node
                    .metadata
                    .get("item_kind")
                    .is_some_and(|value| value == "flutter_asset")
        })
        .filter_map(|node| {
            node.metadata.get("asset_path").cloned().or_else(|| {
                node.label
                    .strip_prefix("flutter asset:")
                    .map(str::to_string)
            })
        })
        .collect()
}

pub(crate) fn flutter_asset_read_path(node: &Node) -> Option<String> {
    if node.kind != NodeKind::Config {
        return None;
    }
    if node
        .metadata
        .get("config_kind")
        .is_some_and(|value| value == "flutter_asset_read")
    {
        return node.metadata.get("value").cloned().or_else(|| {
            node.label
                .strip_prefix("flutter asset read:")
                .map(str::to_string)
        });
    }
    let label = node.label.trim();
    (looks_like_flutter_asset_path(label)).then(|| label.to_string())
}

pub(crate) fn looks_like_flutter_asset_path(path: &str) -> bool {
    let path = path.trim();
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains("://")
        && (path.starts_with("assets/")
            || path.starts_with("asset/")
            || path.contains("/assets/")
            || path.contains("/asset/"))
}

pub(crate) fn flutter_asset_is_declared(asset_path: &str, declarations: &[String]) -> bool {
    declarations.iter().any(|declared| {
        let declared = declared.trim();
        !declared.is_empty()
            && (asset_path == declared
                || (declared.ends_with('/') && asset_path.starts_with(declared)))
    })
}

pub(crate) fn add_rationale_risk_comment_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "rationale_comment")
        {
            continue;
        }
        let Some(kind) = node.metadata.get("rationale_kind").map(String::as_str) else {
            continue;
        };
        let edges = incoming_edge_indexes(graph, node.id, EdgeKind::Contains);
        let path = node.span.as_ref().map(|span| span.path.as_str());
        let location = node
            .span
            .as_ref()
            .map(|span| format!("{}:{}", span.path, span.start_line))
            .unwrap_or_else(|| "unknown location".to_string());
        // A note left in vendored code is upstream's: redis carries
        // jemalloc's FIXMEs and dune carries opam's, and reading them as
        // loudly as a project's own buries the ones somebody here can act
        // on. A note in a fixture or a test is about that test — 44 of the
        // corpus's 207 — and the same reasoning applies.
        let elsewhere = path
            .is_some_and(|path| is_vendored_source_path(path) || is_test_like_source_path(path));
        let severity = match kind {
            "security" if !elsewhere => InsightSeverity::Error,
            "security" => InsightSeverity::Warning,
            "fixme" | "hack" | "bug" | "xxx" if !elsewhere => InsightSeverity::Warning,
            "fixme" | "hack" | "bug" | "xxx" => InsightSeverity::Info,
            _ => continue,
        };
        insights.push(Insight {
            kind: "rationale_risk_comment".to_string(),
            severity,
            message: format!(
                "{} comment `{}` should be reviewed at {location}",
                kind.to_ascii_uppercase(),
                node.label
            ),
            nodes: std::iter::once(node.id)
                .chain(
                    edges
                        .iter()
                        .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
                )
                .collect(),
            edges,
        });
    }
}

pub(crate) fn add_sensitive_ci_environment_literal_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Environment || !sensitive_config_label(&node.label) {
            continue;
        }
        // One node holds the variable; what a workflow assigns to it is
        // written on the edge from the job that sets it.
        let edges: Vec<usize> = incoming_edge_indexes(graph, node.id, EdgeKind::ReadsEnvironment)
            .into_iter()
            .filter(|index| {
                graph.edges.get(*index).is_some_and(|edge| {
                    edge.metadata
                        .get("item_kind")
                        .is_some_and(|kind| kind == "ci_environment")
                        && edge
                            .metadata
                            .get("value_kind")
                            .is_some_and(|kind| kind == "literal")
                })
            })
            .collect();
        if edges.is_empty() {
            continue;
        }
        let assignment = edges.first().and_then(|index| graph.edges.get(*index));
        let source = assignment
            .and_then(|edge| edge.metadata.get("source"))
            .map(String::as_str)
            .unwrap_or("ci");
        let scope = assignment
            .and_then(|edge| edge.metadata.get("scope"))
            .map(String::as_str)
            .unwrap_or("job");
        insights.push(Insight {
            kind: "sensitive_ci_environment_literal".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{source} {scope} environment `{}` looks sensitive and is assigned a literal value",
                node.label
            ),
            nodes: std::iter::once(node.id)
                .chain(
                    edges
                        .iter()
                        .filter_map(|index| graph.edges.get(*index).map(|edge| edge.source)),
                )
                .collect(),
            edges,
        });
    }
}

pub(crate) fn sensitive_config_default_candidate(label: &str, default_value: &str) -> bool {
    sensitive_config_label(label) || credential_like_default(default_value)
}

pub(crate) fn sensitive_config_label(label: &str) -> bool {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("public_key")
        && !normalized.contains("private_key")
        && !normalized.contains("secret")
    {
        return false;
    }

    [
        "password",
        "passwd",
        "passphrase",
        "secret",
        "token",
        "credential",
        "private_key",
        "api_key",
        "access_key",
        "signing_key",
        "encryption_key",
        "jwt",
    ]
    .iter()
    .any(|indicator| normalized.contains(indicator))
}

pub(crate) fn credential_like_default(default_value: &str) -> bool {
    let normalized = default_value
        .trim()
        .trim_matches(|character| matches!(character, '"' | '\'' | '`'))
        .to_ascii_lowercase();
    (normalized.contains("://") && normalized.contains('@'))
        || normalized.contains("password=")
        || normalized.contains("passwd=")
        || normalized.contains("token=")
        || normalized.contains("secret=")
        || placeholder_credential_default(&normalized)
}

pub(crate) fn placeholder_credential_default(default_value: &str) -> bool {
    let tokens = default_value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let compact = tokens.join("");
    if matches!(
        compact.as_str(),
        "changeme"
            | "changeit"
            | "replaceme"
            | "replaceit"
            | "replacewithsecret"
            | "replacewithtoken"
            | "replacewithapikey"
            | "yourpassword"
            | "yoursecret"
            | "yourtoken"
            | "yourapikey"
            | "examplesecret"
            | "exampletoken"
            | "exampleapikey"
            | "dummysecret"
            | "dummytoken"
            | "dummyapikey"
            | "todosecret"
            | "todotoken"
            | "fixmesecret"
            | "fixmetoken"
    ) {
        return true;
    }

    let has_placeholder = tokens.iter().any(|token| {
        matches!(
            *token,
            "changeme"
                | "changeit"
                | "replace"
                | "replaceit"
                | "replaceme"
                | "todo"
                | "fixme"
                | "example"
                | "sample"
                | "dummy"
                | "placeholder"
                | "your"
        )
    });
    let has_credential = tokens.iter().any(|token| {
        matches!(
            *token,
            "password"
                | "passwd"
                | "passphrase"
                | "secret"
                | "token"
                | "credential"
                | "credentials"
                | "apikey"
                | "jwt"
        ) || *token == "key"
    });
    has_placeholder && has_credential
}

/// The package ids the project's own manifests claim for it, as recorded
/// on the repository node by the scan.
pub(crate) fn project_own_package_ids(graph: &CodeGraph) -> BTreeSet<String> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == graph.root)
        .and_then(|node| node.metadata.get("own_package_ids"))
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn add_undeclared_import_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let nodes_by_id = node_index(graph);
    let own_packages = project_own_package_ids(graph);
    let mut grouped: BTreeMap<(String, String), UndeclaredImportGroup> = BTreeMap::new();
    let declared = declared_package_ids(graph);
    let declared_ecosystems: BTreeSet<_> = declared
        .iter()
        .filter_map(|package_id| {
            package_id
                .split_once(':')
                .map(|(ecosystem, _)| ecosystem.to_string())
        })
        .collect();

    if declared_ecosystems.is_empty() {
        return;
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::Imports {
            continue;
        }

        let Some(source_node) = nodes_by_id.get(&edge.source).copied() else {
            continue;
        };
        if is_dependency_manifest_source_path(&source_node.label) {
            continue;
        }
        let Some(import_node) = nodes_by_id.get(&edge.target).copied() else {
            continue;
        };
        if import_node
            .metadata
            .get("import_scope")
            .is_some_and(|scope| scope == "local")
            // `try: import simplejson as json / except ImportError: import
            // json` is a project stating that it runs without the package,
            // which is why it does not declare it.
            || import_node
                .metadata
                .get("optional")
                .is_some_and(|value| value == "true")
        {
            continue;
        }
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        if matches!(language, "c" | "cpp") {
            continue;
        }
        // An import the scan resolved inside the repository is not an
        // outside dependency to declare. Vue's `@vue/runtime-test` is one
        // of its own packages, and 54 of the corpus warnings said the
        // project had failed to declare a dependency on itself.
        if import_node
            .metadata
            .get("import_scope")
            .is_some_and(|scope| scope == "local" || scope == "workspace")
        {
            continue;
        }
        // `import(`${pkgDir}/package.json`)` names no package: the
        // specifier is built at runtime, and reading the template as a name
        // produced findings about `${pkgbasepath}`.
        if import_node.label.contains("${") {
            continue;
        }
        let imports = import_package_candidates(language, &import_node.label, &declared_ecosystems);
        if imports.is_empty() {
            continue;
        }
        // `mod cli;` puts `cli` in the file's own scope, and `use cli::*`
        // beneath it names that module rather than a crate anybody
        // published.
        if language == "rust"
            && imports
                .iter()
                .any(|import| declares_module(graph, &source_node.label, import.package.as_str()))
        {
            continue;
        }
        // Nothing declares a dependency on itself. guzzle's own sources
        // `use GuzzleHttp\…`, which names the package its composer.json
        // claims — 363 findings said the project had failed to require
        // itself.
        if imports.iter().any(|import| {
            own_packages.contains(&format!("{}:{}", import.ecosystem, import.package))
        }) {
            continue;
        }
        // `Psr\Http\Message\RequestInterface` could come from psr/http or
        // from psr/http-message, and only the second is a package anybody
        // publishes, so name the most specific candidate.
        let Some(import) = imports
            .iter()
            .filter(|import| declared_ecosystems.contains(import.ecosystem.as_str()))
            .max_by_key(|import| import.package.len())
        else {
            continue;
        };
        if imports
            .iter()
            .any(|import| is_declared_package(&declared, &import.ecosystem, &import.package))
        {
            continue;
        }
        // `import type { TrustedHTML } from 'trusted-types/lib'` is served
        // by `@types/trusted-types`, which is what vue declares. A type-only
        // import needs the types package and nothing else at run time.
        if import_node.label.trim_start().starts_with("import type ")
            && imports.iter().any(|import| {
                is_declared_package(
                    &declared,
                    &import.ecosystem,
                    &types_package_name(&import.package),
                )
            })
        {
            continue;
        }

        grouped
            .entry((import.ecosystem.clone(), import.package.clone()))
            .or_insert_with(|| UndeclaredImportGroup {
                sources: BTreeSet::new(),
                production_source: false,
                nodes: Vec::new(),
                edges: Vec::new(),
            })
            .record(source_node.label.as_str(), edge.source, edge.target, index);
    }

    // One finding per package rather than per import site: guzzle imports
    // `psr/http` from 169 places, and 169 identical findings say no more
    // than one that counts them.
    for ((ecosystem, package), group) in grouped {
        // `format_backtick_list` counts what it leaves out, so counting it
        // again here said "and 58 more and 58 more".
        let where_from = format_backtick_list(group.sources.iter().map(String::as_str), 3);
        insights.push(Insight {
            kind: "undeclared_external_import".to_string(),
            // A test's fixture package and an example script's numpy are
            // not the program's dependencies, so they read as notes rather
            // than warnings, as an unresolved import from a test does.
            severity: if group.production_source {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "`{package}` is imported from {where_from} but no matching {ecosystem} dependency was found"
            ),
            nodes: group.nodes,
            edges: group.edges,
        });
    }
}

/// Where one undeclared package is imported from, so the finding can name
/// the package once instead of once per import site.
struct UndeclaredImportGroup {
    sources: BTreeSet<String>,
    /// Whether any importer is the program itself rather than its tests,
    /// examples, docs or build scripts.
    production_source: bool,
    nodes: Vec<NodeId>,
    edges: Vec<usize>,
}

impl UndeclaredImportGroup {
    fn record(&mut self, source: &str, source_id: NodeId, import_id: NodeId, edge: usize) {
        self.production_source |= !is_test_like_source_path(source)
            && !is_repository_tooling_source_path(source)
            && !is_vendored_source_path(source);
        self.sources.insert(source.to_string());
        for node in [source_id, import_id] {
            if !self.nodes.contains(&node) {
                self.nodes.push(node);
            }
        }
        self.edges.push(edge);
    }
}

pub(crate) fn add_unused_dependency_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let nodes_by_id = node_index(graph);
    let used_packages = dependency_usage_packages(graph);
    let used_ecosystems: BTreeSet<_> = used_packages
        .iter()
        .map(|(_, import)| import.ecosystem.as_str())
        .collect();
    if used_ecosystems.is_empty() {
        return;
    }

    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn
            || edge
                .metadata
                .get("dependency_kind")
                .is_none_or(|kind| kind != "runtime")
        {
            continue;
        }

        let Some(dependency) = nodes_by_id.get(&edge.target).copied() else {
            continue;
        };
        let Some(package_id) = dependency.metadata.get("package_id") else {
            continue;
        };
        let Some((ecosystem, _)) = package_id.split_once(':') else {
            continue;
        };
        if !used_ecosystems.contains(ecosystem) {
            continue;
        }
        if used_packages
            .iter()
            .any(|(_, import)| import_matches_package_id(package_id, import))
        {
            continue;
        }

        let source = graph
            .nodes
            .iter()
            .find(|node| node.id == edge.source)
            .map(|node| node.label.as_str())
            .unwrap_or("unknown");
        insights.push(Insight {
            kind: "unused_declared_dependency".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "`{source}` declares `{}` but no matching import was found",
                dependency.label
            ),
            nodes: vec![edge.source, edge.target],
            edges: vec![index],
        });
    }
}

/// Whether a manifest belongs to the program rather than to an example app,
/// a test fixture or something the project vendored. A dart workspace's
/// `pkgs/ok_http/example/pubspec.yaml` pinning its own version of a package
/// is the example's business.
fn manifest_is_the_projects_own(label: &str) -> bool {
    !is_test_like_source_path(label)
        && !is_vendored_source_path(label)
        && !is_repository_tooling_source_path(label)
}

/// A version constraint read as the range of versions it admits, so two
/// declarations can be asked whether any single version satisfies both.
/// `anyhow 1.0.75` and `anyhow 1.0.103` are one dependency in a Cargo
/// workspace; `blinker ==1.6.2` and `blinker >=1.9.0` are two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VersionRange {
    low: Option<(u64, u64, u64)>,
    low_inclusive: bool,
    high: Option<(u64, u64, u64)>,
    high_inclusive: bool,
}

impl VersionRange {
    fn any() -> Self {
        Self {
            low: None,
            low_inclusive: true,
            high: None,
            high_inclusive: true,
        }
    }

    fn intersect(self, other: Self) -> Option<Self> {
        let (low, low_inclusive) = match (self.low, other.low) {
            (Some(left), Some(right)) if left == right => {
                (Some(left), self.low_inclusive && other.low_inclusive)
            }
            (Some(left), Some(right)) if left > right => (Some(left), self.low_inclusive),
            (Some(_), Some(right)) => (Some(right), other.low_inclusive),
            (Some(left), None) => (Some(left), self.low_inclusive),
            (None, Some(right)) => (Some(right), other.low_inclusive),
            (None, None) => (None, true),
        };
        let (high, high_inclusive) = match (self.high, other.high) {
            (Some(left), Some(right)) if left == right => {
                (Some(left), self.high_inclusive && other.high_inclusive)
            }
            (Some(left), Some(right)) if left < right => (Some(left), self.high_inclusive),
            (Some(_), Some(right)) => (Some(right), other.high_inclusive),
            (Some(left), None) => (Some(left), self.high_inclusive),
            (None, Some(right)) => (Some(right), other.high_inclusive),
            (None, None) => (None, true),
        };
        let empty = match (low, high) {
            (Some(low), Some(high)) => {
                low > high || (low == high && !(low_inclusive && high_inclusive))
            }
            _ => false,
        };
        (!empty).then_some(Self {
            low,
            low_inclusive,
            high,
            high_inclusive,
        })
    }
}

/// The next version a caret range excludes: `^1.2.3` stops at 2.0.0, and
/// `^0.4.1` at 0.5.0, because a leading zero makes the next component the
/// breaking one.
fn caret_upper_bound(version: (u64, u64, u64)) -> (u64, u64, u64) {
    match version {
        (0, 0, patch) => (0, 0, patch + 1),
        (0, minor, _) => (0, minor + 1, 0),
        (major, _, _) => (major + 1, 0, 0),
    }
}

fn parse_version_number(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim();
    let core = text
        .split(['+', '-'])
        .next()
        .unwrap_or(text)
        .trim_end_matches('.');
    let mut parts = core.split('.');
    let major = parts.next()?.trim().parse::<u64>().ok()?;
    let minor = parts
        .next()
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .unwrap_or(0);
    let patch = parts
        .next()
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .unwrap_or(0);
    Some((major, minor, patch))
}

/// One clause of a constraint, such as `>=1.2` or `^0.4` or `1.0.75`.
fn parse_version_clause(clause: &str, bare_is_exact: bool) -> Option<VersionRange> {
    let clause = clause.trim();
    if clause.is_empty() || clause == "*" || clause.eq_ignore_ascii_case("any") {
        return Some(VersionRange::any());
    }
    let (operator, rest) = if let Some(rest) = clause.strip_prefix(">=") {
        (">=", rest)
    } else if let Some(rest) = clause.strip_prefix("<=") {
        ("<=", rest)
    } else if let Some(rest) = clause.strip_prefix("==") {
        ("==", rest)
    } else if let Some(rest) = clause.strip_prefix("!=") {
        return parse_version_number(rest).map(|_| VersionRange::any());
    } else if let Some(rest) = clause.strip_prefix("~=") {
        ("~", rest)
    } else if let Some(rest) = clause.strip_prefix('>') {
        (">", rest)
    } else if let Some(rest) = clause.strip_prefix('<') {
        ("<", rest)
    } else if let Some(rest) = clause.strip_prefix('^') {
        ("^", rest)
    } else if let Some(rest) = clause.strip_prefix('~') {
        ("~", rest)
    } else if let Some(rest) = clause.strip_prefix('=') {
        ("==", rest)
    } else {
        ("", clause)
    };
    let version = parse_version_number(rest)?;
    Some(match operator {
        ">=" => VersionRange {
            low: Some(version),
            low_inclusive: true,
            high: None,
            high_inclusive: true,
        },
        ">" => VersionRange {
            low: Some(version),
            low_inclusive: false,
            high: None,
            high_inclusive: true,
        },
        "<=" => VersionRange {
            low: None,
            low_inclusive: true,
            high: Some(version),
            high_inclusive: true,
        },
        "<" => VersionRange {
            low: None,
            low_inclusive: true,
            high: Some(version),
            high_inclusive: false,
        },
        "==" => VersionRange {
            low: Some(version),
            low_inclusive: true,
            high: Some(version),
            high_inclusive: true,
        },
        "^" => VersionRange {
            low: Some(version),
            low_inclusive: true,
            high: Some(caret_upper_bound(version)),
            high_inclusive: false,
        },
        "~" => VersionRange {
            low: Some(version),
            low_inclusive: true,
            high: Some((version.0, version.1 + 1, 0)),
            high_inclusive: false,
        },
        _ if bare_is_exact => VersionRange {
            low: Some(version),
            low_inclusive: true,
            high: Some(version),
            high_inclusive: true,
        },
        // Cargo and pub read a bare version as "compatible with this one".
        _ => VersionRange {
            low: Some(version),
            low_inclusive: true,
            high: Some(caret_upper_bound(version)),
            high_inclusive: false,
        },
    })
}

/// The whole constraint, which may be several clauses that all have to hold
/// (`>=0.1.5 <2.0.0`), or several the resolver may choose between (`^1 || ^2`).
fn parse_version_constraint(constraint: &str, bare_is_exact: bool) -> Option<VersionRange> {
    // An alternation admits everything its widest branch does; reading only
    // the first branch would call `^1 || ^2` a conflict with `^2`.
    if constraint.contains("||") {
        return Some(VersionRange::any());
    }
    constraint
        .split([',', ' '])
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .try_fold(VersionRange::any(), |range, clause| {
            parse_version_clause(clause, bare_is_exact).and_then(|clause| range.intersect(clause))
        })
}

/// Whether one version could satisfy every constraint a package was declared
/// with. An unreadable constraint answers "unknown", which reports rather
/// than hides.
pub(crate) fn constraints_can_agree(ecosystem: &str, constraints: &[&str]) -> bool {
    // npm and python pin an exact version when no operator is written;
    // cargo and pub read the same text as a compatible range.
    let bare_is_exact = matches!(ecosystem, "npm" | "python" | "composer" | "go");
    let mut range = VersionRange::any();
    for constraint in constraints {
        let Some(parsed) = parse_version_constraint(constraint, bare_is_exact) else {
            return false;
        };
        let Some(narrowed) = range.intersect(parsed) else {
            return false;
        };
        range = narrowed;
    }
    true
}

pub(crate) fn add_conflicting_dependency_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let mut groups: BTreeMap<NodeId, Vec<(usize, String)>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn {
            continue;
        }
        if edge
            .metadata
            .get("dependency_version_kind")
            .is_some_and(|kind| kind == "locked")
        {
            continue;
        }
        let Some(version) = edge
            .metadata
            .get("dependency_version")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        // `catalog:` and `workspace:*` say where to get the package, and
        // the version they stand for is written somewhere else. Reading
        // one as a constraint makes every catalogued package disagree with
        // itself.
        if names_a_dependency_source(version) {
            continue;
        }
        groups
            .entry(edge.target)
            .or_default()
            .push((index, version.to_string()));
    }

    for (target, declarations) in groups {
        let distinct_versions: BTreeSet<_> = declarations
            .iter()
            .map(|(_, version)| version.as_str())
            .collect();
        if distinct_versions.len() < 2 {
            continue;
        }
        // Two texts are not two requirements: a Cargo workspace where one
        // crate asks for `anyhow 1.0.75` and another for `1.0.103` installs
        // one version that satisfies both, and 25 of the corpora's 32
        // findings were that. `blinker ==1.6.2` against `>=1.9.0` is not.
        let ecosystem = graph
            .nodes
            .iter()
            .find(|node| node.id == target)
            .and_then(|node| node.metadata.get("package_id"))
            .and_then(|package_id| package_id.split_once(':').map(|(head, _)| head))
            .unwrap_or_default();
        if constraints_can_agree(
            ecosystem,
            &distinct_versions.iter().copied().collect::<Vec<_>>(),
        ) {
            continue;
        }

        // Two Go modules resolve independently: `_examples/go.mod` asking
        // for `golang.org/x/text v0.38.0` says nothing about the module
        // beside it asking for `v0.41.0`, because neither build ever sees
        // the other's requirement. Seven of gqlgen's eight findings were
        // that.
        let manifest_labels: BTreeSet<&str> = declarations
            .iter()
            .filter_map(|(index, _)| graph.edges.get(*index))
            .filter_map(|edge| node_label(graph, edge.source))
            .collect();
        if manifest_labels.len() > 1
            && manifest_labels
                .iter()
                .all(|label| label.ends_with("go.mod") || label.ends_with("go.sum"))
        {
            continue;
        }

        let mut nodes = BTreeSet::from([target]);
        let edge_indexes: Vec<_> = declarations
            .iter()
            .map(|(index, _)| {
                if let Some(edge) = graph.edges.get(*index) {
                    nodes.insert(edge.source);
                }
                *index
            })
            .collect();
        let versions = distinct_versions
            .iter()
            .take(4)
            .map(|version| format!("`{version}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let package = node_label(graph, target).unwrap_or("unknown");
        // Which files disagree is the whole of what a reader needs, and
        // three quarters of these are a workspace where an example app and
        // the library it shows off each pinned their own version.
        let manifests = format_backtick_list(
            declarations
                .iter()
                .filter_map(|(index, _)| graph.edges.get(*index))
                .filter_map(|edge| node_label(graph, edge.source))
                .collect::<BTreeSet<_>>()
                .into_iter(),
            3,
        );
        let declared_by_the_project = declarations
            .iter()
            .filter_map(|(index, _)| graph.edges.get(*index))
            .filter_map(|edge| node_label(graph, edge.source))
            .any(manifest_is_the_projects_own);
        insights.push(Insight {
            kind: "conflicting_dependency_declaration".to_string(),
            severity: if declared_by_the_project {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "Dependency `{package}` is declared with multiple constraints: {versions} in {manifests}"
            ),
            nodes: nodes.into_iter().collect(),
            edges: edge_indexes,
        });
    }
}

/// One line of a manifest asking for a package.
struct ScopeDeclaration {
    edge: usize,
    package: NodeId,
    scope: String,
}

pub(crate) fn add_mixed_dependency_scope_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    let nodes_by_id = node_index(graph);
    // Keyed by the manifest that declares it, because a workspace where one
    // module needs a package directly and another only inherits it has
    // declared nothing twice. Terraform's root `go.mod` and its
    // `internal/legacy/go.mod` disagreed about 69 modules that way.
    let mut groups: BTreeMap<(NodeId, String), Vec<ScopeDeclaration>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn {
            continue;
        }
        let Some(scope) = edge
            .metadata
            .get("dependency_kind")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target).copied() else {
            continue;
        };
        let key = target
            .metadata
            .get("package_id")
            .cloned()
            .unwrap_or_else(|| format!("node:{}", target.id.0));
        groups
            .entry((edge.source, key))
            .or_default()
            .push(ScopeDeclaration {
                edge: index,
                package: edge.target,
                scope: scope.to_string(),
            });
    }

    for ((manifest, _), declarations) in groups {
        // An optional declaration says "install this to use that feature",
        // which every dev dependency for the same package agrees with:
        // monolog tests eight of its optional handlers.
        let scopes: BTreeSet<_> = declarations
            .iter()
            .map(|declaration| declaration.scope.as_str())
            .filter(|scope| *scope != "optional")
            .collect();
        if scopes.len() < 2 {
            continue;
        }
        let Some(package) = declarations
            .first()
            .and_then(|declaration| node_label(graph, declaration.package))
        else {
            continue;
        };
        let manifest_label = node_label(graph, manifest).unwrap_or("an unknown manifest");
        // A lockfile records what every project in a workspace resolved to,
        // so one package arriving as a dependency of one project and a
        // development dependency of another is the normal shape of the file
        // rather than a disagreement anybody wrote down.
        if is_dependency_lockfile(manifest_label) {
            continue;
        }

        let mut nodes = BTreeSet::from([manifest]);
        let mut edges = Vec::new();
        for declaration in &declarations {
            nodes.insert(declaration.package);
            edges.push(declaration.edge);
        }
        let scope_list = format_backtick_list(scopes.iter().copied(), 6);
        insights.push(Insight {
            kind: "mixed_dependency_scope".to_string(),
            severity: if manifest_is_the_projects_own(manifest_label) {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "`{manifest_label}` declares dependency `{package}` in multiple dependency scopes: {scope_list}"
            ),
            nodes: nodes.into_iter().collect(),
            edges,
        });
    }
}

/// Whether a file declares a module of its own by that name, as `mod
/// cli;` does for the `use cli::*` written under it.
fn declares_module(graph: &CodeGraph, file: &str, name: &str) -> bool {
    graph.nodes.iter().any(|node| {
        node.kind == NodeKind::Module
            && node.label == name
            && node.span.as_ref().is_some_and(|span| span.path == file)
    })
}

/// The DefinitelyTyped package that carries types for another: `lodash`
/// has `@types/lodash`, and a scoped `@vue/shared` has
/// `@types/vue__shared`.
fn types_package_name(package: &str) -> String {
    match package
        .strip_prefix('@')
        .and_then(|rest| rest.split_once('/'))
    {
        Some((scope, name)) => format!("@types/{scope}__{name}"),
        None => format!("@types/{package}"),
    }
}

/// Whether a declared version names where to get the package rather than
/// which version of it. `catalog:` defers to a pnpm catalog, `workspace:*`
/// to a sibling package, and `file:`, `link:`, `git+...` and `npm:` to a
/// place to fetch it from.
pub(crate) fn names_a_dependency_source(version: &str) -> bool {
    let version = version.trim();
    version == "catalog:"
        || version.starts_with("catalog:")
        || version.starts_with("workspace:")
        || version.starts_with("link:")
        || version.starts_with("file:")
        || version.starts_with("path:")
        || version.starts_with("npm:")
        || version.starts_with("git+")
        || version.starts_with("github:")
        || version.starts_with("git:")
}

/// Whether a path is a resolved lockfile rather than a declaration. What a
/// lockfile says is the result of resolving the declarations, so it is
/// evidence about the resolver and not about what a person asked for.
pub(crate) fn is_dependency_lockfile(path: &str) -> bool {
    let file = path
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    matches!(
        file.as_str(),
        "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lockb"
            | "composer.lock"
            | "cargo.lock"
            | "gemfile.lock"
            | "poetry.lock"
            | "pdm.lock"
            | "uv.lock"
            | "go.sum"
    ) || file.ends_with(".lock")
}

/// Whether a path is a build tool's own configuration — `eslint.config.js`,
/// `vite.config.ts`, `.eslintrc.js`, `babel.config.cjs`. Such a file
/// exists to configure a development tool, so importing one is not
/// shipping it.
pub(crate) fn is_tool_configuration_source_path(path: &str) -> bool {
    let file = path
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let Some((stem, extension)) = file.rsplit_once('.') else {
        return false;
    };
    if !matches!(extension, "js" | "cjs" | "mjs" | "ts" | "cts" | "mts") {
        return false;
    }
    stem.ends_with(".config") || stem.starts_with('.') && stem.ends_with("rc")
}

/// A repository's own build, release and benchmark tooling: `scripts/build.js`,
/// `gulpfile.js`, `docs/scripts/utils.js`, `__benchmarks__/effect.bench.ts`.
/// It runs on a developer's machine or in CI rather than shipping, so a dev
/// dependency is exactly where what it imports belongs.
pub(crate) fn is_repository_tooling_source_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    // `gulpfile.js` is a file in most projects and a directory in some:
    // django-oscar keeps `gulpfile.js/index.js` and its subtasks there.
    if normalized
        .split('/')
        .any(|segment| segment.starts_with("gulpfile.") || segment.starts_with("gruntfile."))
    {
        return true;
    }
    normalized.split('/').any(|segment| {
        matches!(
            segment,
            "scripts"
                | "tools"
                | "bench"
                | "benchmarks"
                | "__benchmarks__"
                // gson keeps its benchmarks in `metrics/`, and a project's
                // CMake probes sit beside its build rather than in it.
                | "metrics"
                | "cmake"
                | "doc"
                | "docs"
        )
    })
}

/// The go.mod whose module a file belongs to: the nearest one above it.
/// Nothing else answers for that file, because a Go build reads exactly
/// one manifest.
fn go_module_manifest<'a>(manifests: &[&'a str], path: &str) -> Option<&'a str> {
    manifests
        .iter()
        .filter(|manifest| {
            let directory = manifest.strip_suffix("go.mod").unwrap_or_default();
            directory.is_empty() || path.starts_with(directory)
        })
        .max_by_key(|manifest| manifest.len())
        .copied()
}

/// What the packages in a repository ship, read from the `files` field
/// each manifest states. openzeppelin publishes `/contracts/**/*.sol` and
/// keeps its hardhat plugins and its formal-verification runner outside
/// that: those directories are tooling, and a dev dependency is exactly
/// what they should import.
#[derive(Default)]
pub(crate) struct PublishedPaths {
    /// Every package manifest, by the directory it governs, and the paths
    /// it publishes when it says. The nearest manifest is the one that
    /// speaks for a file, so the others must not answer for it.
    packages: Vec<(String, Option<PublishedGlobs>)>,
}

#[derive(Default)]
struct PublishedGlobs {
    include: Vec<String>,
    exclude: Vec<String>,
}

impl PublishedGlobs {
    fn covers(&self, path: &str) -> bool {
        let inside = |prefix: &String| {
            prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}/"))
        };
        self.include.iter().any(inside) && !self.exclude.iter().any(inside)
    }
}

impl PublishedPaths {
    /// Whether the package that owns this path says the path does not
    /// ship. A package that says nothing, or that publishes only a build
    /// product the scan never held -- vue's `files: ["dist"]` over sources
    /// in `src/` -- answers no: it knows nothing about this file.
    fn excludes(&self, path: &str) -> bool {
        self.packages
            .iter()
            .filter(|(directory, _)| {
                directory.is_empty() || path.starts_with(&format!("{directory}/"))
            })
            .max_by_key(|(directory, _)| directory.len())
            .and_then(|(_, globs)| globs.as_ref())
            .is_some_and(|globs| !globs.covers(path))
    }
}

/// The manifests' `files` globs, anchored on the directory each manifest
/// sits in. A glob's literal head is all that is read: `**/*.sol` under
/// `contracts/` publishes everything there, whatever the extension says.
fn published_paths(graph: &CodeGraph) -> PublishedPaths {
    let path_index = node_path_index(graph);
    let source_paths: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File && node.metadata.contains_key("language"))
        .map(|node| {
            path_index
                .get(&node.id)
                .map(String::as_str)
                .unwrap_or(node.label.as_str())
        })
        .collect();

    let mut packages = Vec::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::File || !node.label.ends_with("package.json") {
            continue;
        }
        let base = node
            .label
            .rsplit_once('/')
            .map(|(directory, _)| directory.to_string())
            .unwrap_or_default();
        let globs = node
            .metadata
            .get("published_paths")
            .map(|globs| published_globs(globs, &base))
            // A package that publishes a build product the scan never held
            // says nothing about which of its sources ship.
            .filter(|globs| source_paths.iter().any(|path| globs.covers(path)));
        packages.push((base, globs));
    }
    PublishedPaths { packages }
}

fn published_globs(globs: &str, base: &str) -> PublishedGlobs {
    let mut published = PublishedGlobs::default();
    for glob in globs.lines() {
        let (negated, glob) = match glob.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, glob),
        };
        let head = glob
            .split(['*', '?', '[', '{'])
            .next()
            .unwrap_or_default()
            .trim_start_matches("./")
            .trim_start_matches('/')
            .trim_end_matches('/');
        let prefix = match (base.is_empty(), head.is_empty()) {
            (true, _) => head.to_string(),
            (false, true) => base.to_string(),
            (false, false) => format!("{base}/{head}"),
        };
        if negated {
            published.exclude.push(prefix);
        } else {
            published.include.push(prefix);
        }
    }
    published
}

pub(crate) fn add_non_runtime_dependency_import_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let nodes_by_id = node_index(graph);
    let declarations = dependency_declarations_by_package(graph);
    if declarations.is_empty() {
        return;
    }
    let declared_ecosystems = declared_ecosystems_from_package_ids(declarations.keys());

    let path_index = node_path_index(graph);
    let published = published_paths(graph);
    let go_manifests: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File && node.label.ends_with("go.mod"))
        .map(|node| node.label.as_str())
        .collect();
    let mut reported = BTreeSet::new();
    let mut grouped: BTreeMap<String, NonRuntimeImport> = BTreeMap::new();
    for (import_edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::Imports {
            continue;
        }
        let Some(source) = nodes_by_id.get(&edge.source).copied() else {
            continue;
        };
        if is_dependency_manifest_source_path(&source.label) {
            continue;
        }
        if node_path_matches(source, &path_index, "test")
            || path_index
                .get(&source.id)
                .is_some_and(|path| is_test_like_source_path(path))
            || is_test_like_source_path(&source.label)
        {
            continue;
        }
        // `eslint.config.js` importing `eslint`, `vite.config.js`
        // importing `vite`: a build tool's own configuration is not the
        // code that ships, and 23 of Vue's 74 findings were that.
        let source_path = path_index
            .get(&source.id)
            .map(String::as_str)
            .unwrap_or(source.label.as_str());
        if is_tool_configuration_source_path(source_path)
            || is_repository_tooling_source_path(source_path)
            || is_vendored_source_path(source_path)
        {
            continue;
        }
        // The manifest says what ships. Code outside it cannot turn a dev
        // dependency into a runtime one, whatever the directory is called.
        if published.excludes(source_path) {
            continue;
        }
        let Some(import_node) = nodes_by_id.get(&edge.target).copied() else {
            continue;
        };
        if import_node
            .metadata
            .get("import_scope")
            .is_some_and(|scope| scope == "local" || scope == "workspace")
        {
            continue;
        }
        // `import type { Program } from '@babel/types'` is erased before
        // anything runs, so it cannot make a dev dependency a runtime one.
        // Vue writes 651 such imports, and 33 of its findings were them.
        // Python writes the same thing as `if TYPE_CHECKING:`.
        if import_node.label.trim_start().starts_with("import type ")
            || import_node
                .metadata
                .get("type_only")
                .is_some_and(|value| value == "true")
            // `try: import cryptography / except ImportError:` states that
            // the program runs without the package: flask and requests each
            // write one, and that is what an optional dependency is.
            || import_node
                .metadata
                .get("optional")
                .is_some_and(|value| value == "true")
        {
            continue;
        }
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        let imports = import_package_candidates(language, &import_node.label, &declared_ecosystems);
        // A Go import resolves to the most specific module that provides
        // it: terraform imports `cloud.google.com/go/storage`, which its
        // go.mod declares, and reading it as the `cloud.google.com/go`
        // beside it answers for the wrong requirement.
        let governing = go_module_manifest(&go_manifests, source_path);
        let Some((package_id, package_declarations)) = imports.iter().find_map(|import| {
            declarations
                .iter()
                .filter(|(package_id, _)| import_matches_package_id(package_id, import))
                // A Go build reads one go.mod: terraform's GCS backend is
                // its own module, requiring `cloud.google.com/go/storage`
                // outright, and the root manifest beside it -- which marks
                // the same module indirect -- says nothing about that file.
                .filter(|(_, package_declarations)| {
                    let Some(governing) = governing else {
                        return true;
                    };
                    package_declarations
                        .iter()
                        .any(|declaration| node_label(graph, declaration.source) == Some(governing))
                })
                .max_by_key(|(package_id, _)| package_id.len())
        }) else {
            continue;
        };
        let scopes: BTreeSet<_> = package_declarations
            .iter()
            .map(|declaration| declaration.kind.as_str())
            .collect();
        // An optional dependency imported from the code that needs it is
        // the pattern, not a mistake: composer's `suggest` and a Python
        // extra both say "install this to use that handler".
        if scopes.contains("runtime") || scopes.contains("optional") {
            continue;
        }
        if !reported.insert((edge.source, package_id.clone())) {
            continue;
        }

        let source_label = node_label(graph, edge.source).unwrap_or("unknown");
        let package = package_declarations
            .first()
            .and_then(|declaration| node_label(graph, declaration.target))
            .unwrap_or(package_id.as_str());
        let group = grouped
            .entry(package.to_string())
            .or_insert_with(|| NonRuntimeImport {
                scopes: BTreeSet::new(),
                sources: BTreeSet::new(),
                nodes: BTreeSet::new(),
                edges: Vec::new(),
            });
        group
            .scopes
            .extend(scopes.iter().map(|scope| scope.to_string()));
        group.sources.insert(source_label.to_string());
        group.nodes.insert(edge.source);
        group.nodes.insert(edge.target);
        group.edges.push(import_edge_index);
        for declaration in package_declarations {
            group.nodes.insert(declaration.source);
            group.nodes.insert(declaration.target);
            group.edges.push(declaration.edge_index);
        }
    }

    // One finding per package, as with the undeclared imports: vue reports
    // six files importing `vitest` and five importing `picocolors`, which
    // is two facts rather than eleven.
    for (package, group) in grouped {
        let scope_list = format_backtick_list(group.scopes.iter().map(String::as_str), 6);
        let sources = format_backtick_list(group.sources.iter().map(String::as_str), 3);
        let verb = if group.sources.len() == 1 {
            "imports"
        } else {
            "import"
        };
        let mut edges = group.edges;
        edges.sort_unstable();
        edges.dedup();
        insights.push(Insight {
            kind: "non_runtime_dependency_import".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{sources} {verb} `{package}` from production-like code, but the package is declared only as {scope_list}"
            ),
            nodes: group.nodes.into_iter().collect(),
            edges,
        });
    }
}

/// Where one package declared for development only is imported from.
struct NonRuntimeImport {
    scopes: BTreeSet<String>,
    sources: BTreeSet<String>,
    nodes: BTreeSet<NodeId>,
    edges: Vec<usize>,
}

pub(crate) fn add_test_only_runtime_dependency_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let declarations = dependency_declarations_by_package(graph);
    if declarations.is_empty() {
        return;
    }
    let usages = dependency_import_usages_by_package(graph);
    if usages.is_empty() {
        return;
    }

    for (package_id, package_declarations) in declarations {
        let runtime_declarations = package_declarations
            .iter()
            .filter(|declaration| declaration.kind == "runtime")
            .collect::<Vec<_>>();
        if runtime_declarations.is_empty() {
            continue;
        }
        let Some(package_usages) = usages.get(&package_id).filter(|usages| !usages.is_empty())
        else {
            continue;
        };
        if package_usages.iter().any(|usage| !usage.test_like) {
            continue;
        }

        let mut nodes = BTreeSet::new();
        let mut edges = Vec::new();
        for declaration in runtime_declarations {
            nodes.insert(declaration.source);
            nodes.insert(declaration.target);
            edges.push(declaration.edge_index);
        }
        for usage in package_usages {
            nodes.insert(usage.source);
            nodes.insert(usage.target);
            edges.push(usage.edge_index);
        }
        let package = package_declarations
            .first()
            .and_then(|declaration| node_label(graph, declaration.target))
            .unwrap_or(package_id.as_str());
        insights.push(Insight {
            kind: "test_only_runtime_dependency".to_string(),
            severity: InsightSeverity::Info,
            message: format!(
                "Dependency `{package}` is declared as runtime but is only imported from test-like sources"
            ),
            nodes: nodes.into_iter().collect(),
            edges,
        });
    }
}

#[derive(Debug)]
pub(crate) struct DependencyDeclaration {
    pub(crate) edge_index: usize,
    pub(crate) source: NodeId,
    pub(crate) target: NodeId,
    pub(crate) kind: String,
}

pub(crate) fn dependency_declarations_by_package(
    graph: &CodeGraph,
) -> BTreeMap<String, Vec<DependencyDeclaration>> {
    let nodes_by_id = node_index(graph);
    let mut declarations: BTreeMap<String, Vec<DependencyDeclaration>> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::DependsOn {
            continue;
        }
        let Some(kind) = edge
            .metadata
            .get("dependency_kind")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target).copied() else {
            continue;
        };
        let Some(package_id) = target.metadata.get("package_id") else {
            continue;
        };
        declarations
            .entry(package_id.clone())
            .or_default()
            .push(DependencyDeclaration {
                edge_index,
                source: edge.source,
                target: edge.target,
                kind: kind.to_string(),
            });
    }
    declarations
}

#[derive(Debug)]
pub(crate) struct DependencyImportUsage {
    pub(crate) edge_index: usize,
    pub(crate) source: NodeId,
    pub(crate) target: NodeId,
    pub(crate) test_like: bool,
}

pub(crate) fn dependency_import_usages_by_package(
    graph: &CodeGraph,
) -> BTreeMap<String, Vec<DependencyImportUsage>> {
    let nodes_by_id = node_index(graph);
    let path_index = node_path_index(graph);
    let declared = declared_package_ids(graph);
    let declared_ecosystems = declared_ecosystems_from_package_ids(declared.iter());
    let mut usages: BTreeMap<String, Vec<DependencyImportUsage>> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind != EdgeKind::Imports {
            continue;
        }
        let Some(source) = nodes_by_id.get(&edge.source).copied() else {
            continue;
        };
        if is_dependency_manifest_source_path(&source.label) {
            continue;
        }
        let Some(import_node) = nodes_by_id.get(&edge.target).copied() else {
            continue;
        };
        if import_node
            .metadata
            .get("import_scope")
            .is_some_and(|scope| scope == "local")
        {
            continue;
        }
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        let imports = import_package_candidates(language, &import_node.label, &declared_ecosystems);
        let Some(package_id) = imports.iter().find_map(|import| {
            declared
                .iter()
                .find(|package_id| import_matches_package_id(package_id, import))
                .cloned()
        }) else {
            continue;
        };
        let test_like = path_index
            .get(&source.id)
            .is_some_and(|path| is_test_like_source_path(path))
            || is_test_like_source_path(&source.label);
        usages
            .entry(package_id)
            .or_default()
            .push(DependencyImportUsage {
                edge_index,
                source: edge.source,
                target: edge.target,
                test_like,
            });
    }
    usages
}

/// Files that serve tests without being named as tests. Go says it in
/// the imports: a file that is not `_test.go` and still imports
/// `testing` compiles the test framework into the package, which is what
/// terraform's `cloud_mock.go` and `remote/testing.go` do to stand up
/// `httptest` servers.
fn test_scaffolding_paths(graph: &CodeGraph) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for edge in &graph.edges {
        if edge.kind != EdgeKind::Imports {
            continue;
        }
        let Some(import) = graph.nodes.iter().find(|node| node.id == edge.target) else {
            continue;
        };
        if import.metadata.get("language").map(String::as_str) != Some("go") {
            continue;
        }
        let target = import
            .metadata
            .get("import_target")
            .map(String::as_str)
            .unwrap_or(import.label.as_str())
            .trim_matches(['"', ' ']);
        if !matches!(target, "testing" | "testing/quick" | "net/http/httptest") {
            continue;
        }
        if let Some(source) = graph.nodes.iter().find(|node| node.id == edge.source)
            && source.kind == NodeKind::File
        {
            paths.insert(source.label.clone());
        }
    }
    paths
}

pub(crate) fn add_duplicate_framework_route_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    let scaffolding = test_scaffolding_paths(graph);
    let mut groups: BTreeMap<(String, String), Vec<NodeId>> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "framework_route")
        {
            continue;
        }
        // A duplicate route is a conflict only within one application, and
        // the graph does not model the application object. Tests build one
        // per case — flask declares `GET /` eleven times across its suite
        // and repeatedly inside a single file — so a route declared there
        // says nothing about the routing table the program serves. 22 of
        // flask's 25 duplicate groups lived entirely in tests.
        if node.span.as_ref().is_some_and(|span| {
            is_test_like_source_path(&span.path) || scaffolding.contains(&span.path)
        }) {
            continue;
        }
        let Some(path) = node
            .metadata
            .get("path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let method = node
            .metadata
            .get("method")
            .map(|method| method.trim())
            .filter(|method| !method.is_empty())
            .unwrap_or("ROUTE")
            .to_ascii_uppercase();
        // Django mounts each application's URLconf under a prefix of its
        // own, so `path("")` in twenty apps is twenty different URLs:
        // django-oscar declares `/` twenty times and none of them collides.
        // Within one URLconf it still would, so that is what is compared.
        let scope = if node.metadata.get("framework").map(String::as_str) == Some("django") {
            node.span
                .as_ref()
                .map(|span| span.path.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        groups
            .entry((method, format!("{scope}\u{1f}{path}")))
            .or_default()
            .push(node.id);
    }

    for ((method, scoped_path), nodes) in groups {
        if nodes.len() < 2 {
            continue;
        }
        let path = scoped_path
            .split_once('\u{1f}')
            .map(|(_, path)| path.to_string())
            .unwrap_or(scoped_path);

        let handlers = nodes
            .iter()
            .filter_map(|id| graph.nodes.iter().find(|node| node.id == *id))
            .filter_map(|node| node.metadata.get("handler").map(String::as_str))
            .collect::<BTreeSet<_>>();
        let handler_text = if handlers.is_empty() {
            "multiple handlers".to_string()
        } else {
            handlers
                .iter()
                .take(5)
                .map(|handler| format!("`{handler}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let edge_indexes = nodes
            .iter()
            .flat_map(|node| outgoing_edge_indexes(graph, *node, EdgeKind::References))
            .collect();
        // Which files declare it is what tells a conflict from two
        // programs: terraform's three findings are three separate mock
        // servers, one per package, and nothing in the count says so.
        let files = format_backtick_list(
            nodes
                .iter()
                .filter_map(|id| graph.nodes.iter().find(|node| node.id == *id))
                .filter_map(|node| node.span.as_ref().map(|span| span.path.as_str()))
                .collect::<BTreeSet<_>>()
                .into_iter(),
            3,
        );

        insights.push(Insight {
            kind: "duplicate_framework_route".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Route `{method} {path}` is declared {} times in {files} ({handler_text})",
                nodes.len()
            ),
            nodes,
            edges: edge_indexes,
        });
    }
}

pub(crate) fn add_unresolved_framework_route_handler_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for node in &graph.nodes {
        if node.kind != NodeKind::Entrypoint
            || node
                .metadata
                .get("item_kind")
                .is_none_or(|kind| kind != "framework_route")
        {
            continue;
        }
        let Some(handler) = node
            .metadata
            .get("handler")
            .map(|handler| handler.trim())
            .filter(|handler| !handler.is_empty())
        else {
            continue;
        };

        let resolved = graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::References
                && edge
                    .metadata
                    .get("resolution")
                    .is_some_and(|resolution| resolution == "framework_route_handler")
        });
        if resolved {
            continue;
        }

        let method = node
            .metadata
            .get("method")
            .map(|method| method.trim())
            .filter(|method| !method.is_empty())
            .unwrap_or("ROUTE");
        let path = node
            .metadata
            .get("path")
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
            .unwrap_or(&node.label);
        let framework = node
            .metadata
            .get("framework")
            .map(|framework| framework.trim())
            .filter(|framework| !framework.is_empty())
            .unwrap_or("framework");
        let mut edges = incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint);
        edges.extend(outgoing_edge_indexes(graph, node.id, EdgeKind::References));
        edges.sort_unstable();
        edges.dedup();

        // A framework's own tests declare routes to exercise the router:
        // every one of gin's 15 and eleven of express's live in a test file,
        // where the handler is a local closure rather than a definition.
        // The duplicate-route rule already reads a route there as a fixture.
        let declared_by_the_project = node
            .span
            .as_ref()
            .is_none_or(|span| manifest_is_the_projects_own(&span.path));
        insights.push(Insight {
            kind: "unresolved_framework_route_handler".to_string(),
            severity: if declared_by_the_project {
                InsightSeverity::Warning
            } else {
                InsightSeverity::Info
            },
            message: format!(
                "{framework} route `{method} {path}` references handler `{handler}` but no matching function was found"
            ),
            nodes: vec![node.id],
            edges,
        });
    }
}

pub(crate) fn add_custom_rule_violation_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    for node in &graph.nodes {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "custom_rule_violation")
        {
            continue;
        }

        let rule_kind = node
            .metadata
            .get("rule_kind")
            .map(String::as_str)
            .unwrap_or("violation");
        let message = node
            .metadata
            .get("message")
            .cloned()
            .unwrap_or_else(|| node.label.clone());
        let severity = node
            .metadata
            .get("severity")
            .map(|value| insight_severity_from_str(value))
            .unwrap_or(InsightSeverity::Warning);
        let mut edges = outgoing_edge_indexes(graph, node.id, EdgeKind::References);
        if let Some(edge_index) = node
            .metadata
            .get("violated_edge_index")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| *index < graph.edges.len())
        {
            edges.push(edge_index);
            edges.sort_unstable();
            edges.dedup();
        }

        insights.push(Insight {
            kind: format!("custom_rule_{rule_kind}"),
            severity,
            message,
            nodes: vec![node.id],
            edges,
        });
    }
}

pub(crate) fn insight_severity_from_str(value: &str) -> InsightSeverity {
    // Metadata-sourced severities stay lenient: unknown values read as
    // warnings instead of failing the whole report.
    value.parse().unwrap_or(InsightSeverity::Warning)
}

/// Whether the whole component is one method written several times --
/// Kotlin's `indexOf` has four signatures and okio's `Buffer` implements
/// them all. A call names the method rather than one signature, so the
/// graph links it to every overload, and the overloads then appear to
/// call each other. "`indexOf` -> `indexOf` -> `indexOf`" is not a cycle
/// a reader can act on: it is one method.
/// Whether every member of a cycle belongs to the same type. A class whose
/// methods call each other is one unit however many files it is written
/// across - C# splits `PolicyBuilder` over `PolicyBuilder.cs` and
/// `PolicyBuilder.OrSyntax.cs` - so the walk between them is not coupling
/// between components.
fn component_is_one_type(nodes_by_id: &BTreeMap<NodeId, &Node>, component: &[NodeId]) -> bool {
    let mut owner: Option<&str> = None;
    for id in component {
        let Some(node) = nodes_by_id.get(id) else {
            return false;
        };
        let Some(node_owner) = node.metadata.get("owner_type") else {
            return false;
        };
        if *owner.get_or_insert(node_owner.as_str()) != node_owner {
            return false;
        }
    }
    owner.is_some()
}

fn component_is_one_method(nodes_by_id: &BTreeMap<NodeId, &Node>, component: &[NodeId]) -> bool {
    let mut label: Option<&str> = None;
    let mut owner: Option<&str> = None;
    for id in component {
        let Some(node) = nodes_by_id.get(id) else {
            return false;
        };
        let Some(node_owner) = node.metadata.get("owner_type") else {
            return false;
        };
        if *label.get_or_insert(node.label.as_str()) != node.label {
            return false;
        }
        if *owner.get_or_insert(node_owner.as_str()) != node_owner {
            return false;
        }
    }
    label.is_some()
}

pub(crate) fn add_dependency_cycle_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
    const MAX_CYCLE_INSIGHTS: usize = 50;

    let nodes_by_id = nodes_by_id_index(graph);

    let mut nodes = BTreeSet::new();
    let mut adjacency: BTreeMap<NodeId, Vec<(NodeId, usize)>> = BTreeMap::new();
    let mut reverse: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for (index, edge) in graph.edges.iter().enumerate() {
        // Two documents pointing at each other is how prose is written:
        // okio's recipes page and its java.io page each link to the other,
        // which is navigation rather than a dependency either one has.
        if !is_cycle_edge(&edge.kind)
            || edge
                .metadata
                .get("source")
                .is_some_and(|source| source == "markdown")
            // An import Python erases at run time - `if TYPE_CHECKING:` -
            // is not a dependency that can close a cycle when the program
            // runs: requests writes `_types.py` that way.
            || edge
                .metadata
                .get("type_only")
                .is_some_and(|value| value == "true")
            // A Dart `part` and the file it belongs to are one library
            // written across two files, and each names the other by
            // definition.
            || [edge.source, edge.target].iter().any(|id| {
                nodes_by_id
                    .get(id)
                    .and_then(|node| node.metadata.get("import_form"))
                    .is_some_and(|form| form == "part")
            })
        {
            continue;
        }
        nodes.insert(edge.source);
        nodes.insert(edge.target);
        adjacency
            .entry(edge.source)
            .or_default()
            .push((edge.target, index));
        reverse.entry(edge.target).or_default().push(edge.source);
    }

    let mut visited = BTreeSet::new();
    let mut order = Vec::new();
    for node in &nodes {
        if visited.contains(node) {
            continue;
        }
        fill_finish_order(*node, &adjacency, &mut visited, &mut order);
    }

    let mut assigned = BTreeSet::new();
    for node in order.into_iter().rev() {
        if assigned.contains(&node) {
            continue;
        }
        let component = reverse_component(node, &reverse, &mut assigned);
        if component.len() < 2 {
            continue;
        }
        // Collect edges only for real cycles: singleton components are the
        // overwhelming majority, and a full edge scan per component made
        // cycle detection quadratic (audit F11).
        let component_nodes: BTreeSet<_> = component.iter().copied().collect();
        let component_edges: Vec<_> = graph
            .edges
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| {
                if is_cycle_edge(&edge.kind)
                    && component_nodes.contains(&edge.source)
                    && component_nodes.contains(&edge.target)
                {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();

        if component_is_one_method(&nodes_by_id, &component) {
            continue;
        }

        let labels = component
            .iter()
            .filter_map(|id| node_label(graph, *id))
            .take(5)
            .map(|label| format!("`{label}`"))
            .collect::<Vec<_>>()
            .join(" -> ");
        let suffix = if component.len() > 5 { " -> ..." } else { "" };
        // A cycle inside one file is mutual recursion — a parser calling
        // itself down a tree — and every one of terraform's 50 cycles and
        // dune's 50 is of that kind. A cycle that crosses files is the
        // coupling the finding exists to surface, so only that one is a
        // warning.
        let placed: Vec<&str> = component
            .iter()
            .filter_map(|id| graph.nodes.iter().find(|node| node.id == *id))
            .filter_map(|node| node.span.as_ref().map(|span| span.path.as_str()))
            .collect();
        let files: BTreeSet<&str> = placed.iter().copied().collect();
        // Unknown is not the same as confined: a cycle is only local when
        // every node in it is known to sit in the one file.
        // A class is one unit however many files it is written across: C#
        // splits `PolicyBuilder` over `PolicyBuilder.cs` and
        // `PolicyBuilder.OrSyntax.cs`, and its methods calling each other is
        // not coupling between parts of the program.
        let one_type = component_is_one_type(&nodes_by_id, &component);
        let crosses_files = !(one_type || (files.len() == 1 && placed.len() == component.len()));
        // A cycle among vendored files is upstream's shape: redis carries
        // jemalloc's and lua's, and dune carries re's. A cycle among test
        // files is the harness's: kong's `spec/helpers/perf.lua` and the
        // `spec/helpers/perf/git.lua` beside it require each other, and
        // that is the suite's shape rather than the program's.
        // A ring needs every one of its links, so a cycle one of whose
        // imports is written under `#[cfg(test)]` exists only in the test
        // build: ripgrep's searcher imports its own `testutil` there.
        let closed_by_a_test = component.iter().any(|id| {
            nodes_by_id.get(id).is_some_and(|node| {
                node.metadata.get("test_context").map(String::as_str) == Some("true")
            })
        });
        let outside_the_program = closed_by_a_test
            || (!files.is_empty()
                && files
                    .iter()
                    .all(|file| is_vendored_source_path(file) || is_test_like_source_path(file)));
        let severity = if crosses_files && !outside_the_program {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Info
        };
        let scope = if crosses_files {
            "across files"
        } else if one_type && files.len() > 1 {
            "inside one type"
        } else {
            "inside one file"
        };
        insights.push(Insight {
            kind: "dependency_cycle".to_string(),
            severity,
            message: format!("Directed dependency cycle {scope} involving {labels}{suffix}"),
            nodes: component,
            edges: component_edges,
        });

        if insights
            .iter()
            .filter(|insight| insight.kind == "dependency_cycle")
            .count()
            >= MAX_CYCLE_INSIGHTS
        {
            return;
        }
    }
}

pub(crate) fn fill_finish_order(
    start: NodeId,
    adjacency: &BTreeMap<NodeId, Vec<(NodeId, usize)>>,
    visited: &mut BTreeSet<NodeId>,
    order: &mut Vec<NodeId>,
) {
    let mut stack = vec![(start, false)];
    while let Some((node, finished)) = stack.pop() {
        if finished {
            order.push(node);
            continue;
        }
        if !visited.insert(node) {
            continue;
        }
        stack.push((node, true));
        if let Some(edges) = adjacency.get(&node) {
            for (target, _) in edges.iter().rev() {
                if !visited.contains(target) {
                    stack.push((*target, false));
                }
            }
        }
    }
}

pub(crate) fn reverse_component(
    start: NodeId,
    reverse: &BTreeMap<NodeId, Vec<NodeId>>,
    assigned: &mut BTreeSet<NodeId>,
) -> Vec<NodeId> {
    let mut component = Vec::new();
    let mut stack = vec![start];
    while let Some(node) = stack.pop() {
        if !assigned.insert(node) {
            continue;
        }
        component.push(node);
        if let Some(sources) = reverse.get(&node) {
            for source in sources.iter().rev() {
                if !assigned.contains(source) {
                    stack.push(*source);
                }
            }
        }
    }
    component.sort();
    component
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportPackage {
    pub(crate) ecosystem: String,
    pub(crate) package: String,
}

pub(crate) fn declared_package_ids(graph: &CodeGraph) -> BTreeSet<String> {
    graph
        .nodes
        .iter()
        .filter_map(|node| {
            if node
                .metadata
                .get("item_kind")
                .is_some_and(|value| value == "dependency")
            {
                node.metadata.get("package_id").cloned()
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn import_packages(graph: &CodeGraph) -> Vec<(usize, ImportPackage)> {
    let nodes_by_id = node_index(graph);
    let declared = declared_package_ids(graph);
    let declared_ecosystems = declared_ecosystems_from_package_ids(declared.iter());
    graph
        .edges
        .iter()
        .enumerate()
        .flat_map(|(index, edge)| {
            if edge.kind != EdgeKind::Imports {
                return Vec::new();
            }
            if graph
                .nodes
                .iter()
                .find(|node| node.id == edge.source)
                .is_some_and(|node| is_dependency_manifest_source_path(&node.label))
            {
                return Vec::new();
            }
            let Some(import_node) = nodes_by_id.get(&edge.target).copied() else {
                return Vec::new();
            };
            let Some(language) = import_node.metadata.get("language") else {
                return Vec::new();
            };
            import_package_candidates(language, &import_node.label, &declared_ecosystems)
                .into_iter()
                .map(move |import| (index, import))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn dependency_usage_packages(graph: &CodeGraph) -> Vec<(usize, ImportPackage)> {
    let mut packages = import_packages(graph);
    for (index, node) in graph.nodes.iter().enumerate() {
        if node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "call")
            || node
                .metadata
                .get("language")
                .is_none_or(|language| language != "rust")
        {
            continue;
        }
        if let Some(package) = rust_path_package(&node.label) {
            packages.push((
                index,
                ImportPackage {
                    ecosystem: "cargo".to_string(),
                    package,
                },
            ));
        }
    }
    packages
}

pub(crate) fn import_package_candidate(language: &str, label: &str) -> Option<ImportPackage> {
    match language {
        "rust" => rust_import_package(label).map(|package| ImportPackage {
            ecosystem: "cargo".to_string(),
            package,
        }),
        "python" => python_import_package(label).map(|package| ImportPackage {
            ecosystem: "python".to_string(),
            package,
        }),
        "javascript" | "typescript" | "tsx" => {
            js_import_package(label).map(|package| ImportPackage {
                ecosystem: "npm".to_string(),
                package,
            })
        }
        "go" => go_import_package(label).map(|package| ImportPackage {
            ecosystem: "go".to_string(),
            package,
        }),
        "dart" => dart_import_package(label).map(|package| ImportPackage {
            ecosystem: "dart".to_string(),
            package,
        }),
        "php" => php_import_packages(label)
            .into_iter()
            .next()
            .map(|package| ImportPackage {
                ecosystem: "composer".to_string(),
                package,
            }),
        _ => None,
    }
}

pub(crate) fn import_package_candidates(
    language: &str,
    label: &str,
    declared_ecosystems: &BTreeSet<String>,
) -> Vec<ImportPackage> {
    if matches!(language, "c" | "cpp") {
        let Some(package) = c_family_include_package(label) else {
            return Vec::new();
        };
        return ["vcpkg", "conan", "cmake"]
            .into_iter()
            .filter(|ecosystem| declared_ecosystems.contains(*ecosystem))
            .map(|ecosystem| ImportPackage {
                ecosystem: ecosystem.to_string(),
                package: package.clone(),
            })
            .collect();
    }

    if language == "php" {
        if !declared_ecosystems.contains("composer") {
            return Vec::new();
        }
        return php_import_packages(label)
            .into_iter()
            .map(|package| ImportPackage {
                ecosystem: "composer".to_string(),
                package,
            })
            .collect();
    }

    if language == "python" {
        let Some(package) = python_import_package(label) else {
            return Vec::new();
        };
        return python_distribution_name(&package)
            .into_iter()
            .map(ToString::to_string)
            .chain([package])
            .map(|package| ImportPackage {
                ecosystem: "python".to_string(),
                package,
            })
            .collect();
    }

    import_package_candidate(language, label)
        .into_iter()
        .collect()
}

/// The distribution a Python module comes from, where the two names differ:
/// `import yaml` installs PyYAML and `import dotenv` python-dotenv. Only
/// well-known pairs are listed; every other module shares its name with the
/// package that ships it.
fn python_distribution_name(module: &str) -> Option<&'static str> {
    Some(match module {
        "attr" => "attrs",
        "bs4" => "beautifulsoup4",
        "cv2" => "opencv-python",
        "dateutil" => "python-dateutil",
        "dotenv" => "python-dotenv",
        "elftools" => "pyelftools",
        "jwt" => "pyjwt",
        "openssl" => "pyopenssl",
        "pil" => "pillow",
        "serial" => "pyserial",
        "sklearn" => "scikit-learn",
        "yaml" => "pyyaml",
        _ => return None,
    })
}

pub(crate) fn import_matches_package_id(package_id: &str, import: &ImportPackage) -> bool {
    let Some((ecosystem, package)) = package_id.split_once(':') else {
        return false;
    };
    if ecosystem != import.ecosystem {
        return false;
    }

    match ecosystem {
        "go" => import.package == package || import.package.starts_with(&format!("{package}/")),
        "cargo" => {
            let canonical = import.package.to_ascii_lowercase();
            let hyphenated = canonical.replace('_', "-");
            let underscored = canonical.replace('-', "_");
            package == canonical || package == hyphenated || package == underscored
        }
        "python" => {
            let canonical = canonical_python_package_name(&import.package);
            package == canonical || python_distribution_carries_module(package, &canonical)
        }
        "npm" | "dart" => package == import.package.to_ascii_lowercase(),
        // A composer package's vendor is whoever publishes it, and the
        // namespace a class sits in says nothing about that: `Elastica\\`
        // comes from `ruflin/elastica` and `Gelf\\` from
        // `graylog2/gelf-php`. What the two do share is the name after the
        // slash.
        "composer" => {
            let imported = import.package.to_ascii_lowercase();
            package == imported || composer_names_the_same_library(package, &imported)
        }
        "vcpkg" | "conan" | "cmake" => package == import.package.to_ascii_lowercase(),
        _ => package == import.package,
    }
}

/// Whether a declared composer package is the library an import names,
/// going by the part after the vendor: `ruflin/elastica` ships `Elastica\`,
/// `graylog2/gelf-php` ships `Gelf\`, `aws/aws-sdk-php` ships `Aws\`.
fn composer_names_the_same_library(declared: &str, imported: &str) -> bool {
    let glue = |value: &str| {
        value
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
    };
    let library = declared
        .split_once('/')
        .map(|(_, name)| name)
        .unwrap_or(declared);
    let (Some(root), Some(declared_root)) =
        (imported.split('/').next().map(glue), Some(glue(library)))
    else {
        return false;
    };
    // `aws/aws-sdk-php` publishes `Aws\`: a short root still matches when
    // the declared name opens with it as a word of its own.
    if library.starts_with(&format!("{root}-")) {
        return true;
    }
    // Otherwise a short root — `db`, `io` — says too little on its own.
    if root.len() < 4 {
        return false;
    }
    declared_root == root || declared_root.starts_with(&root) || root.starts_with(&declared_root)
}

pub(crate) fn rust_import_package(label: &str) -> Option<String> {
    let value = label.trim().strip_prefix("use ")?;
    let first = value
        .trim()
        .trim_start_matches("::")
        .split([':', ';', ',', '{', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())?;
    if names_the_language_itself(first) {
        return None;
    }
    // `use FastMatchResult::*;` brings an enum's variants into scope; a
    // crate is named in lower case, and ripgrep's own enum read as a
    // dependency it never declared.
    if first.starts_with(char::is_uppercase) {
        return None;
    }
    Some(first.to_ascii_lowercase())
}

/// Whether a Rust path starts at something the compiler itself provides
/// rather than at a crate the project declares. `proc_macro` is handed to a
/// procedural-macro crate the way `std` is handed to every other, and
/// serde_derive's `use proc_macro::TokenStream` read as an undeclared
/// dependency.
fn names_the_language_itself(first: &str) -> bool {
    matches!(
        first,
        "std" | "core" | "alloc" | "proc_macro" | "test" | "crate" | "self" | "super"
    )
}

pub(crate) fn rust_path_package(label: &str) -> Option<String> {
    let first = label
        .trim()
        .trim_start_matches("::")
        .split("::")
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())?;
    if first.contains('.') || names_the_language_itself(first) {
        None
    } else {
        Some(first.to_ascii_lowercase())
    }
}

pub(crate) fn python_import_package(label: &str) -> Option<String> {
    let value = label.trim();
    let package = if let Some(rest) = value.strip_prefix("import ") {
        rest.split([',', ' ', '\n', '\t'])
            .find(|part| !part.is_empty())
            .and_then(|part| part.split('.').next())
    } else if let Some(rest) = value.strip_prefix("from ") {
        rest.split_whitespace()
            .next()
            .and_then(|part| part.split('.').next())
    } else {
        None
    }?;

    // `_typeshed` and its kind exist only for type checkers; nothing
    // installs them, so nothing declares them either. The test comes
    // before canonicalisation, which turns the underscore into a dash.
    if package.starts_with('_') {
        return None;
    }
    // A handful of standard modules are not written in lower case —
    // `cProfile` is one, and pytudes profiles two of its notebooks with it
    // — so the module is tested as written before it is canonicalised.
    if is_python_stdlib_package(package) {
        return None;
    }
    let package = canonical_python_package_name(package);
    if is_python_stdlib_package(&package) || package.is_empty() {
        None
    } else {
        Some(package)
    }
}

pub(crate) fn js_import_package(label: &str) -> Option<String> {
    let module = first_quoted_string(label)?;
    // `node:fs`, `bun:test`, `jsr:@std/assert`, `npm:chalk@5` and
    // `https://deno.land/x/...` all say where the module comes from. None
    // of them is a package that a manifest declares.
    if module.starts_with('.')
        || module.starts_with('/')
        || module
            .split('/')
            .next()
            .is_some_and(|head| head.contains(':'))
        || is_node_builtin_module(&module)
    {
        return None;
    }

    if module.starts_with('@') {
        let mut parts = module.split('/');
        let scope = parts.next()?;
        let name = parts.next()?;
        Some(format!("{scope}/{name}").to_ascii_lowercase())
    } else {
        module
            .split('/')
            .next()
            .filter(|part| !part.is_empty())
            .map(|package| package.to_ascii_lowercase())
    }
}

pub(crate) fn go_import_package(label: &str) -> Option<String> {
    for module in quoted_strings(label) {
        if module.starts_with('.') || module.starts_with('/') {
            continue;
        }
        let first = module.split('/').next().unwrap_or("");
        if first.contains('.') {
            return Some(module);
        }
    }
    None
}

pub(crate) fn dart_import_package(label: &str) -> Option<String> {
    let uri = first_quoted_string(label)?;
    if uri.starts_with('.')
        || uri.starts_with('/')
        || uri.starts_with("dart:")
        || uri.contains("://")
    {
        return None;
    }
    let rest = uri.strip_prefix("package:")?;
    rest.split('/')
        .next()
        .map(str::trim)
        .filter(|package| !package.is_empty())
        .map(|package| package.to_ascii_lowercase())
}

pub(crate) fn php_import_packages(label: &str) -> Vec<String> {
    let mut packages = Vec::new();
    for namespace in php_import_namespaces(label) {
        for package in php_namespace_package_candidates(&namespace) {
            if !packages.contains(&package) {
                packages.push(package);
            }
        }
    }
    packages
}

pub(crate) fn php_import_namespaces(label: &str) -> Vec<String> {
    let mut value = label.trim().trim_end_matches(';').trim();
    if let Some(rest) = value.strip_prefix("use ") {
        value = rest.trim();
    }
    value = value
        .strip_prefix("function ")
        .or_else(|| value.strip_prefix("const "))
        .unwrap_or(value)
        .trim();

    if let Some((prefix, rest)) = value.split_once('{') {
        let prefix = prefix.trim().trim_end_matches('\\');
        let Some((group, _)) = rest.split_once('}') else {
            return Vec::new();
        };
        return group
            .split(',')
            .filter_map(|part| {
                let clause = php_namespace_without_alias(part);
                if clause.is_empty() {
                    None
                } else if prefix.is_empty() {
                    Some(clause.to_string())
                } else {
                    Some(format!("{prefix}\\{clause}"))
                }
            })
            .collect();
    }

    let namespace = php_namespace_without_alias(value);
    if namespace.is_empty() {
        Vec::new()
    } else {
        vec![namespace.to_string()]
    }
}

pub(crate) fn php_namespace_without_alias(value: &str) -> &str {
    value
        .split_once(" as ")
        .map(|(namespace, _)| namespace)
        .unwrap_or(value)
        .trim()
        .trim_start_matches('\\')
}

pub(crate) fn php_namespace_package_candidates(namespace: &str) -> Vec<String> {
    let parts = namespace
        .split('\\')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 || is_php_non_composer_namespace_root(parts[0]) {
        return Vec::new();
    }

    let mut packages = Vec::new();
    match parts.as_slice() {
        ["Monolog", ..] => packages.push("monolog/monolog".to_string()),
        ["PHPUnit", ..] => packages.push("phpunit/phpunit".to_string()),
        ["GuzzleHttp", ..] => packages.push("guzzlehttp/guzzle".to_string()),
        ["Symfony", "Component", component, ..] => {
            packages.push(format!("symfony/{}", composer_package_part(component)));
        }
        ["Psr", component, rest @ ..] => {
            // `Psr\Log` is psr/log, but `Psr\Http\Message` is
            // psr/http-message rather than psr/http. Offer both, and let
            // the declared set decide which one the project actually has.
            packages.push(format!("psr/{}", composer_package_part(component)));
            if let Some(second) = rest.first() {
                packages.push(format!(
                    "psr/{}-{}",
                    composer_package_part(component),
                    composer_package_part(second)
                ));
            }
        }
        _ => {}
    }

    // A capital run is a word boundary in one package name and part of the
    // word in the next - `Doctrine\CouchDB` is doctrine/couchdb, `MongoDB`
    // is the vendor mongodb - so both spellings are offered and the
    // declared set decides.
    for vendor in composer_package_spellings(parts[0]) {
        if let Some(component) = parts.get(1) {
            for component in composer_package_spellings(component) {
                packages.push(format!("{vendor}/{component}"));
            }
        }
        packages.push(format!("{vendor}/{vendor}"));
    }
    packages.retain(|package| package.split('/').all(|part| !part.is_empty()));
    packages.dedup();
    packages
}

pub(crate) fn is_php_non_composer_namespace_root(root: &str) -> bool {
    matches!(
        root,
        "App"
            | "Tests"
            | "Test"
            | "Database"
            | "Config"
            | "DateTime"
            | "DateTimeImmutable"
            | "DateTimeInterface"
            | "DateInterval"
            | "DateTimeZone"
            | "Exception"
            | "RuntimeException"
            | "InvalidArgumentException"
            | "Throwable"
            | "Closure"
            | "ArrayObject"
            | "Iterator"
            | "IteratorAggregate"
            | "Traversable"
            | "Countable"
            | "JsonSerializable"
            | "PDO"
    )
}

/// The spellings a namespace segment can take in a package name: the
/// word-separated one, and the one that keeps a capital run glued to the
/// word before it.
pub(crate) fn composer_package_spellings(value: &str) -> Vec<String> {
    let separated = composer_package_part(value);
    let glued = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    let mut spellings = vec![separated];
    if !glued.is_empty() && !spellings.contains(&glued) {
        spellings.push(glued);
    }
    spellings
}

/// A namespace segment as composer writes it in a package name. A run of
/// capitals is one word - `CouchDB` is `couchdb`, the way doctrine publishes
/// it - while a single capital opens a new one: `DynamoDb` is `dynamo-db`.
pub(crate) fn composer_package_part(value: &str) -> String {
    let characters: Vec<char> = value.trim().chars().collect();
    let mut normalized = String::new();
    let mut previous_separator = false;
    for (index, character) in characters.iter().copied().enumerate() {
        if matches!(character, '_' | '-' | '.') {
            if !previous_separator && !normalized.is_empty() {
                normalized.push('-');
                previous_separator = true;
            }
            continue;
        }
        if character.is_ascii_uppercase() {
            // Inside a run of capitals the word has not ended; it ends where
            // the next lowercase letter starts a new one.
            let follows_capital = index
                .checked_sub(1)
                .and_then(|previous| characters.get(previous))
                .is_some_and(|previous| previous.is_ascii_uppercase());
            if !normalized.is_empty() && !previous_separator && !follows_capital {
                normalized.push('-');
            }
            normalized.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_separator = false;
        }
    }
    normalized.trim_matches('-').to_string()
}

pub(crate) fn c_family_include_package(label: &str) -> Option<String> {
    let header = include_header_name(label)?;
    let package = header
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches(".hpp")
        .trim_end_matches(".hh")
        .trim_end_matches(".hxx")
        .trim_end_matches(".h")
        .to_ascii_lowercase();
    if package.is_empty()
        || matches!(
            package.as_str(),
            "assert"
                | "complex"
                | "ctype"
                | "errno"
                | "float"
                | "inttypes"
                | "iso646"
                | "limits"
                | "locale"
                | "math"
                | "setjmp"
                | "signal"
                | "stdalign"
                | "stdarg"
                | "stdatomic"
                | "stdbool"
                | "stddef"
                | "stdint"
                | "stdio"
                | "stdlib"
                | "stdnoreturn"
                | "string"
                | "tgmath"
                | "threads"
                | "time"
                | "uchar"
                | "wchar"
                | "wctype"
                | "algorithm"
                | "array"
                | "atomic"
                | "bit"
                | "chrono"
                | "concepts"
                | "coroutine"
                | "deque"
                | "exception"
                | "filesystem"
                | "format"
                | "fstream"
                | "functional"
                | "future"
                | "initializer_list"
                | "iostream"
                | "istream"
                | "iterator"
                | "map"
                | "memory"
                | "mutex"
                | "optional"
                | "ostream"
                | "queue"
                | "ranges"
                | "regex"
                | "set"
                | "span"
                | "sstream"
                | "stdexcept"
                | "string_view"
                | "thread"
                | "tuple"
                | "type_traits"
                | "unordered_map"
                | "unordered_set"
                | "utility"
                | "variant"
                | "vector"
        )
    {
        None
    } else {
        Some(package)
    }
}

pub(crate) fn include_header_name(label: &str) -> Option<String> {
    let value = label.trim();
    if let Some(start) = value.find('<') {
        let rest = &value[start + 1..];
        let end = rest.find('>')?;
        return Some(rest[..end].trim().to_string());
    }
    quoted_strings(value).into_iter().next()
}

pub(crate) fn declared_ecosystems_from_package_ids<'a>(
    package_ids: impl IntoIterator<Item = &'a String>,
) -> BTreeSet<String> {
    package_ids
        .into_iter()
        .filter_map(|package_id| {
            package_id
                .split_once(':')
                .map(|(ecosystem, _)| ecosystem.to_string())
        })
        .collect()
}

/// Whether a declared Python distribution is the one that installs a
/// module: Django's ecosystem publishes `django-treebeard` for `treebeard`,
/// `sorl-thumbnail` for `sorl`, `django-phonenumber-field` for
/// `phonenumber_field`. The module's name is a run of the distribution's
/// own words, which is what these conventions have in common.
fn python_distribution_carries_module(declared: &str, module: &str) -> bool {
    if module.len() < 4 {
        return false;
    }
    let words: Vec<&str> = declared.split(['-', '_', '.']).collect();
    let wanted: Vec<&str> = module.split(['-', '_', '.']).collect();
    if wanted.is_empty() || words.len() <= wanted.len() {
        return false;
    }
    words.windows(wanted.len()).any(|window| window == wanted)
}

pub(crate) fn is_declared_package(
    declared: &BTreeSet<String>,
    ecosystem: &str,
    package: &str,
) -> bool {
    match ecosystem {
        "go" => declared.iter().any(|package_id| {
            package_id.strip_prefix("go:").is_some_and(|module| {
                package == module || package.starts_with(&format!("{module}/"))
            })
        }),
        "cargo" => {
            let canonical = package.to_ascii_lowercase();
            let hyphenated = canonical.replace('_', "-");
            let underscored = canonical.replace('-', "_");
            declared.contains(&format!("cargo:{canonical}"))
                || declared.contains(&format!("cargo:{hyphenated}"))
                || declared.contains(&format!("cargo:{underscored}"))
        }
        "python" => {
            let canonical = canonical_python_package_name(package);
            declared.contains(&format!("python:{canonical}"))
                || declared.iter().any(|package_id| {
                    package_id.strip_prefix("python:").is_some_and(|declared| {
                        python_distribution_carries_module(declared, &canonical)
                    })
                })
        }
        "npm" => declared.contains(&format!("npm:{}", package.to_ascii_lowercase())),
        "composer" => {
            let imported = package.to_ascii_lowercase();
            declared.contains(&format!("composer:{imported}"))
                || declared.iter().any(|package_id| {
                    package_id
                        .strip_prefix("composer:")
                        .is_some_and(|declared| {
                            composer_names_the_same_library(declared, &imported)
                        })
                })
        }
        "vcpkg" | "conan" | "cmake" => {
            declared.contains(&format!("{ecosystem}:{}", package.to_ascii_lowercase()))
        }
        _ => declared.contains(&format!("{ecosystem}:{package}")),
    }
}

pub(crate) fn canonical_python_package_name(name: &str) -> String {
    let mut normalized = String::new();
    let mut previous_separator = false;
    for character in name.trim().chars() {
        if matches!(character, '-' | '_' | '.') {
            if !previous_separator {
                normalized.push('-');
            }
            previous_separator = true;
        } else {
            normalized.extend(character.to_lowercase());
            previous_separator = false;
        }
    }
    normalized
}

pub(crate) fn first_quoted_string(value: &str) -> Option<String> {
    quoted_strings(value).into_iter().next()
}

pub(crate) fn quoted_strings(value: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut quote = None;
    let mut start = 0;

    for (index, character) in value.char_indices() {
        match quote {
            Some(current_quote) if character == current_quote => {
                strings.push(value[start..index].to_string());
                quote = None;
            }
            None if character == '"' || character == '\'' || character == '`' => {
                quote = Some(character);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    strings
}

/// Whether a specifier names a module Node ships with, which no manifest
/// declares. The list is `module.builtinModules` in full, subpaths
/// included: a partial list is worse than none, because every name
/// missing from it becomes a warning that the project forgot a
/// dependency. Modules only reachable as `node:test` or `node:sqlite` are
/// covered by the prefix itself.
pub(crate) fn is_node_builtin_module(module: &str) -> bool {
    matches!(
        module,
        "assert"
            | "assert/strict"
            | "async_hooks"
            | "buffer"
            | "child_process"
            | "cluster"
            | "console"
            | "constants"
            | "crypto"
            | "dgram"
            | "diagnostics_channel"
            | "dns"
            | "dns/promises"
            | "domain"
            | "events"
            | "fs"
            | "fs/promises"
            | "http"
            | "http2"
            | "https"
            | "inspector"
            | "inspector/promises"
            | "module"
            | "net"
            | "os"
            | "path"
            | "path/posix"
            | "path/win32"
            | "perf_hooks"
            | "process"
            | "punycode"
            | "querystring"
            | "readline"
            | "readline/promises"
            | "repl"
            | "stream"
            | "stream/consumers"
            | "stream/promises"
            | "stream/web"
            | "string_decoder"
            | "sys"
            | "timers"
            | "timers/promises"
            | "tls"
            | "trace_events"
            | "tty"
            | "url"
            | "util"
            | "util/types"
            | "v8"
            | "vm"
            | "wasi"
            | "worker_threads"
            | "zlib"
    )
}
