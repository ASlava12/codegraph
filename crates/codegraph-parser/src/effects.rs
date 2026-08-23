//! Effect-fact detection: environment reads, config reads, and error
//! constructs, with per-language key and fallback-default extraction.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use crate::*;

pub(crate) fn is_environment_read(language: Language, node: Node<'_>, source: &[u8]) -> bool {
    let text = short_node_text(node, source);
    let call = call_label(language, node, source);

    match language {
        Language::Rust => {
            is_call_node(language, node, source)
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "env::var" | "std::env::var" | "var"))
        }
        // Match only the innermost access form, not any enclosing node whose
        // text merely contains it — `int(os.getenv("K"))` is one env read, not
        // two, and `parseInt(process.env.PORT)` is one, not three.
        Language::Python => match node.kind() {
            "call" => call
                .as_deref()
                .is_some_and(|value| matches!(value, "os.getenv" | "os.environ.get")),
            "subscript" => named_child_text(node, "value", source).as_deref() == Some("os.environ"),
            _ => false,
        },
        Language::JavaScript | Language::TypeScript | Language::Tsx => match node.kind() {
            // `process.env.PORT` — a member access on exactly `process.env`.
            "member_expression" | "subscript_expression" => {
                named_child_text(node, "object", source).as_deref() == Some("process.env")
                    || (text.as_deref() == Some("process.env")
                        && !node.parent().is_some_and(|parent| {
                            // Bare `process.env` only counts when it is not the
                            // object of an enclosing keyed access (that access
                            // is the fact).
                            matches!(parent.kind(), "member_expression" | "subscript_expression")
                        }))
            }
            _ => false,
        },
        Language::Go => {
            is_call_node(language, node, source)
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "os.Getenv" | "Getenv"))
        }
        Language::C | Language::Cpp | Language::Php => {
            is_call_node(language, node, source)
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(simple_name(value), "getenv"))
        }
        // Match the exact access form, like the other languages: a
        // `contains` test also fired on the inner member_expression and on
        // any enclosing node, filing several facts for one physical read.
        Language::Dart => match node.kind() {
            // `String.fromEnvironment('K')` and its bool/int/double siblings.
            "call_expression" => named_child_text(node, "function", source)
                .as_deref()
                .is_some_and(|callee| {
                    matches!(
                        callee,
                        "String.fromEnvironment"
                            | "bool.fromEnvironment"
                            | "int.fromEnvironment"
                            | "double.fromEnvironment"
                    )
                }),
            // `Platform.environment['K']`
            "index_expression" | "null_aware_index_expression" => {
                named_child_text(node, "object", source).as_deref() == Some("Platform.environment")
            }
            _ => false,
        },
        Language::Bash => {
            // Only expansions read a variable: `$VAR` / `${VAR:-default}`.
            // A bare variable_name is also the LHS of an assignment or a
            // `for` loop variable — counting those flooded the graph with
            // phantom env reads for every local.
            node.kind() == "variable_name"
                && node
                    .parent()
                    .is_some_and(|parent| matches!(parent.kind(), "expansion" | "simple_expansion"))
        }
        Language::Ruby => match node.kind() {
            // `ENV['KEY']`
            "element_reference" => {
                named_child_text(node, "object", source).as_deref() == Some("ENV")
            }
            // `ENV.fetch('KEY', 'default')` / `ENV.key?('KEY')`
            "call" => named_child_text(node, "receiver", source).as_deref() == Some("ENV"),
            _ => false,
        },
        Language::Java => {
            node.kind() == "method_invocation"
                && named_child_text(node, "name", source).as_deref() == Some("getenv")
                && named_child_text(node, "object", source).as_deref() == Some("System")
        }
        Language::CSharp => {
            node.kind() == "invocation_expression"
                && call
                    .as_deref()
                    .is_some_and(|value| simple_name(value) == "GetEnvironmentVariable")
        }
        Language::Kotlin => {
            // `System.getenv("K")` — the callee is the first named child.
            node.kind() == "call_expression"
                && node
                    .named_child(0)
                    .and_then(|child| node_text(child, source))
                    .is_some_and(|callee| callee == "System.getenv")
        }
        Language::Swift => {
            // `ProcessInfo.processInfo.environment["K"]` parses as a
            // call_expression whose callee navigation ends in `.environment`.
            node.kind() == "call_expression"
                && node
                    .named_child(0)
                    .and_then(|child| node_text(child, source))
                    .is_some_and(|callee| {
                        callee.starts_with("ProcessInfo") && callee.ends_with(".environment")
                    })
        }
        Language::Scala => {
            node.kind() == "call_expression"
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "sys.env.get" | "System.getenv"))
        }
        Language::Lua => {
            node.kind() == "function_call"
                && named_child_text(node, "name", source).as_deref() == Some("os.getenv")
        }
        Language::Elixir => elixir_call_target(node, source)
            .as_deref()
            .is_some_and(|target| {
                matches!(
                    target,
                    "System.get_env" | "System.fetch_env" | "System.fetch_env!"
                )
            }),
        Language::Zig => {
            node.kind() == "call_expression"
                && call
                    .as_deref()
                    .is_some_and(|value| simple_name(value) == "getenv")
        }
        Language::Haskell => {
            is_call_node(language, node, source)
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "getEnv" | "lookupEnv"))
        }
        Language::OCaml => {
            node.kind() == "application_expression"
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "Sys.getenv" | "Sys.getenv_opt"))
        }
        Language::Erlang => {
            node.kind() == "remote"
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "os:getenv" | "os:env"))
        }
        Language::Nix => {
            is_call_node(language, node, source)
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "builtins.getEnv" | "getEnv"))
        }
        Language::R => {
            node.kind() == "call"
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "Sys.getenv" | "Sys.setenv"))
        }
        Language::Julia => match node.kind() {
            // `ENV["KEY"]`
            "index_expression" => {
                node.named_child(0)
                    .and_then(|child| node_text(child, source))
                    .as_deref()
                    == Some("ENV")
            }
            // `get(ENV, "KEY", "default")`
            "call_expression" => {
                call.as_deref() == Some("get")
                    && node
                        .child_by_field_name("argument_list")
                        .or_else(|| node.named_child(1))
                        .and_then(|args| args.named_child(0))
                        .and_then(|first| node_text(first, source))
                        .as_deref()
                        == Some("ENV")
            }
            _ => false,
        },
    }
}

