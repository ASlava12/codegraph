//! Local import/include target resolution across languages, plus shared
//! path normalization utilities.

use std::collections::BTreeSet;

use codegraph_parser::Language;

#[allow(unused_imports)]
use crate::*;

pub(crate) fn local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    cmake_include_dirs: &[String],
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    match language {
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            js_local_import_target(source_label, import_label)
        }
        Language::Python => python_local_import_target(source_label, import_label),
        Language::C | Language::Cpp => {
            c_local_import_target(source_label, import_label, cmake_include_dirs)
        }
        Language::Php => php_local_import_target(source_label, import_label),
        Language::Bash => bash_local_import_target(source_label, import_label),
        Language::Rust => rust_local_import_target(source_label, import_label),
        Language::Go => go_local_import_target(source_label, import_label),
        Language::Dart => dart_local_import_target(source_label, import_label, dart_packages),
        Language::Nix => nix_local_import_target(source_label, import_label),
        Language::Erlang => erlang_local_import_target(source_label, import_label),
        // No deterministic local-file resolution for these import systems yet
        // (classpaths / gem load paths / assembly references); imports still
        // land as facts and can join package hubs.
        Language::Ruby
        | Language::Java
        | Language::CSharp
        | Language::Kotlin
        | Language::Swift
        | Language::Scala
        | Language::Lua
        | Language::Elixir
        | Language::Zig
        | Language::Haskell
        | Language::OCaml
        | Language::Julia
        | Language::R => None,
    }
}

/// `import ./helper.nix` and `import ./dir` name a path relative to the
/// file that writes them, and a directory means its `default.nix`.
/// `import <nixpkgs>` names a channel, which is not a file in this tree.
pub(crate) fn nix_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let rest = import_label.trim().strip_prefix("import")?.trim_start();
    let path = rest
        .split_whitespace()
        .next()?
        .trim_end_matches(&[';', ')', ','][..]);
    if !(path.starts_with("./") || path.starts_with("../")) {
        return None;
    }
    let joined = join_path(path_dir(source_label).as_deref(), path);
    let mut candidates = vec![joined.clone()];
    if !joined.ends_with(".nix") {
        candidates.push(format!("{joined}/default.nix"));
        candidates.push(format!("{joined}.nix"));
    }
    Some(LocalImportTarget {
        target: path.to_string(),
        candidates,
    })
}

/// `-include("cowboy.hrl")` names a header this project ships, next to the
/// module or under the conventional `include/` and `src/` directories.
pub(crate) fn erlang_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let rest = import_label.trim().strip_prefix("-include")?;
    let path = rest
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(&[')', '.', ' '][..])
        .trim_matches('"');
    if path.is_empty() || path.starts_with('/') {
        return None;
    }
    let directory = path_dir(source_label);
    let mut candidates = vec![join_path(directory.as_deref(), path)];
    // A module in `src/` includes a header from the application's
    // `include/`, which is its sibling.
    if let Some(parent) = directory
        .as_deref()
        .and_then(|dir| dir.rsplit_once('/').map(|(head, _)| head.to_string()))
    {
        candidates.push(format!("{parent}/include/{path}"));
        candidates.push(format!("{parent}/src/{path}"));
    }
    candidates.push(format!("include/{path}"));
    candidates.push(format!("src/{path}"));
    Some(LocalImportTarget {
        target: path.to_string(),
        candidates,
    })
}

pub(crate) fn possible_local_import_target(
    language: Language,
    source_label: &str,
    import_label: &str,
    go_modules: &[GoModuleRoot],
    dart_packages: &[DartPackageRoot],
    npm_packages: &[NpmPackageRoot],
) -> Option<LocalImportTarget> {
    match language {
        Language::Python => python_absolute_local_import_target(source_label, import_label),
        Language::Go => go_module_import_target(import_label, go_modules),
        Language::Dart => dart_package_import_target(import_label, dart_packages),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            npm_package_import_target(import_label, npm_packages)
        }
        _ => None,
    }
}

