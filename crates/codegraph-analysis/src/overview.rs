//! Project overview reports: summary, architecture map, language
//! dependencies, surprising links, hotspots, communities, entrypoints.

use codegraph_core::{
    CodeGraph, Confidence, Edge, EdgeKind, Node, NodeId, NodeKind, is_vendored_source_path,
};
use std::collections::{BTreeMap, BTreeSet};

#[allow(unused_imports)]
use crate::*;

pub fn summarize(graph: &CodeGraph) -> GraphSummary {
    let mut node_kinds = BTreeMap::new();
    let mut edge_kinds = BTreeMap::new();
    let mut edge_confidences = BTreeMap::new();
    let mut edge_relations = BTreeMap::new();
    let mut edge_sources = BTreeMap::new();
    let mut languages = BTreeMap::new();
    let mut annotation_facets: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut skipped_files = 0;

    for node in &graph.nodes {
        *node_kinds.entry(kind_name(&node.kind)).or_insert(0) += 1;
        if matches!(node.kind, NodeKind::File)
            && node
                .metadata
                .get("skipped")
                .is_some_and(|value| value == "true")
        {
            skipped_files += 1;
        }
        if let Some(language) = node.metadata.get("language") {
            *languages.entry(language.clone()).or_insert(0) += 1;
        }
        for (key, value) in &node.metadata {
            if !key.starts_with("annotation.") || value.trim().is_empty() {
                continue;
            }
            *annotation_facets
                .entry(key.clone())
                .or_default()
                .entry(value.clone())
                .or_insert(0) += 1;
        }
    }

    for edge in &graph.edges {
        *edge_kinds.entry(edge_kind_name(&edge.kind)).or_insert(0) += 1;
        *edge_confidences
            .entry(confidence_name(edge.confidence))
            .or_insert(0) += 1;
        if let Some(relation) = edge
            .metadata
            .get("relation")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            *edge_relations.entry(relation.to_string()).or_insert(0) += 1;
        }
        if let Some(source) = edge
            .metadata
            .get("source")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            *edge_sources.entry(source.to_string()).or_insert(0) += 1;
        }
    }

    GraphSummary {
        nodes: graph.nodes.len(),
        edges: graph.edges.len(),
        node_kinds,
        edge_kinds,
        edge_confidences,
        edge_relations,
        edge_sources,
        languages,
        annotation_facets,
        entrypoints: graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Entrypoint)
            .count(),
        skipped_files,
    }
}

