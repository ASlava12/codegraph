use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const CODEGRAPH_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Repository,
    Directory,
    File,
    Module,
    Function,
    Entrypoint,
    Type,
    Config,
    Environment,
    ExternalDependency,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    Defines,
    References,
    ReadsConfig,
    ReadsEnvironment,
    MayError,
    Entrypoint,
    DependsOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Exact,
    Semantic,
    Syntactic,
    Heuristic,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    pub span: Option<SourceSpan>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    pub confidence: Confidence,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeGraph {
    pub schema_version: u32,
    pub root: NodeId,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl CodeGraph {
    pub fn new(root_label: impl Into<String>) -> Self {
        let root = NodeId(1);
        Self {
            schema_version: CODEGRAPH_SCHEMA_VERSION,
            root,
            nodes: vec![Node {
                id: root,
                kind: NodeKind::Repository,
                label: root_label.into(),
                span: None,
                metadata: BTreeMap::new(),
            }],
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, kind: NodeKind, label: impl Into<String>) -> NodeId {
        self.add_node_with_metadata(kind, label, None, BTreeMap::new())
    }

    pub fn add_node_with_span(
        &mut self,
        kind: NodeKind,
        label: impl Into<String>,
        span: SourceSpan,
    ) -> NodeId {
        self.add_node_with_metadata(kind, label, Some(span), BTreeMap::new())
    }

    pub fn add_node_with_metadata(
        &mut self,
        kind: NodeKind,
        label: impl Into<String>,
        span: Option<SourceSpan>,
        metadata: BTreeMap<String, String>,
    ) -> NodeId {
        let id = NodeId(self.nodes.len() as u64 + 1);
        self.nodes.push(Node {
            id,
            kind,
            label: label.into(),
            span,
            metadata,
        });
        id
    }

    pub fn add_edge(
        &mut self,
        source: NodeId,
        target: NodeId,
        kind: EdgeKind,
        confidence: Confidence,
    ) {
        self.add_edge_with_metadata(source, target, kind, confidence, BTreeMap::new());
    }

    pub fn add_edge_with_metadata(
        &mut self,
        source: NodeId,
        target: NodeId,
        kind: EdgeKind,
        confidence: Confidence,
        metadata: BTreeMap<String, String>,
    ) {
        self.edges.push(Edge {
            source,
            target,
            kind,
            confidence,
            metadata,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_levels_round_trip_as_stable_snake_case_values() {
        let cases = [
            (Confidence::Exact, "exact"),
            (Confidence::Semantic, "semantic"),
            (Confidence::Syntactic, "syntactic"),
            (Confidence::Heuristic, "heuristic"),
            (Confidence::Unknown, "unknown"),
        ];

        for (confidence, expected) in cases {
            let encoded = serde_json::to_value(confidence).expect("serialize confidence");
            assert_eq!(encoded, serde_json::json!(expected));
            let decoded: Confidence =
                serde_json::from_value(encoded).expect("deserialize confidence");
            assert_eq!(decoded, confidence);
        }
    }
}