/// A workspace import: `@vue/shared` resolves to the directory whose
/// package.json claims that name. A specifier that matches no package in
/// the repository is left alone — that one really did leave.
pub(crate) fn npm_package_import_target(
    import_label: &str,
    npm_packages: &[NpmPackageRoot],
) -> Option<LocalImportTarget> {
    let specifier = first_quoted_value(import_label)?;
    if specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with('/')
        || specifier.starts_with("node:")
    {
        return None;
    }
    let package = npm_packages.iter().find(|package| {
        specifier == package.name || specifier.starts_with(&format!("{}/", package.name))
    })?;
    let suffix = specifier
        .strip_prefix(&package.name)
        .unwrap_or("")
        .trim_start_matches('/');
    let package_dir = join_path(package.dir.as_deref(), suffix);
    Some(LocalImportTarget {
        target: specifier,
        candidates: vec![directory_candidate(&package_dir)],
    })
}

/// What a JavaScript or TypeScript import statement binds: a namespace
/// (`import * as fs`) that later calls qualify with, and the bare names
/// (`import { readFile }`, `import fetch from`) that they do not. Only
/// `import` counts — `export { x } from "mod"` re-exports without binding
/// anything the file can call.
pub(crate) struct JsImportBindings {
    pub(crate) qualifier: Option<String>,
    pub(crate) names: Vec<String>,
}

pub(crate) fn js_import_bindings(import_label: &str) -> JsImportBindings {
    let mut bindings = JsImportBindings {
        qualifier: None,
        names: Vec::new(),
    };
    let statement = import_label.trim();
    let Some(rest) = statement.strip_prefix("import ") else {
        return bindings;
    };
    // `import type { Foo } from` brings in types, which are never called.
    let rest = rest.trim_start().strip_prefix("type ").unwrap_or(rest);
    // `import "./styles.css"` binds nothing.
    let Some((clause, _)) = rest.split_once(" from ") else {
        return bindings;
    };

    for part in split_import_clause(clause) {
        let part = part.trim();
        if let Some(namespace) = part.strip_prefix("* as ") {
            let namespace = namespace.trim();
            if !namespace.is_empty() {
                bindings.qualifier = Some(namespace.to_string());
            }
            continue;
        }
        if let Some(named) = part.strip_prefix('{') {
            for name in named.trim_end_matches('}').split(',') {
                let name = name.trim();
                let name = name.strip_prefix("type ").unwrap_or(name).trim();
                let name = name.rsplit(" as ").next().unwrap_or(name).trim();
                if !name.is_empty() && name != "default" {
                    bindings.names.push(name.to_string());
                }
            }
            continue;
        }
        // A default import: `import fetch from "node-fetch"`.
        if !part.is_empty() {
            bindings.names.push(part.to_string());
        }
    }
    bindings
}

/// Split `fetch, { readFile, writeFile }` into its clauses without cutting
/// the commas that separate names inside the braces.
fn split_import_clause(clause: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for character in clause.chars() {
        match character {
            '{' => {
                depth += 1;
                current.push(character);
            }
            '}' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }
    parts.push(current);
    parts
}

pub(crate) fn js_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let module = first_quoted_value(import_label)?;
    if !(module.starts_with("./") || module.starts_with("../")) {
        return None;
    }
    // A bundler reads `./template/index.html?raw` as that file, asked for
    // in a particular way. The question mark and everything after it say
    // how to load the file, not which one.
    let path = module.split('?').next().unwrap_or(&module);
    Some(LocalImportTarget {
        target: module.clone(),
        candidates: module_file_candidates(
            source_label,
            path,
            &["js", "ts", "tsx", "jsx", "mjs", "cjs", "mts", "cts", "d.ts"],
        ),
    })
}

pub(crate) fn python_absolute_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let value = import_label.trim();
    let (module, imported) = if let Some(rest) = value.strip_prefix("import ") {
        (
            rest.split([',', ' ', '\n', '\t'])
                .find(|part| !part.is_empty())?,
            None,
        )
    } else {
        let rest = value.strip_prefix("from ")?;
        let module = rest.split_whitespace().next()?;
        if module.starts_with('.') {
            return None;
        }
        let imported = rest.split_once(" import ").and_then(|(_, imported)| {
            imported
                .split([',', ' ', '\n', '\t'])
                .find(|part| !part.is_empty())
        });
        (module, imported)
    };
    if module.is_empty() || module.starts_with('.') {
        return None;
    }

    let relative = module.replace('.', "/");
    let mut candidates = Vec::new();
    if let Some(imported) = imported {
        candidates.extend(python_module_candidates(&format!(
            "{relative}/{}",
            imported.replace('.', "/")
        )));
    }
    candidates.extend(python_module_candidates(&relative));
    if let Some(dir) = path_dir(source_label) {
        if let Some(imported) = imported {
            candidates.extend(python_module_candidates(&join_path(
                Some(&dir),
                &format!("{relative}/{}", imported.replace('.', "/")),
            )));
        }
        candidates.extend(python_module_candidates(&join_path(Some(&dir), &relative)));
    }
    dedup_preserving_order(&mut candidates);

    Some(LocalImportTarget {
        target: module.to_string(),
        candidates,
    })
}

