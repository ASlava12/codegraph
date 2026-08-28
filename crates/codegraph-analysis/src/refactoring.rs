//! The refactoring product: journeys, component dependencies and
//! contracts, refactor-context bundles, seams, and impact reports.

use codegraph_core::{CodeGraph, Confidence, Edge, EdgeKind, Node, NodeId, NodeKind, SourceSpan};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[allow(unused_imports)]
use crate::*;

pub const JOURNEY_SCHEMA: &str = "codegraph.journey.v1";

pub fn journey(graph: &CodeGraph, request: JourneyRequest) -> Result<JourneyReport, QueryError> {
    let insight_report = insights(graph);
    journey_with_insights(graph, request, &insight_report)
}

/// Journey over an already-computed insight report, so bundles (refactor
/// context) evaluate insights once instead of once per section.
pub(crate) fn journey_with_insights(
    graph: &CodeGraph,
    request: JourneyRequest,
    insight_report: &InsightReport,
) -> Result<JourneyReport, QueryError> {
    let max_depth = request.max_depth.clamp(1, 32);
    let from = request.from.trim();
    let to = request.to.trim();
    if from.is_empty() || to.is_empty() {
        return Err(QueryError::new(
            "journey requires `from` and `to` labels or node ids",
        ));
    }
    let start = resolve_node_reference(graph, from)
        .ok_or_else(|| node_not_found_error(graph, "journey start", from))?;
    let target = resolve_node_reference(graph, to)
        .ok_or_else(|| node_not_found_error(graph, "journey target", to))?;
    let start_node = graph
        .nodes
        .iter()
        .find(|node| node.id == start)
        .cloned()
        .ok_or_else(|| node_not_found_error(graph, "journey start", from))?;
    let target_node = graph
        .nodes
        .iter()
        .find(|node| node.id == target)
        .cloned()
        .ok_or_else(|| node_not_found_error(graph, "journey target", to))?;
    let path_limit = if request.path_limit == 0 {
        3
    } else {
        request.path_limit.clamp(1, 10)
    };

    let adjacency = TraceAdjacency::build(graph);
    let mut edge_paths: Vec<Vec<usize>> = Vec::new();
    let mut truncated = false;
    if start == target {
        edge_paths.push(Vec::new());
    } else {
        let mut banned_edges: BTreeSet<usize> = BTreeSet::new();
        while edge_paths.len() < path_limit {
            let (found, hit_depth_bound) =
                journey_shortest_path(graph, &adjacency, start, target, max_depth, &banned_edges)?;
            truncated = truncated || hit_depth_bound;
            let Some(edge_indexes) = found else {
                break;
            };
            banned_edges.extend(edge_indexes.iter().copied());
            edge_paths.push(edge_indexes);
        }
    }

    let mut paths = edge_paths
        .into_iter()
        .map(|edge_indexes| {
            let mut steps = Vec::with_capacity(edge_indexes.len() + 1);
            let mut confidence_score = 0usize;
            let mut lowest: Option<Confidence> = None;
            steps.push(JourneyStep {
                step: 1,
                transition: None,
                explanation: None,
                fragile: false,
                fragile_reasons: Vec::new(),
                block: journey_block(insight_report, graph, &start_node, None, true, 0),
            });
            for (offset, edge_index) in edge_indexes.iter().enumerate() {
                let Some(edge) = graph.edges.get(*edge_index) else {
                    continue;
                };
                let Some(node) = graph.nodes.iter().find(|node| node.id == edge.target) else {
                    continue;
                };
                confidence_score += journey_confidence_weight(edge.confidence);
                if lowest.is_none_or(|current| {
                    journey_confidence_weight(edge.confidence) > journey_confidence_weight(current)
                }) {
                    lowest = Some(edge.confidence);
                }
                let transition = WorkflowTransition {
                    id: format!("jt-{edge_index}"),
                    source: workflow_block_id(edge.source),
                    target: workflow_block_id(edge.target),
                    source_node_id: edge.source,
                    target_node_id: edge.target,
                    edge: edge_with_index(*edge_index, edge),
                    edge_index: *edge_index,
                    risk_refs: workflow_risk_refs_for_edge(insight_report, *edge_index),
                    compacted: false,
                    compacted_count: 1,
                };
                let block =
                    journey_block(insight_report, graph, node, Some(edge), false, offset + 1);
                let fragile_reasons = journey_fragile_reasons(edge, &transition, &block);
                steps.push(JourneyStep {
                    step: offset + 2,
                    explanation: Some(journey_hop_explanation(edge)),
                    fragile: !fragile_reasons.is_empty(),
                    fragile_reasons,
                    transition: Some(transition),
                    block,
                });
            }
            let risk_summary = journey_risk_summary(graph, &adjacency, &steps);
            JourneyPath {
                rank: 0,
                total_steps: steps.len(),
                confidence_score,
                lowest_confidence: lowest.map(|confidence| confidence_name(confidence).to_string()),
                risk_summary,
                steps,
            }
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| (path.confidence_score, path.total_steps));
    for (index, path) in paths.iter_mut().enumerate() {
        path.rank = index + 1;
    }

    let target_reference = durable_node_reference(&target_node);
    let start_reference = durable_node_reference(&start_node);
    let suggested_commands = vec![
        format!("codegraph impact {target_reference} ."),
        format!("codegraph refactor-context {target_reference} . --from {start_reference}"),
        format!("codegraph node-card . --node-id {target_reference}"),
    ];

    Ok(JourneyReport {
        schema: JOURNEY_SCHEMA.to_string(),
        // Both ends are resolved from a name, so both can be a choice the
        // caller did not make.
        notes: shared_name_note(graph, from, &start_node)
            .into_iter()
            .chain(shared_name_note(graph, to, &target_node))
            .collect(),
        from: start_node,
        to: target_node,
        max_depth,
        total_paths: paths.len(),
        truncated: truncated && paths.is_empty(),
        paths,
        suggested_commands,
    })
}

pub fn component_dependencies(
    graph: &CodeGraph,
    request: ComponentDependencyRequest,
) -> Result<ComponentDependencyReport, QueryError> {
    let group_limit = request.group_limit.clamp(1, 100);
    let edge_limit = request.edge_limit.clamp(1, 50);
    let target = request.target.trim();
    if target.is_empty() {
        return Err(QueryError::new(
            "component dependencies require a `target` label or node id",
        ));
    }
    let target_id = resolve_node_reference(graph, target)
        .ok_or_else(|| node_not_found_error(graph, "component target", target))?;
    let target_node = graph
        .nodes
        .iter()
        .find(|node| node.id == target_id)
        .cloned()
        .ok_or_else(|| node_not_found_error(graph, "component target", target))?;
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let areas = node_architecture_areas(graph, &nodes_by_id);

    let mut area_groups: BTreeMap<String, ComponentDependencyGroup> = BTreeMap::new();
    let mut package_groups: BTreeMap<String, ComponentDependencyGroup> = BTreeMap::new();
    let mut language_groups: BTreeMap<String, ComponentDependencyGroup> = BTreeMap::new();
    let mut total_incoming = 0usize;
    let mut total_outgoing = 0usize;

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.kind == EdgeKind::Contains {
            continue;
        }
        let (neighbor_id, incoming) = if edge.target == target_id {
            (edge.source, true)
        } else if edge.source == target_id {
            (edge.target, false)
        } else {
            continue;
        };
        let Some(neighbor) = nodes_by_id.get(&neighbor_id) else {
            continue;
        };
        if incoming {
            total_incoming += 1;
        } else {
            total_outgoing += 1;
        }

        let area_key = areas
            .get(&neighbor_id)
            .cloned()
            .unwrap_or_else(|| "(external)".to_string());
        component_group_add(
            &mut area_groups,
            area_key,
            edge,
            edge_index,
            incoming,
            edge_limit,
        );
        if let Some(package_id) = neighbor.metadata.get("package_id") {
            component_group_add(
                &mut package_groups,
                package_id.clone(),
                edge,
                edge_index,
                incoming,
                edge_limit,
            );
        }
        if let Some(language) = neighbor.metadata.get("language") {
            component_group_add(
                &mut language_groups,
                language.clone(),
                edge,
                edge_index,
                incoming,
                edge_limit,
            );
        }
    }

    let (areas_list, areas_truncated) = component_sorted_groups(area_groups, group_limit);
    let (packages_list, packages_truncated) = component_sorted_groups(package_groups, group_limit);
    let (languages_list, languages_truncated) =
        component_sorted_groups(language_groups, group_limit);

    Ok(ComponentDependencyReport {
        notes: shared_name_note(graph, target, &target_node)
            .into_iter()
            .collect(),
        area: areas.get(&target_id).cloned(),
        target: target_node,
        total_incoming,
        total_outgoing,
        areas: areas_list,
        packages: packages_list,
        languages: languages_list,
        truncated: areas_truncated || packages_truncated || languages_truncated,
    })
}