pub(crate) fn is_config_read(language: Language, node: Node<'_>, source: &[u8]) -> bool {
    if language != Language::Bash && !is_call_node(language, node, source) {
        return false;
    }

    let Some(text) = short_node_text(node, source) else {
        return false;
    };
    if !looks_like_config_text(&text) {
        return false;
    }

    let call = call_label(language, node, source);
    match language {
        Language::Rust => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "read_to_string" | "read" | "open" | "from_reader" | "from_str"
            )
        }),
        Language::Python => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "open" | "load" | "safe_load" | "dotenv_values" | "load_dotenv"
            )
        }),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            call.as_deref()
                .is_some_and(|value| match simple_name(value) {
                    "readFile" | "readFileSync" => true,
                    // `require('./package.json')` reads configuration, but
                    // `require('lodash')` is a module import and is already
                    // indexed as one.
                    "require" => first_string_literal(node, source)
                        .is_some_and(|path| is_data_file_path(&path)),
                    _ => false,
                })
        }
        Language::Go => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "ReadFile" | "Open" | "GetString")),
        Language::C | Language::Cpp => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "fopen" | "open")),
        Language::Php => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "parse_ini_file" | "file_get_contents" | "include" | "require"
            )
        }),
        Language::Dart => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "loadString" | "fromAsset" | "File" | "readAsString" | "readAsStringSync"
            )
        }),
        Language::Bash => {
            node.kind() == "command" && command_text_starts_with(source, node, &["source", "."])
        }
        Language::Ruby => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "read" | "load_file" | "load" | "parse" | "foreach"
            )
        }),
        Language::Java => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "readString" | "readAllBytes" | "readAllLines" | "getProperty" | "load"
            )
        }),
        Language::CSharp => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "ReadAllText" | "ReadAllLines" | "ReadAllBytes" | "OpenText" | "OpenRead"
            )
        }),
        Language::Kotlin => call.as_deref().is_some_and(|value| {
            matches!(
                simple_name(value),
                "readText" | "readLines" | "getProperty" | "load"
            )
        }),
        Language::Scala => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "fromFile" | "getProperty" | "load")),
        // No reliable deterministic config-read shape identified yet.
        Language::Swift | Language::Zig => false,
        Language::Lua => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "dofile" | "loadfile" | "open")),
        Language::Elixir => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "read" | "read!" | "load")),
        Language::Haskell => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "readFile" | "decodeFileStrict")),
        Language::OCaml => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "open_in" | "input_line")),
        Language::Julia => call
            .as_deref()
            .is_some_and(|value| matches!(simple_name(value), "read" | "readlines" | "open")),
        Language::Erlang => call
            .as_deref()
            .is_some_and(|value| matches!(value, "file:read_file" | "file:consult")),
        Language::Nix => call
            .as_deref()
            .is_some_and(|value| matches!(value, "builtins.readFile" | "builtins.fromJSON")),
        Language::R => call.as_deref().is_some_and(|value| {
            matches!(simple_name(value), "read.csv" | "readRDS" | "readLines")
        }),
    }
}

