//! Graph merge: combine several graph artifacts into one, conflict-safely.
//!
//! `codegraph merge` combines exported graph JSON files and/or registered
//! projects (project graphs, docs graphs, incident graphs, external-system
//! graphs) into one typed graph. Nodes merge only when kind, label, and
//! source path all match; every merged node and edge records which inputs
//! contributed it in `merge_sources` metadata. The strategy is conflict-safe
//! for committed artifacts: inputs are processed in sorted-by-name order, ids
//! are reassigned deterministically, and the same inputs always produce
//! byte-identical output, so re-merging in CI or by teammates never churns a
//! committed file. Metadata disagreements keep the first source's value and
//! are enumerated in the merge report instead of being dropped silently;
//! duplicate edges collapse to the highest-confidence contributor.

use anyhow::{Context, Result, bail};
use codegraph_core::{CodeGraph, Confidence, Edge, Node, NodeId};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

pub const MERGE_SCHEMA: &str = "codegraph.merge.v1";
pub const MERGE_SOURCES_KEY: &str = "merge_sources";
const CONFLICT_LIMIT: usize = 200;

#[derive(Debug)]
pub struct MergeInput {
    /// Short source name recorded in provenance metadata.
    pub name: String,
    /// Where the graph came from (file path or registry project).
    pub origin: String,
    pub graph: CodeGraph,
}

#[derive(Debug, Serialize)]
pub struct MergeInputReport {
    pub name: String,
    pub origin: String,
    pub nodes: usize,
    pub edges: usize,
}

#[derive(Debug, Serialize)]
pub struct MergeConflict {
    pub node: String,
    pub field: String,
    pub kept_source: String,
    pub dropped_source: String,
}

#[derive(Debug, Serialize)]
pub struct MergeReport {
    pub schema: String,
    pub label: String,
    pub inputs: Vec<MergeInputReport>,
    pub total_nodes: usize,
    pub total_edges: usize,
    /// Nodes with two or more contributing inputs.
    pub merged_nodes: usize,
    /// Duplicate edges collapsed into one.
    pub deduped_edges: usize,
    /// Edge confidence upgrades taken from a better contributor.
    pub confidence_upgrades: usize,
    pub metadata_conflicts: Vec<MergeConflict>,
    pub metadata_conflicts_truncated: bool,
}

fn confidence_rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::Exact => 0,
        Confidence::Semantic => 1,
        Confidence::Syntactic => 2,
        Confidence::Heuristic => 3,
        Confidence::Unknown => 4,
    }
}

fn node_key(node: &Node) -> String {
    let kind = serde_json::to_string(&node.kind).unwrap_or_default();
    let path = node
        .span
        .as_ref()
        .map(|span| span.path.as_str())
        .or_else(|| node.metadata.get("path").map(String::as_str))
        .unwrap_or("");
    format!("{kind}\u{1}{}\u{1}{path}", node.label)
}

fn append_source(metadata: &mut BTreeMap<String, String>, source: &str) {
    let entry = metadata.entry(MERGE_SOURCES_KEY.to_string()).or_default();
    if entry.is_empty() {
        *entry = source.to_string();
    } else if !entry.split(',').any(|existing| existing == source) {
        entry.push(',');
        entry.push_str(source);
    }
}

