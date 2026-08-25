//! Shared graph helpers: edge indexes, workflow filters, path/area
//! classification, and confidence naming.

pub(crate) use codegraph_core::is_test_like_source_path;
use codegraph_core::{CodeGraph, Edge, EdgeKind, Node, NodeId, NodeKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[allow(unused_imports)]
use crate::*;

/// Nodes by id, for loops that would otherwise scan every node per edge.
///
/// Ids are handed out densely, but a sliced graph keeps the original ids with
/// fewer nodes, so this indexes what is actually there rather than assuming
/// the id is the position.
/// Resolve the node a trace, flow, or journey starts from.
///
/// Labels repeat, and `main` is the worst case: it names 15 nodes on
/// terraform — two shell functions in CI scripts and thirteen Go programs, all
/// of them entrypoints. Taking the first match takes the alphabetically first
/// file, which is how `.github/scripts/equivalence-test.sh` came to start a Go
/// project's flow. When a label is ambiguous, rank the candidates by what a
/// reader means by it: an entrypoint over a plain node, production code over
/// tests and fixtures, and the shallowest path — a program's own `main.go`
/// sits at the repository root, its helpers do not. Ties keep graph order, so
/// the choice is deterministic, and the caller reports which node it used.
pub(crate) fn resolve_trace_start<'a>(
    graph: &'a CodeGraph,
    start: &TraceStart,
) -> Option<&'a Node> {
    match start {
        TraceStart::NodeId(id) => graph.nodes.iter().find(|node| node.id == *id),
        // A durable `cg-*` id names one node exactly; anything else is a
        // label, ranked as described above.
        TraceStart::Label(label) => graph
            .nodes
            .iter()
            .find(|node| {
                node.metadata
                    .get("stable_id")
                    .is_some_and(|stable_id| stable_id == label)
            })
            .or_else(|| best_labelled_node(graph, label)),
    }
}

/// What a report owes its reader when the name it was given belongs to
/// more than one definition: how many there were, and which one the
/// answer is about. Without it, "nothing depends on Blueprint" reads as a
/// fact about the name when it is only a fact about one of two
/// declarations.
pub(crate) fn shared_name_note(graph: &CodeGraph, label: &str, chosen: &Node) -> Option<String> {
    let matches = labelled_node_count(graph, label);
    if matches < 2 {
        return None;
    }
    let where_it_is = chosen
        .span
        .as_ref()
        .map(|span| format!("{}:{}", span.path, span.start_line))
        .unwrap_or_else(|| format!("node {}", chosen.id));
    Some(format!(
        "{matches} definitions are named `{label}`; this answer is about the one at {where_it_is}"
    ))
}

/// How many definitions answer to a label. A trace has one start, so when
/// this is more than one the answer is about a choice the query did not
/// state — flask has two `Blueprint`s, and "nothing depends on Blueprint"
/// means nothing depends on *the one that was picked*.
pub(crate) fn labelled_node_count(graph: &CodeGraph, label: &str) -> usize {
    graph
        .nodes
        .iter()
        .filter(|node| node.label == label && declares_its_name(node))
        .count()
}

/// The stand-in node an unresolved or ambiguous call points at. It carries
/// the call's label but defines nothing, so it must not be counted among
/// the definitions that answer to that name.
pub(crate) fn is_call_placeholder(node: &Node) -> bool {
    node.kind == NodeKind::ExternalDependency
        && node
            .metadata
            .get("item_kind")
            .is_some_and(|kind| kind == "call")
}

/// Whether this node declares the thing it is named after. An error
/// construct takes the name of the call it wraps, so this repository holds
/// 104 nodes labelled `scan_project` for `scan_project(..).unwrap()` and
/// one for the function; a reader who names it means the function.
pub(crate) fn declares_its_name(node: &Node) -> bool {
    !is_call_placeholder(node)
        && !matches!(
            node.kind,
            NodeKind::ControlFlow | NodeKind::Environment | NodeKind::Config
        )
}