pub fn architecture_map(
    graph: &CodeGraph,
    group_limit: usize,
    edge_limit: usize,
) -> ArchitectureMap {
    let group_limit = group_limit.clamp(1, 500);
    let edge_limit = edge_limit.clamp(1, 2_000);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut node_groups = BTreeMap::new();
    let mut groups: BTreeMap<String, ArchitectureGroup> = BTreeMap::new();

    let project = ProjectAreas::from_graph(graph);
    for node in &graph.nodes {
        if node.kind != NodeKind::File {
            continue;
        }
        let (id, label) = project.group_for_path(&node.label);
        let group = groups
            .entry(id.clone())
            .or_insert_with(|| ArchitectureGroup {
                id: id.clone(),
                label,
                files: 0,
                symbols: 0,
                entrypoints: 0,
                skipped_files: 0,
                languages: BTreeMap::new(),
            });
        group.files += 1;
        if node
            .metadata
            .get("skipped")
            .is_some_and(|value| value == "true")
        {
            group.skipped_files += 1;
        }
        if let Some(language) = node.metadata.get("language") {
            *group.languages.entry(language.clone()).or_insert(0) += 1;
        }
        node_groups.insert(node.id, id);
    }

    for edge in &graph.edges {
        if edge.kind != EdgeKind::Contains {
            continue;
        }
        let Some(source_group) = node_groups.get(&edge.source).cloned() else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target) else {
            continue;
        };
        node_groups.entry(target.id).or_insert(source_group);
    }

    for node in &graph.nodes {
        let Some(group_id) = node_groups.get(&node.id) else {
            continue;
        };
        let Some(group) = groups.get_mut(group_id) else {
            continue;
        };
        if is_architecture_symbol(&node.kind) {
            group.symbols += 1;
        }
        if node.kind == NodeKind::Entrypoint {
            group.entrypoints += 1;
        }
    }

    let mut edges: BTreeMap<(String, String), ArchitectureEdge> = BTreeMap::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind) {
            continue;
        }
        let Some(source_group) = node_groups.get(&edge.source) else {
            continue;
        };
        let Some(target_group) = node_groups.get(&edge.target) else {
            continue;
        };
        if source_group == target_group {
            continue;
        }
        let key = (source_group.clone(), target_group.clone());
        let architecture_edge = edges.entry(key).or_insert_with(|| ArchitectureEdge {
            source: source_group.clone(),
            target: target_group.clone(),
            count: 0,
            edge_kinds: BTreeMap::new(),
            confidences: BTreeMap::new(),
            edge_indexes: Vec::new(),
        });
        architecture_edge.count += 1;
        if architecture_edge.edge_indexes.len() < LINK_EVIDENCE_LIMIT {
            architecture_edge.edge_indexes.push(edge_index);
        }
        *architecture_edge
            .edge_kinds
            .entry(edge_kind_name(&edge.kind))
            .or_insert(0) += 1;
        *architecture_edge
            .confidences
            .entry(confidence_name(edge.confidence))
            .or_insert(0) += 1;
    }

    let total_groups = groups.len();
    let total_edges = edges.len();
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|left, right| {
        right
            .files
            .cmp(&left.files)
            .then_with(|| right.symbols.cmp(&left.symbols))
            .then_with(|| left.label.cmp(&right.label))
    });
    let mut edges: Vec<_> = edges.into_values().collect();
    edges.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
    });
    groups.truncate(group_limit);
    edges.truncate(edge_limit);

    ArchitectureMap {
        groups,
        edges,
        total_groups,
        total_edges,
        truncated_groups: total_groups > group_limit,
        truncated_edges: total_edges > edge_limit,
    }
}

/// How many edge indexes a grouped link keeps as evidence.
///
/// The indexes exist so a reader can jump to a few real edges behind a
/// summary; keeping every one turns a summary back into the graph. terraform's
/// go->go link alone held 190459 of them — 1.4MB, 78% of the whole report,
/// which is unusable for an agent that has to read it.
const LINK_EVIDENCE_LIMIT: usize = 100;

pub fn language_dependencies(graph: &CodeGraph, limit: usize) -> LanguageDependencyReport {
    let limit = limit.clamp(1, 500);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut links: BTreeMap<(String, String), LanguageDependency> = BTreeMap::new();
    let mut total_edges = 0;
    let mut cross_language_edges = 0;

    for (edge_index, edge) in graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| is_architecture_dependency_edge(&edge.kind))
    {
        let source_language = node_language(&nodes_by_id, edge.source);
        let target_language = node_language(&nodes_by_id, edge.target);
        if source_language == "unknown" && target_language == "unknown" {
            continue;
        }
        total_edges += 1;
        if source_language != target_language {
            cross_language_edges += 1;
        }

        let link = links
            .entry((source_language.clone(), target_language.clone()))
            .or_insert_with(|| LanguageDependency {
                source_language: source_language.clone(),
                target_language: target_language.clone(),
                count: 0,
                edge_kinds: BTreeMap::new(),
                confidences: BTreeMap::new(),
                edge_indexes: Vec::new(),
            });
        link.count += 1;
        *link
            .edge_kinds
            .entry(edge_kind_name(&edge.kind))
            .or_insert(0) += 1;
        *link
            .confidences
            .entry(confidence_name(edge.confidence))
            .or_insert(0) += 1;
        if link.edge_indexes.len() < LINK_EVIDENCE_LIMIT {
            link.edge_indexes.push(edge_index);
        }
    }

    let total_links = links.len();
    let mut links: Vec<_> = links.into_values().collect();
    links.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.source_language.cmp(&right.source_language))
            .then_with(|| left.target_language.cmp(&right.target_language))
    });
    links.truncate(limit);

    LanguageDependencyReport {
        links,
        total_links,
        total_edges,
        cross_language_edges,
        truncated: total_links > limit,
    }
}

