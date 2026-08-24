//! Unit tests for language detection and syntax-fact extraction.

use super::*;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn language_registry_exposes_all_builtin_adapters() {
    let adapters = language_adapters();
    let languages = adapters
        .iter()
        .map(|adapter| adapter.info().language)
        .collect::<BTreeSet<_>>();

    assert_eq!(adapters.len(), 26);
    assert_eq!(
        languages,
        BTreeSet::from([
            "bash",
            "c",
            "cpp",
            "csharp",
            "dart",
            "go",
            "haskell",
            "elixir",
            "erlang",
            "java",
            "javascript",
            "julia",
            "kotlin",
            "lua",
            "nix",
            "ocaml",
            "php",
            "python",
            "r",
            "ruby",
            "rust",
            "scala",
            "swift",
            "tsx",
            "typescript",
            "zig",
        ])
    );
    assert!(
        adapters
            .iter()
            .all(|adapter| adapter.info().parser == "tree-sitter")
    );
}

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
        ("main.dart", Language::Dart),
        ("index.php", Language::Php),
        ("deploy.sh", Language::Bash),
    ];

    for (path, language) in cases {
        assert_eq!(Language::detect(Path::new(path)), Some(language));
        assert_eq!(
            adapter_for_path(Path::new(path)).map(|adapter| adapter.language()),
            Some(language)
        );
    }
}

#[test]
fn language_adapters_parse_sources() {
    let adapter = adapter_for_language(Language::Rust).unwrap();
    let parsed = adapter
        .parse(Path::new("src/main.rs"), b"fn main() {}\n")
        .unwrap();

    assert_eq!(parsed.language, Language::Rust);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| { item.kind == ParsedItemKind::Entrypoint && item.label == "main" })
    );
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
fn parses_control_flow_markers_with_parent_function() {
    let cases = [
        (
            "src/main.rs",
            Language::Rust,
            "async fn worker() { if ready() { for item in items() { item.await; } } }\n",
            "worker",
            ["branch: if", "loop: for", "async: await"],
        ),
        (
            "app.py",
            Language::Python,
            "async def worker():\n    if ready:\n        for item in items:\n            await item\n",
            "worker",
            ["branch: if", "loop: for", "async: await"],
        ),
        (
            "index.js",
            Language::JavaScript,
            "async function worker() { if (ready) { for (const item of items) { await item; } } }\n",
            "worker",
            ["branch: if", "loop: for", "async: await"],
        ),
        (
            "main.go",
            Language::Go,
            "package main\nfunc worker() { if ready { for _, item := range items { go use(item) } } }\n",
            "worker",
            ["branch: if", "loop: for", "async: go"],
        ),
        (
            "main.c",
            Language::C,
            "void worker() { if (ready) { for (int i = 0; i < 3; i++) {} } }\n",
            "worker",
            ["branch: if", "loop: for", ""],
        ),
        (
            "app.php",
            Language::Php,
            "<?php function worker() { if ($ready) { foreach ($items as $item) {} } }\n",
            "worker",
            ["branch: if", "loop: foreach", ""],
        ),
        (
            "script.sh",
            Language::Bash,
            "worker() { if true; then for item in a b; do echo \"$item\"; done; fi; }\n",
            "worker",
            ["branch: if", "loop: for", ""],
        ),
    ];

    for (path, language, source, parent, expected_labels) in cases {
        let parsed = parse_source(path, source.as_bytes(), language).unwrap();
        for expected_label in expected_labels
            .into_iter()
            .filter(|label| !label.is_empty())
        {
            assert!(
                parsed.items.iter().any(|item| {
                    matches!(
                        item.kind,
                        ParsedItemKind::Branch | ParsedItemKind::Loop | ParsedItemKind::Async
                    ) && item.label == expected_label
                        && item.parent.as_deref() == Some(parent)
                        && item.metadata.contains_key("control_kind")
                }),
                "missing {expected_label} in {path}: {:#?}",
                parsed.items
            );
        }
    }
}

#[test]
fn parses_return_markers_with_parent_function() {
    let cases = [
        (
            "src/main.rs",
            Language::Rust,
            "fn worker() -> bool { if ready() { return true; } false }\n",
        ),
        (
            "app.py",
            Language::Python,
            "def worker():\n    return compute()\n",
        ),
        (
            "index.js",
            Language::JavaScript,
            "function worker() { return compute(); }\n",
        ),
        (
            "main.go",
            Language::Go,
            "package main\nfunc worker() int { return compute() }\n",
        ),
        (
            "main.c",
            Language::C,
            "int worker() { return compute(); }\n",
        ),
        (
            "app.php",
            Language::Php,
            "<?php function worker() { return compute(); }\n",
        ),
        (
            "lib/main.dart",
            Language::Dart,
            "void worker() { if (ready) { return; } compute(); }\n",
        ),
    ];

    for (path, language, source) in cases {
        let parsed = parse_source(path, source.as_bytes(), language).unwrap();
        assert!(
            parsed.items.iter().any(|item| {
                item.kind == ParsedItemKind::Return
                    && item.label == "return: return"
                    && item.parent.as_deref() == Some("worker")
                    && item.metadata.get("control_kind").map(String::as_str) == Some("return")
            }),
            "missing return marker in {path}: {:#?}",
            parsed.items
        );
    }
}

#[test]
fn parses_go_import_specs_individually() {
    let parsed = parse_source(
        "main.go",
        br#"package main
import (
    "fmt"
    "github.com/acme/demo/internal/auth"
)
func main() {}
"#,
        Language::Go,
    )
    .unwrap();

    let imports = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Import)
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(imports.contains(&"\"fmt\""));
    assert!(imports.contains(&"\"github.com/acme/demo/internal/auth\""));
    assert!(
        !imports.iter().any(|label| label.starts_with("import (")),
        "Go import declarations should be split into per-import facts"
    );
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
fn parses_python_environment_default_values() {
    let parsed = parse_source(
        "app.py",
        br#"import os
PORT = os.getenv("PORT", "8000")
DATABASE_URL = os.environ.get("DATABASE_URL", "sqlite:///local.db")
"#,
        Language::Python,
    )
    .unwrap();

    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");

    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("8000")
    );
    let database_url = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "DATABASE_URL")
        .expect("missing DATABASE_URL env read");
    assert_eq!(
        database_url
            .metadata
            .get("default_value")
            .map(String::as_str),
        Some("sqlite:///local.db")
    );
}

#[test]
fn effect_facts_are_never_labelled_with_nothing() {
    let parsed = parse_source(
        "main.rs",
        br#"fn main() {
    let raw = std::fs::read_to_string("");
    if raw.is_err() {
        panic!("");
    }
}
"#,
        Language::Rust,
    )
    .unwrap();

    // Both facts are real, so both are kept — but an empty string literal is
    // not an identity, and a node labelled with nothing is not a node.
    for item in &parsed.items {
        assert!(
            !item.label.trim().is_empty(),
            "empty label on {:?}",
            item.kind
        );
    }
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error),
        "the panic is still a fact"
    );
}

#[test]
fn javascript_config_reads_need_a_config_to_read() {
    let parsed = parse_source(
        "build.js",
        br#"const fs = require('fs')
const pkg = require('./package.json')
const lodash = require('lodash')

function load(configPath) {
    const parsed = ts.parseJsonConfigFileContent(ts.readConfigFile(configPath, fs.readFile).config)
    return fs.readFileSync('config.yaml')
}
"#,
        Language::JavaScript,
    )
    .unwrap();

    let configs = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::ConfigRead)
        .map(|item| item.label.as_str())
        .collect::<BTreeSet<_>>();

    // `require` of a data file reads configuration; `require('lodash')` and
    // `require('fs')` are module imports, and an expression that merely
    // mentions `readFile` is not a read at all — matching on text filed both
    // of those, labelled with a slab of source code.
    assert_eq!(
        configs,
        BTreeSet::from(["./package.json", "config.yaml"]),
        "unexpected config reads"
    );
}

#[test]
fn parses_javascript_environment_default_values() {
    let parsed = parse_source(
        "app.js",
        br#"const port = process.env.PORT || "3000";
const token = process.env["TOKEN"] ?? "dev-token";
"#,
        Language::JavaScript,
    )
    .unwrap();

    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");
    let token = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "TOKEN")
        .expect("missing TOKEN env read");

    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("3000")
    );
    assert_eq!(
        token.metadata.get("default_value").map(String::as_str),
        Some("dev-token")
    );
}

#[test]
fn parses_php_environment_default_values() {
    let parsed = parse_source(
        "index.php",
        br#"<?php
$port = getenv('PORT') ?: '8080';
"#,
        Language::Php,
    )
    .unwrap();

    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");

    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("8080")
    );
}

#[test]
fn parses_php_namespace_use_declarations_as_imports() {
    let parsed = parse_source(
        "src/App.php",
        br#"<?php
use Monolog\Logger;
use Symfony\Component\{Console\Application, HttpFoundation\Request as HttpRequest};
class App {}
"#,
        Language::Php,
    )
    .unwrap();

    let imports = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Import)
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(imports.contains(&"use Monolog\\Logger;"));
    assert!(imports.iter().any(|label| {
        label.contains("Symfony\\Component")
            && label.contains("Console\\Application")
            && label.contains("HttpFoundation\\Request")
    }));
}

#[test]
fn parses_rust_environment_default_values() {
    let parsed = parse_source(
        "src/main.rs",
        br#"fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "7000".to_string());
}
"#,
        Language::Rust,
    )
    .unwrap();

    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");

    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("7000")
    );
}

