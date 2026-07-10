//! Language identities, detection patterns, and the language adapter
//! registry that maps files to Tree-sitter grammars.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tree_sitter::Language as TreeSitterLanguage;

use crate::{ParseError, ParsedFile, parse_source};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    C,
    Cpp,
    Dart,
    Php,
    Bash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageAdapterInfo {
    pub language: &'static str,
    pub parser: &'static str,
    pub extensions: &'static [&'static str],
    pub file_names: &'static [&'static str],
}

pub trait LanguageAdapter: Sync {
    fn language(&self) -> Language;

    fn name(&self) -> &'static str {
        self.language().name()
    }

    fn parser(&self) -> &'static str {
        "tree-sitter"
    }

    fn extensions(&self) -> &'static [&'static str];

    fn file_names(&self) -> &'static [&'static str] {
        &[]
    }

    fn matches_path(&self, path: &Path) -> bool {
        let file_name = path.file_name().and_then(|value| value.to_str());
        if file_name.is_some_and(|value| self.file_names().contains(&value)) {
            return true;
        }

        path.extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| self.extensions().contains(&extension))
    }

    fn parse(&self, path: &Path, source: &[u8]) -> Result<ParsedFile, ParseError> {
        parse_source(path, source, self.language())
    }

    fn info(&self) -> LanguageAdapterInfo {
        LanguageAdapterInfo {
            language: self.name(),
            parser: self.parser(),
            extensions: self.extensions(),
            file_names: self.file_names(),
        }
    }
}

struct BuiltinLanguageAdapter {
    language: Language,
    extensions: &'static [&'static str],
    file_names: &'static [&'static str],
}

impl LanguageAdapter for BuiltinLanguageAdapter {
    fn language(&self) -> Language {
        self.language
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    fn file_names(&self) -> &'static [&'static str] {
        self.file_names
    }
}

static RUST_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Rust,
    extensions: &["rs"],
    file_names: &[],
};
static PYTHON_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Python,
    extensions: &["py", "pyw"],
    file_names: &[],
};
static JAVASCRIPT_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::JavaScript,
    extensions: &["js", "mjs", "cjs"],
    file_names: &[],
};
static TYPESCRIPT_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::TypeScript,
    extensions: &["ts", "mts", "cts"],
    file_names: &[],
};
static TSX_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Tsx,
    extensions: &["tsx"],
    file_names: &[],
};
static GO_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Go,
    extensions: &["go"],
    file_names: &[],
};
static C_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::C,
    extensions: &["c", "h"],
    file_names: &[],
};
static CPP_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Cpp,
    extensions: &["cc", "cpp", "cxx", "hpp", "hh", "hxx"],
    file_names: &[],
};
static DART_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Dart,
    extensions: &["dart"],
    file_names: &[],
};
static PHP_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Php,
    extensions: &["php", "phtml"],
    file_names: &[],
};
static BASH_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Bash,
    extensions: &["sh", "bash", "zsh", "ksh"],
    file_names: &["Makefile"],
};

static LANGUAGE_ADAPTERS: [&dyn LanguageAdapter; 11] = [
    &RUST_ADAPTER,
    &PYTHON_ADAPTER,
    &JAVASCRIPT_ADAPTER,
    &TYPESCRIPT_ADAPTER,
    &TSX_ADAPTER,
    &GO_ADAPTER,
    &C_ADAPTER,
    &CPP_ADAPTER,
    &DART_ADAPTER,
    &PHP_ADAPTER,
    &BASH_ADAPTER,
];

pub fn language_adapters() -> &'static [&'static dyn LanguageAdapter] {
    &LANGUAGE_ADAPTERS
}

pub fn adapter_for_language(language: Language) -> Option<&'static dyn LanguageAdapter> {
    language_adapters()
        .iter()
        .copied()
        .find(|adapter| adapter.language() == language)
}

pub fn adapter_for_path(path: &Path) -> Option<&'static dyn LanguageAdapter> {
    language_adapters()
        .iter()
        .copied()
        .find(|adapter| adapter.matches_path(path))
}

impl Language {
    pub fn detect(path: &Path) -> Option<Self> {
        adapter_for_path(path).map(LanguageAdapter::language)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Dart => "dart",
            Self::Php => "php",
            Self::Bash => "bash",
        }
    }

    pub(crate) fn tree_sitter_language(self) -> TreeSitterLanguage {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::Dart => tree_sitter_dart::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