/// The node a label most likely means, ranked as described on
/// [`resolve_trace_start`].
pub(crate) fn best_labelled_node<'a>(graph: &'a CodeGraph, label: &str) -> Option<&'a Node> {
    let mut matches = graph.nodes.iter().filter(|node| node.label == label);
    let first = matches.next()?;
    if matches.next().is_none() {
        return Some(first);
    }

    let entrypoint_ids: BTreeSet<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Entrypoint)
        .map(|edge| edge.target)
        .collect();
    // Declared programs: a node that a manifest-declared entrypoint
    // points at, such as the `main` behind `cargo bin:codegraph-cli`.
    // A build script's `main` has no declaration behind it, and a
    // shebang script is a weaker claim than a manifest binary — a
    // vendored `gen_travis.py` should not outrank a project's own
    // program.
    let entrypoint_nodes: BTreeSet<NodeId> = graph
        .nodes
        .iter()
        .filter(|node| {
            node.kind == NodeKind::Entrypoint
                && node
                    .metadata
                    .get("source")
                    .is_some_and(|source| source == "manifest")
        })
        .map(|node| node.id)
        .collect();
    let declared_ids: BTreeSet<NodeId> = graph
        .edges
        .iter()
        .filter(|edge| entrypoint_nodes.contains(&edge.source))
        .map(|edge| edge.target)
        .collect();
    graph
        .nodes
        .iter()
        .filter(|node| node.label == label)
        .min_by_key(|node| {
            let path = node.span.as_ref().map(|span| span.path.as_str());
            (
                u8::from(!declares_its_name(node)),
                u8::from(path.is_some_and(is_test_like_source_path)),
                u8::from(!declared_ids.contains(&node.id)),
                u8::from(!entrypoint_ids.contains(&node.id)),
                path.map_or(usize::MAX, |path| path.matches('/').count()),
                node.id,
            )
        })
        .or(Some(first))
}

pub(crate) fn node_index(graph: &CodeGraph) -> BTreeMap<NodeId, &Node> {
    graph.nodes.iter().map(|node| (node.id, node)).collect()
}

pub(crate) fn incoming_edge_indexes(
    graph: &CodeGraph,
    target: NodeId,
    kind: EdgeKind,
) -> Vec<usize> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            if edge.target == target && edge.kind == kind {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn outgoing_edge_indexes(
    graph: &CodeGraph,
    source: NodeId,
    kind: EdgeKind,
) -> Vec<usize> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            if edge.source == source && edge.kind == kind {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn is_cycle_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls | EdgeKind::References | EdgeKind::Imports | EdgeKind::DependsOn
    )
}

pub(crate) fn is_trace_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Calls
            | EdgeKind::References
            | EdgeKind::Imports
            | EdgeKind::ReadsConfig
            | EdgeKind::ReadsEnvironment
            | EdgeKind::MayError
            | EdgeKind::DependsOn
    )
}

pub(crate) fn normalize_workflow_filters(filters: WorkflowFilters) -> WorkflowFilters {
    WorkflowFilters {
        edge_kind: normalize_workflow_filter(filters.edge_kind),
        confidence: normalize_workflow_filter(filters.confidence),
        language: normalize_workflow_filter(filters.language),
        risk_severity: normalize_workflow_filter(filters.risk_severity),
        block_kind: normalize_workflow_filter(filters.block_kind),
    }
}

pub(crate) fn normalize_workflow_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn workflow_included_node_ids(
    start: &Node,
    blocks: &[WorkflowBlock],
    transitions: &[WorkflowTransition],
    filters: &WorkflowFilters,
) -> BTreeSet<NodeId> {
    let has_block_filters = filters.language.is_some()
        || filters.risk_severity.is_some()
        || filters.block_kind.is_some();
    if !has_block_filters {
        return blocks.iter().map(|block| block.node.id).collect();
    }

    let mut included = BTreeSet::from([start.id]);
    for block in blocks {
        if workflow_block_filter_matches(block, filters) {
            included.insert(block.node.id);
        }
    }
    if filters.risk_severity.is_some() {
        let block_by_node = blocks
            .iter()
            .map(|block| (block.node.id, block))
            .collect::<BTreeMap<_, _>>();
        for transition in transitions {
            if !workflow_transition_filter_matches(transition, filters) {
                continue;
            }
            let source_matches = block_by_node
                .get(&transition.source_node_id)
                .is_some_and(|block| workflow_block_non_risk_filters_match(block, filters));
            let target_matches = block_by_node
                .get(&transition.target_node_id)
                .is_some_and(|block| workflow_block_non_risk_filters_match(block, filters));
            if source_matches && target_matches {
                included.insert(transition.source_node_id);
                included.insert(transition.target_node_id);
            }
        }
    }
    included
}