pub(crate) fn component_group_add(
    groups: &mut BTreeMap<String, ComponentDependencyGroup>,
    key: String,
    edge: &Edge,
    edge_index: usize,
    incoming: bool,
    edge_limit: usize,
) {
    let group = groups
        .entry(key.clone())
        .or_insert_with(|| ComponentDependencyGroup {
            key,
            incoming: 0,
            outgoing: 0,
            edge_kinds: BTreeMap::new(),
            confidence_counts: BTreeMap::new(),
            sample_edge_indexes: Vec::new(),
        });
    if incoming {
        group.incoming += 1;
    } else {
        group.outgoing += 1;
    }
    *group
        .edge_kinds
        .entry(edge_kind_name(&edge.kind).to_string())
        .or_insert(0) += 1;
    *group
        .confidence_counts
        .entry(confidence_name(edge.confidence).to_string())
        .or_insert(0) += 1;
    if group.sample_edge_indexes.len() < edge_limit {
        group.sample_edge_indexes.push(edge_index);
    }
}

pub(crate) fn component_sorted_groups(
    groups: BTreeMap<String, ComponentDependencyGroup>,
    group_limit: usize,
) -> (Vec<ComponentDependencyGroup>, bool) {
    let total = groups.len();
    let mut list = groups.into_values().collect::<Vec<_>>();
    list.sort_by(|left, right| {
        (right.incoming + right.outgoing)
            .cmp(&(left.incoming + left.outgoing))
            .then_with(|| left.key.cmp(&right.key))
    });
    list.truncate(group_limit);
    (list, total > group_limit)
}

pub fn component_contract(
    graph: &CodeGraph,
    request: ComponentContractRequest,
) -> Result<ComponentContractReport, QueryError> {
    let edge_limit = request.edge_limit.clamp(1, 500);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let areas = node_architecture_areas(graph, &nodes_by_id);
    let known_areas = areas.values().cloned().collect::<BTreeSet<_>>();
    let source_area = component_resolve_area(&known_areas, &request.source)?;
    let target_area = component_resolve_area(&known_areas, &request.target)?;
    let insight_report = insights(graph);

    let mut edge_kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut confidence_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut edges = Vec::new();
    let mut total_edges = 0usize;
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind) {
            continue;
        }
        if areas.get(&edge.source) != Some(&source_area)
            || areas.get(&edge.target) != Some(&target_area)
        {
            continue;
        }
        if source_area == target_area && edge.source == edge.target {
            continue;
        }
        total_edges += 1;
        *edge_kinds
            .entry(edge_kind_name(&edge.kind).to_string())
            .or_insert(0) += 1;
        *confidence_counts
            .entry(confidence_name(edge.confidence).to_string())
            .or_insert(0) += 1;
        if edges.len() < edge_limit {
            edges.push(ComponentContractEdge {
                edge_index,
                edge: edge_with_index(edge_index, edge),
                source_label: nodes_by_id
                    .get(&edge.source)
                    .map(|node| node.label.clone())
                    .unwrap_or_default(),
                target_label: nodes_by_id
                    .get(&edge.target)
                    .map(|node| node.label.clone())
                    .unwrap_or_default(),
                risk_count: workflow_risk_refs_for_edge(&insight_report, edge_index).len(),
            });
        }
    }

    Ok(ComponentContractReport {
        source_area,
        target_area,
        truncated: edges.len() < total_edges,
        total_edges,
        edge_kinds,
        confidence_counts,
        edges,
    })
}

