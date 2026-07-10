//! Syntax-node text helpers: identifiers, string literals, labels, and
//! quoting/truncation utilities shared by extraction and effect detection.

use tree_sitter::{Node, TreeCursor};

pub(crate) fn short_ancestor_text(
    node: Node<'_>,
    source: &[u8],
    predicate: impl Fn(&str) -> bool,
) -> Option<String> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if let Some(text) = short_node_text(candidate, source)
            && predicate(&text)
        {
            return Some(text);
        }
        current = candidate.parent();
    }
    None
}

pub(crate) fn named_child_text(node: Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| node_text(child, source))
}

pub(crate) fn first_identifier_in_field(
    node: Node<'_>,
    field: &str,
    source: &[u8],
) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| first_identifier(child, source))
}

pub(crate) fn first_identifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "identifier"
            | "field_identifier"
            | "type_identifier"
            | "namespace_identifier"
            | "property_identifier"
            | "variable_name"
            | "identifier_dollar_escaped"
            | "name"
    ) {
        return node_text(node, source);
    }

    let mut cursor: TreeCursor<'_> = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(value) = first_identifier(child, source) {
            return Some(value);
        }
    }
    None
}

pub(crate) fn first_string_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind().contains("string") || matches!(node.kind(), "raw_string_literal") {
        return node_text(node, source).map(strip_quotes);
    }

    let mut cursor: TreeCursor<'_> = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(value) = first_string_literal(child, source) {
            return Some(value);
        }
    }
    None
}

pub(crate) fn all_string_literals(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut values = Vec::new();
    collect_string_literals(node, source, &mut values);
    values
}

pub(crate) fn collect_string_literals(node: Node<'_>, source: &[u8], values: &mut Vec<String>) {
    if node.kind().contains("string") || matches!(node.kind(), "raw_string_literal") {
        if let Some(value) = node_text(node, source).map(strip_quotes) {
            values.push(value);
        }
        return;
    }

    let mut cursor: TreeCursor<'_> = node.walk();
    for child in node.children(&mut cursor) {
        collect_string_literals(child, source, values);
    }
}

pub(crate) fn quoted_string_values(value: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut quote = None;
    let mut current = String::new();
    let mut escaped = false;

    for character in value.chars() {
        match quote {
            Some(_) if escaped => {
                current.push(character);
                escaped = false;
            }
            Some(_) if character == '\\' => {
                escaped = true;
            }
            Some(current_quote) if character == current_quote => {
                values.push(std::mem::take(&mut current));
                quote = None;
            }
            Some(_) => current.push(character),
            None if character == '"' || character == '\'' || character == '`' => {
                quote = Some(character);
            }
            None => {}
        }
    }

    values
}

pub(crate) fn node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source).ok().map(|value| value.to_string())
}

pub(crate) fn short_node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.end_byte().saturating_sub(node.start_byte()) > 240 {
        return None;
    }
    node_text(node, source)
}

pub(crate) fn source_line_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    let start = node.start_byte().min(source.len());
    let line_start = source[..start]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = source[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    std::str::from_utf8(&source[line_start..line_end])
        .ok()
        .map(str::to_string)
}

pub(crate) fn compact_label(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn strip_quotes(value: String) -> String {
    value
        .trim()
        .trim_start_matches(['r', 'b', 'u'])
        .trim_matches(['"', '\'', '`'])
        .to_string()
}

pub(crate) fn truncate_label(value: String, max_len: usize) -> String {
    if value.len() <= max_len {
        value
    } else {
        format!("{}...", &value[..max_len])
    }
}

pub(crate) fn clean_call_label(value: &str) -> String {
    let compact = compact_label(value.to_string());
    compact.trim_end_matches('!').to_string()
}

pub(crate) fn simple_name(value: &str) -> &str {
    value
        .rsplit([':', '.', '\\', '>'])
        .find(|part| !part.is_empty() && *part != "-")
        .unwrap_or(value)
}

pub(crate) fn is_identifier_part(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphanumeric()
}

pub(crate) fn looks_like_config_text(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        ".env",
        ".toml",
        ".yaml",
        ".yml",
        ".json",
        ".ini",
        ".conf",
        ".cfg",
        ".properties",
        "config",
        "settings",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

pub(crate) fn command_text_starts_with(source: &[u8], node: Node<'_>, prefixes: &[&str]) -> bool {
    let Some(text) = node_text(node, source) else {
        return false;
    };
    prefixes.iter().any(|prefix| {
        text == *prefix
            || text
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with(char::is_whitespace))
    })
}
