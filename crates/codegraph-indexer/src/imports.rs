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
        // `#import "AFURLSessionManager.h"` names a header the way C does.
        Language::C | Language::Cpp | Language::ObjectiveC => {
            c_local_import_target(source_label, import_label, cmake_include_dirs)
        }
        Language::Php => php_local_import_target(source_label, import_label),
        Language::Hcl => hcl_local_import_target(source_label, import_label),
        Language::Solidity => solidity_local_import_target(source_label, import_label),
        Language::Proto => proto_local_import_target(source_label, import_label),
        // A GraphQL schema states its types in one document; nothing in the
        // language names another file.
        Language::GraphQl => None,
        Language::Bash => bash_local_import_target(source_label, import_label),

        Language::Go => go_local_import_target(source_label, import_label),
        Language::Dart => dart_local_import_target(source_label, import_label, dart_packages),
        Language::Nix => nix_local_import_target(source_label, import_label),
        Language::Erlang => erlang_local_import_target(source_label, import_label),
        Language::Julia => julia_local_import_target(source_label, import_label),
        Language::Ruby => ruby_local_import_target(source_label, import_label),
        Language::Zig => zig_local_import_target(source_label, import_label),
        // No deterministic local-file resolution for these import systems yet
        // (classpaths / gem load paths / assembly references); imports still
        // land as facts and can join package hubs.
        Language::Rust
        | Language::Java
        | Language::CSharp
        | Language::Kotlin
        | Language::Swift
        | Language::Scala
        | Language::Lua
        | Language::Elixir
        | Language::Haskell
        | Language::OCaml
        | Language::R => None,
    }
}

/// Haskell requires a module's name to match the path it is written at, so
/// `import ShellCheck.AST` names `ShellCheck/AST.hs` under one of the
/// project's source roots. shellcheck imports its own modules 134 times;
/// the rest name libraries and are left alone.
pub(crate) fn haskell_local_import_target(import_label: &str) -> Option<LocalImportTarget> {
    let rest = import_label.trim().strip_prefix("import")?.trim_start();
    let rest = rest.strip_prefix("qualified").map_or(rest, str::trim_start);
    let module = rest
        .split(|character: char| character.is_whitespace() || character == '(')
        .next()?
        .trim();
    if module.is_empty() || !module.starts_with(|character: char| character.is_ascii_uppercase()) {
        return None;
    }
    let path = module.replace('.', "/");
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates: vec![
            format!("src/{path}.hs"),
            format!("lib/{path}.hs"),
            format!("{path}.hs"),
        ],
    })
}

/// `import "./Ownable.sol";` names a file beside this one; a path that
/// starts with a package name — `@openzeppelin/contracts/...` — names a
/// dependency the project installed.
pub(crate) fn solidity_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = import_label.trim();
    if !(path.starts_with("./") || path.starts_with("../")) {
        return None;
    }
    let joined = join_path(path_dir(source_label).as_deref(), path);
    Some(LocalImportTarget {
        target: path.to_string(),
        candidates: vec![joined],
    })
}

/// `import "google/protobuf/timestamp.proto";` names a file by the path a
/// compiler would find it under: from a root of the repository, or beside
/// the file that imports it.
pub(crate) fn proto_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = import_label.trim().trim_matches('"');
    // `google/protobuf/timestamp.proto` and its siblings ship with the
    // compiler, and `google/api/...` with googleapis: an import of one names
    // a dependency rather than a file of this repository.
    if path.is_empty() || path.starts_with("google/") || path.starts_with("grpc/") {
        return None;
    }
    let mut candidates = vec![path.to_string()];
    if let Some(directory) = path_dir(source_label) {
        candidates.push(join_path(Some(directory.as_str()), path));
    }
    Some(LocalImportTarget {
        target: path.to_string(),
        candidates,
    })
}

