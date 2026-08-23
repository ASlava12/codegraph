//! User-owned graph annotations from `.codegraph/annotations.toml` and
//! custom rule checks from `.codegraph/rules.toml`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use codegraph_core::{CodeGraph, Confidence, EdgeKind, NodeId, NodeKind};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn apply_graph_annotations(context: &mut IndexContext) {
    let annotations = context.annotations.clone();
    for message in annotations.parse_errors {
        add_annotation_parse_error(context, message);
    }

    for annotation in annotations.node_annotations {
        for node in &mut context.graph.nodes {
            if !node_annotation_matches(node, &annotation) {
                continue;
            }
            let ids = append_metadata_list(
                node.metadata.remove("annotation_ids"),
                annotation.id.as_str(),
            );
            node.metadata.insert("annotation_ids".to_string(), ids);
            node.metadata
                .insert("annotation_source".to_string(), "user".to_string());
            for (key, value) in &annotation.set {
                node.metadata
                    .insert(format!("annotation.{key}"), value.clone());
            }
        }
    }
}

pub(crate) fn add_annotation_parse_error(context: &mut IndexContext, message: String) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "annotation_error".to_string());
    metadata.insert("source".to_string(), "annotation".to_string());
    metadata.insert("message".to_string(), message);
    let id = context.graph.add_node_with_metadata(
        NodeKind::Unknown,
        "annotation parse error",
        None,
        metadata,
    );
    let root_id = context.graph.root;
    add_edge_once(context, root_id, id, EdgeKind::Contains, Confidence::Exact);
}

pub(crate) fn node_annotation_matches(
    node: &codegraph_core::Node,
    annotation: &NodeAnnotationRule,
) -> bool {
    annotation
        .kind
        .as_deref()
        .is_none_or(|expected| text_matches(node_kind_name(&node.kind), expected))
        && annotation
            .label
            .as_deref()
            .is_none_or(|expected| text_matches(&node.label, expected))
        && annotation.language.as_deref().is_none_or(|expected| {
            node.metadata
                .get("language")
                .is_some_and(|value| text_matches(value, expected))
        })
        && annotation.item_kind.as_deref().is_none_or(|expected| {
            node.metadata
                .get("item_kind")
                .is_some_and(|value| text_matches(value, expected))
        })
        && annotation.metadata.iter().all(|(key, expected)| {
            node.metadata
                .get(key)
                .is_some_and(|value| text_matches(value, expected))
        })
}

pub(crate) fn append_metadata_list(existing: Option<String>, value: &str) -> String {
    let mut values: BTreeSet<String> = existing
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    values.insert(value.to_string());
    values.into_iter().collect::<Vec<_>>().join(",")
}

pub(crate) fn graph_annotations(root: &Path) -> GraphAnnotations {
    let path = root.join(".codegraph").join("annotations.toml");
    if !path.is_file() {
        return GraphAnnotations::default();
    }

    let Ok(source) = fs::read_to_string(&path) else {
        return GraphAnnotations {
            parse_errors: vec![format!(
                "Could not read graph annotations from {}",
                path.display()
            )],
            ..GraphAnnotations::default()
        };
    };

    let Ok(value) = toml::from_str::<toml::Value>(&source) else {
        return GraphAnnotations {
            parse_errors: vec![format!(
                "Could not parse graph annotations from {}",
                path.display()
            )],
            ..GraphAnnotations::default()
        };
    };

    let annotations = value.get("annotations").unwrap_or(&value);
    GraphAnnotations {
        node_annotations: rule_array(annotations, "node")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| node_annotation_rule(value, index + 1))
            .collect(),
        parse_errors: Vec::new(),
    }
}

pub(crate) fn node_annotation_rule(
    value: &toml::Value,
    index: usize,
) -> Option<NodeAnnotationRule> {
    let set = string_table(value.get("set")?)?;
    if set.is_empty() {
        return None;
    }

    let id = value
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("node:{index}"));

    Some(NodeAnnotationRule {
        id,
        kind: optional_string(value, "kind"),
        label: optional_string(value, "label"),
        language: optional_string(value, "language"),
        item_kind: optional_string(value, "item_kind"),
        metadata: value
            .get("metadata")
            .and_then(string_table)
            .unwrap_or_default(),
        set,
    })
}