pub(crate) fn workflow_edge_filter_matches(edge: &Edge, filters: &WorkflowFilters) -> bool {
    filters
        .edge_kind
        .as_deref()
        .is_none_or(|expected| text_matches(&edge_kind_name(&edge.kind), expected))
        && filters
            .confidence
            .as_deref()
            .is_none_or(|expected| text_matches(&confidence_name(edge.confidence), expected))
}

pub(crate) fn workflow_block_filter_matches(
    block: &WorkflowBlock,
    filters: &WorkflowFilters,
) -> bool {
    workflow_block_non_risk_filters_match(block, filters)
        && filters.risk_severity.as_deref().is_none_or(|expected| {
            block
                .risk_refs
                .iter()
                .any(|risk| text_matches(severity_name(risk.severity), expected))
        })
}

pub(crate) fn workflow_block_non_risk_filters_match(
    block: &WorkflowBlock,
    filters: &WorkflowFilters,
) -> bool {
    filters
        .language
        .as_deref()
        .is_none_or(|expected| workflow_node_language_matches(&block.node, expected))
        && filters.block_kind.as_deref().is_none_or(|expected| {
            text_matches(&workflow_block_kind_filter_name(&block.kind), expected)
                || text_matches(workflow_block_kind_label(&block.kind), expected)
        })
}

pub(crate) fn workflow_node_language_matches(node: &Node, expected: &str) -> bool {
    node.metadata
        .get("language")
        .is_some_and(|language| text_matches(language, expected))
}

pub(crate) fn workflow_transition_filter_matches(
    transition: &WorkflowTransition,
    filters: &WorkflowFilters,
) -> bool {
    workflow_edge_filter_matches(&transition.edge, filters)
        && filters.risk_severity.as_deref().is_none_or(|expected| {
            transition
                .risk_refs
                .iter()
                .any(|risk| text_matches(severity_name(risk.severity), expected))
        })
}

pub(crate) fn workflow_block_id(id: NodeId) -> String {
    format!("wb-{}", id.0)
}

pub(crate) fn workflow_block_kind(
    node: &Node,
    incoming_edge: Option<&Edge>,
    is_start: bool,
) -> WorkflowBlockKind {
    if is_start {
        return WorkflowBlockKind::Start;
    }
    if node.kind == NodeKind::ExternalDependency {
        return WorkflowBlockKind::ExternalBoundary;
    }
    match node.metadata.get("item_kind").map(String::as_str) {
        Some("branch") => return WorkflowBlockKind::Branch,
        Some("loop") => return WorkflowBlockKind::Loop,
        Some("async") => return WorkflowBlockKind::Async,
        Some("return") => return WorkflowBlockKind::Return,
        _ => {}
    }
    match incoming_edge.map(|edge| &edge.kind) {
        Some(EdgeKind::Calls) => WorkflowBlockKind::Call,
        Some(EdgeKind::ReadsConfig) => WorkflowBlockKind::ConfigRead,
        Some(EdgeKind::ReadsEnvironment) => WorkflowBlockKind::EnvironmentRead,
        Some(EdgeKind::MayError) => WorkflowBlockKind::Error,
        Some(EdgeKind::DependsOn) => WorkflowBlockKind::Dependency,
        Some(EdgeKind::Imports) => WorkflowBlockKind::Import,
        Some(EdgeKind::References) => WorkflowBlockKind::Reference,
        _ => WorkflowBlockKind::Unknown,
    }
}