pub(crate) fn is_error_construct(language: Language, node: Node<'_>, source: &[u8]) -> bool {
    match language {
        Language::Rust => {
            node.kind() == "try_expression"
                || (is_call_node(language, node, source)
                    && call_label(language, node, source)
                        .as_deref()
                        .is_some_and(|value| {
                            matches!(simple_name(value), "panic" | "unwrap" | "expect")
                        }))
        }
        Language::Python => node.kind() == "raise_statement",
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            node.kind() == "throw_statement"
        }
        Language::Go => {
            is_call_node(language, node, source)
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| {
                        // Go has no `raise`/`throw`: outside of `panic`, an
                        // error is constructed and returned, so construction is
                        // where the error flow starts. Match the qualified name
                        // — an unrelated `pool.New(...)` is not an error.
                        simple_name(value) == "panic"
                            || matches!(
                                value,
                                "errors.New"
                                    | "errors.Join"
                                    | "errors.Wrap"
                                    | "errors.Wrapf"
                                    | "fmt.Errorf"
                            )
                    })
        }
        Language::C => {
            is_call_node(language, node, source)
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| matches!(simple_name(value), "abort" | "exit"))
        }
        Language::Cpp => {
            node.kind() == "throw_statement"
                || (is_call_node(language, node, source)
                    && call_label(language, node, source)
                        .as_deref()
                        .is_some_and(|value| matches!(simple_name(value), "abort" | "exit")))
        }
        Language::Php => node.kind() == "throw_expression",
        Language::Dart => {
            matches!(node.kind(), "throw_expression" | "rethrow_statement")
                || (is_call_node(language, node, source)
                    && call_label(language, node, source)
                        .as_deref()
                        .is_some_and(|value| matches!(simple_name(value), "throw")))
        }
        Language::Bash => {
            node.kind() == "command" && command_text_starts_with(source, node, &["exit"])
        }
        Language::Ruby => {
            node.kind() == "call"
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| simple_name(value) == "raise")
        }
        Language::Java => node.kind() == "throw_statement",
        Language::CSharp => matches!(node.kind(), "throw_statement" | "throw_expression"),
        Language::Kotlin | Language::Scala => node.kind() == "throw_expression",
        Language::Lua => {
            node.kind() == "function_call"
                && named_child_text(node, "name", source)
                    .as_deref()
                    .is_some_and(|name| matches!(name, "error" | "assert"))
        }
        Language::Elixir => elixir_call_target(node, source)
            .as_deref()
            .is_some_and(|target| matches!(target, "raise" | "throw")),
        Language::Zig => {
            node.kind() == "builtin_function"
                && node
                    .named_child(0)
                    .and_then(|child| node_text(child, source))
                    .as_deref()
                    == Some("@panic")
        }
        Language::Haskell => {
            is_call_node(language, node, source)
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| {
                        matches!(
                            value,
                            "error" | "errorWithoutStackTrace" | "throwIO" | "throw"
                        )
                    })
        }
        Language::OCaml => {
            node.kind() == "application_expression"
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| {
                        matches!(simple_name(value), "failwith" | "raise" | "invalid_arg")
                    })
        }
        Language::Julia => {
            node.kind() == "call_expression"
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| matches!(value, "throw" | "error" | "rethrow"))
        }
        Language::Erlang => {
            is_call_node(language, node, source)
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| matches!(value, "throw" | "error" | "exit"))
        }
        Language::Nix => {
            is_call_node(language, node, source)
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| {
                        matches!(
                            value,
                            "throw" | "abort" | "builtins.throw" | "builtins.abort"
                        )
                    })
        }
        Language::R => {
            node.kind() == "call"
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| matches!(value, "stop" | "stopifnot" | "warning"))
        }
        Language::Swift => {
            is_call_node(language, node, source)
                && call_label(language, node, source)
                    .as_deref()
                    .is_some_and(|value| {
                        matches!(
                            value,
                            "fatalError" | "preconditionFailure" | "assertionFailure"
                        )
                    })
        }
    }
}