pub(crate) fn optional_string(value: &toml::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn string_table(value: &toml::Value) -> Option<BTreeMap<String, String>> {
    Some(
        value
            .as_table()?
            .iter()
            .filter_map(|(key, value)| {
                toml_scalar_to_string(value).map(|value| (key.to_string(), value))
            })
            .collect(),
    )
}

pub(crate) fn toml_scalar_to_string(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(value) => Some(value.clone()),
        toml::Value::Integer(value) => Some(value.to_string()),
        toml::Value::Float(value) => Some(value.to_string()),
        toml::Value::Boolean(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(crate) fn text_matches(actual: &str, expected: &str) -> bool {
    let actual = actual.to_ascii_lowercase();
    let expected = expected.trim().to_ascii_lowercase();
    !expected.is_empty() && (actual == expected || actual.contains(&expected))
}

pub(crate) fn node_kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Repository => "repository",
        NodeKind::Directory => "directory",
        NodeKind::File => "file",
        NodeKind::Module => "module",
        NodeKind::Function => "function",
        NodeKind::Entrypoint => "entrypoint",
        NodeKind::Type => "type",
        NodeKind::Config => "config",
        NodeKind::Environment => "environment",
        NodeKind::ExternalDependency => "external_dependency",
        NodeKind::ControlFlow => "control_flow",
        NodeKind::Unknown => "unknown",
    }
}

pub(crate) fn edge_kind_name(kind: &EdgeKind) -> &'static str {
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
}