pub(crate) fn workflow_block_kind_label(kind: &WorkflowBlockKind) -> &'static str {
    match kind {
        WorkflowBlockKind::Start => "start",
        WorkflowBlockKind::Call => "call",
        WorkflowBlockKind::ConfigRead => "config",
        WorkflowBlockKind::EnvironmentRead => "env",
        WorkflowBlockKind::Dependency => "dependency",
        WorkflowBlockKind::Import => "import",
        WorkflowBlockKind::Branch => "branch",
        WorkflowBlockKind::Loop => "loop",
        WorkflowBlockKind::Async => "async",
        WorkflowBlockKind::Return => "return",
        WorkflowBlockKind::Error => "error",
        WorkflowBlockKind::Reference => "reference",
        WorkflowBlockKind::ExternalBoundary => "external",
        WorkflowBlockKind::Unknown => "node",
    }
}

pub(crate) fn workflow_block_kind_filter_name(kind: &WorkflowBlockKind) -> String {
    serde_json_name(kind).unwrap_or_else(|| workflow_block_kind_label(kind).to_string())
}

pub(crate) fn workflow_risk_refs_for_node(
    report: &InsightReport,
    node_id: NodeId,
) -> Vec<WorkflowRiskRef> {
    report
        .insights
        .iter()
        .enumerate()
        .filter(|(_, insight)| insight.nodes.contains(&node_id))
        .take(8)
        .map(|(insight_index, insight)| workflow_risk_ref(insight_index, insight))
        .collect()
}

pub(crate) fn workflow_risk_refs_for_edge(
    report: &InsightReport,
    edge_index: usize,
) -> Vec<WorkflowRiskRef> {
    report
        .insights
        .iter()
        .enumerate()
        .filter(|(_, insight)| insight.edges.contains(&edge_index))
        .take(8)
        .map(|(insight_index, insight)| workflow_risk_ref(insight_index, insight))
        .collect()
}

pub(crate) fn workflow_risk_ref(insight_index: usize, insight: &Insight) -> WorkflowRiskRef {
    WorkflowRiskRef {
        insight_index,
        kind: insight.kind.clone(),
        severity: insight.severity,
        message: insight.message.clone(),
        edge_indexes: insight.edges.clone(),
    }
}

pub(crate) fn mermaid_report_block_id(id: &str) -> String {
    let mut normalized = String::from("B");
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
        } else {
            normalized.push('_');
        }
    }
    normalized
}

pub(crate) fn mermaid_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '|'], " ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraceDirection {
    Outgoing,
    Incoming,
}

/// Per-request adjacency over trace edges (plus `Contains` for container
/// expansion), built in one pass so BFS traversals cost O(V + E) instead of a
/// full edge scan per visited node. Edge indexes are stored in edge order, so
/// iteration order is identical to the old linear filter.
pub(crate) struct TraceAdjacency {
    outgoing: BTreeMap<NodeId, Vec<usize>>,
    incoming: BTreeMap<NodeId, Vec<usize>>,
    contains_outgoing: BTreeMap<NodeId, Vec<usize>>,
}

impl TraceAdjacency {
    pub(crate) fn build(graph: &CodeGraph) -> Self {
        let mut outgoing: BTreeMap<NodeId, Vec<usize>> = BTreeMap::new();
        let mut incoming: BTreeMap<NodeId, Vec<usize>> = BTreeMap::new();
        let mut contains_outgoing: BTreeMap<NodeId, Vec<usize>> = BTreeMap::new();
        for (index, edge) in graph.edges.iter().enumerate() {
            if is_trace_edge(&edge.kind) {
                outgoing.entry(edge.source).or_default().push(index);
                incoming.entry(edge.target).or_default().push(index);
            } else if edge.kind == EdgeKind::Contains {
                contains_outgoing
                    .entry(edge.source)
                    .or_default()
                    .push(index);
            }
        }
        Self {
            outgoing,
            incoming,
            contains_outgoing,
        }
    }