#[test]
fn ruby_visibility_follows_the_class_body() {
    // `private` on a line of its own changes what follows it, `private def
    // foo` changes that one definition, and `private :foo` names a method
    // written elsewhere. A nested class keeps its own answer.
    let source = b"class A\n  def shown; end\n  private\n  def hidden; end\n  public\n  def shown_again; end\n  private def inline; end\n  def named_later; end\n  private :named_later\n\n  class Inner\n    private\n    def buried; end\n  end\n\n  def after_inner; end\nend\n";
    let parsed = parse_source("a.rb", source, Language::Ruby).unwrap();
    let visibility_of = |label: &str| -> String {
        parsed
            .items
            .iter()
            .find(|item| {
                item.label == label
                    && matches!(
                        item.kind,
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint
                    )
            })
            .and_then(|item| item.metadata.get("visibility").cloned())
            .unwrap_or_else(|| format!("no `{label}`"))
    };

    assert_eq!(visibility_of("shown"), "public");
    assert_eq!(visibility_of("hidden"), "private");
    assert_eq!(visibility_of("shown_again"), "public");
    assert_eq!(visibility_of("inline"), "private");
    // `private :named_later` reaches a definition written above it.
    assert_eq!(visibility_of("named_later"), "private");
    assert_eq!(visibility_of("buried"), "private");
    // The inner class's `private` says nothing about the outer one.
    assert_eq!(visibility_of("after_inner"), "public");
}

#[test]
fn erlang_and_haskell_export_lists_are_read() {
    // Both write the list at the top of the file, and what is not on it
    // cannot be called from another module. A module that gives no list at
    // all gives everything.
    // A call carries the same label as the function it names, so the
    // definition is what this looks for.
    let visibility_of = |path: &str, source: &[u8], language: Language, label: &str| -> String {
        parse_source(path, source, language)
            .unwrap()
            .items
            .iter()
            .find(|item| {
                item.label == label
                    && matches!(
                        item.kind,
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint
                    )
            })
            .and_then(|item| item.metadata.get("visibility").cloned())
            .unwrap_or_else(|| format!("no `{label}` in {path}"))
    };

    let erlang = b"-module(demo).\n-export([start/1]).\n-export_type([state/0]).\n\nstart(Args) -> helper(Args).\nhelper(Args) -> Args.\n";
    assert_eq!(
        visibility_of("demo.erl", erlang, Language::Erlang, "start"),
        "public"
    );
    // `-export_type` lists types, not functions, and does not let `helper`
    // out.
    assert_eq!(
        visibility_of("demo.erl", erlang, Language::Erlang, "helper"),
        "private"
    );

    let open = b"-module(demo).\n-compile(export_all).\n\nhelper(Args) -> Args.\n";
    assert_eq!(
        visibility_of("demo.erl", open, Language::Erlang, "helper"),
        "public"
    );

    let haskell = b"module ShellCheck.Checker (checkScript) where\n\ncheckScript :: String -> Int\ncheckScript s = helper s\n\nhelper :: String -> Int\nhelper s = 1\n";
    assert_eq!(
        visibility_of("Checker.hs", haskell, Language::Haskell, "checkScript"),
        "public"
    );
    assert_eq!(
        visibility_of("Checker.hs", haskell, Language::Haskell, "helper"),
        "private"
    );

    let everything = b"module Main where\n\nhelper :: Int\nhelper = 1\n";
    assert_eq!(
        visibility_of("Main.hs", everything, Language::Haskell, "helper"),
        "public"
    );
}

#[test]
fn every_language_that_states_its_visibility_is_read() {
    // A library's coverage finding counts what it exports, and a function
    // nobody calls is either dead or the API. Both need the language to be
    // read where it says so: `static` in C, `local` in Lua, `defp` in
    // Elixir, `pub` in Zig, a modifier in C#, PHP, Swift and Scala, and a
    // leading underscore in Dart.
    // A call carries the same label as the function it names, so the
    // definition is what this looks for.
    let visibility_of = |path: &str, source: &[u8], language: Language, label: &str| -> String {
        parse_source(path, source, language)
            .unwrap()
            .items
            .iter()
            .find(|item| {
                item.label == label
                    && matches!(
                        item.kind,
                        ParsedItemKind::Function | ParsedItemKind::Entrypoint
                    )
            })
            .and_then(|item| item.metadata.get("visibility").cloned())
            .unwrap_or_else(|| format!("no `{label}` in {path}"))
    };

    let c = b"static int hidden(void) { return 1; }\nint shared(void) { return 2; }\n";
    assert_eq!(visibility_of("a.c", c, Language::C, "hidden"), "private");
    assert_eq!(visibility_of("a.c", c, Language::C, "shared"), "public");

    let lua =
        b"local function hidden() end\nlocal bound = function() end\nfunction M.shared() end\n";
    assert_eq!(
        visibility_of("a.lua", lua, Language::Lua, "hidden"),
        "private"
    );
    // `local bound = function() end` keeps it to the file too, though the
    // expression itself opens with `function`.
    assert_eq!(
        visibility_of("a.lua", lua, Language::Lua, "bound"),
        "private"
    );
    assert_eq!(
        visibility_of("a.lua", lua, Language::Lua, "M.shared"),
        "public"
    );

    let elixir = b"defmodule A do\n  def shared do\n  end\n  defp hidden do\n  end\nend\n";
    assert_eq!(
        visibility_of("a.ex", elixir, Language::Elixir, "hidden"),
        "private"
    );
    assert_eq!(
        visibility_of("a.ex", elixir, Language::Elixir, "shared"),
        "public"
    );

    let zig = b"pub fn shared() void {}\nfn hidden() void {}\n";
    assert_eq!(
        visibility_of("a.zig", zig, Language::Zig, "shared"),
        "public"
    );
    assert_eq!(
        visibility_of("a.zig", zig, Language::Zig, "hidden"),
        "private"
    );

    let dart = b"void shared() {}\nvoid _hidden() {}\n";
    assert_eq!(
        visibility_of("a.dart", dart, Language::Dart, "_hidden"),
        "private"
    );

    let php = b"<?php\nclass A { public function shared() {} private function hidden() {} }\n";
    assert_eq!(
        visibility_of("a.php", php, Language::Php, "hidden"),
        "private"
    );
    assert_eq!(
        visibility_of("a.php", php, Language::Php, "shared"),
        "public"
    );

    // A C# member says nothing when it is private, and Swift's silence
    // means its own module.
    let csharp = b"class A { public void Shared() {} void Hidden() {} }\n";
    assert_eq!(
        visibility_of("a.cs", csharp, Language::CSharp, "Hidden"),
        "private"
    );
    let swift =
        b"public func shared() {}\nfunc scoped() {}\nfileprivate func lock() {}\nprivate func held() {}\n";
    assert_eq!(
        visibility_of("a.swift", swift, Language::Swift, "scoped"),
        "crate"
    );
    // `fileprivate` is a definition no other file can name, which is what
    // `private` records here -- Alamofire writes 17 of them.
    assert_eq!(
        visibility_of("a.swift", swift, Language::Swift, "lock"),
        "private"
    );
    assert_eq!(
        visibility_of("a.swift", swift, Language::Swift, "held"),
        "private"
    );
}

#[test]
fn java_and_kotlin_say_what_they_offer_outwards() {
    // okio and gson are libraries, and without a visibility fact the
    // coverage finding could not say so: it now reads "counting the 3428
    // exported functions as starting points reaches 92%" for okio.
    let java = parse_source(
        "A.java",
        b"class A {\n  public void open() {}\n  private void hidden() {}\n  void shared() {}\n}\n",
        Language::Java,
    )
    .unwrap();
    let visibility_of = |parsed: &ParsedFile, label: &str| -> String {
        parsed
            .items
            .iter()
            .find(|item| item.label == label)
            .and_then(|item| item.metadata.get("visibility").cloned())
            .unwrap_or_else(|| format!("no `{label}`"))
    };
    assert_eq!(visibility_of(&java, "open"), "public");
    assert_eq!(visibility_of(&java, "hidden"), "private");
    // Java means package-private when it says nothing.
    assert_eq!(visibility_of(&java, "shared"), "package");

    let kotlin = parse_source(
        "B.kt",
        b"fun open() {}\nprivate fun hidden() {}\ninternal fun shared() {}\n",
        Language::Kotlin,
    )
    .unwrap();
    // Kotlin means public when it says nothing.
    assert_eq!(visibility_of(&kotlin, "open"), "public");
    assert_eq!(visibility_of(&kotlin, "hidden"), "private");
    assert_eq!(visibility_of(&kotlin, "shared"), "crate");
}

#[test]
fn a_future_import_is_a_directive_not_a_dependency() {
    // `from __future__ import annotations` turns on a language feature;
    // there is no package called `__future__` to depend on. Python's
    // grammar gives it a node of its own, which the extraction does not
    // list -- and should not, though flask writes the line in 45 files and
    // it looks like an import in every one of them.
    let parsed = parse_source(
        "app.py",
        b"from __future__ import annotations\n\nimport os\nfrom datetime import datetime\n",
        Language::Python,
    )
    .unwrap();

    let imports: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Import)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(
        imports,
        vec!["import os", "from datetime import datetime"],
        "a language directive is not one of the file's imports"
    );
}

#[test]
fn a_named_function_expression_keeps_its_own_name() {
    // `new Promise(function dispatchXhrRequest(resolve) {...})` is a
    // function with a name of its own and nothing to bind it to. Axios
    // writes six of its own that way, and the benchmark oracle counted
    // every one as missing.
    let source = "new Promise(function dispatchXhrRequest(resolve) { send(resolve); });\n\
                  request.on('error', function handleError(err) { fail(err); });\n\
                  let plain = function inner(x) { return x; };\n\
                  items.map(function (x) { return x * 2; });\n";
    let parsed = parse_source("app.js", source.as_bytes(), Language::JavaScript).unwrap();
    let mut functions: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Function)
        .map(|item| item.label.as_str())
        .collect();
    functions.sort_unstable();
    assert_eq!(
        functions,
        vec!["dispatchXhrRequest", "handleError", "plain"],
        "a binding still wins over the function's own name, and an anonymous callback stays anonymous"
    );
}

