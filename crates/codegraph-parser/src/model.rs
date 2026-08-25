//! Parsed-fact model shared with the indexer: files, items, item kinds,
//! and parser errors.

use std::collections::BTreeMap;

use codegraph_core::SourceSpan;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Language;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("tree-sitter language setup failed for {language}: {message}")]
    LanguageSetup { language: Language, message: String },
    #[error("tree-sitter failed to produce a syntax tree for {language}")]
    ParseFailed { language: Language },
    #[error("source is not valid utf-8")]
    InvalidUtf8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFile {
    pub language: Language,
    pub items: Vec<ParsedItem>,
    /// Named type usages kept separate from structural items so the indexer
    /// can resolve them after every file-level type declaration is known.
    #[serde(default)]
    pub type_references: Vec<ParsedTypeReference>,
    /// Inclusive line ranges covered by string literals and comments.
    /// Text-scanning detectors need them: a route written in a docstring
    /// or a commented-out one is not a route the program serves.
    #[serde(default)]
    pub quoted_line_ranges: Vec<(u32, u32)>,
    /// Inclusive line ranges covered by string literals alone. A `// HACK`
    /// written inside a test's raw string is a fixture, not a note about
    /// this repository.
    #[serde(default)]
    pub string_line_ranges: Vec<(u32, u32)>,
    /// Names the file binds to a string literal at the top level:
    /// `const envLogFile = "TF_LOG_PATH"` is how a Go program spells the
    /// environment variables it reads, and the read names the constant
    /// rather than the variable.
    #[serde(default)]
    pub string_constants: Vec<(String, String)>,
    pub has_error_nodes: bool,
    /// The line the parser first lost the thread on, when it did. A file
    /// with an error node still yields facts, and where the grammar
    /// stumbled is what tells a project's own broken file from syntax the
    /// grammar does not cover.
    #[serde(default)]
    pub first_error_line: Option<u32>,
}

impl ParsedFile {
    /// Whether a line sits inside a string literal or a comment.
    pub fn line_is_quoted(&self, line: u32) -> bool {
        self.quoted_line_ranges
            .iter()
            .any(|(start, end)| line >= *start && line <= *end)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedTypeReference {
    pub label: String,
    pub span: SourceSpan,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedItem {
    pub kind: ParsedItemKind,
    pub label: String,
    pub span: SourceSpan,
    pub parent: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    Branch,
    Loop,
    Async,
    Return,
}

pub(crate) fn parsed_item_kind_label(kind: ParsedItemKind) -> &'static str {
    match kind {
        ParsedItemKind::Branch => "branch",
        ParsedItemKind::Loop => "loop",
        ParsedItemKind::Async => "async",
        ParsedItemKind::Return => "return",
        _ => "fact",
    }
}