/// Merge inputs into one graph. Deterministic: inputs are sorted by name,
/// node ids are reassigned in processing order, and reruns over the same
/// inputs produce identical output.
pub fn merge_graphs(mut inputs: Vec<MergeInput>, label: &str) -> Result<(CodeGraph, MergeReport)> {
    if inputs.len() < 2 {
        bail!("merge needs at least two input graphs");
    }
    inputs.sort_by(|a, b| a.name.cmp(&b.name));
    for window in inputs.windows(2) {
        if window[0].name == window[1].name {
            bail!(
                "duplicate input name `{}`; give each input a distinct name",
                window[0].name
            );
        }
    }

    let mut merged = CodeGraph::new(label);
    let mut key_to_id: BTreeMap<String, NodeId> = BTreeMap::new();
    let mut contributor_counts: BTreeMap<NodeId, usize> = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut truncated = false;
    let mut input_reports = Vec::new();

    // First pass: nodes, with per-input id remapping.
    let mut remaps: Vec<BTreeMap<NodeId, NodeId>> = Vec::new();
    for input in &inputs {
        let mut remap = BTreeMap::new();
        for node in &input.graph.nodes {
            let key = node_key(node);
            if let Some(&existing_id) = key_to_id.get(&key) {
                remap.insert(node.id, existing_id);
                *contributor_counts.entry(existing_id).or_insert(1) += 1;
                let existing = merged
                    .nodes
                    .iter_mut()
                    .find(|candidate| candidate.id == existing_id)
                    .expect("merged node exists");
                for (field, value) in &node.metadata {
                    match existing.metadata.get(field) {
                        None => {
                            existing.metadata.insert(field.clone(), value.clone());
                        }
                        Some(kept) if kept != value => {
                            if conflicts.len() < CONFLICT_LIMIT {
                                conflicts.push(MergeConflict {
                                    node: node.label.clone(),
                                    field: field.clone(),
                                    kept_source: existing
                                        .metadata
                                        .get(MERGE_SOURCES_KEY)
                                        .cloned()
                                        .unwrap_or_default(),
                                    dropped_source: input.name.clone(),
                                });
                            } else {
                                truncated = true;
                            }
                        }
                        Some(_) => {}
                    }
                }
                append_source(&mut existing.metadata, &input.name);
            } else {
                let mut metadata = node.metadata.clone();
                append_source(&mut metadata, &input.name);
                let id = merged.add_node_with_metadata(
                    node.kind.clone(),
                    node.label.clone(),
                    node.span.clone(),
                    metadata,
                );
                key_to_id.insert(key, id);
                remap.insert(node.id, id);
            }
        }
        // Keep every input reachable from the merged root.
        if let Some(&mapped_root) = remap.get(&input.graph.root) {
            merged.add_edge(
                merged.root,
                mapped_root,
                codegraph_core::EdgeKind::Contains,
                Confidence::Exact,
            );
        }
        input_reports.push(MergeInputReport {
            name: input.name.clone(),
            origin: input.origin.clone(),
            nodes: input.graph.nodes.len(),
            edges: input.graph.edges.len(),
        });
        remaps.push(remap);
    }

    // Second pass: edges, deduped by (source, target, kind).
    let mut edge_slots: BTreeMap<(u64, u64, String), usize> = BTreeMap::new();
    let mut deduped_edges = 0usize;
    let mut confidence_upgrades = 0usize;
    for (input, remap) in inputs.iter().zip(&remaps) {
        for edge in &input.graph.edges {
            let (Some(&source), Some(&target)) = (remap.get(&edge.source), remap.get(&edge.target))
            else {
                continue;
            };
            let kind = serde_json::to_string(&edge.kind).unwrap_or_default();
            let slot_key = (source.0, target.0, kind);
            match edge_slots.get(&slot_key) {
                Some(&index) => {
                    deduped_edges += 1;
                    let existing = &mut merged.edges[index];
                    if confidence_rank(edge.confidence) < confidence_rank(existing.confidence) {
                        existing.confidence = edge.confidence;
                        confidence_upgrades += 1;
                    }
                    for (field, value) in &edge.metadata {
                        existing
                            .metadata
                            .entry(field.clone())
                            .or_insert_with(|| value.clone());
                    }
                    append_source(&mut existing.metadata, &input.name);
                }
                None => {
                    let mut metadata = edge.metadata.clone();
                    append_source(&mut metadata, &input.name);
                    merged.edges.push(Edge {
                        source,
                        target,
                        kind: edge.kind,
                        confidence: edge.confidence,
                        metadata,
                    });
                    edge_slots.insert(slot_key, merged.edges.len() - 1);
                }
            }
        }
    }

    let merged_nodes = contributor_counts.len();
    let report = MergeReport {
        schema: MERGE_SCHEMA.to_string(),
        label: label.to_string(),
        inputs: input_reports,
        total_nodes: merged.nodes.len(),
        total_edges: merged.edges.len(),
        merged_nodes,
        deduped_edges,
        confidence_upgrades,
        metadata_conflicts: conflicts,
        metadata_conflicts_truncated: truncated,
    };
    Ok((merged, report))
}