#[test]
fn parses_kotlin_native_environment_reads() {
    // okio reads TMPDIR through `platform.posix.getenv` and TEMP through
    // the Windows `_wgetenv`, and only `System.getenv` was recognised: the
    // project's own benchmark oracle found four of its seven environment
    // reads missing.
    let parsed = parse_source(
        "Variant.kt",
        br#"import platform.posix._wgetenv
import platform.posix.getenv

fun tmpdir(): String? {
    val jvm = System.getenv("HOME")
    val unix = getenv("TMPDIR")
    val windows = _wgetenv("TEMP".wcstr)
    return jvm ?: unix ?: windows
}
"#,
        Language::Kotlin,
    )
    .unwrap();

    let mut keys: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .map(|item| item.label.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["HOME", "TEMP", "TMPDIR"]);
}

#[test]
fn parses_python_configuration_mapping_reads() {
    // Flask keeps configuration in a mapping on the application, so
    // `app.config["SECRET_KEY"]` reads that key the way `os.environ` reads
    // the environment. Asking flask where SECRET_KEY is read used to
    // answer with nothing at all.
    let parsed = parse_source(
        "app.py",
        br#"def start(app, name):
    secret = app.config["SECRET_KEY"]
    port = app.config.get("PORT", 8080)
    debug = current_app.config['DEBUG']
    computed = app.config[name]
    other = app.settings["NOT_CONFIG"]
    return secret, port, debug, computed, other
"#,
        Language::Python,
    )
    .unwrap();

    let reads: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::ConfigRead)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(
        reads,
        vec!["SECRET_KEY", "PORT", "DEBUG"],
        "a key the code works out, and a mapping that is not `config`, name nothing to record"
    );

    let port = parsed
        .items
        .iter()
        .find(|item| item.label == "PORT")
        .expect("the PORT read");
    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("8080"),
        "`get` states the fallback outright"
    );
    let secret = parsed
        .items
        .iter()
        .find(|item| item.label == "SECRET_KEY")
        .expect("the SECRET_KEY read");
    assert!(!secret.metadata.contains_key("default_value"));
}

#[test]
fn parses_bash_environment_default_values() {
    let parsed = parse_source(
        "entrypoint.sh",
        br#"#!/usr/bin/env bash
PORT="${PORT:-5000}"
"#,
        Language::Bash,
    )
    .unwrap();

    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");

    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("5000")
    );
    // The script assigns the default to the variable it read, which gives
    // every later `$PORT` a value.
    assert_eq!(
        port.metadata.get("defaults_variable").map(String::as_str),
        Some("true")
    );

    let parsed = parse_source(
        "other.sh",
        br#"#!/usr/bin/env bash
echo "${PORT:-5000}"
DIR="${OTHER:-/tmp}"
"#,
        Language::Bash,
    )
    .unwrap();
    for (label, name) in [
        ("PORT", "printing a default sets nothing"),
        ("OTHER", "the assignment names another variable"),
    ] {
        let read = parsed
            .items
            .iter()
            .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == label)
            .unwrap_or_else(|| panic!("missing {label} env read"));
        assert!(!read.metadata.contains_key("defaults_variable"), "{name}");
    }
}

#[test]
fn bash_local_variables_are_not_environment_reads() {
    let parsed = parse_source(
        "release.sh",
        br#"#!/usr/bin/env bash
arch="amd64"
readonly build_target=wasm32
PORT="${PORT:-8080}"
export TOKEN="$GITHUB_TOKEN"
read answer
for file in a b; do
    echo "$file"
done
echo "$arch $build_target $PORT $TOKEN $answer $HOME"
"#,
        Language::Bash,
    )
    .unwrap();

    let keys = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .map(|item| item.label.as_str())
        .collect::<BTreeSet<_>>();

    // `PORT` keeps its read because the script only falls back to a default;
    // `GITHUB_TOKEN` and `HOME` are never assigned here.
    assert_eq!(
        keys,
        BTreeSet::from(["GITHUB_TOKEN", "HOME", "PORT"]),
        "locals leaked into environment reads"
    );

    // The surviving PORT read still carries its fallback.
    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");
    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("8080")
    );
}

#[test]
fn a_rust_error_is_named_by_its_own_message() {
    let parsed = parse_source(
        "main.rs",
        br#"fn demo(root: &Path, path: &str) {
    let dir = root.join("src").unwrap();
    let text = read(path).expect("failed to read");
    let plan = query_graph(path).map_err(|error| error.to_string())?;
    panic!("boom");
}
"#,
        Language::Rust,
    )
    .unwrap();

    let errors = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .map(|item| item.label.as_str())
        .collect::<BTreeSet<_>>();

    // `unwrap()` carries no message, so the fact is named after the call that
    // can fail — not after `"src"`, a string that merely sits inside the
    // expression and named 186 error facts in this repository.
    assert!(errors.contains("root.join"), "got {errors:?}");
    assert!(!errors.contains("src"), "got {errors:?}");
    // `expect` and `panic!` do carry a message, and that is their identity.
    assert!(errors.contains("failed to read"), "got {errors:?}");
    assert!(errors.contains("boom"), "got {errors:?}");
    // `?` is named after what it propagates from.
    assert!(errors.contains("query_graph"), "got {errors:?}");
}

#[test]
fn a_definition_records_whether_it_faces_outwards() {
    fn visibility<'a>(parsed: &'a ParsedFile, label: &str) -> Option<&'a str> {
        parsed
            .items
            .iter()
            .find(|item| item.kind == ParsedItemKind::Function && item.label == label)
            .unwrap_or_else(|| panic!("missing {label}"))
            .metadata
            .get("visibility")
            .map(String::as_str)
    }

    let rust = parse_source(
        "lib.rs",
        b"pub fn exported() {}\npub(crate) fn shared() {}\nfn hidden() {}\n",
        Language::Rust,
    )
    .unwrap();
    assert_eq!(visibility(&rust, "exported"), Some("public"));
    assert_eq!(visibility(&rust, "shared"), Some("crate"));
    assert_eq!(visibility(&rust, "hidden"), Some("private"));

    // Go states it with a capital letter, and the compiler enforces it.
    let go = parse_source(
        "app.go",
        b"package app\n\nfunc Exported() {}\n\nfunc hidden() {}\n",
        Language::Go,
    )
    .unwrap();
    assert_eq!(visibility(&go, "Exported"), Some("public"));
    assert_eq!(visibility(&go, "hidden"), Some("private"));

    let ts = parse_source(
        "index.ts",
        b"export function exported() {}\nfunction hidden() {}\n",
        Language::TypeScript,
    )
    .unwrap();
    assert_eq!(visibility(&ts, "exported"), Some("public"));
    assert_eq!(visibility(&ts, "hidden"), Some("private"));

    let python = parse_source(
        "app.py",
        b"def public_api():\n    pass\n\ndef _internal():\n    pass\n",
        Language::Python,
    )
    .unwrap();
    assert_eq!(visibility(&python, "public_api"), Some("public"));
    assert_eq!(visibility(&python, "_internal"), Some("private"));
}

#[test]
fn type_arguments_are_not_part_of_what_is_called() {
    let csharp = parse_source(
        "reader.cs",
        br#"class Reader {
    void Run() {
        JsonConvert.DeserializeObject<Dictionary<string, int>>(text);
        Handle<Foo>(value);
    }
}
"#,
        Language::CSharp,
    )
    .unwrap();
    let labels = |parsed: &ParsedFile| {
        parsed
            .items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::Call)
            .map(|item| item.label.clone())
            .collect::<BTreeSet<_>>()
    };
    let names = labels(&csharp);

    // Every instantiation names the same callee; keeping the type arguments
    // minted a node per instantiation and stopped the label matching the
    // method's declaration.
    assert!(
        names.contains("JsonConvert.DeserializeObject"),
        "got {names:?}"
    );
    assert!(names.contains("Handle"), "got {names:?}");
    assert!(
        names.iter().all(|label| !label.contains('<')),
        "no type arguments should survive: {names:?}"
    );
}

#[test]
fn a_call_label_names_the_callee_not_the_expression_before_it() {
    let rust = parse_source(
        "main.rs",
        br#"fn main() {
    let text = std::fs::read_to_string("Cargo.toml").unwrap();
    let first = text
        .lines()
        .next();
    let trimmed = parts[index + 1..].join("-");
}
"#,
        Language::Rust,
    )
    .unwrap();
    let labels = |parsed: &ParsedFile| {
        parsed
            .items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::Call)
            .map(|item| item.label.clone())
            .collect::<BTreeSet<_>>()
    };
    let rust_labels = labels(&rust);

    // The receiver expression is not part of what is being called: keeping it
    // minted a node per receiver, and half of this repository's call targets
    // were such expressions.
    assert!(
        rust_labels.contains("unwrap"),
        "expected `unwrap`, got {rust_labels:?}"
    );
    assert!(
        rust_labels.contains("join"),
        "expected `join`, got {rust_labels:?}"
    );
    // A chain written across lines is one name, not three words.
    assert!(
        rust_labels.iter().all(|label| !label.contains(' ')),
        "labels must not carry whitespace: {rust_labels:?}"
    );

    let go = parse_source(
        "addr.go",
        br#"package addrs

func demo(name string) Absolute {
    return OutputValue{Name: name}.Absolute()
}
"#,
        Language::Go,
    )
    .unwrap();
    assert!(
        labels(&go).contains("Absolute"),
        "expected `Absolute`, got {:?}",
        labels(&go)
    );
}

