//! Unit tests for the analysis crate, organized by feature area.

use super::*;
use codegraph_core::{
    COMPUTED_ENVIRONMENT_KEY, CodeGraph, Confidence, EdgeKind, NodeId, NodeKind, SourceSpan,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn svg_export_renders_deterministic_circle_layout() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/<main>.rs");
    let function = graph.add_node(NodeKind::Function, "run & serve");
    graph.add_edge(file, function, EdgeKind::Contains, Confidence::Exact);

    let svg = export_svg(&graph, 500, 1500);
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.ends_with("</svg>\n"));
    assert!(svg.contains("<circle"));
    assert!(svg.contains("<line"));
    // Labels and titles are XML-escaped.
    assert!(svg.contains("src/&lt;main&gt;.rs"));
    assert!(svg.contains("run &amp; serve"));
    assert!(!svg.contains("<main>"));
    // Deterministic: same graph renders the same bytes.
    assert_eq!(svg, export_svg(&graph, 500, 1500));

    // Truncation is recorded and respected.
    let truncated = export_svg(&graph, 2, 1);
    assert!(truncated.contains("3 of 3 nodes") || truncated.contains("2 of 3 nodes"));
    assert!(truncated.contains("of 3 nodes"));
}

#[test]
fn insight_severity_parses_shared_spellings() {
    assert_eq!("info".parse::<InsightSeverity>(), Ok(InsightSeverity::Info));
    assert_eq!(
        " Warning ".parse::<InsightSeverity>(),
        Ok(InsightSeverity::Warning)
    );
    assert_eq!(
        "warn".parse::<InsightSeverity>(),
        Ok(InsightSeverity::Warning)
    );
    assert_eq!(
        "ERROR".parse::<InsightSeverity>(),
        Ok(InsightSeverity::Error)
    );
    assert!(
        "fatal"
            .parse::<InsightSeverity>()
            .unwrap_err()
            .contains("expected info, warning, or error")
    );
}

#[test]
fn report_limits_clamp_to_published_bounds() {
    let clamped = ProjectReportLimits {
        architecture_group_limit: 0,
        architecture_edge_limit: usize::MAX,
        language_link_limit: 0,
        hotspot_limit: usize::MAX,
        community_limit: 0,
        insight_limit: usize::MAX,
        file_summary_limit: 0,
        node_summary_limit: usize::MAX,
        fail_on: InsightSeverity::Warning,
    }
    .clamped();
    assert_eq!(clamped.architecture_group_limit, 1);
    assert_eq!(
        clamped.architecture_edge_limit,
        MAX_REPORT_ARCHITECTURE_EDGE_LIMIT
    );
    assert_eq!(clamped.language_link_limit, 1);
    assert_eq!(clamped.hotspot_limit, MAX_REPORT_HOTSPOT_LIMIT);
    assert_eq!(clamped.insight_limit, MAX_REPORT_INSIGHT_LIMIT);
    assert_eq!(clamped.node_summary_limit, MAX_REPORT_NODE_SUMMARY_LIMIT);
    assert_eq!(clamped.fail_on, InsightSeverity::Warning);
}

#[test]
fn workflow_filters_normalize_user_supplied_parts() {
    let filters = WorkflowFilters::from_parts(
        Some(" calls ".to_string()),
        Some(String::new()),
        None,
        Some("  ".to_string()),
        Some("branch".to_string()),
    );
    assert_eq!(filters.edge_kind.as_deref(), Some("calls"));
    assert_eq!(filters.confidence, None);
    assert_eq!(filters.language, None);
    assert_eq!(filters.risk_severity, None);
    assert_eq!(filters.block_kind.as_deref(), Some("branch"));
}

#[test]
fn summary_counts_graph_facts() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let mut metadata = BTreeMap::new();
    metadata.insert("annotation.domain".to_string(), "payments".to_string());
    metadata.insert("annotation.owner".to_string(), "team-payments".to_string());
    graph.add_node_with_metadata(NodeKind::File, "src/payments.rs", None, metadata);
    graph.add_edge_with_metadata(
        graph.root,
        main,
        EdgeKind::Entrypoint,
        Confidence::Syntactic,
        BTreeMap::from([
            ("relation".to_string(), "entrypoint_function".to_string()),
            ("source".to_string(), "manifest".to_string()),
        ]),
    );

    let summary = summarize(&graph);

    assert_eq!(summary.nodes, 3);
    assert_eq!(summary.edges, 1);
    assert_eq!(summary.entrypoints, 1);
    assert_eq!(summary.node_kinds.get("function"), Some(&1));
    assert_eq!(summary.edge_confidences.get("syntactic"), Some(&1));
    assert_eq!(summary.edge_relations.get("entrypoint_function"), Some(&1));
    assert_eq!(summary.edge_sources.get("manifest"), Some(&1));
    assert_eq!(
        summary
            .annotation_facets
            .get("annotation.domain")
            .and_then(|values| values.get("payments")),
        Some(&1)
    );
    assert_eq!(
        summary
            .annotation_facets
            .get("annotation.owner")
            .and_then(|values| values.get("team-payments")),
        Some(&1)
    );
}

#[test]
fn a_file_that_could_not_be_read_is_a_finding() {
    let mut graph = CodeGraph::new("repo");
    graph.add_node(NodeKind::File, "src/main.rs");
    let unreadable = graph.add_node_with_metadata(
        NodeKind::File,
        "src/secret.rs",
        None,
        BTreeMap::from([(
            "read_error".to_string(),
            "Permission denied (os error 13)".to_string(),
        )]),
    );

    let report = insights(&graph);
    let finding = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unreadable_file")
        .expect("a file with no facts must say why");
    assert_eq!(finding.severity, InsightSeverity::Error);
    assert_eq!(finding.nodes, vec![unreadable]);
    assert!(
        finding.message.contains("Permission denied"),
        "message: {}",
        finding.message
    );
}

#[test]
fn project_report_combines_summary_quality_and_limited_views() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    graph.add_node_with_metadata(
        NodeKind::File,
        "src/broken.rs",
        None,
        BTreeMap::from([("parse_error".to_string(), "unexpected token".to_string())]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_node(NodeKind::Function, "orphan");
    let config = graph.add_node(NodeKind::Config, "DATABASE_URL");
    let unresolved = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "missing",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "call".to_string()),
            ("resolution".to_string(), "unresolved".to_string()),
        ]),
    );
    graph.add_edge(file, main, EdgeKind::Defines, Confidence::Exact);
    graph.add_edge(file, main, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(main, unresolved, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let report = project_report(
        &graph,
        ProjectReportLimits {
            architecture_group_limit: 5,
            architecture_edge_limit: 5,
            language_link_limit: 5,
            hotspot_limit: 1,
            community_limit: 5,
            insight_limit: 1,
            file_summary_limit: 1,
            node_summary_limit: 2,
            fail_on: InsightSeverity::Warning,
        },
    );

    assert_eq!(report.graph_schema_version, graph.schema_version);
    assert_eq!(report.summary.nodes, graph.nodes.len());
    assert_eq!(report.entrypoints.len(), 1);
    assert_eq!(report.hotspots.hotspots.len(), 1);
    assert_eq!(
        report.hotspots.total_architectural_hubs + report.hotspots.total_utility_hubs,
        report.hotspots.total_candidates
    );
    assert!(!report.surprising_links.links.is_empty());
    assert!(!report.communities.communities.is_empty());
    assert_eq!(report.quality_gate.fail_on, "warning");
    assert_eq!(report.insights.insights.len(), 1);
    assert_eq!(report.file_summaries.files.len(), 1);
    assert_eq!(report.file_summaries.total_files, 2);
    assert!(report.file_summaries.truncated);
    assert_eq!(report.node_summaries.nodes.len(), 2);
    assert!(report.node_summaries.total_nodes >= 3);
    assert!(report.node_summaries.truncated);
    assert!(
        report
            .node_summaries
            .nodes
            .iter()
            .any(|summary| summary.roles.iter().any(|role| role == "entrypoint"))
    );
    assert_eq!(report.insights.total, report.quality_gate.report.total);
    assert!(report.quality_gate.report.insights.len() <= REPORT_QUALITY_GATE_SAMPLE_LIMIT);
    assert_eq!(report.risk_summary.total, report.quality_gate.report.total);
    assert_eq!(report.risk_summary.errors, 1);
    // The unresolved call reads as info on this syntactic-only fixture.
    assert_eq!(report.risk_summary.warnings, 0);
    assert!(report.risk_summary.infos >= 2);
    // The score weighs actionable findings only; infos stay counts.
    assert_eq!(report.risk_summary.score, 100);
    assert_eq!(report.risk_summary.grade, "low");
    assert!(!report.quality_gate.passed);
    assert_eq!(report.quality_gate.failing_insights, 1);
    assert!(
        report
            .risk_summary
            .top_kinds
            .iter()
            .any(|risk| risk.kind == "parse_error" && risk.severity == "error")
    );
    // The reason separates a grammar this scan cannot read from a file
    // nothing can: redis's `life.lua` is Latin-1, not broken Lua.
    assert!(
        report
            .insights
            .insights
            .iter()
            .any(|insight| insight.kind == "parse_error"
                && insight
                    .message
                    .ends_with("failed to parse: unexpected token")),
        "{:?}",
        report
            .insights
            .insights
            .iter()
            .map(|insight| insight.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn facet_names_match_serde_source_of_truth() {
    use codegraph_core::Confidence;
    // The direct-match name helpers replaced per-call serde serialization for
    // speed; they must stay byte-identical to the serde `rename_all` names, so
    // a new or renamed variant fails here until both are updated together.
    for kind in [
        NodeKind::Repository,
        NodeKind::Directory,
        NodeKind::File,
        NodeKind::Module,
        NodeKind::Function,
        NodeKind::Entrypoint,
        NodeKind::Type,
        NodeKind::Config,
        NodeKind::Environment,
        NodeKind::ExternalDependency,
        NodeKind::ControlFlow,
        NodeKind::Unknown,
    ] {
        assert_eq!(
            kind_name(&kind),
            serde_json_name(&kind).unwrap(),
            "node kind {kind:?}"
        );
    }
    for kind in [
        EdgeKind::Contains,
        EdgeKind::Imports,
        EdgeKind::Calls,
        EdgeKind::Defines,
        EdgeKind::References,
        EdgeKind::ReadsConfig,
        EdgeKind::ReadsEnvironment,
        EdgeKind::MayError,
        EdgeKind::Entrypoint,
        EdgeKind::DependsOn,
    ] {
        assert_eq!(
            edge_kind_name(&kind),
            serde_json_name(&kind).unwrap(),
            "edge kind {kind:?}"
        );
    }
    for confidence in [
        Confidence::Exact,
        Confidence::Semantic,
        Confidence::Syntactic,
        Confidence::Heuristic,
        Confidence::Unknown,
    ] {
        assert_eq!(
            confidence_name(confidence),
            serde_json_name(&confidence).unwrap(),
            "confidence {confidence:?}"
        );
    }
}

#[test]
fn summaries_from_adjacency_indexes_match_full_scans() {
    // Guards the report's adjacency-index fast paths: they must produce exactly
    // what the per-node full-graph scans produced, including a self-loop counted
    // once as one incoming and one outgoing edge.
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/lib.rs");
    let a = graph.add_node(NodeKind::Function, "a");
    let b = graph.add_node(NodeKind::Function, "b");
    let config = graph.add_node(NodeKind::Config, "PORT");
    graph.add_edge(file, a, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(file, b, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(a, a, EdgeKind::Calls, Confidence::Heuristic); // self-loop
    graph.add_edge(a, b, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(b, a, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(a, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let dep_a = node_dependency_summary(&graph, a);
    assert_eq!(dep_a.outgoing, 3, "a->a, a->b, a->config");
    assert_eq!(dep_a.incoming, 3, "file->a (Contains), a->a, b->a");

    let nodes_by_id = nodes_by_id_index(&graph);
    let incident = incident_edge_index(&graph);
    let no_edges: Vec<&codegraph_core::Edge> = Vec::new();
    let indexed =
        node_dependency_summary_indexed(&nodes_by_id, incident.get(&a).unwrap_or(&no_edges), a);
    assert_eq!(
        indexed, dep_a,
        "indexed dependency summary matches full scan"
    );

    let file_node = graph.nodes.iter().find(|n| n.id == file).unwrap().clone();
    let outgoing = outgoing_edge_index(&graph);
    let summary = file_node_summary_indexed(&nodes_by_id, &outgoing, &file_node).unwrap();
    assert_eq!(summary.code_symbols, 2);
    assert_eq!(summary.calls, 3, "a->a, a->b, b->a");
    assert_eq!(summary.config_reads, 1, "a->config");
    assert_eq!(summary.trace_edges, 4, "3 calls + 1 config read");
    assert_eq!(
        file_node_summary(&graph, &file_node),
        Some(summary),
        "indexed file summary matches full scan"
    );
}

#[test]
fn project_report_markdown_includes_evidence_and_suggested_questions() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    let main = graph.add_node_with_span(
        NodeKind::Function,
        "main",
        SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 3,
            start_column: 1,
            end_line: 5,
            end_column: 2,
        },
    );
    let config = graph.add_node(NodeKind::Config, "DATABASE_URL");
    let unresolved = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "missing_call",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "call".to_string()),
            ("resolution".to_string(), "unresolved".to_string()),
        ]),
    );
    graph.add_edge(file, main, EdgeKind::Defines, Confidence::Exact);
    graph.add_edge(file, main, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(main, unresolved, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let report = project_report(
        &graph,
        ProjectReportLimits {
            architecture_group_limit: 5,
            architecture_edge_limit: 5,
            language_link_limit: 5,
            hotspot_limit: 5,
            community_limit: 5,
            insight_limit: 5,
            file_summary_limit: 5,
            node_summary_limit: 5,
            fail_on: InsightSeverity::Warning,
        },
    );
    let markdown = project_report_markdown(
        &report,
        &ProjectReportMarkdownOptions {
            title: "CodeGraph Project Report".to_string(),
            root: Some("repo".to_string()),
            generated_at_unix: Some(1_234),
        },
    );

    assert!(markdown.contains("# CodeGraph Project Report"));
    assert!(markdown.contains("- Root: `repo`"));
    assert!(markdown.contains("## Confidence Guide"));
    assert!(markdown.contains("## Compact Node Summaries"));
    assert!(markdown.contains("entrypoint"));
    assert!(markdown.contains("## Compact File Summaries"));
    assert!(markdown.contains("src/main.rs"));
    assert!(markdown.contains("| `exact` | extracted |"));
    assert!(markdown.contains("| `heuristic` | inferred |"));
    assert!(markdown.contains("| `unknown` | ambiguous |"));
    assert!(markdown.contains("## Key Concepts"));
    assert!(markdown.contains("## Communities"));
    assert!(markdown.contains("## Surprising Links"));
    assert!(markdown.contains("## Risks And Insights"));
    assert!(markdown.contains("### Insight Evidence"));
    assert!(markdown.contains("## Suggested Questions"));
    assert!(markdown.contains("missing_call"));
    assert!(markdown.contains("#2"));
    assert!(markdown.contains("What startup flow is reachable from main?"));
}

#[test]
fn architecture_map_groups_files_and_cross_group_edges() {
    let mut graph = CodeGraph::new("repo");
    let api_file = graph.add_node(NodeKind::File, "api/main.rs");
    let core_file = graph.add_node(NodeKind::File, "core/lib.rs");
    let api_main = graph.add_node(NodeKind::Function, "main");
    let core_load = graph.add_node(NodeKind::Function, "load_config");
    graph.add_edge(graph.root, api_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, core_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(
        api_file,
        api_main,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );
    graph.add_edge(
        core_file,
        core_load,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );
    graph.add_edge(api_main, core_load, EdgeKind::Calls, Confidence::Heuristic);

    let map = architecture_map(&graph, 10, 10);

    assert_eq!(map.total_groups, 2);
    assert_eq!(map.total_edges, 1);
    let api = map.groups.iter().find(|group| group.id == "api").unwrap();
    let core = map.groups.iter().find(|group| group.id == "core").unwrap();
    assert_eq!(api.files, 1);
    assert_eq!(api.symbols, 1);
    assert_eq!(core.files, 1);
    assert_eq!(core.symbols, 1);
    assert_eq!(map.edges[0].source, "api");
    assert_eq!(map.edges[0].target, "core");
    assert_eq!(map.edges[0].edge_kinds.get("calls"), Some(&1));
    assert_eq!(map.edges[0].edge_indexes, vec![4]);
}

#[test]
fn surprising_links_rank_cross_area_language_and_heuristic_edges() {
    let mut graph = CodeGraph::new("repo");
    let api_file = graph.add_node_with_metadata(
        NodeKind::File,
        "api/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let scripts_file = graph.add_node_with_metadata(
        NodeKind::File,
        "scripts/deploy.py",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    let handler = graph.add_node_with_metadata(
        NodeKind::Function,
        "handle_request",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let script = graph.add_node_with_metadata(
        NodeKind::Function,
        "deploy",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    graph.add_edge(api_file, handler, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(scripts_file, script, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(handler, script, EdgeKind::Calls, Confidence::Heuristic);

    let report = surprising_links(&graph, 10);

    assert_eq!(report.total_candidates, 1);
    let link = &report.links[0];
    assert_eq!(link.source.label, "handle_request");
    assert_eq!(link.target.label, "deploy");
    assert_eq!(link.source_area, "api");
    assert_eq!(link.target_area, "scripts");
    assert_eq!(link.source_language, "rust");
    assert_eq!(link.target_language, "python");
    assert_eq!(link.confidence, "heuristic");
    assert!(link.score >= 12);
    assert!(link.reasons.contains(&"cross_area".to_string()));
    assert!(link.reasons.contains(&"cross_language".to_string()));
    assert!(link.reasons.contains(&"heuristic_confidence".to_string()));
    assert_eq!(link.edge_index, 2);
}

#[test]
fn language_dependencies_group_edges_by_node_languages() {
    let mut graph = CodeGraph::new("repo");
    let rust_main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let python_helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    let python_config = graph.add_node_with_metadata(
        NodeKind::Config,
        "settings.yaml",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    graph.add_edge(
        rust_main,
        python_helper,
        EdgeKind::Calls,
        Confidence::Heuristic,
    );
    graph.add_edge(
        python_helper,
        python_config,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );
    graph.add_edge(graph.root, rust_main, EdgeKind::Contains, Confidence::Exact);

    let report = language_dependencies(&graph, 10);

    assert_eq!(report.total_links, 2);
    assert_eq!(report.total_edges, 2);
    assert_eq!(report.cross_language_edges, 1);
    let cross = report
        .links
        .iter()
        .find(|link| link.source_language == "rust" && link.target_language == "python")
        .unwrap();
    assert_eq!(cross.count, 1);
    assert_eq!(cross.edge_kinds.get("calls"), Some(&1));
    assert_eq!(cross.confidences.get("heuristic"), Some(&1));
    assert_eq!(cross.edge_indexes, vec![0]);
}

#[test]
fn insights_report_cross_language_heuristic_edges() {
    let mut graph = CodeGraph::new("repo");
    let rust_main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let python_helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    let rust_helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper_rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_edge(
        rust_main,
        python_helper,
        EdgeKind::Calls,
        Confidence::Heuristic,
    );
    graph.add_edge(
        rust_main,
        rust_helper,
        EdgeKind::Calls,
        Confidence::Heuristic,
    );
    graph.add_edge(
        python_helper,
        rust_helper,
        EdgeKind::References,
        Confidence::Exact,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "cross_language_heuristic_edge")
        .expect("expected cross-language heuristic insight");

    // Syntactic-only fixture: heuristic findings read as info.
    assert_eq!(insight.severity, InsightSeverity::Info);
    assert_eq!(insight.edges, vec![0]);
    assert!(insight.nodes.contains(&rust_main));
    assert!(insight.nodes.contains(&python_helper));
    assert_eq!(
        report.by_kind.get("cross_language_heuristic_edge"),
        Some(&1)
    );
}

#[test]
fn hotspots_rank_nodes_by_dependency_degree() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    let settings = graph.add_node(NodeKind::Config, "settings.toml");
    let helper = graph.add_node(NodeKind::Function, "helper");
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        settings,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );

    let report = hotspots(&graph, 2);

    assert_eq!(report.total_candidates, 4);
    assert!(report.truncated);
    assert_eq!(report.hotspots[0].node.label, "load_config");
    assert_eq!(report.hotspots[0].score, 3);
    assert_eq!(report.hotspots[0].incoming, 2);
    assert_eq!(report.hotspots[0].outgoing, 1);
    assert_eq!(report.hotspots[0].hub_kind, "architectural");
    assert_eq!(report.architectural_hubs[0].node.label, "load_config");
    assert_eq!(report.total_architectural_hubs, 4);
    assert_eq!(report.total_utility_hubs, 0);
    assert_eq!(report.hotspots[0].edge_kinds.get("calls"), Some(&2));
    assert_eq!(report.hotspots[0].edge_kinds.get("reads_config"), Some(&1));
}

#[test]
fn a_fixtures_weight_comes_after_the_programs_own() {
    let mut graph = CodeGraph::new("repo");
    let placed = |graph: &mut CodeGraph, kind: NodeKind, label: &str, path: &str| {
        graph.add_node_with_span(
            kind,
            label,
            SourceSpan {
                path: path.to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 4,
                end_column: 1,
            },
        )
    };
    // kong's two highest-scoring files were fixture plugins, and its
    // highest-scoring symbol a fixture's `CtxTests:log`.
    let fixture = graph.add_node(NodeKind::File, "spec/fixtures/plugins/handler.lua");
    let own = graph.add_node(NodeKind::File, "kong/db/schema/init.lua");
    for index in 0..8 {
        let symbol = placed(
            &mut graph,
            NodeKind::Function,
            &format!("CtxTests:log_{index}"),
            "spec/fixtures/plugins/handler.lua",
        );
        graph.add_edge(fixture, symbol, EdgeKind::Contains, Confidence::Exact);
        let target = placed(
            &mut graph,
            NodeKind::Function,
            &format!("fixture_target_{index}"),
            "spec/fixtures/plugins/handler.lua",
        );
        graph.add_edge(symbol, target, EdgeKind::Calls, Confidence::Heuristic);
    }
    for index in 0..3 {
        let symbol = placed(
            &mut graph,
            NodeKind::Function,
            &format!("validate_{index}"),
            "kong/db/schema/init.lua",
        );
        graph.add_edge(own, symbol, EdgeKind::Contains, Confidence::Exact);
        let target = placed(
            &mut graph,
            NodeKind::Function,
            &format!("own_target_{index}"),
            "kong/db/schema/init.lua",
        );
        graph.add_edge(symbol, target, EdgeKind::Calls, Confidence::Heuristic);
    }

    let report = project_report(
        &graph,
        ProjectReportLimits {
            architecture_group_limit: 5,
            architecture_edge_limit: 5,
            language_link_limit: 5,
            hotspot_limit: 5,
            community_limit: 5,
            insight_limit: 10,
            file_summary_limit: 4,
            node_summary_limit: 4,
            fail_on: InsightSeverity::Error,
        },
    );
    assert_eq!(
        report.file_summaries.files[0].node.label,
        "kong/db/schema/init.lua"
    );
    assert!(
        report.node_summaries.nodes[0]
            .node
            .label
            .starts_with("validate_"),
        "{:?}",
        report
            .node_summaries
            .nodes
            .iter()
            .map(|entry| entry.node.label.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_test_helper_hub_comes_after_the_programs_own() {
    let mut graph = CodeGraph::new("repo");
    let placed = |graph: &mut CodeGraph, label: &str, path: &str| {
        graph.add_node_with_span(
            NodeKind::Function,
            label,
            SourceSpan {
                path: path.to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 2,
                end_column: 1,
            },
        )
    };
    // kong's biggest hub is `spec/helpers.lua`, referenced by 372 spec
    // files; its own `load` has fewer callers and is what a reader wants.
    let helper = placed(&mut graph, "helpers", "spec/helpers.lua");
    let load = placed(&mut graph, "load", "kong/init.lua");
    for index in 0..6 {
        let spec = placed(
            &mut graph,
            &format!("spec_{index}"),
            "spec/01-unit/x_spec.lua",
        );
        graph.add_edge(spec, helper, EdgeKind::Calls, Confidence::Heuristic);
    }
    for index in 0..2 {
        let caller = placed(&mut graph, &format!("caller_{index}"), "kong/router.lua");
        graph.add_edge(caller, load, EdgeKind::Calls, Confidence::Heuristic);
    }

    let report = hotspots(&graph, 10);
    assert_eq!(report.hotspots[0].node.label, "load");
    let ranks: Vec<_> = report
        .hotspots
        .iter()
        .map(|hotspot| hotspot.node.label.as_str())
        .collect();
    let helper_rank = ranks.iter().position(|label| *label == "helpers");
    assert!(
        helper_rank.is_some_and(|rank| rank > 0),
        "the test helper is still reported, after the program's own hubs: {ranks:?}"
    );
}

#[test]
fn hotspots_separate_architectural_hubs_from_utility_hubs() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Entrypoint, "server main");
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    let new_fn = graph.add_node(NodeKind::Function, "new");
    let default_fn = graph.add_node(NodeKind::Function, "default");
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, new_fn, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(load_config, new_fn, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(default_fn, new_fn, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, default_fn, EdgeKind::Calls, Confidence::Heuristic);

    let report = hotspots(&graph, 5);

    assert!(
        report
            .utility_hubs
            .iter()
            .any(|hotspot| hotspot.node.label == "new" && hotspot.hub_kind == "utility")
    );
    assert!(
        report
            .architectural_hubs
            .iter()
            .any(|hotspot| hotspot.node.label == "server main"
                && hotspot.hub_kind == "architectural")
    );
    assert!(report.total_utility_hubs >= 2);
    assert!(report.total_architectural_hubs >= 2);
}

#[test]
fn communities_group_related_files_symbols_and_external_edges() {
    let mut graph = CodeGraph::new("repo");
    let api_file = graph.add_node_with_metadata(
        NodeKind::File,
        "api/users.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let core_file = graph.add_node_with_metadata(
        NodeKind::File,
        "core/db.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let _docs_file = graph.add_node(NodeKind::File, "docs/adr.md");
    let route = graph.add_node(NodeKind::Entrypoint, "route GET /users");
    let handler = graph.add_node(NodeKind::Function, "list_users");
    let db = graph.add_node(NodeKind::Function, "load_users");
    graph.add_edge(api_file, route, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(api_file, handler, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(core_file, db, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(route, handler, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(handler, db, EdgeKind::Calls, Confidence::Heuristic);

    let report = communities(&graph, 1);

    assert_eq!(report.total_communities, 3);
    assert_eq!(report.total_nodes, 6);
    assert_eq!(report.total_external_edges, 1);
    assert!(report.truncated);
    let community = &report.communities[0];
    assert_eq!(community.label, "api");
    assert_eq!(community.node_count, 3);
    assert_eq!(community.files, 1);
    assert_eq!(community.entrypoints, 1);
    assert_eq!(community.internal_edges, 3);
    assert_eq!(community.incoming_external_edges, 0);
    assert_eq!(community.outgoing_external_edges, 1);
    assert_eq!(community.languages.get("rust"), Some(&1));
    assert_eq!(community.node_kinds.get("file"), Some(&1));
    assert!(
        community
            .sample_nodes
            .iter()
            .any(|node| node.label == "route GET /users")
    );
    assert_eq!(community.edge_indexes, vec![0, 1, 3, 4]);

    let full = communities(&graph, 10);
    let core = full
        .communities
        .iter()
        .find(|community| community.label == "core")
        .expect("core community should be present");
    assert_eq!(core.incoming_external_edges, 1);
    assert_eq!(core.outgoing_external_edges, 0);
    let docs = full
        .communities
        .iter()
        .find(|community| community.label == "docs")
        .expect("isolated docs file should remain visible as a community");
    assert_eq!(docs.node_count, 1);
    assert_eq!(docs.files, 1);
}

#[test]
fn source_search_filters_limits_and_returns_context() {
    let root = temp_analysis_root();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(
        root.join("src").join("app.py"),
        "def main():\n    token = load_secret()\n    return token\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src").join("config.py"),
        "SECRET_NAME = 'token'\n",
    )
    .unwrap();
    std::fs::write(
        root.join("target").join("generated.py"),
        "token = 'ignored'\n",
    )
    .unwrap();

    let result = search_source(
        &root,
        &SourceSearchRequest {
            query: "TOKEN".to_string(),
            path_filter: Some("src/".to_string()),
            case_sensitive: false,
            limit: 2,
            context: 1,
            include_hidden: false,
            include_ignored: false,
            max_file_size: 1024,
            ignored_names: BTreeSet::from(["target".to_string()]),
            ignored_globs: BTreeSet::from(["fixtures/**".to_string()]),
        },
    );

    assert_eq!(result.total_matches, 3);
    assert_eq!(result.matches.len(), 2);
    assert!(result.truncated);
    assert!(
        result
            .matches
            .iter()
            .all(|item| item.path.starts_with("src/"))
    );
    assert!(
        !result
            .matches
            .iter()
            .any(|item| item.path.contains("target"))
    );
    let app_match = result
        .matches
        .iter()
        .find(|item| item.path == "src/app.py" && item.line == 2)
        .expect("missing app.py token match");
    assert_eq!(app_match.column, 5);
    assert!(
        app_match
            .context
            .iter()
            .any(|line| line.highlight && line.text.contains("token = load_secret()"))
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn source_search_respects_ignored_globs() {
    let root = temp_analysis_root();
    std::fs::create_dir_all(root.join("src").join("generated")).unwrap();
    std::fs::create_dir_all(root.join("src").join("domain")).unwrap();
    std::fs::write(
        root.join("src").join("generated").join("skip.py"),
        "token = 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src").join("domain").join("keep.py"),
        "token = 2\n",
    )
    .unwrap();

    let result = search_source(
        &root,
        &SourceSearchRequest {
            query: "token".to_string(),
            path_filter: None,
            case_sensitive: false,
            limit: 10,
            context: 0,
            include_hidden: false,
            include_ignored: false,
            max_file_size: 1024,
            ignored_names: BTreeSet::new(),
            ignored_globs: BTreeSet::from(["src/generated/**".to_string()]),
        },
    );

    assert_eq!(result.total_matches, 1);
    assert_eq!(result.matches[0].path, "src/domain/keep.py");

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_card_includes_context_source_and_related_insights() {
    let root = temp_analysis_root();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src").join("main.rs"),
        "fn main() {\n    missing();\n}\n",
    )
    .unwrap();
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    let function = graph.add_node_with_span(
        NodeKind::Function,
        "main",
        SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 3,
            end_column: 2,
        },
    );
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "call".to_string());
    metadata.insert("unresolved".to_string(), "true".to_string());
    let call = graph.add_node_with_metadata(NodeKind::Unknown, "missing", None, metadata);
    let env = graph.add_node_with_metadata(
        NodeKind::Environment,
        "DATABASE_URL",
        None,
        BTreeMap::from([(
            "default_value".to_string(),
            "postgres://demo:password@localhost/app".to_string(),
        )]),
    );
    let mut error_metadata = BTreeMap::new();
    error_metadata.insert("item_kind".to_string(), "error".to_string());
    let error = graph.add_node_with_metadata(NodeKind::Unknown, "panic", None, error_metadata);
    graph.add_edge(file, function, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(function, call, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        function,
        env,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(function, error, EdgeKind::MayError, Confidence::Heuristic);

    let card = node_card(
        &graph,
        Some(&root),
        NodeCardRequest {
            node_id: function,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .unwrap()
    .expect("expected node card");

    assert_eq!(card.context.node.id, function);
    assert_eq!(card.context.edges.len(), 4);
    assert_eq!(card.dependency_summary.incoming, 1);
    assert_eq!(card.dependency_summary.outgoing, 3);
    assert_eq!(card.dependency_summary.edge_kinds.get("contains"), Some(&1));
    assert_eq!(card.dependency_summary.edge_kinds.get("calls"), Some(&1));
    assert_eq!(
        card.dependency_summary.edge_kinds.get("reads_environment"),
        Some(&1)
    );
    assert_eq!(
        card.dependency_summary.edge_kinds.get("may_error"),
        Some(&1)
    );
    assert_eq!(card.dependency_summary.confidences.get("exact"), Some(&1));
    assert_eq!(
        card.dependency_summary.confidences.get("heuristic"),
        Some(&3)
    );
    assert_eq!(card.dependency_summary.neighbor_kinds.get("file"), Some(&1));
    assert_eq!(
        card.dependency_summary.neighbor_kinds.get("unknown"),
        Some(&2)
    );
    assert_eq!(
        card.dependency_summary.neighbor_kinds.get("environment"),
        Some(&1)
    );
    assert_eq!(
        card.source.as_ref().map(|source| source.path.as_str()),
        Some("src/main.rs")
    );
    assert!(
        card.source
            .as_ref()
            .unwrap()
            .lines
            .iter()
            .any(|line| line.highlight && line.text.contains("missing"))
    );
    assert_eq!(card.total_insights, 3);
    // Error-flow and heuristic findings read as info on syntactic scans.
    assert_eq!(card.insight_summary.by_severity.get("warning"), Some(&1));
    assert_eq!(card.insight_summary.by_severity.get("info"), Some(&2));
    assert_eq!(
        card.insight_summary.by_kind.get("orphan_function"),
        Some(&1)
    );
    assert_eq!(
        card.insight_summary.by_kind.get("potential_error_flow"),
        Some(&1)
    );
    assert_eq!(
        card.insight_summary.by_kind.get("sensitive_config_default"),
        Some(&1)
    );
    assert!(
        card.insights
            .iter()
            .any(|insight| insight.kind == "orphan_function")
    );
    assert!(
        card.insights
            .iter()
            .any(|insight| insight.kind == "potential_error_flow")
    );
    assert!(
        card.insights
            .iter()
            .any(|insight| insight.kind == "sensitive_config_default")
    );
    assert!(card.actions.iter().any(|action| {
        action.kind == "symbol_graph"
            && action.query
                == format!(
                    "symbols node_id:{} direction:out edge_limit:300",
                    function.0
                )
    }));

    let file_card = node_card(
        &graph,
        Some(&root),
        NodeCardRequest {
            node_id: file,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .unwrap()
    .expect("expected file node card");
    assert_eq!(file_card.context.node.id, file);
    assert_eq!(file_card.total_insights, 3);
    assert_eq!(
        file_card.insight_summary.by_kind.get("orphan_function"),
        Some(&1)
    );
    assert_eq!(
        file_card
            .insight_summary
            .by_kind
            .get("potential_error_flow"),
        Some(&1)
    );
    assert_eq!(
        file_card
            .insight_summary
            .by_kind
            .get("sensitive_config_default"),
        Some(&1)
    );
    assert!(
        file_card
            .insights
            .iter()
            .any(|insight| insight.kind == "orphan_function")
    );
    assert!(
        file_card
            .insights
            .iter()
            .any(|insight| insight.kind == "potential_error_flow")
    );
    assert!(
        file_card
            .insights
            .iter()
            .any(|insight| insight.kind == "sensitive_config_default")
    );
    assert_eq!(
        file_card.source.as_ref().map(|source| source.path.as_str()),
        Some("src/main.rs")
    );
    assert!(
        file_card
            .source
            .as_ref()
            .unwrap()
            .lines
            .iter()
            .any(|line| !line.highlight && line.text.contains("fn main"))
    );
    assert!(file_card.actions.iter().any(|action| {
        action.kind == "file_graph"
            && action.query == "files path:src/main.rs direction:out edge_limit:300"
    }));
    let file_summary = file_card
        .file_summary
        .as_ref()
        .expect("expected file summary");
    assert_eq!(file_summary.contained_nodes, 1);
    assert_eq!(file_summary.code_symbols, 1);
    assert_eq!(file_summary.trace_edges, 3);
    assert_eq!(file_summary.calls, 1);
    assert_eq!(file_summary.unresolved_calls, 1);
    assert_eq!(file_summary.environment_reads, 1);
    assert_eq!(file_summary.error_facts, 1);
    assert_eq!(file_summary.contained_kinds.get("function"), Some(&1));
    assert_eq!(file_summary.trace_edge_kinds.get("calls"), Some(&1));
    assert_eq!(
        file_summary.trace_edge_kinds.get("reads_environment"),
        Some(&1)
    );
    assert_eq!(file_summary.trace_edge_kinds.get("may_error"), Some(&1));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn node_card_suggests_focused_graph_actions() {
    let mut graph = CodeGraph::new("repo");
    let mut dependency_metadata = BTreeMap::new();
    dependency_metadata.insert("item_kind".to_string(), "dependency".to_string());
    dependency_metadata.insert("package_id".to_string(), "cargo:serde".to_string());
    let dependency = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "serde",
        None,
        dependency_metadata,
    );
    let config = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    let mut error_metadata = BTreeMap::new();
    error_metadata.insert("item_kind".to_string(), "error".to_string());
    let error = graph.add_node_with_metadata(NodeKind::Unknown, "panic", None, error_metadata);
    let document = graph.add_node_with_metadata(
        NodeKind::File,
        "docs/adr/0001-runtime.md",
        None,
        BTreeMap::from([
            ("language".to_string(), "markdown".to_string()),
            ("item_kind".to_string(), "document".to_string()),
            ("document_kind".to_string(), "adr".to_string()),
        ]),
    );

    let dependency_card = node_card(
        &graph,
        None,
        NodeCardRequest {
            node_id: dependency,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .unwrap()
    .expect("expected dependency card");
    assert!(dependency_card.actions.iter().any(|action| {
        action.kind == "package_graph"
            && action.query == format!("packages node_id:{} edge_limit:300", dependency.0)
    }));

    let config_card = node_card(
        &graph,
        None,
        NodeCardRequest {
            node_id: config,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .unwrap()
    .expect("expected config card");
    assert!(config_card.actions.iter().any(|action| {
        action.kind == "config_graph"
            && action.query == format!("configs node_id:{} depth:6", config.0)
    }));

    let error_card = node_card(
        &graph,
        None,
        NodeCardRequest {
            node_id: error,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .unwrap()
    .expect("expected error card");
    assert!(error_card.actions.iter().any(|action| {
        action.kind == "error_graph"
            && action.query == format!("errors node_id:{} depth:6", error.0)
    }));

    let document_card = node_card(
        &graph,
        None,
        NodeCardRequest {
            node_id: document,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .unwrap()
    .expect("expected document card");
    assert!(document_card.actions.iter().any(|action| {
        action.kind == "document_graph"
            && action.query == format!("docs node_id:{} edge_limit:300", document.0)
    }));
}

#[test]
fn exports_dot_and_ndjson() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);

    let dot = export_dot(&graph);
    assert!(dot.starts_with("digraph CodeGraph"));
    assert!(dot.contains("main"));
    assert!(dot.contains("contains"));

    let ndjson = export_ndjson(&graph).unwrap();
    let records: Vec<_> = ndjson
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect();
    assert_eq!(records.len(), 4);
    assert_eq!(records[0]["record_type"], "graph");
    assert_eq!(records[1]["record_type"], "node");
    assert_eq!(records[3]["record_type"], "edge");
}

#[test]
fn exports_graphml_with_attributes_and_escaping() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/<main> & \"other\".rs",
        Some(SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 1,
            start_column: 0,
            end_line: 1,
            end_column: 0,
        }),
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge_with_metadata(
        file,
        main,
        EdgeKind::Contains,
        Confidence::Exact,
        BTreeMap::from([
            ("relation".to_string(), "declares".to_string()),
            ("source".to_string(), "parser".to_string()),
        ]),
    );

    let graphml = export_graphml(&graph);
    assert!(graphml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(graphml.contains("<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">"));
    assert!(graphml.contains("<graph id=\"codegraph\" edgedefault=\"directed\">"));
    assert!(
        graphml.contains("src/&lt;main&gt; &amp; &quot;other&quot;.rs"),
        "labels must be XML-escaped"
    );
    assert!(!graphml.contains("<main>"), "raw markup must not leak");
    assert!(graphml.contains("<data key=\"node_path\">src/main.rs</data>"));
    assert!(graphml.contains("<data key=\"node_language\">rust</data>"));
    assert!(graphml.contains("<data key=\"edge_confidence\">exact</data>"));
    assert!(graphml.contains("<data key=\"edge_relation\">declares</data>"));
    assert!(graphml.contains("<data key=\"edge_source\">parser</data>"));
    assert!(graphml.contains(&format!("<node id=\"{}\">", file)));
    assert!(graphml.contains(&format!("<edge source=\"{file}\" target=\"{main}\">")));
    assert!(graphml.ends_with("</graphml>\n"));
    assert_eq!(
        graphml.matches("<node id=").count(),
        graph.nodes.len(),
        "every node exported once"
    );
    assert_eq!(graphml.matches("<edge source=").count(), graph.edges.len());
}

#[test]
fn unmatched_platform_channels_warn_only_with_native_sources() {
    let mut graph = CodeGraph::new("repo");
    let channel = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "flutter method channel:com.example/native",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "platform_channel".to_string()),
            ("source".to_string(), "dart".to_string()),
            ("channel_kind".to_string(), "method".to_string()),
            ("channel_name".to_string(), "com.example/native".to_string()),
        ]),
    );
    graph.add_edge(graph.root, channel, EdgeKind::Contains, Confidence::Exact);

    // Pure Dart package: no native sources, no warning.
    let quiet = insights(&graph);
    assert!(
        !quiet
            .insights
            .iter()
            .any(|insight| insight.kind == "unmatched_platform_channel")
    );

    // A native host file exists but registers nothing: warn.
    graph.add_node(NodeKind::File, "android/app/MainActivity.kt");
    let report = insights(&graph);
    let warning = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unmatched_platform_channel")
        .expect("unmatched channel warning");
    assert_eq!(warning.severity, InsightSeverity::Warning);
    assert!(warning.message.contains("com.example/native"));

    // A matched channel stays quiet.
    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == channel) {
        node.metadata.insert(
            "native_handler_android".to_string(),
            "android/app/MainActivity.kt".to_string(),
        );
    }
    let matched = insights(&graph);
    assert!(
        !matched
            .insights
            .iter()
            .any(|insight| insight.kind == "unmatched_platform_channel")
    );
}

#[test]
fn exports_cypher_and_falkordb_scripts_with_escaping() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/it's \"main\".rs",
        Some(SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 1,
            start_column: 0,
            end_line: 1,
            end_column: 0,
        }),
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge_with_metadata(
        file,
        main,
        EdgeKind::Calls,
        Confidence::Heuristic,
        BTreeMap::from([("source".to_string(), "parser".to_string())]),
    );

    let cypher = export_cypher(&graph);
    assert!(cypher.starts_with("// CodeGraph export for Neo4j"));
    assert!(
        cypher.contains("CREATE INDEX codegraph_node_id IF NOT EXISTS FOR (n:CodeNode) ON (n.id);")
    );
    assert_eq!(
        cypher.matches("CREATE (:CodeNode:").count(),
        graph.nodes.len()
    );
    assert!(cypher.contains(":CodeNode:Function {id:"));
    assert!(
        cypher.contains("label: 'src/it\\'s \"main\".rs'"),
        "single quotes must be escaped: {cypher}"
    );
    assert!(
        cypher.contains("CREATE (a)-[:CALLS {confidence: 'heuristic', source: 'parser'}]->(b);")
    );
    assert!(cypher.contains("MATCH (a:CodeNode {id: "));
    assert_eq!(
        cypher.matches("MATCH (a:CodeNode").count(),
        graph.edges.len()
    );

    let falkordb = export_falkordb(&graph, "codegraph demo");
    assert!(falkordb.starts_with("# CodeGraph export for FalkorDB"));
    assert!(
        falkordb.contains("GRAPH.QUERY codegraph-demo \""),
        "graph key whitespace must be sanitized"
    );
    assert!(
        falkordb.contains("CREATE INDEX FOR (n:CodeNode) ON (n.id)"),
        "falkordb index syntax must not use IF NOT EXISTS"
    );
    assert!(
        falkordb.contains("label: 'src/it\\\\'s \\\"main\\\".rs'"),
        "double quotes must be redis-escaped: {falkordb}"
    );
    assert_eq!(
        falkordb.matches("GRAPH.QUERY codegraph-demo").count(),
        graph.nodes.len() + graph.edges.len() + 1
    );
}

#[test]
fn exports_bounded_mermaid_flowchart_and_html_wrapper() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main <script>");
    let helper = graph.add_node(NodeKind::Function, "helper");
    graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);

    let mermaid = graph_mermaid(&graph, 100, 100);
    assert!(mermaid.starts_with("flowchart LR"));
    assert!(mermaid.contains("calls/heuristic"));
    assert!(
        !mermaid.contains("%% truncated"),
        "small graphs are complete"
    );

    let truncated = graph_mermaid(&graph, 2, 1);
    assert!(truncated.contains("%% truncated: showing 2 of 3 nodes and 1 of 2 edges"));
    assert_eq!(
        truncated.matches("-->").count(),
        1,
        "edge budget is enforced"
    );

    let html = export_graph_mermaid_html(&graph, 100, 100);
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>CodeGraph: repo</title>"));
    assert!(html.contains("<pre class=\"mermaid\">"));
    assert!(html.contains("Mermaid source"));
    assert!(html.contains("cdn.jsdelivr.net/npm/mermaid"));
    assert!(
        html.contains("main &lt;script&gt;"),
        "labels are HTML-escaped"
    );
    assert!(
        !html.contains("main <script>"),
        "raw markup must not leak into the page"
    );

    let sections = export_mermaid_html(
        "Callflows",
        &[
            MermaidSection {
                title: "main".to_string(),
                mermaid: "flowchart TD\n  a --> b".to_string(),
            },
            MermaidSection {
                title: "worker".to_string(),
                mermaid: "flowchart TD\n  c --> d".to_string(),
            },
        ],
    );
    assert_eq!(sections.matches("<section>").count(), 2);
    assert_eq!(sections.matches("<pre class=\"mermaid\">").count(), 2);
}

#[test]
fn trace_follows_dependency_edges() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);

    let result = trace(
        &graph,
        TraceRequest {
            start: TraceStart::Label("main".to_string()),
            max_depth: 1,
        },
    )
    .unwrap();

    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.edges.len(), 1);
    assert_eq!(
        result
            .nodes
            .iter()
            .find(|node| node.node.id == helper)
            .unwrap()
            .depth,
        1
    );
}

#[test]
fn trace_dependents_follows_incoming_dependency_edges() {
    let mut graph = CodeGraph::new("repo");
    let caller = graph.add_node(NodeKind::Function, "caller");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    graph.add_edge(caller, main, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let result = trace_dependents(
        &graph,
        TraceRequest {
            start: TraceStart::Label("settings.toml".to_string()),
            max_depth: 3,
        },
    )
    .unwrap();

    assert_eq!(result.nodes.len(), 4);
    assert_eq!(result.edges.len(), 3);
    assert_eq!(
        result
            .nodes
            .iter()
            .find(|node| node.node.id == caller)
            .unwrap()
            .depth,
        3
    );
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == helper && edge.target == config)
    );
}

#[test]
fn trace_follows_entrypoint_reference_edges() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Syntactic,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );

    let result = trace(
        &graph,
        TraceRequest {
            start: TraceStart::Label("cargo bin:demo".to_string()),
            max_depth: 1,
        },
    )
    .unwrap();

    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.edges.len(), 1);
    assert!(result.nodes.iter().any(|node| node.node.id == main));
}

#[test]
fn trace_entrypoints_returns_filtered_entrypoint_flows() {
    let mut graph = CodeGraph::new("repo");
    let cli_entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:codegraph-cli",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
    );
    let server_entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:codegraph-server",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
    );
    let cli_main = graph.add_node(NodeKind::Function, "cli_main");
    let server_main = graph.add_node(NodeKind::Function, "server_main");
    graph.add_edge(
        graph.root,
        cli_entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        graph.root,
        server_entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        cli_entrypoint,
        cli_main,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(
        server_entrypoint,
        server_main,
        EdgeKind::References,
        Confidence::Syntactic,
    );

    let report = trace_entrypoints(
        &graph,
        EntrypointTraceRequest {
            search: Some("server".to_string()),
            max_depth: 1,
            limit: 10,
        },
    );

    assert_eq!(report.total_entrypoints, 1);
    assert_eq!(report.traces.len(), 1);
    assert_eq!(report.traces[0].start.id, server_entrypoint);
    assert!(
        report.traces[0]
            .nodes
            .iter()
            .any(|node| node.node.id == server_main)
    );
    assert!(
        !report.traces[0]
            .nodes
            .iter()
            .any(|node| node.node.id == cli_main)
    );
}

#[test]
fn a_capped_flow_keeps_the_children_that_lead_somewhere() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, main, EdgeKind::Entrypoint, Confidence::Exact);
    // Written first, and going nowhere: a boundary out of the repository.
    let mut boundaries = Vec::new();
    for index in 0..4 {
        let external = graph.add_node(NodeKind::ExternalDependency, format!("fmt.Print{index}"));
        graph.add_edge(main, external, EdgeKind::Calls, Confidence::Heuristic);
        boundaries.push(external);
    }
    // Written later, and the actual program: a call with a flow of its own.
    let real_main = graph.add_node(NodeKind::Function, "realMain");
    let deeper = graph.add_node(NodeKind::Function, "runCommand");
    graph.add_edge(main, real_main, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(real_main, deeper, EdgeKind::Calls, Confidence::Heuristic);

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label("main".to_string()),
            max_depth: 4,
            block_limit: 50,
            filters: WorkflowFilters::default(),
            compact: false,
            max_fanout: Some(2),
        },
    )
    .expect("workflow report");

    let reached: Vec<_> = report.blocks.iter().map(|block| block.node.id).collect();
    assert!(
        reached.contains(&real_main),
        "a capped flow must keep the call that leads somewhere"
    );
    assert!(
        reached.contains(&deeper),
        "...and reach what lies beyond it"
    );
}

#[test]
fn entrypoints_lead_with_programs_not_ci_jobs() {
    let mut graph = CodeGraph::new("repo");
    let span = |path: &str| {
        Some(SourceSpan {
            path: path.to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 1,
        })
    };
    let mut declare = |label: &str, metadata: BTreeMap<String, String>| {
        let id = graph.add_node_with_metadata(NodeKind::Entrypoint, label, None, metadata);
        graph.add_edge(graph.root, id, EdgeKind::Entrypoint, Confidence::Exact);
        id
    };
    // Declared in graph order: CI first, exactly as the file walk produces it.
    let ci_job = declare(
        "github workflow:CI/test",
        BTreeMap::from([
            ("source".to_string(), "github-actions".to_string()),
            ("entrypoint_kind".to_string(), "workflow_job".to_string()),
        ]),
    );
    let test_binary = declare(
        "cargo bin:harness",
        BTreeMap::from([
            ("source".to_string(), "manifest".to_string()),
            ("entrypoint_kind".to_string(), "binary".to_string()),
            ("target".to_string(), "tests/harness.rs".to_string()),
        ]),
    );
    let script = declare(
        "script:scripts/release.sh",
        BTreeMap::from([
            ("source".to_string(), "shebang".to_string()),
            ("entrypoint_kind".to_string(), "script".to_string()),
        ]),
    );
    let program = declare(
        "cargo bin:app",
        BTreeMap::from([
            ("source".to_string(), "manifest".to_string()),
            ("entrypoint_kind".to_string(), "binary".to_string()),
        ]),
    );
    let route = declare(
        "route GET /health",
        BTreeMap::from([
            ("source".to_string(), "framework".to_string()),
            ("entrypoint_kind".to_string(), "route".to_string()),
        ]),
    );
    let detected_main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        span("cmd/app/main.go"),
        BTreeMap::new(),
    );
    graph.add_edge(
        graph.root,
        detected_main,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );

    let ordered: Vec<NodeId> = entrypoints(&graph)
        .into_iter()
        .map(|node| node.id)
        .collect();

    assert_eq!(
        ordered,
        vec![program, detected_main, route, script, ci_job, test_binary],
        "a declared program comes first and a test binary last"
    );
}

#[test]
fn the_entrypoint_query_answers_in_the_same_order_as_the_overview() {
    let mut graph = CodeGraph::new("repo");
    let mut declare = |label: &str, metadata: BTreeMap<String, String>| {
        let id = graph.add_node_with_metadata(NodeKind::Entrypoint, label, None, metadata);
        graph.add_edge(graph.root, id, EdgeKind::Entrypoint, Confidence::Exact);
        id
    };
    // As the file walk produces them: `.github/` first.
    let ci_job = declare(
        "github workflow:CI/test",
        BTreeMap::from([
            ("source".to_string(), "github-actions".to_string()),
            ("entrypoint_kind".to_string(), "workflow_job".to_string()),
        ]),
    );
    let route = declare(
        "route GET /api/ping",
        BTreeMap::from([
            ("source".to_string(), "framework".to_string()),
            ("entrypoint_kind".to_string(), "route".to_string()),
        ]),
    );
    // koel's `artisan` is how the program runs; a script under `ci/` is
    // how it is tested.
    let ci_script = declare(
        "script:ci/test-complete",
        BTreeMap::from([("source".to_string(), "shebang".to_string())]),
    );
    let console = declare(
        "script:artisan",
        BTreeMap::from([("source".to_string(), "shebang".to_string())]),
    );

    let answered: Vec<NodeId> = query_graph(&graph, "entrypoints")
        .expect("the query runs")
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Entrypoint)
        .map(|node| node.id)
        .collect();

    assert_eq!(
        answered,
        vec![console, route, ci_script, ci_job],
        "{answered:?}"
    );
}

#[test]
fn an_ambiguous_start_label_picks_the_declared_program() {
    let mut graph = CodeGraph::new("repo");
    let span = |path: &str| {
        Some(SourceSpan {
            path: path.to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 1,
        })
    };
    // Graph order is the file walk, so the build script comes first — which is
    // exactly what used to be picked.
    let build_script = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        span("build.rs"),
        BTreeMap::new(),
    );
    let test_main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        span("tests/harness/main.rs"),
        BTreeMap::new(),
    );
    let program = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        span("crates/app/src/main.rs"),
        BTreeMap::new(),
    );
    let declaration = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:app",
        None,
        BTreeMap::from([("source".to_string(), "manifest".to_string())]),
    );
    graph.add_edge(
        graph.root,
        declaration,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        declaration,
        program,
        EdgeKind::References,
        Confidence::Exact,
    );
    // The test binary is declared too, so being declared cannot be the only
    // thing that counts.
    let test_declaration = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:harness",
        None,
        BTreeMap::from([("source".to_string(), "manifest".to_string())]),
    );
    graph.add_edge(
        graph.root,
        test_declaration,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        test_declaration,
        test_main,
        EdgeKind::References,
        Confidence::Exact,
    );

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label("main".to_string()),
            max_depth: 3,
            block_limit: 20,
            filters: WorkflowFilters::default(),
            compact: false,
            max_fanout: None,
        },
    )
    .expect("workflow report");

    assert_eq!(
        report.start.id, program,
        "a manifest-declared, non-test `main` is what the label means"
    );
    assert_ne!(report.start.id, build_script);
    assert_ne!(report.start.id, test_main);
}

#[test]
fn workflow_builds_block_steps_with_risk_context() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
    let main = graph.add_node(NodeKind::Function, "main");
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    let env = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    let config = graph.add_node(NodeKind::Config, "config/app.toml");
    let error = graph.add_node(NodeKind::Unknown, "panic: missing config");
    let package = graph.add_node(NodeKind::ExternalDependency, "serde");
    graph.add_edge(
        graph.root,
        entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        env,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        load_config,
        config,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );
    graph.add_edge(
        load_config,
        error,
        EdgeKind::MayError,
        Confidence::Heuristic,
    );
    graph.add_edge(load_config, package, EdgeKind::DependsOn, Confidence::Exact);

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label("cargo bin:api".to_string()),
            max_depth: 3,
            block_limit: 20,
            filters: WorkflowFilters::default(),
            compact: false,
            max_fanout: None,
        },
    )
    .expect("workflow report");

    assert_eq!(report.start.id, entrypoint);
    // A workflow is a flow: the block a reader sees first is where it
    // starts, and each block after it is at least as far away.
    assert_eq!(
        report.blocks.first().map(|block| block.node.id),
        Some(entrypoint)
    );
    assert!(
        report
            .blocks
            .windows(2)
            .all(|pair| pair[0].depth <= pair[1].depth),
        "{:?}",
        report
            .blocks
            .iter()
            .map(|block| (block.depth, block.node.label.as_str()))
            .collect::<Vec<_>>()
    );
    assert_eq!(report.total_blocks, 7);
    assert_eq!(report.total_transitions, 6);
    assert!(report.blocks.iter().any(|block| {
        block.node.id == entrypoint
            && block.id == format!("wb-{}", entrypoint.0)
            && block.kind == WorkflowBlockKind::Start
            && block.depth == 0
    }));
    assert!(
        report
            .blocks
            .iter()
            .any(|block| { block.node.id == load_config && block.kind == WorkflowBlockKind::Call })
    );
    assert!(
        report.blocks.iter().any(|block| {
            block.node.id == env && block.kind == WorkflowBlockKind::EnvironmentRead
        })
    );
    assert!(
        report.blocks.iter().any(|block| {
            block.node.id == config && block.kind == WorkflowBlockKind::ConfigRead
        })
    );
    assert!(report.blocks.iter().any(|block| {
        block.node.id == error
            && block.kind == WorkflowBlockKind::Error
            && block
                .risk_refs
                .iter()
                .any(|risk| risk.kind == "potential_error_flow")
    }));
    assert!(report.blocks.iter().any(|block| {
        block.node.id == package && block.kind == WorkflowBlockKind::ExternalBoundary
    }));
    assert!(report.transitions.iter().any(|transition| {
        transition.source_node_id == load_config
            && transition.target_node_id == error
            && transition.edge.metadata.contains_key("edge_index")
            && transition
                .risk_refs
                .iter()
                .any(|risk| risk.kind == "potential_error_flow")
    }));

    let mermaid = workflow_mermaid(&report);
    assert!(mermaid.starts_with("flowchart TD"));
    assert!(mermaid.contains("start: cargo bin:api"));
    assert!(mermaid.contains("reads_environment/heuristic"));
}

#[test]
fn workflow_fanout_cap_follows_calls_into_depth() {
    // A wide node (root: 2 imports + 1 call) whose call leads deeper. Unbounded
    // breadth visits everything at shallow depth; the fan-out cap keeps the
    // highest-priority edge (the call) so the budget follows the chain to depth.
    let mut graph = CodeGraph::new("repo");
    let root = graph.add_node(NodeKind::Function, "root");
    let a = graph.add_node(NodeKind::Function, "a");
    let b = graph.add_node(NodeKind::Function, "b");
    let dep1 = graph.add_node(NodeKind::ExternalDependency, "dep1");
    let dep2 = graph.add_node(NodeKind::ExternalDependency, "dep2");
    graph.add_edge(root, dep1, EdgeKind::Imports, Confidence::Heuristic);
    graph.add_edge(root, dep2, EdgeKind::Imports, Confidence::Heuristic);
    graph.add_edge(root, a, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(a, b, EdgeKind::Calls, Confidence::Heuristic);

    let build = |max_fanout| {
        workflow(
            &graph,
            WorkflowRequest {
                start: TraceStart::NodeId(root),
                max_depth: 5,
                block_limit: 100,
                filters: WorkflowFilters::default(),
                compact: false,
                max_fanout,
            },
        )
        .expect("workflow report")
    };

    let full = build(None);
    let full_labels: BTreeSet<_> = full.blocks.iter().map(|b| b.node.label.clone()).collect();
    assert!(full_labels.contains("dep1") && full_labels.contains("dep2"));
    assert!(full_labels.contains("b"));

    let capped = build(Some(1));
    let capped_labels: BTreeSet<_> = capped.blocks.iter().map(|b| b.node.label.clone()).collect();
    assert!(
        capped_labels.contains("a") && capped_labels.contains("b"),
        "the call chain is followed into depth: {capped_labels:?}"
    );
    assert!(
        !capped_labels.contains("dep1") && !capped_labels.contains("dep2"),
        "lower-priority import edges are dropped by the fan-out cap: {capped_labels:?}"
    );
    assert!(capped.truncated, "the fan-out cap marks the flow truncated");
    let root_block = capped
        .blocks
        .iter()
        .find(|block| block.node.label == "root")
        .expect("root block");
    assert_eq!(
        root_block.truncated_children, 2,
        "root reports its two dropped downstream edges"
    );
}

#[test]
fn workflow_fanout_zero_clamps_to_narrowest_not_unbounded() {
    // max_fanout is normalized in the engine: 0 must mean "narrowest" (1), not
    // "unbounded", so every surface (HTTP/CLI/MCP) behaves identically.
    let mut graph = CodeGraph::new("repo");
    let root = graph.add_node(NodeKind::Function, "root");
    let a = graph.add_node(NodeKind::Function, "a");
    let b = graph.add_node(NodeKind::Function, "b");
    let c = graph.add_node(NodeKind::Function, "c");
    graph.add_edge(root, a, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(root, b, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(root, c, EdgeKind::Calls, Confidence::Heuristic);

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::NodeId(root),
            max_depth: 5,
            block_limit: 100,
            filters: WorkflowFilters::default(),
            compact: false,
            max_fanout: Some(0),
        },
    )
    .expect("workflow report");

    // root + exactly one followed child, not all three.
    assert_eq!(report.total_blocks, 2, "0 clamps to a fan-out of 1");
    let root_block = report
        .blocks
        .iter()
        .find(|block| block.node.label == "root")
        .expect("root block");
    assert_eq!(
        root_block.truncated_children, 2,
        "two children were trimmed"
    );
}

#[test]
fn workflow_from_file_expands_into_contained_symbols() {
    // A file node has no execution flow of its own — it only `Contains` the
    // functions it defines. A flow rooted on the file must expand into those
    // functions (and their call chains) instead of dead-ending on one block.
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/handlers.rs");
    let handler = graph.add_node(NodeKind::Function, "handle_request");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let downstream = graph.add_node(NodeKind::Function, "validate");
    graph.add_edge(file, handler, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(file, helper, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(handler, downstream, EdgeKind::Calls, Confidence::Exact);

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::NodeId(file),
            max_depth: 5,
            block_limit: 100,
            filters: WorkflowFilters::default(),
            compact: false,
            max_fanout: None,
        },
    )
    .expect("workflow report");

    let labels: BTreeSet<_> = report.blocks.iter().map(|b| b.node.label.clone()).collect();
    assert!(
        labels.contains("handle_request") && labels.contains("helper"),
        "the file expands into the functions it contains: {labels:?}"
    );
    assert!(
        labels.contains("validate"),
        "and the call chain continues from those functions into depth: {labels:?}"
    );
    assert!(
        report.total_transitions >= 3,
        "contains + call edges become transitions: {}",
        report.total_transitions
    );
}

#[test]
fn journey_skips_dangling_edge_targets_instead_of_leaving_holes() {
    // A shorter route runs through a phantom node (an edge whose target has no
    // node — reachable in a deserialized graph). The BFS must not walk it; it
    // must take the real route so the step chain stays contiguous.
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let mid = graph.add_node(NodeKind::Function, "mid");
    let target = graph.add_node(NodeKind::Function, "target");
    let phantom = codegraph_core::NodeId(9_999_999);
    // Phantom route added first so it has the lower edge index the BFS sees first.
    graph.add_edge(main, phantom, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(phantom, target, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(main, mid, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(mid, target, EdgeKind::Calls, Confidence::Exact);

    let report = journey(
        &graph,
        JourneyRequest {
            from: "main".to_string(),
            to: "target".to_string(),
            max_depth: 8,
            path_limit: 1,
        },
    )
    .expect("journey report");

    let path = &report.paths[0];
    // No step references the phantom, and the transition chain is contiguous.
    for step in &path.steps {
        assert_ne!(step.block.node.id, phantom, "phantom node leaked into path");
    }
    for window in path.steps.windows(2) {
        let prev = &window[0];
        let next = &window[1];
        let transition = next.transition.as_ref().expect("hop has a transition");
        assert_eq!(
            transition.source_node_id, prev.block.node.id,
            "step chain is contiguous (no hole)"
        );
    }
    assert!(
        path.steps.iter().any(|step| step.block.node.id == mid),
        "the real route through mid is taken"
    );
}

#[test]
fn workflow_from_file_with_edge_filter_leaves_no_orphan_blocks() {
    // With an edge_kind filter the contains expansion must obey the same filter
    // the transitions are later held to; otherwise the contains transition is
    // dropped while the expanded block stays, orphaning it. Every block other
    // than the start must have an incoming transition.
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/handlers.rs");
    let handler = graph.add_node(NodeKind::Function, "handle_request");
    let downstream = graph.add_node(NodeKind::Function, "validate");
    graph.add_edge(file, handler, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(handler, downstream, EdgeKind::Calls, Confidence::Exact);

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::NodeId(file),
            max_depth: 5,
            block_limit: 100,
            filters: WorkflowFilters {
                edge_kind: Some("calls".to_string()),
                ..WorkflowFilters::default()
            },
            compact: false,
            max_fanout: None,
        },
    )
    .expect("workflow report");

    let targets: BTreeSet<_> = report
        .transitions
        .iter()
        .map(|transition| transition.target_node_id)
        .collect();
    for block in &report.blocks {
        assert!(
            block.node.id == file || targets.contains(&block.node.id),
            "block {:?} has no incoming transition (orphan)",
            block.node.label
        );
    }
}

#[test]
fn workflow_classifies_control_flow_blocks_from_item_kind() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
    let main = graph.add_node(NodeKind::Function, "main");
    let branch = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "branch: if",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "branch".to_string()),
            ("language".to_string(), "rust".to_string()),
            ("control_kind".to_string(), "if".to_string()),
        ]),
    );
    let loop_node = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "loop: for",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "loop".to_string()),
            ("language".to_string(), "rust".to_string()),
            ("control_kind".to_string(), "for".to_string()),
        ]),
    );
    let async_node = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "async: await",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "async".to_string()),
            ("language".to_string(), "rust".to_string()),
            ("control_kind".to_string(), "await".to_string()),
        ]),
    );
    let return_node = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "return: return",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "return".to_string()),
            ("language".to_string(), "rust".to_string()),
            ("control_kind".to_string(), "return".to_string()),
        ]),
    );
    graph.add_edge(
        graph.root,
        entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(main, branch, EdgeKind::References, Confidence::Heuristic);
    graph.add_edge(main, loop_node, EdgeKind::References, Confidence::Heuristic);
    graph.add_edge(
        main,
        async_node,
        EdgeKind::References,
        Confidence::Heuristic,
    );
    graph.add_edge(
        main,
        return_node,
        EdgeKind::References,
        Confidence::Heuristic,
    );

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label("cargo bin:api".to_string()),
            max_depth: 2,
            block_limit: 20,
            filters: WorkflowFilters::default(),
            compact: false,
            max_fanout: None,
        },
    )
    .expect("workflow report");

    assert!(
        report
            .blocks
            .iter()
            .any(|block| block.node.id == branch && block.kind == WorkflowBlockKind::Branch)
    );
    assert!(
        report
            .blocks
            .iter()
            .any(|block| block.node.id == loop_node && block.kind == WorkflowBlockKind::Loop)
    );
    assert!(
        report
            .blocks
            .iter()
            .any(|block| block.node.id == async_node && block.kind == WorkflowBlockKind::Async)
    );
    assert!(
        report
            .blocks
            .iter()
            .any(|block| block.node.id == return_node && block.kind == WorkflowBlockKind::Return)
    );
}

#[test]
fn workflow_compacts_repeated_low_signal_blocks() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let helper_a = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper_a",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let helper_b = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper_b",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_edge(main, helper_a, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, helper_b, EdgeKind::Calls, Confidence::Heuristic);

    let report = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label("main".to_string()),
            max_depth: 1,
            block_limit: 20,
            filters: WorkflowFilters::default(),
            compact: true,
            max_fanout: None,
        },
    )
    .expect("workflow report");

    assert!(report.compact);
    assert_eq!(report.raw_total_blocks, 3);
    assert_eq!(report.raw_total_transitions, 2);
    assert_eq!(report.total_blocks, 2);
    assert_eq!(report.total_transitions, 1);
    let compacted = report
        .blocks
        .iter()
        .find(|block| block.compacted)
        .expect("compacted block");
    assert_eq!(compacted.compacted_count, 2);
    assert_eq!(compacted.source_node_ids, vec![helper_a, helper_b]);
    assert!(
        compacted
            .node
            .label
            .contains("2 compacted rust call blocks")
    );
    let compacted_transition = report
        .transitions
        .iter()
        .find(|transition| transition.compacted)
        .expect("compacted transition");
    assert_eq!(compacted_transition.compacted_count, 2);

    let mermaid = workflow_mermaid(&report);
    assert!(mermaid.contains("2 compacted rust call blocks"));
}

#[test]
fn workflow_filters_blocks_edges_language_and_risk() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
    let main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let load_config = graph.add_node_with_metadata(
        NodeKind::Function,
        "load_config",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let env = graph.add_node_with_metadata(
        NodeKind::Environment,
        "DATABASE_URL",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "panic: missing config",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_edge(
        graph.root,
        entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(entrypoint, main, EdgeKind::References, Confidence::Exact);
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        env,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        load_config,
        error,
        EdgeKind::MayError,
        Confidence::Heuristic,
    );

    let env_only = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label("main".to_string()),
            max_depth: 3,
            block_limit: 20,
            filters: WorkflowFilters {
                language: Some("rust".to_string()),
                block_kind: Some("environment_read".to_string()),
                ..WorkflowFilters::default()
            },
            compact: false,
            max_fanout: None,
        },
    )
    .expect("environment workflow");
    assert_eq!(
        env_only.filters.block_kind.as_deref(),
        Some("environment_read")
    );
    assert!(
        env_only
            .blocks
            .iter()
            .any(|block| block.node.id == main && block.kind == WorkflowBlockKind::Start)
    );
    assert!(
        env_only.blocks.iter().any(|block| {
            block.node.id == env && block.kind == WorkflowBlockKind::EnvironmentRead
        })
    );
    assert!(!env_only.blocks.iter().any(|block| block.node.id == error));

    let risky_errors = workflow(
        &graph,
        WorkflowRequest {
            start: TraceStart::Label("load_config".to_string()),
            max_depth: 1,
            block_limit: 20,
            filters: WorkflowFilters {
                edge_kind: Some("may_error".to_string()),
                confidence: Some("heuristic".to_string()),
                risk_severity: Some("info".to_string()),
                ..WorkflowFilters::default()
            },
            compact: false,
            max_fanout: None,
        },
    )
    .expect("risk workflow");
    assert_eq!(risky_errors.total_blocks, 2);
    assert_eq!(risky_errors.total_transitions, 1);
    assert!(risky_errors.blocks.iter().any(|block| {
        block.node.id == error
            && block.kind == WorkflowBlockKind::Error
            && block
                .risk_refs
                .iter()
                .any(|risk| risk.severity == InsightSeverity::Info)
    }));
    assert!(risky_errors.transitions.iter().all(|transition| {
        transition.edge.kind == EdgeKind::MayError
            && transition.edge.confidence == Confidence::Heuristic
            && transition
                .risk_refs
                .iter()
                .any(|risk| risk.severity == InsightSeverity::Info)
    }));
}

#[test]
fn workflow_entrypoints_returns_filtered_block_reports() {
    let mut graph = CodeGraph::new("repo");
    let api_entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:api",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
    );
    let worker_entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:worker",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
    );
    let api_main = graph.add_node(NodeKind::Function, "api_main");
    let worker_main = graph.add_node(NodeKind::Function, "worker_main");
    graph.add_edge(
        graph.root,
        api_entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        graph.root,
        worker_entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        api_entrypoint,
        api_main,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(
        worker_entrypoint,
        worker_main,
        EdgeKind::References,
        Confidence::Syntactic,
    );

    let report = workflow_entrypoints(
        &graph,
        EntrypointWorkflowRequest {
            search: Some("api".to_string()),
            entrypoint_kind: None,
            max_depth: 2,
            block_limit: 10,
            limit: 10,
            filters: WorkflowFilters::default(),
            compact: false,
            max_fanout: None,
        },
    );

    assert_eq!(report.max_depth, 2);
    assert_eq!(report.block_limit, 10);
    assert_eq!(report.total_entrypoints, 1);
    assert_eq!(report.workflows.len(), 1);
    assert_eq!(report.workflows[0].start.id, api_entrypoint);
    assert!(
        report.workflows[0]
            .blocks
            .iter()
            .any(|block| block.node.id == api_main && block.kind == WorkflowBlockKind::Reference)
    );
    assert!(
        !report.workflows[0]
            .blocks
            .iter()
            .any(|block| block.node.id == worker_main)
    );
}

#[test]
fn workflow_entrypoints_filters_by_entrypoint_kind() {
    let mut graph = CodeGraph::new("repo");
    let route_entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "GET /health",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "route".to_string())]),
    );
    let make_entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "make build",
        None,
        BTreeMap::from([
            ("entrypoint_kind".to_string(), "make_target".to_string()),
            ("item_kind".to_string(), "makefile_target".to_string()),
        ]),
    );
    let route_handler = graph.add_node(NodeKind::Function, "health_handler");
    let build_script = graph.add_node(NodeKind::File, "scripts/build.sh");
    graph.add_edge(
        graph.root,
        route_entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        graph.root,
        make_entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        route_entrypoint,
        route_handler,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(
        make_entrypoint,
        build_script,
        EdgeKind::References,
        Confidence::Syntactic,
    );

    let request = |entrypoint_kind: &str| EntrypointWorkflowRequest {
        search: None,
        entrypoint_kind: Some(entrypoint_kind.to_string()),
        max_depth: 2,
        block_limit: 10,
        limit: 10,
        filters: WorkflowFilters::default(),
        compact: false,
        max_fanout: None,
    };

    let route_report = workflow_entrypoints(&graph, request("Route"));
    assert_eq!(route_report.entrypoint_kind.as_deref(), Some("route"));
    assert_eq!(route_report.total_entrypoints, 1);
    assert_eq!(route_report.workflows.len(), 1);
    assert_eq!(route_report.workflows[0].start.id, route_entrypoint);

    let make_report = workflow_entrypoints(&graph, request("make_target"));
    assert_eq!(make_report.total_entrypoints, 1);
    assert_eq!(make_report.workflows[0].start.id, make_entrypoint);

    let item_kind_report = workflow_entrypoints(&graph, request("makefile_target"));
    assert_eq!(item_kind_report.total_entrypoints, 0);

    let none_report = workflow_entrypoints(&graph, request("service"));
    assert_eq!(none_report.total_entrypoints, 0);
    assert!(none_report.workflows.is_empty());
}

#[test]
fn journey_builds_step_numbered_chain_between_labels() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:api",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    let branch = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "branch: if",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "branch".to_string()),
            ("control_kind".to_string(), "if".to_string()),
        ]),
    );
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    let unrelated = graph.add_node(NodeKind::Function, "unrelated");
    graph.add_edge(
        graph.root,
        entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(main, unrelated, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, branch, EdgeKind::References, Confidence::Heuristic);
    graph.add_edge(
        branch,
        load_config,
        EdgeKind::References,
        Confidence::Heuristic,
    );

    let report = journey(
        &graph,
        JourneyRequest {
            from: "cargo bin:api".to_string(),
            to: "load_config".to_string(),
            max_depth: 8,
            path_limit: 0,
        },
    )
    .expect("journey report");

    assert_eq!(report.from.id, entrypoint);
    assert_eq!(report.to.id, load_config);
    assert_eq!(report.total_paths, 1);
    let path = &report.paths[0];
    assert_eq!(path.total_steps, 4);
    assert_eq!(
        path.steps.iter().map(|step| step.step).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(path.steps[0].block.kind, WorkflowBlockKind::Start);
    assert!(path.steps[0].transition.is_none());
    assert_eq!(path.steps[2].block.kind, WorkflowBlockKind::Branch);
    assert_eq!(path.steps[3].block.node.id, load_config);
    for step in &path.steps[1..] {
        let transition = step.transition.as_ref().expect("chained transition");
        assert_eq!(transition.target_node_id, step.block.node.id);
        assert!(
            transition
                .edge
                .metadata
                .get("edge_index")
                .is_some_and(|value| value.parse::<usize>().is_ok())
        );
    }
    assert!(
        !path
            .steps
            .iter()
            .any(|step| step.block.node.id == unrelated)
    );
    assert_eq!(report.schema, JOURNEY_SCHEMA);
    assert_eq!(
        report.suggested_commands,
        vec![
            format!("codegraph impact {load_config} ."),
            format!("codegraph refactor-context {load_config} . --from {entrypoint}"),
            format!("codegraph node-card . --node-id {load_config}"),
        ]
    );
}

#[test]
fn journey_ranks_alternative_paths_and_explains_hops() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let via_exact = graph.add_node(NodeKind::Function, "via_exact");
    let target = graph.add_node(NodeKind::Function, "load_config");
    graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);
    // Short but heuristic route: main -> load_config.
    graph.add_edge(main, target, EdgeKind::Calls, Confidence::Heuristic);
    // Longer but exact route: main -> via_exact -> load_config.
    graph.add_edge(main, via_exact, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(via_exact, target, EdgeKind::Calls, Confidence::Exact);
    // Cycle back into the flow: load_config -> main.
    graph.add_edge(target, main, EdgeKind::Calls, Confidence::Exact);

    let report = journey(
        &graph,
        JourneyRequest {
            from: "main".to_string(),
            to: "load_config".to_string(),
            max_depth: 8,
            path_limit: 3,
        },
    )
    .expect("journey report");

    assert_eq!(report.total_paths, 2);
    assert_eq!(
        report
            .paths
            .iter()
            .map(|path| path.rank)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let best = &report.paths[0];
    let alternative = &report.paths[1];
    assert_eq!(best.total_steps, 3, "exact route should rank first");
    assert_eq!(best.confidence_score, 0);
    assert_eq!(best.lowest_confidence.as_deref(), Some("exact"));
    assert!(
        best.steps
            .iter()
            .any(|step| step.block.node.id == via_exact)
    );
    assert_eq!(alternative.total_steps, 2);
    assert_eq!(alternative.lowest_confidence.as_deref(), Some("heuristic"));
    assert!(alternative.confidence_score > best.confidence_score);

    for path in &report.paths {
        for step in &path.steps[1..] {
            let explanation = step.explanation.as_ref().expect("hop explanation");
            assert!(!explanation.confidence.is_empty());
            assert!(!explanation.confidence_note.is_empty());
            assert!(explanation.summary.contains("calls edge"));
            assert!(explanation.summary.contains(&explanation.confidence));
        }
        assert!(path.steps[0].explanation.is_none());
    }

    assert_eq!(best.risk_summary.fragile_transitions, 0);
    assert_eq!(best.risk_summary.low_confidence_hops, 0);
    let fragile_step = alternative
        .steps
        .iter()
        .find(|step| step.fragile)
        .expect("heuristic hop should be fragile");
    assert_eq!(
        fragile_step.fragile_reasons,
        vec!["low_confidence_edge".to_string()]
    );
    assert_eq!(alternative.risk_summary.fragile_transitions, 1);
    assert_eq!(alternative.risk_summary.low_confidence_hops, 1);
    assert!(
        best.risk_summary.cycle_back_edges >= 1,
        "load_config -> main back edge should count as a cycle crossing the flow"
    );
    assert!(alternative.risk_summary.cycle_back_edges >= 1);
}

#[test]
fn component_dependencies_group_by_area_package_and_language() {
    let mut graph = CodeGraph::new("repo");
    let app_file = graph.add_node_with_metadata(
        NodeKind::File,
        "crates/app/src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let lib_file = graph.add_node_with_metadata(
        NodeKind::File,
        "crates/lib/src/lib.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let web_file = graph.add_node_with_metadata(
        NodeKind::File,
        "web/static/app.js",
        None,
        BTreeMap::from([("language".to_string(), "javascript".to_string())]),
    );
    let target = graph.add_node_with_metadata(
        NodeKind::Function,
        "load_config",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let caller = graph.add_node_with_metadata(
        NodeKind::Function,
        "handler",
        None,
        BTreeMap::from([("language".to_string(), "javascript".to_string())]),
    );
    let callee = graph.add_node_with_metadata(
        NodeKind::Function,
        "read_file",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let package = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "serde",
        None,
        BTreeMap::from([("package_id".to_string(), "cargo:serde".to_string())]),
    );
    graph.add_edge(graph.root, app_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, lib_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, web_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(app_file, target, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(web_file, caller, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(lib_file, callee, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(caller, target, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(target, callee, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(target, package, EdgeKind::DependsOn, Confidence::Exact);

    let report = component_dependencies(
        &graph,
        ComponentDependencyRequest {
            target: "load_config".to_string(),
            group_limit: 25,
            edge_limit: 10,
        },
    )
    .expect("component dependency report");

    assert_eq!(report.target.label, "load_config");
    // `crates/` only groups crates, so the areas are the crates themselves
    // rather than one bucket holding the whole repository.
    assert_eq!(report.area.as_deref(), Some("crates/app"));
    assert_eq!(report.total_incoming, 1);
    assert_eq!(report.total_outgoing, 2);
    let web_area = report
        .areas
        .iter()
        .find(|group| group.key == "web")
        .expect("web area group");
    assert_eq!(web_area.incoming, 1);
    assert_eq!(web_area.outgoing, 0);
    assert_eq!(web_area.confidence_counts.get("heuristic"), Some(&1));
    let crates_area = report
        .areas
        .iter()
        .find(|group| group.key == "crates/lib")
        .expect("crates/lib area group");
    assert_eq!(crates_area.outgoing, 1);
    let package_group = report
        .packages
        .iter()
        .find(|group| group.key == "cargo:serde")
        .expect("package group");
    assert_eq!(package_group.outgoing, 1);
    assert!(!package_group.sample_edge_indexes.is_empty());
    let js_language = report
        .languages
        .iter()
        .find(|group| group.key == "javascript")
        .expect("javascript language group");
    assert_eq!(js_language.incoming, 1);

    let missing = component_dependencies(
        &graph,
        ComponentDependencyRequest {
            target: "ghost".to_string(),
            group_limit: 25,
            edge_limit: 10,
        },
    );
    assert!(missing.is_err());
}

#[test]
fn refactor_context_bundles_impact_dependencies_journey_and_risks() {
    let mut graph = CodeGraph::new("repo");
    let src_file = graph.add_node(NodeKind::File, "crates/app/src/lib.rs");
    let entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:app",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "binary".to_string())]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    let target = graph.add_node(NodeKind::Function, "load_config");
    graph.add_edge(graph.root, src_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(
        graph.root,
        entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(src_file, main, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(src_file, target, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(main, target, EdgeKind::Calls, Confidence::Heuristic);

    let bundle = refactor_context(
        &graph,
        RefactorContextRequest {
            target: "load_config".to_string(),
            from: Some("cargo bin:app".to_string()),
            max_depth: 8,
            path_limit: 2,
            dependent_limit: 50,
            risk_limit: 25,
        },
    )
    .expect("refactor context bundle");

    assert_eq!(bundle.schema, REFACTOR_CONTEXT_SCHEMA);
    assert_eq!(bundle.target.label, "load_config");
    assert!(bundle.impact.total_dependents >= 2);
    assert_eq!(bundle.dependencies.target.label, "load_config");
    let journey_report = bundle.journey.as_ref().expect("journey included");
    assert_eq!(journey_report.total_paths, 1);
    assert!(
        bundle
            .risks
            .iter()
            .all(|insight| insight
                .nodes
                .iter()
                .any(|node_id| *node_id == bundle.target.id
                    || bundle
                        .impact
                        .dependents
                        .iter()
                        .any(|dependent| dependent.node.id == *node_id)))
    );
    assert!(bundle.target_source.is_none());

    let without_journey = refactor_context(
        &graph,
        RefactorContextRequest {
            target: "load_config".to_string(),
            from: None,
            max_depth: 8,
            path_limit: 2,
            dependent_limit: 50,
            risk_limit: 25,
        },
    )
    .expect("bundle without journey");
    assert!(without_journey.journey.is_none());

    let missing = refactor_context(
        &graph,
        RefactorContextRequest {
            target: "ghost".to_string(),
            from: None,
            max_depth: 8,
            path_limit: 2,
            dependent_limit: 50,
            risk_limit: 25,
        },
    );
    assert!(missing.is_err());
}

#[test]
fn seams_rank_safe_and_needed_boundaries() {
    let mut graph = CodeGraph::new("repo");
    let web_file = graph.add_node(NodeKind::File, "web/static/app.js");
    let core_file = graph.add_node(NodeKind::File, "crates/core/src/lib.rs");
    let docs_file = graph.add_node(NodeKind::File, "docs/ARCHITECTURE.md");
    let handler = graph.add_node(NodeKind::Function, "handler");
    let alpha = graph.add_node(NodeKind::Function, "alpha");
    let beta = graph.add_node(NodeKind::Function, "beta");
    let doc_section = graph.add_node(NodeKind::Module, "architecture#overview");
    graph.add_edge(graph.root, web_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, core_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, docs_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(web_file, handler, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(core_file, alpha, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(core_file, beta, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(
        docs_file,
        doc_section,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    // Tangled boundary: web -> crates with several heuristic edges.
    graph.add_edge(handler, alpha, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(handler, beta, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(handler, alpha, EdgeKind::References, Confidence::Heuristic);
    // Thin, well-declared boundary: docs -> crates with one exact reference.
    graph.add_edge(doc_section, alpha, EdgeKind::References, Confidence::Exact);

    let report = seams(
        &graph,
        SeamRequest {
            limit: 10,
            edge_limit: 10,
        },
    );

    assert_eq!(report.total_pairs, 2);
    assert!(!report.truncated);
    let safest = &report.safest[0];
    assert_eq!(
        (safest.source_area.as_str(), safest.target_area.as_str()),
        ("docs", "crates")
    );
    assert_eq!(safest.edge_count, 1);
    assert_eq!(safest.low_confidence_edges, 0);
    let needed = &report.most_needed[0];
    assert_eq!(
        (needed.source_area.as_str(), needed.target_area.as_str()),
        ("web", "crates")
    );
    assert_eq!(needed.edge_count, 3);
    assert_eq!(needed.low_confidence_edges, 3);
    assert!(needed.friction_score > safest.friction_score);
    assert!(!needed.sample_edge_indexes.is_empty());
}

#[test]
fn impact_reports_blast_radius_with_risk_weighted_score() {
    let mut graph = CodeGraph::new("repo");
    let src_file = graph.add_node(NodeKind::File, "crates/app/src/lib.rs");
    let test_file = graph.add_node(NodeKind::File, "crates/app/tests/api_test.rs");
    let target = graph.add_node(NodeKind::Function, "load_config");
    let caller = graph.add_node(NodeKind::Function, "handler");
    let test_caller = graph.add_node_with_metadata(
        NodeKind::Function,
        "config_roundtrip_test",
        Some(codegraph_core::SourceSpan {
            path: "crates/app/tests/api_test.rs".to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 3,
            end_column: 1,
        }),
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "GET /config",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "route".to_string())]),
    );
    let unrelated = graph.add_node(NodeKind::Function, "unrelated");
    graph.add_edge(graph.root, src_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, test_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(src_file, target, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(src_file, caller, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(src_file, unrelated, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(caller, target, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(test_caller, target, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        entrypoint,
        caller,
        EdgeKind::References,
        Confidence::Syntactic,
    );

    let report = impact(
        &graph,
        ImpactRequest {
            target: "load_config".to_string(),
            max_depth: 8,
            limit: 100,
        },
    )
    .expect("impact report");

    assert_eq!(report.target.label, "load_config");
    assert!(report.risks_evaluated);
    assert_eq!(report.total_dependents, 3);
    assert_eq!(report.affected_entrypoints.len(), 1);
    assert_eq!(
        report.affected_entrypoints[0].entrypoint_kind.as_deref(),
        Some("route")
    );
    assert_eq!(report.affected_routes, 1);
    assert_eq!(report.affected_tests, 1);
    assert!(
        report
            .dependents
            .iter()
            .any(|dependent| dependent.node.id == test_caller && dependent.is_test)
    );
    assert!(
        !report
            .dependents
            .iter()
            .any(|dependent| dependent.node.id == unrelated)
    );
    assert!(report.impact_score > report.total_dependents + 5);
    assert_eq!(report.dependents[0].distance, 1);
    assert_eq!(report.schema, IMPACT_SCHEMA);
    assert_eq!(
        report.suggested_commands,
        vec![
            format!("codegraph node-card . --node-id {target}"),
            format!("codegraph journey --from {entrypoint} --to {target} ."),
            format!("codegraph refactor-context {target} . --from {entrypoint}"),
        ]
    );

    let fast = impact_fast(
        &graph,
        ImpactRequest {
            target: "load_config".to_string(),
            max_depth: 8,
            limit: 100,
        },
    )
    .expect("fast impact report");
    assert!(!fast.risks_evaluated);
    assert_eq!(fast.total_dependents, report.total_dependents);
    assert_eq!(fast.affected_tests, report.affected_tests);

    let missing = impact(
        &graph,
        ImpactRequest {
            target: "ghost".to_string(),
            max_depth: 8,
            limit: 100,
        },
    );
    assert!(missing.is_err());
}

#[test]
fn component_contract_lists_exact_cross_area_edges() {
    let mut graph = CodeGraph::new("repo");
    let web_file = graph.add_node(NodeKind::File, "web/static/app.js");
    let core_file = graph.add_node(NodeKind::File, "crates/core/src/lib.rs");
    let caller = graph.add_node(NodeKind::Function, "handler");
    let callee = graph.add_node(NodeKind::Function, "load_config");
    graph.add_edge(graph.root, web_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, core_file, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(web_file, caller, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(core_file, callee, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(caller, callee, EdgeKind::Calls, Confidence::Heuristic);

    let report = component_contract(
        &graph,
        ComponentContractRequest {
            source: "web".to_string(),
            target: "crates".to_string(),
            edge_limit: 100,
        },
    )
    .expect("component contract report");

    assert_eq!(report.source_area, "web");
    assert_eq!(report.target_area, "crates");
    assert_eq!(report.total_edges, 1);
    assert!(!report.truncated);
    assert_eq!(report.edge_kinds.get("calls"), Some(&1));
    assert_eq!(report.confidence_counts.get("heuristic"), Some(&1));
    let edge = &report.edges[0];
    assert_eq!(edge.source_label, "handler");
    assert_eq!(edge.target_label, "load_config");
    assert!(
        edge.edge
            .metadata
            .get("edge_index")
            .is_some_and(|value| value == &edge.edge_index.to_string())
    );

    let unknown = component_contract(
        &graph,
        ComponentContractRequest {
            source: "ghost".to_string(),
            target: "crates".to_string(),
            edge_limit: 100,
        },
    );
    assert!(unknown.is_err());
    assert!(
        unknown
            .unwrap_err()
            .to_string()
            .contains("did not match any architecture area")
    );
}

#[test]
fn journey_reports_unresolved_endpoints_and_missing_paths() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let isolated = graph.add_node(NodeKind::Function, "isolated");
    graph.add_edge(graph.root, main, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(graph.root, isolated, EdgeKind::Contains, Confidence::Exact);

    let missing_from = journey(
        &graph,
        JourneyRequest {
            from: "ghost".to_string(),
            to: "main".to_string(),
            max_depth: 4,
            path_limit: 0,
        },
    );
    assert!(missing_from.is_err());

    let no_path = journey(
        &graph,
        JourneyRequest {
            from: "main".to_string(),
            to: "isolated".to_string(),
            max_depth: 4,
            path_limit: 0,
        },
    )
    .expect("journey report without path");
    assert_eq!(no_path.total_paths, 0);
    assert!(no_path.paths.is_empty());

    let same = journey(
        &graph,
        JourneyRequest {
            from: "main".to_string(),
            to: "main".to_string(),
            max_depth: 4,
            path_limit: 0,
        },
    )
    .expect("same-node journey");
    assert_eq!(same.total_paths, 1);
    assert_eq!(same.paths[0].total_steps, 1);
    assert_eq!(same.paths[0].steps[0].block.kind, WorkflowBlockKind::Start);
}

#[test]
fn workflow_query_builds_reports_from_query_result_nodes() {
    let mut graph = CodeGraph::new("repo");
    let api = graph.add_node_with_metadata(
        NodeKind::Function,
        "api_main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let worker = graph.add_node_with_metadata(
        NodeKind::Function,
        "worker_main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let api_helper = graph.add_node(NodeKind::Function, "api_helper");
    let worker_helper = graph.add_node(NodeKind::Function, "worker_helper");
    graph.add_edge(api, api_helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        worker,
        worker_helper,
        EdgeKind::Calls,
        Confidence::Heuristic,
    );

    let report = workflow_query(
        &graph,
        WorkflowQueryRequest {
            query: "nodes kind:function search:main".to_string(),
            max_depth: 2,
            block_limit: 20,
            limit: 1,
            filters: WorkflowFilters {
                edge_kind: Some("calls".to_string()),
                confidence: Some("heuristic".to_string()),
                ..WorkflowFilters::default()
            },
            compact: false,
            max_fanout: None,
        },
    )
    .expect("workflow query report");

    assert_eq!(report.query, "nodes kind:function search:main");
    assert_eq!(report.max_depth, 2);
    assert_eq!(report.block_limit, 20);
    assert_eq!(report.filters.edge_kind.as_deref(), Some("calls"));
    assert_eq!(report.total_query_nodes, 2);
    assert_eq!(report.total_candidates, 2);
    assert_eq!(report.workflows.len(), 1);
    assert!(report.truncated);
    assert_eq!(report.workflows[0].start.id, api);
    assert!(
        report.workflows[0]
            .blocks
            .iter()
            .any(|block| block.node.id == api_helper)
    );
}

#[test]
fn query_filters_nodes_by_kind_label_and_metadata() {
    let mut graph = CodeGraph::new("repo");
    let mut metadata = BTreeMap::new();
    metadata.insert("language".to_string(), "rust".to_string());
    graph.add_node_with_metadata(NodeKind::Function, "load_config", None, metadata);
    graph.add_node(NodeKind::Function, "render");
    graph.add_node(NodeKind::File, "src/main.rs");

    let result = query_graph(
        &graph,
        "nodes kind:function label:load metadata.language:rust",
    )
    .unwrap();

    assert_eq!(result.total_nodes, 1);
    assert_eq!(result.nodes[0].label, "load_config");
    assert!(result.edges.is_empty());
}

#[test]
fn query_annotations_returns_annotated_node_context() {
    let mut graph = CodeGraph::new("repo");
    let mut payment_metadata = BTreeMap::new();
    payment_metadata.insert("language".to_string(), "rust".to_string());
    payment_metadata.insert("annotation.domain".to_string(), "payments".to_string());
    payment_metadata.insert("annotation.layer".to_string(), "service".to_string());
    let payment =
        graph.add_node_with_metadata(NodeKind::Function, "charge_card", None, payment_metadata);
    let database = graph.add_node(NodeKind::Function, "write_payment");
    let mut billing_metadata = BTreeMap::new();
    billing_metadata.insert("annotation.domain".to_string(), "billing".to_string());
    billing_metadata.insert("annotation.team".to_string(), "payments".to_string());
    graph.add_node_with_metadata(NodeKind::Function, "invoice", None, billing_metadata);
    graph.add_edge(payment, database, EdgeKind::Calls, Confidence::Heuristic);

    let result = query_graph(
        &graph,
        "annotations key:domain value:payments direction:out edge_limit:10",
    )
    .unwrap();

    assert_eq!(result.total_edges, 1);
    assert!(result.nodes.iter().any(|node| node.id == payment));
    assert!(result.nodes.iter().any(|node| node.id == database));
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == payment && edge.target == database)
    );
    assert!(!result.nodes.iter().any(|node| node.label == "invoice"));

    let exact = query_graph(&graph, "annotations annotation.domain:payments").unwrap();
    assert_eq!(exact.total_nodes, 2);
    assert!(exact.nodes.iter().any(|node| node.id == payment));

    let error = query_graph(&graph, "annotations nope:value")
        .expect_err("invalid annotations term should fail");
    assert!(
        error
            .to_string()
            .contains("unsupported annotations query term")
    );
}

#[test]
fn query_filters_edges_and_supports_calls_alias() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let other = graph.add_node(NodeKind::Function, "other");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(other, helper, EdgeKind::Calls, Confidence::Heuristic);

    let result = query_graph(&graph, "calls(function:main)").unwrap();

    assert_eq!(result.total_edges, 1);
    assert_eq!(result.edges[0].source, main);
    assert_eq!(result.edges[0].target, helper);
    assert_eq!(
        result.edges[0].metadata.get("edge_index"),
        Some(&"0".to_string())
    );
    assert_eq!(result.nodes.len(), 2);

    let by_index = query_graph(&graph, "edges edge_index:1").unwrap();
    assert_eq!(by_index.total_edges, 1);
    assert_eq!(by_index.edges[0].source, other);
    assert_eq!(by_index.edges[0].target, helper);
    assert_eq!(
        by_index.edges[0].metadata.get("edge_index"),
        Some(&"1".to_string())
    );
}

#[test]
fn query_filters_edges_by_confidence() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let manifest = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(manifest, main, EdgeKind::References, Confidence::Exact);

    let heuristic = query_graph(&graph, "edges confidence:heuristic").unwrap();
    assert_eq!(heuristic.total_edges, 1);
    assert_eq!(heuristic.edges[0].kind, EdgeKind::Calls);

    let exact_reference = query_graph(&graph, "edges kind:references confidence:exact").unwrap();
    assert_eq!(exact_reference.total_edges, 1);
    assert_eq!(exact_reference.edges[0].source, manifest);
}

#[test]
fn explain_edge_returns_provenance_for_matching_edge() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Syntactic,
        BTreeMap::from([
            ("relation".to_string(), "entrypoint_function".to_string()),
            ("resolution".to_string(), "manifest_path".to_string()),
        ]),
    );

    let explanation = explain_edge(
        &graph,
        ExplainEdgeRequest {
            edge_index: None,
            source: Some("cargo bin".to_string()),
            target: Some("main".to_string()),
            kind: Some("references".to_string()),
        },
    )
    .unwrap()
    .expect("missing explanation");

    assert_eq!(explanation.edge_index, 0);
    assert_eq!(explanation.total_matches, 1);
    assert_eq!(explanation.source.id, entrypoint);
    assert_eq!(explanation.target.id, main);
    assert!(explanation.summary.contains("references"));
    assert!(
        explanation
            .evidence
            .iter()
            .any(|item| item == "metadata.relation=entrypoint_function")
    );
    assert!(
        explanation
            .evidence
            .iter()
            .any(|item| item == "confidence=syntactic")
    );
}

#[test]
fn explain_edge_supports_edge_index_lookup() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let explanation = explain_edge(
        &graph,
        ExplainEdgeRequest {
            edge_index: Some(1),
            source: None,
            target: None,
            kind: None,
        },
    )
    .unwrap()
    .expect("missing explanation");

    assert_eq!(explanation.edge_index, 1);
    assert_eq!(explanation.edge.kind, EdgeKind::ReadsConfig);
    assert_eq!(explanation.target.label, "settings.toml");
}

#[test]
fn explain_edge_includes_related_insights() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let service = graph.add_node(NodeKind::Function, "service");
    let repository = graph.add_node(NodeKind::Function, "repository");
    graph.add_edge(main, service, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(service, repository, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(repository, main, EdgeKind::Calls, Confidence::Heuristic);

    let explanation = explain_edge(
        &graph,
        ExplainEdgeRequest {
            edge_index: Some(0),
            source: None,
            target: None,
            kind: None,
        },
    )
    .unwrap()
    .expect("missing explanation");

    assert_eq!(explanation.total_insights, 1);
    assert_eq!(
        explanation.insight_summary.by_severity.get("warning"),
        Some(&1)
    );
    assert_eq!(
        explanation.insight_summary.by_kind.get("dependency_cycle"),
        Some(&1)
    );
    assert_eq!(explanation.insights[0].kind, "dependency_cycle");
    assert!(!explanation.truncated_insights);
}

#[test]
fn query_trace_returns_focused_subgraph() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let dependency = graph.add_node(NodeKind::ExternalDependency, "serde");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, dependency, EdgeKind::DependsOn, Confidence::Exact);

    let result = query_graph(&graph, "trace label:main depth:2").unwrap();

    assert_eq!(result.total_nodes, 3);
    assert_eq!(result.total_edges, 2);
    assert!(result.nodes.iter().any(|node| node.label == "serde"));
}

#[test]
fn query_dependents_returns_reverse_dependency_subgraph() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let result = query_graph(&graph, "dependents label:settings.toml depth:2").unwrap();

    assert_eq!(result.total_nodes, 3);
    assert_eq!(result.total_edges, 2);
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == helper && edge.target == config)
    );
}

#[test]
fn query_path_returns_shortest_dependency_path() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let service = graph.add_node(NodeKind::Function, "service");
    let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    let unrelated = graph.add_node(NodeKind::Function, "unrelated");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, service, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        service,
        database_url,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        unrelated,
        database_url,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let result = query_graph(&graph, "path from:main to:DATABASE_URL depth:4").unwrap();

    assert_eq!(result.total_nodes, 4);
    assert_eq!(result.total_edges, 3);
    assert_eq!(
        result
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        vec!["main", "helper", "service", "DATABASE_URL"]
    );
    assert_eq!(result.edges[0].source, main);
    assert_eq!(result.edges[2].target, database_url);
}

#[test]
fn query_path_respects_depth_and_edge_kind() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let limited = query_graph(&graph, "path from:main to:settings.toml depth:1").unwrap();
    assert!(limited.nodes.is_empty());
    assert!(limited.truncated);

    let calls_only = query_graph(
        &graph,
        "path from:main to:settings.toml depth:3 edge_kind:calls",
    )
    .unwrap();
    assert!(calls_only.nodes.is_empty());
    assert!(!calls_only.truncated);
}

#[test]
fn query_neighbors_returns_directional_neighborhoods() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let service = graph.add_node(NodeKind::Function, "service");
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    let caller = graph.add_node(NodeKind::Function, "caller");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, service, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);
    graph.add_edge(caller, main, EdgeKind::Calls, Confidence::Heuristic);

    let outgoing = query_graph(
        &graph,
        "neighbors label:main direction:out depth:2 edge_kind:calls",
    )
    .unwrap();
    assert_eq!(outgoing.total_edges, 2);
    assert!(outgoing.nodes.iter().any(|node| node.id == main));
    assert!(outgoing.nodes.iter().any(|node| node.id == helper));
    assert!(outgoing.nodes.iter().any(|node| node.id == service));
    assert!(!outgoing.nodes.iter().any(|node| node.id == config));
    assert!(!outgoing.nodes.iter().any(|node| node.id == caller));

    let incoming = query_graph(&graph, "neighbors main direction:in").unwrap();
    assert_eq!(incoming.total_edges, 1);
    assert!(incoming.nodes.iter().any(|node| node.id == caller));
    assert!(!incoming.nodes.iter().any(|node| node.id == helper));
}

#[test]
fn query_symbols_returns_file_and_dependency_context() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/config.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let load_config = graph.add_node_with_metadata(
        NodeKind::Function,
        "load_config",
        None,
        BTreeMap::from([
            ("language".to_string(), "rust".to_string()),
            ("item_kind".to_string(), "function".to_string()),
        ]),
    );
    let helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "parse_config",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let caller = graph.add_node(NodeKind::Function, "main");
    let unrelated = graph.add_node(NodeKind::Function, "render");
    graph.add_edge(file, load_config, EdgeKind::Contains, Confidence::Syntactic);
    graph.add_edge(load_config, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(caller, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(caller, unrelated, EdgeKind::Calls, Confidence::Heuristic);

    let result = query_graph(&graph, "symbols load_config direction:out").unwrap();

    assert!(result.nodes.iter().any(|node| node.id == file));
    assert!(result.nodes.iter().any(|node| node.id == load_config));
    assert!(result.nodes.iter().any(|node| node.id == helper));
    assert!(!result.nodes.iter().any(|node| node.id == caller));
    assert!(!result.nodes.iter().any(|node| node.id == unrelated));
    assert!(result.edges.iter().any(|edge| {
        edge.source == file && edge.target == load_config && edge.kind == EdgeKind::Contains
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == load_config && edge.target == helper && edge.kind == EdgeKind::Calls
    }));
    assert_eq!(result.facets.node_kinds.get("function"), Some(&2));
    assert_eq!(result.facets.edge_kinds.get("calls"), Some(&1));

    let by_path = query_graph(&graph, "symbols path:src/config.rs").unwrap();
    assert!(by_path.nodes.iter().any(|node| node.id == load_config));

    let error =
        query_graph(&graph, "symbols nope:value").expect_err("invalid symbols term should fail");
    assert!(error.to_string().contains("unsupported symbols query term"));
}

#[test]
fn query_files_returns_structure_and_symbol_context() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/config.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let load_config = graph.add_node_with_metadata(
        NodeKind::Function,
        "load_config",
        None,
        BTreeMap::from([
            ("language".to_string(), "rust".to_string()),
            ("item_kind".to_string(), "function".to_string()),
        ]),
    );
    let helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "parse_config",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let import = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "serde::Deserialize",
        None,
        BTreeMap::from([
            ("language".to_string(), "rust".to_string()),
            ("item_kind".to_string(), "import".to_string()),
        ]),
    );
    let env = graph.add_node_with_metadata(
        NodeKind::Environment,
        "DATABASE_URL",
        None,
        BTreeMap::from([("item_kind".to_string(), "environment".to_string())]),
    );
    let caller = graph.add_node(NodeKind::Function, "main");
    let unrelated_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/render.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let unrelated = graph.add_node(NodeKind::Function, "render");
    graph.add_edge(file, load_config, EdgeKind::Contains, Confidence::Syntactic);
    graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(load_config, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        env,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(caller, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        unrelated_file,
        unrelated,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );

    let result = query_graph(
        &graph,
        "files path:src/config.rs direction:out edge_limit:20",
    )
    .unwrap();

    assert!(result.nodes.iter().any(|node| node.id == file));
    assert!(result.nodes.iter().any(|node| node.id == load_config));
    assert!(result.nodes.iter().any(|node| node.id == helper));
    assert!(result.nodes.iter().any(|node| node.id == import));
    assert!(result.nodes.iter().any(|node| node.id == env));
    assert!(!result.nodes.iter().any(|node| node.id == caller));
    assert!(!result.nodes.iter().any(|node| node.id == unrelated_file));
    assert!(!result.nodes.iter().any(|node| node.id == unrelated));
    assert!(result.edges.iter().any(|edge| {
        edge.source == file && edge.target == load_config && edge.kind == EdgeKind::Contains
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == file && edge.target == import && edge.kind == EdgeKind::Imports
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == load_config && edge.target == helper && edge.kind == EdgeKind::Calls
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == load_config && edge.target == env && edge.kind == EdgeKind::ReadsEnvironment
    }));
    assert_eq!(result.facets.node_kinds.get("file"), Some(&1));
    assert_eq!(result.facets.edge_kinds.get("contains"), Some(&1));
    assert_eq!(result.facets.edge_kinds.get("reads_environment"), Some(&1));

    let incoming = query_graph(&graph, "files path:src/config.rs direction:in").unwrap();
    assert!(incoming.nodes.iter().any(|node| node.id == caller));
    assert!(incoming.edges.iter().any(|edge| {
        edge.source == caller && edge.target == load_config && edge.kind == EdgeKind::Calls
    }));

    let by_language = query_graph(&graph, "files language:rust edge_limit:20").unwrap();
    assert!(by_language.nodes.iter().any(|node| node.id == file));

    let error =
        query_graph(&graph, "files nope:value").expect_err("invalid files term should fail");
    assert!(error.to_string().contains("unsupported files query term"));
}

#[test]
fn query_documents_returns_sections_and_code_references() {
    let mut graph = CodeGraph::new("repo");
    let doc = graph.add_node_with_metadata(
        NodeKind::File,
        "docs/adr/0001-runtime.md",
        None,
        BTreeMap::from([
            ("language".to_string(), "markdown".to_string()),
            ("item_kind".to_string(), "document".to_string()),
            ("document_kind".to_string(), "adr".to_string()),
            ("source".to_string(), "markdown".to_string()),
            ("doc_title".to_string(), "Runtime Flow".to_string()),
            ("doc_owner".to_string(), "platform-team".to_string()),
            ("doc_status".to_string(), "approved".to_string()),
            ("doc_tags".to_string(), "runtime, startup".to_string()),
        ]),
    );
    let section = graph.add_node_with_metadata(
        NodeKind::Module,
        "docs/adr/0001-runtime.md#Runtime Flow",
        Some(SourceSpan {
            path: "docs/adr/0001-runtime.md".to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 15,
        }),
        BTreeMap::from([
            ("language".to_string(), "markdown".to_string()),
            ("item_kind".to_string(), "document_section".to_string()),
            ("document_kind".to_string(), "adr".to_string()),
            ("heading".to_string(), "Runtime Flow".to_string()),
            ("anchor".to_string(), "runtime-flow".to_string()),
            ("source".to_string(), "markdown".to_string()),
        ]),
    );
    let file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let function = graph.add_node_with_metadata(
        NodeKind::Function,
        "load_config",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let other_doc = graph.add_node_with_metadata(
        NodeKind::File,
        "docs/readme.md",
        None,
        BTreeMap::from([
            ("language".to_string(), "markdown".to_string()),
            ("item_kind".to_string(), "document".to_string()),
            ("document_kind".to_string(), "markdown".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        doc,
        section,
        EdgeKind::Contains,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "document_section".to_string())]),
    );
    graph.add_edge(file, function, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge_with_metadata(
        section,
        file,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([
            ("relation".to_string(), "markdown_link".to_string()),
            ("source".to_string(), "markdown".to_string()),
            ("target".to_string(), "../../src/main.rs".to_string()),
            ("resolved_path".to_string(), "src/main.rs".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        section,
        function,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([
            (
                "relation".to_string(),
                "markdown_symbol_reference".to_string(),
            ),
            ("source".to_string(), "markdown".to_string()),
            ("symbol".to_string(), "load_config".to_string()),
        ]),
    );

    let result = query_graph(&graph, "docs document_kind:adr target:src/main.rs").unwrap();

    assert!(result.nodes.iter().any(|node| node.id == section));
    assert!(result.nodes.iter().any(|node| node.id == file));
    assert!(result.nodes.iter().any(|node| node.id == doc));
    assert!(!result.nodes.iter().any(|node| node.id == other_doc));
    assert!(result.edges.iter().any(|edge| {
        edge.source == doc
            && edge.target == section
            && edge.kind == EdgeKind::Contains
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "document_section")
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == section
            && edge.target == file
            && edge.kind == EdgeKind::References
            && edge
                .metadata
                .get("relation")
                .is_some_and(|value| value == "markdown_link")
    }));
    assert_eq!(result.facets.languages.get("markdown"), Some(&2));
    assert_eq!(result.facets.languages.get("rust"), Some(&1));
    assert_eq!(result.facets.item_kinds.get("document_section"), Some(&1));

    let by_alias = query_graph(&graph, "adr heading:Runtime edge_limit:20").unwrap();
    assert!(by_alias.nodes.iter().any(|node| node.id == section));
    assert!(by_alias.edges.iter().any(|edge| edge.target == function));

    let by_owner = query_graph(&graph, "docs owner:platform-team").unwrap();
    assert!(by_owner.nodes.iter().any(|node| node.id == doc));
    assert!(!by_owner.nodes.iter().any(|node| node.id == other_doc));
    let by_status = query_graph(&graph, "docs status:approved").unwrap();
    assert!(by_status.nodes.iter().any(|node| node.id == doc));
    let by_tag = query_graph(&graph, "docs tag:runtime").unwrap();
    assert!(by_tag.nodes.iter().any(|node| node.id == doc));

    let error =
        query_graph(&graph, "docs unsupported:value").expect_err("invalid docs term should fail");
    assert!(error.to_string().contains("unsupported docs query term"));
}

#[test]
fn query_sql_returns_schema_and_source_query_context() {
    let mut graph = CodeGraph::new("repo");
    let schema = graph.add_node_with_metadata(
        NodeKind::File,
        "db/schema.sql",
        None,
        BTreeMap::from([
            ("language".to_string(), "sql".to_string()),
            ("item_kind".to_string(), "sql_schema".to_string()),
        ]),
    );
    let users = graph.add_node_with_metadata(
        NodeKind::Type,
        "sql table:users",
        None,
        BTreeMap::from([
            ("language".to_string(), "sql".to_string()),
            ("item_kind".to_string(), "sql_table".to_string()),
            ("table_name".to_string(), "users".to_string()),
            ("table_key".to_string(), "users".to_string()),
        ]),
    );
    let user_id = graph.add_node_with_metadata(
        NodeKind::Config,
        "sql column:users.id",
        None,
        BTreeMap::from([
            ("language".to_string(), "sql".to_string()),
            ("item_kind".to_string(), "sql_column".to_string()),
            ("table_name".to_string(), "users".to_string()),
            ("column_name".to_string(), "id".to_string()),
        ]),
    );
    let rust_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/repo.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let load_users = graph.add_node_with_metadata(
        NodeKind::Function,
        "load_users",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let query = graph.add_node_with_metadata(
        NodeKind::Config,
        "sql query:src/repo.rs:4",
        None,
        BTreeMap::from([
            ("language".to_string(), "rust".to_string()),
            ("item_kind".to_string(), "app_sql_query".to_string()),
            ("operation".to_string(), "select".to_string()),
            ("tables".to_string(), "audit_log,users".to_string()),
            ("unresolved_tables".to_string(), "audit_log".to_string()),
            ("resolution".to_string(), "partial".to_string()),
        ]),
    );

    graph.add_edge_with_metadata(
        schema,
        users,
        EdgeKind::Contains,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "sql_table".to_string())]),
    );
    graph.add_edge_with_metadata(
        users,
        user_id,
        EdgeKind::Contains,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "sql_column".to_string())]),
    );
    graph.add_edge(rust_file, load_users, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge_with_metadata(
        load_users,
        query,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([("relation".to_string(), "app_sql_query".to_string())]),
    );
    graph.add_edge_with_metadata(
        query,
        users,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([
            (
                "relation".to_string(),
                "app_sql_table_reference".to_string(),
            ),
            ("operation".to_string(), "select".to_string()),
            ("table".to_string(), "users".to_string()),
        ]),
    );

    let result = query_graph(&graph, "sql table:users edge_limit:20").unwrap();

    assert!(result.nodes.iter().any(|node| node.id == users));
    assert!(result.nodes.iter().any(|node| node.id == user_id));
    assert!(result.nodes.iter().any(|node| node.id == schema));
    assert!(result.nodes.iter().any(|node| node.id == query));
    assert!(result.nodes.iter().any(|node| node.id == load_users));
    assert!(!result.nodes.iter().any(|node| node.id == rust_file));
    assert!(result.edges.iter().any(|edge| {
        edge.source == schema
            && edge.target == users
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "sql_table")
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == users
            && edge.target == user_id
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "sql_column")
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == query
            && edge.target == users
            && edge
                .metadata
                .get("relation")
                .is_some_and(|relation| relation == "app_sql_table_reference")
    }));
    assert_eq!(result.facets.item_kinds.get("sql_table"), Some(&1));
    assert_eq!(result.facets.item_kinds.get("app_sql_query"), Some(&1));

    let unresolved = query_graph(&graph, "sql unresolved:true").unwrap();
    assert!(unresolved.nodes.iter().any(|node| node.id == query));
    assert!(unresolved.nodes.iter().any(|node| node.id == users));
    assert!(unresolved.total_nodes >= 2);

    let by_operation = query_graph(&graph, "database operation:select").unwrap();
    assert!(by_operation.nodes.iter().any(|node| node.id == query));

    let error =
        query_graph(&graph, "sql unsupported:value").expect_err("invalid sql term should fail");
    assert!(error.to_string().contains("unsupported sql query term"));

    let card = node_card(
        &graph,
        None,
        NodeCardRequest {
            node_id: query,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .expect("SQL query card should not error")
    .expect("expected SQL query card");
    assert!(card.actions.iter().any(|action| {
        action.kind == "sql_graph"
            && action.query == format!("sql node_id:{} edge_limit:300", query.0)
    }));
}

#[test]
fn query_packages_returns_manifest_and_import_context() {
    let mut graph = CodeGraph::new("repo");
    let cargo_manifest = graph.add_node_with_metadata(
        NodeKind::File,
        "Cargo.toml",
        None,
        BTreeMap::from([("language".to_string(), "toml".to_string())]),
    );
    let rust_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let js_file = graph.add_node_with_metadata(
        NodeKind::File,
        "web/app.js",
        None,
        BTreeMap::from([("language".to_string(), "javascript".to_string())]),
    );
    let serde_dependency = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "serde",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "dependency".to_string()),
            ("ecosystem".to_string(), "cargo".to_string()),
            ("package_id".to_string(), "cargo:serde".to_string()),
            ("source".to_string(), "manifest".to_string()),
        ]),
    );
    let serde_import = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "use serde::Deserialize;",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "rust".to_string()),
        ]),
    );
    let express_import = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "import express from 'express';",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "javascript".to_string()),
        ]),
    );
    let serde_json_dependency = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "serde_json",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "dependency".to_string()),
            ("ecosystem".to_string(), "cargo".to_string()),
            ("package_id".to_string(), "cargo:serde_json".to_string()),
            ("source".to_string(), "manifest".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        cargo_manifest,
        serde_dependency,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "1".to_string()),
            ("source".to_string(), "manifest".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        cargo_manifest,
        serde_json_dependency,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "1".to_string()),
            ("source".to_string(), "manifest".to_string()),
        ]),
    );
    graph.add_edge(
        rust_file,
        serde_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        js_file,
        express_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let result = query_graph(&graph, "packages serde ecosystem:cargo").unwrap();

    assert_eq!(result.returned_nodes, result.nodes.len());
    assert_eq!(result.returned_edges, result.edges.len());
    assert_eq!(
        result.facets.node_kinds.get("external_dependency"),
        Some(&2)
    );
    assert_eq!(result.facets.node_kinds.get("file"), Some(&2));
    assert_eq!(result.facets.edge_kinds.get("depends_on"), Some(&1));
    assert_eq!(result.facets.edge_kinds.get("imports"), Some(&1));
    assert_eq!(result.facets.languages.get("rust"), Some(&2));
    assert!(result.nodes.iter().any(|node| node.id == cargo_manifest));
    assert!(result.nodes.iter().any(|node| node.id == rust_file));
    assert!(result.nodes.iter().any(|node| node.id == serde_dependency));
    assert!(result.nodes.iter().any(|node| node.id == serde_import));
    assert!(
        !result
            .nodes
            .iter()
            .any(|node| node.id == serde_json_dependency)
    );
    assert!(!result.nodes.iter().any(|node| node.id == express_import));
    assert!(result.edges.iter().any(|edge| {
        edge.source == cargo_manifest
            && edge.target == serde_dependency
            && edge.kind == EdgeKind::DependsOn
    }));
    assert!(result.edges.iter().any(|edge| {
        edge.source == rust_file && edge.target == serde_import && edge.kind == EdgeKind::Imports
    }));

    let path_limited = query_graph(&graph, "packages package:serde path:src").unwrap();
    assert_eq!(path_limited.total_edges, 1);
    assert!(
        path_limited
            .edges
            .iter()
            .all(|edge| edge.kind == EdgeKind::Imports)
    );

    let error = query_graph(&graph, "packages unsupported:value")
        .expect_err("invalid packages term should fail");
    assert!(
        error
            .to_string()
            .contains("unsupported packages query term")
    );
}

#[test]
fn query_unreachable_returns_source_file_focus() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let live_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let live_main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let legacy_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/legacy.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let legacy_fn = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_worker",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let test_file = graph.add_node_with_metadata(
        NodeKind::File,
        "tests/legacy_test.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let test_fn = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_test",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        live_file,
        live_main,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );
    graph.add_edge(entry, live_main, EdgeKind::References, Confidence::Exact);
    graph.add_edge(
        legacy_file,
        legacy_fn,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );
    graph.add_edge(
        test_file,
        test_fn,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );

    let result = query_graph(&graph, "unreachable language:rust").unwrap();

    assert!(result.nodes.iter().any(|node| node.id == legacy_file));
    assert!(result.nodes.iter().any(|node| node.id == legacy_fn));
    assert!(result.edges.iter().any(|edge| {
        edge.source == legacy_file && edge.target == legacy_fn && edge.kind == EdgeKind::Contains
    }));
    assert!(!result.nodes.iter().any(|node| node.id == live_file));
    assert!(!result.nodes.iter().any(|node| node.id == live_main));
    assert!(!result.nodes.iter().any(|node| node.id == test_file));
}

#[test]
fn query_unreachable_supports_general_node_scope() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let unused = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_worker",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);

    let result = query_graph(&graph, "unreachable kind:function label:legacy_worker").unwrap();

    assert_eq!(result.total_nodes, 1);
    assert_eq!(result.nodes[0].id, unused);
    assert!(result.edges.is_empty());

    let error =
        query_graph(&graph, "unreachable scope:maybe").expect_err("invalid scope should fail");
    assert!(error.to_string().contains("invalid unreachable scope"));
}

#[test]
fn query_unreachable_returns_config_and_error_flow_scopes() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let live_env = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    let live_error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "panic",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    let legacy_loader = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_loader",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let legacy_env = graph.add_node(NodeKind::Environment, "LEGACY_TOKEN");
    let legacy_worker = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_worker",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let legacy_error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "LegacyError",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
    graph.add_edge(
        main,
        live_env,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(main, live_error, EdgeKind::MayError, Confidence::Heuristic);
    graph.add_edge(
        legacy_loader,
        legacy_env,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        legacy_worker,
        legacy_error,
        EdgeKind::MayError,
        Confidence::Heuristic,
    );

    let configs = query_graph(&graph, "unreachable scope:config search:LEGACY_TOKEN").unwrap();
    assert!(configs.nodes.iter().any(|node| node.id == legacy_loader));
    assert!(configs.nodes.iter().any(|node| node.id == legacy_env));
    assert!(configs.edges.iter().any(|edge| {
        edge.source == legacy_loader
            && edge.target == legacy_env
            && edge.kind == EdgeKind::ReadsEnvironment
    }));
    assert!(!configs.nodes.iter().any(|node| node.id == main));
    assert!(!configs.nodes.iter().any(|node| node.id == live_env));

    let errors = query_graph(&graph, "unreachable scope:errors search:LegacyError").unwrap();
    assert!(errors.nodes.iter().any(|node| node.id == legacy_worker));
    assert!(errors.nodes.iter().any(|node| node.id == legacy_error));
    assert!(errors.edges.iter().any(|edge| {
        edge.source == legacy_worker
            && edge.target == legacy_error
            && edge.kind == EdgeKind::MayError
    }));
    assert!(!errors.nodes.iter().any(|node| node.id == main));
    assert!(!errors.nodes.iter().any(|node| node.id == live_error));
}

#[test]
fn query_diagnostics_returns_diagnostic_context() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let diagnostic = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "error: semantic mismatch",
        Some(SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 3,
            start_column: 9,
            end_line: 3,
            end_column: 10,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "diagnostic".to_string()),
            ("source".to_string(), "lsp".to_string()),
            ("severity".to_string(), "error".to_string()),
            ("diagnostic_source".to_string(), "rustc".to_string()),
            ("diagnostic_code".to_string(), "E0001".to_string()),
            ("message".to_string(), "semantic mismatch".to_string()),
            ("path".to_string(), "src/main.rs".to_string()),
        ]),
    );
    let warning = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "warning: style issue",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "diagnostic".to_string()),
            ("source".to_string(), "lsp".to_string()),
            ("severity".to_string(), "warning".to_string()),
            ("diagnostic_source".to_string(), "rustc".to_string()),
            ("message".to_string(), "style issue".to_string()),
            ("path".to_string(), "src/main.rs".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        file,
        diagnostic,
        EdgeKind::MayError,
        Confidence::Semantic,
        BTreeMap::from([("relation".to_string(), "diagnostic".to_string())]),
    );
    graph.add_edge_with_metadata(
        file,
        warning,
        EdgeKind::MayError,
        Confidence::Semantic,
        BTreeMap::from([("relation".to_string(), "diagnostic".to_string())]),
    );

    let result = query_graph(
        &graph,
        "diagnostics severity:error language:rust code:E0001",
    )
    .unwrap();

    assert_eq!(result.total_edges, 1);
    assert!(result.nodes.iter().any(|node| node.id == file));
    assert!(result.nodes.iter().any(|node| node.id == diagnostic));
    assert!(!result.nodes.iter().any(|node| node.id == warning));
    assert_eq!(result.edges[0].source, file);
    assert_eq!(result.edges[0].target, diagnostic);

    let error = query_graph(&graph, "diagnostics nope:value")
        .expect_err("invalid diagnostics term should fail");
    assert!(
        error
            .to_string()
            .contains("unsupported diagnostics query term")
    );
}

#[test]
fn query_insights_returns_risk_context() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let orphan = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_worker",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);

    let result = query_graph(&graph, "insights severity:info kind:orphan language:rust").unwrap();

    assert_eq!(result.total_nodes, 1);
    assert!(result.nodes.iter().any(|node| node.id == orphan));
    assert!(result.edges.is_empty());

    let by_node = query_graph(&graph, "risks node:legacy_worker").unwrap();
    assert!(by_node.nodes.iter().any(|node| node.id == orphan));

    let error =
        query_graph(&graph, "insights nope:value").expect_err("invalid insights term should fail");
    assert!(
        error
            .to_string()
            .contains("unsupported insights query term")
    );
}

#[test]
fn query_insights_returns_sensitive_config_default_context() {
    let mut graph = CodeGraph::new("repo");
    let load_config = graph.add_node_with_metadata(
        NodeKind::Function,
        "load_config",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    let token = graph.add_node_with_metadata(
        NodeKind::Environment,
        "API_TOKEN",
        None,
        BTreeMap::from([("default_value".to_string(), "local-token".to_string())]),
    );
    graph.add_edge(
        load_config,
        token,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let result = query_graph(&graph, "insights kind:sensitive_config_default").unwrap();

    assert_eq!(result.total_nodes, 2);
    assert_eq!(result.total_edges, 1);
    assert!(result.nodes.iter().any(|node| node.id == load_config));
    assert!(result.nodes.iter().any(|node| node.id == token));
    assert_eq!(result.edges[0].source, load_config);
    assert_eq!(result.edges[0].target, token);
}

#[test]
fn query_entrypoints_returns_start_context() {
    let mut graph = CodeGraph::new("repo");
    let cargo = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:api",
        None,
        BTreeMap::from([
            ("language".to_string(), "rust".to_string()),
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("entrypoint_kind".to_string(), "binary".to_string()),
        ]),
    );
    let npm = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "npm script:start",
        None,
        BTreeMap::from([
            ("language".to_string(), "javascript".to_string()),
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
        ]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, cargo, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(graph.root, npm, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge_with_metadata(
        cargo,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );

    let result = query_graph(&graph, "entrypoints language:rust").unwrap();

    assert!(result.nodes.iter().any(|node| node.id == cargo));
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(!result.nodes.iter().any(|node| node.id == npm));
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == graph.root && edge.target == cargo)
    );
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == cargo && edge.target == main)
    );

    let by_search = query_graph(&graph, "starts api").unwrap();
    assert!(by_search.nodes.iter().any(|node| node.id == cargo));

    let error = query_graph(&graph, "entrypoints nope:value")
        .expect_err("invalid entrypoints term should fail");
    assert!(
        error
            .to_string()
            .contains("unsupported entrypoints query term")
    );
}

#[test]
fn query_routes_returns_route_handler_context() {
    let mut graph = CodeGraph::new("repo");
    let route = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route GET /users",
        Some(SourceSpan {
            path: "src/server.js".to_string(),
            start_line: 7,
            start_column: 1,
            end_line: 7,
            end_column: 30,
        }),
        BTreeMap::from([
            ("language".to_string(), "javascript".to_string()),
            ("item_kind".to_string(), "framework_route".to_string()),
            ("entrypoint_kind".to_string(), "route".to_string()),
            ("framework".to_string(), "express".to_string()),
            ("method".to_string(), "GET".to_string()),
            ("path".to_string(), "/users".to_string()),
            ("handler".to_string(), "listUsers".to_string()),
        ]),
    );
    let other_route = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route POST /users",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "framework_route".to_string()),
            ("framework".to_string(), "express".to_string()),
            ("method".to_string(), "POST".to_string()),
            ("path".to_string(), "/users".to_string()),
            ("handler".to_string(), "createUser".to_string()),
        ]),
    );
    let file = graph.add_node(NodeKind::File, "src/server.js");
    let express_import = graph.add_node(NodeKind::ExternalDependency, "express");
    let handler = graph.add_node(NodeKind::Function, "listUsers");
    let load_config = graph.add_node(NodeKind::Function, "loadConfig");
    let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    graph.add_edge(graph.root, route, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        graph.root,
        other_route,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge_with_metadata(
        route,
        file,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "framework_route_file".to_string())]),
    );
    graph.add_edge(
        file,
        express_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge_with_metadata(
        route,
        handler,
        EdgeKind::References,
        Confidence::Syntactic,
        BTreeMap::from([(
            "resolution".to_string(),
            "framework_route_handler".to_string(),
        )]),
    );
    graph.add_edge(handler, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        database_url,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let result = query_graph(
        &graph,
        "routes method:GET path:/users framework:express depth:3",
    )
    .unwrap();

    assert!(result.nodes.iter().any(|node| node.id == route));
    assert!(result.nodes.iter().any(|node| node.id == file));
    assert!(!result.nodes.iter().any(|node| node.id == express_import));
    assert!(result.nodes.iter().any(|node| node.id == handler));
    assert!(result.nodes.iter().any(|node| node.id == database_url));
    assert!(!result.nodes.iter().any(|node| node.id == other_route));
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == graph.root && edge.target == route)
    );
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == route && edge.target == handler)
    );

    let by_handler = query_graph(&graph, "endpoint handler:listUsers").unwrap();
    assert!(by_handler.nodes.iter().any(|node| node.id == route));

    let by_source = query_graph(&graph, "routes source_path:src/server.js").unwrap();
    assert!(by_source.nodes.iter().any(|node| node.id == route));

    let error =
        query_graph(&graph, "routes nope:value").expect_err("invalid routes term should fail");
    assert!(error.to_string().contains("unsupported routes query term"));
}

#[test]
fn query_configs_returns_reader_and_entrypoint_context() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    let database_url = graph.add_node_with_metadata(
        NodeKind::Environment,
        "DATABASE_URL",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let helper = graph.add_node(NodeKind::Function, "helper");
    let settings = graph.add_node(NodeKind::Config, "config/app.toml");
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        database_url,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        helper,
        settings,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );

    let result = query_graph(&graph, "configs target:DATABASE depth:4").unwrap();

    assert!(result.nodes.iter().any(|node| node.id == entrypoint));
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(result.nodes.iter().any(|node| node.id == load_config));
    assert!(result.nodes.iter().any(|node| node.id == database_url));
    assert!(!result.nodes.iter().any(|node| node.id == settings));
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == load_config && edge.target == database_url)
    );

    let all_configs = query_graph(&graph, "config").unwrap();
    assert!(all_configs.nodes.iter().any(|node| node.id == settings));
    assert!(
        all_configs
            .edges
            .iter()
            .any(|edge| edge.source == helper && edge.target == settings)
    );

    let by_search = query_graph(&graph, "env DATABASE").unwrap();
    assert!(by_search.nodes.iter().any(|node| node.id == database_url));

    let error =
        query_graph(&graph, "configs nope:value").expect_err("invalid configs term should fail");
    assert!(error.to_string().contains("unsupported configs query term"));
}

#[test]
fn natural_query_maps_config_question_to_bounded_query() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        database_url,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let report = natural_query(
        &graph,
        NaturalQueryRequest {
            question: "Where is DATABASE_URL read from the environment?".to_string(),
            compact: false,
        },
    )
    .unwrap();

    assert_eq!(
        report.generated_query,
        "configs target:DATABASE_URL depth:6"
    );
    assert_eq!(report.schema, NATURAL_QUERY_SCHEMA);
    assert_eq!(
        report.cli_snippet,
        "codegraph query 'configs target:DATABASE_URL depth:6' ."
    );
    assert_eq!(report.rule, "config_or_environment");
    assert_eq!(report.confidence, "high");
    assert!(report.result.nodes.iter().any(|node| node.id == entrypoint));
    assert!(
        report
            .result
            .nodes
            .iter()
            .any(|node| node.id == load_config)
    );
    assert!(
        report
            .result
            .nodes
            .iter()
            .any(|node| node.id == database_url)
    );
}

#[test]
fn natural_query_routes_env_tokens_with_read_verbs_to_config_rule() {
    let mut graph = CodeGraph::new("repo");
    let server = graph.add_node(NodeKind::Function, "run_server");
    let token = graph.add_node(NodeKind::Environment, "CODEGRAPH_API_TOKEN");
    graph.add_edge(
        server,
        token,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    // "api" inside the identifier must not pull this into the route rule.
    let report = natural_query(
        &graph,
        NaturalQueryRequest {
            question: "Where is CODEGRAPH_API_TOKEN read?".to_string(),
            compact: false,
        },
    )
    .unwrap();
    assert_eq!(report.rule, "config_or_environment");
    assert_eq!(
        report.generated_query,
        "configs target:CODEGRAPH_API_TOKEN depth:6"
    );
    assert_eq!(report.confidence, "high");
    assert!(report.result.nodes.iter().any(|node| node.id == token));

    // Genuine route questions still reach the route rule.
    let route_report = natural_query(
        &graph,
        NaturalQueryRequest {
            question: "Which handler serves the /users endpoint?".to_string(),
            compact: false,
        },
    )
    .unwrap();
    assert_eq!(route_report.rule, "route_or_endpoint");
}

#[test]
fn a_grouped_link_keeps_a_sample_of_evidence_not_all_of_it() {
    let mut graph = CodeGraph::new("repo");
    let mut previous = None;
    for index in 0..250 {
        let node = graph.add_node_with_metadata(
            NodeKind::Function,
            format!("f{index}"),
            Some(SourceSpan {
                path: "main.go".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 2,
                end_column: 1,
            }),
            BTreeMap::from([("language".to_string(), "go".to_string())]),
        );
        if let Some(previous) = previous {
            graph.add_edge(previous, node, EdgeKind::Calls, Confidence::Heuristic);
        }
        previous = Some(node);
    }

    let report = language_dependencies(&graph, 50);
    let link = report.links.first().expect("one go->go link");

    // The count is the fact; the indexes are there to jump to a few real
    // edges. Keeping every one made this single link 1.4MB on terraform.
    assert_eq!(link.count, 249);
    assert_eq!(link.edge_indexes.len(), 100);
}

#[test]
fn an_ambiguous_query_anchor_resolves_to_the_program() {
    let mut graph = CodeGraph::new("repo");
    let span = |path: &str| {
        Some(SourceSpan {
            path: path.to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 1,
        })
    };
    // Graph order puts the CI script first, which is what `path:` and
    // `neighbors label:` used to bind to.
    let script_main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        span(".github/scripts/release.sh"),
        BTreeMap::new(),
    );
    let program_main =
        graph.add_node_with_metadata(NodeKind::Function, "main", span("main.go"), BTreeMap::new());
    let helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper",
        span("main.go"),
        BTreeMap::new(),
    );
    graph.add_edge(program_main, helper, EdgeKind::Calls, Confidence::Heuristic);

    let result = query_graph(&graph, "path from:main to:helper").expect("path query");
    let ids: Vec<_> = result.nodes.iter().map(|node| node.id).collect();
    assert_eq!(
        ids,
        vec![program_main, helper],
        "the path must start at the program, not at the CI script"
    );
    assert!(!ids.contains(&script_main));
}

#[test]
fn a_question_the_graph_answers_outright_reaches_its_query() {
    let plan = |question: &str| natural_query_plan(question).expect("plan");

    // These three fell through to a text search for one of their own words:
    // `cycles` matched nothing, `depend` matched a dependabot file, and
    // `tests` matched fifty nodes with the word in their name.
    assert_eq!(plan("Show me the cycles").rule, "dependency_cycle");
    assert_eq!(plan("Покажи циклы").rule, "dependency_cycle");
    assert_eq!(
        plan("What does this project depend on?").rule,
        "package_or_import"
    );
    assert_eq!(
        plan("How do I run the tests?").rule,
        "entrypoint_or_startup"
    );
    assert_eq!(plan("Как запустить проект?").rule, "entrypoint_or_startup");
    // A question that merely contains the word `run` is not one of them.
    assert_ne!(
        plan("What functions run in parallel?").rule,
        "entrypoint_or_startup"
    );

    // The question an agent asks before an edit.
    let impact = plan("What would break if I change `compileScript`?");
    assert_eq!(impact.rule, "reverse_dependency_or_impact");
    assert_eq!(
        impact.generated_query,
        "dependents label:compileScript depth:4"
    );
    assert_eq!(
        plan("Что сломается если изменить `compileScript`?").rule,
        "reverse_dependency_or_impact"
    );
    // What a reader means by "most coupled" is which nodes carry the most
    // edges.
    assert_eq!(
        plan("Which modules are most coupled?").rule,
        "hotspot_or_centrality"
    );

    // An entry point is written as two words as often as one.
    assert_eq!(
        plan("Where is the entry point of the CLI?").rule,
        "entrypoint_or_startup"
    );
    // What a library offers outwards is what it declares public.
    assert_eq!(plan("What are the public APIs?").rule, "public_api_surface");
    assert_eq!(
        plan("What does this library export?").generated_query,
        "symbols metadata.visibility:public limit:50"
    );
}

#[test]
fn a_symbol_named_after_a_keyword_does_not_hijack_the_question() {
    let rule = |question: &str| natural_query_plan(question).expect("plan").rule;

    // The keyword search ran over the whole question, so the name being asked
    // about decided the route: `load_config` made every question a config
    // question, `handleError` an error question, `mainLoop` a startup one.
    assert_eq!(rule("What calls `load_config`?"), "call_neighborhood");
    assert_eq!(rule("What calls `handleError`?"), "call_neighborhood");
    assert_eq!(rule("Who uses `mainLoop`?"), "reverse_dependency_or_impact");
    // The question's own words still route it.
    assert_eq!(rule("Where is `PORT` read?"), "config_or_environment");

    // ...including when the word a rule looks for is also the anchor the
    // question leaves behind: stripping it would route these nowhere.
    assert_eq!(
        rule("Where does the program start?"),
        "entrypoint_or_startup"
    );
    assert_eq!(rule("Which code is unused?"), "unreachable_or_unused");
    assert_eq!(rule("What are the hotspots?"), "hotspot_or_centrality");
}

#[test]
fn a_query_says_which_definition_of_a_shared_name_it_answered_about() {
    let mut graph = CodeGraph::new("repo");
    let first = graph.add_node_with_metadata(
        NodeKind::Function,
        "load",
        Some(SourceSpan {
            path: "src/config.rs".to_string(),
            start_line: 10,
            start_column: 1,
            end_line: 20,
            end_column: 2,
        }),
        BTreeMap::from([("item_kind".to_string(), "function".to_string())]),
    );
    let second = graph.add_node_with_metadata(
        NodeKind::Function,
        "load",
        Some(SourceSpan {
            path: "src/assets.rs".to_string(),
            start_line: 5,
            start_column: 1,
            end_line: 8,
            end_column: 2,
        }),
        BTreeMap::from([("item_kind".to_string(), "function".to_string())]),
    );
    let caller = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(caller, second, EdgeKind::Calls, Confidence::Exact);

    // A neighbors query has one start, and kong has two functions called
    // `load`: "nothing calls load" is only true of the one that was picked.
    let neighbors =
        query_graph(&graph, "neighbors label:load direction:in depth:2").expect("neighbors query");
    assert_eq!(neighbors.notes.len(), 1, "{:?}", neighbors.notes);
    assert!(
        neighbors.notes[0].contains("2 definitions are named `load`"),
        "{:?}",
        neighbors.notes
    );

    // Both ends of a path are picks of the same kind.
    let path = query_graph(&graph, "path from:load to:main depth:4").expect("path query");
    assert!(
        path.notes.iter().any(|note| note.contains("`load`")),
        "{:?}",
        path.notes
    );

    // A durable id names one node, so there is nothing to say.
    let stable = graph
        .nodes
        .iter()
        .find(|node| node.id == first)
        .and_then(|node| node.metadata.get("stable_id").cloned());
    if let Some(stable) = stable {
        let exact = query_graph(&graph, &format!("neighbors label:{stable} depth:1"))
            .expect("neighbors by id");
        assert!(exact.notes.is_empty(), "{:?}", exact.notes);
    }
}

#[test]
fn a_question_about_a_topic_is_not_a_question_about_files() {
    let plan = |question: &str| {
        let plan = natural_query_plan(question).expect("plan");
        (plan.rule, plan.generated_query)
    };

    // The noun naming the kind of thing asked about used to decide the
    // command, so "the riskiest files" ran a file search and answered with
    // files rather than with findings.
    assert_eq!(
        plan("what are the riskiest files?"),
        ("risk_or_insight".to_string(), "insights".to_string())
    );
    assert_eq!(
        plan("which files are most central?"),
        (
            "hotspot_or_centrality".to_string(),
            "hotspots min_score:3 edge_limit:300".to_string()
        )
    );
    // A schema is part of the graph: kong declares its tables inside Lua
    // migrations, and this question used to search for the word `define`.
    assert_eq!(
        plan("which tables does the database schema define?"),
        ("sql_schema".to_string(), "sql limit:50".to_string())
    );
    // A file still routes a file question.
    assert_eq!(
        plan("which files import `serde`?").0,
        "package_or_import".to_string()
    );
    // And a named file's imports are the file's, not a package's: nothing is
    // called `kong/init.lua`.
    assert_eq!(
        plan("what does kong/init.lua import?"),
        (
            "file_imports".to_string(),
            "files path:kong/init.lua direction:out edge_limit:300".to_string()
        )
    );
}

#[test]
fn a_question_ending_on_its_verb_is_still_about_its_subject() {
    let anchor = |question: &str| natural_query_guessed_anchor(question);

    // The guess took the last word, and a question ends on its verb as often
    // as on its subject: "where are plugins loaded?" is about plugins.
    assert_eq!(
        anchor("where are plugins loaded?").as_deref(),
        Some("plugins")
    );
    assert_eq!(
        anchor("where is the router configured?").as_deref(),
        Some("router")
    );
    // With nothing but a verb left, the verb is still the best guess.
    assert_eq!(anchor("what is cached?").as_deref(), Some("cached"));
}

#[test]
fn an_imperative_opening_a_question_is_not_a_symbol() {
    let query = |question: &str| natural_query_plan(question).expect("plan").generated_query;

    // `Show`/`Find` start a request; they do not name anything, and searching
    // for them returned nothing every time.
    assert!(
        !query("Show me the riskiest code").contains("Show"),
        "{}",
        query("Show me the riskiest code")
    );
    assert!(
        query("Show functions named Eval").contains("Eval"),
        "{}",
        query("Show functions named Eval")
    );
}

#[test]
fn a_call_question_points_the_way_it_was_asked() {
    let plan = |question: &str| natural_query_plan(question).expect("plan").generated_query;

    // Only "who calls" was recognised, so every other caller phrasing was
    // answered with the callee list — the opposite of the question.
    assert!(
        plan("What calls `load_config`?").contains("direction:in"),
        "{}",
        plan("What calls `load_config`?")
    );
    assert!(
        plan("Which functions call `load_config`?").contains("direction:in"),
        "{}",
        plan("Which functions call `load_config`?")
    );
    assert!(
        plan("What does `load_config` call?").contains("direction:out"),
        "{}",
        plan("What does `load_config` call?")
    );
    assert!(
        plan("functions called by `load_config`").contains("direction:out"),
        "{}",
        plan("functions called by `load_config`")
    );

    // "depends on" is a reverse-dependency question and used to fall through
    // to a plain text search.
    assert!(
        plan("What depends on `load_config`?").starts_with("dependents"),
        "{}",
        plan("What depends on `load_config`?")
    );
    // ...but the forward question must not be answered with the reverse.
    assert!(
        !plan("What does `load_config` depend on?").starts_with("dependents"),
        "{}",
        plan("What does `load_config` depend on?")
    );
}

#[test]
fn natural_query_supports_russian_call_questions() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let worker = graph.add_node(NodeKind::Function, "worker");
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(worker, load_config, EdgeKind::Calls, Confidence::Heuristic);

    let report = natural_query(
        &graph,
        NaturalQueryRequest {
            question: "Кто вызывает load_config?".to_string(),
            compact: false,
        },
    )
    .unwrap();

    assert_eq!(
        report.generated_query,
        "neighbors label:load_config direction:in depth:2 edge_kind:calls"
    );
    assert_eq!(report.rule, "call_neighborhood");
    assert!(!report.result.compact);
    assert!(report.result.nodes.iter().any(|node| node.id == main));
    assert!(report.result.nodes.iter().any(|node| node.id == worker));
    assert!(
        report
            .result
            .nodes
            .iter()
            .any(|node| node.id == load_config)
    );
}

#[test]
fn query_errors_returns_source_and_entrypoint_context() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "npm script:start");
    let main = graph.add_node(NodeKind::Function, "main");
    let load_data = graph.add_node(NodeKind::Function, "loadData");
    let error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "failed to load data",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    let helper = graph.add_node(NodeKind::Function, "helper");
    let panic = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "panic",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );
    graph.add_edge(main, load_data, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(load_data, error, EdgeKind::MayError, Confidence::Heuristic);
    graph.add_edge(helper, panic, EdgeKind::MayError, Confidence::Heuristic);

    let result = query_graph(&graph, "errors target:load depth:4").unwrap();

    assert!(result.nodes.iter().any(|node| node.id == entrypoint));
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(result.nodes.iter().any(|node| node.id == load_data));
    assert!(result.nodes.iter().any(|node| node.id == error));
    assert!(!result.nodes.iter().any(|node| node.id == panic));
    assert!(
        result
            .edges
            .iter()
            .any(|edge| edge.source == load_data && edge.target == error)
    );

    let all_errors = query_graph(&graph, "errors").unwrap();
    assert!(all_errors.nodes.iter().any(|node| node.id == panic));
    assert!(
        all_errors
            .edges
            .iter()
            .any(|edge| edge.source == helper && edge.target == panic)
    );

    let by_search = query_graph(&graph, "exceptions panic").unwrap();
    assert!(by_search.nodes.iter().any(|node| node.id == panic));

    let error_query =
        query_graph(&graph, "errors nope:value").expect_err("invalid errors term should fail");
    assert!(
        error_query
            .to_string()
            .contains("unsupported errors query term")
    );
}

#[test]
fn query_cycles_returns_dependency_cycle_context() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let service = graph.add_node_with_metadata(
        NodeKind::Function,
        "service",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let repository = graph.add_node_with_metadata(
        NodeKind::Function,
        "repository",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let helper = graph.add_node(NodeKind::Function, "helper");
    graph.add_edge(main, service, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(service, repository, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(repository, main, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, service, EdgeKind::Calls, Confidence::Heuristic);

    let result = query_graph(&graph, "cycles language:rust").unwrap();

    assert_eq!(result.total_nodes, 3);
    assert_eq!(result.total_edges, 3);
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(result.nodes.iter().any(|node| node.id == service));
    assert!(result.nodes.iter().any(|node| node.id == repository));
    assert!(!result.nodes.iter().any(|node| node.id == helper));
    assert!(result.edges.iter().all(|edge| edge.kind == EdgeKind::Calls));

    let by_label = query_graph(&graph, "cycle label:repository").unwrap();
    assert!(by_label.nodes.iter().any(|node| node.id == repository));

    let by_edge_kind = query_graph(&graph, "cycles edge_kind:calls").unwrap();
    assert_eq!(by_edge_kind.total_edges, 3);

    let error =
        query_graph(&graph, "cycles nope:value").expect_err("invalid cycles term should fail");
    assert!(error.to_string().contains("unsupported cycles query term"));
}

#[test]
fn a_query_opens_with_what_it_matched() {
    // Filtering the whole graph by an id set returned the repository node
    // first and the answer somewhere in the hundreds that followed.
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    let entry = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:demo",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "bin".to_string())]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(file, entry, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);

    let result = query_graph(&graph, "entrypoints").unwrap();
    assert_eq!(
        result.nodes.first().map(|node| node.id),
        Some(entry),
        "{:?}",
        result
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>()
    );
    // The context an entrypoint reaches still travels with it.
    assert!(result.nodes.iter().any(|node| node.id == main));
}

#[test]
fn query_hotspots_returns_high_degree_context() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let python_worker = graph.add_node_with_metadata(
        NodeKind::Function,
        "worker",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, main, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(python_worker, main, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let result = query_graph(
        &graph,
        "hotspots language:rust min_score:3 limit:1 edge_limit:3",
    )
    .unwrap();

    assert!(result.truncated);
    assert_eq!(result.total_edges, 4);
    assert_eq!(result.edges.len(), 3);
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(result.nodes.iter().any(|node| node.id == python_worker));
    // The hotspot itself comes first and says what put it there;
    // everything after it is a neighbour reached through its edges.
    assert_eq!(result.nodes[0].id, main);
    assert_eq!(
        result.nodes[0]
            .metadata
            .get("hotspot_score")
            .map(String::as_str),
        Some("4")
    );
    assert!(
        result.nodes[1..]
            .iter()
            .all(|node| !node.metadata.contains_key("hotspot_score"))
    );

    let incoming = query_graph(&graph, "hotspots label:main direction:in").unwrap();
    assert_eq!(incoming.total_edges, 2);
    assert!(incoming.edges.iter().all(|edge| edge.target == main));

    let by_edge_kind = query_graph(&graph, "hotspots edge_kind:reads_config").unwrap();
    assert_eq!(by_edge_kind.total_edges, 1);
    assert!(
        by_edge_kind
            .edges
            .iter()
            .all(|edge| edge.kind == EdgeKind::ReadsConfig)
    );

    let error =
        query_graph(&graph, "hotspots nope:value").expect_err("invalid hotspots term should fail");
    assert!(
        error
            .to_string()
            .contains("unsupported hotspots query term")
    );
}

#[test]
fn compact_query_result_collapses_low_signal_nodes() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let helper_a = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper_a",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let helper_b = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper_b",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let config = graph.add_node(NodeKind::Config, "DATABASE_URL");
    graph.add_edge(main, helper_a, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, helper_b, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let result = query_graph(&graph, "neighbors main direction:out").unwrap();
    let compacted = compact_query_result(result);

    assert!(compacted.compact);
    assert_eq!(compacted.raw_total_nodes, 4);
    assert_eq!(compacted.raw_total_edges, 3);
    assert_eq!(compacted.total_nodes, 3);
    assert_eq!(compacted.total_edges, 2);
    assert_eq!(compacted.compacted_nodes, 2);
    assert_eq!(compacted.compacted_edges, 1);
    assert!(compacted.nodes.iter().any(|node| node.id == config));
    let aggregate = compacted
        .nodes
        .iter()
        .find(|node| {
            node.metadata
                .get("compacted")
                .is_some_and(|value| value == "true")
        })
        .expect("compacted aggregate");
    assert_eq!(
        aggregate.metadata.get("compacted_count"),
        Some(&"2".to_string())
    );
    assert!(aggregate.label.contains("2 compacted rust function nodes"));
    assert!(compacted.edges.iter().any(|edge| {
        edge.metadata
            .get("compacted")
            .is_some_and(|value| value == "true")
    }));
}

#[test]
fn trace_config_returns_readers_and_entrypoint_paths() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let load_config = graph.add_node(NodeKind::Function, "load_config");
    let database_url = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );
    graph.add_edge(main, load_config, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        load_config,
        database_url,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let result = trace_config(
        &graph,
        ConfigTraceRequest {
            target: "DATABASE".to_string(),
            max_depth: 4,
            limit: 10,
        },
    );

    assert_eq!(result.total_matches, 1);
    assert_eq!(result.total_readers, 1);
    assert_eq!(result.total_paths, 1);
    assert!(!result.truncated);
    let matched = &result.matches[0];
    assert_eq!(matched.target.id, database_url);
    assert_eq!(matched.readers[0].node.id, load_config);
    assert_eq!(matched.readers[0].role, "reads");
    assert_eq!(
        matched.paths[0]
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        vec!["cargo bin:demo", "main", "load_config", "DATABASE_URL"]
    );
    assert!(matched.paths[0].reached_entrypoint);
}

#[test]
fn a_job_that_assigns_a_variable_is_not_a_reader() {
    let mut graph = CodeGraph::new("repo");
    let job = graph.add_node(NodeKind::Entrypoint, "github workflow:CI/deploy");
    let script = graph.add_node(NodeKind::File, "src/deploy.sh");
    let token = graph.add_node(NodeKind::Environment, "DEPLOY_TOKEN");
    graph.add_edge_with_metadata(
        job,
        token,
        EdgeKind::ReadsEnvironment,
        Confidence::Exact,
        BTreeMap::from([("item_kind".to_string(), "ci_environment".to_string())]),
    );
    graph.add_edge_with_metadata(
        script,
        token,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
        BTreeMap::from([("item_kind".to_string(), "environment_read".to_string())]),
    );

    let result = trace_config(
        &graph,
        ConfigTraceRequest {
            target: "DEPLOY_TOKEN".to_string(),
            max_depth: 4,
            limit: 10,
        },
    );

    let roles: Vec<(&str, &str)> = result.matches[0]
        .readers
        .iter()
        .map(|reader| (reader.node.label.as_str(), reader.role.as_str()))
        .collect();
    assert_eq!(
        roles,
        vec![
            ("github workflow:CI/deploy", "sets"),
            ("src/deploy.sh", "reads"),
        ]
    );
}

#[test]
fn trace_config_falls_back_to_direct_reader_path() {
    let mut graph = CodeGraph::new("repo");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let config = graph.add_node(NodeKind::Config, "config/app.toml");
    graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let result = trace_config(
        &graph,
        ConfigTraceRequest {
            target: "app.toml".to_string(),
            max_depth: 2,
            limit: 10,
        },
    );

    assert_eq!(result.total_matches, 1);
    assert_eq!(result.matches[0].paths.len(), 1);
    assert_eq!(
        result.matches[0].paths[0]
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        vec!["helper", "config/app.toml"]
    );
    assert!(!result.matches[0].paths[0].reached_entrypoint);
}

#[test]
fn trace_errors_returns_sources_and_entrypoint_paths() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "npm script:start");
    let main = graph.add_node(NodeKind::Function, "main");
    let load_data = graph.add_node(NodeKind::Function, "loadData");
    let error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "failed to load data",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );
    graph.add_edge(main, load_data, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(load_data, error, EdgeKind::MayError, Confidence::Heuristic);

    let result = trace_errors(
        &graph,
        ErrorTraceRequest {
            target: "load data".to_string(),
            max_depth: 4,
            limit: 10,
        },
    );

    assert_eq!(result.total_matches, 1);
    assert_eq!(result.total_sources, 1);
    assert_eq!(result.total_paths, 1);
    assert!(!result.truncated);
    let matched = &result.matches[0];
    assert_eq!(matched.error.id, error);
    assert_eq!(matched.sources[0].node.id, load_data);
    assert_eq!(
        matched.paths[0]
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "npm script:start",
            "main",
            "loadData",
            "failed to load data"
        ]
    );
    assert!(matched.paths[0].reached_entrypoint);
}

#[test]
fn trace_errors_falls_back_to_direct_source_path() {
    let mut graph = CodeGraph::new("repo");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "panic",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    graph.add_edge(helper, error, EdgeKind::MayError, Confidence::Heuristic);

    let result = trace_errors(
        &graph,
        ErrorTraceRequest {
            target: "panic".to_string(),
            max_depth: 2,
            limit: 10,
        },
    );

    assert_eq!(result.total_matches, 1);
    assert_eq!(result.matches[0].paths.len(), 1);
    assert_eq!(
        result.matches[0].paths[0]
            .nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<Vec<_>>(),
        vec!["helper", "panic"]
    );
    assert!(!result.matches[0].paths[0].reached_entrypoint);
}

#[test]
fn focus_subgraph_returns_selected_nodes_and_edges() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let result = focus_subgraph(
        &graph,
        FocusRequest {
            node_ids: vec![main],
            edge_indexes: vec![1],
            edge_limit: 10,
        },
    );

    assert_eq!(result.query, "focus");
    assert_eq!(result.total_edges, 1);
    assert_eq!(result.edges[0].source, helper);
    assert_eq!(result.edges[0].target, config);
    assert_eq!(
        result.edges[0].metadata.get("edge_index"),
        Some(&"1".to_string())
    );
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(result.nodes.iter().any(|node| node.id == helper));
    assert!(result.nodes.iter().any(|node| node.id == config));
}

#[test]
fn focus_subgraph_expands_node_only_focus_to_incident_edges() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let unrelated = graph.add_node(NodeKind::Function, "unrelated");
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([
            ("relation".to_string(), "entrypoint_function".to_string()),
            ("source".to_string(), "manifest".to_string()),
        ]),
    );
    graph.add_edge(unrelated, main, EdgeKind::Calls, Confidence::Heuristic);

    let result = focus_subgraph(
        &graph,
        FocusRequest {
            node_ids: vec![entrypoint],
            edge_indexes: Vec::new(),
            edge_limit: 10,
        },
    );

    assert_eq!(result.total_edges, 1);
    assert_eq!(result.edges[0].source, entrypoint);
    assert_eq!(result.edges[0].target, main);
    assert!(result.nodes.iter().any(|node| node.id == entrypoint));
    assert!(result.nodes.iter().any(|node| node.id == main));
    assert!(!result.nodes.iter().any(|node| node.id == unrelated));
}

#[test]
fn graph_slice_filters_and_pages_nodes() {
    let mut graph = CodeGraph::new("repo");
    let mut metadata = BTreeMap::new();
    metadata.insert("language".to_string(), "rust".to_string());
    metadata.insert("item_kind".to_string(), "function".to_string());
    let main = graph.add_node_with_metadata(NodeKind::Function, "main", None, metadata);
    let helper = graph.add_node(NodeKind::Function, "helper");
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(file, main, EdgeKind::Contains, Confidence::Syntactic);

    let result = slice_graph(
        &graph,
        GraphSliceRequest {
            node_offset: 0,
            node_limit: 1,
            edge_offset: 0,
            edge_limit: 10,
            path_prefix: None,
            kind: Some("function".to_string()),
            search: None,
            language: None,
            item_kind: None,
            edge_kind: None,
            confidence: None,
            edge_relation: None,
            edge_source: None,
        },
    );

    assert_eq!(result.total_nodes, 2);
    assert_eq!(result.nodes.len(), 1);
    assert!(result.truncated_nodes);
    assert!(result.edges.is_empty());

    let result = slice_graph(
        &graph,
        GraphSliceRequest {
            node_offset: 0,
            node_limit: 10,
            edge_offset: 0,
            edge_limit: 10,
            path_prefix: None,
            kind: Some("function".to_string()),
            search: Some("rust".to_string()),
            language: Some("rust".to_string()),
            item_kind: Some("function".to_string()),
            edge_kind: None,
            confidence: None,
            edge_relation: None,
            edge_source: None,
        },
    );

    assert_eq!(result.total_nodes, 1);
    assert_eq!(result.nodes[0].label, "main");
}

#[test]
fn graph_slice_pages_edges_inside_returned_node_page() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let other = graph.add_node(NodeKind::Function, "other");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(helper, other, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, other, EdgeKind::References, Confidence::Heuristic);

    let result = slice_graph(
        &graph,
        GraphSliceRequest {
            node_offset: 0,
            node_limit: 10,
            edge_offset: 0,
            edge_limit: 1,
            path_prefix: None,
            kind: Some("function".to_string()),
            search: None,
            language: None,
            item_kind: None,
            edge_kind: Some("calls".to_string()),
            confidence: None,
            edge_relation: None,
            edge_source: None,
        },
    );

    assert_eq!(result.total_nodes, 3);
    assert_eq!(result.total_edges, 2);
    assert_eq!(result.edges.len(), 1);
    assert_eq!(result.edges[0].source, main);
    assert_eq!(
        result.edges[0].metadata.get("edge_index"),
        Some(&"0".to_string())
    );
    assert!(result.truncated_edges);
}

#[test]
fn graph_slice_filters_nodes_by_path_prefix() {
    let mut graph = CodeGraph::new("repo");
    let api_file = graph.add_node(NodeKind::File, "api/main.rs");
    let core_file = graph.add_node(NodeKind::File, "core/lib.rs");
    let api_main = graph.add_node(NodeKind::Function, "main");
    let core_helper = graph.add_node(NodeKind::Function, "helper");
    graph.add_edge(
        api_file,
        api_main,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );
    graph.add_edge(
        core_file,
        core_helper,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );

    let result = slice_graph(
        &graph,
        GraphSliceRequest {
            node_offset: 0,
            node_limit: 10,
            edge_offset: 0,
            edge_limit: 10,
            path_prefix: Some("api".to_string()),
            kind: None,
            search: None,
            language: None,
            item_kind: None,
            edge_kind: None,
            confidence: None,
            edge_relation: None,
            edge_source: None,
        },
    );

    let labels: BTreeSet<_> = result
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .collect();
    assert_eq!(result.total_nodes, 2);
    assert!(labels.contains("api/main.rs"));
    assert!(labels.contains("main"));
    assert!(!labels.contains("core/lib.rs"));
    assert!(!labels.contains("helper"));
}

#[test]
fn graph_slice_filters_edges_by_confidence() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge_with_metadata(
        entrypoint,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([
            ("relation".to_string(), "entrypoint_function".to_string()),
            ("source".to_string(), "manifest".to_string()),
        ]),
    );

    let result = slice_graph(
        &graph,
        GraphSliceRequest {
            node_offset: 0,
            node_limit: 10,
            edge_offset: 0,
            edge_limit: 10,
            path_prefix: None,
            kind: None,
            search: None,
            language: None,
            item_kind: None,
            edge_kind: None,
            confidence: Some("exact".to_string()),
            edge_relation: Some("entrypoint_function".to_string()),
            edge_source: Some("manifest".to_string()),
        },
    );

    assert_eq!(result.total_edges, 1);
    assert_eq!(result.edges[0].source, entrypoint);
    assert_eq!(result.edges[0].confidence, Confidence::Exact);
}

#[test]
fn node_context_returns_limited_neighbor_edges() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    let config = graph.add_node(NodeKind::Config, "config/app.toml");
    graph.add_edge(file, main, EdgeKind::Contains, Confidence::Syntactic);
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(main, config, EdgeKind::ReadsConfig, Confidence::Heuristic);

    let context = node_context(&graph, main, 2).unwrap();

    assert_eq!(context.node.label, "main");
    assert_eq!(context.total_edges, 3);
    assert_eq!(context.edges.len(), 2);
    assert_eq!(
        context.edges[0].metadata.get("edge_index"),
        Some(&"0".to_string())
    );
    assert_eq!(
        context.edges[1].metadata.get("edge_index"),
        Some(&"1".to_string())
    );
    assert!(context.truncated_edges);
    assert!(context.nodes.iter().any(|node| node.id == main));
    assert!(context.nodes.iter().any(|node| node.id == file));
    assert!(context.nodes.iter().any(|node| node.id == helper));
}

#[test]
fn node_context_returns_none_for_missing_node() {
    let graph = CodeGraph::new("repo");

    assert!(node_context(&graph, NodeId(999), 10).is_none());
}

#[test]
fn insights_report_unresolved_calls_and_orphans() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let orphan = graph.add_node(NodeKind::Function, "orphan");
    let unresolved = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "missing",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "call".to_string()),
            ("resolution".to_string(), "unresolved".to_string()),
        ]),
    );
    graph.add_edge(main, unresolved, EdgeKind::Calls, Confidence::Heuristic);

    let report = insights(&graph);

    assert!(
        report
            .insights
            .iter()
            .any(|insight| insight.kind == "unresolved_call")
    );
    assert!(
        report.insights.iter().any(|insight| {
            insight.kind == "orphan_function" && insight.nodes.contains(&orphan)
        })
    );
}

#[test]
fn an_empty_answer_says_what_was_looked_for() {
    // `configs target:SECRET_KEY` came back empty and said nothing, which
    // reads as "this project has no such key" -- a claim the scan cannot
    // make, since it may simply not read that form.
    let mut graph = CodeGraph::new("repo");
    let reader = graph.add_node(NodeKind::Function, "load");
    let debug = graph.add_node(NodeKind::Environment, "DEBUG_MODE");
    graph.add_edge(
        reader,
        debug,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let missing = query_graph(&graph, "configs target:SECRET_KEY").expect("the query runs");
    assert!(missing.nodes.is_empty());
    assert!(
        missing
            .notes
            .first()
            .is_some_and(|note| note.contains("`SECRET_KEY`")),
        "{:?}",
        missing.notes
    );

    // Not a substring of the real key, so nothing matches it outright.
    let near = query_graph(&graph, "configs target:DEBUG_MOED").expect("the query runs");
    assert!(
        near.notes
            .first()
            .is_some_and(|note| note.contains("`DEBUG_MODE`")),
        "a close key is named: {:?}",
        near.notes
    );

    let found = query_graph(&graph, "configs target:DEBUG_MODE").expect("the query runs");
    assert!(!found.nodes.is_empty());
    assert!(found.notes.is_empty(), "{:?}", found.notes);
}

#[test]
fn node_not_found_errors_suggest_near_matches() {
    let mut graph = CodeGraph::new("repo");
    graph.add_node(NodeKind::Function, "scan_project");
    graph.add_node(NodeKind::Function, "load_config");
    graph.add_node(NodeKind::Function, "t");

    let typo = node_not_found_error(&graph, "impact target", "scan_projct");
    assert!(
        typo.to_string().contains("`scan_project`"),
        "close labels are suggested: {typo}"
    );
    assert!(
        !typo.to_string().contains("`t`"),
        "trivial short labels are not suggested: {typo}"
    );

    let nothing = node_not_found_error(&graph, "impact target", "zzzzqqqq");
    assert!(
        nothing.to_string().contains("entrypoints"),
        "no-match errors point at discovery commands: {nothing}"
    );
}

#[test]
fn unresolved_call_insights_calibrate_severity_and_group_by_label() {
    let mut graph = CodeGraph::new("repo");
    let caller = graph.add_node(NodeKind::Function, "main");
    let placeholder_metadata = BTreeMap::from([
        ("item_kind".to_string(), "call".to_string()),
        ("resolution".to_string(), "unresolved".to_string()),
    ]);
    let rust_helper = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "helper",
        None,
        placeholder_metadata.clone(),
    );
    let js_helper = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "helper",
        None,
        placeholder_metadata.clone(),
    );
    let builtin = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "format",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "call".to_string()),
            ("resolution".to_string(), "builtin".to_string()),
        ]),
    );
    graph.add_edge(caller, rust_helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(caller, js_helper, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(caller, builtin, EdgeKind::Calls, Confidence::Heuristic);

    let report = insights(&graph);
    let unresolved: Vec<_> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "unresolved_call")
        .collect();
    assert_eq!(
        unresolved.len(),
        1,
        "same-label placeholders share one finding, builtins are excluded"
    );
    assert_eq!(unresolved[0].severity, InsightSeverity::Info);
    assert!(unresolved[0].nodes.contains(&rust_helper));
    assert!(unresolved[0].nodes.contains(&js_helper));
    assert_eq!(unresolved[0].edges.len(), 2);

    // Semantic enrichment present: still-unresolved calls become warnings.
    let resolved_target = graph.add_node(NodeKind::Function, "load_config");
    graph.add_edge(
        caller,
        resolved_target,
        EdgeKind::Calls,
        Confidence::Semantic,
    );
    let enriched = insights(&graph);
    let enriched_unresolved = enriched
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_call")
        .expect("unresolved call finding");
    assert_eq!(enriched_unresolved.severity, InsightSeverity::Warning);
}

#[test]
fn insights_report_ambiguous_call_resolution() {
    let mut graph = CodeGraph::new("repo");
    let caller = graph.add_node(NodeKind::Function, "main");
    let left = graph.add_node(NodeKind::Function, "parse");
    let right = graph.add_node(NodeKind::Function, "parser::parse");
    let single = graph.add_node(NodeKind::Function, "load_config");
    graph.add_edge_with_metadata(
        caller,
        left,
        EdgeKind::Calls,
        Confidence::Heuristic,
        BTreeMap::from([
            ("call_label".to_string(), "parse".to_string()),
            ("resolution".to_string(), "ambiguous".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        caller,
        right,
        EdgeKind::Calls,
        Confidence::Heuristic,
        BTreeMap::from([
            ("call_label".to_string(), "parse".to_string()),
            ("resolution".to_string(), "ambiguous".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        caller,
        single,
        EdgeKind::Calls,
        Confidence::Heuristic,
        BTreeMap::from([
            ("call_label".to_string(), "load_config".to_string()),
            ("resolution".to_string(), "resolved".to_string()),
        ]),
    );

    let report = insights(&graph);
    let ambiguous = report
        .insights
        .iter()
        .find(|insight| insight.kind == "ambiguous_call_resolution")
        .expect("expected ambiguous call insight");

    // Syntactic-only fixture: heuristic findings read as info.
    assert_eq!(ambiguous.severity, InsightSeverity::Info);
    assert!(ambiguous.message.contains("main"));
    assert!(ambiguous.message.contains("parse"));
    assert!(ambiguous.nodes.contains(&caller));
    assert!(ambiguous.nodes.contains(&left));
    assert!(ambiguous.nodes.contains(&right));
    assert!(!ambiguous.nodes.contains(&single));
    assert_eq!(ambiguous.edges.len(), 2);
}

#[test]
fn an_import_from_code_the_package_never_ships_is_a_note() {
    let mut graph = CodeGraph::new("repo");
    graph.add_node_with_metadata(
        NodeKind::File,
        "package.json",
        None,
        BTreeMap::from([(
            "published_paths".to_string(),
            "/contracts/**/*.sol".to_string(),
        )]),
    );
    let contract = graph.add_node_with_metadata(
        NodeKind::File,
        "contracts/token/ERC20.sol",
        None,
        BTreeMap::from([("language".to_string(), "solidity".to_string())]),
    );
    let harness = graph.add_node_with_metadata(
        NodeKind::File,
        "fv/harnesses/AccessControlHarness.sol",
        None,
        BTreeMap::from([("language".to_string(), "solidity".to_string())]),
    );
    let unresolved_import = |graph: &mut CodeGraph, target: &str| {
        graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            format!("import \"{target}\";"),
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "solidity".to_string()),
                ("import_scope".to_string(), "local".to_string()),
                ("import_target".to_string(), target.to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
            ]),
        )
    };
    let from_contract = unresolved_import(&mut graph, "./Missing.sol");
    let from_harness = unresolved_import(&mut graph, "../patched/access/AccessControl.sol");
    graph.add_edge(
        contract,
        from_contract,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        harness,
        from_harness,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let severity = |target: &str| {
        report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "unresolved_local_import" && insight.message.contains(target)
            })
            .map(|insight| insight.severity)
    };

    // The contract ships: a dead import there is a defect in the program.
    assert_eq!(severity("./Missing.sol"), Some(InsightSeverity::Warning));
    // The harness does not, and `make` writes the tree it imports.
    assert_eq!(severity("../patched/"), Some(InsightSeverity::Info));
}

#[test]
fn insights_report_unresolved_local_imports() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/app.js");
    let import = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "import missing from './missing.js';",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "javascript".to_string()),
            ("import_scope".to_string(), "local".to_string()),
            ("import_target".to_string(), "./missing.js".to_string()),
            ("resolution".to_string(), "unresolved".to_string()),
        ]),
    );
    let external = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "import express from 'express';",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "javascript".to_string()),
        ]),
    );
    graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(file, external, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_local_import")
        .expect("expected unresolved local import insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("src/app.js"));
    assert!(insight.message.contains("./missing.js"));
    assert!(insight.nodes.contains(&file));
    assert!(insight.nodes.contains(&import));
    assert!(!insight.nodes.contains(&external));
    assert_eq!(insight.edges.len(), 1);
}

#[test]
fn insights_keep_quiet_about_built_and_substituted_import_targets() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "packages/vue/index.js");
    // A build writes the first into a directory no scan walks, and the
    // package build fills in the second.
    for (label, target) in [
        ("require('./dist/vue.cjs.js')", "./dist/vue.cjs.js"),
        ("source @HOME_MANAGER_LIB@", "@HOME_MANAGER_LIB@"),
    ] {
        let import = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            label,
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "javascript".to_string()),
                ("import_scope".to_string(), "local".to_string()),
                ("import_target".to_string(), target.to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
            ]),
        );
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "unresolved_local_import"),
        "built and substituted targets are not missing files: {:?}",
        report
            .insights
            .iter()
            .map(|insight| insight.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn insights_report_unresolved_sql_table_references() {
    let mut graph = CodeGraph::new("repo");
    let load_users = graph.add_node(NodeKind::Function, "load_users");
    let users = graph.add_node_with_metadata(
        NodeKind::Type,
        "sql table:users",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "sql_table".to_string()),
            ("table_name".to_string(), "users".to_string()),
        ]),
    );
    let query = graph.add_node_with_metadata(
        NodeKind::Config,
        "sql query:src/repo.py:2",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "app_sql_query".to_string()),
            ("operation".to_string(), "select".to_string()),
            ("tables".to_string(), "audit_log,users".to_string()),
            ("unresolved_tables".to_string(), "audit_log".to_string()),
            ("resolution".to_string(), "partial".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        load_users,
        query,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([("relation".to_string(), "app_sql_query".to_string())]),
    );
    graph.add_edge_with_metadata(
        query,
        users,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([(
            "relation".to_string(),
            "app_sql_table_reference".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_sql_table_reference")
        .expect("expected unresolved SQL table insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("audit_log"));
    assert!(insight.message.contains("load_users"));
    assert!(insight.nodes.contains(&load_users));
    assert!(insight.nodes.contains(&query));
    assert!(insight.nodes.contains(&users));
    assert_eq!(
        report.by_kind.get("unresolved_sql_table_reference"),
        Some(&1)
    );
}

#[test]
fn a_name_the_query_fills_in_is_not_a_missing_table() {
    // `fmt.Sprintf("... FROM %s.%s", schema, table)` writes the name when
    // it runs, and `information_schema.schemata` is the database's own
    // catalogue. Neither is a table anybody forgot to define.
    let mut graph = CodeGraph::new("repo");
    let configure = graph.add_node(NodeKind::Function, "Configure");
    for (label, unresolved) in [
        ("sql query:backend.go:108", "information_schema.schemata"),
        ("sql query:client.go:64", "%s.%s"),
        ("sql query:state.go:12", "%s, audit_log"),
    ] {
        let query = graph.add_node_with_metadata(
            NodeKind::Config,
            label,
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "app_sql_query".to_string()),
                ("operation".to_string(), "select".to_string()),
                ("unresolved_tables".to_string(), unresolved.to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            configure,
            query,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([("relation".to_string(), "app_sql_query".to_string())]),
        );
    }

    let report = insights(&graph);
    let found: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "unresolved_sql_table_reference")
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].message.contains("`audit_log`"),
        "the real name survives on its own: {}",
        found[0].message
    );
    assert!(!found[0].message.contains('%'), "{}", found[0].message);
}

#[test]
fn fixture_driven_unresolved_findings_read_as_info() {
    let mut graph = CodeGraph::new("repo");

    // SQL string extracted from an inline `#[cfg(test)]` module.
    let test_fn = graph.add_node(NodeKind::Function, "roundtrip_test");
    let inline_query = graph.add_node_with_metadata(
        NodeKind::Config,
        "sql query:src/lib.rs:900",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "app_sql_query".to_string()),
            ("operation".to_string(), "select".to_string()),
            ("unresolved_tables".to_string(), "missing".to_string()),
            ("test_context".to_string(), "true".to_string()),
        ]),
    );
    graph.add_edge(
        test_fn,
        inline_query,
        EdgeKind::References,
        Confidence::Heuristic,
    );

    // SQL string extracted from a test-convention file path.
    let fixture_fn = graph.add_node(NodeKind::Function, "seed_db");
    let fixture_query = graph.add_node_with_metadata(
        NodeKind::Config,
        "sql query:tests/seed.py:3",
        Some(codegraph_core::SourceSpan {
            path: "tests/seed.py".to_string(),
            start_line: 3,
            start_column: 1,
            end_line: 3,
            end_column: 40,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "app_sql_query".to_string()),
            ("operation".to_string(), "select".to_string()),
            ("unresolved_tables".to_string(), "missing".to_string()),
        ]),
    );
    graph.add_edge(
        fixture_fn,
        fixture_query,
        EdgeKind::References,
        Confidence::Heuristic,
    );

    // Unresolved local import declared by a test-convention file.
    let test_file = graph.add_node(NodeKind::File, "src/app_test.py");
    let fixture_import = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "from helpers import seed",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("import_scope".to_string(), "local".to_string()),
            ("import_target".to_string(), "helpers".to_string()),
            ("resolution".to_string(), "unresolved".to_string()),
        ]),
    );
    graph.add_edge(
        test_file,
        fixture_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let severities = |kind: &str| {
        report
            .insights
            .iter()
            .filter(|insight| insight.kind == kind)
            .map(|insight| insight.severity)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        severities("unresolved_sql_table_reference"),
        vec![InsightSeverity::Info, InsightSeverity::Info]
    );
    assert_eq!(
        severities("unresolved_local_import"),
        vec![InsightSeverity::Info]
    );
}

#[test]
fn insights_do_not_report_manifest_referenced_functions_as_orphans() {
    let mut graph = CodeGraph::new("repo");
    let entrypoint = graph.add_node(NodeKind::Entrypoint, "python console_script:cg");
    let referenced = graph.add_node(NodeKind::Function, "main");
    let orphan = graph.add_node(NodeKind::Function, "unused");
    graph.add_edge_with_metadata(
        entrypoint,
        referenced,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );

    let report = insights(&graph);

    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "orphan_function" && insight.nodes.contains(&referenced)
    }));
    assert!(
        report.insights.iter().any(|insight| {
            insight.kind == "orphan_function" && insight.nodes.contains(&orphan)
        })
    );
}

#[test]
fn insights_report_duplicate_entrypoints() {
    let mut graph = CodeGraph::new("repo");
    let left = graph.add_node(NodeKind::Entrypoint, "npm script:start");
    let right = graph.add_node(NodeKind::Entrypoint, "npm script:start");
    let unique = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
    graph.add_edge(graph.root, left, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(graph.root, right, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(graph.root, unique, EdgeKind::Entrypoint, Confidence::Exact);

    let report = insights(&graph);
    let duplicate = report
        .insights
        .iter()
        .find(|insight| insight.kind == "duplicate_entrypoint_label")
        .expect("expected duplicate entrypoint insight");

    // A note about the labels rather than a defect: nine Makefiles with an
    // `all` target is how a C project is built.
    assert_eq!(duplicate.severity, InsightSeverity::Info);
    assert_eq!(duplicate.nodes, vec![left, right]);
    assert!(duplicate.message.contains("npm script:start"));
    assert_eq!(duplicate.edges.len(), 2);
    assert!(!duplicate.nodes.contains(&unique));
}

#[test]
fn insights_report_ambiguous_manifest_entrypoint_targets() {
    let mut graph = CodeGraph::new("repo");
    let ambiguous = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "python console_script:serve",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("target".to_string(), "app:serve".to_string()),
        ]),
    );
    let first = graph.add_node(NodeKind::Function, "app::serve");
    let second = graph.add_node(NodeKind::Function, "legacy::serve");
    let resolved = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:api",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("target".to_string(), "src/main.rs".to_string()),
        ]),
    );
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(
        graph.root,
        ambiguous,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        graph.root,
        resolved,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    for target in [first, second] {
        graph.add_edge_with_metadata(
            ambiguous,
            target,
            EdgeKind::References,
            Confidence::Heuristic,
            BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
        );
    }
    graph.add_edge_with_metadata(
        resolved,
        file,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_file".to_string())]),
    );
    graph.add_edge_with_metadata(
        resolved,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "ambiguous_entrypoint_target")
        .expect("expected ambiguous entrypoint target insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("python console_script:serve"));
    assert!(insight.message.contains("functions"));
    assert!(insight.nodes.contains(&ambiguous));
    assert!(insight.nodes.contains(&first));
    assert!(insight.nodes.contains(&second));
    assert!(!insight.nodes.contains(&resolved));
    assert_eq!(insight.edges.len(), 2);
}

#[test]
fn insights_report_duplicate_functions_and_error_flow() {
    let mut graph = CodeGraph::new("repo");
    let left = graph.add_node(NodeKind::Function, "parse");
    let right = graph.add_node(NodeKind::Function, "parse");
    let error = graph.add_node(NodeKind::Unknown, "panic");
    graph.add_edge(left, error, EdgeKind::MayError, Confidence::Heuristic);

    let report = insights(&graph);

    assert!(report.insights.iter().any(|insight| {
        insight.kind == "duplicate_function_label"
            && insight.nodes.contains(&left)
            && insight.nodes.contains(&right)
    }));
    assert!(
        report
            .insights
            .iter()
            .any(|insight| insight.kind == "potential_error_flow")
    );
}

#[test]
fn a_variable_set_before_a_command_is_read_as_the_path_it_holds() {
    let mut graph = CodeGraph::new("repo");
    // openzeppelin runs `env SRC=./fv/harnesses hardhat build`: the shell
    // sets a variable, and the directory it names is right there.
    let entrypoint = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "npm script:compile:harnesses",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            (
                "target".to_string(),
                "env SRC=./fv/harnesses hardhat build --noExpose".to_string(),
            ),
        ]),
    );
    graph.add_node(NodeKind::Directory, "fv/harnesses");
    graph.add_edge(
        graph.root,
        entrypoint,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "unresolved_entrypoint_target"),
        "{:?}",
        report
            .insights
            .iter()
            .map(|insight| insight.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn insights_report_unresolved_manifest_entrypoints() {
    let mut graph = CodeGraph::new("repo");
    let broken = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "npm script:start",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("target".to_string(), "node missing.js".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:demo",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("target".to_string(), "src/main.rs".to_string()),
        ]),
    );
    let targetless = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo package:repo",
        None,
        BTreeMap::from([("item_kind".to_string(), "manifest_entrypoint".to_string())]),
    );
    let main_file = graph.add_node(NodeKind::File, "src/main.rs");
    graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        graph.root,
        resolved,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        graph.root,
        targetless,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge_with_metadata(
        resolved,
        main_file,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_file".to_string())]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_entrypoint_target")
        .expect("expected unresolved entrypoint insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert_eq!(insight.nodes, vec![broken]);
    assert!(insight.message.contains("missing.js"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_entrypoint_target"
            && (insight.nodes.contains(&resolved) || insight.nodes.contains(&targetless))
    }));
}

#[test]
fn insights_report_unresolved_makefile_command_paths() {
    let mut graph = CodeGraph::new("repo");
    let broken = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "make target:deploy",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "makefile_target".to_string()),
            (
                "command".to_string(),
                "./scripts/deploy.sh --prod".to_string(),
            ),
            ("command_path".to_string(), "scripts/deploy.sh".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "make target:test",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "makefile_target".to_string()),
            ("command".to_string(), "./scripts/test.sh".to_string()),
            ("command_path".to_string(), "scripts/test.sh".to_string()),
        ]),
    );
    let shell_only = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "make target:build",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "makefile_target".to_string()),
            ("command".to_string(), "cargo test --workspace".to_string()),
        ]),
    );
    let test_script = graph.add_node(NodeKind::File, "scripts/test.sh");
    graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        graph.root,
        resolved,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge(
        graph.root,
        shell_only,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge_with_metadata(
        resolved,
        test_script,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([
            ("relation".to_string(), "entrypoint_file".to_string()),
            ("resolution".to_string(), "make_command_path".to_string()),
        ]),
    );

    // `go run ./tools/protobuf-compile .` names a package directory, which
    // is neither a file nor missing.
    let directory_target = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "make target:protobuf",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "makefile_target".to_string()),
            (
                "command".to_string(),
                "go run ./tools/protobuf-compile .".to_string(),
            ),
            (
                "command_path".to_string(),
                "tools/protobuf-compile".to_string(),
            ),
        ]),
    );
    graph.add_edge(
        graph.root,
        directory_target,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_node(NodeKind::Directory, "tools/protobuf-compile");

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_makefile_command_path")
        .expect("expected unresolved Makefile command path insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert_eq!(insight.nodes, vec![broken]);
    assert!(insight.message.contains("scripts/deploy.sh"));

    // What the build produces is missing only until the build runs:
    // redis's Makefile runs `./src/redis-server` and dune's runs
    // `_boot/dune.exe`, neither of which is a file anybody wrote.
    let built = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "make target:start",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "makefile_target".to_string()),
            ("command".to_string(), "./src/redis-server".to_string()),
            ("command_path".to_string(), "src/redis-server".to_string()),
        ]),
    );
    graph.add_edge(graph.root, built, EdgeKind::Entrypoint, Confidence::Exact);
    let report = insights(&graph);
    let built_insight = report
        .insights
        .iter()
        .find(|insight| {
            insight.kind == "unresolved_makefile_command_path" && insight.nodes.contains(&built)
        })
        .expect("still recorded, and as a note");
    assert_eq!(built_insight.severity, InsightSeverity::Info);
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_makefile_command_path"
            && (insight.nodes.contains(&resolved)
                || insight.nodes.contains(&shell_only)
                || insight.nodes.contains(&directory_target))
    }));
}

#[test]
fn insights_report_unresolved_dockerfile_command_paths() {
    let mut graph = CodeGraph::new("repo");
    let broken = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "docker entrypoint:./docker/start.sh",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "dockerfile_entrypoint".to_string()),
            ("command".to_string(), "./docker/start.sh".to_string()),
            ("command_path".to_string(), "docker/start.sh".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "docker cmd:./docker/migrate.sh",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "dockerfile_entrypoint".to_string()),
            ("command".to_string(), "./docker/migrate.sh".to_string()),
            ("command_path".to_string(), "docker/migrate.sh".to_string()),
        ]),
    );
    let migrate_script = graph.add_node(NodeKind::File, "docker/migrate.sh");
    graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        graph.root,
        resolved,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge_with_metadata(
        resolved,
        migrate_script,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([
            ("relation".to_string(), "entrypoint_file".to_string()),
            ("resolution".to_string(), "docker_command_path".to_string()),
        ]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_dockerfile_command_path")
        .expect("expected unresolved Dockerfile command path insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert_eq!(insight.nodes, vec![broken]);
    assert!(insight.message.contains("docker/start.sh"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_dockerfile_command_path" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_compose_command_paths() {
    let mut graph = CodeGraph::new("repo");
    let broken = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "compose service:web",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_service".to_string()),
            ("command".to_string(), "./scripts/start.sh".to_string()),
            ("command_path".to_string(), "scripts/start.sh".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "compose service:worker",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_service".to_string()),
            ("command".to_string(), "./scripts/worker.sh".to_string()),
            ("command_path".to_string(), "scripts/worker.sh".to_string()),
        ]),
    );
    let worker_script = graph.add_node(NodeKind::File, "scripts/worker.sh");
    graph.add_edge(graph.root, broken, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        graph.root,
        resolved,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge_with_metadata(
        resolved,
        worker_script,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([
            ("relation".to_string(), "entrypoint_file".to_string()),
            ("resolution".to_string(), "compose_command_path".to_string()),
        ]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_compose_command_path")
        .expect("expected unresolved Compose command path insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert_eq!(insight.nodes, vec![broken]);
    assert!(insight.message.contains("scripts/start.sh"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_compose_command_path" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_compose_env_file_paths() {
    let mut graph = CodeGraph::new("repo");
    let web = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "compose service:web",
        None,
        BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
    );
    let worker = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "compose service:worker",
        None,
        BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
    );
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "compose env file:config/missing.env",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_env_file".to_string()),
            ("service".to_string(), "web".to_string()),
            (
                "env_file_path".to_string(),
                "config/missing.env".to_string(),
            ),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "compose env file:worker.env",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_env_file".to_string()),
            ("service".to_string(), "worker".to_string()),
            ("env_file_path".to_string(), "worker.env".to_string()),
        ]),
    );
    let worker_env = graph.add_node(NodeKind::File, "worker.env");
    graph.add_edge(graph.root, web, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(graph.root, worker, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge_with_metadata(
        web,
        missing,
        EdgeKind::ReadsConfig,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "compose_env_file".to_string())]),
    );
    graph.add_edge_with_metadata(
        worker,
        resolved,
        EdgeKind::ReadsConfig,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "compose_env_file".to_string())]),
    );
    graph.add_edge_with_metadata(
        resolved,
        worker_env,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([(
            "resolution".to_string(),
            "compose_env_file_path".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_compose_env_file_path")
        .expect("expected unresolved Compose env_file path insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&missing));
    assert!(insight.nodes.contains(&web));
    assert!(insight.message.contains("config/missing.env"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_compose_env_file_path" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_compose_volume_source_paths() {
    let mut graph = CodeGraph::new("repo");
    let web = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "compose service:web",
        None,
        BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
    );
    let worker = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "compose service:worker",
        None,
        BTreeMap::from([("item_kind".to_string(), "compose_service".to_string())]),
    );
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "compose volume:config/missing->/app/config",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_volume".to_string()),
            ("service".to_string(), "web".to_string()),
            (
                "local_source_path".to_string(),
                "config/missing".to_string(),
            ),
            ("target_path".to_string(), "/app/config".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "compose volume:config->/app/config",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_volume".to_string()),
            ("service".to_string(), "worker".to_string()),
            ("local_source_path".to_string(), "config".to_string()),
            ("target_path".to_string(), "/app/config".to_string()),
        ]),
    );
    let config_dir = graph.add_node(NodeKind::Directory, "config");
    graph.add_edge_with_metadata(
        web,
        missing,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "compose_volume".to_string())]),
    );
    graph.add_edge_with_metadata(
        worker,
        resolved,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "compose_volume".to_string())]),
    );
    graph.add_edge_with_metadata(
        resolved,
        config_dir,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([(
            "resolution".to_string(),
            "compose_volume_source_path".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_compose_volume_source_path")
        .expect("expected unresolved Compose volume source path insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&missing));
    assert!(insight.nodes.contains(&web));
    assert!(insight.message.contains("config/missing"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_compose_volume_source_path" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_github_actions_local_actions() {
    let mut graph = CodeGraph::new("repo");
    let build = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "github workflow:CI/build",
        None,
        BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
    );
    let deploy = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "github workflow:CI/deploy",
        None,
        BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
    );
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "github action:.github/actions/missing",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "github_actions_local_action".to_string(),
            ),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "build".to_string()),
            (
                "local_action_path".to_string(),
                ".github/actions/missing".to_string(),
            ),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "github action:.github/actions/setup",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "github_actions_local_action".to_string(),
            ),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "deploy".to_string()),
            (
                "local_action_path".to_string(),
                ".github/actions/setup".to_string(),
            ),
        ]),
    );
    let setup_dir = graph.add_node(NodeKind::Directory, ".github/actions/setup");
    graph.add_edge_with_metadata(
        build,
        missing,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "github_actions_uses".to_string())]),
    );
    graph.add_edge_with_metadata(
        deploy,
        resolved,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "github_actions_uses".to_string())]),
    );
    graph.add_edge_with_metadata(
        resolved,
        setup_dir,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([(
            "resolution".to_string(),
            "github_actions_local_action_path".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_github_actions_local_action")
        .expect("expected unresolved GitHub Actions local action insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&missing));
    assert!(insight.nodes.contains(&build));
    assert!(insight.message.contains(".github/actions/missing"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_github_actions_local_action"
            && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_github_actions_job_needs() {
    let mut graph = CodeGraph::new("repo");
    let build = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "github workflow:CI/build",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "github_actions_job".to_string()),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "build".to_string()),
        ]),
    );
    let deploy = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "github workflow:CI/deploy",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "github_actions_job".to_string()),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "deploy".to_string()),
            ("needs".to_string(), "build,missing".to_string()),
        ]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_github_actions_job_need")
        .expect("expected unresolved GitHub Actions job need insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert_eq!(insight.nodes, vec![deploy]);
    assert!(insight.message.contains("missing"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_github_actions_job_need" && insight.message.contains("build")
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_github_actions_job_need" && insight.nodes.contains(&build)
    }));
}

#[test]
fn insights_report_unresolved_github_actions_run_paths() {
    let mut graph = CodeGraph::new("repo");
    let build = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "github workflow:CI/build",
        None,
        BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
    );
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "github run:CI/build/10",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "github_actions_run_step".to_string(),
            ),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "build".to_string()),
            ("command".to_string(), "./scripts/missing.sh".to_string()),
            ("command_path".to_string(), "scripts/missing.sh".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "github run:CI/build/11",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "github_actions_run_step".to_string(),
            ),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "build".to_string()),
            ("command".to_string(), "./scripts/test.sh".to_string()),
            ("command_path".to_string(), "scripts/test.sh".to_string()),
        ]),
    );
    let test_script = graph.add_node(NodeKind::File, "scripts/test.sh");
    for step in [missing, resolved] {
        graph.add_edge_with_metadata(
            build,
            step,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "github_actions_run".to_string())]),
        );
    }
    graph.add_edge_with_metadata(
        build,
        test_script,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([(
            "resolution".to_string(),
            "github_actions_run_command_path".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_github_actions_run_path")
        .expect("expected unresolved GitHub Actions run path insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&missing));
    assert!(insight.nodes.contains(&build));
    assert!(insight.message.contains("scripts/missing.sh"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_github_actions_run_path" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_keep_quiet_about_installed_and_unscanned_command_paths() {
    let mut graph = CodeGraph::new("repo");
    let build = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "github workflow:CI/build",
        None,
        BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
    );
    // composer writes the first, a virtualenv the second, and no default scan
    // walks the third; the repository is missing none of them.
    let quiet = [
        "vendor/bin/phpunit",
        "docs/venv/bin/mkdocs",
        ".buildscript/prepare.sh",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, command_path)| {
        graph.add_node_with_metadata(
            NodeKind::Config,
            format!("github run:CI/build/{index}"),
            None,
            BTreeMap::from([
                (
                    "item_kind".to_string(),
                    "github_actions_run_step".to_string(),
                ),
                ("workflow".to_string(), "CI".to_string()),
                ("job".to_string(), "build".to_string()),
                ("command".to_string(), command_path.to_string()),
                ("command_path".to_string(), command_path.to_string()),
            ]),
        )
    })
    .collect::<Vec<_>>();
    // `make -C docs/mkdocs` runs in a directory the scan did walk.
    let directory_step = graph.add_node_with_metadata(
        NodeKind::Config,
        "github run:CI/build/9",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "github_actions_run_step".to_string(),
            ),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "build".to_string()),
            (
                "command".to_string(),
                "make build -C docs/mkdocs".to_string(),
            ),
            ("command_path".to_string(), "docs/mkdocs".to_string()),
        ]),
    );
    graph.add_node(NodeKind::Directory, "docs/mkdocs");
    for step in quiet.iter().copied().chain([directory_step]) {
        graph.add_edge_with_metadata(
            build,
            step,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "github_actions_run".to_string())]),
        );
    }

    // `rm -f build.log` names the file it deletes, not one that must exist.
    let written_step = graph.add_node_with_metadata(
        NodeKind::Config,
        "github run:CI/build/10",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "github_actions_run_step".to_string(),
            ),
            ("workflow".to_string(), "CI".to_string()),
            ("job".to_string(), "build".to_string()),
            ("command".to_string(), "rm -f logs/build.log".to_string()),
            ("command_path".to_string(), "logs/build.log".to_string()),
            ("command_path_role".to_string(), "written".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        build,
        written_step,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "github_actions_run".to_string())]),
    );

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "unresolved_github_actions_run_path"),
        "installed, unscanned, written and directory paths are not missing files: {:?}",
        report
            .insights
            .iter()
            .map(|insight| insight.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn insights_report_unresolved_gitlab_ci_script_paths() {
    let mut graph = CodeGraph::new("repo");
    let build = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "gitlab job:build",
        None,
        BTreeMap::from([("item_kind".to_string(), "gitlab_ci_job".to_string())]),
    );
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "gitlab script:build/10",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "gitlab_ci_script".to_string()),
            ("job".to_string(), "build".to_string()),
            ("command".to_string(), "./scripts/missing.sh".to_string()),
            ("command_path".to_string(), "scripts/missing.sh".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "gitlab script:build/11",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "gitlab_ci_script".to_string()),
            ("job".to_string(), "build".to_string()),
            ("command".to_string(), "./scripts/test.sh".to_string()),
            ("command_path".to_string(), "scripts/test.sh".to_string()),
        ]),
    );
    let test_script = graph.add_node(NodeKind::File, "scripts/test.sh");
    for script in [missing, resolved] {
        graph.add_edge_with_metadata(
            build,
            script,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "gitlab_ci_script".to_string())]),
        );
    }
    graph.add_edge_with_metadata(
        build,
        test_script,
        EdgeKind::References,
        Confidence::Heuristic,
        BTreeMap::from([(
            "resolution".to_string(),
            "gitlab_ci_script_command_path".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_gitlab_ci_script_path")
        .expect("expected unresolved GitLab CI script path insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&missing));
    assert!(insight.nodes.contains(&build));
    assert!(insight.message.contains("scripts/missing.sh"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_gitlab_ci_script_path" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_gitlab_ci_job_dependencies() {
    let mut graph = CodeGraph::new("repo");
    let build = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "gitlab job:build",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "gitlab_ci_job".to_string()),
            ("job".to_string(), "build".to_string()),
        ]),
    );
    let deploy = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "gitlab job:deploy",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "gitlab_ci_job".to_string()),
            ("job".to_string(), "deploy".to_string()),
            ("needs".to_string(), "build,missing-need".to_string()),
            (
                "dependencies".to_string(),
                "build,missing-artifacts".to_string(),
            ),
        ]),
    );

    let report = insights(&graph);
    let missing_need = report
        .insights
        .iter()
        .find(|insight| {
            insight.kind == "unresolved_gitlab_ci_job_dependency"
                && insight.message.contains("missing-need")
        })
        .expect("expected unresolved GitLab CI need insight");
    let missing_artifacts = report
        .insights
        .iter()
        .find(|insight| {
            insight.kind == "unresolved_gitlab_ci_job_dependency"
                && insight.message.contains("missing-artifacts")
        })
        .expect("expected unresolved GitLab CI dependency insight");

    assert_eq!(missing_need.severity, InsightSeverity::Warning);
    assert_eq!(missing_need.nodes, vec![deploy]);
    assert_eq!(missing_artifacts.nodes, vec![deploy]);
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_gitlab_ci_job_dependency" && insight.message.contains("`build`")
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_gitlab_ci_job_dependency" && insight.nodes.contains(&build)
    }));
}

#[test]
fn insights_report_unresolved_kubernetes_config_refs() {
    let mut graph = CodeGraph::new("repo");
    let web = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "k8s deployment:prod/web",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_workload".to_string()),
            ("kubernetes_kind".to_string(), "Deployment".to_string()),
            ("name".to_string(), "web".to_string()),
            ("namespace".to_string(), "prod".to_string()),
        ]),
    );
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s config ref:configmap prod/missing-config",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_config_ref".to_string()),
            ("config_kind".to_string(), "configmap".to_string()),
            ("name".to_string(), "missing-config".to_string()),
            ("namespace".to_string(), "prod".to_string()),
            ("workload".to_string(), "web".to_string()),
            ("workload_kind".to_string(), "Deployment".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s config ref:secret prod/app-secret",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_config_ref".to_string()),
            ("config_kind".to_string(), "secret".to_string()),
            ("name".to_string(), "app-secret".to_string()),
            ("namespace".to_string(), "prod".to_string()),
            ("workload".to_string(), "web".to_string()),
            ("workload_kind".to_string(), "Deployment".to_string()),
        ]),
    );
    let secret = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s secret:prod/app-secret",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_config".to_string()),
            ("config_kind".to_string(), "secret".to_string()),
            ("name".to_string(), "app-secret".to_string()),
            ("namespace".to_string(), "prod".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        web,
        missing,
        EdgeKind::ReadsConfig,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "kubernetes_config_ref".to_string())]),
    );
    graph.add_edge_with_metadata(
        web,
        resolved,
        EdgeKind::ReadsConfig,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "kubernetes_config_ref".to_string())]),
    );
    graph.add_edge_with_metadata(
        resolved,
        secret,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([(
            "resolution".to_string(),
            "kubernetes_config_ref".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_kubernetes_config_ref")
        .expect("expected unresolved Kubernetes config ref insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&missing));
    assert!(insight.nodes.contains(&web));
    assert!(insight.message.contains("prod/missing-config"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_kubernetes_config_ref" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_kubernetes_ingress_backends() {
    let mut graph = CodeGraph::new("repo");
    let ingress = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "k8s ingress:prod/web",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_ingress".to_string()),
            ("name".to_string(), "web".to_string()),
            ("namespace".to_string(), "prod".to_string()),
        ]),
    );
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s service ref:prod/missing",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "kubernetes_service_ref".to_string(),
            ),
            ("name".to_string(), "missing".to_string()),
            ("namespace".to_string(), "prod".to_string()),
            ("ingress".to_string(), "web".to_string()),
            ("host".to_string(), "example.test".to_string()),
            ("path".to_string(), "/missing".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s service ref:prod/api",
        None,
        BTreeMap::from([
            (
                "item_kind".to_string(),
                "kubernetes_service_ref".to_string(),
            ),
            ("name".to_string(), "api".to_string()),
            ("namespace".to_string(), "prod".to_string()),
            ("ingress".to_string(), "web".to_string()),
            ("host".to_string(), "example.test".to_string()),
            ("path".to_string(), "/api".to_string()),
        ]),
    );
    let service = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s service:prod/api",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_service".to_string()),
            ("name".to_string(), "api".to_string()),
            ("namespace".to_string(), "prod".to_string()),
        ]),
    );
    for service_ref in [missing, resolved] {
        graph.add_edge_with_metadata(
            ingress,
            service_ref,
            EdgeKind::References,
            Confidence::Exact,
            BTreeMap::from([(
                "relation".to_string(),
                "kubernetes_ingress_backend".to_string(),
            )]),
        );
    }
    graph.add_edge_with_metadata(
        resolved,
        service,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([(
            "resolution".to_string(),
            "kubernetes_service_ref".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_kubernetes_ingress_backend")
        .expect("expected unresolved Kubernetes ingress backend insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&missing));
    assert!(insight.nodes.contains(&ingress));
    assert!(insight.message.contains("example.test/missing"));
    assert!(insight.message.contains("prod/missing"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_kubernetes_ingress_backend" && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_unresolved_kubernetes_service_selectors() {
    let mut graph = CodeGraph::new("repo");
    let missing = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s service:prod/orphan",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_service".to_string()),
            ("name".to_string(), "orphan".to_string()),
            ("namespace".to_string(), "prod".to_string()),
            ("selector".to_string(), "app=missing".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Config,
        "k8s service:prod/web",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_service".to_string()),
            ("name".to_string(), "web".to_string()),
            ("namespace".to_string(), "prod".to_string()),
            ("selector".to_string(), "app=web".to_string()),
        ]),
    );
    let web = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "k8s deployment:prod/web",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "kubernetes_workload".to_string()),
            ("name".to_string(), "web".to_string()),
            ("namespace".to_string(), "prod".to_string()),
            ("pod_labels".to_string(), "app=web".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        resolved,
        web,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([(
            "relation".to_string(),
            "kubernetes_service_selector".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_kubernetes_service_selector")
        .expect("expected unresolved Kubernetes service selector insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert_eq!(insight.nodes, vec![missing]);
    assert!(insight.message.contains("prod/orphan"));
    assert!(insight.message.contains("app=missing"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unresolved_kubernetes_service_selector"
            && insight.nodes.contains(&resolved)
    }));
}

#[test]
fn insights_report_duplicate_compose_published_ports() {
    let mut graph = CodeGraph::new("repo");
    let web = graph.add_node(NodeKind::Entrypoint, "compose service:web");
    let admin = graph.add_node(NodeKind::Entrypoint, "compose service:admin");
    let worker = graph.add_node(NodeKind::Entrypoint, "compose service:worker");
    let web_port = graph.add_node_with_metadata(
        NodeKind::Config,
        "compose port:8080->80/tcp",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_port".to_string()),
            ("service".to_string(), "web".to_string()),
            ("published_port".to_string(), "8080".to_string()),
            ("target_port".to_string(), "80".to_string()),
            ("protocol".to_string(), "tcp".to_string()),
        ]),
    );
    let admin_port = graph.add_node_with_metadata(
        NodeKind::Config,
        "compose port:8080->8080/tcp",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_port".to_string()),
            ("service".to_string(), "admin".to_string()),
            ("published_port".to_string(), "8080".to_string()),
            ("target_port".to_string(), "8080".to_string()),
            ("protocol".to_string(), "tcp".to_string()),
        ]),
    );
    let worker_port = graph.add_node_with_metadata(
        NodeKind::Config,
        "compose port:8080->9000/udp",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "compose_port".to_string()),
            ("service".to_string(), "worker".to_string()),
            ("published_port".to_string(), "8080".to_string()),
            ("target_port".to_string(), "9000".to_string()),
            ("protocol".to_string(), "udp".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        web,
        web_port,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "compose_port".to_string())]),
    );
    graph.add_edge_with_metadata(
        admin,
        admin_port,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "compose_port".to_string())]),
    );
    graph.add_edge_with_metadata(
        worker,
        worker_port,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "compose_port".to_string())]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "duplicate_compose_published_port")
        .expect("expected duplicate Compose published port insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("8080/tcp"));
    assert!(insight.message.contains("web"));
    assert!(insight.message.contains("admin"));
    assert!(insight.nodes.contains(&web_port));
    assert!(insight.nodes.contains(&admin_port));
    assert!(!insight.nodes.contains(&worker_port));
}

#[test]
fn a_report_cites_a_node_by_the_id_that_survives_an_edit() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/app.rs");
    let handler = graph.add_node_with_metadata(
        NodeKind::Function,
        "handle",
        Some(SourceSpan {
            path: "src/app.rs".to_string(),
            start_line: 12,
            start_column: 1,
            end_line: 30,
            end_column: 2,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "function".to_string()),
            ("stable_id".to_string(), "cg-1234567890abcdef".to_string()),
        ]),
    );
    graph.add_edge(file, handler, EdgeKind::Contains, Confidence::Exact);
    // A cycle gives the report a warning whose evidence names the node.
    let helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper",
        Some(SourceSpan {
            path: "src/helper.rs".to_string(),
            start_line: 3,
            start_column: 1,
            end_line: 9,
            end_column: 2,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "function".to_string()),
            ("stable_id".to_string(), "cg-fedcba0987654321".to_string()),
        ]),
    );
    graph.add_edge(handler, helper, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(helper, handler, EdgeKind::Calls, Confidence::Exact);

    let report = project_report(&graph, ProjectReportLimits::default());
    assert_eq!(
        report.durable_node_ids.get(&handler.to_string()),
        Some(&"cg-1234567890abcdef".to_string()),
        "{:?}",
        report.durable_node_ids
    );

    let markdown = project_report_markdown(&report, &ProjectReportMarkdownOptions::default());
    assert!(
        markdown.contains("cg-1234567890abcdef"),
        "the node summaries cite the durable id"
    );
    assert!(
        !markdown.contains(&format!("`{handler}` `handle`")),
        "and not the positional one"
    );
    assert!(
        markdown.contains("nodes: cg-"),
        "insight evidence cites durable ids: {}",
        markdown
            .lines()
            .find(|line| line.contains("nodes: "))
            .unwrap_or_default()
    );
}

#[test]
fn a_local_helper_is_reached_through_the_function_that_holds_it() {
    let mut graph = CodeGraph::new("repo");
    let trim = graph.add_node_with_metadata(
        NodeKind::Function,
        "trim",
        None,
        BTreeMap::from([("item_kind".to_string(), "function".to_string())]),
    );
    // shellcheck names 167 `where` bindings `f`; each is called by the
    // function it is written in.
    let helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "f",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "function".to_string()),
            ("enclosing_function".to_string(), "trim".to_string()),
        ]),
    );
    graph.add_edge(trim, helper, EdgeKind::Contains, Confidence::Exact);

    let orphans: Vec<_> = insights(&graph)
        .insights
        .into_iter()
        .filter(|insight| insight.kind == "orphan_function")
        .map(|insight| insight.message)
        .collect();
    assert!(
        orphans.iter().any(|message| message.contains("`trim`")),
        "the definition nothing calls is still reported: {orphans:?}"
    );
    assert!(
        !orphans.iter().any(|message| message.contains("`f`")),
        "the local helper is not an orphan: {orphans:?}"
    );
}

#[test]
fn a_class_split_across_files_is_one_unit() {
    let mut graph = CodeGraph::new("repo");
    let function = |graph: &mut CodeGraph, label: &str, path: &str, owner: Option<&str>| {
        let mut metadata = BTreeMap::from([("item_kind".to_string(), "function".to_string())]);
        if let Some(owner) = owner {
            metadata.insert("owner_type".to_string(), owner.to_string());
        }
        graph.add_node_with_metadata(
            NodeKind::Function,
            label,
            Some(SourceSpan {
                path: path.to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 9,
                end_column: 2,
            }),
            metadata,
        )
    };
    // A class whose methods call each other is one unit, however many files
    // C# splits it across.
    let or_result = function(
        &mut graph,
        "OrResult",
        "src/Polly/PolicyBuilder.OrSyntax.cs",
        Some("PolicyBuilder"),
    );
    let builder = function(
        &mut graph,
        "PolicyBuilder",
        "src/Polly/PolicyBuilder.cs",
        Some("PolicyBuilder"),
    );
    graph.add_edge(or_result, builder, EdgeKind::Calls, Confidence::Exact);
    graph.add_edge(builder, or_result, EdgeKind::Calls, Confidence::Exact);

    // Two files that import each other across directories still couple.
    let log = graph.add_node(NodeKind::File, "kong/cmd/utils/log.lua");
    let deprecation = graph.add_node(NodeKind::File, "kong/deprecation.lua");
    graph.add_edge(log, deprecation, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(deprecation, log, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let cycles: Vec<_> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "dependency_cycle")
        .map(|insight| (insight.severity, insight.message.as_str()))
        .collect();
    let severity_of = |needle: &str| {
        cycles
            .iter()
            .find(|(_, message)| message.contains(needle))
            .map(|(severity, _)| *severity)
    };
    // Reported, but as one class's own shape rather than as coupling.
    assert_eq!(
        severity_of("OrResult"),
        Some(InsightSeverity::Info),
        "{cycles:?}"
    );
    assert!(
        cycles
            .iter()
            .any(|(_, message)| message.contains("inside one type")),
        "{cycles:?}"
    );
    assert_eq!(
        severity_of("log.lua"),
        Some(InsightSeverity::Warning),
        "{cycles:?}"
    );
}

#[test]
fn two_constraints_conflict_only_when_no_version_satisfies_both() {
    let agree =
        |ecosystem: &str, constraints: &[&str]| constraints_can_agree(ecosystem, constraints);

    // A Cargo workspace where one crate asks for 1.0.75 and another for
    // 1.0.103 installs one version: a bare cargo version is a range.
    assert!(agree("cargo", &["1.0.75", "1.0.103"]));
    assert!(agree("cargo", &["2", "2.2.7", "2.4.0"]));
    assert!(agree("cargo", &["0.4.0", "0.4.18"]));
    assert!(agree("cargo", &["1", "=1.0.229"]));
    // A leading zero makes the next component the breaking one.
    assert!(!agree("cargo", &["0.4.0", "0.8.0"]));
    assert!(!agree("cargo", &["1.0.0", "2.0.0"]));

    // pub and npm carets, and the ranges pubspec writes out.
    assert!(agree("dart", &["^2.5.0", "^2.11.0"]));
    assert!(agree("dart", &[">=0.1.5 <2.0.0", "^1.0.0"]));
    assert!(agree("npm", &["^5.3.0", "^5.6.2"]));
    assert!(agree("npm", &["*", "~6.0.3"]));
    // npm pins what it writes bare, so two pins are two requirements.
    assert!(!agree("npm", &["4.9.5", "^5"]));
    assert!(!agree("npm", &["12.20.55", "20.19.39"]));

    // Python's `==` against a floor that excludes it.
    assert!(agree("python", &["==3.1.2", ">=3.1.2"]));
    assert!(agree("python", &["==2.1.2", ">=2.1.1"]));
    assert!(!agree("python", &["==1.6.2", ">=1.9.0"]));
    assert!(!agree("python", &["==2.3.3", ">=3.1.0"]));

    // An alternation admits what its widest branch does, and a constraint
    // nothing can read is reported rather than hidden.
    assert!(agree("npm", &["^1 || ^2", "^2.0.0"]));
    assert!(!agree("npm", &["not-a-version", "1.0.0"]));
}

#[test]
fn a_php_namespace_offers_the_spellings_composer_publishes() {
    // `Doctrine\CouchDB` is doctrine/couchdb and `MongoDB\Collection` is
    // mongodb/collection: a run of capitals is one word in the published
    // name, while a single capital opens the next one.
    let candidates = |namespace: &str| php_namespace_package_candidates(namespace);
    assert!(
        candidates("Doctrine\\CouchDB\\Client").contains(&"doctrine/couchdb".to_string()),
        "{:?}",
        candidates("Doctrine\\CouchDB\\Client")
    );
    assert!(
        candidates("MongoDB\\Collection").contains(&"mongodb/collection".to_string()),
        "{:?}",
        candidates("MongoDB\\Collection")
    );
    // The word-separated spelling stays on offer for names published that
    // way, and the vendor-only guess remains the last resort.
    let dynamo = candidates("Aws\\DynamoDb\\DynamoDbClient");
    assert!(dynamo.contains(&"aws/dynamo-db".to_string()), "{dynamo:?}");
    assert!(dynamo.contains(&"aws/aws".to_string()), "{dynamo:?}");
    assert!(
        dynamo.iter().position(|c| c == "aws/aws")
            > dynamo.iter().position(|c| c == "aws/dynamo-db"),
        "the vendor-only guess comes last: {dynamo:?}"
    );
}

#[test]
fn a_route_declared_by_a_test_is_a_fixture() {
    let mut graph = CodeGraph::new("repo");
    let route = |graph: &mut CodeGraph, path: &str, file: &str| {
        graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("route GET {path}"),
            Some(SourceSpan {
                path: file.to_string(),
                start_line: 4,
                start_column: 1,
                end_line: 4,
                end_column: 40,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("framework".to_string(), "express".to_string()),
                ("method".to_string(), "GET".to_string()),
                ("path".to_string(), path.to_string()),
                ("handler".to_string(), "handler".to_string()),
            ]),
        )
    };
    let served = route(&mut graph, "/users", "lib/app.js");
    let fixture = route(&mut graph, "/test", "test/app.router.js");
    for node in [served, fixture] {
        graph.add_edge(graph.root, node, EdgeKind::Entrypoint, Confidence::Exact);
    }

    let report = insights(&graph);
    let severity_of = |needle: &str| {
        report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "unresolved_framework_route_handler"
                    && insight.message.contains(needle)
            })
            .map(|insight| insight.severity)
    };
    assert_eq!(severity_of("/users"), Some(InsightSeverity::Warning));
    assert_eq!(severity_of("/test"), Some(InsightSeverity::Info));
}

#[test]
fn a_dead_end_in_a_test_manifest_is_a_note() {
    let mut graph = CodeGraph::new("repo");
    // json declares a fuzzer target in `tests/thirdparty/Fuzzer` and a CUDA
    // example in `tests/cuda_example`; neither is the program.
    let manifest = graph.add_node(NodeKind::File, "tests/cuda_example/CMakeLists.txt");
    let own_manifest = graph.add_node(NodeKind::File, "CMakeLists.txt");
    let theirs = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cmake executable:json_cuda",
        None,
        BTreeMap::from([("item_kind".to_string(), "manifest_entrypoint".to_string())]),
    );
    let ours = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cmake executable:json_cli",
        None,
        BTreeMap::from([("item_kind".to_string(), "manifest_entrypoint".to_string())]),
    );
    for (file, entrypoint) in [(manifest, theirs), (own_manifest, ours)] {
        graph.add_edge(file, entrypoint, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(
            graph.root,
            entrypoint,
            EdgeKind::Entrypoint,
            Confidence::Exact,
        );
    }

    let report = insights(&graph);
    let severity_of = |needle: &str| {
        report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "entrypoint_dead_end" && insight.message.contains(needle)
            })
            .map(|insight| insight.severity)
    };
    assert_eq!(severity_of("json_cli"), Some(InsightSeverity::Warning));
    assert_eq!(severity_of("json_cuda"), Some(InsightSeverity::Info));
}

#[test]
fn insights_report_entrypoint_dead_ends() {
    let mut graph = CodeGraph::new("repo");
    let dead = graph.add_node(NodeKind::Entrypoint, "npm script:preview");
    let live = graph.add_node(NodeKind::Entrypoint, "cargo bin:api");
    let main = graph.add_node(NodeKind::Function, "main");
    let unresolved_manifest = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "cargo bin:missing",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("target".to_string(), "src/missing.rs".to_string()),
        ]),
    );
    graph.add_edge(graph.root, dead, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(graph.root, live, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        graph.root,
        unresolved_manifest,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    graph.add_edge_with_metadata(
        live,
        main,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_function".to_string())]),
    );

    let report = insights(&graph);
    let dead_end = report
        .insights
        .iter()
        .find(|insight| insight.kind == "entrypoint_dead_end")
        .expect("expected dead-end entrypoint insight");

    assert_eq!(dead_end.severity, InsightSeverity::Warning);
    assert_eq!(dead_end.nodes, vec![dead]);
    assert!(dead_end.edges.iter().any(|index| {
        graph
            .edges
            .get(*index)
            .is_some_and(|edge| edge.source == graph.root && edge.target == dead)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "entrypoint_dead_end"
            && (insight.nodes.contains(&live) || insight.nodes.contains(&unresolved_manifest))
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "unresolved_entrypoint_target"
            && insight.nodes.contains(&unresolved_manifest)
    }));
}

#[test]
fn insights_report_unreachable_config_reads() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let live_config = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    let unused_loader = graph.add_node(NodeKind::Function, "unused_loader");
    let unused_config = graph.add_node(NodeKind::Config, "config/legacy.toml");
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
    graph.add_edge(
        main,
        live_config,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        unused_loader,
        unused_config,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unreachable_config_read")
        .expect("expected unreachable config read insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert_eq!(insight.nodes, vec![unused_loader, unused_config]);
    assert!(insight.message.contains("unused_loader"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unreachable_config_read" && insight.nodes.contains(&main)
    }));
}

#[test]
fn thin_reachability_does_not_warn_about_being_thin() {
    // The coverage finding already says entrypoints reach almost nothing
    // here. Repeating that once per configuration read describes the gap,
    // not the read, so those findings drop to Info.
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
    for index in 0..40 {
        graph.add_node(NodeKind::Function, format!("unreached_{index}"));
    }

    let reader = graph.add_node(NodeKind::Function, "load_settings");
    let key = graph.add_node(NodeKind::Environment, "DATABASE_URL");
    graph.add_edge(
        reader,
        key,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let report = insights(&graph);
    let read = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unreachable_config_read")
        .expect("the read is still reported");
    assert_eq!(read.severity, InsightSeverity::Info);
    assert!(
        report
            .insights
            .iter()
            .any(|insight| insight.kind == "low_entrypoint_coverage"),
        "the gap itself is still said once"
    );
}

#[test]
fn one_unreachable_reader_and_key_is_one_finding() {
    // A shell function that reads $HOME_MANAGER_BACKUP_EXT on two lines
    // reported the same sentence twice, and a key the code assembles at
    // runtime named nothing worth going to look at.
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);

    let reader = graph.add_node(NodeKind::Function, "checkCollision");
    let key = graph.add_node(NodeKind::Environment, "HOME_MANAGER_BACKUP_EXT");
    let computed = graph.add_node(NodeKind::Environment, COMPUTED_ENVIRONMENT_KEY);
    for line in ["23", "25"] {
        graph.add_edge_with_metadata(
            reader,
            key,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
            BTreeMap::from([("line".to_string(), line.to_string())]),
        );
    }
    graph.add_edge(
        reader,
        computed,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let report = insights(&graph);
    let found: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "unreachable_config_read")
        .collect();

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].message.contains("HOME_MANAGER_BACKUP_EXT"),
        "{}",
        found[0].message
    );
    assert_eq!(found[0].edges.len(), 2, "both reads stay as evidence");
}

#[test]
fn one_missing_file_is_one_finding() {
    // 63 of redis's source files include the same generated jemalloc
    // header. The same sentence 63 times says nothing the first one did
    // not, while `./utils` written in two directories means two files.
    let mut graph = CodeGraph::new("repo");
    let unresolved = |graph: &mut CodeGraph, source: &str, target: &str| {
        let file = graph.add_node(NodeKind::File, source);
        let import = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            format!("#include \"{target}\""),
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("import_scope".to_string(), "local".to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
                ("import_target".to_string(), target.to_string()),
            ]),
        );
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
    };
    unresolved(&mut graph, "src/arena.c", "jemalloc/internal/preamble.h");
    unresolved(&mut graph, "src/base.c", "jemalloc/internal/preamble.h");
    unresolved(&mut graph, "src/bin.c", "jemalloc/internal/preamble.h");
    unresolved(&mut graph, "packages/a/index.ts", "./utils");
    unresolved(&mut graph, "packages/b/index.ts", "./utils");
    unresolved(&mut graph, "configure", "./$cache_file");

    let report = insights(&graph);
    let found: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "unresolved_local_import")
        .collect();

    assert_eq!(found.len(), 3, "{found:?}");
    let header = found
        .iter()
        .find(|insight| insight.message.contains("preamble.h"))
        .expect("the shared header");
    assert!(
        header.message.starts_with(
            "`src/arena.c`, `src/base.c`, `src/bin.c` import local target `jemalloc/internal/preamble.h`"
        ),
        "{}",
        header.message
    );
    assert_eq!(header.edges.len(), 3, "every include stays as evidence");
    assert_eq!(
        found
            .iter()
            .filter(|insight| insight.message.contains("`./utils`"))
            .count(),
        2,
        "the same spelling in two directories is two files"
    );
    assert!(
        found
            .iter()
            .all(|insight| !insight.message.contains("cache_file")),
        "a target the script computes names no file"
    );
}

#[test]
fn a_demonstration_program_is_not_the_library_being_started() {
    // gson's five `main`s sit in `extras/.../examples` and `metrics/`, and
    // serde's three are `build.rs`: none of them is the library running, so
    // "entrypoints reach 0% of its functions" is the shape of a library
    // rather than something to fix.
    let build_a_graph = |main_path: &str| {
        let mut graph = CodeGraph::new("repo");
        let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
        let main = graph.add_node_with_metadata(
            NodeKind::Function,
            "main",
            Some(SourceSpan {
                path: main_path.to_string(),
                start_line: 1,
                start_column: 0,
                end_line: 3,
                end_column: 1,
            }),
            BTreeMap::from([("entrypoint_kind".to_string(), "program".to_string())]),
        );
        graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
        graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
        for index in 0..60 {
            graph.add_node_with_metadata(
                NodeKind::Function,
                format!("api_{index}"),
                Some(SourceSpan {
                    path: format!("src/api_{index}.rs"),
                    start_line: 1,
                    start_column: 0,
                    end_line: 2,
                    end_column: 1,
                }),
                BTreeMap::new(),
            );
        }
        graph
    };
    let severity_of = |graph: &CodeGraph| {
        insights(graph)
            .insights
            .iter()
            .find(|insight| insight.kind == "low_entrypoint_coverage")
            .map(|insight| insight.severity)
    };

    assert_eq!(
        severity_of(&build_a_graph("metrics/src/Benchmark.java")),
        Some(InsightSeverity::Info)
    );
    assert_eq!(
        severity_of(&build_a_graph("build.rs")),
        Some(InsightSeverity::Info)
    );
    // A program the project actually ships still says so.
    assert_eq!(
        severity_of(&build_a_graph("src/main.rs")),
        Some(InsightSeverity::Warning)
    );
}

#[test]
fn insights_report_low_entrypoint_coverage() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
    // Every function is called from main and the calls resolve, so coverage is
    // high and the diagnostic stays quiet.
    for index in 0..40 {
        let orphan = graph.add_node(NodeKind::Function, format!("worker_{index}"));
        graph.add_edge(main, orphan, EdgeKind::Calls, Confidence::Heuristic);
    }

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "low_entrypoint_coverage");
    assert!(
        insight.is_none(),
        "resolved calls keep coverage high: {insight:?}"
    );

    let mut sparse = CodeGraph::new("repo");
    let sparse_entry = sparse.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    // The program the binary starts, as the scan records one.
    let sparse_main = sparse.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("entrypoint_kind".to_string(), "program".to_string())]),
    );
    sparse.add_edge(
        sparse.root,
        sparse_entry,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );
    sparse.add_edge(
        sparse_entry,
        sparse_main,
        EdgeKind::References,
        Confidence::Exact,
    );
    let placeholder = sparse.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "client.Do",
        None,
        BTreeMap::from([("resolution".to_string(), "ambiguous".to_string())]),
    );
    sparse.add_edge(
        sparse_main,
        placeholder,
        EdgeKind::Calls,
        Confidence::Heuristic,
    );
    for index in 0..40 {
        sparse.add_node(NodeKind::Function, format!("worker_{index}"));
    }

    let report = insights(&sparse);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "low_entrypoint_coverage")
        .expect("expected a coverage diagnostic");
    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(
        insight.message.contains("1 of 41 functions"),
        "{}",
        insight.message
    );
    assert!(
        insight.message.contains("0% of calls"),
        "{}",
        insight.message
    );
    assert!(insight.nodes.contains(&sparse_main) || insight.nodes.contains(&sparse_entry));
}

#[test]
fn documents_that_link_to_each_other_are_not_a_dependency_cycle() {
    let mut graph = CodeGraph::new("repo");
    let recipes = graph.add_node(NodeKind::File, "docs/recipes.md");
    let java_io = graph.add_node(NodeKind::File, "docs/java_io_recipes.md");
    for (source, target) in [(recipes, java_io), (java_io, recipes)] {
        graph.add_edge_with_metadata(
            source,
            target,
            EdgeKind::References,
            Confidence::Syntactic,
            BTreeMap::from([
                ("relation".to_string(), "markdown_link".to_string()),
                ("source".to_string(), "markdown".to_string()),
            ]),
        );
    }
    // Two source files that import each other still are one.
    let app = graph.add_node(NodeKind::File, "src/app.rs");
    let db = graph.add_node(NodeKind::File, "src/db.rs");
    graph.add_edge(app, db, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(db, app, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let cycles = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "dependency_cycle")
        .map(|insight| insight.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(cycles.len(), 1, "{cycles:?}");
    assert!(cycles[0].contains("src/"), "{cycles:?}");
}

#[test]
fn a_library_with_no_program_reads_its_coverage_as_context() {
    // Alamofire, dplyr and ecto have no `main` and no route into their own
    // code, so "entrypoints reach 0%" describes a library rather than a gap.
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "github workflow:CI/build");
    let script = graph.add_node(NodeKind::File, "scripts/build.sh");
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge_with_metadata(
        entry,
        script,
        EdgeKind::References,
        Confidence::Exact,
        BTreeMap::from([("relation".to_string(), "entrypoint_file".to_string())]),
    );
    for index in 0..40 {
        graph.add_node(NodeKind::Function, format!("helper_{index}"));
    }

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "low_entrypoint_coverage")
        .expect("expected a coverage diagnostic");
    assert_eq!(insight.severity, InsightSeverity::Info);
    assert!(
        insight.message.starts_with("no program entrypoint starts"),
        "{}",
        insight.message
    );
}

#[test]
fn entrypoint_reachability_descends_from_a_file_into_its_symbols() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "script:scripts/release.sh");
    let file = graph.add_node(NodeKind::File, "scripts/release.sh");
    let publish = graph.add_node(NodeKind::Function, "publish");
    let failure = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "exit 1",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    // The entrypoint names the script file, not the function inside it.
    graph.add_edge(entry, file, EdgeKind::References, Confidence::Exact);
    graph.add_edge(file, publish, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(publish, failure, EdgeKind::MayError, Confidence::Heuristic);

    let report = insights(&graph);
    assert!(
        !report.insights.iter().any(|insight| {
            insight.kind == "unreachable_error_flow" && insight.nodes.contains(&publish)
        }),
        "a function inside an entrypoint script is reachable"
    );
}

#[test]
fn insights_report_unreachable_error_flows() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main = graph.add_node(NodeKind::Function, "main");
    let live_error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "panic",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    let legacy_worker = graph.add_node(NodeKind::Function, "legacy_worker");
    let legacy_error = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "LegacyError",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
    graph.add_edge(main, live_error, EdgeKind::MayError, Confidence::Heuristic);
    graph.add_edge(
        legacy_worker,
        legacy_error,
        EdgeKind::MayError,
        Confidence::Heuristic,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unreachable_error_flow")
        .expect("expected unreachable error flow insight");

    // Error-flow facts are informational context.
    assert_eq!(insight.severity, InsightSeverity::Info);
    assert_eq!(insight.nodes, vec![legacy_worker, legacy_error]);
    assert!(insight.message.contains("legacy_worker"));
    assert!(insight.message.contains("LegacyError"));
    assert_eq!(report.by_kind.get("unreachable_error_flow"), Some(&1));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unreachable_error_flow" && insight.nodes.contains(&main)
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "potential_error_flow" && insight.nodes.contains(&main)
    }));
}

#[test]
fn insights_report_unreachable_source_files() {
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let live_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let live_main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let legacy_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/legacy.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let legacy_fn = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_worker",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let test_file = graph.add_node_with_metadata(
        NodeKind::File,
        "tests/legacy_test.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let test_fn = graph.add_node_with_metadata(
        NodeKind::Function,
        "legacy_test",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        live_file,
        live_main,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );
    graph.add_edge(entry, live_main, EdgeKind::References, Confidence::Exact);
    graph.add_edge(
        legacy_file,
        legacy_fn,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );
    graph.add_edge(
        test_file,
        test_fn,
        EdgeKind::Contains,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unreachable_source_file")
        .expect("expected unreachable source file insight");

    assert_eq!(insight.severity, InsightSeverity::Info);
    assert_eq!(insight.nodes, vec![legacy_file]);
    assert!(insight.message.contains("src/legacy.rs"));
    assert!(insight.message.contains("rust"));
    assert_eq!(insight.edges.len(), 1);
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unreachable_source_file"
            && (insight.nodes.contains(&live_file) || insight.nodes.contains(&test_file))
    }));
}

#[test]
fn risk_rows_count_what_they_are_labelled() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "pyproject.toml");
    let package = dependency_node(&mut graph, "numpy", "python:numpy");
    graph.add_edge(manifest, package, EdgeKind::DependsOn, Confidence::Exact);
    // One import from the program and two from examples: one warning and
    // two notes of the same kind.
    for (path, module) in [
        ("src/app/main.py", "import pandas"),
        ("examples/demo.py", "import tqdm"),
        ("examples/other.py", "import h5py"),
    ] {
        let file = graph.add_node(NodeKind::File, path);
        let import = import_node(&mut graph, module, "python");
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    // A small insight limit truncates the embedded sample; the summary is
    // built from the whole report, so the counts must not follow it.
    let report = project_report(
        &graph,
        ProjectReportLimits {
            architecture_group_limit: 5,
            architecture_edge_limit: 5,
            language_link_limit: 5,
            hotspot_limit: 1,
            community_limit: 5,
            insight_limit: 1,
            file_summary_limit: 1,
            node_summary_limit: 2,
            fail_on: InsightSeverity::Error,
        },
    );
    let rows: Vec<_> = report
        .risk_summary
        .top_kinds
        .iter()
        .filter(|row| row.kind == "undeclared_external_import")
        .map(|row| (row.severity.as_str(), row.count))
        .collect();
    assert!(rows.contains(&("warning", 1)), "{rows:?}");
    assert!(rows.contains(&("info", 2)), "{rows:?}");
    // Every warning row adds up to the summary's own count.
    let warning_rows: usize = report
        .risk_summary
        .top_kinds
        .iter()
        .filter(|row| row.severity == "warning")
        .map(|row| row.count)
        .sum();
    assert_eq!(warning_rows, report.risk_summary.warnings);
}

#[test]
fn an_example_apps_own_pinning_is_a_note() {
    let mut graph = CodeGraph::new("repo");
    let package = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "http",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "dependency".to_string()),
            ("package_id".to_string(), "pub:http".to_string()),
        ]),
    );
    let declare = |graph: &mut CodeGraph, manifest: &str, version: &str, scope: &str| {
        let file = graph.add_node(NodeKind::File, manifest);
        graph.add_edge_with_metadata(
            file,
            package,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_version".to_string(), version.to_string()),
                ("dependency_kind".to_string(), scope.to_string()),
            ]),
        );
    };
    // Two example apps that cannot agree on a version is the examples'
    // business — and they have to actually disagree: `^1.2.0` and `^1.5.0`
    // install one version between them, `^1.2.0` and `^2.0.0` do not.
    declare(
        &mut graph,
        "pkgs/ok_http/example/pubspec.yaml",
        "^1.2.0",
        "runtime",
    );
    declare(
        &mut graph,
        "pkgs/cronet_http/example/pubspec.yaml",
        "^2.0.0",
        "runtime",
    );

    let report = insights(&graph);
    let conflicting: Vec<_> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "conflicting_dependency_declaration")
        .collect();
    assert_eq!(conflicting.len(), 1, "{conflicting:?}");
    assert_eq!(conflicting[0].severity, InsightSeverity::Info);

    // The library itself disagreeing with an example is the project's.
    let mut own = CodeGraph::new("repo");
    let owned_package = own.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "http",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "dependency".to_string()),
            ("package_id".to_string(), "pub:http".to_string()),
        ]),
    );
    for (manifest, version) in [
        ("pkgs/http/pubspec.yaml", "^2.0.0"),
        ("pkgs/ok_http/example/pubspec.yaml", "^1.2.0"),
    ] {
        let file = own.add_node(NodeKind::File, manifest);
        own.add_edge_with_metadata(
            file,
            owned_package,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_version".to_string(), version.to_string())]),
        );
    }
    let report = insights(&own);
    assert!(
        report.insights.iter().any(
            |insight| insight.kind == "conflicting_dependency_declaration"
                && insight.severity == InsightSeverity::Warning
        ),
        "{:?}",
        report
            .insights
            .iter()
            .map(|insight| (insight.kind.as_str(), insight.severity))
            .collect::<Vec<_>>()
    );
}

#[test]
fn config_read_only_by_vendored_code_is_a_note() {
    let mut graph = CodeGraph::new("repo");
    let configure = graph.add_node(NodeKind::Function, "as_fn_error");
    // jemalloc's generated `configure` gives `as_lineno` three defaults.
    let key = graph.add_node(NodeKind::Environment, "as_lineno");
    for default_value in ["$1", "$3", "$LINENO"] {
        graph.add_edge_with_metadata(
            configure,
            key,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
            BTreeMap::from([
                ("default_value".to_string(), default_value.to_string()),
                ("file".to_string(), "deps/jemalloc/configure".to_string()),
            ]),
        );
    }
    // The project's own script gives PORT two.
    let server = graph.add_node(NodeKind::Function, "main");
    let port = graph.add_node(NodeKind::Environment, "PORT");
    for (default_value, file) in [("8000", "src/serve.sh"), ("9000", "src/worker.sh")] {
        graph.add_edge_with_metadata(
            server,
            port,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
            BTreeMap::from([
                ("default_value".to_string(), default_value.to_string()),
                ("file".to_string(), file.to_string()),
            ]),
        );
    }

    let report = insights(&graph);
    let severity_of = |needle: &str| {
        report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "conflicting_config_default" && insight.message.contains(needle)
            })
            .map(|insight| insight.severity)
    };
    assert_eq!(severity_of("PORT"), Some(InsightSeverity::Warning));
    assert_eq!(severity_of("as_lineno"), Some(InsightSeverity::Info));
}

#[test]
fn insights_report_conflicting_config_defaults() {
    let mut graph = CodeGraph::new("repo");
    let first_reader = graph.add_node(NodeKind::Function, "api_server");
    let second_reader = graph.add_node(NodeKind::Function, "worker");
    let first_env = graph.add_node_with_metadata(
        NodeKind::Environment,
        "PORT",
        None,
        BTreeMap::from([("default_value".to_string(), "8000".to_string())]),
    );
    let second_env = graph.add_node_with_metadata(
        NodeKind::Environment,
        "PORT",
        None,
        BTreeMap::from([("default_value".to_string(), "9000".to_string())]),
    );
    let extra_envs = ["3000", "5000", "7000", "8080", "9090", "9091", "9092"]
        .into_iter()
        .map(|default_value| {
            graph.add_node_with_metadata(
                NodeKind::Environment,
                "PORT",
                None,
                BTreeMap::from([("default_value".to_string(), default_value.to_string())]),
            )
        })
        .collect::<Vec<_>>();
    let stable_env = graph.add_node_with_metadata(
        NodeKind::Environment,
        "HOST",
        None,
        BTreeMap::from([("default_value".to_string(), "127.0.0.1".to_string())]),
    );
    graph.add_edge(
        first_reader,
        first_env,
        EdgeKind::ReadsEnvironment,
        codegraph_core::Confidence::Heuristic,
    );
    graph.add_edge(
        second_reader,
        second_env,
        EdgeKind::ReadsEnvironment,
        codegraph_core::Confidence::Heuristic,
    );
    for env in &extra_envs {
        graph.add_edge(
            second_reader,
            *env,
            EdgeKind::ReadsEnvironment,
            codegraph_core::Confidence::Heuristic,
        );
    }
    graph.add_edge(
        second_reader,
        stable_env,
        EdgeKind::ReadsEnvironment,
        codegraph_core::Confidence::Heuristic,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "conflicting_config_default")
        .expect("expected conflicting config default insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("PORT"));
    assert!(insight.message.contains("8000"));
    assert!(insight.message.contains("9000"));
    assert!(insight.message.contains("9091"));
    assert!(insight.message.contains("and 1 more"));
    assert!(insight.nodes.contains(&first_env));
    assert!(insight.nodes.contains(&second_env));
    assert!(extra_envs.iter().all(|env| insight.nodes.contains(env)));
    assert_eq!(insight.edges.len(), 9);
    assert!(!insight.nodes.contains(&stable_env));
}

#[test]
fn a_workflow_that_sets_a_variable_is_not_asking_for_one() {
    let mut graph = CodeGraph::new("repo");
    // ripgrep's workflow writes `TARGET_DIR: ./target` in its `env:` block
    // and a script reads `${TARGET_DIR:-target}`. The block states the
    // value; it does not ask the runner for one.
    let job = graph.add_node(NodeKind::Entrypoint, "ci.yml#build");
    let script = graph.add_node(NodeKind::File, "ci/test-complete");
    let target = graph.add_node(NodeKind::Environment, "TARGET_DIR");
    graph.add_edge_with_metadata(
        job,
        target,
        EdgeKind::ReadsEnvironment,
        Confidence::Exact,
        BTreeMap::from([
            ("relation".to_string(), "ci_environment".to_string()),
            ("value_present".to_string(), "true".to_string()),
            ("file".to_string(), ".github/workflows/ci.yml".to_string()),
            ("line".to_string(), "42".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        script,
        target,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
        BTreeMap::from([
            ("file".to_string(), "ci/test-complete".to_string()),
            ("line".to_string(), "21".to_string()),
            ("default_value".to_string(), "target".to_string()),
        ]),
    );

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "mixed_config_requirement"),
        "{:?}",
        report
            .insights
            .iter()
            .map(|insight| insight.message.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_shell_answers_for_a_variable_wherever_the_read_is_written() {
    let mut graph = CodeGraph::new("repo");
    // dune declares `confirm ()` above the assignments and calls it below;
    // kong keeps its defaults in `release-lib.sh` and sources that file.
    let library = graph.add_node(NodeKind::File, "scripts/release-lib.sh");
    let script = graph.add_node(NodeKind::File, "scripts/make-release");
    let cut = graph.add_node(NodeKind::File, "release-cut.sh");
    let confirm = graph.add_node(NodeKind::Function, "confirm");
    let import = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "source \"$(dirname \"$0\")/release-lib.sh\"",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "bash".to_string()),
        ]),
    );
    graph.add_edge(script, import, EdgeKind::Imports, Confidence::Syntactic);

    let org = graph.add_node(NodeKind::Environment, "GITHUB_ORG");
    let remote = graph.add_node(NodeKind::Environment, "DUNE_REMOTE");
    let read = |graph: &mut CodeGraph,
                source: NodeId,
                target: NodeId,
                file: &str,
                line: u32,
                default: Option<&str>,
                assigns: bool| {
        let mut metadata = BTreeMap::from([
            ("file".to_string(), file.to_string()),
            ("line".to_string(), line.to_string()),
        ]);
        if let Some(default) = default {
            metadata.insert("default_value".to_string(), default.to_string());
        }
        if assigns {
            metadata.insert("defaults_variable".to_string(), "true".to_string());
        }
        graph.add_edge_with_metadata(
            source,
            target,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
            metadata,
        );
    };

    // The library gives the variable a value; the script that sources it
    // reads what the library put there.
    read(
        &mut graph,
        library,
        org,
        "scripts/release-lib.sh",
        9,
        Some("Kong"),
        true,
    );
    read(
        &mut graph,
        script,
        org,
        "scripts/make-release",
        240,
        None,
        false,
    );

    // Two assignments in a row are one chain of fallbacks, and the
    // function above them reads what the chain settled on.
    read(
        &mut graph,
        cut,
        remote,
        "release-cut.sh",
        84,
        Some("$(git config remote.pushdefault)"),
        true,
    );
    read(
        &mut graph,
        cut,
        remote,
        "release-cut.sh",
        86,
        Some("origin"),
        true,
    );
    read(
        &mut graph,
        confirm,
        remote,
        "release-cut.sh",
        70,
        None,
        false,
    );

    let report = insights(&graph);
    let found: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| {
            matches!(
                insight.kind.as_str(),
                "mixed_config_requirement" | "conflicting_config_default"
            )
        })
        .map(|insight| insight.message.as_str())
        .collect();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_script_that_gave_the_variable_a_value_still_has_it() {
    // `GOPATH=${GOPATH:-$(go env GOPATH)}` on line 65 hands the variable a
    // value, so `$GOPATH` on line 68 reads what the script itself put
    // there. A default merely printed guards nothing.
    let mut graph = CodeGraph::new("repo");
    let build = graph.add_node(NodeKind::Function, "build");
    let gopath = graph.add_node(NodeKind::Environment, "GOPATH");
    let editor = graph.add_node(NodeKind::Environment, "EDITOR");
    let read = |graph: &mut CodeGraph, target, file: &str, line: u32, default, assigns| {
        let mut metadata = BTreeMap::from([
            ("file".to_string(), file.to_string()),
            ("line".to_string(), line.to_string()),
        ]);
        if let Some(default) = default {
            metadata.insert("default_value".to_string(), String::from(default));
        }
        if assigns {
            metadata.insert("defaults_variable".to_string(), "true".to_string());
        }
        graph.add_edge_with_metadata(
            build,
            target,
            EdgeKind::ReadsEnvironment,
            Confidence::Heuristic,
            metadata,
        );
    };
    read(
        &mut graph,
        gopath,
        "scripts/build.sh",
        65,
        Some("$(go env GOPATH)"),
        true,
    );
    read(&mut graph, gopath, "scripts/build.sh", 68, None, false);
    read(
        &mut graph,
        editor,
        "scripts/release.sh",
        4,
        Some("vi"),
        false,
    );
    read(&mut graph, editor, "scripts/release.sh", 20, None, false);

    let report = insights(&graph);
    let found: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "mixed_config_requirement")
        .map(|insight| insight.message.as_str())
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("EDITOR"), "{}", found[0]);
}

#[test]
fn insights_report_mixed_config_requirement_defaults() {
    let mut graph = CodeGraph::new("repo");
    let api = graph.add_node(NodeKind::Function, "api_server");
    let worker = graph.add_node(NodeKind::Function, "worker");
    let required_port = graph.add_node(NodeKind::Environment, "PORT");
    let default_port = graph.add_node_with_metadata(
        NodeKind::Environment,
        "PORT",
        None,
        BTreeMap::from([("default_value".to_string(), "8080".to_string())]),
    );
    let stable_host = graph.add_node_with_metadata(
        NodeKind::Environment,
        "HOST",
        None,
        BTreeMap::from([("default_value".to_string(), "127.0.0.1".to_string())]),
    );
    let unused_required_port = graph.add_node(NodeKind::Environment, "PORT");
    graph.add_edge(
        api,
        required_port,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        worker,
        default_port,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        api,
        stable_host,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "mixed_config_requirement")
        .expect("expected mixed config requirement insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("PORT"));
    assert!(insight.message.contains("required"));
    assert!(insight.message.contains("8080"));
    assert!(insight.nodes.contains(&required_port));
    assert!(insight.nodes.contains(&default_port));
    assert!(!insight.nodes.contains(&stable_host));
    assert!(!insight.nodes.contains(&unused_required_port));
    assert_eq!(insight.edges.len(), 2);
    assert_eq!(report.by_kind.get("mixed_config_requirement"), Some(&1));
}

#[test]
fn insights_report_undeclared_flutter_asset_reads() {
    let mut graph = CodeGraph::new("repo");
    let pubspec = graph.add_node(NodeKind::File, "pubspec.yaml");
    let main = graph.add_node(NodeKind::Function, "main");
    let declared_file = graph.add_node_with_metadata(
        NodeKind::Config,
        "flutter asset:assets/config/app.json",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "flutter_asset".to_string()),
            (
                "asset_path".to_string(),
                "assets/config/app.json".to_string(),
            ),
        ]),
    );
    let declared_dir = graph.add_node_with_metadata(
        NodeKind::Config,
        "flutter asset:assets/images/",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "flutter_asset".to_string()),
            ("asset_path".to_string(), "assets/images/".to_string()),
        ]),
    );
    let declared_read = graph.add_node_with_metadata(
        NodeKind::Config,
        "flutter asset read:assets/config/app.json",
        None,
        BTreeMap::from([
            ("config_kind".to_string(), "flutter_asset_read".to_string()),
            ("value".to_string(), "assets/config/app.json".to_string()),
        ]),
    );
    let directory_read = graph.add_node_with_metadata(
        NodeKind::Config,
        "flutter asset read:assets/images/logo.png",
        None,
        BTreeMap::from([
            ("config_kind".to_string(), "flutter_asset_read".to_string()),
            ("value".to_string(), "assets/images/logo.png".to_string()),
        ]),
    );
    let missing_read = graph.add_node_with_metadata(
        NodeKind::Config,
        "flutter asset read:assets/missing/secret.json",
        None,
        BTreeMap::from([
            ("config_kind".to_string(), "flutter_asset_read".to_string()),
            (
                "value".to_string(),
                "assets/missing/secret.json".to_string(),
            ),
        ]),
    );
    graph.add_edge(
        pubspec,
        declared_file,
        EdgeKind::Contains,
        Confidence::Exact,
    );
    graph.add_edge(pubspec, declared_dir, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(
        main,
        declared_read,
        EdgeKind::ReadsConfig,
        Confidence::Syntactic,
    );
    graph.add_edge(
        main,
        directory_read,
        EdgeKind::ReadsConfig,
        Confidence::Syntactic,
    );
    graph.add_edge(
        main,
        missing_read,
        EdgeKind::ReadsConfig,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "undeclared_flutter_asset")
        .expect("expected undeclared Flutter asset insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("assets/missing/secret.json"));
    assert!(insight.nodes.contains(&main));
    assert!(insight.nodes.contains(&missing_read));
    assert!(!insight.nodes.contains(&declared_read));
    assert!(!insight.nodes.contains(&directory_read));
    assert_eq!(report.by_kind.get("undeclared_flutter_asset"), Some(&1));
}

#[test]
fn a_note_left_in_vendored_code_is_upstreams() {
    let mut graph = CodeGraph::new("repo");
    let mine = graph.add_node(NodeKind::File, "src/server.c");
    let theirs = graph.add_node(NodeKind::File, "deps/jemalloc/src/arena.c");
    let comment = |graph: &mut CodeGraph, path: &str, text: &str| {
        graph.add_node_with_metadata(
            NodeKind::Unknown,
            text,
            Some(SourceSpan {
                path: path.to_string(),
                start_line: 9,
                start_column: 1,
                end_line: 9,
                end_column: 20,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "rationale_comment".to_string()),
                ("rationale_kind".to_string(), "fixme".to_string()),
            ]),
        )
    };
    let ours = comment(&mut graph, "src/server.c", "FIXME: drop the retry loop");
    let vendored = comment(
        &mut graph,
        "deps/jemalloc/src/arena.c",
        "FIXME: really hppa2.0-hp",
    );
    // A note left in a fixture is about that fixture.
    let in_a_test = comment(
        &mut graph,
        "tests/modules/services/php-fpm_test.c",
        "FIXME: this fixture assumes a socket",
    );
    graph.add_edge(mine, ours, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(theirs, vendored, EdgeKind::Contains, Confidence::Exact);
    graph.add_edge(mine, in_a_test, EdgeKind::Contains, Confidence::Exact);

    let report = insights(&graph);
    let severity_of = |needle: &str| {
        report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "rationale_risk_comment" && insight.message.contains(needle)
            })
            .map(|insight| insight.severity)
    };
    assert_eq!(severity_of("retry loop"), Some(InsightSeverity::Warning));
    assert_eq!(severity_of("hppa2.0-hp"), Some(InsightSeverity::Info));
    assert_eq!(severity_of("assumes a socket"), Some(InsightSeverity::Info));
}

#[test]
fn a_dart_part_is_not_a_dependency_cycle() {
    // `frame_reader.dart` says `part of 'frames.dart'` and `frames.dart`
    // says `part 'frame_reader.dart'`: one library written across two
    // files, which named each other by definition.
    let mut graph = CodeGraph::new("repo");
    let library = graph.add_node(NodeKind::File, "lib/frames.dart");
    let part = graph.add_node(NodeKind::File, "lib/frame_reader.dart");
    let import_metadata = |target: &str| {
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "dart".to_string()),
            ("import_form".to_string(), "part".to_string()),
            ("resolved_path".to_string(), target.to_string()),
        ])
    };
    let declares = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "part 'frame_reader.dart';",
        None,
        import_metadata("lib/frame_reader.dart"),
    );
    let belongs = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "part of 'frames.dart';",
        None,
        import_metadata("lib/frames.dart"),
    );
    graph.add_edge(library, declares, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(declares, part, EdgeKind::References, Confidence::Syntactic);
    graph.add_edge(part, belongs, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        belongs,
        library,
        EdgeKind::References,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "dependency_cycle"),
        "one library across two files is not two files depending on each other"
    );
}

#[test]
fn insights_report_rationale_risk_comments() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/auth.rs");
    let security = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "SECURITY: verify token audience",
        Some(SourceSpan {
            path: "src/auth.rs".to_string(),
            start_line: 7,
            start_column: 1,
            end_line: 7,
            end_column: 35,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "rationale_comment".to_string()),
            ("rationale_kind".to_string(), "security".to_string()),
        ]),
    );
    let fixme = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "FIXME: handle retry backoff",
        Some(SourceSpan {
            path: "src/auth.rs".to_string(),
            start_line: 12,
            start_column: 5,
            end_line: 12,
            end_column: 33,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "rationale_comment".to_string()),
            ("rationale_kind".to_string(), "fixme".to_string()),
        ]),
    );
    let why = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "WHY: keep startup simple",
        Some(SourceSpan {
            path: "src/auth.rs".to_string(),
            start_line: 3,
            start_column: 1,
            end_line: 3,
            end_column: 27,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "rationale_comment".to_string()),
            ("rationale_kind".to_string(), "why".to_string()),
        ]),
    );
    for node in [security, fixme, why] {
        graph.add_edge_with_metadata(
            file,
            node,
            EdgeKind::Contains,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "rationale_comment".to_string())]),
        );
    }

    let report = insights(&graph);
    let rationale = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "rationale_risk_comment")
        .collect::<Vec<_>>();

    assert_eq!(rationale.len(), 2);
    assert!(rationale.iter().any(|insight| {
        insight.severity == InsightSeverity::Error
            && insight.nodes.contains(&security)
            && insight.nodes.contains(&file)
            && insight.message.contains("SECURITY")
            && insight.message.contains("src/auth.rs:7")
    }));
    assert!(rationale.iter().any(|insight| {
        insight.severity == InsightSeverity::Warning
            && insight.nodes.contains(&fixme)
            && insight.nodes.contains(&file)
            && insight.message.contains("FIXME")
            && insight.message.contains("src/auth.rs:12")
    }));
    assert!(!rationale.iter().any(|insight| insight.nodes.contains(&why)));
    assert_eq!(report.by_kind.get("rationale_risk_comment"), Some(&2));
}

#[test]
fn insights_report_sensitive_config_defaults_without_leaking_values() {
    let mut graph = CodeGraph::new("repo");
    let api = graph.add_node(NodeKind::Function, "api_server");
    let worker = graph.add_node(NodeKind::Function, "worker");
    let secret = graph.add_node_with_metadata(
        NodeKind::Environment,
        "API_TOKEN",
        None,
        BTreeMap::from([("default_value".to_string(), "dev-super-secret".to_string())]),
    );
    let database_url = graph.add_node_with_metadata(
        NodeKind::Environment,
        "DATABASE_URL",
        None,
        BTreeMap::from([(
            "default_value".to_string(),
            "postgres://demo:password@localhost/app".to_string(),
        )]),
    );
    let auth_header = graph.add_node_with_metadata(
        NodeKind::Config,
        "service.auth_header",
        None,
        BTreeMap::from([("default_value".to_string(), "replace-me-token".to_string())]),
    );
    let port = graph.add_node_with_metadata(
        NodeKind::Environment,
        "PORT",
        None,
        BTreeMap::from([("default_value".to_string(), "8080".to_string())]),
    );
    let public_key = graph.add_node_with_metadata(
        NodeKind::Environment,
        "PUBLIC_KEY",
        None,
        BTreeMap::from([("default_value".to_string(), "public-demo-key".to_string())]),
    );
    let callback_url = graph.add_node_with_metadata(
        NodeKind::Config,
        "CALLBACK_URL",
        None,
        BTreeMap::from([(
            "default_value".to_string(),
            "https://example.com/callback".to_string(),
        )]),
    );
    graph.add_edge(
        api,
        secret,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        api,
        database_url,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        api,
        auth_header,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );
    graph.add_edge(
        worker,
        port,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        worker,
        public_key,
        EdgeKind::ReadsEnvironment,
        Confidence::Heuristic,
    );
    graph.add_edge(
        worker,
        callback_url,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );

    let report = insights(&graph);
    let sensitive = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "sensitive_config_default")
        .collect::<Vec<_>>();

    assert_eq!(sensitive.len(), 3);
    assert!(sensitive.iter().any(|insight| {
        insight.nodes.contains(&secret)
            && insight.nodes.contains(&api)
            && insight.message.contains("API_TOKEN")
            && !insight.message.contains("dev-super-secret")
    }));
    assert!(sensitive.iter().any(|insight| {
        insight.nodes.contains(&database_url)
            && insight.message.contains("DATABASE_URL")
            && !insight.message.contains("postgres://")
    }));
    assert!(sensitive.iter().any(|insight| {
        insight.nodes.contains(&auth_header)
            && insight.nodes.contains(&api)
            && insight.message.contains("service.auth_header")
            && !insight.message.contains("replace-me-token")
    }));
    assert!(
        !sensitive.iter().any(|insight| insight.nodes.contains(&port)
            || insight.nodes.contains(&public_key)
            || insight.nodes.contains(&callback_url))
    );
    assert_eq!(report.by_kind.get("sensitive_config_default"), Some(&3));
    assert_eq!(report.by_severity.get("warning"), Some(&3));
}

#[test]
fn insights_report_sensitive_ci_environment_literals_without_leaking_values() {
    let mut graph = CodeGraph::new("repo");
    let job = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "github workflow:CI/deploy",
        None,
        BTreeMap::from([("item_kind".to_string(), "github_actions_job".to_string())]),
    );
    // The variable is one node; what the workflow assigns to it rides on
    // the edge from the job that sets it.
    let literal_secret = graph.add_node(NodeKind::Environment, "API_TOKEN");
    let secret_reference = graph.add_node(NodeKind::Environment, "DEPLOY_TOKEN");
    let ordinary_literal = graph.add_node(NodeKind::Environment, "BUILD_MODE");
    for (environment, value_kind) in [
        (literal_secret, "literal"),
        (secret_reference, "secret_reference"),
        (ordinary_literal, "literal"),
    ] {
        graph.add_edge_with_metadata(
            job,
            environment,
            EdgeKind::ReadsEnvironment,
            Confidence::Exact,
            BTreeMap::from([
                ("item_kind".to_string(), "ci_environment".to_string()),
                ("relation".to_string(), "ci_environment".to_string()),
                ("source".to_string(), "github-actions".to_string()),
                ("scope".to_string(), "job".to_string()),
                ("value_kind".to_string(), value_kind.to_string()),
            ]),
        );
    }

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "sensitive_ci_environment_literal")
        .expect("expected sensitive CI environment literal insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.nodes.contains(&literal_secret));
    assert!(insight.nodes.contains(&job));
    assert!(insight.message.contains("API_TOKEN"));
    assert!(insight.message.contains("github-actions job"));
    assert!(!insight.message.contains("dev-super-secret"));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "sensitive_ci_environment_literal"
            && (insight.nodes.contains(&secret_reference)
                || insight.nodes.contains(&ordinary_literal))
    }));
}

#[test]
fn a_cycle_among_test_files_is_the_suite_shape() {
    let mut graph = CodeGraph::new("repo");
    let placed = |graph: &mut CodeGraph, label: &str, path: &str| {
        graph.add_node_with_metadata(
            NodeKind::Function,
            label,
            Some(SourceSpan {
                path: path.to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 9,
                end_column: 2,
            }),
            BTreeMap::new(),
        )
    };
    // kong's `spec/helpers/perf.lua` and the `spec/helpers/perf/git.lua`
    // beside it require each other.
    let helper = placed(&mut graph, "perf", "spec/helpers/perf.lua");
    let git = placed(&mut graph, "git", "spec/helpers/perf/git.lua");
    graph.add_edge(helper, git, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(git, helper, EdgeKind::Calls, Confidence::Heuristic);
    // And a cycle in the program itself, for contrast.
    let application = placed(&mut graph, "application", "src/oscar/core/application.py");
    let loading = placed(&mut graph, "loading", "src/oscar/core/loading.py");
    graph.add_edge(application, loading, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(loading, application, EdgeKind::Calls, Confidence::Heuristic);

    let report = insights(&graph);
    let severity_of = |node: NodeId| {
        report
            .insights
            .iter()
            .find(|insight| insight.kind == "dependency_cycle" && insight.nodes.contains(&node))
            .map(|insight| insight.severity)
    };

    assert_eq!(severity_of(helper), Some(InsightSeverity::Info));
    assert_eq!(severity_of(application), Some(InsightSeverity::Warning));

    // A ring needs every link: one written under `#[cfg(test)]` makes the
    // whole cycle a test-build cycle, wherever its files live.
    let searcher = placed(
        &mut graph,
        "searcher",
        "crates/searcher/src/searcher/mod.rs",
    );
    let testutil = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "use crate::testutil::RegexMatcher;",
        Some(SourceSpan {
            path: "crates/searcher/src/searcher/mod.rs".to_string(),
            start_line: 1048,
            start_column: 1,
            end_line: 1048,
            end_column: 40,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("test_context".to_string(), "true".to_string()),
        ]),
    );
    graph.add_edge(searcher, testutil, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(testutil, searcher, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let severity = report
        .insights
        .iter()
        .find(|insight| insight.kind == "dependency_cycle" && insight.nodes.contains(&searcher))
        .map(|insight| insight.severity);
    assert_eq!(severity, Some(InsightSeverity::Info));

    // But a component that still rings without the test import is the
    // program's, however many test imports it also holds.
    let sink = placed(&mut graph, "sink", "crates/searcher/src/sink.rs");
    graph.add_edge(searcher, sink, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(sink, searcher, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let severity = report
        .insights
        .iter()
        .find(|insight| insight.kind == "dependency_cycle" && insight.nodes.contains(&sink))
        .map(|insight| insight.severity);
    assert_eq!(severity, Some(InsightSeverity::Warning));
}

#[test]
fn insights_report_dependency_cycles() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let service = graph.add_node(NodeKind::Function, "service");
    let repository = graph.add_node(NodeKind::Function, "repository");
    let config = graph.add_node(NodeKind::Config, "settings.toml");
    graph.add_edge(main, service, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(service, repository, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(repository, main, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(
        service,
        config,
        EdgeKind::ReadsConfig,
        Confidence::Heuristic,
    );

    let report = insights(&graph);
    let cycle = report
        .insights
        .iter()
        .find(|insight| insight.kind == "dependency_cycle")
        .expect("expected dependency cycle insight");

    assert_eq!(cycle.severity, InsightSeverity::Warning);
    assert_eq!(cycle.nodes, vec![main, service, repository]);
    assert_eq!(cycle.edges.len(), 3);
    assert!(cycle.message.contains("main"));
    assert!(!cycle.nodes.contains(&config));
}

#[test]
fn the_report_summarizes_nodes_a_reader_can_open() {
    let mut graph = CodeGraph::new("repo");
    let caller = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        Some(SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 9,
            end_column: 2,
        }),
        BTreeMap::from([("item_kind".to_string(), "function".to_string())]),
    );
    // The stand-in for a name nothing resolved, with the span of a call site.
    let placeholder = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "to_string",
        Some(SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 4,
            start_column: 9,
            end_line: 4,
            end_column: 20,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "call".to_string()),
            ("resolution".to_string(), "unresolved".to_string()),
        ]),
    );
    for _ in 0..5 {
        graph.add_edge(caller, placeholder, EdgeKind::Calls, Confidence::Heuristic);
    }

    let report = project_report(&graph, ProjectReportLimits::default());
    assert!(
        report
            .node_summaries
            .nodes
            .iter()
            .any(|summary| summary.node.id == caller)
    );
    assert!(
        !report
            .node_summaries
            .nodes
            .iter()
            .any(|summary| summary.node.id == placeholder),
        "an unresolved-call placeholder is not a node to open"
    );
}

#[test]
fn a_suggestion_is_a_name_a_reader_can_ask_about() {
    let mut graph = CodeGraph::new("repo");
    graph.add_node(NodeKind::Function, "scan_project");
    // An import statement and an error construct carry a label too.
    graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "use codegraph_indexer::scan_project;",
        None,
        BTreeMap::from([("item_kind".to_string(), "import".to_string())]),
    );
    graph.add_node_with_metadata(
        NodeKind::ControlFlow,
        "scan_project",
        None,
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );

    let error = impact(
        &graph,
        ImpactRequest {
            target: "scan_projekt".to_string(),
            max_depth: 3,
            limit: 10,
        },
    )
    .expect_err("expected a miss");
    assert_eq!(
        error.to_string(),
        "impact target `scan_projekt` did not match a node; did you mean `scan_project`?"
    );
}

#[test]
fn explain_edge_says_what_it_searched_for() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    let helper = graph.add_node(NodeKind::Function, "helper");
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);

    let error = explain_edge(
        &graph,
        ExplainEdgeRequest {
            edge_index: None,
            source: Some("nosuchthing".to_string()),
            target: Some("alsonot".to_string()),
            kind: None,
        },
    )
    .expect_err("a filter that matches nothing is not an empty answer");
    assert_eq!(
        error.to_string(),
        "no edge matched source `nosuchthing`, target `alsonot`"
    );

    // ...and a filter that matches still answers.
    let found = explain_edge(
        &graph,
        ExplainEdgeRequest {
            edge_index: None,
            source: Some("main".to_string()),
            target: None,
            kind: None,
        },
    )
    .expect("expected an explanation");
    assert_eq!(found.expect("edge explanation").edge.target, helper);
}

#[test]
fn every_surface_takes_the_durable_id() {
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node_with_metadata(
        NodeKind::Function,
        "main",
        None,
        BTreeMap::from([("stable_id".to_string(), "cg-1111111111111111".to_string())]),
    );
    let helper = graph.add_node_with_metadata(
        NodeKind::Function,
        "helper",
        None,
        BTreeMap::from([("stable_id".to_string(), "cg-2222222222222222".to_string())]),
    );
    graph.add_edge(main, helper, EdgeKind::Calls, Confidence::Heuristic);

    // A trace start, which `workflow` and both traces resolve through.
    let start = resolve_trace_start(
        &graph,
        &TraceStart::Label("cg-2222222222222222".to_string()),
    )
    .expect("the durable id names a node");
    assert_eq!(start.id, helper);
    // ...and an edge endpoint, which `explain-edge` matches on.
    assert!(endpoint_matches(&graph, main, "cg-1111111111111111"));
    assert!(!endpoint_matches(&graph, helper, "cg-1111111111111111"));
}

#[test]
fn a_name_means_the_definition_not_the_error_that_wraps_it() {
    let mut graph = CodeGraph::new("repo");
    // `scan_project(..).unwrap()` gives the error construct the name of
    // the call it wraps, and a repository holds many of them.
    let error = graph.add_node_with_metadata(
        NodeKind::ControlFlow,
        "scan_project",
        Some(SourceSpan {
            path: "crates/cli/src/bench.rs".to_string(),
            start_line: 657,
            start_column: 1,
            end_line: 657,
            end_column: 40,
        }),
        BTreeMap::from([("item_kind".to_string(), "error".to_string())]),
    );
    let function = graph.add_node_with_metadata(
        NodeKind::Function,
        "scan_project",
        Some(SourceSpan {
            path: "crates/indexer/src/scan.rs".to_string(),
            start_line: 57,
            start_column: 1,
            end_line: 90,
            end_column: 2,
        }),
        BTreeMap::from([("item_kind".to_string(), "function".to_string())]),
    );

    let chosen = best_labelled_node(&graph, "scan_project").expect("nothing matched");
    assert_eq!(
        chosen.id, function,
        "the error construct outranked the function"
    );
    assert_ne!(chosen.id, error);
    // ...and one definition answers to the name, so there is nothing to warn about.
    assert_eq!(labelled_node_count(&graph, "scan_project"), 1);
}

#[test]
fn one_methods_overloads_are_not_a_cycle() {
    let mut graph = CodeGraph::new("repo");
    let owner = BTreeMap::from([("owner_type".to_string(), "Buffer".to_string())]);
    let short = graph.add_node_with_metadata(NodeKind::Function, "indexOf", None, owner.clone());
    let long = graph.add_node_with_metadata(NodeKind::Function, "indexOf", None, owner.clone());
    graph.add_edge(short, long, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(long, short, EdgeKind::Calls, Confidence::Heuristic);
    // A second method of the same type, named differently, still cycles.
    let read = graph.add_node_with_metadata(NodeKind::Function, "read", None, owner.clone());
    let fill = graph.add_node_with_metadata(NodeKind::Function, "fill", None, owner);
    graph.add_edge(read, fill, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge(fill, read, EdgeKind::Calls, Confidence::Heuristic);

    let cycles: Vec<_> = insights(&graph)
        .insights
        .into_iter()
        .filter(|insight| insight.kind == "dependency_cycle")
        .collect();
    assert_eq!(cycles.len(), 1, "cycles: {cycles:?}");
    assert_eq!(cycles[0].nodes, vec![read, fill]);
}

#[test]
fn build_tooling_may_import_a_dev_dependency() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let esbuild = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "esbuild",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "dependency".to_string()),
            ("package_id".to_string(), "npm:esbuild".to_string()),
            ("dependency_kind".to_string(), "dev".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        manifest,
        esbuild,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );

    for path in ["scripts/dev.js", "gulpfile.js", "src/index.js"] {
        let file = graph.add_node(NodeKind::File, path);
        let import = import_node(&mut graph, "import esbuild from \"esbuild\";", "javascript");
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let report = insights(&graph);
    let reported = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "non_runtime_dependency_import")
        .map(|insight| insight.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert!(reported[0].contains("src/index.js"), "{reported:?}");
    assert!(!reported[0].contains("scripts/dev.js"), "{reported:?}");
    assert!(!reported[0].contains("gulpfile.js"), "{reported:?}");
}

#[test]
fn a_django_app_mounts_its_urlconf_under_a_prefix_of_its_own() {
    let mut graph = CodeGraph::new("repo");
    let route = |graph: &mut CodeGraph, path: &str, file: &str| {
        let node = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("route ROUTE {path}"),
            Some(SourceSpan {
                path: file.to_string(),
                start_line: 4,
                start_column: 0,
                end_line: 4,
                end_column: 0,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("framework".to_string(), "django".to_string()),
                ("method".to_string(), "ROUTE".to_string()),
                ("path".to_string(), path.to_string()),
            ]),
        );
        graph.add_edge(graph.root, node, EdgeKind::Entrypoint, Confidence::Exact);
        node
    };
    // Two applications, each mounted under a prefix of its own: the same
    // written path is two different URLs.
    route(&mut graph, "/", "src/oscar/apps/basket/apps.py");
    route(&mut graph, "/", "src/oscar/apps/catalogue/apps.py");
    // Twice in one URLconf is still a collision.
    route(&mut graph, "/orders/", "src/oscar/apps/order/apps.py");
    route(&mut graph, "/orders/", "src/oscar/apps/order/apps.py");

    let duplicates = insights(&graph)
        .insights
        .into_iter()
        .filter(|insight| insight.kind == "duplicate_framework_route")
        .map(|insight| insight.message)
        .collect::<Vec<_>>();
    assert_eq!(duplicates.len(), 1, "{duplicates:?}");
    assert!(duplicates[0].contains("/orders/"), "{duplicates:?}");
}

#[test]
fn a_workspace_script_runs_where_its_manifest_is() {
    let mut graph = CodeGraph::new("repo");
    graph.add_node(NodeKind::File, "packages/tsc/package.json");
    graph.add_node(NodeKind::File, "packages/tsc/tsconfig.bench.json");
    let entry = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "npm script:build:bench",
        Some(SourceSpan {
            path: "packages/tsc/package.json".to_string(),
            start_line: 12,
            start_column: 0,
            end_line: 12,
            end_column: 0,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("ecosystem".to_string(), "npm".to_string()),
            (
                "target".to_string(),
                "tsc -p tsconfig.bench.json".to_string(),
            ),
        ]),
    );
    let conditions = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "npm script:dev",
        Some(SourceSpan {
            path: "package.json".to_string(),
            start_line: 4,
            start_column: 0,
            end_line: 4,
            end_column: 0,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "manifest_entrypoint".to_string()),
            ("ecosystem".to_string(), "npm".to_string()),
            (
                "target".to_string(),
                "tsx --conditions @zod/source".to_string(),
            ),
        ]),
    );
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(
        graph.root,
        conditions,
        EdgeKind::Entrypoint,
        Confidence::Exact,
    );

    let unresolved = insights(&graph)
        .insights
        .into_iter()
        .filter(|insight| insight.kind == "unresolved_entrypoint_target")
        .map(|insight| insight.message)
        .collect::<Vec<_>>();
    // The config sits beside the manifest that runs it, and `@zod/source`
    // is a package specifier rather than a directory.
    assert!(unresolved.is_empty(), "{unresolved:?}");
}

#[test]
fn a_quoted_include_of_an_installed_library_is_a_note() {
    let mut graph = CodeGraph::new("repo");
    graph.add_node(NodeKind::Directory, "src");
    let source = graph.add_node(NodeKind::File, "src/tls.c");
    let unresolved = |graph: &mut CodeGraph, label: &str, target: &str| {
        let node = graph.add_node_with_metadata(
            NodeKind::ExternalDependency,
            label,
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "import".to_string()),
                ("language".to_string(), "c".to_string()),
                ("import_scope".to_string(), "local".to_string()),
                ("import_target".to_string(), target.to_string()),
                ("resolution".to_string(), "unresolved".to_string()),
            ]),
        );
        graph.add_edge(source, node, EdgeKind::Imports, Confidence::Syntactic);
        node
    };
    // redis holds no `openssl/` directory, so that header comes from the
    // include path; `release.h` is a file it should hold next door.
    unresolved(&mut graph, "#include \"openssl/ssl.h\"", "openssl/ssl.h");
    unresolved(&mut graph, "#include \"release.h\"", "release.h");

    let severity_of = |needle: &str| {
        insights(&graph)
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "unresolved_local_import" && insight.message.contains(needle)
            })
            .map(|insight| insight.severity)
    };
    assert_eq!(severity_of("openssl/ssl.h"), Some(InsightSeverity::Info));
    assert_eq!(severity_of("release.h"), Some(InsightSeverity::Warning));
}

#[test]
fn a_composer_package_is_matched_by_the_library_it_ships() {
    let mut graph = CodeGraph::new("repo");
    let source = graph.add_node(NodeKind::File, "src/Monolog/Handler/ElasticaHandler.php");
    // A composer vendor is whoever publishes the package, and the namespace
    // says nothing about that: monolog declares these and imports those.
    for (label, package_id) in [
        ("ruflin/elastica", "composer:ruflin/elastica"),
        ("graylog2/gelf-php", "composer:graylog2/gelf-php"),
        ("aws/aws-sdk-php", "composer:aws/aws-sdk-php"),
    ] {
        let declared = dependency_node(&mut graph, label, package_id);
        graph.add_edge(source, declared, EdgeKind::DependsOn, Confidence::Exact);
    }
    for label in [
        "use Elastica\\Client;",
        "use Gelf\\Message;",
        "use Aws\\DynamoDb\\DynamoDbClient;",
    ] {
        let import = import_node(&mut graph, label, "php");
        graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let report = insights(&graph);
    let undeclared = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .map(|insight| insight.message.as_str())
        .collect::<Vec<_>>();
    assert!(undeclared.is_empty(), "{undeclared:?}");
}

#[test]
fn a_rust_use_names_a_crate_only_when_it_is_one() {
    let mut graph = CodeGraph::new("repo");
    let source = graph.add_node(NodeKind::File, "crates/searcher/src/core.rs");
    // ripgrep declares `memmap = { package = "memmap2" }`, and the key is
    // what the code writes.
    let declared = dependency_node(&mut graph, "memmap", "cargo:memmap");
    graph.add_edge(source, declared, EdgeKind::DependsOn, Confidence::Exact);
    for label in [
        "use memmap::Mmap;",
        // An enum's variants brought into scope, not a crate.
        "use FastMatchResult::*;",
        // Handed to a procedural-macro crate the way `std` is to every
        // other.
        "use proc_macro::TokenStream;",
    ] {
        let import = import_node(&mut graph, label, "rust");
        graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let report = insights(&graph);
    let undeclared = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .map(|insight| insight.message.as_str())
        .collect::<Vec<_>>();
    assert!(undeclared.is_empty(), "{undeclared:?}");
}

#[test]
fn a_django_distribution_carries_the_module_it_installs() {
    let mut graph = CodeGraph::new("repo");
    let source = graph.add_node(NodeKind::File, "src/oscar/apps/catalogue/models.py");
    // Django's ecosystem publishes `django-treebeard` for `treebeard` and
    // `sorl-thumbnail` for `sorl`: django-oscar declares six such and the
    // graph called every one of them undeclared.
    for (label, package_id) in [
        ("django-treebeard", "python:django-treebeard"),
        ("sorl-thumbnail", "python:sorl-thumbnail"),
        (
            "django-phonenumber-field",
            "python:django-phonenumber-field",
        ),
    ] {
        let declared = dependency_node(&mut graph, label, package_id);
        graph.add_edge(source, declared, EdgeKind::DependsOn, Confidence::Exact);
    }
    for label in [
        "from treebeard.mp_tree import MP_Node",
        "from sorl.thumbnail import get_thumbnail",
        "from phonenumber_field.modelfields import PhoneNumberField",
        // Nothing declares this one.
        "import unicodecsv",
    ] {
        let import = import_node(&mut graph, label, "python");
        graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let undeclared = insights(&graph)
        .insights
        .into_iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .map(|insight| insight.message)
        .collect::<Vec<_>>();
    assert_eq!(undeclared.len(), 1, "{undeclared:?}");
    assert!(undeclared[0].contains("unicodecsv"), "{undeclared:?}");
}

#[test]
fn an_undeclared_import_names_the_package_that_ships_the_module() {
    let mut graph = CodeGraph::new("repo");
    let source = graph.add_node(NodeKind::File, "src/app.py");
    let example = graph.add_node(NodeKind::File, "examples/demo.py");
    // `import yaml` installs PyYAML, so the declared package covers it.
    let declared = dependency_node(&mut graph, "PyYAML", "python:pyyaml");
    graph.add_edge(source, declared, EdgeKind::DependsOn, Confidence::Exact);
    let yaml = import_node(&mut graph, "import yaml", "python");
    let dotenv = import_node(&mut graph, "from dotenv import load_dotenv", "python");
    let numpy = import_node(&mut graph, "import numpy", "python");
    graph.add_edge(source, yaml, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(source, dotenv, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(example, numpy, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let undeclared = |needle: &str| {
        report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "undeclared_external_import" && insight.message.contains(needle)
            })
            .map(|insight| insight.severity)
    };
    assert_eq!(undeclared("yaml"), None, "PyYAML ships the yaml module");
    // The message names the distribution rather than the module.
    assert_eq!(
        undeclared("python-dotenv"),
        Some(InsightSeverity::Warning),
        "{:?}",
        report
            .insights
            .iter()
            .map(|insight| insight.message.as_str())
            .collect::<Vec<_>>()
    );
    // An example script's dependency is not the program's.
    assert_eq!(undeclared("numpy"), Some(InsightSeverity::Info));
}

#[test]
fn insights_report_undeclared_external_imports() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/main.ts");
    let react = dependency_node(&mut graph, "react", "npm:react");
    graph.add_edge(file, react, EdgeKind::DependsOn, Confidence::Exact);

    let react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
    let express_import = import_node(&mut graph, "import express from \"express\";", "typescript");
    let lodash_require = import_node(&mut graph, "require(\"lodash\")", "javascript");
    let local_python_import = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "import service",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "python".to_string()),
            ("import_scope".to_string(), "local".to_string()),
            ("resolution".to_string(), "resolved".to_string()),
        ]),
    );
    let fs_import = import_node(&mut graph, "import fs from \"node:fs\";", "typescript");
    graph.add_edge(file, react_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        file,
        express_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        file,
        lodash_require,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        file,
        local_python_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(file, fs_import, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("express")
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("lodash")
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("react")
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("service")
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("node:fs")
    }));
}

#[test]
fn insights_report_unused_declared_runtime_dependencies() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let file = graph.add_node(NodeKind::File, "src/main.ts");
    let react = dependency_node(&mut graph, "react", "npm:react");
    let lodash = dependency_node(&mut graph, "lodash", "npm:lodash");
    let vite = dependency_node(&mut graph, "vite", "npm:vite");
    graph.add_edge_with_metadata(
        manifest,
        react,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        lodash,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        vite,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );

    let react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
    graph.add_edge(file, react_import, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let unused = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unused_declared_dependency")
        .expect("expected unused declared dependency insight");

    assert_eq!(unused.severity, InsightSeverity::Info);
    assert!(unused.message.contains("lodash"));
    assert!(unused.nodes.contains(&manifest));
    assert!(unused.nodes.contains(&lodash));
    assert!(!unused.nodes.contains(&react));
    assert!(!unused.nodes.contains(&vite));
    assert_eq!(unused.edges.len(), 1);
}

#[test]
fn unused_dependency_insights_follow_rust_direct_crate_paths() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "Cargo.toml");
    let function = graph.add_node(NodeKind::Function, "load_manifest");
    let toml = dependency_node(&mut graph, "toml", "cargo:toml");
    let serde = dependency_node(&mut graph, "serde", "cargo:serde");
    let call = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "toml::from_str",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "call".to_string()),
            ("language".to_string(), "rust".to_string()),
        ]),
    );
    graph.add_edge(function, call, EdgeKind::Calls, Confidence::Heuristic);
    graph.add_edge_with_metadata(
        manifest,
        toml,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        serde,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );

    let report = insights(&graph);
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&toml)
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&serde)
    }));
}

#[test]
fn dependency_insights_ignore_setup_py_manifest_imports() {
    let mut graph = CodeGraph::new("repo");
    let setup_file = graph.add_node_with_metadata(
        NodeKind::File,
        "setup.py",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    let app_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/app.py",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );
    let requests = dependency_node(&mut graph, "requests", "python:requests");
    let fastapi = dependency_node(&mut graph, "fastapi", "python:fastapi");
    graph.add_edge_with_metadata(
        setup_file,
        requests,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        setup_file,
        fastapi,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );

    let setup_import = import_node(&mut graph, "from setuptools import setup", "python");
    let setup_requests = import_node(&mut graph, "import requests", "python");
    let fastapi_import = import_node(&mut graph, "from fastapi import FastAPI", "python");
    graph.add_edge(
        setup_file,
        setup_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        setup_file,
        setup_requests,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        app_file,
        fastapi_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("setuptools")
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&requests)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&fastapi)
    }));
}

#[test]
fn insights_match_c_family_package_manager_includes() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "vcpkg.json");
    let file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.cpp",
        None,
        BTreeMap::from([("language".to_string(), "cpp".to_string())]),
    );
    let fmt = dependency_node(&mut graph, "fmt", "vcpkg:fmt");
    let zlib = dependency_node(&mut graph, "zlib", "vcpkg:zlib");
    let curl = dependency_node(&mut graph, "curl", "vcpkg:curl");
    let spdlog = dependency_node(&mut graph, "spdlog", "conan:spdlog");
    let cmake = dependency_node(&mut graph, "cmake", "conan:cmake");
    let openssl = dependency_node(&mut graph, "openssl", "cmake:openssl");

    for dependency in [fmt, zlib, curl, spdlog, openssl] {
        graph.add_edge_with_metadata(
            manifest,
            dependency,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
    }
    graph.add_edge_with_metadata(
        manifest,
        cmake,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "build".to_string())]),
    );

    let fmt_include = import_node(&mut graph, "#include <fmt/core.h>", "cpp");
    let zlib_include = import_node(&mut graph, "#include <zlib.h>", "cpp");
    let spdlog_include = import_node(&mut graph, "#include <spdlog/spdlog.h>", "cpp");
    let cmake_include = import_node(&mut graph, "#include <cmake/tool.h>", "cpp");
    let openssl_include = import_node(&mut graph, "#include <openssl/ssl.h>", "cpp");
    graph.add_edge(file, fmt_include, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(file, zlib_include, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        file,
        spdlog_include,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        file,
        cmake_include,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        file,
        openssl_include,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&curl)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&fmt)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&zlib)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&spdlog)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&openssl)
    }));
    let non_runtime = report
        .insights
        .iter()
        .find(|insight| {
            insight.kind == "non_runtime_dependency_import" && insight.nodes.contains(&cmake)
        })
        .expect("expected C++ build dependency import insight");
    assert!(non_runtime.nodes.contains(&file));
    assert!(non_runtime.nodes.contains(&cmake_include));
}

#[test]
fn a_go_import_answers_to_the_module_that_provides_it() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "go.mod");
    let source = graph.add_node_with_metadata(
        NodeKind::File,
        "internal/backend/gcs/client.go",
        None,
        BTreeMap::from([("language".to_string(), "go".to_string())]),
    );
    // terraform declares both, and only the longer one provides the
    // package the file imports.
    let parent = dependency_node(&mut graph, "cloud.google.com/go", "go:cloud.google.com/go");
    let storage = dependency_node(
        &mut graph,
        "cloud.google.com/go/storage",
        "go:cloud.google.com/go/storage",
    );
    graph.add_edge_with_metadata(
        manifest,
        parent,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        storage,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    let import = import_node(&mut graph, "\"cloud.google.com/go/storage\"", "go");
    graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "non_runtime_dependency_import"),
        "the storage module is a runtime requirement"
    );
}

#[test]
fn a_go_file_is_answered_by_its_own_module() {
    let mut graph = CodeGraph::new("repo");
    // terraform's GCS backend is a module of its own: its go.mod requires
    // the storage module outright, while the root manifest beside it marks
    // the same module indirect.
    let root_manifest = graph.add_node(NodeKind::File, "go.mod");
    let backend_manifest =
        graph.add_node(NodeKind::File, "internal/backend/remote-state/gcs/go.mod");
    let source = graph.add_node_with_metadata(
        NodeKind::File,
        "internal/backend/remote-state/gcs/client.go",
        None,
        BTreeMap::from([("language".to_string(), "go".to_string())]),
    );
    let storage = dependency_node(
        &mut graph,
        "cloud.google.com/go/storage",
        "go:cloud.google.com/go/storage",
    );
    graph.add_edge_with_metadata(
        root_manifest,
        storage,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "indirect".to_string())]),
    );
    graph.add_edge_with_metadata(
        backend_manifest,
        storage,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    let import = import_node(&mut graph, "\"cloud.google.com/go/storage\"", "go");
    graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "non_runtime_dependency_import"),
        "the module that builds this file requires the package outright"
    );
}

#[test]
fn go_scaffolding_that_serves_tests_declares_no_routes_of_the_program() {
    let mut graph = CodeGraph::new("repo");
    // terraform stands up two `httptest` servers in files Go does not name
    // as tests; both register `/api/v2/ping`, and neither is the program.
    for path in [
        "internal/command/cloud_mock.go",
        "internal/backend/remote/testing.go",
    ] {
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            path,
            None,
            BTreeMap::from([("language".to_string(), "go".to_string())]),
        );
        let import = import_node(&mut graph, "\"testing\"", "go");
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
        let route = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("route:{path}"),
            Some(SourceSpan {
                path: path.to_string(),
                start_line: 30,
                start_column: 0,
                end_line: 30,
                end_column: 0,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("path".to_string(), "/api/v2/ping".to_string()),
                ("method".to_string(), "get".to_string()),
            ]),
        );
        graph.add_edge(graph.root, route, EdgeKind::Entrypoint, Confidence::Exact);
    }

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "duplicate_framework_route"),
        "two mock servers are not one routing table"
    );
}

#[test]
fn two_go_modules_do_not_disagree_with_each_other() {
    let mut graph = CodeGraph::new("repo");
    // gqlgen keeps a module per example, and each pins its own versions.
    let root = graph.add_node(NodeKind::File, "go.mod");
    let example = graph.add_node(NodeKind::File, "_examples/go.mod");
    let text = dependency_node(&mut graph, "golang.org/x/text", "go:golang.org/x/text");
    for (manifest, version) in [(root, "v0.41.0"), (example, "v0.38.0")] {
        graph.add_edge_with_metadata(
            manifest,
            text,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "runtime".to_string()),
                ("dependency_version".to_string(), version.to_string()),
            ]),
        );
    }

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "conflicting_dependency_declaration"),
        "neither build ever sees the other's requirement"
    );
}

#[test]
fn one_missing_env_file_is_one_finding() {
    let mut graph = CodeGraph::new("repo");
    let compose = graph.add_node(NodeKind::File, "docker-compose.yml");
    for service in ["app", "db", "redis"] {
        let env = graph.add_node_with_metadata(
            NodeKind::Config,
            format!("compose env_file:{service}"),
            Some(SourceSpan {
                path: "docker-compose.yml".to_string(),
                start_line: 3,
                start_column: 0,
                end_line: 3,
                end_column: 0,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "compose_env_file".to_string()),
                ("env_file_path".to_string(), ".env".to_string()),
                ("service".to_string(), service.to_string()),
            ]),
        );
        graph.add_edge_with_metadata(
            compose,
            env,
            EdgeKind::ReadsConfig,
            Confidence::Exact,
            BTreeMap::from([("relation".to_string(), "compose_env_file".to_string())]),
        );
    }

    let report = insights(&graph);
    let findings: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "unresolved_compose_env_file_path")
        .collect();

    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(findings[0].message.contains("`app`"), "{findings:?}");
    assert!(findings[0].message.contains("`redis`"), "{findings:?}");
}

#[test]
fn insights_report_conflicting_dependency_declarations() {
    let mut graph = CodeGraph::new("repo");
    let root_manifest = graph.add_node(NodeKind::File, "Cargo.toml");
    let app_manifest = graph.add_node(NodeKind::File, "crates/app/Cargo.toml");
    let lockfile = graph.add_node(NodeKind::File, "Cargo.lock");
    let serde = dependency_node(&mut graph, "serde", "cargo:serde");
    let anyhow = dependency_node(&mut graph, "anyhow", "cargo:anyhow");
    graph.add_edge_with_metadata(
        root_manifest,
        serde,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "1".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        app_manifest,
        serde,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "2".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        lockfile,
        serde,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "1.2.3".to_string()),
            ("dependency_version_kind".to_string(), "locked".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        app_manifest,
        anyhow,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "1".to_string()),
        ]),
    );

    let report = insights(&graph);
    let conflict = report
        .insights
        .iter()
        .find(|insight| insight.kind == "conflicting_dependency_declaration")
        .expect("expected conflicting dependency declaration insight");

    assert_eq!(conflict.severity, InsightSeverity::Warning);
    assert!(conflict.message.contains("serde"));
    assert!(conflict.message.contains("`1`"));
    assert!(conflict.message.contains("`2`"));
    assert!(conflict.nodes.contains(&root_manifest));
    assert!(conflict.nodes.contains(&app_manifest));
    assert!(conflict.nodes.contains(&serde));
    assert!(!conflict.nodes.contains(&anyhow));
    assert_eq!(conflict.edges.len(), 2);
    assert!(
        conflict
            .message
            .contains("`Cargo.toml`, `crates/app/Cargo.toml`"),
        "which files disagree is the whole of what a reader needs: {}",
        conflict.message
    );
}

#[test]
fn an_unreachable_file_says_what_it_offers() {
    // Most files a library never reaches from an entrypoint are its API:
    // 129 of okio's 176, 383 of terraform's 500. "Contains code but is not
    // reachable" reads as dead code for all of them.
    let mut graph = CodeGraph::new("repo");
    let entry = graph.add_node(NodeKind::Entrypoint, "cargo bin:demo");
    let main_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, entry, EdgeKind::Entrypoint, Confidence::Exact);
    graph.add_edge(entry, main, EdgeKind::References, Confidence::Exact);
    graph.add_edge(main_file, main, EdgeKind::Contains, Confidence::Exact);

    let api_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/api.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let offered = graph.add_node_with_metadata(
        NodeKind::Function,
        "parse",
        None,
        BTreeMap::from([("visibility".to_string(), "public".to_string())]),
    );
    graph.add_edge(api_file, offered, EdgeKind::Contains, Confidence::Exact);

    let dead_file = graph.add_node_with_metadata(
        NodeKind::File,
        "src/dead.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    let hidden = graph.add_node_with_metadata(
        NodeKind::Function,
        "forgotten",
        None,
        BTreeMap::from([("visibility".to_string(), "private".to_string())]),
    );
    graph.add_edge(dead_file, hidden, EdgeKind::Contains, Confidence::Exact);

    let report = insights(&graph);
    let message_for = |path: &str| -> String {
        report
            .insights
            .iter()
            .find(|insight| {
                insight.kind == "unreachable_source_file" && insight.message.contains(path)
            })
            .map(|insight| insight.message.clone())
            .unwrap_or_else(|| format!("no finding for {path}"))
    };
    assert!(
        message_for("src/api.rs").contains("one of its functions is exported"),
        "{}",
        message_for("src/api.rs")
    );
    assert!(
        !message_for("src/dead.rs").contains("exported"),
        "{}",
        message_for("src/dead.rs")
    );
}

#[test]
fn an_orphan_says_whether_it_is_dead_or_the_api() {
    // A function nobody in the repository calls is either dead code or the
    // API. Terraform has 11406 exported functions with no in-repo caller
    // against 592 unexported ones, and one sentence for both says nothing
    // a reader can act on.
    let mut graph = CodeGraph::new("repo");
    graph.add_node_with_metadata(
        NodeKind::Function,
        "ParseAddress",
        None,
        BTreeMap::from([("visibility".to_string(), "public".to_string())]),
    );
    graph.add_node_with_metadata(
        NodeKind::Function,
        "parseInternal",
        None,
        BTreeMap::from([("visibility".to_string(), "private".to_string())]),
    );

    let report = insights(&graph);
    let orphans: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "orphan_function")
        .map(|insight| insight.message.as_str())
        .collect();
    assert_eq!(orphans.len(), 2, "{orphans:?}");
    assert!(
        orphans
            .iter()
            .any(|message| message.contains("`ParseAddress`") && message.contains("exported")),
        "{orphans:?}"
    );
    assert!(
        orphans
            .iter()
            .any(|message| message.contains("`parseInternal`") && !message.contains("exported")),
        "{orphans:?}"
    );
}

#[test]
fn a_catalogued_version_does_not_disagree_with_itself() {
    // `catalog:` says the version lives in the pnpm catalog. Read as a
    // constraint it disagrees with every real one, and vue core had five
    // packages arguing with themselves that way.
    let mut graph = CodeGraph::new("repo");
    let root = graph.add_node(NodeKind::File, "package.json");
    let package = graph.add_node(NodeKind::File, "packages/app/package.json");
    let vite = dependency_node(&mut graph, "@vitejs/plugin-vue", "npm:@vitejs/plugin-vue");
    for (manifest, version) in [(root, "^6.0.8"), (package, "catalog:")] {
        graph.add_edge_with_metadata(
            manifest,
            vite,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), "dev".to_string()),
                ("dependency_version".to_string(), version.to_string()),
            ]),
        );
    }

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "conflicting_dependency_declaration"),
        "{:?}",
        report.insights
    );
}

#[test]
fn insights_ignore_locked_versions_as_conflicting_constraints() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let lockfile = graph.add_node(NodeKind::File, "package-lock.json");
    let react = dependency_node(&mut graph, "react", "npm:react");
    graph.add_edge_with_metadata(
        manifest,
        react,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "^19.0.0".to_string()),
            (
                "dependency_version_kind".to_string(),
                "constraint".to_string(),
            ),
        ]),
    );
    graph.add_edge_with_metadata(
        lockfile,
        react,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([
            ("dependency_kind".to_string(), "runtime".to_string()),
            ("dependency_version".to_string(), "19.0.0".to_string()),
            ("dependency_version_kind".to_string(), "locked".to_string()),
        ]),
    );

    let report = insights(&graph);
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "conflicting_dependency_declaration")
    );
}

#[test]
fn insights_report_mixed_dependency_scopes() {
    // One manifest asking for a package twice, in two scopes, is something
    // a person wrote down and can fix. Two manifests disagreeing is a
    // workspace where each project decided for itself, and a lockfile
    // holding both scopes is what resolving that workspace produced.
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let workspace_manifest = graph.add_node(NodeKind::File, "packages/app/package.json");
    let lock = graph.add_node(NodeKind::File, "pnpm-lock.yaml");
    let react = dependency_node(&mut graph, "react", "npm:react");
    let lodash = dependency_node(&mut graph, "lodash", "npm:lodash");
    let vue = dependency_node(&mut graph, "vue", "npm:vue");

    let declare = |graph: &mut CodeGraph, source, target, scope: &str| {
        graph.add_edge_with_metadata(
            source,
            target,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([
                ("dependency_kind".to_string(), scope.to_string()),
                ("dependency_version".to_string(), "^18".to_string()),
            ]),
        );
    };
    declare(&mut graph, manifest, react, "runtime");
    declare(&mut graph, manifest, react, "dev");
    declare(&mut graph, manifest, lodash, "runtime");
    declare(&mut graph, workspace_manifest, lodash, "dev");
    declare(&mut graph, lock, vue, "runtime");
    declare(&mut graph, lock, vue, "dev");

    let report = insights(&graph);
    let mixed = report
        .insights
        .iter()
        .find(|insight| insight.kind == "mixed_dependency_scope")
        .expect("expected mixed dependency scope insight");

    assert_eq!(mixed.severity, InsightSeverity::Warning);
    assert!(mixed.message.contains("react"));
    assert!(
        mixed.message.contains("`package.json`"),
        "{}",
        mixed.message
    );
    assert!(mixed.message.contains("`runtime`"));
    assert!(mixed.message.contains("`dev`"));
    assert!(mixed.nodes.contains(&manifest));
    assert!(mixed.nodes.contains(&react));
    assert!(!mixed.nodes.contains(&lodash));
    assert!(!mixed.nodes.contains(&workspace_manifest));
    assert_eq!(mixed.edges.len(), 2);
    assert_eq!(report.by_kind.get("mixed_dependency_scope"), Some(&1));
    assert!(
        !report
            .insights
            .iter()
            .any(|insight| insight.kind == "conflicting_dependency_declaration")
    );
}
#[test]
fn a_configuration_with_a_name_of_its_own_is_still_a_configuration() {
    // koel builds its service worker with `vite.config.sw.js`, which is a
    // configuration for vite however many names it carries.
    for path in [
        "vite.config.sw.js",
        "jest.config.base.ts",
        "packages/app/vite.config.ts",
        ".eslintrc.js",
    ] {
        assert!(is_tool_configuration_source_path(path), "{path}");
    }
    for path in ["src/config.ts", "app/config/database.js", "src/main.ts"] {
        assert!(!is_tool_configuration_source_path(path), "{path}");
    }
}

#[test]
fn a_package_that_autoloads_the_namespace_is_the_one_that_declares_it() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "composer.json");
    let framework = dependency_node(
        &mut graph,
        "laravel/framework",
        "composer:laravel/framework",
    );
    graph.nodes[framework.0 as usize - 1].metadata.insert(
        "autoloaded_namespaces".to_string(),
        "Illuminate\\".to_string(),
    );
    graph.add_edge_with_metadata(
        manifest,
        framework,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    let source = graph.add_node_with_metadata(
        NodeKind::File,
        "app/Event.php",
        None,
        BTreeMap::from([("language".to_string(), "php".to_string())]),
    );
    let broadcasting = import_node(&mut graph, "use Illuminate\\Broadcasting\\Channel;", "php");
    let permission = import_node(&mut graph, "use Spatie\\Permission\\Models\\Role;", "php");
    graph.add_edge(
        source,
        broadcasting,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(source, permission, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let undeclared: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .map(|insight| insight.message.as_str())
        .collect();

    // The framework autoloads `Illuminate\`, and nothing autoloads
    // `Spatie\Permission\`.
    assert_eq!(undeclared.len(), 1, "{undeclared:?}");
    assert!(undeclared[0].contains("spatie"), "{undeclared:?}");
}

#[test]
fn a_package_the_program_runs_without_need_not_be_declared() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "pyproject.toml");
    let declared = dependency_node(&mut graph, "urllib3", "python:urllib3");
    graph.add_edge_with_metadata(
        manifest,
        declared,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    let compat = graph.add_node_with_metadata(
        NodeKind::File,
        "src/requests/compat.py",
        None,
        BTreeMap::from([("language".to_string(), "python".to_string())]),
    );

    // requests opens with `try: import simplejson`, and imports it again
    // below under `if has_simplejson:`. The first says the program runs
    // without it, which settles the package.
    let guarded = graph.add_node_with_metadata(
        NodeKind::ExternalDependency,
        "import simplejson as json",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "import".to_string()),
            ("language".to_string(), "python".to_string()),
            ("optional".to_string(), "true".to_string()),
        ]),
    );
    let plain = import_node(
        &mut graph,
        "from simplejson import JSONDecodeError",
        "python",
    );
    graph.add_edge(compat, guarded, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(compat, plain, EdgeKind::Imports, Confidence::Syntactic);

    // And a package nothing guards is still reported.
    let missing = import_node(&mut graph, "import chardet", "python");
    graph.add_edge(compat, missing, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let packages: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .map(|insight| insight.message.as_str())
        .collect();

    assert_eq!(packages.len(), 1, "{packages:?}");
    assert!(packages[0].contains("chardet"), "{packages:?}");
}

#[test]
fn a_package_that_says_what_it_ships_is_believed_about_what_it_does_not() {
    let mut graph = CodeGraph::new("repo");
    // openzeppelin publishes its contracts and nothing else; its hardhat
    // plugins are tooling, and `dev` is where their packages belong.
    let manifest = graph.add_node_with_metadata(
        NodeKind::File,
        "package.json",
        None,
        BTreeMap::from([(
            "published_paths".to_string(),
            "/contracts/**/*.sol\n!/contracts/mocks/**/*".to_string(),
        )]),
    );
    let shipped = graph.add_node_with_metadata(
        NodeKind::File,
        "contracts/token/ERC20.sol",
        None,
        BTreeMap::from([("language".to_string(), "solidity".to_string())]),
    );
    let plugin = graph.add_node_with_metadata(
        NodeKind::File,
        "hardhat/plugin.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let mock = graph.add_node_with_metadata(
        NodeKind::File,
        "contracts/mocks/AccountMock.sol",
        None,
        BTreeMap::from([("language".to_string(), "solidity".to_string())]),
    );
    let hardhat = dependency_node(&mut graph, "hardhat", "npm:hardhat");
    graph.add_edge_with_metadata(
        manifest,
        hardhat,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );
    for source in [shipped, plugin, mock] {
        let import = import_node(&mut graph, "import \"hardhat\";", "typescript");
        graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let report = insights(&graph);
    let messages: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "non_runtime_dependency_import")
        .map(|insight| insight.message.as_str())
        .collect();

    // The contract ships, so importing a dev package there is a finding.
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].contains("contracts/token/ERC20.sol"),
        "{messages:?}"
    );
    // The plugin is outside what the package publishes, and the mock is
    // excluded by name.
    assert!(!messages[0].contains("hardhat/plugin.ts"), "{messages:?}");
    assert!(!messages[0].contains("AccountMock"), "{messages:?}");
}

#[test]
fn a_package_that_publishes_only_a_build_product_says_nothing_about_its_sources() {
    let mut graph = CodeGraph::new("repo");
    // vue's compiler-sfc publishes `dist`, which the scan never held. Its
    // sources still ship, bundled, so the findings there stand.
    let manifest = graph.add_node_with_metadata(
        NodeKind::File,
        "packages/compiler-sfc/package.json",
        None,
        BTreeMap::from([("published_paths".to_string(), "dist".to_string())]),
    );
    let source = graph.add_node_with_metadata(
        NodeKind::File,
        "packages/compiler-sfc/src/cache.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let lru = dependency_node(&mut graph, "lru-cache", "npm:lru-cache");
    graph.add_edge_with_metadata(
        manifest,
        lru,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );
    let import = import_node(&mut graph, "import LRU from \"lru-cache\";", "typescript");
    graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    assert_eq!(
        report.by_kind.get("non_runtime_dependency_import"),
        Some(&1)
    );
}

#[test]
fn insights_report_non_runtime_dependency_imports_from_production_sources() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let app = graph.add_node_with_metadata(
        NodeKind::File,
        "src/app.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let test = graph.add_node_with_metadata(
        NodeKind::File,
        "tests/app.test.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let spec = graph.add_node_with_metadata(
        NodeKind::File,
        "src/__tests__/setup.spec.tsx",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let react = dependency_node(&mut graph, "react", "npm:react");
    let vite = dependency_node(&mut graph, "vite", "npm:vite");
    graph.add_edge_with_metadata(
        manifest,
        react,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        vite,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );

    let app_vite_import = import_node(
        &mut graph,
        "import { defineConfig } from \"vite\";",
        "typescript",
    );
    let app_react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
    let test_vite_import = import_node(&mut graph, "import { test } from \"vite\";", "typescript");
    let spec_vite_import = import_node(
        &mut graph,
        "import { defineConfig } from \"vite\";",
        "typescript",
    );
    graph.add_edge(
        app,
        app_vite_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        app,
        app_react_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        test,
        test_vite_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        spec,
        spec_vite_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "non_runtime_dependency_import")
        .expect("expected non-runtime dependency import insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("src/app.ts"));
    assert!(insight.message.contains("vite"));
    assert!(insight.message.contains("`dev`"));
    assert!(insight.nodes.contains(&app));
    assert!(insight.nodes.contains(&app_vite_import));
    assert!(insight.nodes.contains(&manifest));
    assert!(insight.nodes.contains(&vite));
    assert!(!insight.nodes.contains(&app_react_import));
    assert!(!insight.nodes.contains(&test));
    assert!(!insight.nodes.contains(&test_vite_import));
    assert!(!insight.nodes.contains(&spec));
    assert!(!insight.nodes.contains(&spec_vite_import));
    assert_eq!(
        report.by_kind.get("non_runtime_dependency_import"),
        Some(&1)
    );
    // One finding per package: six of vue's files import `vitest` and five
    // import `picocolors`, which is two facts rather than eleven.
    assert!(
        insight.message.starts_with("`src/app.ts` imports `vite`"),
        "{}",
        insight.message
    );
}

#[test]
fn insights_report_go_indirect_dependency_imports_from_production_sources() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "go.mod");
    let app = graph.add_node_with_metadata(
        NodeKind::File,
        "cmd/server/main.go",
        None,
        BTreeMap::from([("language".to_string(), "go".to_string())]),
    );
    let sys = dependency_node(&mut graph, "golang.org/x/sys", "go:golang.org/x/sys");
    graph.add_edge_with_metadata(
        manifest,
        sys,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "indirect".to_string())]),
    );

    let unix_import = import_node(&mut graph, "import \"golang.org/x/sys/unix\"", "go");
    graph.add_edge(app, unix_import, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "non_runtime_dependency_import")
        .expect("expected direct import of indirect Go dependency insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("cmd/server/main.go"));
    assert!(insight.message.contains("golang.org/x/sys"));
    assert!(insight.message.contains("`indirect`"));
    assert!(insight.nodes.contains(&manifest));
    assert!(insight.nodes.contains(&app));
    assert!(insight.nodes.contains(&sys));
    assert!(insight.nodes.contains(&unix_import));
    assert_eq!(
        report.by_kind.get("non_runtime_dependency_import"),
        Some(&1)
    );
}

#[test]
fn insights_report_runtime_dependencies_used_only_by_tests() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let test = graph.add_node_with_metadata(
        NodeKind::File,
        "tests/app.test.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let app = graph.add_node_with_metadata(
        NodeKind::File,
        "src/app.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let jest = dependency_node(&mut graph, "jest", "npm:jest");
    let react = dependency_node(&mut graph, "react", "npm:react");
    let vite = dependency_node(&mut graph, "vite", "npm:vite");
    graph.add_edge_with_metadata(
        manifest,
        jest,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        react,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        vite,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );

    let jest_import = import_node(
        &mut graph,
        "import { describe } from \"jest\";",
        "typescript",
    );
    let react_import = import_node(&mut graph, "import React from \"react\";", "typescript");
    let vite_import = import_node(&mut graph, "import { test } from \"vite\";", "typescript");
    graph.add_edge(test, jest_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(app, react_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(test, vite_import, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "test_only_runtime_dependency")
        .expect("expected test-only runtime dependency insight");

    assert_eq!(insight.severity, InsightSeverity::Info);
    assert!(insight.message.contains("jest"));
    assert!(insight.nodes.contains(&manifest));
    assert!(insight.nodes.contains(&jest));
    assert!(insight.nodes.contains(&test));
    assert!(insight.nodes.contains(&jest_import));
    assert!(!insight.nodes.contains(&app));
    assert!(!insight.nodes.contains(&react));
    assert!(!insight.nodes.contains(&vite));
    assert_eq!(report.by_kind.get("test_only_runtime_dependency"), Some(&1));
}

#[test]
fn insights_match_dart_pubspec_dependency_scopes() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "pubspec.yaml");
    let app = graph.add_node_with_metadata(
        NodeKind::File,
        "lib/main.dart",
        None,
        BTreeMap::from([("language".to_string(), "dart".to_string())]),
    );
    let test = graph.add_node_with_metadata(
        NodeKind::File,
        "test/widget_test.dart",
        None,
        BTreeMap::from([("language".to_string(), "dart".to_string())]),
    );
    let generated = graph.add_node_with_metadata(
        NodeKind::File,
        "lib/src/user.freezed.dart",
        None,
        BTreeMap::from([("language".to_string(), "dart".to_string())]),
    );
    let http = dependency_node(&mut graph, "http", "dart:http");
    let build_runner = dependency_node(&mut graph, "build_runner", "dart:build_runner");
    let test_dep = dependency_node(&mut graph, "test", "dart:test");
    let collection = dependency_node(&mut graph, "collection", "dart:collection");
    graph.add_edge_with_metadata(
        manifest,
        http,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        build_runner,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        test_dep,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    graph.add_edge_with_metadata(
        manifest,
        collection,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );

    let http_import = import_node(&mut graph, "import 'package:http/http.dart';", "dart");
    let build_runner_import = import_node(
        &mut graph,
        "import 'package:build_runner/build_runner.dart';",
        "dart",
    );
    let test_import = import_node(&mut graph, "import 'package:test/test.dart';", "dart");
    let undeclared_import = import_node(
        &mut graph,
        "import 'package:riverpod/riverpod.dart';",
        "dart",
    );
    let sdk_import = import_node(&mut graph, "import 'dart:io';", "dart");
    let generated_build_import = import_node(
        &mut graph,
        "import 'package:build_runner/build_runner.dart';",
        "dart",
    );
    graph.add_edge(app, http_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        app,
        build_runner_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(test, test_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        app,
        undeclared_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(app, sdk_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        generated,
        generated_build_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("http")
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.message.contains("dart:io")
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import"
            && insight.message.contains("riverpod")
            && insight.nodes.contains(&undeclared_import)
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&collection)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&http)
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "non_runtime_dependency_import"
            && insight.nodes.contains(&build_runner_import)
            && !insight.nodes.contains(&generated_build_import)
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "test_only_runtime_dependency"
            && insight.nodes.contains(&test_dep)
            && insight.nodes.contains(&test_import)
    }));
}

#[test]
fn test_like_source_paths_cover_common_language_conventions() {
    for path in [
        "src/__tests__/app.spec.tsx",
        "crates/codegraph-indexer/src/tests.rs",
        "web/components/Button.test.jsx",
        "internal/server/server_test.go",
        "tests/test_api.py",
        "tests/api_test.py",
        "src/FooTest.php",
        "src/FooSpec.php",
        "native/foo_test.cpp",
        "native/test_parser.cc",
        "scripts/deploy_test.sh",
        "scripts/deploy.bats",
        "pkg/testdata/input.go",
        "test/widget_test.dart",
        "integration_test/app_test.dart",
        "lib/src/user.g.dart",
        "lib/src/user.freezed.dart",
        "lib/generated/assets.gen.dart",
        ".dart_tool/build/generated/app/lib/main.dart",
    ] {
        assert!(
            is_test_like_source_path(path),
            "expected test-like path: {path}"
        );
    }

    for path in [
        "src/app.ts",
        "src/context.php",
        "src/contest.php",
        "cmd/server/main.go",
        "native/parser.cpp",
        "scripts/deploy.sh",
        "lib/main.dart",
        "lib/src/user.dart",
    ] {
        assert!(
            !is_test_like_source_path(path),
            "expected production-like path: {path}"
        );
    }
}

#[test]
fn insights_report_duplicate_framework_routes() {
    let mut graph = CodeGraph::new("repo");
    let span = |path: &str| SourceSpan {
        path: path.to_string(),
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 1,
    };
    let first = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route GET /users",
        Some(span("src/routes.py")),
        BTreeMap::from([
            ("item_kind".to_string(), "framework_route".to_string()),
            ("method".to_string(), "GET".to_string()),
            ("path".to_string(), "/users".to_string()),
            ("handler".to_string(), "list_users".to_string()),
        ]),
    );
    let second = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route GET /users",
        Some(span("src/legacy.py")),
        BTreeMap::from([
            ("item_kind".to_string(), "framework_route".to_string()),
            ("method".to_string(), "GET".to_string()),
            ("path".to_string(), "/users".to_string()),
            ("handler".to_string(), "legacy_users".to_string()),
        ]),
    );
    let post = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route POST /users",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "framework_route".to_string()),
            ("method".to_string(), "POST".to_string()),
            ("path".to_string(), "/users".to_string()),
            ("handler".to_string(), "create_user".to_string()),
        ]),
    );
    let list_users = graph.add_node(NodeKind::Function, "list_users");
    let legacy_users = graph.add_node(NodeKind::Function, "legacy_users");
    graph.add_edge(
        first,
        list_users,
        EdgeKind::References,
        Confidence::Syntactic,
    );
    graph.add_edge(
        second,
        legacy_users,
        EdgeKind::References,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let duplicate = report
        .insights
        .iter()
        .find(|insight| insight.kind == "duplicate_framework_route")
        .expect("expected duplicate route insight");

    assert_eq!(duplicate.severity, InsightSeverity::Warning);
    assert!(duplicate.message.contains("GET /users"));
    assert!(duplicate.message.contains("list_users"));
    assert!(duplicate.message.contains("legacy_users"));
    // Which files declare it is what tells a conflict from two programs:
    // terraform's duplicates are separate mock servers, one per package.
    assert!(
        duplicate
            .message
            .contains("`src/legacy.py`, `src/routes.py`"),
        "{}",
        duplicate.message
    );
    assert!(duplicate.nodes.contains(&first));
    assert!(duplicate.nodes.contains(&second));
    assert!(!duplicate.nodes.contains(&post));
    assert_eq!(duplicate.edges.len(), 2);
}

#[test]
fn insights_report_unresolved_framework_route_handlers() {
    let mut graph = CodeGraph::new("repo");
    let unresolved = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route GET /missing",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "framework_route".to_string()),
            ("framework".to_string(), "fastapi".to_string()),
            ("method".to_string(), "GET".to_string()),
            ("path".to_string(), "/missing".to_string()),
            ("handler".to_string(), "missing_handler".to_string()),
        ]),
    );
    let resolved = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route POST /users",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "framework_route".to_string()),
            ("framework".to_string(), "fastapi".to_string()),
            ("method".to_string(), "POST".to_string()),
            ("path".to_string(), "/users".to_string()),
            ("handler".to_string(), "create_user".to_string()),
        ]),
    );
    let inline = graph.add_node_with_metadata(
        NodeKind::Entrypoint,
        "route GET /inline",
        None,
        BTreeMap::from([
            ("item_kind".to_string(), "framework_route".to_string()),
            ("framework".to_string(), "express".to_string()),
            ("method".to_string(), "GET".to_string()),
            ("path".to_string(), "/inline".to_string()),
        ]),
    );
    let file = graph.add_node(NodeKind::File, "api.py");
    let handler = graph.add_node(NodeKind::Function, "create_user");
    graph.add_edge(
        graph.root,
        unresolved,
        EdgeKind::Entrypoint,
        Confidence::Syntactic,
    );
    graph.add_edge(
        graph.root,
        resolved,
        EdgeKind::Entrypoint,
        Confidence::Syntactic,
    );
    graph.add_edge(
        graph.root,
        inline,
        EdgeKind::Entrypoint,
        Confidence::Syntactic,
    );
    graph.add_edge_with_metadata(
        unresolved,
        file,
        EdgeKind::References,
        Confidence::Syntactic,
        BTreeMap::from([("resolution".to_string(), "framework_route_file".to_string())]),
    );
    graph.add_edge_with_metadata(
        resolved,
        handler,
        EdgeKind::References,
        Confidence::Syntactic,
        BTreeMap::from([(
            "resolution".to_string(),
            "framework_route_handler".to_string(),
        )]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "unresolved_framework_route_handler")
        .expect("expected unresolved route handler insight");

    assert_eq!(insight.severity, InsightSeverity::Warning);
    assert!(insight.message.contains("GET /missing"));
    assert!(insight.message.contains("missing_handler"));
    assert!(insight.nodes.contains(&unresolved));
    assert!(!insight.nodes.contains(&resolved));
    assert!(!insight.nodes.contains(&inline));
    assert_eq!(insight.edges.len(), 2);
    assert_eq!(
        report.by_kind.get("unresolved_framework_route_handler"),
        Some(&1)
    );
}

#[test]
fn insights_report_custom_rule_violations() {
    let mut graph = CodeGraph::new("repo");
    let caller = graph.add_node(NodeKind::Function, "render");
    let callee = graph.add_node(NodeKind::Function, "query_user");
    graph.add_edge(caller, callee, EdgeKind::Calls, Confidence::Heuristic);
    let violated_edge_index = graph.edges.len() - 1;
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "custom_rule_violation".to_string());
    metadata.insert("rule_id".to_string(), "ui-cannot-call-db".to_string());
    metadata.insert("rule_kind".to_string(), "forbidden_edge".to_string());
    metadata.insert("severity".to_string(), "error".to_string());
    metadata.insert(
        "message".to_string(),
        "UI layer must not call database layer directly".to_string(),
    );
    metadata.insert(
        "violated_edge_index".to_string(),
        violated_edge_index.to_string(),
    );
    let violation = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "custom rule violation:no-left-pad",
        None,
        metadata,
    );
    graph.add_edge(violation, caller, EdgeKind::References, Confidence::Exact);
    graph.add_edge(violation, callee, EdgeKind::References, Confidence::Exact);

    let report = insights(&graph);
    let custom = report
        .insights
        .iter()
        .find(|insight| insight.kind == "custom_rule_forbidden_edge")
        .expect("expected custom rule insight");

    assert_eq!(custom.severity, InsightSeverity::Error);
    assert_eq!(
        custom.message,
        "UI layer must not call database layer directly"
    );
    assert_eq!(custom.nodes, vec![violation]);
    assert!(custom.edges.contains(&violated_edge_index));
    assert_eq!(custom.edges.len(), 3);
}

#[test]
fn filter_insight_report_filters_and_limits_results() {
    let report = InsightReport {
        total: 3,
        by_severity: BTreeMap::from([("error".to_string(), 1), ("warning".to_string(), 2)]),
        by_kind: BTreeMap::from([
            ("dependency_cycle".to_string(), 1),
            ("parse_error".to_string(), 1),
            ("undeclared_external_import".to_string(), 1),
        ]),
        insights: vec![
            Insight {
                kind: "dependency_cycle".to_string(),
                severity: InsightSeverity::Warning,
                message: "cycle through service".to_string(),
                nodes: vec![NodeId(1)],
                edges: vec![10],
            },
            Insight {
                kind: "undeclared_external_import".to_string(),
                severity: InsightSeverity::Warning,
                message: "imports express".to_string(),
                nodes: vec![NodeId(2)],
                edges: vec![11],
            },
            Insight {
                kind: "parse_error".to_string(),
                severity: InsightSeverity::Error,
                message: "broken file".to_string(),
                nodes: vec![NodeId(3)],
                edges: Vec::new(),
            },
        ],
    };

    let filtered = filter_insight_report(
        report,
        &InsightFilter {
            severity: Some(InsightSeverity::Warning),
            kind: Some("dependency".to_string()),
            search: Some("cycle".to_string()),
            limit: 1,
        },
    );

    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.by_severity.get("error"), None);
    assert_eq!(filtered.by_severity.get("warning"), Some(&1));
    assert_eq!(filtered.by_kind.get("dependency_cycle"), Some(&1));
    assert_eq!(filtered.by_kind.get("parse_error"), None);
    assert_eq!(filtered.insights.len(), 1);
    assert_eq!(filtered.insights[0].kind, "dependency_cycle");
}

#[test]
fn check_insights_respects_severity_thresholds() {
    let report = InsightReport {
        total: 6,
        by_severity: BTreeMap::from([
            ("info".to_string(), 3),
            ("warning".to_string(), 2),
            ("error".to_string(), 1),
        ]),
        by_kind: BTreeMap::new(),
        insights: Vec::new(),
    };

    let error_check = check_insights(report.clone(), InsightSeverity::Error);
    assert!(!error_check.passed);
    assert_eq!(error_check.fail_on, "error");
    assert_eq!(error_check.failing_insights, 1);

    let warning_check = check_insights(report.clone(), InsightSeverity::Warning);
    assert!(!warning_check.passed);
    assert_eq!(warning_check.failing_insights, 3);

    let clean_report = InsightReport {
        total: 3,
        by_severity: BTreeMap::from([("info".to_string(), 3)]),
        by_kind: BTreeMap::new(),
        insights: Vec::new(),
    };
    let clean_check = check_insights(clean_report, InsightSeverity::Warning);
    assert!(clean_check.passed);
    assert_eq!(clean_check.failing_insights, 0);
}

#[test]
fn bounded_quality_gate_caps_sample_and_keeps_totals() {
    let severities = [
        InsightSeverity::Info,
        InsightSeverity::Warning,
        InsightSeverity::Error,
    ];
    let insights: Vec<Insight> = (0..30)
        .map(|index| Insight {
            kind: format!("kind_{index}"),
            severity: severities[index % 3],
            message: format!("finding {index}"),
            nodes: Vec::new(),
            edges: Vec::new(),
        })
        .collect();
    let report = InsightReport {
        total: 30,
        by_severity: BTreeMap::from([
            ("error".to_string(), 10),
            ("warning".to_string(), 10),
            ("info".to_string(), 10),
        ]),
        by_kind: BTreeMap::new(),
        insights,
    };

    let gate = bounded_quality_gate(check_insights(report, InsightSeverity::Warning), 25);

    assert!(!gate.passed);
    assert_eq!(gate.failing_insights, 20);
    assert_eq!(gate.report.total, 30);
    assert_eq!(gate.report.by_severity.get("info"), Some(&10));
    assert_eq!(gate.report.insights.len(), 25);
    assert!(
        gate.report.insights[..10]
            .iter()
            .all(|insight| insight.severity == InsightSeverity::Error)
    );
    assert!(
        gate.report.insights[10..20]
            .iter()
            .all(|insight| insight.severity == InsightSeverity::Warning)
    );
    assert!(
        gate.report.insights[20..]
            .iter()
            .all(|insight| insight.severity == InsightSeverity::Info)
    );
}

#[test]
fn insights_report_skipped_large_files() {
    let mut graph = CodeGraph::new("repo");
    let skipped = |graph: &mut CodeGraph, label: &str, extra: &[(&str, &str)]| {
        let mut metadata = BTreeMap::from([
            ("skipped".to_string(), "true".to_string()),
            ("skipped_reason".to_string(), "max_file_size".to_string()),
            ("file_size_bytes".to_string(), "8192".to_string()),
            ("max_file_size_bytes".to_string(), "4096".to_string()),
        ]);
        for (key, value) in extra {
            metadata.insert(key.to_string(), value.to_string());
        }
        graph.add_node_with_metadata(NodeKind::File, label, None, metadata)
    };
    // The scan records what a file is even when it is too large to read.
    let source = skipped(&mut graph, "src/huge.rs", &[("language", "rust")]);
    let notebook = skipped(
        &mut graph,
        "ipynb/Advent.ipynb",
        &[("item_kind", "notebook")],
    );
    let data = skipped(&mut graph, "data/text/big.txt", &[]);

    let summary = summarize(&graph);
    assert_eq!(summary.skipped_files, 3);

    let report = insights(&graph);
    let severity_of = |node: NodeId| {
        report
            .insights
            .iter()
            .find(|insight| insight.kind == "skipped_large_file" && insight.nodes.contains(&node))
            .map(|insight| insight.severity)
    };

    // A file the scan would have read for facts is missing from the graph.
    assert_eq!(severity_of(source), Some(InsightSeverity::Warning));
    assert_eq!(severity_of(notebook), Some(InsightSeverity::Warning));
    // pytudes keeps thirteen text corpora it never meant anyone to parse.
    assert_eq!(severity_of(data), Some(InsightSeverity::Info));

    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "skipped_large_file")
        .expect("expected skipped large file insight");
    assert!(insight.message.contains("8192"));
}

#[test]
fn insights_report_semantic_diagnostics() {
    let mut graph = CodeGraph::new("repo");
    let file = graph.add_node(NodeKind::File, "src/main.rs");
    let diagnostic = graph.add_node_with_metadata(
        NodeKind::Unknown,
        "error: semantic mismatch",
        Some(SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 3,
            start_column: 9,
            end_line: 3,
            end_column: 10,
        }),
        BTreeMap::from([
            ("item_kind".to_string(), "diagnostic".to_string()),
            ("source".to_string(), "lsp".to_string()),
            ("severity".to_string(), "error".to_string()),
            ("diagnostic_source".to_string(), "rustc".to_string()),
            ("diagnostic_code".to_string(), "E0001".to_string()),
            ("message".to_string(), "semantic mismatch".to_string()),
            ("path".to_string(), "src/main.rs".to_string()),
            ("line".to_string(), "3".to_string()),
            ("column".to_string(), "9".to_string()),
        ]),
    );
    graph.add_edge_with_metadata(
        file,
        diagnostic,
        EdgeKind::MayError,
        Confidence::Semantic,
        BTreeMap::from([("relation".to_string(), "diagnostic".to_string())]),
    );

    let report = insights(&graph);
    let insight = report
        .insights
        .iter()
        .find(|insight| insight.kind == "semantic_diagnostic")
        .expect("expected semantic diagnostic insight");

    assert_eq!(insight.severity, InsightSeverity::Error);
    assert_eq!(insight.nodes, vec![diagnostic, file]);
    assert_eq!(insight.edges, vec![0]);
    assert!(insight.message.contains("rustc error"));
    assert!(insight.message.contains("src/main.rs:3:9"));
    assert!(insight.message.contains("E0001"));
    assert_eq!(report.by_severity.get("error"), Some(&1));
    assert_eq!(report.by_kind.get("semantic_diagnostic"), Some(&1));

    let card = node_card(
        &graph,
        None,
        NodeCardRequest {
            node_id: file,
            edge_limit: 10,
            source_context: 1,
            insight_limit: 10,
        },
    )
    .unwrap()
    .expect("expected file card");
    assert!(
        card.insights
            .iter()
            .any(|insight| insight.kind == "semantic_diagnostic")
    );
}

#[test]
fn insights_match_php_composer_namespace_imports() {
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "composer.json");
    let app = graph.add_node_with_metadata(
        NodeKind::File,
        "src/App.php",
        None,
        BTreeMap::from([("language".to_string(), "php".to_string())]),
    );
    let test = graph.add_node_with_metadata(
        NodeKind::File,
        "tests/AppTest.php",
        None,
        BTreeMap::from([("language".to_string(), "php".to_string())]),
    );

    let monolog = dependency_node(&mut graph, "monolog/monolog", "composer:monolog/monolog");
    let symfony_console =
        dependency_node(&mut graph, "symfony/console", "composer:symfony/console");
    let phpunit = dependency_node(&mut graph, "phpunit/phpunit", "composer:phpunit/phpunit");
    let doctrine = dependency_node(&mut graph, "doctrine/orm", "composer:doctrine/orm");
    for dependency in [monolog, symfony_console, doctrine] {
        graph.add_edge_with_metadata(
            manifest,
            dependency,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
        );
    }
    graph.add_edge_with_metadata(
        manifest,
        phpunit,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );

    let monolog_import = import_node(&mut graph, "use Monolog\\Logger;", "php");
    let symfony_import = import_node(
        &mut graph,
        "use Symfony\\Component\\Console\\Application;",
        "php",
    );
    let phpunit_app_import = import_node(&mut graph, "use PHPUnit\\Framework\\TestCase;", "php");
    let phpunit_test_import = import_node(&mut graph, "use PHPUnit\\Framework\\TestCase;", "php");
    let undeclared_import = import_node(
        &mut graph,
        "use Acme\\Missing\\Client as MissingClient;",
        "php",
    );
    let local_import = import_node(&mut graph, "use App\\Domain\\Service;", "php");
    let builtin_import = import_node(&mut graph, "use DateTimeImmutable;", "php");

    graph.add_edge(
        app,
        monolog_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        app,
        symfony_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        app,
        phpunit_app_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        test,
        phpunit_test_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        app,
        undeclared_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(app, local_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        app,
        builtin_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&monolog)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&symfony_console)
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "unused_declared_dependency" && insight.nodes.contains(&doctrine)
    }));
    assert!(report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import"
            && insight.message.contains("acme/missing")
            && insight.nodes.contains(&undeclared_import)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.nodes.contains(&local_import)
    }));
    assert!(!report.insights.iter().any(|insight| {
        insight.kind == "undeclared_external_import" && insight.nodes.contains(&builtin_import)
    }));
    let non_runtime = report
        .insights
        .iter()
        .find(|insight| {
            insight.kind == "non_runtime_dependency_import" && insight.nodes.contains(&phpunit)
        })
        .expect("expected PHP production import of dev Composer dependency");
    assert!(non_runtime.message.contains("phpunit/phpunit"));
    assert!(non_runtime.nodes.contains(&app));
    assert!(non_runtime.nodes.contains(&phpunit_app_import));
    assert!(!non_runtime.nodes.contains(&test));
    assert!(!non_runtime.nodes.contains(&phpunit_test_import));
}

#[test]
fn insights_match_cargo_python_and_go_import_conventions() {
    let mut graph = CodeGraph::new("repo");
    let rust_file = graph.add_node(NodeKind::File, "src/lib.rs");
    let python_file = graph.add_node(NodeKind::File, "app.py");
    let go_file = graph.add_node(NodeKind::File, "main.go");

    let serde_json = dependency_node(&mut graph, "serde-json", "cargo:serde-json");
    let fastapi = dependency_node(&mut graph, "fastapi", "python:fastapi");
    let gin = dependency_node(
        &mut graph,
        "github.com/gin-gonic/gin",
        "go:github.com/gin-gonic/gin",
    );
    graph.add_edge(
        rust_file,
        serde_json,
        EdgeKind::DependsOn,
        Confidence::Exact,
    );
    graph.add_edge(python_file, fastapi, EdgeKind::DependsOn, Confidence::Exact);
    graph.add_edge(go_file, gin, EdgeKind::DependsOn, Confidence::Exact);

    for (file, label, language) in [
        (rust_file, "use serde_json::Value;", "rust"),
        (rust_file, "use anyhow::Result;", "rust"),
        (rust_file, "use std::fs;", "rust"),
        (python_file, "from fastapi import FastAPI", "python"),
        (python_file, "import requests", "python"),
        (python_file, "import os", "python"),
        (go_file, "import \"github.com/gin-gonic/gin/binding\"", "go"),
        (go_file, "import \"github.com/pkg/errors\"", "go"),
        (go_file, "import \"fmt\"", "go"),
    ] {
        let import = import_node(&mut graph, label, language);
        graph.add_edge(file, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let report = insights(&graph);
    for expected in ["anyhow", "requests", "github.com/pkg/errors"] {
        assert!(
            report.insights.iter().any(|insight| {
                insight.kind == "undeclared_external_import" && insight.message.contains(expected)
            }),
            "missing undeclared import insight for {expected}"
        );
    }
    for ignored in [
        "serde_json",
        "fastapi",
        "github.com/gin-gonic/gin/binding",
        "std::fs",
        "os",
        "fmt",
    ] {
        assert!(
            !report.insights.iter().any(|insight| {
                insight.kind == "undeclared_external_import" && insight.message.contains(ignored)
            }),
            "unexpected undeclared import insight for {ignored}"
        );
    }
}

fn import_node(graph: &mut CodeGraph, label: &str, language: &str) -> NodeId {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "import".to_string());
    metadata.insert("language".to_string(), language.to_string());
    graph.add_node_with_metadata(NodeKind::ExternalDependency, label, None, metadata)
}

fn dependency_node(graph: &mut CodeGraph, label: &str, package_id: &str) -> NodeId {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "dependency".to_string());
    metadata.insert("package_id".to_string(), package_id.to_string());
    graph.add_node_with_metadata(NodeKind::ExternalDependency, label, None, metadata)
}

fn temp_analysis_root() -> std::path::PathBuf {
    static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let process_id = std::process::id();
    std::env::temp_dir().join(format!(
        "codegraph-analysis-test-{process_id}-{counter}-{nanos}"
    ))
}

#[test]
fn node_reference_substring_fallback_requires_a_unique_match() {
    // With several substring candidates the old fallback bound to whichever
    // node came first — nondeterministic relative to the user's intent.
    let mut graph = CodeGraph::new("repo");
    graph.add_node(NodeKind::Function, "handler_v2");
    graph.add_node(NodeKind::Function, "handler_v3");
    let unique = graph.add_node(NodeKind::Function, "load_config_once");

    assert_eq!(
        resolve_node_reference(&graph, "handler"),
        None,
        "ambiguous substring must not resolve"
    );
    assert_eq!(
        resolve_node_reference(&graph, "load_config"),
        Some(unique),
        "unique substring still resolves"
    );
    assert_eq!(
        resolve_node_reference(&graph, "handler_v2"),
        Some(codegraph_core::NodeId(2)),
        "exact label wins regardless of substring ambiguity"
    );
}

#[test]
fn a_word_from_the_question_is_not_used_as_a_name_it_cannot_be() {
    // When nothing in a question looks like a name, the last ordinary word
    // was taken as one: "what routes exist" became `routes handler:exist`
    // and answered with nothing, where the question deserved the project's
    // routes. The guess survives only when the project really has
    // something by that name.
    let asked = |graph: &CodeGraph, question: &str| {
        natural_query(
            graph,
            NaturalQueryRequest {
                question: question.to_string(),
                compact: false,
            },
        )
        .expect("ask")
        .generated_query
    };

    let without_the_name = CodeGraph::new("repo");
    assert_eq!(
        asked(&without_the_name, "What routes exist?"),
        "routes depth:4 edge_limit:300"
    );

    let mut with_the_name = CodeGraph::new("repo");
    with_the_name.add_node(NodeKind::Function, "exist");
    with_the_name.add_node(NodeKind::Function, "route");
    assert_eq!(
        asked(&with_the_name, "What routes exist?"),
        "routes handler:exist depth:4 edge_limit:300",
        "a project that really has an `exist` keeps the filter"
    );

    // Naming calling outright settles the question before a topic word can.
    let rule = |question: &str| natural_query_plan(question).expect("plan").rule;
    assert_eq!(rule("who calls route"), "call_neighborhood");
    assert_eq!(rule("does main call init"), "call_neighborhood");
}

#[test]
fn a_trace_says_which_definition_it_started_from() {
    // A trace has one start. When several definitions answer to the name,
    // "nothing depends on Blueprint" means nothing depends on the one that
    // happened to be picked — a claim the reader could not check.
    let mut graph = CodeGraph::new("repo");
    let first = graph.add_node_with_span(
        NodeKind::Type,
        "Blueprint",
        SourceSpan {
            path: "src/blueprints.rs".to_string(),
            start_line: 18,
            start_column: 1,
            end_line: 40,
            end_column: 1,
        },
    );
    graph.add_node_with_span(
        NodeKind::Type,
        "Blueprint",
        SourceSpan {
            path: "src/sansio/blueprints.rs".to_string(),
            start_line: 7,
            start_column: 1,
            end_line: 20,
            end_column: 1,
        },
    );
    let caller = graph.add_node(NodeKind::Function, "register");
    graph.add_edge(caller, first, EdgeKind::References, Confidence::Syntactic);

    let result = query_graph(&graph, "dependents label:Blueprint depth:3").expect("query");
    assert_eq!(result.notes.len(), 1, "{:?}", result.notes);
    assert!(
        result.notes[0].contains("2 definitions are named `Blueprint`")
            && result.notes[0].contains("src/blueprints.rs:18"),
        "{:?}",
        result.notes
    );

    // One definition needs no such warning.
    let single = query_graph(&graph, "dependents label:register depth:3").expect("query");
    assert!(single.notes.is_empty(), "{:?}", single.notes);
}

#[test]
fn every_label_started_report_says_which_definition_it_took() {
    // The note belongs wherever a name is turned into one node: impact and
    // component-dependencies resolve a target, trace and workflow resolve a
    // start. Each of them answered about one of several definitions
    // without saying so.
    let mut graph = CodeGraph::new("repo");
    graph.add_node_with_span(
        NodeKind::Function,
        "handle",
        SourceSpan {
            path: "src/first.rs".to_string(),
            start_line: 4,
            start_column: 1,
            end_line: 9,
            end_column: 1,
        },
    );
    graph.add_node_with_span(
        NodeKind::Function,
        "handle",
        SourceSpan {
            path: "src/second.rs".to_string(),
            start_line: 11,
            start_column: 1,
            end_line: 20,
            end_column: 1,
        },
    );
    let only_one = graph.add_node(NodeKind::Function, "unique");
    graph.add_edge(only_one, only_one, EdgeKind::Calls, Confidence::Syntactic);

    let note_of = |notes: &[String]| notes.first().cloned().unwrap_or_default();

    let impacted = impact(
        &graph,
        ImpactRequest {
            target: "handle".to_string(),
            max_depth: 3,
            limit: 20,
        },
    )
    .expect("impact");
    assert!(
        note_of(&impacted.notes).contains("2 definitions are named `handle`"),
        "{:?}",
        impacted.notes
    );

    let traced = trace_dependents(
        &graph,
        TraceRequest {
            start: TraceStart::Label("handle".to_string()),
            max_depth: 3,
        },
    )
    .expect("trace");
    assert!(
        note_of(&traced.notes).contains("2 definitions are named `handle`"),
        "{:?}",
        traced.notes
    );

    // A journey resolves a name at both ends, so both can be a choice.
    let travelled = journey(
        &graph,
        JourneyRequest {
            from: "handle".to_string(),
            to: "unique".to_string(),
            max_depth: 3,
            path_limit: 3,
        },
    )
    .expect("journey");
    assert!(
        note_of(&travelled.notes).contains("2 definitions are named `handle`"),
        "{:?}",
        travelled.notes
    );

    // A name only one definition answers to needs no such warning.
    let unique = impact(
        &graph,
        ImpactRequest {
            target: "unique".to_string(),
            max_depth: 3,
            limit: 20,
        },
    )
    .expect("impact");
    assert!(unique.notes.is_empty(), "{:?}", unique.notes);
}

#[test]
fn explaining_a_call_says_what_resolved_it() {
    // "syntax-level fact" is true of every syntactic edge and tells the
    // reader nothing about this one. The call already records what
    // narrowed it down, so the explanation can name it.
    let mut graph = CodeGraph::new("repo");
    let caller = graph.add_node(NodeKind::Function, "run");
    let target = graph.add_node(NodeKind::Function, "build");
    graph.add_edge_with_metadata(
        caller,
        target,
        EdgeKind::Calls,
        Confidence::Syntactic,
        BTreeMap::from([
            ("call_label".to_string(), "build".to_string()),
            ("resolution".to_string(), "resolved".to_string()),
            ("resolution_basis".to_string(), "import".to_string()),
        ]),
    );

    let report = explain_edge(
        &graph,
        ExplainEdgeRequest {
            edge_index: Some(0),
            kind: None,
            source: None,
            target: None,
        },
    )
    .expect("explain")
    .expect("the edge exists");
    assert!(
        report.evidence.iter().any(|item| item
            == "resolution_note=an import in the calling file names the target's module"),
        "{:?}",
        report.evidence
    );

    // Every basis the resolver can write has words for the reader; a
    // missing one would leave the explanation quieter than the fact. The
    // list comes from the resolver itself, so a new narrowing cannot be
    // added without one.
    for basis in codegraph_indexer::RESOLUTION_BASES {
        assert!(
            resolution_basis_evidence(basis).is_some(),
            "no words for `{basis}`"
        );
    }
    // A basis the reader would not recognise adds nothing rather than
    // echoing an internal token.
    assert_eq!(resolution_basis_evidence("something_new"), None);
}

#[test]
fn a_container_directory_is_not_an_architecture_area() {
    // terraform keeps 4677 of its files under `internal/` and Vue all
    // twelve packages under `packages/`. Calling that one area describes
    // nothing: the architecture map said "internal: 4677 files" and
    // stopped. A directory that only groups other directories is not an
    // area; what it groups is.
    let mut graph = CodeGraph::new("repo");
    for path in [
        "internal/command/apply.go",
        "internal/command/plan.go",
        "internal/terraform/graph.go",
        "internal/configs/parser.go",
        "docs/guide.md",
        "README.md",
    ] {
        graph.add_node(NodeKind::File, path);
    }
    let project = ProjectAreas::from_graph(&graph);
    assert_eq!(
        project.group_for_path("internal/command/apply.go").0,
        "internal/command"
    );
    assert_eq!(
        project.group_for_path("internal/terraform/graph.go").0,
        "internal/terraform"
    );
    // A directory that holds its own files stays one area.
    assert_eq!(project.group_for_path("docs/guide.md").0, "docs");
    assert_eq!(project.group_for_path("README.md").0, ".");

    // A project whose top level already divides it is left alone.
    let mut flat = CodeGraph::new("repo");
    for path in [
        "src/app.py",
        "src/helpers/util.py",
        "tests/test_app.py",
        "docs/index.md",
    ] {
        flat.add_node(NodeKind::File, path);
    }
    let flat_areas = ProjectAreas::from_graph(&flat);
    assert_eq!(flat_areas.group_for_path("src/helpers/util.py").0, "src");
}

#[test]
fn a_common_crossing_is_the_architecture_and_a_rare_one_is_the_surprise() {
    // Two subsystems that exchange a thousand calls are how the project is
    // built. terraform crosses an area 58566 times across 874 pairs, so
    // scoring every crossing alike left 163872 candidates that no ranking
    // could tell apart. A markdown file mentioning a symbol is not a
    // dependency at all, yet it crossed an area, crossed a language and was
    // rare by construction — it outscored every real link.
    let mut graph = CodeGraph::new("repo");
    let busy_source = graph.add_node(NodeKind::File, "src/engine/run.rs");
    let busy_target = graph.add_node(NodeKind::File, "src/state/store.rs");
    let odd_source = graph.add_node(NodeKind::File, "src/cli/main.rs");
    let odd_target = graph.add_node(NodeKind::File, "src/vendor/patch.rs");
    let doc = graph.add_node_with_metadata(
        NodeKind::File,
        "docs/guide.md",
        None,
        BTreeMap::from([("language".to_string(), "markdown".to_string())]),
    );
    for _ in 0..12 {
        graph.add_edge(
            busy_source,
            busy_target,
            EdgeKind::Calls,
            Confidence::Heuristic,
        );
        graph.add_edge(
            busy_target,
            busy_source,
            EdgeKind::Calls,
            Confidence::Heuristic,
        );
    }
    graph.add_edge(
        odd_source,
        odd_target,
        EdgeKind::Calls,
        Confidence::Heuristic,
    );
    graph.add_edge(
        doc,
        busy_target,
        EdgeKind::References,
        Confidence::Heuristic,
    );

    let report = surprising_links(&graph, 20);
    assert!(
        report
            .links
            .iter()
            .all(|link| link.source.label != "docs/guide.md"),
        "a document mention is not a dependency: {:?}",
        report
            .links
            .iter()
            .map(|l| &l.source.label)
            .collect::<Vec<_>>()
    );
    let rare = report
        .links
        .iter()
        .find(|link| link.source.label == "src/cli/main.rs")
        .expect("the one-off crossing is reported");
    assert!(rare.reasons.iter().any(|reason| reason == "rare_crossing"));
    let common = report
        .links
        .iter()
        .find(|link| link.source.label == "src/engine/run.rs");
    assert!(
        common.is_none_or(|link| link.score < rare.score),
        "the everyday crossing must not outrank the one-off"
    );
}

#[test]
fn the_standard_library_is_not_an_undeclared_dependency() {
    // The list of Python's standard modules held 41 of the 194 names, so
    // `import ssl`, `import struct` and `import weakref` each read as a
    // dependency the project had failed to declare — 74 of flask's 155
    // warnings were of that kind. A stub module that only type checkers
    // read is not a dependency either.
    for module in [
        "ssl",
        "struct",
        "weakref",
        "platform",
        "decimal",
        "types",
        "traceback",
        "contextvars",
        "copy",
        "textwrap",
        "atexit",
        "difflib",
        "multiprocessing",
        "ast",
        "operator",
    ] {
        assert_eq!(
            python_import_package(&format!("import {module}")),
            None,
            "`{module}` is part of Python"
        );
    }
    assert_eq!(
        python_import_package("from _typeshed.wsgi import StartResponse"),
        None,
        "a type-checker stub is installed by nothing"
    );
    // A real dependency still has to be declared.
    assert_eq!(
        python_import_package("import dotenv"),
        Some("dotenv".to_string())
    );
    assert_eq!(
        python_import_package("from pallets_sphinx_themes import get_version"),
        Some("pallets-sphinx-themes".to_string())
    );
}

#[test]
fn a_route_a_test_declares_is_not_a_duplicate_of_the_applications() {
    // A duplicate route is a conflict only within one application, and the
    // graph does not model the application object. flask's suite declares
    // `GET /` eleven times — eleven applications, one per case, and
    // repeatedly inside a single file — so 22 of its 25 duplicate groups
    // described nothing.
    let mut graph = CodeGraph::new("repo");
    let route = |graph: &mut CodeGraph, path: &str, file: &str, line: u32| {
        graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            format!("route GET {path}"),
            Some(SourceSpan {
                path: file.to_string(),
                start_line: line,
                start_column: 1,
                end_line: line,
                end_column: 1,
            }),
            BTreeMap::from([
                ("item_kind".to_string(), "framework_route".to_string()),
                ("path".to_string(), path.to_string()),
                ("method".to_string(), "GET".to_string()),
            ]),
        )
    };
    route(&mut graph, "/", "tests/test_basic.py", 10);
    route(&mut graph, "/", "tests/test_basic.py", 40);
    route(&mut graph, "/", "tests/test_reqctx.py", 12);
    route(&mut graph, "/health", "src/app/api.py", 20);
    route(&mut graph, "/health", "src/app/legacy.py", 8);

    let report = insights(&graph);
    let duplicates: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "duplicate_framework_route")
        .collect();
    assert_eq!(duplicates.len(), 1, "{duplicates:?}");
    assert!(
        duplicates[0].message.contains("/health"),
        "only the application's own routes can collide: {}",
        duplicates[0].message
    );
}

#[test]
fn a_type_import_and_a_tool_config_do_not_ship_a_dev_dependency() {
    // `import type { Program } from '@babel/types'` is erased before
    // anything runs, and `eslint.config.js` exists to configure a tool
    // rather than to ship. 44 of Vue core's 74 findings were one or the
    // other.
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let source = graph.add_node_with_metadata(
        NodeKind::File,
        "packages/compiler/src/ast.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let config = graph.add_node_with_metadata(
        NodeKind::File,
        "eslint.config.js",
        None,
        BTreeMap::from([("language".to_string(), "javascript".to_string())]),
    );
    let babel = dependency_node(&mut graph, "@babel/types", "npm:@babel/types");
    let eslint = dependency_node(&mut graph, "eslint", "npm:eslint");
    for package in [babel, eslint] {
        graph.add_edge_with_metadata(
            manifest,
            package,
            EdgeKind::DependsOn,
            Confidence::Exact,
            BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
        );
    }

    let type_import = import_node(
        &mut graph,
        "import type { Program } from \"@babel/types\";",
        "typescript",
    );
    let value_import = import_node(
        &mut graph,
        "import { parse } from \"@babel/types\";",
        "typescript",
    );
    let config_import = import_node(&mut graph, "import eslint from \"eslint\";", "javascript");
    graph.add_edge(
        source,
        type_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    graph.add_edge(
        config,
        config_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let quiet = insights(&graph);
    assert!(
        !quiet
            .insights
            .iter()
            .any(|insight| insight.kind == "non_runtime_dependency_import"),
        "{:?}",
        quiet
            .insights
            .iter()
            .filter(|insight| insight.kind == "non_runtime_dependency_import")
            .map(|insight| &insight.message)
            .collect::<Vec<_>>()
    );

    // The same package imported for its value is a real finding.
    graph.add_edge(
        source,
        value_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );
    let loud = insights(&graph);
    assert!(
        loud.insights
            .iter()
            .any(|insight| insight.kind == "non_runtime_dependency_import"),
        "a value import of a dev dependency still ships it"
    );
}

#[test]
fn a_script_that_runs_a_program_declares_no_missing_file() {
    // `vitepress dev`, `patch-package --exclude nothing`, `eslint
    // lib/**/*.js`: a script that runs a tool names no file in the
    // repository, so nothing there failed to resolve. 44 of the 59
    // unresolved entrypoint targets across the corpora were commands and
    // several more were glob patterns.
    for command in [
        "vitepress dev",
        "patch-package --exclude nothing",
        "npm run docs:build && npm run test",
        "eslint --fix lib/**/*.js",
        "mocha --timeout 10000 \"tests/**/*.module.test.cjs\"",
    ] {
        assert!(!names_a_path(command), "`{command}` names no file");
    }
    for target in [
        "dist/index.js",
        "node scripts/build.js",
        "./bin/run.sh",
        "python -m app.main",
        "src/main.rs",
    ] {
        assert!(names_a_path(target), "`{target}` does name a file");
    }
}

#[test]
fn a_project_does_not_import_itself_from_outside() {
    // guzzle's own sources `use GuzzleHttp\…`, which names the package its
    // composer.json claims, and 363 findings said the project had failed
    // to require itself. `Psr\Http\Message` is psr/http-message, not the
    // psr/http the two-segment rule produced.
    let candidates = php_namespace_package_candidates("Psr\\Http\\Message\\RequestInterface");
    assert!(
        candidates.contains(&"psr/http-message".to_string()),
        "{candidates:?}"
    );
    assert!(
        candidates.contains(&"psr/http".to_string()),
        "the shorter reading stays a candidate: {candidates:?}"
    );

    let mut graph = CodeGraph::new("repo");
    let root = graph.root;
    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == root) {
        node.metadata.insert(
            "own_package_ids".to_string(),
            "composer:guzzlehttp/guzzle".to_string(),
        );
    }
    let manifest = graph.add_node(NodeKind::File, "composer.json");
    let source = graph.add_node_with_metadata(
        NodeKind::File,
        "src/Client.php",
        None,
        BTreeMap::from([("language".to_string(), "php".to_string())]),
    );
    let promises = dependency_node(
        &mut graph,
        "guzzlehttp/promises",
        "composer:guzzlehttp/promises",
    );
    graph.add_edge_with_metadata(
        manifest,
        promises,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );
    let own_import = import_node(&mut graph, "use GuzzleHttp\\Psr7\\Request;", "php");
    let other_import = import_node(&mut graph, "use Monolog\\Logger;", "php");
    graph.add_edge(source, own_import, EdgeKind::Imports, Confidence::Syntactic);
    graph.add_edge(
        source,
        other_import,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let undeclared: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .map(|insight| insight.message.as_str())
        .collect();
    assert_eq!(undeclared.len(), 1, "{undeclared:?}");
    assert!(
        undeclared[0].contains("monolog/monolog"),
        "only a package that really is outside: {}",
        undeclared[0]
    );
}

#[test]
fn a_test_and_a_build_config_are_reached_by_their_own_runners() {
    // "reads `X` but is not reachable from any entrypoint" describes the
    // tooling when the reader is a test or a build config: a test runner
    // runs one and a bundler the other. 65% of terraform's findings of
    // this kind were tests, 81% of kong's, and Vue's were its vite and
    // rollup configs.
    let mut graph = CodeGraph::new("repo");
    let main = graph.add_node(NodeKind::Function, "main");
    graph.add_edge(graph.root, main, EdgeKind::Entrypoint, Confidence::Exact);
    let setting = graph.add_node(NodeKind::Environment, "API_TOKEN");

    let reader = |graph: &mut CodeGraph, label: &str, path: &str| {
        let id = graph.add_node_with_span(
            NodeKind::Function,
            label,
            SourceSpan {
                path: path.to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 2,
                end_column: 1,
            },
        );
        graph.add_edge(
            id,
            setting,
            EdgeKind::ReadsEnvironment,
            Confidence::Syntactic,
        );
        id
    };
    reader(&mut graph, "test_reads_token", "tests/test_client.py");
    reader(&mut graph, "createConfig", "vite.config.ts");
    reader(&mut graph, "load_settings", "src/settings.py");

    let report = insights(&graph);
    let messages: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "unreachable_config_read")
        .map(|insight| insight.message.as_str())
        .collect();
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].contains("load_settings"),
        "only the program's own unreachable reader: {}",
        messages[0]
    );
}

#[test]
fn recursion_inside_a_file_is_not_the_coupling_a_cycle_warns_about() {
    // `ProposedNew -> proposedNew -> proposedNewBlockOrObject -> …` is a
    // parser walking a tree, and every one of terraform's 50 cycles and
    // dune's 50 is of that kind. A cycle that crosses files is the
    // coupling the finding exists to surface.
    let mut graph = CodeGraph::new("repo");
    let at = |graph: &mut CodeGraph, label: &str, path: &str| {
        graph.add_node_with_span(
            NodeKind::Function,
            label,
            SourceSpan {
                path: path.to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 2,
                end_column: 1,
            },
        )
    };
    let walk = at(&mut graph, "walk", "src/parse.rs");
    let walk_block = at(&mut graph, "walk_block", "src/parse.rs");
    graph.add_edge(walk, walk_block, EdgeKind::Calls, Confidence::Syntactic);
    graph.add_edge(walk_block, walk, EdgeKind::Calls, Confidence::Syntactic);

    let store = at(&mut graph, "store_write", "src/store.rs");
    let cache = at(&mut graph, "cache_fill", "src/cache.rs");
    graph.add_edge(store, cache, EdgeKind::Calls, Confidence::Syntactic);
    graph.add_edge(cache, store, EdgeKind::Calls, Confidence::Syntactic);

    let report = insights(&graph);
    let cycles: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "dependency_cycle")
        .collect();
    assert_eq!(cycles.len(), 2, "both cycles are still reported");

    let inside = cycles
        .iter()
        .find(|insight| insight.message.contains("inside one file"))
        .expect("the recursion is reported");
    assert_eq!(inside.severity, InsightSeverity::Info);

    let across = cycles
        .iter()
        .find(|insight| insight.message.contains("across files"))
        .expect("the coupling is reported");
    assert_eq!(across.severity, InsightSeverity::Warning);
}

#[test]
fn an_entrypoint_pointing_at_a_directory_or_a_url_resolved_fine() {
    // `vite packages-private/sfc-playground --host` names a directory that
    // is right there, `open http://localhost:3000/x.html` names somewhere
    // else entirely, and `node --test .vitepress/search.test.js` names a
    // file inside a directory the scan never opened.
    let mut graph = CodeGraph::new("repo");
    graph.add_node(NodeKind::Directory, "packages-private/sfc-playground");
    graph.add_node(NodeKind::File, "scripts/build.js");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    for (label, target) in [
        (
            "npm script:dev",
            "vite packages-private/sfc-playground --host",
        ),
        (
            "npm script:open",
            "open http://localhost:3000/packages-private/local.html",
        ),
        (
            "npm script:test:search",
            "node --test .vitepress/search.test.js",
        ),
        ("npm script:missing", "node scripts/absent.js"),
    ] {
        let entry = graph.add_node_with_metadata(
            NodeKind::Entrypoint,
            label,
            None,
            BTreeMap::from([
                ("item_kind".to_string(), "manifest_entrypoint".to_string()),
                ("target".to_string(), target.to_string()),
            ]),
        );
        graph.add_edge(manifest, entry, EdgeKind::Entrypoint, Confidence::Exact);
    }

    let report = insights(&graph);
    let found: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "unresolved_entrypoint_target")
        .map(|insight| insight.message.as_str())
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("scripts/absent.js"), "{}", found[0]);
}

#[test]
fn a_module_a_file_declares_is_not_a_crate() {
    // `mod cli;` puts `cli` in main.rs's own scope, and the `use cli::*`
    // beneath it names that module. Reading it as a package had this
    // repository's own scan reporting a cargo dependency nobody declared.
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "Cargo.toml");
    let dependency = dependency_node(&mut graph, "serde", "cargo:serde");
    graph.add_edge_with_metadata(
        manifest,
        dependency,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );

    let main = graph.add_node_with_metadata(
        NodeKind::File,
        "src/main.rs",
        None,
        BTreeMap::from([("language".to_string(), "rust".to_string())]),
    );
    graph.add_node_with_span(
        NodeKind::Module,
        "cli",
        SourceSpan {
            path: "src/main.rs".to_string(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 9,
        },
    );
    let import = import_node(&mut graph, "use cli::*;", "rust");
    graph.add_edge(main, import, EdgeKind::Imports, Confidence::Syntactic);
    let other = import_node(&mut graph, "use anyhow::Result;", "rust");
    graph.add_edge(main, other, EdgeKind::Imports, Confidence::Syntactic);

    let report = insights(&graph);
    let undeclared: Vec<&str> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .map(|insight| insight.message.as_str())
        .collect();
    assert_eq!(undeclared.len(), 1, "{undeclared:?}");
    assert!(undeclared[0].contains("anyhow"), "{}", undeclared[0]);
}

#[test]
fn a_specifier_that_names_no_package_is_not_undeclared() {
    // `node:fs` and `bun:test` say which runtime provides the module,
    // `http2` is one Node ships, and `import type` from `trusted-types` is
    // served by the `@types/trusted-types` that vue declares.
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let types = dependency_node(
        &mut graph,
        "@types/trusted-types",
        "npm:@types/trusted-types",
    );
    graph.add_edge_with_metadata(
        manifest,
        types,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "dev".to_string())]),
    );

    let source = graph.add_node_with_metadata(
        NodeKind::File,
        "src/dom.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    for label in [
        "import http2 from \"http2\"",
        "import { readFile } from \"fs/promises\"",
        "import { test } from \"bun:test\"",
        "import { assert } from \"jsr:@std/assert\"",
        "import type { TrustedHTML } from \"trusted-types/lib\"",
    ] {
        let import = import_node(&mut graph, label, "typescript");
        graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);
    }

    let report = insights(&graph);
    let found: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .collect();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn one_undeclared_package_is_one_finding() {
    // guzzle imports `psr/http` from 169 places, and 169 identical findings
    // say no more than one that counts them. A specifier built at runtime
    // names no package at all.
    let mut graph = CodeGraph::new("repo");
    let manifest = graph.add_node(NodeKind::File, "package.json");
    let declared = dependency_node(&mut graph, "vue", "npm:vue");
    graph.add_edge_with_metadata(
        manifest,
        declared,
        EdgeKind::DependsOn,
        Confidence::Exact,
        BTreeMap::from([("dependency_kind".to_string(), "runtime".to_string())]),
    );

    for file in ["src/a.ts", "src/b.ts", "src/c.ts", "src/d.ts"] {
        let source = graph.add_node_with_metadata(
            NodeKind::File,
            file,
            None,
            BTreeMap::from([("language".to_string(), "typescript".to_string())]),
        );
        let import = import_node(&mut graph, "import { z } from \"zod\";", "typescript");
        graph.add_edge(source, import, EdgeKind::Imports, Confidence::Syntactic);
    }
    let templated_source = graph.add_node_with_metadata(
        NodeKind::File,
        "scripts/build.ts",
        None,
        BTreeMap::from([("language".to_string(), "typescript".to_string())]),
    );
    let templated = import_node(&mut graph, "import(`${pkgDir}/package.json`)", "typescript");
    graph.add_edge(
        templated_source,
        templated,
        EdgeKind::Imports,
        Confidence::Syntactic,
    );

    let report = insights(&graph);
    let undeclared: Vec<&Insight> = report
        .insights
        .iter()
        .filter(|insight| insight.kind == "undeclared_external_import")
        .collect();
    assert_eq!(undeclared.len(), 1, "{:?}", undeclared);
    assert!(
        undeclared[0].message.starts_with("`zod` is imported from"),
        "{}",
        undeclared[0].message
    );
    assert!(
        undeclared[0].message.contains("and 1 more"),
        "the finding counts the sites it does not name: {}",
        undeclared[0].message
    );
    assert_eq!(undeclared[0].edges.len(), 4, "every site stays as evidence");
}
