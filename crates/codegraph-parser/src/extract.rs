//! Tree walking and fact classification: parse a source file and turn
//! syntax nodes into structural, call, entrypoint, and control-flow items.

use std::collections::BTreeMap;
use std::path::Path;

use codegraph_core::SourceSpan;
use tree_sitter::{Node, Parser};

use crate::*;

pub fn parse_source(
    path: impl AsRef<Path>,
    source: &[u8],
    language: Language,
) -> Result<ParsedFile, ParseError> {
    let path = path.as_ref();
    let source_text = std::str::from_utf8(source).map_err(|_| ParseError::InvalidUtf8)?;
    let mut parser = Parser::new();
    parser
        .set_language(&language.tree_sitter_language())
        .map_err(|error| ParseError::LanguageSetup {
            language,
            message: error.to_string(),
        })?;

    let tree = parser
        .parse(source_text, None)
        .ok_or(ParseError::ParseFailed { language })?;
    let root = tree.root_node();
    let mut items = Vec::new();
    collect_items(
        language,
        root,
        source_text.as_bytes(),
        &path.to_string_lossy(),
        None,
        &mut items,
        0,
    );
    dedupe_items(&mut items);

    Ok(ParsedFile {
        language,
        items,
        has_error_nodes: root.has_error(),
    })
}

/// Deepest syntax-tree level walked. Real source nests far below this; the cap
/// exists only to keep a pathological (minified/generated) file from
/// overflowing the stack — hitting it degrades gracefully (deeper facts are
/// skipped) instead of aborting the process.
const MAX_TREE_DEPTH: usize = 512;

pub(crate) fn collect_items(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    current_function: Option<String>,
    items: &mut Vec<ParsedItem>,
    depth: usize,
) {
    if depth >= MAX_TREE_DEPTH {
        return;
    }
    if let Some(effect) = classify_effect(language, node, source, path, current_function.as_deref())
    {
        items.push(effect);
    }
    if let Some(control_flow) =
        classify_control_flow(language, node, source, path, current_function.as_deref())
    {
        items.push(control_flow);
    }

    if let Some(function_name) = current_function.as_deref()
        && let Some(call) = classify_call(language, node, source, path, function_name)
    {
        items.push(call);
    }

    let mut next_function = current_function;
    if let Some(item) = classify_node(language, node, source, path) {
        if matches!(
            item.kind,
            ParsedItemKind::Function | ParsedItemKind::Entrypoint
        ) {
            next_function = Some(item.label.clone());
        }
        items.push(item);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(
            language,
            child,
            source,
            path,
            next_function.clone(),
            items,
            depth + 1,
        );
    }
}