/// How few edges an ordered pair of areas may exchange and still count as a
/// one-off. A median would not do: with three pairs half of them are below
/// it by definition, and this must mean the same thing in a small project
/// as in terraform, where 36% of the 874 pairs touch three times or fewer
/// and account for 546 of the 58566 crossings.
const MAX_RARE_CROSSING_EDGES: usize = 3;

pub fn surprising_links(graph: &CodeGraph, limit: usize) -> SurprisingLinkReport {
    let limit = limit.clamp(1, MAX_REPORT_ARCHITECTURE_EDGE_LIMIT);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let node_areas = node_architecture_areas(graph, &nodes_by_id);
    let mut links = Vec::new();

    // How often each ordered pair of areas exchanges an edge, so the score
    // can tell a one-off crossing from the project's own structure.
    let mut crossing_counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for edge in &graph.edges {
        if !is_architecture_dependency_edge(&edge.kind) {
            continue;
        }
        let (Some(source_area), Some(target_area)) =
            (node_areas.get(&edge.source), node_areas.get(&edge.target))
        else {
            continue;
        };
        if source_area != target_area {
            *crossing_counts
                .entry((source_area.clone(), target_area.clone()))
                .or_default() += 1;
        }
    }

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_architecture_dependency_edge(&edge.kind) {
            continue;
        }
        let Some(source) = nodes_by_id.get(&edge.source) else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target) else {
            continue;
        };
        // A markdown file that mentions a symbol is not a dependency the
        // system has. Such a link crosses an area, crosses a language and
        // is rare by construction, so it scored above every real one.
        if is_document_query_node(source) {
            continue;
        }
        // An entrypoint reaching the code that serves it is the
        // architecture, not a surprise: it crosses an area by
        // construction -- a route sits in `routes/` and its controller in
        // `app/` -- and eight of koel's top ten links were a route
        // reaching its own `__invoke`.
        if edge
            .metadata
            .get("relation")
            .is_some_and(|relation| relation.starts_with("entrypoint_"))
        {
            continue;
        }
        let source_area = node_areas
            .get(&edge.source)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let target_area = node_areas
            .get(&edge.target)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let source_language = node_language(&nodes_by_id, edge.source);
        let target_language = node_language(&nodes_by_id, edge.target);
        let mut score = 0;
        let mut reasons = Vec::new();

        if source_area != "unknown" && target_area != "unknown" && source_area != target_area {
            score += 5;
            reasons.push("cross_area".to_string());
            // Two subsystems that exchange a thousand calls are the
            // architecture, not a surprise. One that talks to another once
            // is the link worth looking at, and without this the score
            // could not tell them apart: terraform crosses an area 58566
            // times across 874 pairs, so every edge scored the same.
            if crossing_counts
                .get(&(source_area.clone(), target_area.clone()))
                .is_some_and(|count| *count <= MAX_RARE_CROSSING_EDGES)
            {
                score += 3;
                reasons.push("rare_crossing".to_string());
            }
        }
        if source_language != "unknown"
            && target_language != "unknown"
            && source_language != target_language
        {
            score += 4;
            reasons.push("cross_language".to_string());
        }
        match edge.confidence {
            Confidence::Heuristic => {
                score += 3;
                reasons.push("heuristic_confidence".to_string());
            }
            Confidence::Unknown => {
                score += 2;
                reasons.push("unknown_confidence".to_string());
            }
            Confidence::Semantic | Confidence::Syntactic | Confidence::Exact => {}
        }
        if matches!(
            edge.kind,
            EdgeKind::MayError
                | EdgeKind::ReadsConfig
                | EdgeKind::ReadsEnvironment
                | EdgeKind::DependsOn
        ) {
            score += 2;
            reasons.push(format!("edge_kind:{}", edge_kind_name(&edge.kind)));
        }
        if source.kind == NodeKind::Entrypoint || target.kind == NodeKind::Entrypoint {
            score += 1;
            reasons.push("entrypoint_boundary".to_string());
        }
        if score == 0 {
            continue;
        }

        links.push(SurprisingLink {
            source: (*source).clone(),
            target: (*target).clone(),
            source_area,
            target_area,
            source_language,
            target_language,
            edge_kind: edge_kind_name(&edge.kind),
            confidence: confidence_name(edge.confidence),
            score,
            reasons,
            edge_index,
        });
    }

    links.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.source_area.cmp(&right.source_area))
            .then_with(|| left.target_area.cmp(&right.target_area))
            .then_with(|| left.edge_index.cmp(&right.edge_index))
    });
    let total_candidates = links.len();
    links.truncate(limit);

    SurprisingLinkReport {
        links,
        total_candidates,
        truncated: total_candidates > limit,
    }
}