    pub(crate) fn trace_edges<'graph>(
        &self,
        graph: &'graph CodeGraph,
        node_id: NodeId,
        direction: TraceDirection,
    ) -> impl Iterator<Item = (usize, &'graph Edge)> {
        let indexes = match direction {
            TraceDirection::Outgoing => self.outgoing.get(&node_id),
            TraceDirection::Incoming => self.incoming.get(&node_id),
        };
        indexes
            .into_iter()
            .flatten()
            .filter_map(move |index| graph.edges.get(*index).map(|edge| (*index, edge)))
    }

    pub(crate) fn contains_edges<'graph>(
        &self,
        graph: &'graph CodeGraph,
        node_id: NodeId,
    ) -> impl Iterator<Item = (usize, &'graph Edge)> {
        self.contains_outgoing
            .get(&node_id)
            .into_iter()
            .flatten()
            .filter_map(move |index| graph.edges.get(*index).map(|edge| (*index, edge)))
    }
}

pub(crate) fn trace_next_node(edge: &Edge, node_id: NodeId, direction: TraceDirection) -> NodeId {
    match direction {
        TraceDirection::Outgoing => edge.target,
        TraceDirection::Incoming => {
            debug_assert_eq!(edge.target, node_id);
            edge.source
        }
    }
}

pub(crate) fn entrypoint_reachable_nodes(graph: &CodeGraph) -> BTreeSet<NodeId> {
    entrypoint_reachable_nodes_from(graph, &BTreeSet::new())
}

/// The same walk, seeded with extra roots — a library's exported functions,
/// say, which nothing in the repository has to call for them to run.
pub(crate) fn entrypoint_reachable_nodes_from(
    graph: &CodeGraph,
    extra_roots: &BTreeSet<NodeId>,
) -> BTreeSet<NodeId> {
    let mut reachable: BTreeSet<NodeId> = extra_roots.clone();
    let mut queue: VecDeque<NodeId> = extra_roots.iter().copied().collect();

    // Most entrypoints name a file (`script:scripts/build.sh`), not a
    // function, and a file holds its symbols through `contains`. Without that
    // step the walk stops at the file and calls everything inside it
    // unreachable — on terraform only 5% of functions came out reachable, on a
    // TypeScript repository 1%. `contains` is followed only out of a file, so
    // the repository root and its directories still do not reach the whole
    // project by containment alone.
    let file_nodes: BTreeSet<NodeId> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::File)
        .map(|node| node.id)
        .collect();

    // One adjacency pass keeps the BFS O(V + E) instead of rescanning every
    // edge per visited node (audit F11).
    let mut outgoing: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for edge in &graph.edges {
        if edge.kind == EdgeKind::Entrypoint && reachable.insert(edge.target) {
            queue.push_back(edge.target);
        }
        if is_trace_edge(&edge.kind)
            || (edge.kind == EdgeKind::Contains && file_nodes.contains(&edge.source))
        {
            outgoing.entry(edge.source).or_default().push(edge.target);
        }
    }

    while let Some(node) = queue.pop_front() {
        for target in outgoing.get(&node).map(Vec::as_slice).unwrap_or(&[]) {
            if reachable.insert(*target) {
                queue.push_back(*target);
            }
        }
    }

    reachable
}

pub(crate) fn is_source_file_candidate(graph: &CodeGraph, node: &Node) -> bool {
    node.kind == NodeKind::File
        && node.metadata.contains_key("language")
        // A document is not code, and its headings are not symbols the
        // program runs: every project in the corpus was told that its
        // `README.md` "contains markdown code but is not reachable from
        // any entrypoint", which nothing can act on.
        && node
            .metadata
            .get("item_kind")
            .is_none_or(|kind| kind != "document")
        && !node.metadata.contains_key("skipped_reason")
        && !is_test_like_source_path(&node.label)
        && graph.edges.iter().any(|edge| {
            edge.source == node.id
                && edge.kind == EdgeKind::Contains
                && graph
                    .nodes
                    .iter()
                    .any(|child| child.id == edge.target && is_code_symbol(&child.kind))
        })
}

