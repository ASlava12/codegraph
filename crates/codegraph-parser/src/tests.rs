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

    assert_eq!(adapters.len(), 11);
    assert_eq!(
        languages,
        BTreeSet::from([
            "bash",
            "c",
            "cpp",
            "dart",
            "go",
            "javascript",
            "php",
            "python",
            "rust",
            "tsx",
            "typescript",
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