/// A Terraform module names its source, and a source that starts with `./`
/// or `../` is a directory of this repository: `source = "../modules/vpc"`
/// is the configuration in that directory, which the registry and git forms
/// are not.
pub(crate) fn hcl_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = import_label.trim();
    if !(path.starts_with("./") || path.starts_with("../")) {
        return None;
    }
    let joined = join_path(path_dir(source_label).as_deref(), path);
    Some(LocalImportTarget {
        target: path.to_string(),
        // A module is a directory, and every file in it is part of it; the
        // conventional entry file stands for the whole.
        candidates: vec![
            format!("{joined}/main.tf"),
            format!("{joined}/terraform.tf"),
            format!("{joined}/variables.tf"),
            joined,
        ],
    })
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
        Language::C | Language::Cpp => c_system_header_target(source_label, import_label),
        // `use crate::de` in a project that compiles always names something:
        // a module file, a module written inline, or a name the crate root
        // re-exports from elsewhere. Only the first is a file, so a miss is
        // not a module serde failed to ship - it had 24 of them.
        Language::Rust => rust_local_import_target(source_label, import_label),
        Language::Python => python_absolute_local_import_target(source_label, import_label),
        Language::Go => go_module_import_target(import_label, go_modules),
        Language::Dart => dart_package_import_target(import_label, dart_packages),
        Language::JavaScript | Language::TypeScript | Language::Tsx => {
            npm_package_import_target(import_label, npm_packages)
        }
        // A Haskell import names a module, and most of them are a
        // library's: `import Data.List` is not a file this project failed
        // to ship, so a miss must stay quiet.
        Language::Haskell => haskell_local_import_target(import_label),
        Language::Elixir => elixir_local_import_target(import_label),
        Language::Java | Language::Kotlin | Language::Scala => {
            jvm_local_import_target(language, import_label)
        }
        Language::Php => php_namespace_import_target(import_label),
        Language::Lua => lua_local_import_target(import_label),
        Language::OCaml => ocaml_local_import_target(import_label),
        _ => None,
    }
}

/// `use GuzzleHttp\Exception\InvalidArgumentException;` names a class, and
/// PSR-4 maps its namespace onto a directory -- `GuzzleHttp\` onto `src/`
/// here. The prefix a project maps varies, so each suffix of the namespace
/// path is a candidate and the resolver takes the one file that ends with
/// it. A name that matches nothing is a vendor class, which is why this is
/// a possible local import rather than a required one.
pub(crate) fn php_namespace_import_target(import_label: &str) -> Option<LocalImportTarget> {
    let rest = import_label.trim().strip_prefix("use")?.trim_start();
    let rest = rest.strip_prefix("function").map_or(rest, str::trim_start);
    let target = rest
        .split(|character: char| character.is_whitespace() || character == ';' || character == ',')
        .next()?
        .trim()
        .trim_start_matches('\\');
    let segments: Vec<&str> = target.split('\\').filter(|part| !part.is_empty()).collect();
    if segments.len() < 2 {
        return None;
    }
    let mut candidates = Vec::new();
    for start in 0..segments.len() - 1 {
        candidates.push(format!("{}.php", segments[start..].join("/")));
    }
    Some(LocalImportTarget {
        target: target.to_string(),
        candidates,
    })
}

/// `@import("ast.zig")` names a file beside this one; `@import("std")`
/// names the standard library. zls writes 394 imports and every path-shaped
/// one points at a file in the tree.
pub(crate) fn zig_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let path = first_quoted_value(import_label)?;
    if !path.ends_with(".zig") || path.starts_with('/') {
        return None;
    }
    Some(LocalImportTarget {
        target: path.clone(),
        candidates: vec![join_path(path_dir(source_label).as_deref(), &path)],
    })
}