#[test]
fn dart_methods_know_the_type_they_belong_to() {
    let parsed = parse_source(
        "client.dart",
        br#"class Client {
  void send() {}
}

mixin Retry {
  void retry() {}
}

extension Extras on Client {
  void extra() {}
}

void plain() {}
"#,
        Language::Dart,
    )
    .unwrap();

    let owner = |label: &str| {
        parsed
            .items
            .iter()
            .find(|item| item.kind == ParsedItemKind::Function && item.label == label)
            .unwrap_or_else(|| panic!("missing {label}"))
            .metadata
            .get("owner_type")
            .map(String::as_str)
    };

    assert_eq!(owner("send"), Some("Client"));
    assert_eq!(owner("retry"), Some("Retry"));
    // An extension's methods are called on the type it extends, so that type
    // owns them — `Extras` is not what a call says.
    assert_eq!(owner("extra"), Some("Client"));
    assert_eq!(owner("plain"), None);
}

#[test]
fn go_methods_know_the_type_they_belong_to() {
    let parsed = parse_source(
        "backend.go",
        br#"package backend

type Backend struct{}

func (b *Backend) Configure() error {
    return nil
}

func (b Backend) Name() string {
    return "backend"
}

func Plain() {}
"#,
        Language::Go,
    )
    .unwrap();

    let owner = |label: &str| {
        parsed
            .items
            .iter()
            .find(|item| item.kind == ParsedItemKind::Function && item.label == label)
            .unwrap_or_else(|| panic!("missing {label}"))
            .metadata
            .get("owner_type")
            .map(String::as_str)
    };

    // Go names the owner in the receiver, not in an enclosing block, so
    // walking ancestors found nothing and every Go method was ownerless.
    assert_eq!(owner("Configure"), Some("Backend"));
    assert_eq!(owner("Name"), Some("Backend"));
    assert_eq!(owner("Plain"), None, "a plain function has no owner");
}

#[test]
fn go_error_constructors_are_error_facts() {
    let parsed = parse_source(
        "backend.go",
        br#"package main

import (
    "errors"
    "fmt"
)

func load(name string) error {
    if name == "" {
        return errors.New("empty name")
    }
    if len(name) > 64 {
        return fmt.Errorf("name %q is too long", name)
    }
    pool := cache.New(name)
    if pool == nil {
        panic("no pool")
    }
    return nil
}
"#,
        Language::Go,
    )
    .unwrap();

    let errors = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .map(|item| item.label.as_str())
        .collect::<BTreeSet<_>>();

    // Go signals failure by constructing an error and returning it, so
    // construction is the error fact; `cache.New` is not one.
    assert_eq!(
        errors,
        BTreeSet::from(["empty name", "name %q is too long", "no pool"]),
        "unexpected Go error facts"
    );
}

#[test]
fn parses_go_environment_default_values() {
    let parsed = parse_source(
        "main.go",
        br#"package main

import (
    "cmp"
    "os"
)

func main() {
    port := cmp.Or(os.Getenv("PORT"), "9090")
    _ = port
}
"#,
        Language::Go,
    )
    .unwrap();

    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");

    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("9090")
    );
}

#[test]
fn parses_c_family_environment_default_values() {
    let c = parse_source(
        "main.c",
        br#"#include <stdlib.h>
int main(void) {
    const char *port = getenv("PORT") ?: "9091";
    return port ? 0 : 1;
}
"#,
        Language::C,
    )
    .unwrap();
    let cpp = parse_source(
        "main.cpp",
        br#"#include <cstdlib>
int main() {
    auto port = std::getenv("PORT") ?: "9092";
    return port ? 0 : 1;
}
"#,
        Language::Cpp,
    )
    .unwrap();

    let c_port = c
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing C PORT env read");
    let cpp_port = cpp
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing C++ PORT env read");

    assert_eq!(
        c_port.metadata.get("default_value").map(String::as_str),
        Some("9091")
    );
    assert_eq!(
        cpp_port.metadata.get("default_value").map(String::as_str),
        Some("9092")
    );
}

#[test]
fn parses_dart_symbols_effects_and_calls() {
    let parsed = parse_source(
        "lib/main.dart",
        br#"import 'package:flutter/material.dart';
part 'src/app_part.dart';

class App {}
mixin Bootable {}
extension AppExt on App {}

App buildApp() => App();

void main() {
  const port = String.fromEnvironment('PORT', defaultValue: '8080');
  final api = Platform.environment['API_URL'] ?? 'http://localhost';
  final config = rootBundle.loadString('assets/config/app.json');
  runApp(App());
  throw StateError('broken');
}
"#,
        Language::Dart,
    )
    .unwrap();

    assert!(
        parsed
            .items
            .iter()
            .any(|item| { item.kind == ParsedItemKind::Entrypoint && item.label == "main" })
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Type && item.label == "App")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Type && item.label == "Bootable")
    );
    assert!(
        parsed
            .items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::Import)
            .any(|item| item.label.contains("package:flutter/material.dart"))
    );
    assert!(
        parsed
            .items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::Import)
            .any(|item| item.label.contains("src/app_part.dart"))
    );
    assert!(parsed.items.iter().any(|item| {
        item.kind == ParsedItemKind::Call
            && item.label == "runApp"
            && item.parent.as_deref() == Some("main")
    }));
    assert!(
        parsed
            .type_references
            .iter()
            .any(|reference| reference.label == "App")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| { item.kind == ParsedItemKind::Function && item.label == "buildApp" })
    );

    let port = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "PORT")
        .expect("missing PORT env read");
    assert_eq!(
        port.metadata.get("default_value").map(String::as_str),
        Some("8080")
    );
    let api = parsed
        .items
        .iter()
        .find(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_URL")
        .expect("missing API_URL env read");
    assert_eq!(
        api.metadata.get("default_value").map(String::as_str),
        Some("http://localhost")
    );
    assert!(parsed.items.iter().any(|item| {
        item.kind == ParsedItemKind::ConfigRead && item.label == "assets/config/app.json"
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
            "main.dart",
            Language::Dart,
            "import 'package:flutter/widgets.dart';\nclass App {}\nvoid main() { runApp(App()); }\n",
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

#[test]
fn multibyte_labels_truncate_instead_of_panicking() {
    // Regression: truncate_label sliced at a fixed byte offset, panicking when
    // the cut landed inside a multi-byte char. The "a" prefix shifts every
    // two-byte Cyrillic char to an odd offset so the 120-byte cut is mid-char.
    let default = format!("a{}", "я".repeat(70));
    let source = format!("import os\nVALUE = os.getenv(\"KEY\", \"{default}\")\n");
    let parsed = parse_source("app.py", source.as_bytes(), Language::Python).unwrap();
    assert!(
        parsed
            .items
            .iter()
            .any(|item| matches!(item.kind, ParsedItemKind::EnvironmentRead)),
        "env read fact survives truncation: {:?}",
        parsed.items
    );
}

#[test]
fn strip_quotes_preserves_bare_values_and_unwraps_prefixed_literals() {
    // Bare values must survive (regression: `user` -> `ser`, `ruby` -> `y`).
    assert_eq!(strip_quotes("user".to_string()), "user");
    assert_eq!(strip_quotes("ruby".to_string()), "ruby");
    assert_eq!(strip_quotes("build".to_string()), "build");
    // Real string-literal prefixes are still unwrapped.
    assert_eq!(strip_quotes("r\"path\"".to_string()), "path");
    assert_eq!(strip_quotes("b'y'".to_string()), "y");
    assert_eq!(strip_quotes("\"plain\"".to_string()), "plain");
}

#[test]
fn deeply_nested_source_does_not_overflow_the_stack() {
    // A pathological (minified/generated) file with thousands of nesting levels
    // would overflow the stack via unbounded recursion. The depth cap must let
    // it finish. Run on a small stack so an uncapped recursion would abort.
    let depth = 8000;
    let mut source = String::from("let x = ");
    source.push_str(&"(".repeat(depth));
    source.push('0');
    source.push_str(&")".repeat(depth));
    source.push_str(";\n");

    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            parse_source("deep.js", source.as_bytes(), Language::JavaScript)
                .expect("parse deep source")
                .items
                .len()
        })
        .expect("spawn worker");
    // Joins cleanly (no abort) with the cap in place.
    let _ = handle.join().expect("worker finished without overflow");
}

#[test]
fn environment_reads_are_counted_once_per_access() {
    // Substring matching used to file 2-3 facts per physical access (every
    // enclosing call/member node whose text contained the pattern) and turn
    // every bash local into a phantom env read.
    let count = |path: &str, language: Language, source: &str| {
        parse_source(path, source.as_bytes(), language)
            .unwrap()
            .items
            .into_iter()
            .filter(|item| matches!(item.kind, ParsedItemKind::EnvironmentRead))
            .count()
    };

    assert_eq!(
        count(
            "app.py",
            Language::Python,
            "import os\nPORT = int(os.getenv(\"PORT\", \"8000\"))\n"
        ),
        1,
        "wrapped os.getenv is one read"
    );
    assert_eq!(
        count(
            "app.py",
            Language::Python,
            "import os\nHOME = os.environ[\"HOME\"]\n"
        ),
        1,
        "os.environ subscript is one read"
    );
    assert_eq!(
        count(
            "app.js",
            Language::JavaScript,
            "const port = parseInt(process.env.PORT);\n"
        ),
        1,
        "wrapped process.env access is one read"
    );
    assert_eq!(
        count(
            "run.sh",
            Language::Bash,
            "PORT=\"${PORT:-5000}\"\necho \"$PORT\"\n"
        ),
        2,
        "the expansion and the echo read count; the assignment LHS does not"
    );
    assert_eq!(
        count(
            "loop.sh",
            Language::Bash,
            "for item in a b c; do\n  echo ok\ndone\n"
        ),
        0,
        "a for-loop variable is not an env read"
    );
}

#[test]
fn ruby_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"require 'json'
module Billing
  class Invoice
    def total(items)
      key = ENV['API_KEY']
      mode = ENV.fetch('MODE', 'dev')
      if items.empty?
        raise ArgumentError, "empty"
      end
      items.each do |item|
        compute(item)
      end
      return 42
    end
  end
end
"#;
    let parsed = parse_source("invoice.rb", source.as_bytes(), Language::Ruby).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(has(ParsedItemKind::Module, "Billing"), "{:?}", parsed.items);
    assert!(has(ParsedItemKind::Type, "Invoice"));
    assert!(has(ParsedItemKind::Function, "total"));
    assert!(has(ParsedItemKind::Import, "require 'json'"));
    assert!(has(ParsedItemKind::Call, "compute"));
    assert!(
        has(ParsedItemKind::Call, "each"),
        "call label uses the method field"
    );
    let env_reads: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .collect();
    assert_eq!(env_reads.len(), 2, "{env_reads:?}");
    assert!(env_reads.iter().any(|item| item.label == "API_KEY"));
    assert!(
        env_reads
            .iter()
            .any(|item| item.metadata.get("default_value").map(String::as_str) == Some("dev")),
        "ENV.fetch default is captured: {env_reads:?}"
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Return, "return: return"));
}

