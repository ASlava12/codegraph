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
    // A guessed anchor is a word from the question, not a name from the
    // project: "what routes exist" became `routes handler:exist` and
    // answered with nothing at all, where the question deserved the
    // project's routes. Keep the filter only when the name is really there.
    if plan.anchor_is_guessed
        && plan
            .term
            .as_deref()
            .is_some_and(|term| !graph_names_exactly(graph, term))
    {
        plan = widened_plan(&request.question, plan.term.as_deref())?;
    }
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
    // The name was there, but the question did not turn out to be about it:
    // flask has something called `read`, yet "what config does the app read"
    // is not asking about it, and `configs target:read` answered with
    // nothing. A guess that yields nothing was not a name in this question.
    if plan.anchor_is_guessed && result.nodes.is_empty() {
        let widened = widened_plan(&request.question, plan.term.as_deref())?;
        if widened.generated_query != plan.generated_query
            && let Ok(widened_result) = query_graph(graph, &widened.generated_query)
            && !widened_result.nodes.is_empty()
        {
            alternatives.insert(0, plan.generated_query.clone());
            plan = widened;
            result = widened_result;
        }
    }
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

/// The plan to fall back to when a guessed anchor was not a name in this
/// project. The word is no longer a filter, but it is still the reader's
/// best search: "what depends on lyaml" is answered better by the three
/// files that mention lyaml than by the project's first fifty nodes.
fn widened_plan(question: &str, guess: Option<&str>) -> Result<NaturalQueryPlan, QueryError> {
    let mut plan = natural_query_plan_with_anchor(question, false)?;
    if let Some(guess) = guess
        && plan.generated_query == natural_query_fallback_query(None)
    {
        plan.generated_query = natural_query_fallback_query(Some(guess));
        plan.term = Some(guess.to_string());
        plan.rule = "general_search_from_question_word".to_string();
        plan.confidence = "low".to_string();
    }
    Ok(plan)
}

/// Whether the project has something by exactly this name. The bar is
/// deliberately higher than the query language's substring filters: the
/// caller is checking a guess, and `exists` must not be taken as evidence
/// that "exist" was a name, nor `startswith` that "start" was one.
fn graph_names_exactly(graph: &CodeGraph, term: &str) -> bool {
    let needle = term.to_lowercase();
    graph
        .nodes
        .iter()
        .any(|node| node.label.to_lowercase() == needle)
}

#[derive(Debug, Clone)]
pub(crate) struct NaturalQueryPlan {
    pub(crate) generated_query: String,
    pub(crate) rule: String,
    pub(crate) confidence: String,
    pub(crate) term: Option<String>,
    /// The anchor is a word taken from the question because nothing in it
    /// looked like a name. Filtering on it is only sound once the project
    /// is known to have something by that name.
    pub(crate) anchor_is_guessed: bool,
    pub(crate) alternatives: Vec<String>,
}

pub(crate) fn natural_query_plan(question: &str) -> Result<NaturalQueryPlan, QueryError> {
    natural_query_plan_with_anchor(question, true)
}