pub(crate) fn apply_custom_rules(context: &mut IndexContext) {
    let rules = context.custom_rules.clone();
    for message in rules.parse_errors {
        add_custom_rule_violation(
            context,
            "rules_parse_error",
            "parse_error",
            "error",
            message,
            None,
        );
    }

    for rule in rules.forbidden_dependencies {
        let matches = matching_dependency_nodes(&context.graph, &rule);
        for dependency in matches {
            let dependency_label =
                graph_node_label(&context.graph, dependency).unwrap_or("unknown");
            let message = rule.message.clone().unwrap_or_else(|| {
                format!(
                    "Dependency `{dependency_label}` is forbidden by custom rule `{}`",
                    rule.id
                )
            });
            add_custom_rule_violation(
                context,
                &rule.id,
                "forbidden_dependency",
                &rule.severity,
                message,
                Some(dependency),
            );
        }
    }

    for rule in rules.forbidden_edges {
        let matches = matching_forbidden_edges(&context.graph, &rule);
        for edge_match in matches {
            let source = graph_node_label(&context.graph, edge_match.source).unwrap_or("unknown");
            let target = graph_node_label(&context.graph, edge_match.target).unwrap_or("unknown");
            let message = rule.message.clone().unwrap_or_else(|| {
                format!(
                    "Edge `{source}` -> `{target}` violates custom rule `{}`",
                    rule.id
                )
            });
            add_custom_rule_violation_with_targets(
                context,
                &rule.id,
                "forbidden_edge",
                &rule.severity,
                message,
                &[edge_match.source, edge_match.target],
                Some(edge_match.edge_index),
            );
        }
    }

    for rule in rules.required_configs {
        if custom_rule_config_exists(&context.graph, &rule.target) {
            continue;
        }
        let message = rule.message.clone().unwrap_or_else(|| {
            format!(
                "Required config or environment target `{}` is missing for custom rule `{}`",
                rule.target, rule.id
            )
        });
        add_custom_rule_violation(
            context,
            &rule.id,
            "required_config",
            &rule.severity,
            message,
            None,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ForbiddenEdgeMatch {
    edge_index: usize,
    source: NodeId,
    target: NodeId,
}

pub(crate) fn graph_node_label(graph: &CodeGraph, id: NodeId) -> Option<&str> {
    graph
        .nodes
        .iter()
        .find(|node| node.id == id)
        .map(|node| node.label.as_str())
}

pub(crate) fn matching_dependency_nodes(
    graph: &CodeGraph,
    rule: &ForbiddenDependencyRule,
) -> Vec<NodeId> {
    graph
        .nodes
        .iter()
        .filter_map(|node| {
            if node.kind != NodeKind::ExternalDependency
                || node
                    .metadata
                    .get("item_kind")
                    .is_none_or(|kind| kind != "dependency")
            {
                return None;
            }

            let package_id = node.metadata.get("package_id")?;
            if forbidden_dependency_matches(package_id, &node.label, rule) {
                Some(node.id)
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn matching_forbidden_edges(
    graph: &CodeGraph,
    rule: &ForbiddenEdgeRule,
) -> Vec<ForbiddenEdgeMatch> {
    graph
        .edges
        .iter()
        .enumerate()
        .filter_map(|(edge_index, edge)| {
            if rule
                .edge_kind
                .as_deref()
                .is_some_and(|expected| !text_matches(edge_kind_name(&edge.kind), expected))
            {
                return None;
            }
            // Once per edge, so searching the whole node list here made a
            // rule cost the graph's edges times its nodes.
            let source = graph_node(graph, edge.source)?;
            let target = graph_node(graph, edge.target)?;
            if endpoint_rule_matches(
                source,
                rule.source_kind.as_deref(),
                rule.source_label.as_deref(),
                &rule.source_metadata,
            ) && endpoint_rule_matches(
                target,
                rule.target_kind.as_deref(),
                rule.target_label.as_deref(),
                &rule.target_metadata,
            ) {
                Some(ForbiddenEdgeMatch {
                    edge_index,
                    source: edge.source,
                    target: edge.target,
                })
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn endpoint_rule_matches(
    node: &codegraph_core::Node,
    kind: Option<&str>,
    label: Option<&str>,
    metadata: &BTreeMap<String, String>,
) -> bool {
    kind.is_none_or(|expected| text_matches(node_kind_name(&node.kind), expected))
        && label.is_none_or(|expected| text_matches(&node.label, expected))
        && metadata.iter().all(|(key, expected)| {
            node.metadata
                .get(key)
                .is_some_and(|value| text_matches(value, expected))
        })
}

pub(crate) fn forbidden_dependency_matches(
    package_id: &str,
    package_label: &str,
    rule: &ForbiddenDependencyRule,
) -> bool {
    let Some((ecosystem, package)) = package_id.split_once(':') else {
        return false;
    };
    if rule
        .ecosystem
        .as_deref()
        .is_some_and(|expected| !expected.eq_ignore_ascii_case(ecosystem))
    {
        return false;
    }

    let expected = canonical_package_name(ecosystem, &rule.package);
    package.eq_ignore_ascii_case(&expected)
        || package_label.eq_ignore_ascii_case(&rule.package)
        || package_label.eq_ignore_ascii_case(&expected)
}

pub(crate) fn custom_rule_config_exists(graph: &CodeGraph, target: &str) -> bool {
    graph.nodes.iter().any(|node| {
        if !matches!(node.kind, NodeKind::Config | NodeKind::Environment) {
            return false;
        }
        custom_rule_text_matches(&node.label, target)
            || node
                .metadata
                .values()
                .any(|value| custom_rule_text_matches(value, target))
    })
}

pub(crate) fn custom_rule_text_matches(value: &str, expected: &str) -> bool {
    let value = value.to_ascii_lowercase();
    let expected = expected.trim().to_ascii_lowercase();
    !expected.is_empty() && (value == expected || value.contains(&expected))
}

pub(crate) fn add_custom_rule_violation(
    context: &mut IndexContext,
    rule_id: &str,
    rule_kind: &str,
    severity: &str,
    message: String,
    target: Option<NodeId>,
) {
    let targets = target.into_iter().collect::<Vec<_>>();
    add_custom_rule_violation_with_targets(
        context, rule_id, rule_kind, severity, message, &targets, None,
    );
}

pub(crate) fn add_custom_rule_violation_with_targets(
    context: &mut IndexContext,
    rule_id: &str,
    rule_kind: &str,
    severity: &str,
    message: String,
    targets: &[NodeId],
    violated_edge_index: Option<usize>,
) {
    let mut metadata = BTreeMap::new();
    metadata.insert("item_kind".to_string(), "custom_rule_violation".to_string());
    metadata.insert("source".to_string(), "custom_rule".to_string());
    metadata.insert("rule_id".to_string(), rule_id.to_string());
    metadata.insert("rule_kind".to_string(), rule_kind.to_string());
    metadata.insert(
        "severity".to_string(),
        normalize_rule_severity(severity).to_string(),
    );
    metadata.insert("message".to_string(), message.clone());
    if let Some(edge_index) = violated_edge_index {
        metadata.insert("violated_edge_index".to_string(), edge_index.to_string());
    }

    let violation = context.graph.add_node_with_metadata(
        NodeKind::Unknown,
        format!("custom rule violation:{rule_id}"),
        None,
        metadata,
    );
    let root_id = context.graph.root;
    add_edge_once(
        context,
        root_id,
        violation,
        EdgeKind::Contains,
        Confidence::Exact,
    );

    for target in targets {
        let mut edge_metadata = BTreeMap::new();
        edge_metadata.insert("source".to_string(), "custom_rule".to_string());
        edge_metadata.insert("relation".to_string(), "custom_rule_target".to_string());
        edge_metadata.insert("rule_id".to_string(), rule_id.to_string());
        add_edge_once_with_metadata(
            context,
            violation,
            *target,
            EdgeKind::References,
            Confidence::Exact,
            edge_metadata,
        );
    }
}

pub(crate) fn custom_rules(root: &Path) -> CustomRules {
    let path = root.join(".codegraph").join("rules.toml");
    if !path.is_file() {
        return CustomRules::default();
    }

    let Ok(source) = fs::read_to_string(&path) else {
        return CustomRules {
            parse_errors: vec![format!(
                "Could not read custom rules from {}",
                path.display()
            )],
            ..CustomRules::default()
        };
    };

    let Ok(value) = toml::from_str::<toml::Value>(&source) else {
        return CustomRules {
            parse_errors: vec![format!(
                "Could not parse custom rules from {}",
                path.display()
            )],
            ..CustomRules::default()
        };
    };

    let rules = value.get("rules").unwrap_or(&value);
    CustomRules {
        forbidden_dependencies: rule_array(rules, "forbidden_dependency")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| forbidden_dependency_rule(value, index + 1))
            .collect(),
        forbidden_edges: rule_array(rules, "forbidden_edge")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| forbidden_edge_rule(value, index + 1))
            .collect(),
        required_configs: rule_array(rules, "required_config")
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| required_config_rule(value, index + 1))
            .collect(),
        parse_errors: Vec::new(),
    }
}

pub(crate) fn rule_array<'a>(rules: &'a toml::Value, name: &str) -> Vec<&'a toml::Value> {
    rules
        .get(name)
        .and_then(|value| value.as_array())
        .map(|values| values.iter().collect())
        .unwrap_or_default()
}

pub(crate) fn forbidden_dependency_rule(
    value: &toml::Value,
    index: usize,
) -> Option<ForbiddenDependencyRule> {
    let package = value.get("package")?.as_str()?.trim().to_string();
    if package.is_empty() {
        return None;
    }
    Some(ForbiddenDependencyRule {
        id: rule_id(value, "forbidden_dependency", index, &package),
        package,
        ecosystem: value
            .get("ecosystem")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty()),
        severity: rule_severity(value),
        message: rule_message(value),
    })
}

pub(crate) fn forbidden_edge_rule(value: &toml::Value, index: usize) -> Option<ForbiddenEdgeRule> {
    let source_metadata = value
        .get("source_metadata")
        .and_then(string_table)
        .unwrap_or_default();
    let target_metadata = value
        .get("target_metadata")
        .and_then(string_table)
        .unwrap_or_default();
    let source_kind = optional_string(value, "source_kind");
    let source_label = optional_string(value, "source_label");
    let target_kind = optional_string(value, "target_kind");
    let target_label = optional_string(value, "target_label");

    if source_metadata.is_empty()
        && target_metadata.is_empty()
        && source_kind.is_none()
        && source_label.is_none()
        && target_kind.is_none()
        && target_label.is_none()
    {
        return None;
    }

    Some(ForbiddenEdgeRule {
        id: rule_id(value, "forbidden_edge", index, "edge"),
        edge_kind: optional_string(value, "edge_kind"),
        source_kind,
        source_label,
        source_metadata,
        target_kind,
        target_label,
        target_metadata,
        severity: rule_severity(value),
        message: rule_message(value),
    })
}

pub(crate) fn required_config_rule(
    value: &toml::Value,
    index: usize,
) -> Option<RequiredConfigRule> {
    let target = value.get("target")?.as_str()?.trim().to_string();
    if target.is_empty() {
        return None;
    }
    Some(RequiredConfigRule {
        id: rule_id(value, "required_config", index, &target),
        target,
        severity: rule_severity(value),
        message: rule_message(value),
    })
}

pub(crate) fn rule_id(value: &toml::Value, kind: &str, index: usize, fallback: &str) -> String {
    value
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{kind}:{index}:{fallback}"))
}

pub(crate) fn rule_severity(value: &toml::Value) -> String {
    value
        .get("severity")
        .and_then(|value| value.as_str())
        .map(normalize_rule_severity)
        .unwrap_or("warning")
        .to_string()
}

pub(crate) fn normalize_rule_severity(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" => "error",
        "info" => "info",
        _ => "warning",
    }
}

pub(crate) fn rule_message(value: &toml::Value) -> Option<String> {
    value
        .get("message")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