pub(crate) fn file_has_reachable_code(
    graph: &CodeGraph,
    file_id: NodeId,
    reachable: &BTreeSet<NodeId>,
) -> bool {
    graph.edges.iter().any(|edge| {
        edge.source == file_id
            && edge.kind == EdgeKind::Contains
            && reachable.contains(&edge.target)
    })
}

pub(crate) fn contained_code_edge_indexes(graph: &CodeGraph, file_id: NodeId) -> Vec<usize> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            (edge.source == file_id && edge.kind == EdgeKind::Contains).then_some(index)
        })
        .collect()
}

pub(crate) fn is_code_symbol(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Function | NodeKind::Type | NodeKind::Module | NodeKind::Entrypoint
    )
}

pub(crate) fn is_dependency_manifest_source_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    matches!(
        normalized.rsplit('/').next().unwrap_or(normalized.as_str()),
        "setup.py"
    )
}

pub(crate) fn node_label(graph: &CodeGraph, id: NodeId) -> Option<&str> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| node.label.as_str())
}

pub(crate) fn kind_name(kind: &codegraph_core::NodeKind) -> String {
    node_kind_name(kind).to_string()
}

/// Direct snake_case name for a node kind. Mirrors the `#[serde(rename_all =
/// "snake_case")]` names on `NodeKind` without a per-call serde round-trip,
/// which dominates report generation when called once per graph edge.
pub(crate) fn node_kind_name(kind: &codegraph_core::NodeKind) -> &'static str {
    use codegraph_core::NodeKind::*;
    match kind {
        Repository => "repository",
        Directory => "directory",
        File => "file",
        Module => "module",
        Function => "function",
        Entrypoint => "entrypoint",
        Type => "type",
        Config => "config",
        Environment => "environment",
        ExternalDependency => "external_dependency",
        ControlFlow => "control_flow",
        Unknown => "unknown",
    }
}

/// How a project divides into areas. The top-level directory is the usual
/// answer, but plenty of repositories put everything under one container —
/// terraform's `internal/` holds 4677 of its files and Vue's `packages/`
/// all twelve packages — and calling that one area describes nothing. A
/// directory that only groups other directories is not an area; what it
/// groups is.
pub(crate) struct ProjectAreas {
    /// Top-level directories whose children are the real areas.
    split: BTreeSet<String>,
}

impl ProjectAreas {
    pub(crate) fn from_graph(graph: &CodeGraph) -> Self {
        let paths: Vec<String> = graph
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::File)
            .map(|node| node.label.replace('\\', "/"))
            .collect();
        let total = paths.len();
        let mut beneath: BTreeMap<&str, usize> = BTreeMap::new();
        let mut direct: BTreeMap<&str, usize> = BTreeMap::new();
        let mut children: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for path in &paths {
            let mut parts = path.split('/');
            let Some(top) = parts.next() else {
                continue;
            };
            let rest: Vec<&str> = parts.collect();
            if rest.is_empty() {
                continue;
            }
            *beneath.entry(top).or_default() += 1;
            if rest.len() == 1 {
                *direct.entry(top).or_default() += 1;
            } else {
                children.entry(top).or_default().insert(rest[0]);
            }
        }

        let split = beneath
            .iter()
            .filter(|(top, count)| {
                // Several children to divide into, almost nothing of its
                // own, and большая часть проекта underneath: that is a
                // container rather than an area.
                children.get(*top).is_some_and(|kids| kids.len() >= 2)
                    && direct.get(*top).copied().unwrap_or(0) * 10 <= **count
                    && **count * 2 > total
            })
            .map(|(top, _)| (*top).to_string())
            .collect();
        Self { split }
    }

    /// The area a repository-relative path belongs to, as an id and a label.
    pub(crate) fn group_for_path(&self, path: &str) -> (String, String) {
        let normalized = path.trim_matches('/').replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();
        if parts.len() > 2 && self.split.contains(parts[0]) {
            let area = format!("{}/{}", parts[0], parts[1]);
            return (area.clone(), area);
        }
        architecture_group_for_path(path)
    }

    pub(crate) fn community_id_for_path(&self, path: &str) -> String {
        format!("area:{}", self.group_for_path(path).0)
    }
}

