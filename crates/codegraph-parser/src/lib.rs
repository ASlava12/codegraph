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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedItemKind {
    Function,
    Type,
    Module,
    Import,
    Entrypoint,
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
    items: &mut Vec<ParsedItem>,
) {
    if let Some(item) = classify_node(language, node, source, path) {
        items.push(item);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(language, child, source, path, items);
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
    })
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

fn node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source).ok().map(|value| value.to_string())
}

fn compact_label(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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