pub(crate) fn component_resolve_area(
    known_areas: &BTreeSet<String>,
    requested: &str,
) -> Result<String, QueryError> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err(QueryError::new(
            "component contract requires `source` and `target` area names",
        ));
    }
    if let Some(exact) = known_areas
        .iter()
        .find(|area| area.eq_ignore_ascii_case(requested))
    {
        return Ok(exact.clone());
    }
    let matches = known_areas
        .iter()
        .filter(|area| {
            area.to_ascii_lowercase()
                .contains(&requested.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(QueryError::new(format!(
            "component area `{requested}` did not match any architecture area; known areas: {}",
            known_areas
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))),
        many => Err(QueryError::new(format!(
            "component area `{requested}` is ambiguous; candidates: {}",
            many.iter()
                .take(12)
                .map(|area| area.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

pub const REFACTOR_CONTEXT_SCHEMA: &str = "codegraph.refactor_context.v1";

pub fn refactor_context(
    graph: &CodeGraph,
    request: RefactorContextRequest,
) -> Result<RefactorContextBundle, QueryError> {
    let max_depth = request.max_depth.clamp(1, 32);
    let risk_limit = request.risk_limit.clamp(1, 200);
    // Insights are evaluated once per bundle and shared with the impact
    // section instead of being recomputed per report (audit F11).
    let insight_report = insights(graph);
    let impact_report = impact_with_insights(
        graph,
        ImpactRequest {
            target: request.target.clone(),
            max_depth,
            limit: request.dependent_limit,
        },
        &insight_report,
    )?;
    let dependencies = component_dependencies(
        graph,
        ComponentDependencyRequest {
            target: request.target.clone(),
            group_limit: 25,
            edge_limit: 10,
        },
    )?;
    let journey_report = match &request.from {
        Some(from) if !from.trim().is_empty() => Some(journey_with_insights(
            graph,
            JourneyRequest {
                from: from.clone(),
                to: request.target.clone(),
                // Journey exploration stays bounded even when callers ask
                // for deep impact traversal (audit F11).
                max_depth: max_depth.min(12),
                path_limit: request.path_limit.clamp(1, 10),
            },
            &insight_report,
        )?),
        _ => None,
    };

    let mut relevant: BTreeSet<NodeId> = impact_report
        .dependents
        .iter()
        .map(|dependent| dependent.node.id)
        .collect();
    relevant.insert(impact_report.target.id);
    let matching = insight_report
        .insights
        .iter()
        .filter(|insight| {
            insight
                .nodes
                .iter()
                .any(|node_id| relevant.contains(node_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let total_risks = matching.len();
    let risks_truncated = total_risks > risk_limit;
    let risks = matching.into_iter().take(risk_limit).collect::<Vec<_>>();

    Ok(RefactorContextBundle {
        schema: REFACTOR_CONTEXT_SCHEMA.to_string(),
        target: impact_report.target.clone(),
        area: impact_report.area.clone(),
        impact: impact_report,
        dependencies,
        journey: journey_report,
        total_risks,
        risks,
        risks_truncated,
        target_source: None,
    })
}

pub fn seams(graph: &CodeGraph, request: SeamRequest) -> SeamReport {
    let limit = request.limit.clamp(1, 100);
    let edge_limit = request.edge_limit.clamp(1, 50);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let areas = node_architecture_areas(graph, &nodes_by_id);
    let insight_report = insights(graph);

    let mut pairs: BTreeMap<(String, String), SeamCandidate> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind) {
            continue;
        }
        let (Some(source_area), Some(target_area)) =
            (areas.get(&edge.source), areas.get(&edge.target))
        else {
            continue;
        };
        if source_area == target_area {
            continue;
        }
        let candidate = pairs
            .entry((source_area.clone(), target_area.clone()))
            .or_insert_with(|| SeamCandidate {
                source_area: source_area.clone(),
                target_area: target_area.clone(),
                edge_count: 0,
                edge_kinds: BTreeMap::new(),
                confidence_counts: BTreeMap::new(),
                low_confidence_edges: 0,
                risk_count: 0,
                friction_score: 0,
                sample_edge_indexes: Vec::new(),
            });
        candidate.edge_count += 1;
        *candidate
            .edge_kinds
            .entry(edge_kind_name(&edge.kind).to_string())
            .or_insert(0) += 1;
        *candidate
            .confidence_counts
            .entry(confidence_name(edge.confidence).to_string())
            .or_insert(0) += 1;
        if matches!(edge.confidence, Confidence::Heuristic | Confidence::Unknown) {
            candidate.low_confidence_edges += 1;
        }
        candidate.risk_count += workflow_risk_refs_for_edge(&insight_report, edge_index).len();
        if candidate.sample_edge_indexes.len() < edge_limit {
            candidate.sample_edge_indexes.push(edge_index);
        }
    }

    let mut candidates = pairs.into_values().collect::<Vec<_>>();
    for candidate in &mut candidates {
        candidate.friction_score = candidate.edge_count
            + candidate.low_confidence_edges * 2
            + candidate.risk_count * 3
            + candidate.edge_kinds.len();
    }
    let total_pairs = candidates.len();

    // A suite leans on the program by design: mastodon's specs reach
    // `app/models` 937 times and `app/lib` 832, which topped the seams
    // worth untangling and said nothing anyone would act on. A boundary
    // with the suite on one side is neither a seam to extract nor coupling
    // to fix, so it follows the boundaries inside the program in both
    // lists.
    let touches_a_suite = |candidate: &SeamCandidate| {
        u8::from(
            is_test_like_source_path(&candidate.source_area)
                || is_test_like_source_path(&candidate.target_area),
        )
    };
    let mut safest = candidates.clone();
    safest.sort_by(|left, right| {
        touches_a_suite(left)
            .cmp(&touches_a_suite(right))
            .then_with(|| left.friction_score.cmp(&right.friction_score))
            .then_with(|| left.edge_count.cmp(&right.edge_count))
            .then_with(|| {
                (left.source_area.as_str(), left.target_area.as_str())
                    .cmp(&(right.source_area.as_str(), right.target_area.as_str()))
            })
    });
    safest.truncate(limit);

    let mut most_needed = candidates;
    most_needed.sort_by(|left, right| {
        touches_a_suite(left)
            .cmp(&touches_a_suite(right))
            .then_with(|| right.friction_score.cmp(&left.friction_score))
            .then_with(|| right.edge_count.cmp(&left.edge_count))
            .then_with(|| {
                (left.source_area.as_str(), left.target_area.as_str())
                    .cmp(&(right.source_area.as_str(), right.target_area.as_str()))
            })
    });
    most_needed.truncate(limit);

    SeamReport {
        truncated: total_pairs > limit,
        total_pairs,
        safest,
        most_needed,
    }
}

pub const IMPACT_SCHEMA: &str = "codegraph.impact.v1";

pub fn impact(graph: &CodeGraph, request: ImpactRequest) -> Result<ImpactReport, QueryError> {
    let insight_report = insights(graph);
    impact_with_insights_mode(graph, request, &insight_report, true)
}

/// Exact reverse-dependency traversal without the repository-wide insight
/// pass. Intended for latency-sensitive agent calls.
pub fn impact_fast(graph: &CodeGraph, request: ImpactRequest) -> Result<ImpactReport, QueryError> {
    impact_with_insights_mode(
        graph,
        request,
        &InsightReport {
            total: 0,
            by_severity: BTreeMap::new(),
            by_kind: BTreeMap::new(),
            insights: Vec::new(),
        },
        false,
    )
}

/// [`impact`] over an already-computed insight report, so bundle callers such
/// as [`refactor_context`] evaluate insights once per request (audit F11).
pub(crate) fn impact_with_insights(
    graph: &CodeGraph,
    request: ImpactRequest,
    insight_report: &InsightReport,
) -> Result<ImpactReport, QueryError> {
    impact_with_insights_mode(graph, request, insight_report, true)
}

pub(crate) fn impact_with_insights_mode(
    graph: &CodeGraph,
    request: ImpactRequest,
    insight_report: &InsightReport,
    risks_evaluated: bool,
) -> Result<ImpactReport, QueryError> {
    let max_depth = request.max_depth.clamp(1, 32);
    let limit = request.limit.clamp(1, 1_000);
    let target = request.target.trim();
    if target.is_empty() {
        return Err(QueryError::new(
            "impact requires a `target` label or node id",
        ));
    }
    let target_id = resolve_node_reference(graph, target)
        .ok_or_else(|| node_not_found_error(graph, "impact target", target))?;
    let target_node = graph
        .nodes
        .iter()
        .find(|node| node.id == target_id)
        .cloned()
        .ok_or_else(|| node_not_found_error(graph, "impact target", target))?;
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let node_areas = node_architecture_areas(graph, &nodes_by_id);

    // Reverse adjacency built once keeps the dependents BFS O(V + E).
    let mut incoming: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for edge in &graph.edges {
        if is_trace_edge(&edge.kind) {
            incoming.entry(edge.target).or_default().push(edge.source);
        }
    }

    let mut visited = BTreeSet::from([target_id]);
    let mut queue = VecDeque::from([(target_id, 0usize)]);
    // Changing a module means changing what it declares. Nothing points at
    // an OCaml module node -- a call reaches the function, not the module
    // around it -- so asking what depends on `Path` answered nothing at all
    // while hundreds of files used what path.ml holds. The members seed the
    // walk without being dependents of themselves.
    // The same holds for a directory or a file: nothing points at a Go
    // package, so asking what depends on `internal/addrs` answered nothing
    // while its `NewDefaultProvider` alone is called 1139 times. What a
    // container holds is what changing it changes.
    if matches!(target_node.kind, NodeKind::Directory | NodeKind::File) {
        let mut contained: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        for edge in &graph.edges {
            if edge.kind == EdgeKind::Contains {
                contained.entry(edge.source).or_default().push(edge.target);
            }
        }
        // The repository holds every file directly, so a directory node
        // holds nothing to walk down from: the files it stands for are the
        // ones whose path it prefixes.
        let mut walk = VecDeque::from([target_id]);
        if target_node.kind == NodeKind::Directory {
            let prefix = format!("{}/", target_node.label);
            for node in &graph.nodes {
                if node.kind == NodeKind::File && node.label.starts_with(&prefix) {
                    walk.push_back(node.id);
                    visited.insert(node.id);
                }
            }
        }
        while let Some(current) = walk.pop_front() {
            for held in contained.get(&current).map(Vec::as_slice).unwrap_or(&[]) {
                if !visited.insert(*held) {
                    continue;
                }
                queue.push_back((*held, 0));
                walk.push_back(*held);
            }
        }
    }
    if target_node.kind == NodeKind::Module
        && let Some(module_path) = target_node.span.as_ref().map(|span| span.path.as_str())
    {
        // A module that *is* the file holds everything the file declares:
        // a Haskell module's types carry no owner, and shellcheck's
        // `ShellCheck.Analytics` is reached through those as much as
        // through its functions. A module written inside a file can claim
        // only what names it.
        let whole_file =
            target_node.metadata.get("module_scope").map(String::as_str) == Some("file");
        for node in &graph.nodes {
            if node.id == target_id
                || node.span.as_ref().map(|span| span.path.as_str()) != Some(module_path)
                || (!whole_file
                    && node.metadata.get("owner_type").map(String::as_str)
                        != Some(&target_node.label))
            {
                continue;
            }
            if visited.insert(node.id) {
                queue.push_back((node.id, 0));
            }
        }
    }
    let mut reached: Vec<(NodeId, usize)> = Vec::new();
    let mut truncated = false;
    while let Some((node_id, depth)) = queue.pop_front() {
        let sources = incoming.get(&node_id).map(Vec::as_slice).unwrap_or(&[]);
        if depth >= max_depth {
            if !sources.is_empty() {
                truncated = true;
            }
            continue;
        }
        for source in sources {
            if !visited.insert(*source) {
                continue;
            }
            reached.push((*source, depth + 1));
            queue.push_back((*source, depth + 1));
        }
    }

    let mut dependents = Vec::new();
    let mut affected_entrypoints = Vec::new();
    let mut affected_routes = 0usize;
    let mut affected_tests = 0usize;
    let mut languages: BTreeMap<String, usize> = BTreeMap::new();
    let mut areas: BTreeMap<String, usize> = BTreeMap::new();
    let mut severity_counts: BTreeMap<String, usize> = BTreeMap::new();

    for (node_id, distance) in &reached {
        let Some(node) = nodes_by_id.get(node_id) else {
            continue;
        };
        if node.kind == NodeKind::Repository {
            continue;
        }
        let risk_refs = workflow_risk_refs_for_node(insight_report, *node_id);
        for risk in &risk_refs {
            *severity_counts
                .entry(severity_name(risk.severity).to_string())
                .or_insert(0) += 1;
        }
        if let Some(language) = node.metadata.get("language") {
            *languages.entry(language.clone()).or_insert(0) += 1;
        }
        if let Some(area) = node_areas.get(node_id) {
            *areas.entry(area.clone()).or_insert(0) += 1;
        }
        let is_test = node
            .span
            .as_ref()
            .map(|span| is_test_like_source_path(&span.path))
            .or_else(|| {
                (node.kind == NodeKind::File).then(|| is_test_like_source_path(&node.label))
            })
            .unwrap_or(false);
        if is_test {
            affected_tests += 1;
        }
        if node.kind == NodeKind::Entrypoint {
            let entrypoint_kind = node.metadata.get("entrypoint_kind").cloned();
            if entrypoint_kind.as_deref() == Some("route") {
                affected_routes += 1;
            }
            affected_entrypoints.push(ImpactEntrypoint {
                node: (*node).clone(),
                entrypoint_kind,
                distance: *distance,
            });
        }
        dependents.push(ImpactDependent {
            node: (*node).clone(),
            distance: *distance,
            is_test,
            risk_count: risk_refs.len(),
        });
    }

    dependents.sort_by(|left, right| {
        left.distance
            .cmp(&right.distance)
            .then_with(|| left.is_test.cmp(&right.is_test))
            .then_with(|| dependent_source_path(left).cmp(dependent_source_path(right)))
            .then_with(|| left.node.label.cmp(&right.node.label))
    });
    // A class may be referenced by dozens of methods in one large UI file.
    // Put one representative from each source file first so a bounded agent
    // response preserves repository-wide recall before listing repetitions.
    let mut represented_paths = BTreeSet::new();
    let mut representatives = Vec::new();
    let mut repetitions = Vec::new();
    for dependent in dependents {
        if represented_paths.insert(dependent_source_path(&dependent).to_string()) {
            representatives.push(dependent);
        } else {
            repetitions.push(dependent);
        }
    }
    representatives.extend(repetitions);
    let mut dependents = representatives;
    affected_entrypoints.sort_by(|left, right| {
        left.distance
            .cmp(&right.distance)
            .then_with(|| left.node.label.cmp(&right.node.label))
    });
    let total_dependents = dependents.len();
    let listed_truncated = total_dependents > limit;
    dependents.truncate(limit);

    // The suite in the blast radius was counted twice, once inside
    // `total_dependents` and once again on its own, so a change reaching a
    // well-covered area scored as the more dangerous one. What a change can
    // reach of the program is the risk; the tests it also reaches say what to
    // run. `total_dependents - affected_tests` is the program's share.
    // A library declares its routes in its tests -- flask 297 of 307, express
    // 205 of 218 -- so weighting every affected entrypoint at five let the
    // suite back in after the line above took it out of the dependents.
    let program_entrypoints = affected_entrypoints
        .iter()
        .filter(|entrypoint| entrypoint_rank(&entrypoint.node) < TEST_ENTRYPOINT_RANK)
        .count();
    let impact_score = total_dependents.saturating_sub(affected_tests)
        + program_entrypoints * 5
        + severity_counts.get("error").copied().unwrap_or(0) * 5
        + severity_counts.get("warning").copied().unwrap_or(0) * 2
        + severity_counts.get("info").copied().unwrap_or(0);

    let suggested_commands = impact_suggested_commands(&target_node, &affected_entrypoints);

    Ok(ImpactReport {
        schema: IMPACT_SCHEMA.to_string(),
        notes: shared_name_note(graph, target, &target_node)
            .into_iter()
            .collect(),
        area: node_areas.get(&target_id).cloned(),
        target: target_node,
        max_depth,
        total_dependents,
        affected_entrypoints,
        affected_routes,
        affected_tests,
        languages,
        areas,
        severity_counts,
        risks_evaluated,
        impact_score,
        dependents,
        truncated: truncated || listed_truncated,
        suggested_commands,
    })
}

pub(crate) fn dependent_source_path(dependent: &ImpactDependent) -> &str {
    dependent
        .node
        .span
        .as_ref()
        .map(|span| span.path.as_str())
        .unwrap_or(dependent.node.label.as_str())
}

/// Copy-paste-ready CLI follow-ups for an impact report: inspect the target,
/// then read or bundle the path from the nearest affected entrypoint.
pub(crate) fn impact_suggested_commands(
    target: &Node,
    affected_entrypoints: &[ImpactEntrypoint],
) -> Vec<String> {
    let target = durable_node_reference(target);
    let mut commands = vec![format!("codegraph node-card . --node-id {target}")];
    match affected_entrypoints.first() {
        Some(entrypoint) => {
            let from = durable_node_reference(&entrypoint.node);
            commands.push(format!("codegraph journey --from {from} --to {target} ."));
            commands.push(format!(
                "codegraph refactor-context {target} . --from {from}"
            ));
        }
        None => commands.push(format!("codegraph refactor-context {target} .")),
    }
    commands
}

pub(crate) fn journey_shortest_path(
    graph: &CodeGraph,
    adjacency: &TraceAdjacency,
    start: NodeId,
    target: NodeId,
    max_depth: usize,
    banned_edges: &BTreeSet<usize>,
) -> Result<(Option<Vec<usize>>, bool), QueryError> {
    let node_ids: BTreeSet<NodeId> = graph.nodes.iter().map(|node| node.id).collect();
    let mut visited = BTreeSet::from([start]);
    let mut parents: BTreeMap<NodeId, (NodeId, usize)> = BTreeMap::new();
    let mut queue = VecDeque::from([(start, 0usize)]);
    let mut hit_depth_bound = false;

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            if adjacency
                .trace_edges(graph, node_id, TraceDirection::Outgoing)
                .any(|(edge_index, _)| !banned_edges.contains(&edge_index))
            {
                hit_depth_bound = true;
            }
            continue;
        }
        for (edge_index, edge) in adjacency.trace_edges(graph, node_id, TraceDirection::Outgoing) {
            if banned_edges.contains(&edge_index) {
                continue;
            }
            // A dangling edge target (no node — possible in a deserialized or
            // externally built graph) must not be walked: the path would later
            // skip the missing node and leave a hole with a broken step chain.
            if !node_ids.contains(&edge.target) {
                continue;
            }
            if !visited.insert(edge.target) {
                continue;
            }
            parents.insert(edge.target, (node_id, edge_index));
            if edge.target == target {
                return Ok((
                    Some(reconstruct_path_edges(start, target, &parents)?),
                    hit_depth_bound,
                ));
            }
            queue.push_back((edge.target, depth + 1));
        }
    }

    Ok((None, hit_depth_bound))
}

pub(crate) fn journey_confidence_weight(confidence: Confidence) -> usize {
    match confidence {
        Confidence::Exact => 0,
        Confidence::Semantic => 1,
        Confidence::Syntactic => 2,
        Confidence::Heuristic => 3,
        Confidence::Unknown => 4,
    }
}

pub(crate) fn journey_fragile_reasons(
    edge: &Edge,
    transition: &WorkflowTransition,
    block: &WorkflowBlock,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if matches!(edge.confidence, Confidence::Heuristic | Confidence::Unknown) {
        reasons.push("low_confidence_edge".to_string());
    }
    let risk_kinds = transition
        .risk_refs
        .iter()
        .chain(block.risk_refs.iter())
        .map(|risk| risk.kind.as_str())
        .collect::<BTreeSet<_>>();
    if risk_kinds.contains("ambiguous_call_resolution")
        || edge.metadata.get("resolution").map(String::as_str) == Some("ambiguous")
    {
        reasons.push("ambiguous_call".to_string());
    }
    if risk_kinds.contains("unresolved_call")
        || edge.metadata.get("resolution").map(String::as_str) == Some("unresolved")
    {
        reasons.push("unresolved_call".to_string());
    }
    if risk_kinds
        .iter()
        .any(|kind| kind.starts_with("duplicate_") && kind.ends_with("_label"))
    {
        reasons.push("duplicate_label".to_string());
    }
    reasons
}

pub(crate) fn journey_risk_summary(
    graph: &CodeGraph,
    adjacency: &TraceAdjacency,
    steps: &[JourneyStep],
) -> JourneyRiskSummary {
    let mut summary = JourneyRiskSummary::default();
    let mut step_order: BTreeMap<NodeId, usize> = BTreeMap::new();
    for (index, step) in steps.iter().enumerate() {
        step_order.entry(step.block.node.id).or_insert(index);
    }

    for step in steps {
        if !step.block.risk_refs.is_empty() {
            summary.risky_steps += 1;
        }
        for risk in &step.block.risk_refs {
            *summary
                .severity_counts
                .entry(severity_name(risk.severity).to_string())
                .or_insert(0) += 1;
        }
        if let Some(transition) = &step.transition {
            if !transition.risk_refs.is_empty() {
                summary.risky_transitions += 1;
            }
            for risk in &transition.risk_refs {
                *summary
                    .severity_counts
                    .entry(severity_name(risk.severity).to_string())
                    .or_insert(0) += 1;
            }
        }
        if step.fragile {
            summary.fragile_transitions += 1;
        }
        for reason in &step.fragile_reasons {
            match reason.as_str() {
                "low_confidence_edge" => summary.low_confidence_hops += 1,
                "ambiguous_call" => summary.ambiguous_calls += 1,
                "unresolved_call" => summary.unresolved_calls += 1,
                "duplicate_label" => summary.duplicate_labels += 1,
                _ => {}
            }
        }
    }

    for (step_index, step) in steps.iter().enumerate() {
        for (_, edge) in adjacency.trace_edges(graph, step.block.node.id, TraceDirection::Outgoing)
        {
            if step_order
                .get(&edge.target)
                .is_some_and(|target_index| *target_index <= step_index)
            {
                summary.cycle_back_edges += 1;
            }
        }
    }

    summary
}

pub(crate) fn journey_hop_explanation(edge: &Edge) -> JourneyHopExplanation {
    let confidence = confidence_name(edge.confidence).to_string();
    let confidence_note = confidence_evidence(edge.confidence)
        .trim_start_matches("confidence_note=")
        .to_string();
    let relation = edge.metadata.get("relation").cloned();
    let provenance_source = edge
        .metadata
        .get("source")
        .or_else(|| edge.metadata.get("parser"))
        .cloned();
    let mut summary = format!(
        "{} edge ({confidence}): {confidence_note}",
        edge_kind_name(&edge.kind)
    );
    if let Some(relation) = &relation {
        summary.push_str(&format!(", relation {relation}"));
    }
    if let Some(source) = &provenance_source {
        summary.push_str(&format!(", via {source}"));
    }
    JourneyHopExplanation {
        confidence,
        confidence_note,
        relation,
        provenance_source,
        summary,
    }
}

/// Where a step happens. A call the resolver could not place -- one a
/// dependency or the platform answers -- has no definition to open, so
/// its placeholder carries the span of whichever call site minted it:
/// 14620 of koel's 17732 calls into such a node name a file other than
/// the one the call is written in, and a step that opens the wrong file
/// is worse than one that opens none. The edge that reached it says where
/// this step is -- the caller's file, at the line the call is written on.
fn workflow_step_node(node: &Node, incoming_edge: Option<&Edge>, caller: Option<&Node>) -> Node {
    let mut step = node.clone();
    if node.kind != NodeKind::ExternalDependency {
        return step;
    }
    let Some(edge) = incoming_edge else {
        return step;
    };
    let (Some(line), Some(column)) = (
        edge.metadata
            .get("line")
            .and_then(|line| line.parse::<u32>().ok()),
        edge.metadata
            .get("column")
            .and_then(|column| column.parse::<u32>().ok()),
    ) else {
        return step;
    };
    let Some(path) = caller
        .and_then(|caller| caller.span.as_ref())
        .map(|span| span.path.clone())
    else {
        return step;
    };
    step.span = Some(SourceSpan {
        path,
        start_line: line,
        start_column: column,
        end_line: line,
        end_column: column,
    });
    step
}

pub(crate) fn journey_block(
    insight_report: &InsightReport,
    graph: &CodeGraph,
    node: &Node,
    incoming_edge: Option<&Edge>,
    is_start: bool,
    depth: usize,
) -> WorkflowBlock {
    let caller =
        incoming_edge.and_then(|edge| graph.nodes.iter().find(|node| node.id == edge.source));
    WorkflowBlock {
        id: workflow_block_id(node.id),
        kind: workflow_block_kind(node, incoming_edge, is_start),
        node: workflow_step_node(node, incoming_edge, caller),
        depth,
        source_node_ids: vec![node.id],
        risk_refs: workflow_risk_refs_for_node(insight_report, node.id),
        compacted: false,
        compacted_count: 1,
        truncated_children: 0,
    }
}

// Priority for fan-out capping: lower is kept first. The call chain (Calls)
// wins, then config/env reads and control-relevant references, with imports and
// dependency edges dropped first since they are the least useful for following
// execution.
fn workflow_edge_priority(kind: &EdgeKind) -> u8 {
    match kind {
        EdgeKind::Calls => 0,
        EdgeKind::ReadsConfig | EdgeKind::ReadsEnvironment => 1,
        EdgeKind::References | EdgeKind::Defines | EdgeKind::Entrypoint => 2,
        EdgeKind::MayError => 3,
        EdgeKind::Imports | EdgeKind::DependsOn | EdgeKind::Contains => 4,
    }
}

/// Outgoing edges the workflow traversal follows from `node_id`: the trace edges
/// that pass the active filters, plus — when the node is a container (file or
/// directory) that has no execution flow of its own — the `Contains` edges to
/// the code symbols (and nested containers) it defines. That way a flow rooted
/// on a file expands into the flows of the functions it holds instead of
/// dead-ending on a lonely "leaf" block.
///
/// `expand_contains` says whether this node is one the flow is rooted on: the
/// start container, or a container the start expands into. A container the flow
/// only *arrives at* — the file a route names as its definition site, a module
/// an import reaches — is a landmark on the way, not the subject. Expanding it
/// would replace the flow being followed with everything else the file happens
/// to hold: from one mastodon route that was 196 unrelated sibling routes,
/// crowding the handler's own call chain out of the block budget.
fn workflow_followable_edges<'graph>(
    graph: &'graph CodeGraph,
    adjacency: &TraceAdjacency,
    nodes_by_id: &BTreeMap<NodeId, &'graph Node>,
    node_id: NodeId,
    filters: &WorkflowFilters,
    expand_contains: bool,
) -> Vec<(usize, &'graph Edge)> {
    let mut edges: Vec<(usize, &Edge)> = adjacency
        .trace_edges(graph, node_id, TraceDirection::Outgoing)
        .filter(|(_, edge)| workflow_edge_filter_matches(edge, filters))
        .collect();
    if expand_contains
        && matches!(
            nodes_by_id.get(&node_id).map(|node| &node.kind),
            Some(NodeKind::File | NodeKind::Directory)
        )
    {
        for (edge_index, edge) in adjacency.contains_edges(graph, node_id) {
            if edge.kind == EdgeKind::Contains
                // The same edge filter the transitions are later held to
                // (workflow_transition_filter_matches); without it an
                // edge_kind/confidence filter would drop the contains
                // transition but keep the expanded block, leaving it orphaned.
                && workflow_edge_filter_matches(edge, filters)
                && nodes_by_id.get(&edge.target).is_some_and(|node| {
                    is_code_symbol(&node.kind)
                        || matches!(node.kind, NodeKind::File | NodeKind::Directory)
                })
            {
                edges.push((edge_index, edge));
            }
        }
    }
    edges
}

pub(crate) fn workflow_with_insight_report(
    graph: &CodeGraph,
    request: WorkflowRequest,
    insight_report: &InsightReport,
) -> Option<WorkflowReport> {
    let max_depth = request.max_depth.clamp(1, 32);
    let block_limit = request.block_limit.clamp(1, 1_000);
    // Normalize the fan-out cap here so every surface (HTTP, CLI, MCP) behaves
    // identically regardless of what it passes: `Some(0)` means "narrowest",
    // not "unbounded", and the cap never exceeds 200. `None` stays unbounded.
    let max_fanout = request.max_fanout.map(|value| value.clamp(1, 200));
    let filters = normalize_workflow_filters(request.filters);
    let start = resolve_trace_start(graph, &request.start)?.clone();
    let notes = match &request.start {
        TraceStart::Label(label) => shared_name_note(graph, label, &start).into_iter().collect(),
        TraceStart::NodeId(_) => Vec::new(),
    };
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let adjacency = TraceAdjacency::build(graph);

    let mut visited = BTreeSet::new();
    let mut depths = BTreeMap::new();
    let mut incoming = BTreeMap::new();
    let mut queue = VecDeque::new();
    let mut transition_indexes = BTreeSet::new();
    let mut truncated = false;
    let mut truncated_children: BTreeMap<NodeId, usize> = BTreeMap::new();
    // Containers the flow is rooted on: the start, and any container the start
    // expands into (a directory holds files that hold symbols). Only these
    // expand into what they contain -- see `workflow_followable_edges`.
    let mut rooted_containers = BTreeSet::new();

    visited.insert(start.id);
    depths.insert(start.id, 0);
    rooted_containers.insert(start.id);
    queue.push_back((start.id, 0));

    while let Some((node_id, depth)) = queue.pop_front() {
        let expand_contains = rooted_containers.contains(&node_id);
        if depth >= max_depth {
            if !workflow_followable_edges(
                graph,
                &adjacency,
                &nodes_by_id,
                node_id,
                &filters,
                expand_contains,
            )
            .is_empty()
            {
                truncated = true;
            }
            continue;
        }

        let mut followable = workflow_followable_edges(
            graph,
            &adjacency,
            &nodes_by_id,
            node_id,
            &filters,
            expand_contains,
        );
        // Cap the fan-out per node, keeping the highest-priority edges (calls
        // first), so the block budget follows the call chain into depth instead
        // of exhausting on one wide node. Unset = unbounded breadth (default).
        if let Some(max_fanout) = max_fanout.filter(|value| followable.len() > *value) {
            followable.sort_by_key(|(edge_index, edge)| {
                // Among equal edge kinds the order used to be the order the
                // call sites appear in the file, so a capped node kept
                // whichever calls were written first — on terraform's `main`
                // that meant `fmt.Sprintf` and a tracing helper while the
                // branch into the command layer was dropped. Keep what leads
                // somewhere: code in this repository before a boundary out of
                // it, and a target with a flow of its own before a leaf.
                let target = nodes_by_id.get(&edge.target);
                let leaves_repository = target.is_none_or(|node| {
                    node.kind == NodeKind::ExternalDependency || node.kind == NodeKind::Unknown
                });
                let is_leaf = workflow_followable_edges(
                    graph,
                    &adjacency,
                    &nodes_by_id,
                    edge.target,
                    &filters,
                    expand_contains && edge.kind == EdgeKind::Contains,
                )
                .is_empty();
                (
                    workflow_edge_priority(&edge.kind),
                    u8::from(leaves_repository),
                    u8::from(is_leaf),
                    *edge_index,
                )
            });
            truncated_children.insert(node_id, followable.len() - max_fanout);
            followable.truncate(max_fanout);
            truncated = true;
        }
        for (edge_index, edge) in followable {
            let next = edge.target;
            if expand_contains && edge.kind == EdgeKind::Contains {
                rooted_containers.insert(next);
            }
            if visited.contains(&next) {
                transition_indexes.insert(edge_index);
                continue;
            }
            if visited.len() >= block_limit {
                truncated = true;
                continue;
            }
            visited.insert(next);
            depths.insert(next, depth + 1);
            incoming.insert(next, edge_index);
            transition_indexes.insert(edge_index);
            queue.push_back((next, depth + 1));
        }
    }

    // A workflow is a flow, so its blocks come out in the order the flow
    // reaches them: the target first, then each step away from it. Filtering
    // the graph by the visited set returned them in file-walk order, which
    // opened ripgrep's `main` workflow with `from_low_args`.
    let mut visited_in_flow_order: Vec<NodeId> = visited.iter().copied().collect();
    visited_in_flow_order.sort_by_key(|node_id| {
        (
            depths.get(node_id).copied().unwrap_or(usize::MAX),
            node_id.0,
        )
    });
    let nodes_in_flow_order = visited_in_flow_order
        .into_iter()
        .filter_map(|node_id| nodes_by_id.get(&node_id).copied());
    let all_blocks = nodes_in_flow_order
        .map(|node| {
            let incoming_edge = incoming
                .get(&node.id)
                .and_then(|edge_index| graph.edges.get(*edge_index));
            let caller = incoming_edge.and_then(|edge| nodes_by_id.get(&edge.source).copied());
            WorkflowBlock {
                id: workflow_block_id(node.id),
                kind: workflow_block_kind(node, incoming_edge, node.id == start.id),
                node: workflow_step_node(node, incoming_edge, caller),
                depth: depths.get(&node.id).copied().unwrap_or(0),
                source_node_ids: vec![node.id],
                risk_refs: workflow_risk_refs_for_node(insight_report, node.id),
                compacted: false,
                compacted_count: 1,
                truncated_children: truncated_children.get(&node.id).copied().unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    let all_transitions = transition_indexes
        .iter()
        .filter_map(|edge_index| {
            let edge = graph.edges.get(*edge_index)?;
            if !visited.contains(&edge.source) || !visited.contains(&edge.target) {
                return None;
            }
            Some(WorkflowTransition {
                id: format!("wt-{edge_index}"),
                source: workflow_block_id(edge.source),
                target: workflow_block_id(edge.target),
                source_node_id: edge.source,
                target_node_id: edge.target,
                edge: edge_with_index(*edge_index, edge),
                edge_index: *edge_index,
                risk_refs: workflow_risk_refs_for_edge(insight_report, *edge_index),
                compacted: false,
                compacted_count: 1,
            })
        })
        .collect::<Vec<_>>();
    let included_node_ids =
        workflow_included_node_ids(&start, &all_blocks, &all_transitions, &filters);
    let blocks = all_blocks
        .into_iter()
        .filter(|block| included_node_ids.contains(&block.node.id))
        .collect::<Vec<_>>();
    let transitions = all_transitions
        .into_iter()
        .filter(|transition| {
            included_node_ids.contains(&transition.source_node_id)
                && included_node_ids.contains(&transition.target_node_id)
                && workflow_transition_filter_matches(transition, &filters)
        })
        .collect::<Vec<_>>();
    let raw_total_blocks = blocks.len();
    let raw_total_transitions = transitions.len();
    let (blocks, transitions) = if request.compact {
        compact_workflow_blocks_and_transitions(blocks, transitions)
    } else {
        (blocks, transitions)
    };

    Some(WorkflowReport {
        notes,
        start,
        max_depth,
        block_limit,
        filters,
        compact: request.compact,
        total_blocks: blocks.len(),
        total_transitions: transitions.len(),
        raw_total_blocks,
        raw_total_transitions,
        blocks,
        transitions,
        truncated,
    })
}

pub(crate) fn compact_workflow_blocks_and_transitions(
    blocks: Vec<WorkflowBlock>,
    transitions: Vec<WorkflowTransition>,
) -> (Vec<WorkflowBlock>, Vec<WorkflowTransition>) {
    let mut group_members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, block) in blocks.iter().enumerate() {
        if let Some(group_key) = workflow_compaction_group_key(block) {
            group_members.entry(group_key).or_default().push(index);
        }
    }
    group_members.retain(|_, members| members.len() > 1);
    if group_members.is_empty() {
        return (blocks, transitions);
    }

    let mut original_to_compact_block = BTreeMap::new();
    let mut compact_blocks = Vec::new();
    for (group_index, (group_key, members)) in group_members.iter().enumerate() {
        let compact_id = format!("wc-{}", group_index + 1);
        for member in members {
            original_to_compact_block.insert(blocks[*member].id.clone(), compact_id.clone());
        }
        compact_blocks.push(workflow_compacted_block(
            group_index,
            group_key,
            members.iter().map(|index| &blocks[*index]).collect(),
        ));
    }

    let mut visible_blocks = blocks
        .iter()
        .filter(|block| !original_to_compact_block.contains_key(&block.id))
        .cloned()
        .collect::<Vec<_>>();
    visible_blocks.extend(compact_blocks);
    visible_blocks.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then_with(|| {
                workflow_block_kind_label(&left.kind).cmp(workflow_block_kind_label(&right.kind))
            })
            .then_with(|| left.node.label.cmp(&right.node.label))
            .then_with(|| left.id.cmp(&right.id))
    });

    let compact_block_ids = visible_blocks
        .iter()
        .filter(|block| block.compacted)
        .map(|block| block.id.clone())
        .collect::<BTreeSet<_>>();
    let mut transition_by_key: BTreeMap<(String, String, String, String), WorkflowTransition> =
        BTreeMap::new();
    for transition in transitions {
        let source = original_to_compact_block
            .get(&transition.source)
            .cloned()
            .unwrap_or_else(|| transition.source.clone());
        let target = original_to_compact_block
            .get(&transition.target)
            .cloned()
            .unwrap_or_else(|| transition.target.clone());
        if source == target {
            continue;
        }

        let key = (
            source.clone(),
            target.clone(),
            edge_kind_name(&transition.edge.kind),
            confidence_name(transition.edge.confidence),
        );
        if let Some(existing) = transition_by_key.get_mut(&key) {
            existing.compacted = true;
            existing.compacted_count += transition.compacted_count.max(1);
            existing.risk_refs.extend(transition.risk_refs);
            continue;
        }

        let mut transition = transition;
        transition.id = format!("wtc-{}", transition.edge_index);
        transition.source = source.clone();
        transition.target = target.clone();
        transition.compacted = transition.compacted
            || compact_block_ids.contains(&source)
            || compact_block_ids.contains(&target);
        transition.compacted_count = transition.compacted_count.max(1);
        transition_by_key.insert(key, transition);
    }

    (visible_blocks, transition_by_key.into_values().collect())
}

pub(crate) fn workflow_compaction_group_key(block: &WorkflowBlock) -> Option<String> {
    if block.compacted || !block.risk_refs.is_empty() || block.kind == WorkflowBlockKind::Start {
        return None;
    }
    let low_signal_kind = matches!(
        block.kind,
        WorkflowBlockKind::Call
            | WorkflowBlockKind::Import
            | WorkflowBlockKind::Reference
            | WorkflowBlockKind::Unknown
    );
    if !low_signal_kind {
        return None;
    }
    let language = block
        .node
        .metadata
        .get("language")
        .map(String::as_str)
        .unwrap_or("unknown");
    Some(format!(
        "{}:{}:{}",
        block.depth,
        workflow_block_kind_filter_name(&block.kind),
        language
    ))
}

pub(crate) fn workflow_compacted_block(
    group_index: usize,
    group_key: &str,
    members: Vec<&WorkflowBlock>,
) -> WorkflowBlock {
    let representative = members
        .first()
        .expect("workflow compaction groups are non-empty");
    let count = members.len();
    let label = workflow_compacted_label(group_key, count, &representative.kind);
    let mut source_node_ids = members
        .iter()
        .flat_map(|block| block.source_node_ids.iter().copied())
        .collect::<Vec<_>>();
    source_node_ids.sort();
    source_node_ids.dedup();

    WorkflowBlock {
        id: format!("wc-{}", group_index + 1),
        kind: representative.kind.clone(),
        node: Node {
            id: NodeId(9_000_000_000 + group_index as u64 + 1),
            kind: NodeKind::Unknown,
            label,
            span: None,
            metadata: BTreeMap::from([
                ("compacted".to_string(), "true".to_string()),
                ("compacted_count".to_string(), count.to_string()),
                (
                    "compacted_kind".to_string(),
                    workflow_block_kind_filter_name(&representative.kind),
                ),
            ]),
        },
        depth: representative.depth,
        source_node_ids,
        risk_refs: Vec::new(),
        compacted: true,
        compacted_count: count,
        truncated_children: 0,
    }
}

pub(crate) fn workflow_compacted_label(
    group_key: &str,
    count: usize,
    kind: &WorkflowBlockKind,
) -> String {
    let mut parts = group_key.split(':');
    let _depth = parts.next();
    let kind_name = parts
        .next()
        .unwrap_or_else(|| workflow_block_kind_label(kind));
    let language = parts.next().unwrap_or("unknown");
    if language == "unknown" {
        format!("{count} compacted {kind_name} blocks")
    } else {
        format!("{count} compacted {language} {kind_name} blocks")
    }
}