pub(crate) fn architecture_group_for_path(path: &str) -> (String, String) {
    let normalized = path.trim_matches('/').replace('\\', "/");
    let Some((first, _)) = normalized.split_once('/') else {
        return (".".to_string(), "root".to_string());
    };
    let first = first.trim();
    if first.is_empty() {
        (".".to_string(), "root".to_string())
    } else {
        (first.to_string(), first.to_string())
    }
}

pub(crate) fn node_architecture_areas(
    graph: &CodeGraph,
    nodes_by_id: &BTreeMap<NodeId, &Node>,
) -> BTreeMap<NodeId, String> {
    let project = ProjectAreas::from_graph(graph);
    let mut areas = BTreeMap::new();
    for node in nodes_by_id.values() {
        if node.kind == NodeKind::File {
            let (area, _) = project.group_for_path(&node.label);
            areas.insert(node.id, area);
        }
    }
    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Contains)
    {
        let Some(area) = areas.get(&edge.source).cloned() else {
            continue;
        };
        areas.entry(edge.target).or_insert(area);
    }
    areas
}

pub(crate) fn is_architecture_symbol(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Module
            | NodeKind::Function
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Config
            | NodeKind::Environment
    )
}

pub(crate) fn is_architecture_dependency_edge(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Imports
            | EdgeKind::Calls
            | EdgeKind::References
            | EdgeKind::ReadsConfig
            | EdgeKind::ReadsEnvironment
            | EdgeKind::MayError
            | EdgeKind::Entrypoint
            | EdgeKind::DependsOn
    )
}

pub(crate) fn is_hotspot_candidate(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::File
            | NodeKind::Module
            | NodeKind::Function
            | NodeKind::Entrypoint
            | NodeKind::Type
            | NodeKind::Config
            | NodeKind::Environment
    )
}

pub(crate) fn node_language(nodes_by_id: &BTreeMap<NodeId, &Node>, id: NodeId) -> String {
    nodes_by_id
        .get(&id)
        .and_then(|node| node.metadata.get("language"))
        .map(|language| language.trim())
        .filter(|language| !language.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

pub(crate) fn edge_kind_name(kind: &EdgeKind) -> String {
    // Fully-qualified variants (no `use EdgeKind::*`) so the import detector does
    // not read the bare glob as an undeclared external crate named `edgekind`.
    match kind {
        EdgeKind::Contains => "contains",
        EdgeKind::Imports => "imports",
        EdgeKind::Calls => "calls",
        EdgeKind::Defines => "defines",
        EdgeKind::References => "references",
        EdgeKind::ReadsConfig => "reads_config",
        EdgeKind::ReadsEnvironment => "reads_environment",
        EdgeKind::MayError => "may_error",
        EdgeKind::Entrypoint => "entrypoint",
        EdgeKind::DependsOn => "depends_on",
    }
    .to_string()
}

pub(crate) fn confidence_name(confidence: codegraph_core::Confidence) -> String {
    use codegraph_core::Confidence::*;
    match confidence {
        Exact => "exact",
        Semantic => "semantic",
        Syntactic => "syntactic",
        Heuristic => "heuristic",
        Unknown => "unknown",
    }
    .to_string()
}

pub(crate) fn severity_name(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Info => "info",
        InsightSeverity::Warning => "warning",
        InsightSeverity::Error => "error",
    }
}

pub(crate) fn dot_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub(crate) fn dot_color(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Repository => "#5cc8a7",
        NodeKind::Directory => "#7f9cff",
        NodeKind::File => "#67b7dc",
        NodeKind::Module => "#8ccf7e",
        NodeKind::Function => "#f2c14e",
        NodeKind::Entrypoint => "#5cc8a7",
        NodeKind::Type => "#df7e7e",
        NodeKind::Config => "#e5b454",
        NodeKind::Environment => "#d8a657",
        NodeKind::ExternalDependency => "#b88ee6",
        NodeKind::ControlFlow => "#9aa7d8",
        NodeKind::Unknown => "#a5adb3",
    }
}

pub(crate) fn serde_json_name<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
}