/// `import com.google.gson.Gson;` names the file the package directory
/// holds, which Java, Kotlin and Scala all lay out the same way. The
/// source root varies by build tool, so the package path is the candidate
/// and the resolver finds the one file that ends with it.
pub(crate) fn jvm_local_import_target(
    language: Language,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let rest = import_label.trim().strip_prefix("import")?.trim_start();
    let rest = rest.strip_prefix("static").map_or(rest, str::trim_start);
    let target = rest
        .split(|character: char| character.is_whitespace() || character == ';')
        .next()?
        .trim();
    // A wildcard names a package, not a file: `import java.util.*` and
    // Scala's `import cats.implicits._`.
    if target.is_empty() || target.ends_with('*') || target.ends_with('_') || !target.contains('.')
    {
        return None;
    }
    let extension = match language {
        Language::Kotlin => "kt",
        Language::Scala => "scala",
        _ => "java",
    };
    let path = target.replace('.', "/");
    Some(LocalImportTarget {
        target: target.to_string(),
        candidates: vec![format!("{path}.{extension}")],
    })
}

/// `include("abstractdataframe.jl")` splices in a file beside this one --
/// the only Julia import that names a path rather than a package.
pub(crate) fn julia_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    if !import_label.trim_start().starts_with("include") {
        return None;
    }
    let path = first_quoted_value(import_label)?;
    if path.is_empty() || path.starts_with('/') {
        return None;
    }
    Some(LocalImportTarget {
        target: path.clone(),
        candidates: vec![join_path(path_dir(source_label).as_deref(), &path)],
    })
}

/// `require_relative "helpers"` names the file beside this one; sinatra
/// writes 45 of them.
pub(crate) fn ruby_local_import_target(
    source_label: &str,
    import_label: &str,
) -> Option<LocalImportTarget> {
    let rest = import_label.trim().strip_prefix("require_relative")?;
    let path = first_quoted_value(rest)?;
    if path.is_empty() {
        return None;
    }
    let joined = join_path(path_dir(source_label).as_deref(), &path);
    let mut candidates = vec![format!("{joined}.rb")];
    if joined.ends_with(".rb") {
        candidates.insert(0, joined.clone());
    }
    Some(LocalImportTarget {
        target: path,
        candidates,
    })
}

/// `alias Ecto.Query` names the module Elixir compiles from
/// `lib/ecto/query.ex`: the module path underscored, which is the mapping
/// `mix` itself expects. Ecto writes 568 of these.
pub(crate) fn elixir_local_import_target(import_label: &str) -> Option<LocalImportTarget> {
    let rest = ["alias ", "import ", "use ", "require "]
        .iter()
        .find_map(|head| import_label.trim().strip_prefix(head))?
        .trim_start();
    let module = rest
        .split(|character: char| character.is_whitespace() || character == ',')
        .next()?
        .trim();
    if module.is_empty() || !module.starts_with(|character: char| character.is_ascii_uppercase()) {
        return None;
    }
    let path = module
        .split('.')
        .map(underscore_module_segment)
        .collect::<Vec<_>>()
        .join("/");
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates: vec![
            format!("lib/{path}.ex"),
            format!("{path}.ex"),
            format!("lib/{path}/{}.ex", path.rsplit('/').next().unwrap_or(&path)),
        ],
    })
}

/// `HTTPClient` -> `http_client`, the conversion `Macro.underscore` does:
/// an underscore before each capital that starts a word.
fn underscore_module_segment(segment: &str) -> String {
    let characters: Vec<char> = segment.chars().collect();
    let mut out = String::with_capacity(segment.len() + 4);
    for (index, character) in characters.iter().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            let previous = characters[index - 1];
            let next_is_lower = characters
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
            if previous.is_ascii_lowercase() || previous.is_ascii_digit() || next_is_lower {
                out.push('_');
            }
        }
        out.extend(character.to_lowercase());
    }
    out
}