pub(crate) fn natural_query_plan_with_anchor(
    question: &str,
    allow_guessed_anchor: bool,
) -> Result<NaturalQueryPlan, QueryError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(QueryError::new("natural-language question is empty"));
    }

    let lower = question.to_lowercase();
    let mut candidates = natural_query_candidates(question);
    let mut anchor_is_guessed = false;
    if candidates.is_empty()
        && allow_guessed_anchor
        && let Some(guess) = natural_query_guessed_anchor(question)
    {
        candidates.push(guess);
        anchor_is_guessed = true;
    }
    let term = candidates.first().cloned();
    // Route on the question, not on the name being asked about. `load_config`
    // contains "config", so "What calls `load_config`?" was answered with the
    // config-flow query; the same hijack hit every symbol named after a
    // keyword — `handleError`, `mainLoop`, `routeTable`.
    let routing = term
        .as_deref()
        .filter(|term| natural_query_token_looks_specific(term))
        .map(|term| lower.replace(&term.to_lowercase(), " "))
        .unwrap_or_else(|| lower.clone());
    // A word the routing rules match on is what the question is about,
    // not a name to filter by: "what are the public APIs" read `APIs` as
    // a name because of the capitals, so `search:APIs` answered with
    // nothing at all -- and so did `routes handler:HTTP` and `configs
    // target:CI`.
    let term = term.filter(|term| !natural_query_routes_on_this_word(term));
    let quoted_term = term.as_deref().map(quote_query_value);
    // A guessed anchor is a word from the question, not a name from the
    // project, and topic questions ask about the project as a whole: "what
    // are the riskiest files?" is answered by the findings, not by searching
    // them for the word `files`.
    let filter_term = if anchor_is_guessed {
        None
    } else {
        quoted_term.clone()
    };
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
        &routing,
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
    // Naming calling outright settles the question before any topic word
    // can: `who calls route` asks about the function named `route`, not
    // about the project's HTTP routes, and `does main call init` is not a
    // question about entrypoints. The topic rules below all key on nouns
    // that a symbol's own name may contain.
    } else if natural_query_mentions_any(
        &routing,
        &["call", "caller", "callee", "invoke", "вызов", "вызыва"],
    ) {
        if let Some(term) = quoted_term.as_deref() {
            let direction = natural_call_direction(&lower, Some(term));
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
    } else if natural_query_mentions_any(
        &routing,
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
        &routing,
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
        &routing,
        &[
            "entrypoint",
            // Written as two words as often as one.
            "entry point",
            "startup",
            "start",
            "main",
            "boot",
            // "how do I run the tests" asks what starts the project, and
            // reached a text search for the word `tests`.
            "how do i run",
            "how to run",
            "как запус",
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
        &routing,
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
        &routing,
        &[
            "dependent",
            "impact",
            "who uses",
            "used by",
            "depends on",
            "depend on",
            // The question an agent asks before an edit: "what would break
            // if I change X" is what depends on X.
            "would break",
            "will break",
            "if i change",
            "if i remove",
            "if i rename",
            "что сломает",
            "если изменить",
            "кто использ",
            "кто завис",
            "зависит от",
            "зависят от",
            "влияни",
        ],
    ) && !(routing.contains("does") && routing.contains("depend"))
    {
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
        &routing,
        &[
            // "what does this project depend on" asks the same question as
            // "what are its dependencies", and reached a text search for
            // the word `depend`.
            "depend",
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
        // "what does kong/init.lua import?" names a file, not a package:
        // `packages package:kong/init.lua` matched nothing, because no
        // dependency is called that.
        if let Some(raw_term) = term
            .as_deref()
            .filter(|term| term.contains('/') || term.contains('.'))
        {
            (
                format!(
                    "files path:{} direction:out edge_limit:300",
                    quote_query_value(raw_term)
                ),
                "file_imports".to_string(),
                "high".to_string(),
            )
        } else if let Some(term) = quoted_term.as_deref() {
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
        &routing,
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
        &routing,
        &[
            // The anchor is taken out of the question before routing, so
            // "what are the public APIs" routes on "what are the public".
            "public",
            "api surface",
            "exported",
            "export",
            "публичн",
            "экспортир",
        ],
    ) {
        // What a library offers outwards is what it declares public, and
        // the graph records that wherever the language states it.
        if let Some(term) = filter_term.as_deref() {
            (
                format!("symbols metadata.visibility:public search:{term} limit:50"),
                "public_api_surface".to_string(),
                "medium".to_string(),
            )
        } else {
            (
                "symbols metadata.visibility:public limit:50".to_string(),
                "public_api_surface".to_string(),
                "medium".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &routing,
        &["cycle", "circular", "circle", "цикл", "кольц"],
    ) {
        // The graph answers this one outright, and "show me the cycles"
        // used to fall through to a text search that matched nothing.
        if let Some(term) = filter_term.as_deref() {
            (
                format!("cycles search:{term}"),
                "dependency_cycle".to_string(),
                "high".to_string(),
            )
        } else {
            (
                "cycles".to_string(),
                "dependency_cycle".to_string(),
                "high".to_string(),
            )
        }
    } else if natural_query_mentions_any(
        &routing,
        &[
            "hotspot",
            "hub",
            "central",
            // What a reader means by "most coupled" is which nodes carry
            // the most edges.
            "coupled",
            "coupling",
            "связан",
            "important",
            "важн",
            "централ",
            "узел",
        ],
    ) {
        if let Some(term) = filter_term.as_deref() {
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
        &routing,
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
        if let Some(term) = filter_term.as_deref() {
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
    } else if natural_query_mentions_any(
        &routing,
        &[
            "table",
            "column",
            "schema",
            "sql",
            "таблиц",
            "колонк",
            "схем",
        ],
    ) {
        // A project's schema is a first-class part of its graph: kong
        // declares 39 tables inside Lua migrations, and "which tables does
        // the schema define?" used to search for the word `define`.
        match filter_term.as_deref() {
            Some(term) => (
                format!("sql table:{term} limit:50"),
                "sql_schema".to_string(),
                "high".to_string(),
            ),
            None => (
                "sql limit:50".to_string(),
                "sql_schema".to_string(),
                "medium".to_string(),
            ),
        }
    } else if natural_query_mentions_any(&routing, &["file", "source", "path", "файл", "исход"])
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
        &routing,
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
        anchor_is_guessed,
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

/// Which way a question about calls points.
///
/// "What calls `X`?" asks for callers and "what does `X` call?" asks for
/// callees, but only the phrase "who calls" was recognised, so every other
/// caller phrasing was answered with the callee list — the opposite of the
/// question. Explicit phrasings win; otherwise the verb's position settles it,
/// since a caller question puts the verb before the subject and a callee
/// question puts it after.
pub(crate) fn natural_call_direction(lower: &str, term: Option<&str>) -> &'static str {
    if natural_query_mentions_any(
        lower,
        &["called by", "invoked by", "callee", "кого вызывает"],
    ) || (lower.contains("does") && lower.contains("call"))
    {
        return "out";
    }
    if natural_query_mentions_any(
        lower,
        &[
            "who calls",
            "what calls",
            "which calls",
            "that calls",
            "callers",
            "calls to",
            "call sites",
            "кто вызывает",
            "вызывающие",
        ],
    ) {
        return "in";
    }

    let verb = ["call", "invoke", "вызыва"]
        .iter()
        .filter_map(|verb| lower.find(verb))
        .min();
    let subject = term
        .map(str::to_lowercase)
        .and_then(|term| lower.find(&term));
    match (verb, subject) {
        (Some(verb), Some(subject)) if verb < subject => "in",
        _ => "out",
    }
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

    candidates
}

/// The word a question is about when nothing in it looks like a name. It
/// is a guess by construction — the last word that is not a stop word —
/// so callers must confirm the project has something by that name before
/// filtering on it.
pub(crate) fn natural_query_guessed_anchor(question: &str) -> Option<String> {
    let words = question
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                !character.is_alphanumeric() && !matches!(character, '_' | '.' | '/' | '-')
            })
        })
        .filter(|token| token.chars().count() >= 3)
        .filter(|token| !natural_query_stop_word(&token.to_lowercase()))
        .collect::<Vec<_>>();
    // The thing asked about is a name, and a question ends on its verb as
    // often as on its subject: "where are plugins loaded?" is about plugins.
    words
        .iter()
        .rfind(|token| !reads_as_a_verb(&token.to_lowercase()))
        .or_else(|| words.last())
        .map(ToString::to_string)
}

/// Whether a word reads as an English verb form rather than as a name. A
/// name may end the same way (`embedded`, `mapping`), so this only breaks a
/// tie between words the question already offered.
fn reads_as_a_verb(word: &str) -> bool {
    (word.ends_with("ed") || word.ends_with("ing")) && word.chars().count() > 4
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

/// Whether the word is what the question is *about* rather than a name
/// in the project: the words the routing rules themselves match on.
pub(crate) fn natural_query_routes_on_this_word(token: &str) -> bool {
    let lower = token.to_lowercase();
    let singular = lower.strip_suffix('s').unwrap_or(&lower);
    matches!(
        singular,
        "api" | "http" | "https" | "ci" | "url" | "uri" | "endpoint" | "interface" | "surface"
    )
}

pub(crate) fn natural_query_stop_word(token: &str) -> bool {
    matches!(
        token,
        // Words that open a question rather than name a thing. Without these
        // `Show me the riskiest code` searched for a symbol called `Show`.
        "show"
            | "find"
            | "list"
            | "give"
            | "tell"
            | "explain"
            | "describe"
            | "покажи"
            | "найди"
            | "перечисли"
            | "объясни"
            | "what"
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
