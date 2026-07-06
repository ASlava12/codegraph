use codegraph_core::SourceSpan;
use std::fmt;
use std::path::Path;
use thiserror::Error;
use tree_sitter::{Language as TreeSitterLanguage, Node, Parser, TreeCursor};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("tree-sitter language setup failed for {language}: {message}")]
    LanguageSetup { language: Language, message: String },
    #[error("tree-sitter failed to produce a syntax tree for {language}")]
    ParseFailed { language: Language },
    #[error("source is not valid utf-8")]
    InvalidUtf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    C,
    Cpp,
    Php,
    Bash,
}

impl Language {
    pub fn detect(path: &Path) -> Option<Self> {
        let file_name = path.file_name()?.to_str()?;
        let extension = path.extension().and_then(|value| value.to_str());

        if file_name == "Makefile" {
            return Some(Self::Bash);
        }

        match extension {
            Some("rs") => Some(Self::Rust),
            Some("py") | Some("pyw") => Some(Self::Python),
            Some("js") | Some("mjs") | Some("cjs") => Some(Self::JavaScript),
            Some("ts") | Some("mts") | Some("cts") => Some(Self::TypeScript),
            Some("tsx") => Some(Self::Tsx),
            Some("go") => Some(Self::Go),
            Some("c") | Some("h") => Some(Self::C),
            Some("cc") | Some("cpp") | Some("cxx") | Some("hpp") | Some("hh") | Some("hxx") => {
                Some(Self::Cpp)
            }
            Some("php") | Some("phtml") => Some(Self::Php),
            Some("sh") | Some("bash") | Some("zsh") | Some("ksh") => Some(Self::Bash),
            _ => None,
        }
    }

    fn tree_sitter_language(self) -> TreeSitterLanguage {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Php => "php",
            Self::Bash => "bash",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    pub language: Language,
    pub items: Vec<ParsedItem>,
    pub has_error_nodes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedItem {
    pub kind: ParsedItemKind,
    pub label: String,
    pub span: SourceSpan,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedItemKind {
    Function,
    Type,
    Module,
    Import,
    Entrypoint,
    Call,
    EnvironmentRead,
    ConfigRead,
    Error,
}

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
    );
    dedupe_items(&mut items);

    Ok(ParsedFile {
        language,
        items,
        has_error_nodes: root.has_error(),
    })
}

fn collect_items(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    current_function: Option<String>,
    items: &mut Vec<ParsedItem>,
) {
    if let Some(effect) = classify_effect(language, node, source, path, current_function.as_deref())
    {
        items.push(effect);
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
        collect_items(language, child, source, path, next_function.clone(), items);
    }
}

fn classify_node(
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
            "import_declaration" => ParsedItemKind::Import,
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
            | "require_once_expression" => ParsedItemKind::Import,
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
    })
}

fn classify_call(
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
    })
}

fn classify_effect(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    path: &str,
    function_name: Option<&str>,
) -> Option<ParsedItem> {
    let label = if is_environment_read(language, node, source) {
        effect_label(node, source).map(|label| (ParsedItemKind::EnvironmentRead, label))
    } else if is_config_read(language, node, source) {
        effect_label(node, source).map(|label| (ParsedItemKind::ConfigRead, label))
    } else if is_error_construct(language, node, source) {
        effect_label(node, source).map(|label| (ParsedItemKind::Error, label))
    } else {
        None
    }?;

    Some(ParsedItem {
        kind: label.0,
        label: label.1,
        span: span_for(path, node),
        parent: function_name.map(ToString::to_string),
    })
}

fn is_environment_read(language: Language, node: Node<'_>, source: &[u8]) -> bool {
    let text = short_node_text(node, source);
    let call = call_label(language, node, source);

    match language {
        Language::Rust => {
            is_call_node(language, node, source)
                && call
                    .as_deref()
                    .is_some_and(|value| matches!(value, "env::var" | "std::env::var" | "var"))
        }
        Language::Python => {
            matches!(node.kind(), "call" | "subscript")
                && text.as_deref().is_some_and(|value| {
                    value.contains("os.getenv") || value.contains("os.environ")
                })
        }
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            matches!(
                node.kind(),
                "call_expression" | "member_expression" | "subscript_expression"
            ) && text
                .as_deref()
                .is_some_and(|value| value.contains("process.env"))
        }
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
        Language::Bash => node.kind() == "variable_name",
    }
}

fn is_config_read(language: Language, node: Node<'_>, source: &[u8]) -> bool {
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
            text.contains("readFile")
                || text.contains("require(")
                || call
                    .as_deref()
                    .is_some_and(|value| matches!(simple_name(value), "readFile" | "readFileSync"))
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
        Language::Bash => {
            node.kind() == "command" && command_text_starts_with(source, node, &["source", "."])
        }
    }
}

fn is_error_construct(language: Language, node: Node<'_>, source: &[u8]) -> bool {
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
                    .is_some_and(|value| simple_name(value) == "panic")
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
        Language::Bash => {
            node.kind() == "command" && command_text_starts_with(source, node, &["exit"])
        }
    }
}

fn effect_label(node: Node<'_>, source: &[u8]) -> Option<String> {
    first_string_literal(node, source)
        .or_else(|| node_text(node, source).map(compact_label))
        .map(|value| truncate_label(value, 120))
}

fn is_call_node(language: Language, node: Node<'_>, source: &[u8]) -> bool {
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
        Language::Bash => {
            node.kind() == "command" && !command_text_starts_with(source, node, &["source", "."])
        }
    }
}