pub fn hotspots(graph: &CodeGraph, limit: usize) -> HotspotReport {
    let limit = limit.clamp(1, 500);
    let mut hotspots = hotspot_stats(graph, |_| true, NeighborDirection::Both);
    let total_candidates = hotspots.len();
    let architectural_candidates: Vec<_> = hotspots
        .iter()
        .filter(|hotspot| hotspot.hub_kind == "architectural")
        .cloned()
        .collect();
    let utility_candidates: Vec<_> = hotspots
        .iter()
        .filter(|hotspot| hotspot.hub_kind == "utility")
        .cloned()
        .collect();
    let total_architectural_hubs = architectural_candidates.len();
    let total_utility_hubs = utility_candidates.len();
    let mut architectural_hubs = architectural_candidates;
    let mut utility_hubs = utility_candidates;
    hotspots.truncate(limit);
    architectural_hubs.truncate(limit);
    utility_hubs.truncate(limit);

    HotspotReport {
        hotspots,
        architectural_hubs,
        utility_hubs,
        total_candidates,
        total_architectural_hubs,
        total_utility_hubs,
        truncated: total_candidates > limit,
    }
}

pub fn communities(graph: &CodeGraph, limit: usize) -> CommunityReport {
    let limit = limit.clamp(1, MAX_REPORT_COMMUNITY_LIMIT);
    let nodes_by_id: BTreeMap<NodeId, &Node> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let project = ProjectAreas::from_graph(graph);
    let mut node_community: BTreeMap<NodeId, String> = BTreeMap::new();
    let mut components: BTreeMap<String, BTreeSet<NodeId>> = BTreeMap::new();

    for node in graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
    {
        let (group_id, _) = project.group_for_path(&node.label);
        let community_id = format!("area:{group_id}");
        node_community.insert(node.id, community_id.clone());
        components.entry(community_id).or_default().insert(node.id);
    }

    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Contains)
    {
        let Some(source_community) = node_community.get(&edge.source).cloned() else {
            continue;
        };
        let Some(target) = nodes_by_id.get(&edge.target) else {
            continue;
        };
        if is_architecture_symbol(&target.kind) {
            node_community.insert(edge.target, source_community.clone());
            components
                .entry(source_community)
                .or_default()
                .insert(edge.target);
        }
    }

    for node in graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File || is_architecture_symbol(&node.kind))
    {
        if node_community.contains_key(&node.id) {
            continue;
        }
        let community_id = format!("isolated:{}", node.id.0);
        node_community.insert(node.id, community_id.clone());
        components.entry(community_id).or_default().insert(node.id);
    }

    let mut communities: Vec<_> = components
        .into_iter()
        .map(|(community_id, component)| {
            graph_community(
                graph,
                &nodes_by_id,
                &node_community,
                community_id,
                component,
            )
        })
        .collect();
    let total_communities = communities.len();
    let total_nodes = communities
        .iter()
        .map(|community| community.node_count)
        .sum();
    let total_internal_edges = communities
        .iter()
        .map(|community| community.internal_edges)
        .sum();
    let total_external_edges = graph
        .edges
        .iter()
        .filter(|edge| is_community_report_edge(edge))
        .filter_map(|edge| {
            let source_community = node_community.get(&edge.source)?;
            let target_community = node_community.get(&edge.target)?;
            (source_community != target_community).then_some(())
        })
        .count();
    communities.sort_by(|left, right| {
        right
            .node_count
            .cmp(&left.node_count)
            .then_with(|| right.internal_edges.cmp(&left.internal_edges))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });
    communities.truncate(limit);

    CommunityReport {
        communities,
        total_communities,
        total_nodes,
        total_internal_edges,
        total_external_edges,
        truncated: total_communities > limit,
    }
}