pub(crate) fn python_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let value = import_label.trim();
    let rest = value.strip_prefix("from ")?;
    let dot_count = rest
        .chars()
        .take_while(|character| *character == '.')
        .count();
    if dot_count == 0 {
        return None;
    }
    let rest = &rest[dot_count..];
    let (module, imported) = rest.split_once(" import ")?;
    let imported = imported
        .split([',', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())
        .unwrap_or("");
    let target = if module.trim().is_empty() {
        imported.to_string()
    } else {
        module.trim().to_string()
    };
    if target.is_empty() {
        return None;
    }

    let mut base = path_dir(source_label);
    for _ in 1..dot_count {
        if let Some(parent) = base.as_deref().and_then(path_dir) {
            base = Some(parent);
        } else {
            base = None;
        }
    }
    let relative = target.replace('.', "/");
    let module_path = join_path(base.as_deref(), &relative);
    let candidates = python_module_candidates(&module_path);
    Some(LocalImportTarget {
        target: format!("{}{}", ".".repeat(dot_count), target),
        candidates,
    })
}

pub(crate) fn python_module_candidates(module_path: &str) -> Vec<String> {
    let mut candidates = with_file_extensions(module_path, &["py"]);
    candidates.push(normalize_path(&format!("{module_path}/__init__.py")));
    candidates
}

pub(crate) fn dedup_preserving_order(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

pub(crate) fn c_local_import_target(
    source_label: &str,
    import_label: &str,
    cmake_include_dirs: &[String],
) -> Option<LocalImportTarget> {
    let header = first_quoted_value(import_label)?;
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &header)];
    candidates.extend(
        cmake_include_dirs
            .iter()
            .map(|include_dir| join_path(Some(include_dir), &header)),
    );
    // The header as written, so a project whose include path comes from a
    // Makefile rather than CMake can still be matched by the unique-suffix
    // rule: redis compiles with `-Ideps/jemalloc/include`, and 911 of its
    // includes had nothing to resolve against.
    candidates.push(header.clone());
    dedup_preserving_order(&mut candidates);
    Some(LocalImportTarget {
        target: header.clone(),
        candidates,
    })
}

pub(crate) fn php_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if path.contains("://") || path.starts_with('/') {
        return None;
    }
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &path)];
    if !path_has_extension(&path) {
        candidates.push(join_path(
            path_dir(source_label).as_deref(),
            &format!("{path}.php"),
        ));
    }
    Some(LocalImportTarget {
        target: path,
        candidates,
    })
}

pub(crate) fn bash_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let mut parts = import_label.split_whitespace();
    let command = parts.next()?;
    if !matches!(command, "source" | ".") {
        return None;
    }
    let path = parts.next()?.trim_matches(['"', '\'']);
    if path.starts_with('/') || path.starts_with('$') || path.contains("://") {
        return None;
    }
    Some(LocalImportTarget {
        target: path.to_string(),
        candidates: vec![join_path(path_dir(source_label).as_deref(), path)],
    })
}

pub(crate) fn rust_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let value = import_label.trim().strip_prefix("use ")?.trim();
    let (base, rest) = if let Some(rest) = value.strip_prefix("crate::") {
        (rust_crate_root(source_label), rest)
    } else if let Some(rest) = value.strip_prefix("self::") {
        (path_dir(source_label), rest)
    } else {
        let rest = value.strip_prefix("super::")?;
        (
            path_dir(source_label).and_then(|path| path_dir(&path)),
            rest,
        )
    };
    let module = rest
        .split([':', ';', ',', '{', ' ', '\n', '\t'])
        .find(|part| !part.is_empty())?;
    if module.is_empty() || matches!(module, "self" | "super" | "crate") {
        return None;
    }
    // Glob imports (`use super::*;`) and leading-uppercase segments
    // (`use crate::ImpactRequest`) name items inside an already-loaded
    // module, not module files that could resolve on disk.
    if module == "*"
        || module
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_uppercase())
    {
        return None;
    }
    let module_path = join_path(base.as_deref(), module);
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates: vec![
            normalize_path(&format!("{module_path}.rs")),
            normalize_path(&format!("{module_path}/mod.rs")),
        ],
    })
}