/// `require "kong.tools.utils"` names `kong/tools/utils.lua`, the path Lua
/// itself derives from the module name. kong writes 3,718 of them, most
/// naming its own modules and the rest naming a rock.
pub(crate) fn lua_local_import_target(import_label: &str) -> Option<LocalImportTarget> {
    let module = first_quoted_value(import_label)?;
    // A bare name is a rock: kong requires `cjson` and `lfs`, and the
    // repository happens to ship `kong/tools/cjson.lua`, which is a
    // different module. Only a dotted path names a file in this tree.
    if module.is_empty() || module.contains('/') || !module.contains('.') {
        return None;
    }
    let path = module.replace('.', "/");
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates: vec![
            format!("{path}.lua"),
            format!("{path}/init.lua"),
            format!("lib/{path}.lua"),
            format!("src/{path}.lua"),
        ],
    })
}

/// `open Stdune` names the module OCaml compiles from `stdune.ml`, the same
/// name-to-file rule call resolution already uses. A submodule path
/// (`Fiber.O`) names the file of its root.
pub(crate) fn ocaml_local_import_target(import_label: &str) -> Option<LocalImportTarget> {
    let rest = import_label
        .trim()
        .strip_prefix("open")
        .or_else(|| import_label.trim().strip_prefix("include"))?
        .trim_start();
    let module = rest
        .split(|character: char| character.is_whitespace() || character == '(')
        .next()?
        .trim();
    let root = module.split('.').next()?;
    if root.is_empty() || !root.starts_with(|character: char| character.is_ascii_uppercase()) {
        return None;
    }
    let file = root.to_ascii_lowercase();
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates: vec![
            format!("{file}.ml"),
            format!("src/{file}.ml"),
            format!("lib/{file}.ml"),
        ],
    })
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