/// Does this path name a data/config file rather than a code module?
fn is_data_file_path(path: &str) -> bool {
    let path = path.trim().trim_end_matches(['"', '\'']);
    let extension = path.rsplit('.').next().unwrap_or_default();
    matches!(
        extension,
        "json" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" | "env" | "properties" | "xml"
    )
}

pub(crate) fn effect_label(
    language: Language,
    kind: ParsedItemKind,
    node: Node<'_>,
    source: &[u8],
) -> Option<String> {
    if kind == ParsedItemKind::EnvironmentRead
        && matches!(
            language,
            Language::JavaScript | Language::TypeScript | Language::Tsx
        )
        && let Some(key) = javascript_env_key(node, source)
    {
        return Some(truncate_label(key, 120));
    }

    if kind == ParsedItemKind::EnvironmentRead
        && language == Language::Dart
        && let Some(key) = dart_env_key(node, source)
    {
        return Some(truncate_label(key, 120));
    }

    first_string_literal(node, source)
        .or_else(|| node_text(node, source).map(compact_label))
        .map(|value| truncate_label(value, 120))
}

pub(crate) fn effect_metadata(
    language: Language,
    kind: ParsedItemKind,
    node: Node<'_>,
    source: &[u8],
) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    if kind != ParsedItemKind::EnvironmentRead {
        return metadata;
    }

    let default_value = match language {
        Language::Rust => rust_env_default_value(node, source),
        Language::Python => python_env_default_value(node, source),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            javascript_env_default_value(node, source)
        }
        Language::Go => go_env_default_value(node, source),
        Language::C | Language::Cpp | Language::Php => getenv_default_value(node, source),
        Language::Dart => dart_env_default_value(node, source),
        Language::Bash => bash_env_default_value(node, source),
        Language::Ruby => ruby_env_default_value(node, source),
        Language::Java
        | Language::CSharp
        | Language::Kotlin
        | Language::Swift
        | Language::Scala
        | Language::Lua
        | Language::Elixir
        | Language::Zig
        | Language::Haskell
        | Language::OCaml
        | Language::Julia
        | Language::Erlang
        | Language::Nix
        | Language::R => None,
    };

    if let Some(default_value) = default_value {
        metadata.insert(
            "default_value".to_string(),
            truncate_label(default_value, 120),
        );
    }
    metadata
}