pub(crate) fn go_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if !(path.starts_with("./") || path.starts_with("../")) {
        return None;
    }
    let package_dir = join_path(path_dir(source_label).as_deref(), &path);
    Some(LocalImportTarget {
        target: path,
        candidates: vec![directory_candidate(&package_dir)],
    })
}

pub(crate) fn go_module_import_target(
    import_label: &str,
    go_modules: &[GoModuleRoot],
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if path.starts_with("./") || path.starts_with("../") || path.starts_with('/') {
        return None;
    }
    let module = go_modules
        .iter()
        .find(|module| path == module.module || path.starts_with(&format!("{}/", module.module)))?;
    let suffix = path
        .strip_prefix(&module.module)
        .unwrap_or("")
        .trim_start_matches('/');
    let package_dir = join_path(module.dir.as_deref(), suffix);
    Some(LocalImportTarget {
        target: path,
        candidates: vec![directory_candidate(&package_dir)],
    })
}

pub(crate) fn dart_local_import_target(
    source_label: &str,
    import_label: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let uri = dart_import_uri(import_label)?;
    if uri.starts_with("./") || uri.starts_with("../") {
        return Some(LocalImportTarget {
            target: uri.clone(),
            candidates: vec![join_path(path_dir(source_label).as_deref(), &uri)],
        });
    }
    if uri.ends_with(".dart") && !uri.starts_with("package:") && !uri.contains("://") {
        return Some(LocalImportTarget {
            target: uri.clone(),
            candidates: vec![join_path(path_dir(source_label).as_deref(), &uri)],
        });
    }
    dart_package_uri_target(&uri, dart_packages)
}

pub(crate) fn dart_package_import_target(
    import_label: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let uri = dart_import_uri(import_label)?;
    dart_package_uri_target(&uri, dart_packages)
}

pub(crate) fn dart_package_uri_target(
    uri: &str,
    dart_packages: &[DartPackageRoot],
) -> Option<LocalImportTarget> {
    let rest = uri.strip_prefix("package:")?;
    let (package, path) = rest.split_once('/')?;
    if package.is_empty() || path.is_empty() {
        return None;
    }
    let package_root = dart_packages.iter().find(|root| root.name == package)?;
    let target = join_path(
        package_root.dir.as_deref(),
        &format!("{}/{path}", package_root.lib_dir),
    );
    Some(LocalImportTarget {
        target: uri.to_string(),
        candidates: vec![target],
    })
}

pub(crate) fn dart_import_uri(import_label: &str) -> Option<String> {
    first_quoted_value(import_label)
}

/// For a generated Dart file (build_runner and protoc conventions), the
/// scan-root-relative path of the source file that generates it.
pub(crate) fn dart_generated_source_name(label: &str) -> Option<String> {
    const GENERATED_SUFFIXES: &[&str] = &[
        ".g.dart",
        ".freezed.dart",
        ".gr.dart",
        ".pb.dart",
        ".pbenum.dart",
        ".pbjson.dart",
        ".pbserver.dart",
        ".mocks.dart",
        ".config.dart",
        ".gen.dart",
    ];
    for suffix in GENERATED_SUFFIXES {
        if let Some(base) = label.strip_suffix(suffix) {
            let file_base = base.rsplit('/').next().unwrap_or(base);
            if !file_base.is_empty() {
                return Some(format!("{base}.dart"));
            }
        }
    }
    None
}

pub(crate) fn directory_candidate(path: &str) -> String {
    let normalized = normalize_path(path);
    if normalized.is_empty() {
        "/".to_string()
    } else {
        format!("{normalized}/")
    }
}

pub(crate) fn module_file_candidates(
    source_label: &str,
    module: &str,
    extensions: &[&str],
) -> Vec<String> {
    let path = join_path(path_dir(source_label).as_deref(), module);
    let mut candidates = with_file_extensions(&path, extensions);
    for extension in extensions {
        candidates.push(normalize_path(&format!("{path}/index.{extension}")));
    }
    candidates
}