#[test]
fn java_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"import java.util.List;
public class App {
    interface Runner { void run(); }
    enum Mode { DEV }
    public static void main(String[] args) {
        String key = System.getenv("API_KEY");
        if (args.length == 0) {
            throw new IllegalArgumentException("empty");
        }
        for (String a : args) {
            process(a);
        }
        return;
    }
}
"#;
    let parsed = parse_source("App.java", source.as_bytes(), Language::Java).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(has(ParsedItemKind::Type, "App"), "{:?}", parsed.items);
    assert!(has(ParsedItemKind::Type, "Runner"));
    assert!(has(ParsedItemKind::Type, "Mode"));
    assert!(
        has(ParsedItemKind::Entrypoint, "main"),
        "main is an entrypoint"
    );
    assert!(has(ParsedItemKind::Call, "process"));
    let env_reads = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .count();
    assert_eq!(env_reads, 1);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_KEY")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Loop, "loop: for"));
    assert!(has(ParsedItemKind::Import, "import java.util.List;"));
}

#[test]
fn csharp_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"using System;
namespace Billing {
    interface IRunner { void Run(); }
    class App {
        static async System.Threading.Tasks.Task Main(string[] args) {
            var key = Environment.GetEnvironmentVariable("API_KEY");
            if (args.Length == 0) {
                throw new ArgumentException("empty");
            }
            foreach (var a in args) { Process(a); }
            var x = await Fetch();
            return;
        }
    }
}
"#;
    let parsed = parse_source("App.cs", source.as_bytes(), Language::CSharp).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(has(ParsedItemKind::Module, "Billing"), "{:?}", parsed.items);
    assert!(has(ParsedItemKind::Type, "App"));
    assert!(has(ParsedItemKind::Type, "IRunner"));
    assert!(
        has(ParsedItemKind::Entrypoint, "Main"),
        "Main is an entrypoint"
    );
    assert!(has(ParsedItemKind::Call, "Process"));
    let env_reads = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .count();
    assert_eq!(env_reads, 1);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_KEY")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Loop, "loop: for"));
    assert!(has(ParsedItemKind::Async, "async: await"));
    assert!(has(ParsedItemKind::Import, "using System;"));
}

#[test]
fn kotlin_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"import java.util.Locale
object Config { }
class App {
    fun main(args: Array<String>) {
        val key = System.getenv("API_KEY")
        if (args.isEmpty()) {
            throw IllegalArgumentException("empty")
        }
        for (a in args) { process(a) }
        return
    }
}
"#;
    let parsed = parse_source("App.kt", source.as_bytes(), Language::Kotlin).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(has(ParsedItemKind::Type, "App"), "{:?}", parsed.items);
    assert!(has(ParsedItemKind::Type, "Config"));
    assert!(has(ParsedItemKind::Entrypoint, "main"));
    assert!(has(ParsedItemKind::Call, "process"));
    let env_reads = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .count();
    assert_eq!(env_reads, 1);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_KEY")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Loop, "loop: for"));
    assert!(has(ParsedItemKind::Import, "import java.util.Locale"));
}

#[test]
fn swift_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"import Foundation
protocol Runner { func run() }
class App {
    func main() {
        let key = ProcessInfo.processInfo.environment["API_KEY"]
        if key == nil {
            fatalError("empty")
        }
        for a in [1, 2] { process(a) }
        guard let k = key else { return }
        return
    }
}
"#;
    let parsed = parse_source("App.swift", source.as_bytes(), Language::Swift).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(has(ParsedItemKind::Type, "App"), "{:?}", parsed.items);
    assert!(has(ParsedItemKind::Type, "Runner"));
    assert!(has(ParsedItemKind::Entrypoint, "main"));
    assert!(has(ParsedItemKind::Function, "run"));
    assert!(has(ParsedItemKind::Call, "process"));
    let env_reads: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .collect();
    assert_eq!(env_reads.len(), 1, "{env_reads:?}");
    assert_eq!(env_reads[0].label, "API_KEY");
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error),
        "fatalError is an error fact"
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Branch, "branch: guard"));
    assert!(has(ParsedItemKind::Loop, "loop: for"));
    assert!(has(ParsedItemKind::Import, "import Foundation"));
}

#[test]
fn scala_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"import java.util.Locale
object App {
  trait Runner { def run(): Unit }
  case class Point(x: Int)
  def main(args: Array[String]): Unit = {
    val key = sys.env.get("API_KEY")
    if (args.isEmpty) {
      throw new IllegalArgumentException("empty")
    }
    for (a <- args) { process(a) }
    args.length match { case 0 => () }
    return
  }
}
"#;
    let parsed = parse_source("App.scala", source.as_bytes(), Language::Scala).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(has(ParsedItemKind::Type, "App"), "{:?}", parsed.items);
    assert!(has(ParsedItemKind::Type, "Runner"));
    assert!(has(ParsedItemKind::Type, "Point"));
    assert!(has(ParsedItemKind::Entrypoint, "main"));
    assert!(has(ParsedItemKind::Call, "process"));
    let env_reads = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .count();
    assert_eq!(env_reads, 1);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_KEY")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Branch, "branch: match"));
    assert!(has(ParsedItemKind::Loop, "loop: for"));
    assert!(has(ParsedItemKind::Import, "import java.util.Locale"));
}

#[test]
fn lua_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"local json = require("json")
local M = {}
function M.total(items)
    local key = os.getenv("API_KEY")
    if #items == 0 then
        error("empty")
    end
    for i, item in ipairs(items) do
        compute(item)
    end
    return 42
end
"#;
    let parsed = parse_source("mod.lua", source.as_bytes(), Language::Lua).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(
        has(ParsedItemKind::Function, "M.total"),
        "{:?}",
        parsed.items
    );
    assert!(has(ParsedItemKind::Import, "require(\"json\")"));
    assert!(has(ParsedItemKind::Call, "compute"));
    let env_reads = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .count();
    assert_eq!(env_reads, 1);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_KEY")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Loop, "loop: for"));
    assert!(has(ParsedItemKind::Return, "return: return"));
}

#[test]
fn elixir_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"defmodule Billing.Invoice do
  import Enum
  def total(items) do
    key = System.get_env("API_KEY")
    if items == [] do
      raise ArgumentError, "empty"
    end
    Enum.each(items, fn item -> compute(item) end)
  end
end
"#;
    let parsed = parse_source("invoice.ex", source.as_bytes(), Language::Elixir).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(
        has(ParsedItemKind::Module, "Billing.Invoice"),
        "{:?}",
        parsed.items
    );
    assert!(has(ParsedItemKind::Function, "total"));
    assert!(has(ParsedItemKind::Call, "Enum.each"));
    assert!(has(ParsedItemKind::Call, "compute"));
    let env_reads = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .count();
    assert_eq!(env_reads, 1);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_KEY")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error),
        "raise is an error fact"
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Import),
        "import Enum lands as an import fact"
    );
}

#[test]
fn zig_adapter_extracts_symbols_calls_effects_and_control_flow() {
    let source = r#"const std = @import("std");
pub fn main() !void {
    const key = std.posix.getenv("API_KEY");
    if (key == null) {
        @panic("empty");
    }
    for (0..3) |i| {
        process(i);
    }
    return;
}
"#;
    let parsed = parse_source("main.zig", source.as_bytes(), Language::Zig).unwrap();
    let has = |kind: ParsedItemKind, label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(
        has(ParsedItemKind::Entrypoint, "main"),
        "{:?}",
        parsed.items
    );
    assert!(has(ParsedItemKind::Call, "process"));
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Import && item.label.contains("@import")),
        "@import lands as an import fact"
    );
    let env_reads = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .count();
    assert_eq!(env_reads, 1);
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::EnvironmentRead && item.label == "API_KEY")
    );
    assert!(
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error),
        "@panic is an error fact"
    );
    assert!(has(ParsedItemKind::Branch, "branch: if"));
    assert!(has(ParsedItemKind::Loop, "loop: for"));
    assert!(has(ParsedItemKind::Return, "return: return"));
}

#[test]
fn a_zig_function_fails_with_an_error_value() {
    // `error.NotFound` is how a Zig function fails, and zls returns one
    // 139 times; only `@panic` was recorded before.
    let source = r#"pub fn find(key: []const u8) !u32 {
    if (key.len == 0) return error.EmptyKey;
    const MyError = error{ Missing };
    _ = MyError;
    return 1;
}
"#;
    let parsed = parse_source("find.zig", source.as_bytes(), Language::Zig).unwrap();
    let errors: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(errors.len(), 1, "items: {:?}", parsed.items);
    assert!(
        errors[0].contains("EmptyKey"),
        "the fact names the error it fails with: {errors:?}"
    );
}