fn call_label(language: Language, node: Node<'_>, source: &[u8]) -> Option<String> {
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

fn item_label(
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

fn named_child_text(node: Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| node_text(child, source))
}

fn first_identifier_in_field(node: Node<'_>, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| first_identifier(child, source))
}

fn first_identifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "identifier"
            | "field_identifier"
            | "type_identifier"
            | "namespace_identifier"
            | "property_identifier"
            | "variable_name"
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

fn first_string_literal(node: Node<'_>, source: &[u8]) -> Option<String> {
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

fn node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source).ok().map(|value| value.to_string())
}

fn short_node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.end_byte().saturating_sub(node.start_byte()) > 240 {
        return None;
    }
    node_text(node, source)
}

fn compact_label(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_quotes(value: String) -> String {
    value
        .trim()
        .trim_start_matches(['r', 'b', 'u'])
        .trim_matches(['"', '\'', '`'])
        .to_string()
}

fn truncate_label(value: String, max_len: usize) -> String {
    if value.len() <= max_len {
        value
    } else {
        format!("{}...", &value[..max_len])
    }
}

fn clean_call_label(value: &str) -> String {
    let compact = compact_label(value.to_string());
    compact.trim_end_matches('!').to_string()
}

fn simple_name(value: &str) -> &str {
    value
        .rsplit([':', '.', '\\', '>'])
        .find(|part| !part.is_empty() && *part != "-")
        .unwrap_or(value)
}

fn looks_like_config_text(value: &str) -> bool {
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

fn command_text_starts_with(source: &[u8], node: Node<'_>, prefixes: &[&str]) -> bool {
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

fn is_entrypoint(language: Language, label: &str) -> bool {
    match language {
        Language::Rust | Language::Go | Language::C | Language::Cpp => label == "main",
        Language::Python => label == "main" || label == "__main__",
        Language::JavaScript | Language::TypeScript | Language::Tsx | Language::Php => {
            label.eq_ignore_ascii_case("main")
        }
        Language::Bash => label == "main",
    }
}

fn span_for(path: &str, node: Node<'_>) -> SourceSpan {
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

fn dedupe_items(items: &mut Vec<ParsedItem>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_target_languages_by_extension() {
        let cases = [
            ("main.rs", Language::Rust),
            ("app.py", Language::Python),
            ("index.js", Language::JavaScript),
            ("view.tsx", Language::Tsx),
            ("main.go", Language::Go),
            ("lib.c", Language::C),
            ("lib.cpp", Language::Cpp),
            ("index.php", Language::Php),
            ("deploy.sh", Language::Bash),
        ];

        for (path, language) in cases {
            assert_eq!(Language::detect(Path::new(path)), Some(language));
        }
    }

    #[test]
    fn parses_rust_symbols() {
        let parsed = parse_source(
            "src/main.rs",
            b"use std::fs;\nstruct App;\nfn main() {}\n",
            Language::Rust,
        )
        .unwrap();

        assert!(
            parsed
                .items
                .iter()
                .any(|item| item.label == "main" && item.kind == ParsedItemKind::Entrypoint)
        );
        assert!(
            parsed
                .items
                .iter()
                .any(|item| item.label == "App" && item.kind == ParsedItemKind::Type)
        );
        assert!(
            parsed
                .items
                .iter()
                .any(|item| item.label == "use std::fs;" && item.kind == ParsedItemKind::Import)
        );
    }

    #[test]
    fn parses_rust_calls_with_parent_function() {
        let parsed = parse_source(
            "src/main.rs",
            b"fn main() { helper(); println!(\"ok\"); }\nfn helper() {}\n",
            Language::Rust,
        )
        .unwrap();

        assert!(parsed.items.iter().any(|item| {
            item.kind == ParsedItemKind::Call
                && item.label == "helper"
                && item.parent.as_deref() == Some("main")
        }));
    }

    #[test]
    fn parses_environment_config_and_error_facts() {
        let parsed = parse_source(
            "src/main.rs",
            br#"fn main() {
                let _ = std::env::var("DATABASE_URL");
                let _ = std::fs::read_to_string("config/app.toml");
                panic!("broken");
            }
            "#,
            Language::Rust,
        )
        .unwrap();

        assert!(parsed.items.iter().any(|item| {
            item.kind == ParsedItemKind::EnvironmentRead && item.label == "DATABASE_URL"
        }));
        assert!(parsed.items.iter().any(|item| {
            item.kind == ParsedItemKind::ConfigRead && item.label == "config/app.toml"
        }));
        assert!(
            parsed
                .items
                .iter()
                .any(|item| item.kind == ParsedItemKind::Error)
        );
    }

    #[test]
    fn parses_mixed_language_smoke_samples() {
        let cases = [
            (
                "app.py",
                Language::Python,
                "import os\nclass App:\n    pass\ndef main():\n    pass\n",
                "main",
            ),
            (
                "index.js",
                Language::JavaScript,
                "import x from 'x';\nclass App {}\nfunction main() {}\n",
                "main",
            ),
            (
                "main.go",
                Language::Go,
                "package main\nimport \"fmt\"\nfunc main() {}\n",
                "main",
            ),
            (
                "main.c",
                Language::C,
                "#include <stdio.h>\nint main() { return 0; }\n",
                "main",
            ),
            (
                "main.cpp",
                Language::Cpp,
                "#include <iostream>\nint main() { return 0; }\n",
                "main",
            ),
            (
                "index.php",
                Language::Php,
                "<?php\nfunction main() {}\n",
                "main",
            ),
            (
                "deploy.sh",
                Language::Bash,
                "main() { echo ok; }\nsource ./env.sh\n",
                "main",
            ),
        ];

        for (path, language, source, expected) in cases {
            let parsed = parse_source(path, source.as_bytes(), language).unwrap();
            assert!(
                parsed.items.iter().any(|item| item.label == expected),
                "{language} did not produce expected symbol {expected}: {:?}",
                parsed.items
            );
        }
    }
}