/// The name a TypeScript file has on disk when an ESM import names the
/// compiled one: `./snapshot.js` is written for `snapshot.ts`, and the
/// same holds for `.mjs`/`.cjs`.
fn typescript_source_of_compiled_import(path: &str) -> Option<String> {
    for extension in [".js", ".mjs", ".cjs", ".jsx"] {
        if let Some(stem) = path.strip_suffix(extension) {
            return Some(stem.to_string());
        }
    }
    None
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
    let extensions = ["js", "ts", "tsx", "jsx", "mjs", "cjs", "mts", "cts", "d.ts"];
    let mut candidates = module_file_candidates(source_label, path, &extensions);
    // TypeScript writes the compiled name in an ESM import — `import ..
    // from "./snapshot.js"` next to `snapshot.ts` — which is the
    // convention the language requires, and zod writes 61 of them.
    if let Some(stem) = typescript_source_of_compiled_import(path) {
        candidates.extend(module_file_candidates(source_label, &stem, &extensions));
    }
    Some(LocalImportTarget {
        target: module.clone(),
        candidates,
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
    // `import typing as t` inside `src/flask/typing.py` names the standard
    // library, not the file it is written in. Without this the module
    // resolved onto itself and read as a dependency cycle.
    if module
        .split('.')
        .next()
        .is_some_and(codegraph_core::is_python_stdlib_package)
    {
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
    // `#include "stdio.h"` searches next to the file first and the system
    // path second, and redis writes four of its libc includes that way. A
    // project that ships its own copy still resolves, as a possible local
    // import; one that does not is including the system header.
    if is_c_system_header(&header) {
        return None;
    }
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &header)];
    // A build gives the compiler its own directories with `-I`: the
    // Flutter runner includes `flutter/generated_plugin_registrant.h`
    // from `windows/runner/`, and the header sits in `windows/flutter/`
    // because CMake puts `windows/` on the include path. Walking out
    // through the directories the file itself sits in finds it, nearest
    // first, without knowing which flags the build passes.
    let mut directory = path_dir(source_label);
    while let Some(current) = directory {
        let parent = path_dir(&current);
        candidates.push(join_path(parent.as_deref(), &header));
        directory = parent;
    }
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

/// A header a C toolchain ships. The list covers the C standard library and
/// the POSIX headers a project is likely to include by name; anything under
/// `sys/` is the operating system's by convention.
pub(crate) fn is_c_system_header(header: &str) -> bool {
    if header.starts_with("sys/") || header.starts_with("bits/") {
        return true;
    }
    matches!(
        header,
        "assert.h"
            | "complex.h"
            | "ctype.h"
            | "dirent.h"
            | "dlfcn.h"
            | "errno.h"
            | "fcntl.h"
            | "fenv.h"
            | "float.h"
            | "grp.h"
            | "inttypes.h"
            | "iso646.h"
            | "limits.h"
            | "locale.h"
            | "math.h"
            | "netdb.h"
            | "poll.h"
            | "pthread.h"
            | "pwd.h"
            | "regex.h"
            | "sched.h"
            | "semaphore.h"
            | "setjmp.h"
            | "signal.h"
            | "stdalign.h"
            | "stdarg.h"
            | "stdatomic.h"
            | "stdbool.h"
            | "stddef.h"
            | "stdint.h"
            | "stdio.h"
            | "stdlib.h"
            | "stdnoreturn.h"
            | "string.h"
            | "strings.h"
            | "syslog.h"
            | "termios.h"
            | "tgmath.h"
            | "threads.h"
            | "time.h"
            | "uchar.h"
            | "unistd.h"
            | "utime.h"
            | "wchar.h"
            | "wctype.h"
    )
}

/// A project may ship a header that shadows a system one, so the system name
/// is still worth resolving -- quietly, because a miss is the toolchain's
/// copy rather than a file the project failed to ship.
fn c_system_header_target(source_label: &str, import_label: &str) -> Option<LocalImportTarget> {
    let header = first_quoted_value(import_label).filter(|header| is_c_system_header(header))?;
    let mut candidates = vec![join_path(path_dir(source_label).as_deref(), &header)];
    candidates.push(header.clone());
    dedup_preserving_order(&mut candidates);
    Some(LocalImportTarget {
        target: header,
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
    // The crate root first, then the directories between the file and it:
    // ripgrep's `crates/core` has no `src/`, so `use crate::flags` in
    // `crates/core/flags/complete/bash.rs` is `crates/core/flags/mod.rs`.
    let mut candidates = Vec::new();
    for root in rust_module_roots(source_label, base.as_deref()) {
        let module_path = join_path(Some(root.as_str()), module);
        candidates.push(normalize_path(&format!("{module_path}.rs")));
        candidates.push(normalize_path(&format!("{module_path}/mod.rs")));
    }
    dedup_preserving_order(&mut candidates);
    Some(LocalImportTarget {
        target: module.to_string(),
        candidates,
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

/// Where a module path may be rooted: the directory the prefix names, then
/// every directory between the file and the repository root. A crate laid
/// out without `src/` is found by the second, and the first keeps the
/// common case exact.
fn rust_module_roots(source_label: &str, base: Option<&str>) -> Vec<String> {
    let mut roots = Vec::new();
    if let Some(base) = base {
        roots.push(base.to_string());
    }
    let mut directory = path_dir(source_label);
    while let Some(current) = directory {
        roots.push(current.clone());
        directory = path_dir(&current);
    }
    roots.push(String::new());
    dedup_preserving_order(&mut roots);
    roots
}

/// The crate a file belongs to, which is what `crate::` names. A workspace
/// has one per member, and reading the file's own directory instead sent
/// serde_derive's `use crate::internals::ast` looking for
/// `serde_derive/src/de/internals.rs`.
pub(crate) fn rust_crate_root(source_label: &str) -> Option<String> {
    if let Some(index) = source_label.rfind("/src/") {
        return Some(source_label[..index + "/src".len()].to_string());
    }
    if source_label.starts_with("src/") {
        return Some("src".to_string());
    }
    path_dir(source_label)
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

/// Whether a line sits inside an `if TYPE_CHECKING:` block. Python erases
/// those imports at run time - they exist for the type checker - so what
/// they name is not something the module needs when it runs.
pub(crate) fn line_is_type_checking_only(source: &str, line: u32) -> bool {
    let mut block_indent: Option<usize> = None;
    for (index, text) in source.lines().enumerate() {
        let current = index as u32 + 1;
        let indent = text.len() - text.trim_start().len();
        let trimmed = text.trim();
        if let Some(open_indent) = block_indent {
            // The block ends at the first line indented no further than the
            // `if` that opened it; blank lines belong to whatever follows.
            if !trimmed.is_empty() && indent <= open_indent {
                block_indent = None;
            } else if current == line {
                return true;
            }
        }
        // `import typing as t` then `if t.TYPE_CHECKING:` is how flask
        // writes it, so what the module is called cannot be part of the
        // test -- only that the block is opened by the flag.
        if block_indent.is_none()
            && trimmed.starts_with("if ")
            && trimmed.ends_with("TYPE_CHECKING:")
            && !trimmed.contains(" not ")
        {
            block_indent = Some(indent);
        }
        if current >= line && block_indent.is_none() {
            return false;
        }
    }
    false
}

/// Whether a Python import sits in a `try:` whose `except` handles the
/// import failing. `try: import cryptography / except ImportError:` states
/// that the program runs without the package, which is what an optional
/// dependency is — and requests writes its `import cryptography` in the
/// `else:` of such a block, which is the same statement about it.
pub(crate) fn line_is_a_guarded_import(source: &str, line: u32) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let Some(index) = (line as usize).checked_sub(1) else {
        return false;
    };
    let Some(target) = lines.get(index) else {
        return false;
    };
    let indent_of = |text: &str| text.len() - text.trim_start().len();
    let target_indent = indent_of(target);

    // The `try:` that holds it, reached by walking out through the blocks
    // it sits in: requests writes its `from cryptography import ..` inside
    // an `if` inside the `try`, and an ImportError anywhere in the body is
    // caught the same way. A `def` or `class` ends the walk: an import
    // there runs when the function is called, not when the module loads.
    let mut try_line = None;
    let mut enclosing_indent = target_indent;
    for (above, text) in lines[..index].iter().enumerate().rev() {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = indent_of(text);
        if indent >= enclosing_indent {
            continue;
        }
        if trimmed == "try:" {
            try_line = Some((above, indent));
            break;
        }
        if trimmed.starts_with("def ")
            || trimmed.starts_with("async def ")
            || trimmed.starts_with("class ")
        {
            break;
        }
        // `except`, `else` and `finally` belong to the `try` above them, so
        // the walk keeps looking at their own level rather than stepping
        // out of it: requests writes one of its imports in such an `else`.
        if trimmed == "else:" || trimmed == "finally:" || trimmed.starts_with("except") {
            enclosing_indent = indent + 1;
            continue;
        }
        enclosing_indent = indent;
    }
    let Some((try_line, try_indent)) = try_line else {
        return false;
    };

    // The handler that closes it: the first `except` at the `try`'s own
    // indent.
    lines[try_line + 1..]
        .iter()
        .find_map(|text| {
            let trimmed = text.trim();
            if trimmed.is_empty() || indent_of(text) > try_indent {
                return None;
            }
            trimmed.strip_prefix("except").map(|handled| {
                handled.contains("ImportError") || handled.contains("ModuleNotFoundError")
            })
        })
        .unwrap_or(false)
}

/// The namespace a C# `using` names. `using static X.Y` names a type and
/// `using A = X.Y` an alias; neither is the namespace itself.
pub(crate) fn csharp_namespace_import(language: Language, import_label: &str) -> Option<String> {
    if language != Language::CSharp {
        return None;
    }
    let value = import_label.trim().trim_end_matches(';').trim();
    let rest = value.strip_prefix("using")?.trim();
    if rest.is_empty() || rest.starts_with("static ") || rest.contains('=') {
        return None;
    }
    let namespace = rest.trim();
    namespace
        .chars()
        .all(|character| character.is_alphanumeric() || matches!(character, '.' | '_'))
        .then(|| namespace.to_string())
}
