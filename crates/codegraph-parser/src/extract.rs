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
    let (kind, control_kind) = control_flow_fact(language, node)?;
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
    }
}

pub(crate) fn call_label(language: Language, node: Node<'_>, source: &[u8]) -> Option<String> {
    if language == Language::Bash {
        return node_text(node, source)
            .and_then(|text| text.split_whitespace().next().map(ToString::to_string));
    }

    if let Some(function) = named_child_text(node, "function", source) {
        return Some(clean_call_label(&function));
    }

    if let Some(name) = named_child_text(node, "name", source) {
        return Some(clean_call_label(&name));
    }

    first_identifier(node, source).map(|name| clean_call_label(&name))
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