pub(crate) fn classify_node(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
) -> Option<ParsedItem> {
    let kind = node.kind();
    let item_kind = match language {
        Language::Rust => match kind {
            "function_item" => ParsedItemKind::Function,
            "struct_item" | "enum_item" | "trait_item" | "type_item" | "union_item" => {
                ParsedItemKind::Type
            }
            "mod_item" => ParsedItemKind::Module,
            "use_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Python => match kind {
            "function_definition" => ParsedItemKind::Function,
            "class_definition" => ParsedItemKind::Type,
            "import_statement" | "import_from_statement" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::JavaScript | Language::TypeScript | Language::Tsx => match kind {
            "function_declaration" | "method_definition" | "generator_function_declaration" => {
                ParsedItemKind::Function
            }
            "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration" => ParsedItemKind::Type,
            "import_statement" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Go => match kind {
            "function_declaration" | "method_declaration" => ParsedItemKind::Function,
            "type_declaration" => ParsedItemKind::Type,
            "import_spec" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::C | Language::Cpp => match kind {
            "function_definition" => ParsedItemKind::Function,
            "struct_specifier" | "union_specifier" | "enum_specifier" | "class_specifier"
            | "type_definition" => ParsedItemKind::Type,
            "namespace_definition" => ParsedItemKind::Module,
            "preproc_include" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Php => match kind {
            "function_definition" | "method_declaration" => ParsedItemKind::Function,
            "class_declaration"
            | "interface_declaration"
            | "trait_declaration"
            | "enum_declaration" => ParsedItemKind::Type,
            "namespace_definition" => ParsedItemKind::Module,
            "require_expression"
            | "include_expression"
            | "include_once_expression"
            | "require_once_expression"
            | "namespace_use_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Dart => match kind {
            "function_declaration"
            | "method_declaration"
            | "constructor_signature"
            | "factory_constructor_signature" => ParsedItemKind::Function,
            "class_declaration"
            | "mixin_declaration"
            | "extension_declaration"
            | "extension_type_declaration"
            | "enum_declaration"
            | "type_alias" => ParsedItemKind::Type,
            "library_name" => ParsedItemKind::Module,
            "import_or_export" | "part_directive" | "part_of_directive" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Bash => match kind {
            "function_definition" => ParsedItemKind::Function,
            "command" if command_text_starts_with(source, node, &["source", "."]) => {
                ParsedItemKind::Import
            }
            _ => return None,
        },
        Language::Ruby => match kind {
            "method" | "singleton_method" => ParsedItemKind::Function,
            "class" => ParsedItemKind::Type,
            "module" => ParsedItemKind::Module,
            "call" if ruby_require_call(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Java => match kind {
            "method_declaration" | "constructor_declaration" => ParsedItemKind::Function,
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration" => ParsedItemKind::Type,
            "import_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::CSharp => match kind {
            "method_declaration" | "constructor_declaration" | "local_function_statement" => {
                ParsedItemKind::Function
            }
            "class_declaration"
            | "interface_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "record_declaration" => ParsedItemKind::Type,
            "namespace_declaration" | "file_scoped_namespace_declaration" => ParsedItemKind::Module,
            "using_directive" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Kotlin => match kind {
            "function_declaration" => ParsedItemKind::Function,
            "class_declaration" | "object_declaration" => ParsedItemKind::Type,
            "import" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Swift => match kind {
            "function_declaration" | "protocol_function_declaration" | "init_declaration" => {
                ParsedItemKind::Function
            }
            // The grammar folds struct/enum into class_declaration.
            "class_declaration" | "protocol_declaration" => ParsedItemKind::Type,
            "import_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Scala => match kind {
            "function_definition" | "function_declaration" => ParsedItemKind::Function,
            "class_definition" | "object_definition" | "trait_definition" | "enum_definition" => {
                ParsedItemKind::Type
            }
            "import_declaration" => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Lua => match kind {
            "function_declaration" => ParsedItemKind::Function,
            "function_call" if lua_require_call(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
        // Elixir syntax is uniform: everything is a `call` whose target picks
        // the special form (defmodule/def/import/...); see elixir_call_target.
        Language::Elixir => match elixir_call_target(node, source).as_deref() {
            Some("defmodule") => ParsedItemKind::Module,
            Some("def" | "defp" | "defmacro" | "defmacrop" | "defdelegate") => {
                ParsedItemKind::Function
            }
            Some("defprotocol" | "defimpl" | "defstruct") => ParsedItemKind::Type,
            Some("import" | "require" | "use" | "alias") => ParsedItemKind::Import,
            _ => return None,
        },
        Language::Zig => match kind {
            "function_declaration" => ParsedItemKind::Function,
            "builtin_function" if zig_import_builtin(node, source) => ParsedItemKind::Import,
            _ => return None,
        },
    };

    let label = item_label(language, item_kind, node, source)?;
    let item_kind = if item_kind == ParsedItemKind::Function && is_entrypoint(language, &label) {
        ParsedItemKind::Entrypoint
    } else {
        item_kind
    };

    Some(ParsedItem {
        kind: item_kind,
        label,
        span: span_for(path, node),
        parent: None,
        metadata: BTreeMap::new(),
    })
}

pub(crate) fn classify_call(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    function_name: &str,
) -> Option<ParsedItem> {
    if !is_call_node(language, node, source) {
        return None;
    }

    let label = call_label(language, node, source)?;
    if label.is_empty() {
        return None;
    }

    Some(ParsedItem {
        kind: ParsedItemKind::Call,
        label,
        span: span_for(path, node),
        parent: Some(function_name.to_string()),
        metadata: BTreeMap::new(),
    })
}

pub(crate) fn classify_effect(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    function_name: Option<&str>,
) -> Option<ParsedItem> {
    let (kind, label, metadata) = if is_environment_read(language, node, source) {
        effect_label(language, ParsedItemKind::EnvironmentRead, node, source).map(|label| {
            (
                ParsedItemKind::EnvironmentRead,
                label,
                effect_metadata(language, ParsedItemKind::EnvironmentRead, node, source),
            )
        })
    } else if is_config_read(language, node, source) {
        effect_label(language, ParsedItemKind::ConfigRead, node, source).map(|label| {
            (
                ParsedItemKind::ConfigRead,
                label,
                effect_metadata(language, ParsedItemKind::ConfigRead, node, source),
            )
        })
    } else if is_error_construct(language, node, source) {
        effect_label(language, ParsedItemKind::Error, node, source).map(|label| {
            (
                ParsedItemKind::Error,
                label,
                effect_metadata(language, ParsedItemKind::Error, node, source),
            )
        })
    } else {
        None
    }?;

    Some(ParsedItem {
        kind,
        label,
        span: span_for(path, node),
        parent: function_name.map(ToString::to_string),
        metadata,
    })
}

pub(crate) fn classify_control_flow(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    function_name: Option<&str>,
) -> Option<ParsedItem> {
    let (kind, control_kind) = if language == Language::Elixir {
        elixir_control_flow_fact(node, source)?
    } else {
        control_flow_fact(language, node)?
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("control_kind".to_string(), control_kind.to_string());
    metadata.insert("syntax_node".to_string(), node.kind().to_string());
    if let Some(snippet) = short_node_text(node, source) {
        metadata.insert(
            "snippet".to_string(),
            truncate_label(compact_label(snippet), 160),
        );
    }

    Some(ParsedItem {
        kind,
        label: format!("{}: {}", parsed_item_kind_label(kind), control_kind),
        span: span_for(path, node),
        parent: function_name.map(ToString::to_string),
        metadata,
    })
}

pub(crate) fn control_flow_fact(
    language: Language,
    node: Node<'_>,
) -> Option<(ParsedItemKind, &'static str)> {
    let kind = node.kind();
    match language {
        Language::Rust => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "match_expression" => Some((ParsedItemKind::Branch, "match")),
            "try_expression" => Some((ParsedItemKind::Branch, "try")),
            "for_expression" => Some((ParsedItemKind::Loop, "for")),
            "while_expression" => Some((ParsedItemKind::Loop, "while")),
            "loop_expression" => Some((ParsedItemKind::Loop, "loop")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "async_block" => Some((ParsedItemKind::Async, "async")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Python => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "match_statement" => Some((ParsedItemKind::Branch, "match")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "except_clause" => Some((ParsedItemKind::Branch, "except")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "await" => Some((ParsedItemKind::Async, "await")),
            "async_function_definition" => Some((ParsedItemKind::Async, "function")),
            "async_for_statement" => Some((ParsedItemKind::Async, "for")),
            "async_with_statement" => Some((ParsedItemKind::Async, "with")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::JavaScript | Language::TypeScript | Language::Tsx => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "switch_case" => Some((ParsedItemKind::Branch, "case")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "ternary_expression" => Some((ParsedItemKind::Branch, "ternary")),
            "for_statement" | "for_in_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Go => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" | "expression_switch_statement" | "type_switch_statement" => {
                Some((ParsedItemKind::Branch, "switch"))
            }
            "select_statement" => Some((ParsedItemKind::Branch, "select")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "go_statement" => Some((ParsedItemKind::Async, "go")),
            "defer_statement" => Some((ParsedItemKind::Async, "defer")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::C | Language::Cpp => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "case_statement" => Some((ParsedItemKind::Branch, "case")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Php => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "match_expression" => Some((ParsedItemKind::Branch, "match")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "foreach_statement" => Some((ParsedItemKind::Loop, "foreach")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Dart => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" | "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" | "for_element" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Bash => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "case_statement" => Some((ParsedItemKind::Branch, "case")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "until_statement" => Some((ParsedItemKind::Loop, "until")),
            _ => None,
        },
        Language::Ruby => match kind {
            "if" | "unless" => Some((ParsedItemKind::Branch, "if")),
            "case" => Some((ParsedItemKind::Branch, "case")),
            "rescue" => Some((ParsedItemKind::Branch, "rescue")),
            "for" => Some((ParsedItemKind::Loop, "for")),
            "while" => Some((ParsedItemKind::Loop, "while")),
            "until" => Some((ParsedItemKind::Loop, "until")),
            "return" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Java => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" | "enhanced_for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::CSharp => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "switch_statement" | "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "try_statement" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" | "foreach_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_statement" => Some((ParsedItemKind::Loop, "do")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Kotlin => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "when_expression" => Some((ParsedItemKind::Branch, "when")),
            "try_expression" => Some((ParsedItemKind::Branch, "try")),
            "catch_block" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "do_while_statement" => Some((ParsedItemKind::Loop, "do")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Swift => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "guard_statement" => Some((ParsedItemKind::Branch, "guard")),
            "switch_statement" => Some((ParsedItemKind::Branch, "switch")),
            "do_statement" => Some((ParsedItemKind::Branch, "do")),
            "catch_block" => Some((ParsedItemKind::Branch, "catch")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "repeat_while_statement" => Some((ParsedItemKind::Loop, "repeat")),
            "await_expression" => Some((ParsedItemKind::Async, "await")),
            "control_transfer_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Scala => match kind {
            "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "match_expression" => Some((ParsedItemKind::Branch, "match")),
            "try_expression" => Some((ParsedItemKind::Branch, "try")),
            "catch_clause" => Some((ParsedItemKind::Branch, "catch")),
            "for_expression" => Some((ParsedItemKind::Loop, "for")),
            "while_expression" => Some((ParsedItemKind::Loop, "while")),
            "do_while_expression" => Some((ParsedItemKind::Loop, "do")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Lua => match kind {
            "if_statement" => Some((ParsedItemKind::Branch, "if")),
            "for_statement" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" => Some((ParsedItemKind::Loop, "while")),
            "repeat_statement" => Some((ParsedItemKind::Loop, "repeat")),
            "return_statement" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
        Language::Elixir => None, // handled below via elixir_control_flow
        Language::Zig => match kind {
            "if_statement" | "if_expression" => Some((ParsedItemKind::Branch, "if")),
            "switch_expression" => Some((ParsedItemKind::Branch, "switch")),
            "for_statement" | "for_expression" => Some((ParsedItemKind::Loop, "for")),
            "while_statement" | "while_expression" => Some((ParsedItemKind::Loop, "while")),
            "defer_statement" => Some((ParsedItemKind::Async, "defer")),
            "return_expression" => Some((ParsedItemKind::Return, "return")),
            _ => None,
        },
    }
}

pub(crate) fn is_call_node(language: Language, node: Node<'_>, source: &[u8]) -> bool {
    match language {
        Language::Rust => matches!(node.kind(), "call_expression" | "macro_invocation"),
        Language::Python => node.kind() == "call",
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            matches!(node.kind(), "call_expression" | "new_expression")
        }
        Language::Go | Language::C | Language::Cpp => node.kind() == "call_expression",
        Language::Php => matches!(
            node.kind(),
            "function_call_expression" | "scoped_call_expression" | "member_call_expression"
        ),
        Language::Dart => matches!(node.kind(), "call_expression" | "constructor_invocation"),
        Language::Bash => {
            node.kind() == "command" && !command_text_starts_with(source, node, &["source", "."])
        }
        Language::Ruby => node.kind() == "call" && !ruby_require_call(node, source),
        Language::Java => {
            matches!(
                node.kind(),
                "method_invocation" | "object_creation_expression"
            )
        }
        Language::CSharp => {
            matches!(
                node.kind(),
                "invocation_expression" | "object_creation_expression"
            )
        }
        Language::Kotlin | Language::Swift => node.kind() == "call_expression",
        Language::Scala => matches!(node.kind(), "call_expression" | "instance_expression"),
        Language::Lua => node.kind() == "function_call" && !lua_require_call(node, source),
        Language::Elixir => {
            node.kind() == "call"
                && elixir_call_target(node, source)
                    .as_deref()
                    .is_some_and(|target| !ELIXIR_SPECIAL_FORMS.contains(&target))
        }
        Language::Zig => node.kind() == "call_expression",
    }
}

/// Elixir special forms parse as ordinary `call` nodes; these targets are
/// definitions, imports, or control flow rather than function calls.
pub(crate) const ELIXIR_SPECIAL_FORMS: &[&str] = &[
    "def",
    "defp",
    "defmodule",
    "defmacro",
    "defmacrop",
    "defimpl",
    "defprotocol",
    "defstruct",
    "defdelegate",
    "defguard",
    "defguardp",
    "import",
    "require",
    "use",
    "alias",
    "if",
    "unless",
    "case",
    "cond",
    "for",
    "with",
    "try",
    "receive",
    "raise",
    "throw",
    "quote",
    "unquote",
];

/// The target text of an Elixir `call` node (an identifier or dotted access).
pub(crate) fn elixir_call_target(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() != "call" {
        return None;
    }
    named_child_text(node, "target", source)
}

/// A Lua `require("module")` call: an import fact, not a call fact.
pub(crate) fn lua_require_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "function_call"
        && named_child_text(node, "name", source).as_deref() == Some("require")
}

/// Zig `@import("std")` builtin.
pub(crate) fn zig_import_builtin(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "builtin_function"
        && node
            .named_child(0)
            .and_then(|child| node_text(child, source))
            .as_deref()
            == Some("@import")
}

/// A Ruby `require`/`require_relative` call: an import fact, not a call fact.
pub(crate) fn ruby_require_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == "call"
        && named_child_text(node, "method", source)
            .as_deref()
            .is_some_and(|method| matches!(method, "require" | "require_relative"))
}

pub(crate) fn call_label(language: Language, node: Node<'_>, source: &[u8]) -> Option<String> {
    if language == Language::Bash {
        return node_text(node, source)
            .and_then(|text| text.split_whitespace().next().map(ToString::to_string));
    }

    if language == Language::Ruby
        && let Some(method) = named_child_text(node, "method", source)
    {
        return Some(clean_call_label(&method));
    }

    // These grammars expose no named callee field; the callee is the first
    // named child (an identifier or dotted navigation), so the label is its
    // trailing path segment: `System.getenv(..)` -> `getenv`.
    if matches!(language, Language::Kotlin | Language::Swift)
        && let Some(callee) = node
            .named_child(0)
            .and_then(|child| node_text(child, source))
    {
        return Some(clean_call_label(simple_name(&callee)));
    }

    if language == Language::Elixir
        && let Some(target) = named_child_text(node, "target", source)
    {
        return Some(clean_call_label(&target));
    }

    if let Some(function) = named_child_text(node, "function", source) {
        return Some(clean_call_label(&function));
    }

    if let Some(name) = named_child_text(node, "name", source) {
        return Some(clean_call_label(&name));
    }

    first_identifier(node, source).map(|name| clean_call_label(&name))
}

/// Control-flow facts for Elixir, whose if/case/cond/for/with parse as calls.
pub(crate) fn elixir_control_flow_fact(
    node: Node<'_>,
    source: &[u8],
) -> Option<(ParsedItemKind, &'static str)> {
    match elixir_call_target(node, source).as_deref() {
        Some("if" | "unless") => Some((ParsedItemKind::Branch, "if")),
        Some("case") => Some((ParsedItemKind::Branch, "case")),
        Some("cond") => Some((ParsedItemKind::Branch, "cond")),
        Some("with") => Some((ParsedItemKind::Branch, "with")),
        Some("try" | "receive") => Some((ParsedItemKind::Branch, "try")),
        Some("for") => Some((ParsedItemKind::Loop, "for")),
        _ => None,
    }
}

/// Labels for Elixir definition calls: the module alias or the function-head
/// target inside the arguments (`defmodule Billing.Invoice`, `def total(..)`).
pub(crate) fn elixir_item_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let arguments = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "arguments")?;
    let first = arguments.named_child(0)?;
    match first.kind() {
        "alias" | "identifier" => node_text(first, source),
        "call" => named_child_text(first, "target", source),
        _ => None,
    }
}

pub(crate) fn item_label(
    language: Language,
    kind: ParsedItemKind,
    node: Node<'_>,
    source: &[u8],
) -> Option<String> {
    if kind == ParsedItemKind::Import {
        return node_text(node, source).map(compact_label);
    }

    if let Some(name) = named_child_text(node, "name", source) {
        return Some(name);
    }

    match language {
        Language::Elixir => elixir_item_label(node, source),
        Language::C | Language::Cpp => first_identifier_in_field(node, "declarator", source)
            .or_else(|| first_identifier(node, source)),
        Language::Go if kind == ParsedItemKind::Function => {
            named_child_text(node, "name", source).or_else(|| first_identifier(node, source))
        }
        Language::Bash => first_identifier(node, source),
        _ => first_identifier(node, source),
    }
}

pub(crate) fn is_entrypoint(language: Language, label: &str) -> bool {
    match language {
        Language::Rust | Language::Go | Language::C | Language::Cpp => label == "main",
        Language::Python => label == "main" || label == "__main__",
        Language::JavaScript | Language::TypeScript | Language::Tsx | Language::Php => {
            label.eq_ignore_ascii_case("main")
        }
        Language::Dart => label == "main",
        Language::Bash => label == "main",
        Language::Ruby | Language::Java => label == "main",
        Language::CSharp => label.eq_ignore_ascii_case("main"),
        Language::Kotlin | Language::Swift | Language::Scala => label == "main",
        Language::Lua | Language::Elixir | Language::Zig => label == "main",
    }
}

pub(crate) fn span_for(path: &str, node: Node<'_>) -> SourceSpan {
    let start = node.start_position();
    let end = node.end_position();
    SourceSpan {
        path: path.to_string(),
        start_line: start.row as u32 + 1,
        start_column: start.column as u32 + 1,
        end_line: end.row as u32 + 1,
        end_column: end.column as u32 + 1,
    }
}

pub(crate) fn dedupe_items(items: &mut Vec<ParsedItem>) {
    items.sort_by(|left, right| {
        (
            left.span.start_line,
            left.span.start_column,
            left.kind as u8,
            &left.label,
        )
            .cmp(&(
                right.span.start_line,
                right.span.start_column,
                right.kind as u8,
                &right.label,
            ))
    });
    items.dedup_by(|left, right| {
        left.kind == right.kind
            && left.label == right.label
            && left.span.start_line == right.span.start_line
            && left.span.start_column == right.span.start_column
    });
}