pub(crate) fn with_file_extensions(path: &str, extensions: &[&str]) -> Vec<String> {
    let mut candidates = Vec::new();
    if path_has_extension(path) {
        candidates.push(normalize_path(path));
    }
    // A dot in a name is not always an extension: `./vFor.spec` is how a
    // test file next door is spelled, and the file is `vFor.spec.ts`. The
    // written name is tried first, so a file that really is named that
    // still wins.
    candidates.extend(
        extensions
            .iter()
            .map(|extension| normalize_path(&format!("{path}.{extension}"))),
    );
    candidates
}

pub(crate) fn path_has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

pub(crate) fn rust_crate_root(source_label: &str) -> Option<String> {
    if source_label == "src/main.rs" || source_label == "src/lib.rs" {
        return Some("src".to_string());
    }
    source_label
        .strip_prefix("src/")
        .map(|_| "src".to_string())
        .or_else(|| path_dir(source_label))
}

pub(crate) fn path_dir(path: &str) -> Option<String> {
    path.rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .filter(|dir| !dir.is_empty())
}

pub(crate) fn join_path(base: Option<&str>, relative: &str) -> String {
    let path = match base {
        Some(base) if !base.is_empty() => format!("{base}/{relative}"),
        _ => relative.to_string(),
    };
    normalize_path(&path)
}

pub(crate) fn normalize_path(path: &str) -> String {
    let mut parts = Vec::new();
    let normalized = path.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}

/// The name a Go import binds: an explicit alias, else the last segment of the
/// package path (`"net/http"` binds `http`). Blank (`_`) and dot imports bind
/// no qualifier, and a package whose name differs from its last path segment
/// simply does not match — the caller then falls back to the old behaviour
/// rather than resolving to the wrong package.
pub(crate) fn go_import_qualifier(import_label: &str) -> Option<String> {
    let alias = import_label.split('"').next().unwrap_or_default().trim();
    if !alias.is_empty() {
        return (alias != "_" && alias != ".").then(|| alias.to_string());
    }
    let path = first_quoted_value(import_label)?;
    let segment = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
    (!segment.is_empty()).then_some(segment)
}

/// The name a Python import binds as a call qualifier.
///
/// `import flask` binds `flask`, `import os.path` binds `os`, `import numpy as
/// np` binds `np`, and `from . import views` binds `views`. A `from x import y`
/// binds `y` as a bare name, not a qualifier, so it is left alone — the call
/// `y()` is unqualified and resolves like any other.
/// The bare names a `from module import a, b as c` statement binds. The
/// qualifier map cannot answer for these: the call site writes
/// `OrderedDict()` with nothing before a dot to match on, so the name
/// itself has to carry where it came from. `from . import views` is left
/// out — that binds a module, which [`python_import_qualifier`] already
/// records.
pub(crate) fn python_imported_names(import_label: &str) -> Vec<String> {
    let statement = import_label.trim();
    let Some(rest) = statement.strip_prefix("from ") else {
        return Vec::new();
    };
    let Some((module, imported)) = rest.split_once(" import ") else {
        return Vec::new();
    };
    if module.trim().trim_matches('.').is_empty() {
        return Vec::new();
    }
    imported
        .split(',')
        .filter_map(|name| {
            let name = name
                .trim()
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim();
            let name = name.rsplit(" as ").next().unwrap_or(name).trim();
            (!name.is_empty() && name != "*").then(|| name.to_string())
        })
        .collect()
}

pub(crate) fn python_import_qualifier(import_label: &str) -> Option<String> {
    let statement = import_label.trim();
    if let Some(rest) = statement.strip_prefix("from ") {
        let (module, imported) = rest.split_once(" import ")?;
        // `from . import views` names a module, so `views.load()` is qualified
        // by it. `from .globals import _cv_app` names a value: `_cv_app.get()`
        // is a method call on it, and reading `_cv_app` as a module sent the
        // call looking inside `globals.py`.
        if !module.trim().trim_matches('.').is_empty() {
            return None;
        }
        let name = imported.split(',').next()?.trim();
        let name = name.rsplit(" as ").next().unwrap_or(name).trim();
        return (!name.is_empty() && name != "*").then(|| name.to_string());
    }

    let rest = statement.strip_prefix("import ")?;
    let first = rest.split(',').next()?.trim();
    if let Some((_, alias)) = first.rsplit_once(" as ") {
        let alias = alias.trim();
        return (!alias.is_empty()).then(|| alias.to_string());
    }
    let head = first.split('.').next()?.trim();
    (!head.is_empty()).then(|| head.to_string())
}