pub(crate) fn python_env_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = short_node_text(node, source)?;
    if !(text.contains("os.getenv") || text.contains("os.environ.get")) {
        return None;
    }
    all_string_literals(node, source).into_iter().nth(1)
}

pub(crate) fn rust_env_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    short_ancestor_text(node, source, |text| {
        (text.contains("env::var") || text.contains("std::env::var"))
            && (text.contains("unwrap_or") || text.contains("unwrap_or_else"))
    })
    .and_then(|expression| quoted_string_values(&expression).into_iter().nth(1))
    .or_else(|| {
        let line = source_line_text(node, source).filter(|text| {
            (text.contains("env::var") || text.contains("std::env::var"))
                && (text.contains("unwrap_or") || text.contains("unwrap_or_else"))
        })?;
        quoted_string_values(&line).into_iter().nth(1)
    })
}

pub(crate) fn javascript_env_key(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = short_node_text(node, source)?;
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if let Some(after_env) = compact.strip_prefix("process.env.") {
        return after_env
            .split(|character: char| !is_identifier_part(character))
            .next()
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
    }
    if compact.starts_with("process.env[") {
        return first_string_literal(node, source);
    }
    None
}

pub(crate) fn javascript_env_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let expression = short_ancestor_text(node, source, |text| {
        text.contains("process.env") && (text.contains("||") || text.contains("??"))
    })?;
    let literals = quoted_string_values(&expression);
    if expression.contains("process.env[") {
        literals.into_iter().nth(1)
    } else {
        literals.into_iter().next()
    }
}

pub(crate) fn dart_env_key(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = short_node_text(node, source)?;
    if text.contains("Platform.environment") {
        return first_string_literal(node, source);
    }
    if text.contains(".fromEnvironment") {
        return all_string_literals(node, source).into_iter().next();
    }
    None
}

pub(crate) fn dart_env_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = short_node_text(node, source)?;
    if text.contains(".fromEnvironment") {
        let literals = all_string_literals(node, source);
        if text.contains("defaultValue")
            && let Some(value) = literals.get(1)
        {
            return Some(value.clone());
        }
        if literals.len() > 1 && text.contains(',') {
            return literals.get(1).cloned();
        }
        return None;
    }
    let expression = short_ancestor_text(node, source, |candidate| {
        candidate.contains("Platform.environment")
            && (candidate.contains("??") || candidate.contains("?:"))
    })?;
    quoted_string_values(&expression).into_iter().nth(1)
}

pub(crate) fn ruby_env_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = short_node_text(node, source)?;
    if text.contains("ENV.fetch") {
        return all_string_literals(node, source).into_iter().nth(1);
    }
    let expression = short_ancestor_text(node, source, |candidate| {
        candidate.contains("ENV[") && candidate.contains("||")
    })?;
    quoted_string_values(&expression).into_iter().nth(1)
}

pub(crate) fn getenv_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let expression = short_ancestor_text(node, source, |text| {
        text.contains("getenv")
            && (text.contains("?:") || text.contains("??") || text.contains("||"))
    })?;
    quoted_string_values(&expression).into_iter().nth(1)
}

pub(crate) fn go_env_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let expression = short_ancestor_text(node, source, |text| {
        text.contains("os.Getenv") && text.contains("cmp.Or")
    })?;
    quoted_string_values(&expression).into_iter().nth(1)
}

pub(crate) fn bash_env_default_value(node: Node<'_>, source: &[u8]) -> Option<String> {
    let key = node_text(node, source)?;
    let expression = short_ancestor_text(node, source, |text| {
        text.contains("${") && text.contains(&key) && (text.contains(":-") || text.contains("-"))
    })?;
    bash_parameter_default(&expression, &key)
}