/// Load a graph artifact from an exported JSON file.
pub fn load_graph_file(path: &Path) -> Result<CodeGraph> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let graph: CodeGraph = serde_json::from_str(&raw)
        .with_context(|| format!("{} is not a valid graph JSON artifact", path.display()))?;
    Ok(graph)
}

/// Derive a unique short name from a file path, deduping with numeric suffixes.
pub fn unique_input_name(path: &Path, taken: &mut Vec<String>) -> String {
    let base = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "graph".to_string());
    let mut candidate = base.clone();
    let mut counter = 2;
    while taken.contains(&candidate) {
        candidate = format!("{base}-{counter}");
        counter += 1;
    }
    taken.push(candidate.clone());
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::{EdgeKind, NodeKind, SourceSpan};

    fn span(path: &str) -> SourceSpan {
        SourceSpan {
            path: path.to_string(),
            start_line: 1,
            start_column: 0,
            end_line: 1,
            end_column: 0,
        }
    }

    fn code_graph() -> CodeGraph {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            Some(span("src/main.rs")),
            BTreeMap::from([("path".to_string(), "src/main.rs".to_string())]),
        );
        let main = graph.add_node_with_span(NodeKind::Function, "main", span("src/main.rs"));
        graph.add_edge(graph.root, file, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(file, main, EdgeKind::Contains, Confidence::Exact);
        graph
    }

    fn docs_graph() -> CodeGraph {
        let mut graph = CodeGraph::new("repo");
        let file = graph.add_node_with_metadata(
            NodeKind::File,
            "src/main.rs",
            Some(span("src/main.rs")),
            BTreeMap::from([
                ("path".to_string(), "src/main.rs".to_string()),
                ("owner".to_string(), "docs-team".to_string()),
            ]),
        );
        let doc = graph.add_node_with_metadata(
            NodeKind::File,
            "docs/adr-1.md",
            Some(span("docs/adr-1.md")),
            BTreeMap::new(),
        );
        graph.add_edge(graph.root, file, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(graph.root, doc, EdgeKind::Contains, Confidence::Exact);
        graph.add_edge(doc, file, EdgeKind::References, Confidence::Heuristic);
        graph
    }

    fn inputs() -> Vec<MergeInput> {
        vec![
            MergeInput {
                name: "code".to_string(),
                origin: "code.json".to_string(),
                graph: code_graph(),
            },
            MergeInput {
                name: "docs".to_string(),
                origin: "docs.json".to_string(),
                graph: docs_graph(),
            },
        ]
    }

    #[test]
    fn merge_unifies_matching_nodes_with_provenance() {
        let (merged, report) = merge_graphs(inputs(), "combined").expect("merge");

        // repo roots (same label) merge; src/main.rs file nodes merge.
        assert_eq!(report.merged_nodes, 2);
        let file = merged
            .nodes
            .iter()
            .find(|node| node.label == "src/main.rs")
            .expect("merged file node");
        assert_eq!(
            file.metadata.get(MERGE_SOURCES_KEY).map(String::as_str),
            Some("code,docs"),
            "both inputs recorded as contributors"
        );
        assert_eq!(
            file.metadata.get("owner").map(String::as_str),
            Some("docs-team"),
            "non-conflicting metadata union is kept"
        );
        let doc = merged
            .nodes
            .iter()
            .find(|node| node.label == "docs/adr-1.md")
            .expect("docs-only node");
        assert_eq!(
            doc.metadata.get(MERGE_SOURCES_KEY).map(String::as_str),
            Some("docs")
        );

        // The docs->code reference edge survives with provenance.
        let doc_edge = merged
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::References)
            .expect("cross-graph reference edge");
        assert_eq!(
            doc_edge.metadata.get(MERGE_SOURCES_KEY).map(String::as_str),
            Some("docs")
        );
        // Duplicate contains edges (root->file in both inputs) dedupe.
        assert!(report.deduped_edges >= 1);
        assert!(report.metadata_conflicts.is_empty());
    }

    #[test]
    fn merge_is_deterministic_regardless_of_input_order() {
        let (merged_a, _) = merge_graphs(inputs(), "combined").expect("merge a");
        let mut reversed = inputs();
        reversed.reverse();
        let (merged_b, _) = merge_graphs(reversed, "combined").expect("merge b");
        assert_eq!(
            serde_json::to_string(&merged_a).unwrap(),
            serde_json::to_string(&merged_b).unwrap(),
            "same inputs must produce byte-identical committed artifacts"
        );
    }

    #[test]
    fn metadata_conflicts_are_reported_not_silent() {
        let mut a = CodeGraph::new("repo");
        a.add_node_with_metadata(
            NodeKind::File,
            "config.toml",
            Some(span("config.toml")),
            BTreeMap::from([("owner".to_string(), "team-a".to_string())]),
        );
        let mut b = CodeGraph::new("repo");
        b.add_node_with_metadata(
            NodeKind::File,
            "config.toml",
            Some(span("config.toml")),
            BTreeMap::from([("owner".to_string(), "team-b".to_string())]),
        );
        let (merged, report) = merge_graphs(
            vec![
                MergeInput {
                    name: "a".to_string(),
                    origin: "a.json".to_string(),
                    graph: a,
                },
                MergeInput {
                    name: "b".to_string(),
                    origin: "b.json".to_string(),
                    graph: b,
                },
            ],
            "combined",
        )
        .expect("merge");

        assert_eq!(report.metadata_conflicts.len(), 1);
        let conflict = &report.metadata_conflicts[0];
        assert_eq!(conflict.field, "owner");
        assert_eq!(conflict.dropped_source, "b");
        let node = merged
            .nodes
            .iter()
            .find(|node| node.label == "config.toml")
            .unwrap();
        assert_eq!(
            node.metadata.get("owner").map(String::as_str),
            Some("team-a"),
            "first sorted source wins deterministically"
        );
    }

    #[test]
    fn edge_confidence_upgrades_to_best_contributor() {
        let mut a = CodeGraph::new("repo");
        let main_a = a.add_node_with_span(NodeKind::Function, "main", span("src/main.rs"));
        let helper_a = a.add_node_with_span(NodeKind::Function, "helper", span("src/main.rs"));
        a.add_edge(main_a, helper_a, EdgeKind::Calls, Confidence::Heuristic);
        let mut b = CodeGraph::new("repo");
        let main_b = b.add_node_with_span(NodeKind::Function, "main", span("src/main.rs"));
        let helper_b = b.add_node_with_span(NodeKind::Function, "helper", span("src/main.rs"));
        b.add_edge(main_b, helper_b, EdgeKind::Calls, Confidence::Semantic);

        let (merged, report) = merge_graphs(
            vec![
                // Named so the heuristic input sorts (and merges) first: the
                // semantic contributor must upgrade the existing edge.
                MergeInput {
                    name: "a-syntax".to_string(),
                    origin: "syntax.json".to_string(),
                    graph: a,
                },
                MergeInput {
                    name: "b-semantic".to_string(),
                    origin: "semantic.json".to_string(),
                    graph: b,
                },
            ],
            "combined",
        )
        .expect("merge");

        let call = merged
            .edges
            .iter()
            .find(|edge| edge.kind == EdgeKind::Calls)
            .expect("call edge");
        assert_eq!(call.confidence, Confidence::Semantic);
        assert_eq!(report.confidence_upgrades, 1);
        assert_eq!(report.deduped_edges, 1);
        assert_eq!(
            call.metadata.get(MERGE_SOURCES_KEY).map(String::as_str),
            Some("a-syntax,b-semantic"),
        );
    }

    #[test]
    fn merge_rejects_single_inputs_and_duplicate_names() {
        let error = merge_graphs(
            vec![MergeInput {
                name: "solo".to_string(),
                origin: "solo.json".to_string(),
                graph: code_graph(),
            }],
            "combined",
        )
        .expect_err("single input");
        assert!(error.to_string().contains("at least two"));

        let error = merge_graphs(
            vec![
                MergeInput {
                    name: "same".to_string(),
                    origin: "a.json".to_string(),
                    graph: code_graph(),
                },
                MergeInput {
                    name: "same".to_string(),
                    origin: "b.json".to_string(),
                    graph: docs_graph(),
                },
            ],
            "combined",
        )
        .expect_err("duplicate names");
        assert!(error.to_string().contains("duplicate input name"));
    }
}