pub(crate) fn graph_community(
    graph: &CodeGraph,
    nodes_by_id: &BTreeMap<NodeId, &Node>,
    node_community: &BTreeMap<NodeId, String>,
    community_id: String,
    component: BTreeSet<NodeId>,
) -> GraphCommunity {
    let mut files = 0;
    let mut entrypoints = 0;
    let mut languages = BTreeMap::new();
    let mut node_kinds = BTreeMap::new();
    let mut area_counts: BTreeMap<String, usize> = BTreeMap::new();
    let project = ProjectAreas::from_graph(graph);
    let mut nodes: Vec<Node> = component
        .iter()
        .filter_map(|id| nodes_by_id.get(id).map(|node| (*node).clone()))
        .collect();
    nodes.sort_by(|left, right| {
        node_rank(&left.kind)
            .cmp(&node_rank(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.id.cmp(&right.id))
    });

    for node in &nodes {
        if node.kind == NodeKind::File {
            files += 1;
            let (_, area) = project.group_for_path(&node.label);
            *area_counts.entry(area).or_insert(0) += 1;
        }
        if node.kind == NodeKind::Entrypoint {
            entrypoints += 1;
        }
        if let Some(language) = node
            .metadata
            .get("language")
            .map(String::as_str)
            .filter(|language| !language.trim().is_empty())
        {
            *languages.entry(language.to_string()).or_insert(0) += 1;
        }
        *node_kinds.entry(kind_name(&node.kind)).or_insert(0) += 1;
    }

    let mut internal_edges = 0;
    let mut incoming_external_edges = 0;
    let mut outgoing_external_edges = 0;
    let mut edge_indexes = Vec::new();
    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if !is_community_report_edge(edge) {
            continue;
        }
        let source_inside = component.contains(&edge.source);
        let target_inside = component.contains(&edge.target);
        match (source_inside, target_inside) {
            (true, true) => {
                internal_edges += 1;
                if edge_indexes.len() < LINK_EVIDENCE_LIMIT {
                    edge_indexes.push(edge_index);
                }
            }
            (true, false) if node_community.contains_key(&edge.target) => {
                outgoing_external_edges += 1;
                if edge_indexes.len() < LINK_EVIDENCE_LIMIT {
                    edge_indexes.push(edge_index);
                }
            }
            (false, true) if node_community.contains_key(&edge.source) => {
                incoming_external_edges += 1;
                if edge_indexes.len() < LINK_EVIDENCE_LIMIT {
                    edge_indexes.push(edge_index);
                }
            }
            _ => {}
        }
    }

    let label = community_label(&area_counts, &languages, nodes.first());
    GraphCommunity {
        id: community_id,
        label: if label.is_empty() {
            "Community".to_string()
        } else {
            label
        },
        node_count: nodes.len(),
        files,
        entrypoints,
        internal_edges,
        incoming_external_edges,
        outgoing_external_edges,
        languages,
        node_kinds,
        sample_nodes: nodes.into_iter().take(8).collect(),
        edge_indexes,
    }
}

pub(crate) fn is_community_report_edge(edge: &Edge) -> bool {
    edge.kind == EdgeKind::Contains || is_architecture_dependency_edge(&edge.kind)
}

pub(crate) fn community_label(
    area_counts: &BTreeMap<String, usize>,
    languages: &BTreeMap<String, usize>,
    first_node: Option<&Node>,
) -> String {
    if let Some((area, _)) = area_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
    {
        return area.clone();
    }
    if let Some((language, _)) = languages
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
    {
        return format!("{language} symbols");
    }
    first_node
        .map(|node| node.label.clone())
        .unwrap_or_else(|| "Community".to_string())
}

pub(crate) fn node_rank(kind: &NodeKind) -> usize {
    match kind {
        NodeKind::Entrypoint => 0,
        NodeKind::File => 1,
        NodeKind::Module => 2,
        NodeKind::Type => 3,
        NodeKind::Function => 4,
        NodeKind::Config => 5,
        NodeKind::Environment => 6,
        NodeKind::ExternalDependency => 7,
        NodeKind::Directory => 8,
        NodeKind::Repository => 9,
        NodeKind::ControlFlow => 10,
        NodeKind::Unknown => 10,
    }
}