#[test]
fn an_r_package_raises_with_abort() {
    // `stop()` is base R; dplyr writes `abort()` and `cli::cli_abort()`
    // 218 times against 26 `stop()`.
    let source = r#"check <- function(x) {
  if (x < 0) abort("negative")
  if (x > 9) cli::cli_abort("too big")
  if (is.na(x)) stop("missing")
  x
}
"#;
    let parsed = parse_source("check.R", source.as_bytes(), Language::R).unwrap();
    let errors = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .count();
    assert_eq!(errors, 3, "items: {:?}", parsed.items);
}

#[test]
fn a_swift_function_throws_and_a_caller_tries() {
    // Alamofire writes 62 `throw`s and 206 `try`s; only the fatalError
    // family was recorded. `try?` turns the failure into nil, so it ends
    // the error path rather than continuing it.
    let source = r#"func load(_ path: String) throws -> Int {
    if path.isEmpty { throw LoadError.empty }
    let value = try parse(path)
    let ignored = try? parse(path)
    _ = ignored
    return value
}
"#;
    let parsed = parse_source("load.swift", source.as_bytes(), Language::Swift).unwrap();
    let errors = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .count();
    assert_eq!(errors, 2, "items: {:?}", parsed.items);
}

#[test]
fn a_haskell_computation_gives_up_with_fail() {
    // shellcheck raises with `fail` 43 times and `error` five; only
    // `error` was recorded.
    let source = r#"parse :: String -> IO Int
parse s = do
  fail "bad input"

check :: Int -> Int
check x = error "negative"
"#;
    let parsed = parse_source("parse.hs", source.as_bytes(), Language::Haskell).unwrap();
    let errors = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .count();
    assert_eq!(errors, 2, "items: {:?}", parsed.items);
}

#[test]
fn a_nix_module_guards_itself_with_assert() {
    // `assert cond; body` stops the evaluation, the role Lua's `assert`
    // plays; home-manager guards its modules with 116 of them.
    let source = r#"{ config, lib }:
assert config.enable;
{
  value = if config.broken then throw "bad" else 1;
}
"#;
    let parsed = parse_source("module.nix", source.as_bytes(), Language::Nix).unwrap();
    let errors = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .count();
    assert_eq!(errors, 2, "items: {:?}", parsed.items);
}

#[test]
fn an_r_function_returns_by_calling_return() {
    // dplyr writes `return(x)` 171 times; the node is an ordinary call,
    // so the kind alone cannot see it.
    let source = r#"pick <- function(x) {
  if (x < 0) return(0)
  for (i in seq_len(x)) print(i)
  x
}
"#;
    let parsed = parse_source("pick.R", source.as_bytes(), Language::R).unwrap();
    let kinds: Vec<ParsedItemKind> = parsed
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                ParsedItemKind::Return | ParsedItemKind::Branch | ParsedItemKind::Loop
            )
        })
        .map(|item| item.kind)
        .collect();
    assert!(
        kinds.contains(&ParsedItemKind::Return),
        "items: {:?}",
        parsed.items
    );
    assert!(kinds.contains(&ParsedItemKind::Branch));
    assert!(kinds.contains(&ParsedItemKind::Loop));
}

#[test]
fn a_zig_container_is_a_type() {
    // zls declares 260 types as `const Name = struct { ... }`; the graph
    // had no Zig types at all.
    let source = r#"pub const Server = struct {
    port: u16,
};
const Mode = enum { fast, slow };
const Value = union { a: u8, b: u16 };
const count: u32 = 3;
"#;
    let parsed = parse_source("server.zig", source.as_bytes(), Language::Zig).unwrap();
    let types: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Type)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(
        types,
        vec!["Server", "Mode", "Value"],
        "items: {:?}",
        parsed.items
    );
}

#[test]
fn an_erlang_record_and_type_are_types() {
    // cowboy declares 23 records and 64 types; a reader calls them
    // `state` and `opts`, without the parameter list.
    let source = r#"-module(demo).
-record(state, {socket, timeout = 5000}).
-type opts() :: map().
-opaque handle() :: reference().
"#;
    let parsed = parse_source("demo.erl", source.as_bytes(), Language::Erlang).unwrap();
    let types: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Type)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(
        types,
        vec!["state", "opts", "handle"],
        "items: {:?}",
        parsed.items
    );
}

#[test]
fn an_elixir_struct_is_named_by_its_module() {
    // `defstruct` names nothing; the struct is the module that declares
    // it, which is how Elixir refers to it (`%Ecto.Changeset{}`). Ecto
    // writes 25 and every one was dropped for want of a name.
    let source = r#"defmodule Ecto.Changeset do
  defstruct [:data, :changes]

  def new, do: %__MODULE__{}
end
"#;
    let parsed = parse_source("changeset.ex", source.as_bytes(), Language::Elixir).unwrap();
    let types: Vec<&str> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Type)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(types, vec!["Ecto.Changeset"], "items: {:?}", parsed.items);
}

#[test]
fn calls_on_literals_are_labeled_by_method_not_by_the_literal() {
    // `"x".to_string()` used to produce a call target carrying the whole
    // literal, so each distinct literal minted its own placeholder node
    // (1890 of them for to_string alone on this repository).
    let source = r#"fn build() -> String {
    let a = "low".to_string();
    let b = "medium".to_string();
    let c = "abc".len();
    format!("{a}{b}{c}")
}
"#;
    let parsed = parse_source("build.rs", source.as_bytes(), Language::Rust).unwrap();
    let call_labels: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Call)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        call_labels.contains(&"to_string"),
        "literal receivers collapse to the method name: {call_labels:?}"
    );
    assert!(call_labels.contains(&"len"));
    assert!(
        !call_labels.iter().any(|label| label.contains('"')),
        "no call label carries a string literal: {call_labels:?}"
    );
    // Ordinary paths keep their qualification.
    let qualified = parse_source(
        "q.rs",
        b"fn run() { let v = Vec::new(); std::env::var(\"K\"); }\n",
        Language::Rust,
    )
    .unwrap();
    let qualified_labels: Vec<_> = qualified
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Call)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        qualified_labels.contains(&"Vec::new"),
        "qualified paths are unchanged: {qualified_labels:?}"
    );
}

#[test]
fn dart_environment_reads_are_counted_once_per_access() {
    // Substring matching fired on the inner member_expression and on the
    // enclosing call, filing several facts per physical read.
    let source = r#"void main() {
  const key = String.fromEnvironment('API_KEY', defaultValue: 'dev');
  final home = Platform.environment['HOME'];
  final port = int.fromEnvironment('PORT');
  helper(key, home, port);
}
"#;
    let parsed = parse_source("main.dart", source.as_bytes(), Language::Dart).unwrap();
    let reads: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
        .collect();
    assert_eq!(reads.len(), 3, "one fact per access: {reads:?}");
    let labels: Vec<_> = reads.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"API_KEY"), "{labels:?}");
    assert!(labels.contains(&"HOME"), "{labels:?}");
    assert!(labels.contains(&"PORT"), "{labels:?}");
    assert!(
        reads
            .iter()
            .any(|item| item.metadata.get("default_value").map(String::as_str) == Some("dev")),
        "fromEnvironment defaultValue is captured: {reads:?}"
    );
}

