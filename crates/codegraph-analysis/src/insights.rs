//! Investigation insights: the public insight API, quality gate, and
//! every insight generator with severity calibration.

use codegraph_core::{COMPUTED_ENVIRONMENT_KEY, CodeGraph, EdgeKind, Node, NodeId, NodeKind};
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
    nodes: BTreeSet<NodeId>,
    edges: Vec<usize>,
}

pub(crate) fn add_unresolved_local_import_insights(graph: &CodeGraph, insights: &mut Vec<Insight>) {
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
        // so there is no file to go looking for.
        if target.contains('$') {
            continue;
        }
        let edges = incoming_edge_indexes(graph, node.id, EdgeKind::Imports);
        let source = edges
            .first()
            .and_then(|index| graph.edges.get(*index))
            .and_then(|edge| node_label(graph, edge.source))
            .unwrap_or("unknown");
        // Imports inside inline test modules or test-convention files are
        // fixture wiring, not production dead links, mirroring the
        // benchmark-oracle test exclusions (Phase 9 dogfooding).
        let production = !node
            .metadata
            .get("test_context")
            .is_some_and(|value| value == "true")
            && !is_test_like_source_path(source);

        let entry = missing
            .entry(missing_import_key(source, target))
            .or_insert_with(|| MissingImport {
                target: target.to_string(),
                sources: BTreeSet::new(),
                production_source: false,
                nodes: BTreeSet::new(),
                edges: Vec::new(),
            });
        entry.sources.insert(source.to_string());
        entry.production_source |= production;
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
            severity: if entry.production_source {
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
        if path_like_tokens(target).any(|token| {
            let token = token
                .trim_start_matches("./")
                .trim_end_matches('/')
                .trim_end_matches('\\');
            scanned_paths.contains(token) || is_unscanned_hidden_path(&scanned_paths, token)
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
        let mut nodes = vec![node.id];
        nodes.extend(compose_env_file_reader_ids(graph, node.id));
        nodes.sort();
        nodes.dedup();
        insights.push(Insight {
            kind: "unresolved_compose_env_file_path".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Compose service `{service}` references env_file `{env_file_path}` but the file was not found"
            ),
            nodes,
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::ReadsConfig),
        });
    }
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
        if resolved {
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
            severity: InsightSeverity::Warning,
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

pub(crate) fn add_unresolved_workflow_command_path_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
    item_kind: &str,
    resolution: &str,
    insight_kind: &str,
    label_prefix: &str,
) {
    let directories: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Directory)
        .map(|node| node.label.as_str())
        .collect();
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
        // `(cd ../deps && $(MAKE) distclean)` names a directory, and
        // `go run ./tools/protobuf-compile .` names a package directory.
        // Neither is a file, and neither is missing.
        if directories.contains(command_path) {
            continue;
        }

        let command = node
            .metadata
            .get("command")
            .map(String::as_str)
            .unwrap_or(command_path);
        insights.push(Insight {
            kind: insight_kind.to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "{label_prefix} `{}` runs `{command}` but command path `{command_path}` was not found",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
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

        insights.push(Insight {
            kind: "entrypoint_dead_end".to_string(),
            severity: InsightSeverity::Warning,
            message: format!(
                "Entrypoint `{}` has no outgoing code, config, dependency, or error flow",
                node.label
            ),
            nodes: vec![node.id],
            edges: incoming_edge_indexes(graph, node.id, EdgeKind::Entrypoint),
        });
    }
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
    insights.push(Insight {
        kind: "low_entrypoint_coverage".to_string(),
        severity: InsightSeverity::Warning,
        message: format!(
            "entrypoints reach {reached} of {} functions ({coverage}%), and {resolution}% of calls resolve to a scanned function — the rest name a dependency, the standard library, or a method the syntax cannot type{exported_note} — treat `unreachable_*` findings as gaps in call resolution, or as a library reached through its API, before reading them as dead code",
            functions.len()
        ),
        nodes: entrypoints.into_iter().take(8).collect(),
        edges: Vec::new(),
    });
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

pub(crate) fn add_conflicting_config_default_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
    for ((kind, label), reads) in config_key_reads(graph) {
        if reads.defaults.len() < 2 {
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

        insights.push(Insight {
            kind: "conflicting_config_default".to_string(),
            severity: InsightSeverity::Warning,
            message: format!("{kind} `{label}` is read with multiple fallback values: {values}"),
            nodes: nodes.into_iter().collect(),
            edges: edges.into_iter().collect(),
        });
    }
}

/// The first line in each file where a script gives the key a default by
/// assigning it to itself.
fn defaulting_assignment_lines(graph: &CodeGraph, reads: &ConfigKeyReads) -> BTreeMap<String, u32> {
    let mut guarded: BTreeMap<String, u32> = BTreeMap::new();
    for index in reads.defaults.values().flat_map(|(_, edges)| edges.iter()) {
        let Some(edge) = graph.edges.get(*index) else {
            continue;
        };
        if edge.metadata.get("defaults_variable").map(String::as_str) != Some("true") {
            continue;
        }
        let (Some(file), Some(line)) = (
            edge.metadata.get("file"),
            edge.metadata.get("line").and_then(|line| line.parse().ok()),
        ) else {
            continue;
        };
        let slot = guarded.entry(file.clone()).or_insert(line);
        *slot = (*slot).min(line);
    }
    guarded
}

/// Whether a read comes after the line that gave the key a default in the
/// same file.
fn read_is_guarded(graph: &CodeGraph, index: usize, guarded: &BTreeMap<String, u32>) -> bool {
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
    guarded.get(file).is_some_and(|guard| line > *guard)
}

pub(crate) fn add_mixed_config_requirement_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
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
            .filter(|index| !read_is_guarded(graph, *index, &guarded))
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

        insights.push(Insight {
            kind: "mixed_config_requirement".to_string(),
            severity: InsightSeverity::Warning,
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
        let severity = match kind {
            "security" => InsightSeverity::Error,
            "fixme" | "hack" | "bug" | "xxx" => InsightSeverity::Warning,
            _ => continue,
        };
        let edges = incoming_edge_indexes(graph, node.id, EdgeKind::Contains);
        let location = node
            .span
            .as_ref()
            .map(|span| format!("{}:{}", span.path, span.start_line))
            .unwrap_or_else(|| "unknown location".to_string());
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
        let Some(import) = imports
            .iter()
            .find(|import| declared_ecosystems.contains(import.ecosystem.as_str()))
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
            severity: InsightSeverity::Warning,
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
    nodes: Vec<NodeId>,
    edges: Vec<usize>,
}

impl UndeclaredImportGroup {
    fn record(&mut self, source: &str, source_id: NodeId, import_id: NodeId, edge: usize) {
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
        insights.push(Insight {
            kind: "conflicting_dependency_declaration".to_string(),
            severity: InsightSeverity::Warning,
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
        let scopes: BTreeSet<_> = declarations
            .iter()
            .map(|declaration| declaration.scope.as_str())
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
            severity: InsightSeverity::Warning,
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
        if path_index
            .get(&source.id)
            .map(String::as_str)
            .map(is_tool_configuration_source_path)
            .unwrap_or_else(|| is_tool_configuration_source_path(&source.label))
        {
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
        if import_node.label.trim_start().starts_with("import type ") {
            continue;
        }
        let Some(language) = import_node.metadata.get("language").map(String::as_str) else {
            continue;
        };
        let imports = import_package_candidates(language, &import_node.label, &declared_ecosystems);
        let Some((package_id, package_declarations)) = imports.iter().find_map(|import| {
            declarations
                .iter()
                .find(|(package_id, _)| import_matches_package_id(package_id, import))
        }) else {
            continue;
        };
        let scopes: BTreeSet<_> = package_declarations
            .iter()
            .map(|declaration| declaration.kind.as_str())
            .collect();
        if scopes.contains("runtime") {
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

pub(crate) fn add_duplicate_framework_route_insights(
    graph: &CodeGraph,
    insights: &mut Vec<Insight>,
) {
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
        if node
            .span
            .as_ref()
            .is_some_and(|span| is_test_like_source_path(&span.path))
        {
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
        groups
            .entry((method, path.to_string()))
            .or_default()
            .push(node.id);
    }

    for ((method, path), nodes) in groups {
        if nodes.len() < 2 {
            continue;
        }

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

        insights.push(Insight {
            kind: "unresolved_framework_route_handler".to_string(),
            severity: InsightSeverity::Warning,
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
        if !is_cycle_edge(&edge.kind) {
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
        let crosses_files = !(files.len() == 1 && placed.len() == component.len());
        let severity = if crosses_files {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Info
        };
        let scope = if crosses_files {
            "across files"
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

    import_package_candidate(language, label)
        .into_iter()
        .collect()
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
        "python" => package == canonical_python_package_name(&import.package),
        "npm" | "composer" | "dart" => package == import.package.to_ascii_lowercase(),
        "vcpkg" | "conan" | "cmake" => package == import.package.to_ascii_lowercase(),
        _ => package == import.package,
    }
}

pub(crate) fn rust_import_package(label: &str) -> Option<String> {
    let value = label.trim().strip_prefix("use ")?;
    let first = value
        .trim()
        .trim_start_matches("::")
        .split([':', ';', ',', '{', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())?;
    if matches!(first, "std" | "core" | "alloc" | "crate" | "self" | "super") {
        None
    } else {
        Some(first.to_ascii_lowercase())
    }
}

pub(crate) fn rust_path_package(label: &str) -> Option<String> {
    let first = label
        .trim()
        .trim_start_matches("::")
        .split("::")
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())?;
    if first.contains('.') || matches!(first, "std" | "core" | "alloc" | "crate" | "self" | "super")
    {
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

    let vendor = composer_package_part(parts[0]);
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

    if let Some(component) = parts.get(1) {
        packages.push(format!("{vendor}/{}", composer_package_part(component)));
    }
    packages.push(format!("{vendor}/{vendor}"));
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

pub(crate) fn composer_package_part(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_separator = false;
    for character in value.trim().chars() {
        if matches!(character, '_' | '-' | '.') {
            if !previous_separator && !normalized.is_empty() {
                normalized.push('-');
                previous_separator = true;
            }
            continue;
        }
        if character.is_ascii_uppercase() {
            if !normalized.is_empty() && !previous_separator {
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
        "python" => declared.contains(&format!(
            "python:{}",
            canonical_python_package_name(package)
        )),
        "npm" => declared.contains(&format!("npm:{}", package.to_ascii_lowercase())),
        "composer" => declared.contains(&format!("composer:{}", package.to_ascii_lowercase())),
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

pub(crate) fn is_python_stdlib_package(package: &str) -> bool {
    // Python's own `sys.stdlib_module_names`, minus the private
    // underscore modules. A partial list is worse than none here: the
    // insight warns that an import is undeclared, and every module it
    // does not know makes that claim about the standard library.
    matches!(
        package,
        "abc"
            | "annotationlib"
            | "antigravity"
            | "argparse"
            | "array"
            | "ast"
            | "asyncio"
            | "atexit"
            | "base64"
            | "bdb"
            | "binascii"
            | "bisect"
            | "builtins"
            | "bz2"
            | "cProfile"
            | "calendar"
            | "cmath"
            | "cmd"
            | "code"
            | "codecs"
            | "codeop"
            | "collections"
            | "colorsys"
            | "compileall"
            | "compression"
            | "concurrent"
            | "configparser"
            | "contextlib"
            | "contextvars"
            | "copy"
            | "copyreg"
            | "csv"
            | "ctypes"
            | "curses"
            | "dataclasses"
            | "datetime"
            | "dbm"
            | "decimal"
            | "difflib"
            | "dis"
            | "doctest"
            | "email"
            | "encodings"
            | "ensurepip"
            | "enum"
            | "errno"
            | "faulthandler"
            | "fcntl"
            | "filecmp"
            | "fileinput"
            | "fnmatch"
            | "fractions"
            | "ftplib"
            | "functools"
            | "gc"
            | "genericpath"
            | "getopt"
            | "getpass"
            | "gettext"
            | "glob"
            | "graphlib"
            | "grp"
            | "gzip"
            | "hashlib"
            | "heapq"
            | "hmac"
            | "html"
            | "http"
            | "idlelib"
            | "imaplib"
            | "importlib"
            | "inspect"
            | "io"
            | "ipaddress"
            | "itertools"
            | "json"
            | "keyword"
            | "linecache"
            | "locale"
            | "logging"
            | "lzma"
            | "mailbox"
            | "marshal"
            | "math"
            | "mimetypes"
            | "mmap"
            | "modulefinder"
            | "msvcrt"
            | "multiprocessing"
            | "netrc"
            | "nt"
            | "ntpath"
            | "nturl2path"
            | "numbers"
            | "opcode"
            | "operator"
            | "optparse"
            | "os"
            | "pathlib"
            | "pdb"
            | "pickle"
            | "pickletools"
            | "pkgutil"
            | "platform"
            | "plistlib"
            | "poplib"
            | "posix"
            | "posixpath"
            | "pprint"
            | "profile"
            | "pstats"
            | "pty"
            | "pwd"
            | "py_compile"
            | "pyclbr"
            | "pydoc"
            | "pydoc_data"
            | "pyexpat"
            | "queue"
            | "quopri"
            | "random"
            | "re"
            | "readline"
            | "reprlib"
            | "resource"
            | "rlcompleter"
            | "runpy"
            | "sched"
            | "secrets"
            | "select"
            | "selectors"
            | "shelve"
            | "shlex"
            | "shutil"
            | "signal"
            | "site"
            | "smtplib"
            | "socket"
            | "socketserver"
            | "sqlite3"
            | "sre_compile"
            | "sre_constants"
            | "sre_parse"
            | "ssl"
            | "stat"
            | "statistics"
            | "string"
            | "stringprep"
            | "struct"
            | "subprocess"
            | "symtable"
            | "sys"
            | "sysconfig"
            | "syslog"
            | "tabnanny"
            | "tarfile"
            | "tempfile"
            | "termios"
            | "textwrap"
            | "this"
            | "threading"
            | "time"
            | "timeit"
            | "tkinter"
            | "token"
            | "tokenize"
            | "tomllib"
            | "trace"
            | "traceback"
            | "tracemalloc"
            | "tty"
            | "turtle"
            | "turtledemo"
            | "types"
            | "typing"
            | "unicodedata"
            | "unittest"
            | "urllib"
            | "uuid"
            | "venv"
            | "warnings"
            | "wave"
            | "weakref"
            | "webbrowser"
            | "winreg"
            | "winsound"
            | "wsgiref"
            | "xml"
            | "xmlrpc"
            | "zipapp"
            | "zipfile"
            | "zipimport"
            | "zlib"
            | "zoneinfo"
    )
}