pub(crate) fn bash_parameter_default(expression: &str, key: &str) -> Option<String> {
    let marker = format!("${{{key}");
    let start = expression.find(&marker)?;
    let rest = &expression[start + marker.len()..];
    let rest = rest.strip_prefix(":-").or_else(|| rest.strip_prefix('-'))?;
    let end = rest.find('}')?;
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(strip_quotes(value.to_string()))
    }
}

/// Drop Bash "environment reads" that are really reads of a variable the same
/// script sets. `$VAR` alone cannot tell the two apart, so a script like
/// terraform's release helpers flooded the graph with keys such as `arch`,
/// `os`, and `version` — locals, not process environment.
///
/// A name counts as local when the file assigns it (`name=…`, `local name=…`,
/// a `for` variable, a `read` target). The exception is the pass-through
/// idiom `PORT="${PORT:-8080}"`, where the assignment's own value expands the
/// name: that read does come from the environment, so the name stays.
///
/// This is deliberately position-blind — a read before the assignment that
/// sets it (`echo "$X"; X=1`) is dropped too. Telling those apart needs flow
/// analysis, and the scanner stays syntactic.
pub(crate) fn drop_bash_local_variable_reads(
    items: &mut Vec<ParsedItem>,
    root: Node<'_>,
    source: &[u8],
) {
    let mut assigned: BTreeSet<String> = BTreeSet::new();
    let mut environment_backed: BTreeSet<String> = BTreeSet::new();

    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "variable_assignment" => {
                if let Some(name) = node
                    .child_by_field_name("name")
                    .and_then(|name| node_text(name, source))
                    .map(|name| bash_variable_base_name(&name).to_string())
                    .filter(|name| !name.is_empty())
                {
                    if let Some(value) = node.child_by_field_name("value")
                        && subtree_expands_variable(value, source, &name)
                    {
                        environment_backed.insert(name.clone());
                    }
                    assigned.insert(name);
                }
            }
            "for_statement" => {
                if let Some(name) = node
                    .child_by_field_name("variable")
                    .and_then(|name| node_text(name, source))
                    .filter(|name| !name.is_empty())
                {
                    assigned.insert(name);
                }
            }
            // `read answer` names its target as a plain word, so the variable
            // never appears as a `variable_name` node.
            "command"
                if node
                    .child_by_field_name("name")
                    .and_then(|name| node_text(name, source))
                    .as_deref()
                    == Some("read") =>
            {
                let mut cursor = node.walk();
                for argument in node.children_by_field_name("argument", &mut cursor) {
                    if let Some(name) = node_text(argument, source)
                        && !name.is_empty()
                        && !name.starts_with('-')
                        && name
                            .chars()
                            .all(|character| character.is_alphanumeric() || character == '_')
                    {
                        assigned.insert(name);
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }

    items.retain(|item| {
        item.kind != ParsedItemKind::EnvironmentRead
            || !assigned.contains(&item.label)
            || environment_backed.contains(&item.label)
    });
}

/// The plain name behind an assignment target, dropping an array subscript
/// (`cache[key]=…` sets `cache`).
fn bash_variable_base_name(name: &str) -> &str {
    name.split(['[', '=']).next().unwrap_or(name).trim()
}

/// Does this subtree expand `name` (`${name:-x}` / `$name`)?
fn subtree_expands_variable(node: Node<'_>, source: &[u8], name: &str) -> bool {
    let mut stack = vec![node];
    while let Some(current) = stack.pop() {
        if matches!(current.kind(), "expansion" | "simple_expansion") {
            let mut cursor = current.walk();
            for child in current.named_children(&mut cursor) {
                if child.kind() == "variable_name"
                    && node_text(child, source).as_deref() == Some(name)
                {
                    return true;
                }
            }
        }
        let mut cursor = current.walk();
        stack.extend(current.named_children(&mut cursor));
    }
    false
}