#[test]
fn haskell_ocaml_julia_adapters_extract_core_facts() {
    struct Case {
        path: &'static str,
        language: Language,
        source: &'static str,
        expected: Vec<(ParsedItemKind, &'static str)>,
    }

    let cases = vec![
        Case {
            path: "Main.hs",
            language: Language::Haskell,
            source: "module Main where\nimport Data.List\ndata Shape = Circle Double\nmain = run 1\nrun k = if k > 0 then error \"bad\" else getEnv \"HOME\"\n",
            expected: vec![
                (ParsedItemKind::Type, "Shape"),
                (ParsedItemKind::Entrypoint, "main"),
                (ParsedItemKind::Function, "run"),
                (ParsedItemKind::Call, "run"),
                (ParsedItemKind::Branch, "branch: if"),
            ],
        },
        Case {
            path: "main.ml",
            language: Language::OCaml,
            source: "open Printf\ntype shape = Circle of float\nlet run k =\n  if k > 0 then failwith \"bad\";\n  Sys.getenv \"HOME\"\n",
            expected: vec![
                (ParsedItemKind::Type, "shape"),
                (ParsedItemKind::Function, "run"),
                (ParsedItemKind::Branch, "branch: if"),
            ],
        },
        Case {
            path: "demo.jl",
            language: Language::Julia,
            source: "module Demo\nstruct Shape\n  r::Float64\nend\nfunction run(k)\n  h = ENV[\"HOME\"]\n  if k > 0\n    throw(ErrorException(\"bad\"))\n  end\n  for i in 1:3\n    println(i)\n  end\n  return h\nend\nend\n",
            expected: vec![
                (ParsedItemKind::Module, "Demo"),
                (ParsedItemKind::Type, "Shape"),
                (ParsedItemKind::Function, "run"),
                (ParsedItemKind::Call, "println"),
                (ParsedItemKind::Branch, "branch: if"),
                (ParsedItemKind::Loop, "loop: for"),
                (ParsedItemKind::Return, "return: return"),
            ],
        },
    ];

    for case in cases {
        let Case {
            path,
            language,
            source,
            expected,
        } = case;
        let parsed = parse_source(path, source.as_bytes(), language).unwrap();
        for (kind, label) in expected {
            assert!(
                parsed
                    .items
                    .iter()
                    .any(|item| item.kind == kind && item.label == label),
                "{language}: missing {kind:?} `{label}` in {:?}",
                parsed
                    .items
                    .iter()
                    .map(|item| (item.kind, item.label.as_str()))
                    .collect::<Vec<_>>()
            );
        }
        // Every language records the env read and the error construct.
        assert_eq!(
            parsed
                .items
                .iter()
                .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
                .count(),
            1,
            "{language}: exactly one env read: {:?}",
            parsed.items
        );
        assert!(
            parsed
                .items
                .iter()
                .any(|item| item.kind == ParsedItemKind::Error),
            "{language}: error construct recorded"
        );
    }
}

#[test]
fn erlang_nix_and_r_adapters_extract_core_facts() {
    // Erlang: the name lives on the function clause; `mod:fun(..)` is a remote
    // node wrapping an inner call, and must count once.
    let erlang = parse_source(
        "demo.erl",
        b"-module(demo).\nmain() ->\n    H = os:getenv(\"HOME\"),\n    case H of\n        false -> throw(bad);\n        _ -> H\n    end.\n",
        Language::Erlang,
    )
    .unwrap();
    let has = |items: &[ParsedItem], kind: ParsedItemKind, label: &str| {
        items
            .iter()
            .any(|item| item.kind == kind && item.label == label)
    };
    assert!(
        has(&erlang.items, ParsedItemKind::Module, "demo"),
        "{:?}",
        erlang.items
    );
    assert!(has(&erlang.items, ParsedItemKind::Entrypoint, "main"));
    assert!(has(&erlang.items, ParsedItemKind::Branch, "branch: case"));
    assert_eq!(
        erlang
            .items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
            .count(),
        1,
        "one env read for os:getenv: {:?}",
        erlang.items
    );
    assert!(
        erlang
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );

    // Nix: a binding whose value is a lambda is the named callable.
    let nix = parse_source(
        "default.nix",
        b"let\n  run = x: if x == \"\" then throw \"bad\" else builtins.getEnv \"HOME\";\nin run \"\"\n",
        Language::Nix,
    )
    .unwrap();
    assert!(
        has(&nix.items, ParsedItemKind::Function, "run"),
        "{:?}",
        nix.items
    );
    assert!(has(&nix.items, ParsedItemKind::Branch, "branch: if"));
    assert_eq!(
        nix.items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
            .count(),
        1,
        "one env read for builtins.getEnv: {:?}",
        nix.items
    );
    assert!(
        nix.items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );

    // R: functions are assignments of a lambda; library() is an import.
    let r = parse_source(
        "run.R",
        b"library(stats)\nrun <- function(k) {\n  h <- Sys.getenv(\"HOME\")\n  if (k > 0) stop(\"bad\")\n  for (i in 1:3) print(i)\n  h\n}\n",
        Language::R,
    )
    .unwrap();
    assert!(
        has(&r.items, ParsedItemKind::Function, "run"),
        "{:?}",
        r.items
    );
    assert!(has(&r.items, ParsedItemKind::Call, "print"));
    assert!(has(&r.items, ParsedItemKind::Branch, "branch: if"));
    assert!(has(&r.items, ParsedItemKind::Loop, "loop: for"));
    assert!(
        r.items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Import),
        "library() is an import: {:?}",
        r.items
    );
    assert_eq!(
        r.items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
            .count(),
        1
    );
    assert!(
        r.items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Error)
    );
}

#[test]
fn haskell_type_signatures_do_not_become_functions() {
    // The `function` kind spells both a definition and a function *type*
    // (`Token -> m ()`), and the type has no `name` field — so its first type
    // identifier used to be recorded as a function (228 phantom functions on
    // shellcheck, `Token` x108 among them).
    let source = "module Demo where\n\
                  analyze :: (Token -> m ()) -> Token -> m Token\n\
                  analyze f t = f t\n\
                  run :: String -> String\n\
                  run s = s\n";
    let parsed = parse_source("Demo.hs", source.as_bytes(), Language::Haskell).unwrap();
    let functions: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                ParsedItemKind::Function | ParsedItemKind::Entrypoint
            )
        })
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        functions.contains(&"analyze") && functions.contains(&"run"),
        "real definitions are still extracted: {functions:?}"
    );
    for type_name in ["Token", "String", "m"] {
        assert!(
            !functions.contains(&type_name),
            "type `{type_name}` must not be a function: {functions:?}"
        );
    }
}

#[test]
fn a_keyword_token_is_not_a_declaration() {
    // Kotlin's grammar names the `import` keyword the same as the statement
    // around it, and the walk reached both: okio filed 2183 import facts
    // whose whole text was the word `import`.
    let source = "package okio.internal\n\nimport okio.Buffer\nimport okio.ByteString\n\nfun size(buffer: Buffer): Long = buffer.size\n";
    let parsed = parse_source("demo.kt", source.as_bytes(), Language::Kotlin).unwrap();
    let imports: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Import)
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(
        imports,
        vec!["import okio.Buffer", "import okio.ByteString"],
        "one fact per import, and each says what it imports"
    );
}

#[test]
fn a_lua_assert_is_named_by_what_it_guards() {
    // kong writes `assert(client:send { method = "GET" })` and filed 1369
    // failure paths called `GET`; `error("msg")` does carry its message.
    let source = "local function fetch(client)\n  local res = assert(client:send { method = \"GET\", path = \"/\" })\n  if not res then\n    error(\"no response\")\n  end\n  return res\nend\n";
    let parsed = parse_source("demo.lua", source.as_bytes(), Language::Lua).unwrap();
    let errors: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        errors
            .iter()
            .any(|label| label.starts_with("assert(client:send")),
        "the guard is named by what it guards: {errors:?}"
    );
    assert!(
        !errors.iter().any(|label| label.trim() == "GET"),
        "and not by an argument inside it: {errors:?}"
    );
    assert!(
        errors.iter().any(|label| label.contains("no response")),
        "error() still carries its message: {errors:?}"
    );
}

#[test]
fn a_swift_try_is_named_by_what_it_calls() {
    // The string inside `try AssertParse(Foo.self, "--name value")` is an
    // argument, and swift-argument-parser's tests filed a hundred failure
    // paths called `--name` and `--foo`.
    let source = "func check() throws {\n  try AssertParse(Foo.self, \"--name value\")\n  throw ParserError.invalidValue\n}\n";
    let parsed = parse_source("demo.swift", source.as_bytes(), Language::Swift).unwrap();
    let errors: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        errors
            .iter()
            .any(|label| label.starts_with("try AssertParse")),
        "a propagating try is named by the call: {errors:?}"
    );
    assert!(
        !errors.iter().any(|label| label.trim() == "--name value"),
        "and not by an argument: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|label| label.contains("ParserError.invalidValue")),
        "a throw still names the error: {errors:?}"
    );
}

#[test]
fn a_cpp_throw_macro_is_a_failure_path_and_an_assertion_is_not() {
    // json writes `JSON_THROW(...)` rather than the keyword, and spdlog
    // `SPDLOG_THROW(...)`; a test framework's assertion about throwing is
    // not a failure path in the code under test.
    let source = "void parse(int value) {\n  if (value < 0) {\n    JSON_THROW(out_of_range::create(401, \"bad value\"));\n  }\n  if (value == 0) {\n    throw std::runtime_error(\"zero\");\n  }\n}\n\nvoid check() {\n  CHECK_THROWS_AS(parse(-1), out_of_range);\n  REQUIRE_THROWS(parse(0));\n}\n";
    let parsed = parse_source("demo.cpp", source.as_bytes(), Language::Cpp).unwrap();
    let errors: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Error)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        errors.iter().any(|label| label.contains("bad value")),
        "the macro throws: {errors:?}"
    );
    assert!(
        errors.iter().any(|label| label.contains("zero")),
        "the keyword still throws: {errors:?}"
    );
    assert!(
        !errors
            .iter()
            .any(|label| label.contains("CHECK_THROWS") || label.contains("REQUIRE_THROWS")),
        "an assertion about throwing is not one: {errors:?}"
    );
}

#[test]
fn julia_methods_keep_the_module_they_extend() {
    // `function Base.names(df)` defines a method of Base.names, not of Base.
    // DataFrames.jl labelled 536 of its methods `Base` before this.
    let source = "Base.names(df::Frame, cols::Colon=:) = names(index(df))\n\
                  function Base.getindex(df::Frame, i) where {T}\n  i\nend\n\
                  function Tables.columns(df::Frame)\n  df\nend\n\
                  function plain(x)\n  x\nend\n";
    let parsed = parse_source("demo.jl", source.as_bytes(), Language::Julia).unwrap();
    let functions: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Function)
        .map(|item| item.label.as_str())
        .collect();
    for expected in ["Base.names", "Base.getindex", "Tables.columns", "plain"] {
        assert!(
            functions.contains(&expected),
            "`{expected}` is a definition label: {functions:?}"
        );
    }
    assert!(
        !functions.contains(&"Base"),
        "the extended module is not a function: {functions:?}"
    );
}

#[test]
fn julia_short_function_definitions_are_definitions_not_calls() {
    // `square(x) = x * x` parses as an assignment whose left side is a call
    // expression. It used to be no definition at all, and its left side was
    // recorded as a call to the function being defined.
    let source = "square(x) = x * x\n\
                  nrow(df::Frame) = size(df, 1)\n\
                  function full(x)\n  square(x)\nend\n";
    let parsed = parse_source("demo.jl", source.as_bytes(), Language::Julia).unwrap();
    let functions: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Function)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        functions.contains(&"square") && functions.contains(&"nrow") && functions.contains(&"full"),
        "short and long definitions are both extracted: {functions:?}"
    );

    let calls: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Call)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        calls.contains(&"size"),
        "calls in a short definition's body are still extracted: {calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|label| **label == "nrow").count(),
        0,
        "the definition head is not a call to itself: {calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|label| **label == "square").count(),
        1,
        "only the real call inside `full` counts: {calls:?}"
    );
}