pub(crate) fn hotspot_stats<F>(
    graph: &CodeGraph,
    edge_filter: F,
    direction: NeighborDirection,
) -> Vec<Hotspot>
where
    F: Fn(&Edge) -> bool,
{
    let candidate_ids: BTreeSet<_> = graph
        .nodes
        .iter()
        .filter(|node| is_hotspot_candidate(&node.kind))
        .map(|node| node.id)
        .collect();
    let nodes_by_id: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    let mut stats: BTreeMap<NodeId, Hotspot> = BTreeMap::new();

    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.kind != EdgeKind::Contains && edge_filter(edge))
    {
        if direction != NeighborDirection::In
            && candidate_ids.contains(&edge.source)
            && let Some(node) = nodes_by_id.get(&edge.source)
        {
            let hotspot = stats.entry(edge.source).or_insert_with(|| Hotspot {
                node: (*node).clone(),
                score: 0,
                incoming: 0,
                outgoing: 0,
                edge_kinds: BTreeMap::new(),
                hub_kind: String::new(),
            });
            hotspot.outgoing += 1;
            hotspot.score += 1;
            *hotspot
                .edge_kinds
                .entry(edge_kind_name(&edge.kind))
                .or_insert(0) += 1;
        }
        if direction != NeighborDirection::Out
            && candidate_ids.contains(&edge.target)
            && let Some(node) = nodes_by_id.get(&edge.target)
        {
            let hotspot = stats.entry(edge.target).or_insert_with(|| Hotspot {
                node: (*node).clone(),
                score: 0,
                incoming: 0,
                outgoing: 0,
                edge_kinds: BTreeMap::new(),
                hub_kind: String::new(),
            });
            hotspot.incoming += 1;
            hotspot.score += 1;
            *hotspot
                .edge_kinds
                .entry(edge_kind_name(&edge.kind))
                .or_insert(0) += 1;
        }
    }

    let mut hotspots: Vec<_> = stats.into_values().collect();
    for hotspot in &mut hotspots {
        hotspot.hub_kind = hotspot_kind(hotspot).to_string();
    }
    hotspots.sort_by(|left, right| {
        hotspot_is_the_projects_own(right)
            .cmp(&hotspot_is_the_projects_own(left))
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| right.incoming.cmp(&left.incoming))
            .then_with(|| right.outgoing.cmp(&left.outgoing))
            .then_with(|| left.node.label.cmp(&right.node.label))
    });
    hotspots
}

/// Whether a hub sits in the program rather than in its tests, examples or
/// vendored code.
///
/// kong's biggest hub is `spec/helpers.lua`, which 372 spec files reference,
/// and redis's is jemalloc's generated `configure`. Both are true and
/// neither answers "what is central here": they filled seven of the ten
/// slots a reader looks at first, so they now come after the project's own
/// hubs rather than instead of them.
fn hotspot_is_the_projects_own(hotspot: &Hotspot) -> bool {
    let path = hotspot
        .node
        .span
        .as_ref()
        .map(|span| span.path.as_str())
        .unwrap_or(hotspot.node.label.as_str());
    !is_test_like_source_path(path) && !is_vendored_source_path(path)
}

pub(crate) fn hotspot_kind(hotspot: &Hotspot) -> &'static str {
    if is_utility_hotspot(hotspot) {
        "utility"
    } else {
        "architectural"
    }
}

