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
    Ruby,
    Java,
    CSharp,
    Kotlin,
    Swift,
    Scala,
    Lua,
    Elixir,
    Zig,
    Haskell,
    OCaml,
    Julia,
    Erlang,
    Nix,
    R,
    Hcl,
    Proto,
    GraphQl,
    Solidity,
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

static RUBY_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Ruby,
    extensions: &["rb", "rake", "gemspec"],
    file_names: &["Rakefile", "Gemfile"],
};
static JAVA_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Java,
    extensions: &["java"],
    file_names: &[],
};
static CSHARP_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::CSharp,
    extensions: &["cs"],
    file_names: &[],
};

static KOTLIN_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Kotlin,
    extensions: &["kt", "kts"],
    file_names: &[],
};
static SWIFT_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Swift,
    extensions: &["swift"],
    file_names: &[],
};
static SCALA_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Scala,
    extensions: &["scala", "sc"],
    file_names: &[],
};

static LUA_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Lua,
    extensions: &["lua"],
    file_names: &[],
};
static ELIXIR_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Elixir,
    extensions: &["ex", "exs"],
    file_names: &[],
};
static ZIG_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Zig,
    extensions: &["zig"],
    file_names: &[],
};

static HASKELL_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Haskell,
    extensions: &["hs"],
    file_names: &[],
};
static OCAML_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::OCaml,
    extensions: &["ml"],
    file_names: &[],
};
static JULIA_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Julia,
    extensions: &["jl"],
    file_names: &[],
};

static ERLANG_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Erlang,
    extensions: &["erl", "hrl"],
    file_names: &[],
};
static NIX_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Nix,
    extensions: &["nix"],
    file_names: &[],
};
// Terraform, Packer, Nomad and Consul all write HCL, and a `.tf` file is
// the shape most projects carry it in.
static HCL_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Hcl,
    extensions: &["tf", "tfvars", "hcl"],
    file_names: &[],
};
// A schema is a graph already: `.proto` states services and messages,
// `.graphql` states types and the fields that reach them.
static PROTO_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Proto,
    extensions: &["proto"],
    file_names: &[],
};
static GRAPHQL_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::GraphQl,
    extensions: &["graphql", "gql", "graphqls"],
    file_names: &[],
};
// A contract is a program that holds money, and what it declares — its
// functions, its events, the contracts it inherits — is the whole of what
// anyone can call.
static SOLIDITY_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::Solidity,
    extensions: &["sol"],
    file_names: &[],
};
static R_ADAPTER: BuiltinLanguageAdapter = BuiltinLanguageAdapter {
    language: Language::R,
    extensions: &["r", "R"],
    file_names: &[],
};

static LANGUAGE_ADAPTERS: [&dyn LanguageAdapter; 30] = [
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
    &RUBY_ADAPTER,
    &JAVA_ADAPTER,
    &CSHARP_ADAPTER,
    &KOTLIN_ADAPTER,
    &SWIFT_ADAPTER,
    &SCALA_ADAPTER,
    &LUA_ADAPTER,
    &ELIXIR_ADAPTER,
    &ZIG_ADAPTER,
    &HASKELL_ADAPTER,
    &OCAML_ADAPTER,
    &JULIA_ADAPTER,
    &ERLANG_ADAPTER,
    &NIX_ADAPTER,
    &R_ADAPTER,
    &HCL_ADAPTER,
    &PROTO_ADAPTER,
    &GRAPHQL_ADAPTER,
    &SOLIDITY_ADAPTER,
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
            Self::Ruby => "ruby",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Scala => "scala",
            Self::Lua => "lua",
            Self::Elixir => "elixir",
            Self::Zig => "zig",
            Self::Haskell => "haskell",
            Self::OCaml => "ocaml",
            Self::Julia => "julia",
            Self::Erlang => "erlang",
            Self::Nix => "nix",
            Self::R => "r",
            Self::Hcl => "hcl",
            Self::Proto => "proto",
            Self::GraphQl => "graphql",
            Self::Solidity => "solidity",
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
            Self::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Self::Swift => tree_sitter_swift::LANGUAGE.into(),
            Self::Scala => tree_sitter_scala::LANGUAGE.into(),
            Self::Lua => tree_sitter_lua::LANGUAGE.into(),
            Self::Elixir => tree_sitter_elixir::LANGUAGE.into(),
            Self::Zig => tree_sitter_zig::LANGUAGE.into(),
            Self::Haskell => tree_sitter_haskell::LANGUAGE.into(),
            Self::OCaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
            Self::Julia => tree_sitter_julia::LANGUAGE.into(),
            Self::Erlang => tree_sitter_erlang::LANGUAGE.into(),
            Self::Nix => tree_sitter_nix::LANGUAGE.into(),
            Self::R => tree_sitter_r::LANGUAGE.into(),
            Self::Hcl => tree_sitter_hcl::LANGUAGE.into(),
            Self::Proto => tree_sitter_proto::LANGUAGE.into(),
            Self::GraphQl => tree_sitter_graphql::LANGUAGE.into(),
            Self::Solidity => tree_sitter_solidity::LANGUAGE.into(),
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