#[test]
fn elixir_guarded_and_one_line_definitions_are_extracted() {
    // `def foo(x) when guard` puts the head on the left of the `when`
    // operator; that shape used to yield no name, dropping the definition
    // entirely (490 of them on ecto). defguard was not classified at all.
    let source = "defmodule M do\n\
                  \x20 def one, do: :ok\n\
                  \x20 def two(x) when is_list(x), do: x\n\
                  \x20 def three(x) do\n    x\n  end\n\
                  \x20 defguard is_ok(v) when v == :ok\n\
                  end\n";
    let parsed = parse_source("m.ex", source.as_bytes(), Language::Elixir).unwrap();
    let functions: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Function)
        .map(|item| item.label.as_str())
        .collect();
    for name in ["one", "two", "three", "is_ok"] {
        assert!(
            functions.contains(&name),
            "`{name}` must be extracted: {functions:?}"
        );
    }
}

#[test]
fn javascript_function_expressions_are_named_by_their_binding() {
    // Most modern JS declares functions as arrow/function expressions bound to
    // a name. They used to be invisible — no function fact, and no calls from
    // their bodies either, since nothing established an enclosing function.
    let source = "const handler = async (req) => { doWork(req); };\n\
                  let plain = function (x) { return helper(x); };\n\
                  const obj = { method: (a) => compute(a) };\n\
                  api.send = () => transmit();\n\
                  items.map((x) => x * 2);\n";
    let parsed = parse_source("app.js", source.as_bytes(), Language::JavaScript).unwrap();
    let functions: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Function)
        .map(|item| item.label.as_str())
        .collect();
    for name in ["handler", "plain", "method", "api.send"] {
        assert!(
            functions.contains(&name),
            "`{name}` is bound to a name and must be extracted: {functions:?}"
        );
    }
    assert_eq!(
        functions.len(),
        4,
        "the anonymous map callback stays anonymous: {functions:?}"
    );

    let calls: Vec<_> = parsed
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Call)
        .map(|item| item.label.as_str())
        .collect();
    for call in ["doWork", "helper", "compute", "transmit"] {
        assert!(
            calls.contains(&call),
            "calls inside function expressions are extracted: {calls:?}"
        );
    }
}

#[test]
fn an_invoked_literal_records_no_call_because_it_has_no_name() {
    // `go func() { … }()` reduced to a call to a function named `func` —
    // 179 of them in terraform — and a JavaScript IIFE put its whole body
    // in the label. Neither names anything, and a graph node labelled with
    // a block of source is noise.
    let go = parse_source(
        "worker.go",
        b"package main\n\nfunc run() {\n\tgo func() { work() }()\n\tdefer func() { done() }()\n}\n",
        Language::Go,
    )
    .unwrap();
    let go_calls: Vec<&str> = go
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Call)
        .map(|item| item.label.as_str())
        .collect();
    assert!(
        !go_calls.contains(&"func"),
        "the literal is not a callable name, got {go_calls:?}"
    );
    assert!(
        go_calls.contains(&"work") && go_calls.contains(&"done"),
        "the literal's own body still calls, got {go_calls:?}"
    );

    let js = parse_source(
        "app.js",
        b"export function boot() {\n  (function () { start(); })();\n  (() => { stop(); })();\n}\n",
        Language::JavaScript,
    )
    .unwrap();
    for item in js
        .items
        .iter()
        .filter(|item| item.kind == ParsedItemKind::Call)
    {
        assert!(
            !item.label.contains(['{', '}', ';', '\n']),
            "a call label holds a name, not source: {:?}",
            item.label
        );
    }
}

#[test]
fn a_lua_binding_names_the_function_it_holds() {
    // Lua modules are written as tables of anonymous functions. Without the
    // binding's name they had no definition to belong to, and kong's 12000
    // calls inside them were credited to the file that loaded them.
    let source = r#"local M = {}

M.handle = function(request)
    return route(request)
end

local handlers = {
    init = function()
        return setup()
    end,
}

register(function()
    return orphaned()
end)

return M
"#;
    let parsed = parse_source("mod.lua", source.as_bytes(), Language::Lua).unwrap();
    let function = |label: &str| {
        parsed
            .items
            .iter()
            .any(|item| item.kind == ParsedItemKind::Function && item.label == label)
    };
    assert!(function("M.handle"), "{:?}", parsed.items);
    assert!(function("init"), "{:?}", parsed.items);

    let parent_of = |label: &str| {
        parsed
            .items
            .iter()
            .find(|item| item.kind == ParsedItemKind::Call && item.label == label)
            .map(|item| item.parent.clone())
    };
    assert_eq!(parent_of("route"), Some(Some("M.handle".to_string())));
    assert_eq!(parent_of("setup"), Some(Some("init".to_string())));
    // The callback handed to `register` is named by nothing, and it runs
    // when something invokes it rather than when the file loads, so its
    // body belongs to neither a definition nor the file.
    assert_eq!(parent_of("orphaned"), None);
}

#[test]
fn haskell_data_constructors_are_the_functions_they_are() {
    // `T_Literal :: Id -> String -> Token` is applied like any function,
    // but only the type it builds was recorded, so every application
    // pointed at nothing. The names in a `deriving` clause are not
    // constructors and must not be picked up with them.
    let source = r#"module T where

data Token = T_Literal Id String | T_Word Id [Token]
  deriving (Show, Eq)

newtype Wrapper = Wrapper { unwrap :: Int }

build :: Id -> Token
build i = T_Literal i "x"
"#;
    let parsed = parse_source("T.hs", source.as_bytes(), Language::Haskell).unwrap();
    let constructor = |label: &str, owner: &str| {
        parsed.items.iter().any(|item| {
            item.kind == ParsedItemKind::Function
                && item.label == label
                && item.metadata.get("owner_type").map(String::as_str) == Some(owner)
        })
    };
    assert!(constructor("T_Literal", "Token"), "{:?}", parsed.items);
    assert!(constructor("T_Word", "Token"));
    assert!(constructor("Wrapper", "Wrapper"), "a newtype has one too");

    for derived in ["Show", "Eq"] {
        assert!(
            !parsed
                .items
                .iter()
                .any(|item| item.kind == ParsedItemKind::Function && item.label == derived),
            "{derived} is a derived class, not a constructor"
        );
    }
}

#[test]
fn an_environment_read_is_named_only_when_the_name_is_known() {
    // A shell's `"${1:-}"` reads the script's own arguments: terraform and
    // redis were recorded as reading variables called `1` and `0`, 581
    // reads in all. And a key that is computed has no name to give, so
    // `os.Getenv(envLogFile)` went into the graph as though a variable
    // were called that.
    let shell = parse_source(
        "run.sh",
        b"VERSION=\"${1:-}\"\nHOME_DIR=\"$HOME\"\nTOKEN=\"${API_TOKEN:-none}\"\necho \"$0 $# $@\"\n",
        Language::Bash,
    )
    .unwrap();
    let read_labels = |parsed: &ParsedFile| -> Vec<String> {
        parsed
            .items
            .iter()
            .filter(|item| item.kind == ParsedItemKind::EnvironmentRead)
            .map(|item| item.label.clone())
            .collect()
    };
    let shell_reads = read_labels(&shell);
    assert!(
        shell_reads.contains(&"HOME".to_string()) && shell_reads.contains(&"API_TOKEN".to_string()),
        "real variables are still read, got {shell_reads:?}"
    );
    for argument in ["1", "0", "#", "@"] {
        assert!(
            !shell_reads.contains(&argument.to_string()),
            "`${argument}` is an argument, not the environment: {shell_reads:?}"
        );
    }

    let go = parse_source(
        "main.go",
        b"package main\n\nimport \"os\"\n\nfunc read() (string, string) {\n\treturn os.Getenv(\"TF_LOG\"), os.Getenv(envLogFile)\n}\n",
        Language::Go,
    )
    .unwrap();
    let go_reads = read_labels(&go);
    assert!(go_reads.contains(&"TF_LOG".to_string()), "{go_reads:?}");
    assert!(
        go_reads.contains(&"<computed name>".to_string()),
        "a computed key has no name to give: {go_reads:?}"
    );
    let computed = go
        .items
        .iter()
        .find(|item| item.label == "<computed name>")
        .expect("the computed read is recorded");
    assert_eq!(
        computed.metadata.get("key_expression").map(String::as_str),
        Some("os.Getenv(envLogFile)"),
        "the expression is kept on the fact"
    );
}

#[test]
fn a_function_like_macro_is_a_definition_the_code_calls() {
    // redis calls `serverAssert`, `UNUSED` and `serverLog` thousands of
    // times and defines every one of them with `#define`. Those calls had
    // nothing to point at: 8677 unresolved C calls across the corpora name
    // a macro the project defines. An object-like `#define LIMIT 10` is a
    // value rather than something to call, and stays out.
    let parsed = parse_source(
        "server.c",
        b"#define UNUSED(V) ((void) V)\n#define LIMIT 10\n#define serverAssert(x) do { if (!(x)) panic(); } while (0)\n\nint main(void) {\n    UNUSED(1);\n    serverAssert(1);\n    return LIMIT;\n}\n",
        Language::C,
    )
    .unwrap();
    let functions: Vec<&ParsedItem> = parsed
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                ParsedItemKind::Function | ParsedItemKind::Entrypoint
            )
        })
        .collect();
    let labels: Vec<&str> = functions.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"UNUSED") && labels.contains(&"serverAssert"),
        "{labels:?}"
    );
    assert!(
        !labels.contains(&"LIMIT"),
        "an object-like macro is a value, not a callable: {labels:?}"
    );
    let macro_item = functions
        .iter()
        .find(|item| item.label == "UNUSED")
        .expect("the macro is recorded");
    assert_eq!(
        macro_item
            .metadata
            .get("definition_form")
            .map(String::as_str),
        Some("macro"),
        "a reader can tell a macro from a function"
    );
}
