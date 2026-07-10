//! Natural-language ask: deterministic question-to-query planning with
//! rules, alternatives, and copy-paste CLI snippets.

use codegraph_core::{CodeGraph, Edge, Node, NodeId, NodeKind};
use std::collections::BTreeMap;

#[allow(unused_imports)]
use crate::*;

pub const NATURAL_QUERY_SCHEMA: &str = "codegraph.ask.v1";

pub fn natural_query(
    graph: &CodeGraph,
    request: NaturalQueryRequest,
) -> Result<NaturalQueryReport, QueryError> {
    let mut plan = natural_query_plan(&request.question)?;
    let mut alternatives = plan.alternatives.clone();
    let mut result = match query_graph(graph, &plan.generated_query) {
        Ok(result) => result,
        Err(error) => {
            let fallback = natural_query_fallback_query(plan.term.as_deref());
            if fallback == plan.generated_query {
                return Err(error);
            }
            alternatives.insert(0, plan.generated_query.clone());
            plan.generated_query = fallback;
            plan.rule = format!("fallback_after_unmatched_anchor: {}", plan.rule);
            plan.confidence = "low".to_string();
            query_graph(graph, &plan.generated_query)?
        }
    };
    if request.compact {
        result = compact_query_result(result);
    }
    alternatives.retain(|alternative| alternative != &plan.generated_query);
    alternatives.sort();
    alternatives.dedup();
    Ok(NaturalQueryReport {
        schema: NATURAL_QUERY_SCHEMA.to_string(),
        cli_snippet: format!("codegraph query '{}' .", plan.generated_query),
        question: request.question,
        generated_query: plan.generated_query,
        rule: plan.rule,
        confidence: plan.confidence,
        result,
        alternatives,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct NaturalQueryPlan {
    pub(crate) generated_query: String,
    pub(crate) rule: String,
    pub(crate) confidence: String,
    pub(crate) term: Option<String>,
    pub(crate) alternatives: Vec<String>,
}

pub(crate) fn natural_query_plan(question: &str) -> Result<NaturalQueryPlan, QueryError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(QueryError::new("natural-language question is empty"));
    }

    let lower = question.to_lowercase();
    let candidates = natural_query_candidates(question);
    let term = candidates.first().cloned();
    let quoted_term = term.as_deref().map(quote_query_value);
    let fallback = natural_query_fallback_query(term.as_deref());
    let mut alternatives = vec![fallback.clone()];
    if let Some(term) = quoted_term.as_deref() {
        alternatives.push(format!("insights search:{term}"));
        alternatives.push(format!(
            "symbols search:{term} direction:out edge_limit:300"
        ));
    } else {
        alternatives.push("insights".to_string());
        alternatives.push("entrypoints".to_string());
    }

    let (generated_query, rule, confidence) = if natural_query_mentions_any(
        &lower,
        &[
            "path",
            "between",
            "from ",
            " to ",
            "trace",
            "путь",
            "между",
            "от ",
            " до ",
            "трасс",
        ],
    ) && candidates.len() >= 2
    {
        (
            format!(
                "path from:{} to:{} depth:6",
                quote_query_value(&candidates[0]),
                quote_query_value(&candidates[1])
            ),
            "path_between_anchors".to_string(),
            "medium".to_string(),
        )
    } else if let Some(env_term) = candidates
        .iter()
        .find(|candidate| screaming_snake_token(candidate))
        .filter(|_| {
            natural_query_mentions_any(
                &lower,
                &[
                    "read",
                    "set",
                    "write",
                    "written",
                    "assign",
                    "загруж",
                    "чита",
                    "запис",
                    "установ",
                    "задан",
                ],
            )
        })
    {
        // An ALL_CAPS token plus a read/set verb is a config/environment
        // question even without the words "config" or "env" — and identifier
        // substrings like API inside CODEGRAPH_API_TOKEN must not pull the
        // question into the route rule.
        (
            format!("configs target:{} depth:6", quote_query_value(env_term)),
            "config_or_environment".to_string(),
            "high".to_string(),
        )
    } else if natural_query_mentions_any(
        &lower,
        &[
            "config",
            "environment",
            "env",
            "setting",
            "перемен",
            "конфиг",
            "настрой",
            "окружен",
        ],
    ) && !natural_query_mentions_any(
        &lower,
        &["call", "caller", "callee", "invoke", "вызов", "вызыва"],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("configs target:{term} depth:6"),
                "config_or_environment".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "configs depth:6".to_string(),
                "config_or_environment".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "error",
            "exception",
            "panic",
            "throw",
            "fail",
            "ошиб",
            "исключ",
            "паник",
            "сбой",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("errors target:{term} depth:6"),
                "error_or_exception".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "errors depth:6".to_string(),
                "error_or_exception".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "entrypoint",
            "startup",
            "start",
            "main",
            "boot",
            "точк",
            "запуск",
            "старт",
            "вход",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("entrypoints search:{term}"),
                "entrypoint_or_startup".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "entrypoints".to_string(),
                "entrypoint_or_startup".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "route",
            "endpoint",
            "http",
            "api",
            "handler",
            "маршрут",
            "эндпоинт",
            "ручк",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            let key = if candidates.first().is_some_and(|term| term.starts_with('/')) {
                "path"
            } else {
                "handler"
            };
            (
                format!("routes {key}:{term} depth:4 edge_limit:300"),
                "route_or_endpoint".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "routes depth:4 edge_limit:300".to_string(),
                "route_or_endpoint".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "dependent",
            "impact",
            "who uses",
            "used by",
            "кто использ",
            "кто завис",
            "влияни",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("dependents label:{term} depth:4"),
                "reverse_dependency_or_impact".to_string(),
                "high".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "dependency",
            "dependencies",
            "package",
            "import",
            "crate",
            "завис",
            "пакет",
            "импорт",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("packages package:{term} edge_limit:300"),
                "package_or_import".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "packages edge_limit:300".to_string(),
                "package_or_import".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &["call", "caller", "callee", "invoke", "вызов", "вызыва"],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            let direction = if natural_query_mentions_any(
                &lower,
                &["who calls", "callers", "called by", "кто вызывает"],
            ) {
                "in"
            } else {
                "out"
            };
            (
                format!("neighbors label:{term} direction:{direction} depth:2 edge_kind:calls"),
                "call_neighborhood".to_string(),
                "medium".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(&lower, &["file", "source", "path", "файл", "исход"])
    {
        if let Some(raw_term) = term.as_deref() {
            let term = quote_query_value(raw_term);
            let key = if raw_term.contains('/') || raw_term.contains('.') {
                "path"
            } else {
                "search"
            };
            (
                format!("files {key}:{term} direction:out edge_limit:300"),
                "file_or_source".to_string(),
                "high".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "symbol",
            "function",
            "class",
            "type",
            "method",
            "функц",
            "класс",
            "метод",
            "тип",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("symbols search:{term} direction:out edge_limit:300"),
                "symbol_search".to_string(),
                "high".to_string(),
            )
        } else {
            (
                fallback.clone(),
                "general_search".to_string(),
                "low".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "unreachable",
            "dead",
            "unused",
            "orphan",
            "мертв",
            "неисп",
            "недостиж",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("unreachable search:{term}"),
                "unreachable_or_unused".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "unreachable".to_string(),
                "unreachable_or_unused".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "hotspot",
            "hub",
            "central",
            "important",
            "важн",
            "централ",
            "узел",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("hotspots search:{term} min_score:3 edge_limit:300"),
                "hotspot_or_centrality".to_string(),
                "medium".to_string(),
            )
        } else {
            (
                "hotspots min_score:3 edge_limit:300".to_string(),
                "hotspot_or_centrality".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &lower,
        &[
            "risk",
            "issue",
            "problem",
            "warning",
            "security",
            "риск",
            "проблем",
            "уязв",
            "предупреж",
        ],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            (
                format!("insights search:{term}"),
                "risk_or_insight".to_string(),
                "medium".to_string(),
            )
        } else {
            (
                "insights".to_string(),
                "risk_or_insight".to_string(),
                "medium".to_string(),
            )
        }
    } else {
        (
            fallback.clone(),
            "general_search".to_string(),
            "low".to_string(),
        )
    };

    alternatives.push(generated_query.clone());
    alternatives.retain(|alternative| alternative != &generated_query);
    alternatives.sort();
    alternatives.dedup();

    Ok(NaturalQueryPlan {
        generated_query,
        rule,
        confidence,
        term,
        alternatives,
    })
}

pub(crate) fn natural_query_fallback_query(term: Option<&str>) -> String {
    term.map(|term| format!("nodes search:{} limit:50", quote_query_value(term)))
        .unwrap_or_else(|| "nodes limit:50".to_string())
}

/// SCREAMING_SNAKE identifiers such as DATABASE_URL or PORT: uppercase
/// letters and digits, at least three characters, starting with a letter.
pub(crate) fn screaming_snake_token(token: &str) -> bool {
    token.len() >= 3
        && token.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && token
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

pub(crate) fn natural_query_mentions_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

pub(crate) fn natural_query_candidates(question: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    candidates.extend(natural_query_quoted_terms(question));

    for token in question.split(|character: char| {
        !(character.is_alphanumeric() || matches!(character, '_' | '.' | '/' | ':' | '-'))
    }) {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '.' | ',' | ';' | ':' | '?' | '!' | '(' | ')' | '[' | ']'
            )
        });
        if token.len() < 2 || candidates.iter().any(|candidate| candidate == token) {
            continue;
        }
        if natural_query_token_looks_specific(token) {
            candidates.push(token.to_string());
        }
    }

    if candidates.is_empty()
        && let Some(token) = question
            .split_whitespace()
            .map(|token| {
                token.trim_matches(|character: char| {
                    !character.is_alphanumeric() && !matches!(character, '_' | '.' | '/' | '-')
                })
            })
            .filter(|token| token.chars().count() >= 3)
            .rfind(|token| !natural_query_stop_word(&token.to_lowercase()))
    {
        candidates.push(token.to_string());
    }

    candidates
}

pub(crate) fn natural_query_quoted_terms(question: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut quote = None;
    let mut current = String::new();
    for character in question.chars() {
        if matches!(character, '"' | '\'' | '`') {
            if quote == Some(character) {
                let term = current.trim();
                if !term.is_empty() {
                    terms.push(term.to_string());
                }
                current.clear();
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            } else {
                current.push(character);
            }
        } else if quote.is_some() {
            current.push(character);
        }
    }
    terms
}

pub(crate) fn natural_query_token_looks_specific(token: &str) -> bool {
    let lower = token.to_lowercase();
    if natural_query_stop_word(&lower) {
        return false;
    }
    token.contains('_')
        || token.contains('/')
        || token.contains('.')
        || token.contains("::")
        || token
            .chars()
            .any(|character| character.is_ascii_uppercase())
        || token.chars().any(|character| character.is_ascii_digit())
}

pub(crate) fn natural_query_stop_word(token: &str) -> bool {
    matches!(
        token,
        "what"
            | "where"
            | "when"
            | "who"
            | "how"
            | "why"
            | "which"
            | "does"
            | "do"
            | "is"
            | "are"
            | "the"
            | "a"
            | "an"
            | "for"
            | "from"
            | "to"
            | "of"
            | "in"
            | "on"
            | "with"
            | "and"
            | "or"
            | "code"
            | "graph"
            | "где"
            | "как"
            | "что"
            | "кто"
            | "куда"
            | "зачем"
            | "почему"
            | "это"
            | "этот"
            | "эта"
            | "для"
            | "из"
            | "от"
            | "до"
            | "по"
            | "в"
            | "на"
            | "и"
            | "или"
            | "код"
            | "граф"
    )
}

pub(crate) fn query_compaction_group_key(node: &Node, edges: &[Edge]) -> Option<String> {
    if !query_compaction_low_signal_node(node) {
        return None;
    }
    let degree = edges
        .iter()
        .filter(|edge| edge.source == node.id || edge.target == node.id)
        .count();
    if degree > 2 {
        return None;
    }
    let language = node
        .metadata
        .get("language")
        .map(String::as_str)
        .unwrap_or("unknown");
    let item_kind = node
        .metadata
        .get("item_kind")
        .map(String::as_str)
        .unwrap_or("unknown");
    Some(format!(
        "{}:{}:{}",
        kind_name(&node.kind),
        language,
        item_kind
    ))
}

pub(crate) fn query_compaction_low_signal_node(node: &Node) -> bool {
    if node
        .metadata
        .get("compacted")
        .is_some_and(|value| value == "true")
    {
        return false;
    }
    if node
        .metadata
        .get("item_kind")
        .is_some_and(|value| matches!(value.as_str(), "entrypoint" | "route" | "config"))
    {
        return false;
    }
    matches!(
        node.kind,
        NodeKind::Function
            | NodeKind::Module
            | NodeKind::Unknown
            | NodeKind::ControlFlow
            | NodeKind::ExternalDependency
    )
}

pub(crate) fn query_compacted_node(id: NodeId, group_key: &str, members: Vec<&Node>) -> Node {
    let count = members.len();
    let mut source_node_ids = members
        .iter()
        .map(|node| node.id.to_string())
        .collect::<Vec<_>>();
    source_node_ids.sort();
    let mut parts = group_key.split(':');
    let kind = parts.next().unwrap_or("node");
    let language = parts.next().unwrap_or("unknown");
    let item_kind = parts.next().unwrap_or("unknown");
    let label = if language == "unknown" && item_kind == "unknown" {
        format!("{count} compacted {kind} nodes")
    } else {
        format!("{count} compacted {language} {kind} nodes")
    };

    Node {
        id,
        kind: NodeKind::Unknown,
        label,
        span: None,
        metadata: BTreeMap::from([
            ("compacted".to_string(), "true".to_string()),
            ("compacted_count".to_string(), count.to_string()),
            ("compacted_kind".to_string(), kind.to_string()),
            ("compacted_language".to_string(), language.to_string()),
            ("compacted_item_kind".to_string(), item_kind.to_string()),
            ("source_node_ids".to_string(), source_node_ids.join(",")),
        ]),
    }
}
