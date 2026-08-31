//! What the graph says about itself: a read-only integrity check over the
//! nodes and edges a scan produced, separate from `insights`, which reports
//! on the code. A graph can be wrong in ways no finding about the code would
//! show -- an edge to a node that is not there, one fact recorded twice, a
//! definition that appears to call itself.

use crate::support::edge_kind_name;
use codegraph_core::{CodeGraph, EdgeKind, NodeId};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const GRAPH_HEALTH_SCHEMA: &str = "codegraph.graph_health.v1";

/// How many samples of each kind a report carries. Enough to look at, not
/// enough to bury the counts they illustrate.
const SAMPLE_LIMIT: usize = 5;

#[derive(Debug, Clone, Serialize)]
pub struct GraphHealthSample {
    pub what: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphHealth {
    pub schema: String,
    pub nodes: usize,
    pub edges: usize,
    /// An edge naming a node this graph does not hold. Nothing can follow it,
    /// and every walk that meets it stops.
    pub dangling_edges: usize,
    /// The same fact recorded twice: identical endpoints, kind and metadata.
    /// Two reads of `PATH` on different lines are two facts, not one twice.
    pub duplicate_edges: usize,
    /// A definition that calls itself by its bare name. This is recursion,
    /// and it is reported because a cycle finding will name it, not because
    /// it is wrong.
    pub recursive_calls: usize,
    /// A definition that appears to call itself through some other object:
    /// axios's `request` "calls itself" through `session.request`, which is
    /// node's. The receiver names something the syntax could not type, and
    /// the call landed on the same name for want of a better answer.
    pub self_calls_through_a_receiver: usize,
    /// The sub-case that is always wrong: `super.x` written inside `x` means
    /// the parent's implementation, never this one.
    pub super_calls_to_self: usize,
    pub samples: Vec<GraphHealthSample>,
    /// Whether anything here is a defect rather than a fact about the code.
    /// Recursion is not; a dangling edge is.
    pub healthy: bool,
}

/// Read the graph's own integrity. Never changes it, never fails: a broken
/// graph is still worth reporting on, and the count is the report.
pub fn graph_health(graph: &CodeGraph) -> GraphHealth {
    let ids: BTreeSet<NodeId> = graph.nodes.iter().map(|node| node.id).collect();
    let labels: BTreeMap<NodeId, &str> = graph
        .nodes
        .iter()
        .map(|node| (node.id, node.label.as_str()))
        .collect();

    let mut dangling_edges = 0usize;
    let mut recursive_calls = 0usize;
    let mut self_calls_through_a_receiver = 0usize;
    let mut super_calls_to_self = 0usize;
    let mut samples: Vec<GraphHealthSample> = Vec::new();
    let mut seen: BTreeSet<(NodeId, NodeId, String, String)> = BTreeSet::new();
    let mut duplicate_edges = 0usize;

    let mut push_sample =
        |what: &str, label: String, file: Option<String>, detail: Option<String>| {
            if samples.iter().filter(|sample| sample.what == what).count() < SAMPLE_LIMIT {
                samples.push(GraphHealthSample {
                    what: what.to_string(),
                    label,
                    file,
                    detail,
                });
            }
        };

    for edge in &graph.edges {
        if !ids.contains(&edge.source) || !ids.contains(&edge.target) {
            dangling_edges += 1;
            push_sample(
                "dangling_edge",
                labels
                    .get(&edge.source)
                    .copied()
                    .unwrap_or("<missing>")
                    .to_string(),
                edge.metadata.get("file").cloned(),
                Some(format!(
                    "{:?} edge to a node the graph does not hold",
                    edge.kind
                )),
            );
            continue;
        }
        let fingerprint = (
            edge.source,
            edge.target,
            edge_kind_name(&edge.kind),
            serde_json::to_string(&edge.metadata).unwrap_or_default(),
        );
        if !seen.insert(fingerprint) {
            duplicate_edges += 1;
            push_sample(
                "duplicate_edge",
                labels.get(&edge.source).copied().unwrap_or("?").to_string(),
                edge.metadata.get("file").cloned(),
                Some(format!(
                    "{:?} recorded twice with the same facts",
                    edge.kind
                )),
            );
        }
        if edge.source != edge.target || edge.kind != EdgeKind::Calls {
            continue;
        }
        let label = edge
            .metadata
            .get("call_label")
            .map(String::as_str)
            .unwrap_or("");
        let Some(receiver) = call_receiver(label) else {
            recursive_calls += 1;
            continue;
        };
        if matches!(receiver, "this" | "self" | "Self") {
            recursive_calls += 1;
            continue;
        }
        self_calls_through_a_receiver += 1;
        if receiver == "super" {
            super_calls_to_self += 1;
            push_sample(
                "super_call_to_self",
                label.to_string(),
                edge.metadata.get("file").cloned(),
                Some(format!(
                    "`{label}` inside `{}` means the parent's, not this one",
                    labels.get(&edge.source).copied().unwrap_or("?")
                )),
            );
        } else {
            push_sample(
                "self_call_through_a_receiver",
                label.to_string(),
                edge.metadata.get("file").cloned(),
                Some(format!("`{receiver}` is not this definition's own object",)),
            );
        }
    }

    samples.sort_by(|left, right| {
        left.what
            .cmp(&right.what)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.file.cmp(&right.file))
    });

    GraphHealth {
        schema: GRAPH_HEALTH_SCHEMA.to_string(),
        nodes: graph.nodes.len(),
        edges: graph.edges.len(),
        dangling_edges,
        duplicate_edges,
        recursive_calls,
        self_calls_through_a_receiver,
        super_calls_to_self,
        samples,
        healthy: dangling_edges == 0 && duplicate_edges == 0 && super_calls_to_self == 0,
    }
}

/// The object a call is written through, when it is written through one.
/// `a.b.c` is called on `a.b`; a bare `c` is called on nothing.
fn call_receiver(label: &str) -> Option<&str> {
    let cut = label.rfind('.').or_else(|| label.rfind(':'))?;
    let receiver = label[..cut].trim_end_matches(':');
    (!receiver.is_empty()).then_some(receiver)
}