pub(crate) fn is_utility_hotspot(hotspot: &Hotspot) -> bool {
    let label = hotspot.node.label.trim();
    let normalized = label.trim_matches('_').to_ascii_lowercase();
    if matches!(
        hotspot.node.kind,
        NodeKind::Config
            | NodeKind::Environment
            | NodeKind::File
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Module
    ) {
        return false;
    }
    if matches!(
        normalized.as_str(),
        "new"
            | "default"
            | "clone"
            | "copy"
            | "drop"
            | "fmt"
            | "debug"
            | "to_string"
            | "to_owned"
            | "into"
            | "from"
            | "as_ref"
            | "as_mut"
            | "unwrap"
            | "expect"
            | "ok"
            | "err"
            | "some"
            | "none"
            | "get"
            | "set"
            | "len"
            | "is_empty"
            | "map"
            | "and_then"
            | "or_else"
            | "parse"
            | "open"
            | "read"
            | "write"
    ) {
        return true;
    }
    if normalized.len() <= 2 && hotspot.score >= 4 {
        return true;
    }
    if normalized.starts_with("get_") || normalized.starts_with("set_") {
        return true;
    }
    hotspot.node.kind == NodeKind::ExternalDependency
        && hotspot
            .node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| kind == "call")
        && hotspot
            .node
            .metadata
            .get("resolution")
            .is_some_and(|resolution| resolution == "unresolved")
}

/// What the graph calls an entrypoint: whatever an `Entrypoint` edge
/// points at. A program the parser recognised is a Function node --
/// Rust's `main`, a C# file of top-level statements -- so asking for
/// `kind == Entrypoint` instead answers every question about where a
/// program starts without the programs.
pub(crate) fn entrypoint_node_ids(graph: &CodeGraph) -> BTreeSet<NodeId> {
    graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Entrypoint)
        .map(|edge| edge.target)
        .collect()
}

pub fn entrypoints(graph: &CodeGraph) -> Vec<Node> {
    let ids = entrypoint_node_ids(graph);

    let mut matched: Vec<Node> = graph
        .nodes
        .iter()
        .filter(|node| ids.contains(&node.id))
        .cloned()
        .collect();
    // Graph order is the file walk, so `.github/` sorted ahead of everything:
    // on redis the first six entrypoints were CI jobs with identical flows,
    // and on terraform they were CI shell scripts, not the program. Order by
    // what a reader opens first instead, keeping graph order inside a tier so
    // the result stays deterministic.
    matched.sort_by_key(|node| {
        (
            entrypoint_rank(node),
            // A program's own entry sits near the root; a helper `main` in
            // `.github/scripts/` or `internal/.../testdata/` does not.
            node.span
                .as_ref()
                .map_or(0, |span| span.path.matches('/').count()),
            node.id,
        )
    });
    matched
}

/// How likely an entrypoint is to be the one someone means: a declared program
/// first, then the routes a server exposes, then scripts and build targets,
/// then CI and container declarations — and tests last, whatever declares them.
pub(crate) fn entrypoint_rank(node: &Node) -> u8 {
    let kind = node
        .metadata
        .get("entrypoint_kind")
        .map(String::as_str)
        .unwrap_or_default();
    let source = node
        .metadata
        .get("source")
        .map(String::as_str)
        .unwrap_or_default();
    let test_like = kind == "test"
        || node
            .span
            .as_ref()
            .is_some_and(|span| is_test_like_source_path(&span.path))
        || node
            .metadata
            .get("target")
            .is_some_and(|target| is_test_like_source_path(target));
    if test_like {
        return 5;
    }
    // A script whose shebang says how to run it is a program too, unless
    // it sits where the repository keeps its build: koel's `artisan` at
    // the root is how the program runs, and ripgrep's `ci/test-complete`
    // is how it is tested.
    // The path is on the span when the scan placed the node, and in the
    // label -- `script:ci/test-complete` -- when it did not.
    let tooling = node
        .span
        .as_ref()
        .map(|span| span.path.as_str())
        .or_else(|| node.label.split_once(':').map(|(_, path)| path))
        .is_some_and(is_repository_tooling_source_path);
    match (source, kind) {
        // What a manifest publishes as a program: a binary, the module a
        // Go repository is, or a console script a package installs.
        (
            "manifest",
            "binary" | "executable" | "app" | "module" | "console_script" | "gui_script",
        ) => 0,
        ("shebang", _) if !tooling => 1,
        // A program the parser recognised: below an explicitly declared
        // binary, above the routes and scripts around it. Graphs scanned
        // before programs were labelled fall back to the node kind.
        ("code", "program") => 1,
        ("", _) if node.kind == NodeKind::Function => 1,
        ("framework", _) => 2,
        ("shebang", _) | ("manifest", _) | ("makefile", _) => 3,
        _ => 4,
    }
}
