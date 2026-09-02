//! Filesystem walk rules: ignore lists, hidden/CI infrastructure paths,
//! and index-relevance checks.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use codegraph_parser::{Language, ParsedItemKind};
use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::DirEntry;

#[allow(unused_imports)]
use crate::*;

pub(crate) fn parsed_item_kind_name(kind: ParsedItemKind) -> &'static str {
    match kind {
        ParsedItemKind::Function => "function",
        ParsedItemKind::Type => "type",
        ParsedItemKind::Module => "module",
        ParsedItemKind::Import => "import",
        ParsedItemKind::Entrypoint => "entrypoint",
        ParsedItemKind::Call => "call",
        ParsedItemKind::EnvironmentRead => "environment_read",
        ParsedItemKind::ConfigRead => "config_read",
        ParsedItemKind::Error => "error",
        ParsedItemKind::Branch => "branch",
        ParsedItemKind::Loop => "loop",
        ParsedItemKind::Async => "async",
        ParsedItemKind::Return => "return",
    }
}

pub(crate) fn is_symbol_item(kind: ParsedItemKind) -> bool {
    matches!(
        kind,
        ParsedItemKind::Function
            | ParsedItemKind::Entrypoint
            | ParsedItemKind::Type
            | ParsedItemKind::Module
            | ParsedItemKind::Import
    )
}

pub(crate) fn is_effect_item(kind: ParsedItemKind) -> bool {
    matches!(
        kind,
        ParsedItemKind::EnvironmentRead
            | ParsedItemKind::ConfigRead
            | ParsedItemKind::Error
            | ParsedItemKind::Branch
            | ParsedItemKind::Loop
            | ParsedItemKind::Async
            | ParsedItemKind::Return
    )
}

/// Whether the walker should descend into / include `entry`. Shared with the
/// cache fingerprint (codegraph-storage) so the set of files that are scanned
/// and the set that are fingerprinted can never diverge.
pub fn should_enter(
    entry: &DirEntry,
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> bool {
    if entry.path() == root {
        return true;
    }
    if !options.include_hidden && is_ci_infrastructure_path(entry.path(), root) {
        return entry_exclusion_without_hidden(entry, root, options, ignored_globs).is_none();
    }

    entry_exclusion(entry, root, options, ignored_globs).is_none()
}

pub(crate) fn entry_exclusion(
    entry: &DirEntry,
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Option<EntryExclusion> {
    if !options.include_hidden && is_hidden(entry) {
        return Some(EntryExclusion::Hidden);
    }

    if !options.include_ignored && is_ignored_name(entry, &options.ignored_names) {
        return Some(EntryExclusion::IgnoredName);
    }

    if !options.include_ignored && is_ignored_glob(entry.path(), root, ignored_globs) {
        return Some(EntryExclusion::IgnoredGlob);
    }

    None
}

pub(crate) fn entry_exclusion_without_hidden(
    entry: &DirEntry,
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> Option<EntryExclusion> {
    if !options.include_ignored && is_ignored_name(entry, &options.ignored_names) {
        return Some(EntryExclusion::IgnoredName);
    }

    if !options.include_ignored && is_ignored_glob(entry.path(), root, ignored_globs) {
        return Some(EntryExclusion::IgnoredGlob);
    }

    None
}

pub(crate) fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.') && name != ".")
}

pub(crate) fn is_ci_infrastructure_path(path: &Path, root: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let parts: Vec<_> = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    matches!(
        parts.as_slice(),
        [".github"]
            | [".github", "workflows", ..]
            | [".github", "actions", ..]
            | [".gitlab-ci.yml"]
            | [".gitlab-ci.yaml"]
            | [".gitlab"]
            | [".gitlab", "ci", ..]
    )
}

pub(crate) fn is_ignored_name(entry: &DirEntry, ignored_names: &BTreeSet<String>) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| ignored_names.contains(name))
}

pub(crate) fn is_ignored_glob(path: &Path, root: &Path, ignored_globs: &Option<GlobSet>) -> bool {
    let Some(ignored_globs) = ignored_globs else {
        return false;
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    !relative.is_empty() && ignored_globs.is_match(relative)
}

pub(crate) fn default_ignored_names() -> BTreeSet<String> {
    [
        ".git",
        ".codegraph",
        ".hg",
        ".svn",
        "target",
        "node_modules",
        "dist",
        "build",
        "graphify-out",
        ".next",
        ".turbo",
        ".venv",
        "__pycache__",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub fn is_index_relevant_file(path: &Path) -> bool {
    if Language::detect(path).is_some() {
        return true;
    }
    if is_markdown_document(path) || is_rst_document(path) || is_asciidoc_document(path) {
        return true;
    }
    if is_plain_text_document(path) {
        return true;
    }
    if is_sql_file(path) {
        return true;
    }
    if is_notebook_path(path) {
        return true;
    }
    // A single-file component states its program in a `<script>` block.
    if is_single_file_component(path) {
        return true;
    }

    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(
            "Cargo.toml"
                | "package.json"
                | "package-lock.json"
                | "pnpm-lock.yaml"
                | "go.mod"
                | "pubspec.yaml"
                | "pyproject.toml"
                | "setup.py"
                | "setup.cfg"
                | "Pipfile"
                | "requirements.txt"
                | "composer.json"
                | "composer.lock"
                | "vcpkg.json"
                | "conanfile.txt"
                | "CMakeLists.txt"
                | "compile_commands.json"
        )
    )
}

pub(crate) fn is_probably_source_file(path: &Path, max_file_size: u64) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.len() <= max_file_size)
}

/// The longest line of a file that a tool packed onto one line, or `None` for
/// code a person wrote. Minifiers strip the whitespace and leave the
/// punctuation, so the widest line is dense with `(){};,` and almost free of
/// spaces — which a long comment, a wide table or an embedded blob is not.
/// Whether a file's opening lines say a generator wrote it.
///
/// The banner is the industry's convention rather than one language's, and
/// it is the only thing that travels: gqlgen writes `generated.go` beside
/// the resolvers a person wrote, terraform writes `checkablekind_string.go`
/// beside `checkablekind.go`, and neither path says anything. 219 of
/// gqlgen's 865 go files carry a banner and hold 14363 of its 18653
/// functions. Nothing but a person's own code should be counted as the
/// program's, and the generator says so itself in its first lines.
/// What a Lua module hands out under a name that another module declares.
/// `spec/helpers.lua` binds `local cmd = reload_module("spec.internal.cmd")`
/// and returns a table of `start_kong = cmd.start_kong`: the name the
/// callers write is this file's, and the definition is that one's. kong
/// writes 103 such fields and 689 of its calls could not choose between the
/// spec files that declare the same names locally.
///
/// The binding is any call with a single string argument that names a
/// module -- `require` is the language's, and a project may wrap it, as
/// kong does.
pub fn lua_module_re_exports(source: &[u8]) -> Option<String> {
    let source = std::str::from_utf8(source).ok()?;
    let mut bound: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
    for line in source.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("local ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let value = value.trim();
        let Some(open) = value.find(['(', '"', '\'']) else {
            continue;
        };
        let after = &value[open..];
        let quoted = after
            .trim_start_matches('(')
            .trim_start()
            .strip_prefix(['"', '\''])?;
        let end = quoted.find(['"', '\''])?;
        let module = &quoted[..end];
        if module.is_empty()
            || !module
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '/')
        {
            continue;
        }
        bound.insert(name, module);
    }
    if bound.is_empty() {
        return None;
    }
    let mut re_exports: Vec<String> = Vec::new();
    for line in source.lines() {
        let line = line.trim().trim_end_matches(',');
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim().rsplit('.').next().unwrap_or("").trim();
        let value = value.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let Some((holder, field)) = value.split_once('.') else {
            continue;
        };
        if field != name || !field.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        if let Some(module) = bound.get(holder.trim()) {
            re_exports.push(format!("{name}>{module}"));
        }
    }
    (!re_exports.is_empty()).then(|| re_exports.join(";"))
}

pub fn generator_banner(source: &[u8]) -> bool {
    const BANNERS: [&str; 6] = [
        "do not edit",
        "do not modify",
        "do not manually edit",
        "automatically generated",
        "@generated",
        "<auto-generated",
    ];
    // The banner sits in the file's opening comment, and how long that is
    // belongs to the generator: ffigen writes seven `// ignore_for_file:`
    // lines before `// AUTO GENERATED FILE, DO NOT EDIT.`, which a
    // six-line window missed by two and left 2457 of that file's calls
    // reading as a person's. Reading the comment block instead stops at
    // the first line of code, so a `DO NOT EDIT` inside a template -- kong
    // returns one as a lua long string -- is not mistaken for a header.
    // What a generator writes at the head of a line when it does not shout.
    const OPENINGS: [&str; 5] = [
        "generated by",
        "generated code",
        "auto-generated",
        "autogenerated",
        "this file is auto-generated",
    ];
    const COMMENT_MARKERS: [&str; 8] = ["//", "--", "/*", "(*", "<!--", "\"\"\"", "'''", "{-"];
    let head = source.get(..8192).unwrap_or(source);
    let head = String::from_utf8_lossy(head);
    let mut header = String::new();
    for line in head.trim_start_matches('\u{feff}').lines().take(80) {
        let trimmed = line.trim_start();
        let is_comment = trimmed.is_empty()
            || trimmed.starts_with(['#', ';', '%', '*'])
            || COMMENT_MARKERS
                .iter()
                .any(|marker| trimmed.starts_with(marker));
        if !is_comment {
            break;
        }
        header.push_str(trimmed);
        header.push('\n');
        // A generator names itself at the head of the line -- `Generated by
        // Django 1.10.6 on ...`, `Generated by Home Manager.` -- while the
        // same words inside a sentence are prose: dplyr documents "a list
        // of columns generated by [vars()]". oscar ships 95 migrations that
        // say only the first thing, in its own source tree.
        let text = trimmed
            .trim_start_matches(['/', '-', '*', '#', ';', '%', '(', '<', '!', '{'])
            .trim_start();
        let text = text.to_ascii_lowercase();
        if OPENINGS.iter().any(|opening| text.starts_with(opening)) {
            return true;
        }
    }
    let header = header.to_ascii_lowercase();
    BANNERS.iter().any(|banner| header.contains(banner))
}

pub fn minified_line_length(source: &[u8]) -> Option<usize> {
    let longest = source
        .split(|byte| *byte == b'\n')
        .max_by_key(|line| line.len())?;
    let width = longest.len();
    let lines = source.iter().filter(|byte| **byte == b'\n').count() + 1;
    if width < 2_000 || source.len() / lines <= 200 {
        return None;
    }
    let spaces = longest
        .iter()
        .filter(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    let punctuation = longest
        .iter()
        .filter(|byte| {
            matches!(
                byte,
                b'{' | b'}'
                    | b'('
                    | b')'
                    | b'['
                    | b']'
                    | b';'
                    | b':'
                    | b','
                    | b'.'
                    | b'='
                    | b'+'
                    | b'-'
                    | b'*'
                    | b'/'
                    | b'<'
                    | b'>'
                    | b'!'
                    | b'&'
                    | b'|'
                    | b'?'
                    | b'%'
                    | b'^'
                    | b'~'
            )
        })
        .count();
    (spaces * 10 < width && punctuation * 10 > width).then_some(width)
}

/// A notebook is JSON holding a program: `.ipynb` names no language the
/// extension registry knows, and the program inside it is Python.
pub(crate) fn is_notebook_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ipynb"))
}

/// The paths a repository states it builds rather than keeps, read from
/// its own `.gitignore`. redis writes `src/release.h` there because a
/// script generates it, and the Flutter example writes
/// `**/windows/flutter/generated_plugin_registrant.h`: an import of
/// either finds nothing in a fresh checkout, and that is the build
/// working as designed rather than a dead link.
pub(crate) struct BuildProducts {
    globs: GlobSet,
    /// The literal paths among the patterns, for the includes that name a
    /// file from an include directory rather than from the repository
    /// root: `#include "flutter/generated_plugin_registrant.h"` names the
    /// tail of a path the `.gitignore` spells out in full.
    literal_paths: Vec<String>,
}

impl BuildProducts {
    pub(crate) fn builds(&self, candidate: &str) -> bool {
        let candidate = candidate.trim_start_matches("./");
        if self.globs.is_match(candidate) {
            return true;
        }
        self.literal_paths
            .iter()
            .any(|path| path.ends_with(&format!("/{candidate}")))
    }
}

pub(crate) fn build_product_globs(root: &Path) -> Option<BuildProducts> {
    let source = fs::read_to_string(root.join(".gitignore")).ok()?;
    let mut builder = GlobSetBuilder::new();
    let mut literal_paths = Vec::new();
    let mut any = false;
    for line in source.lines() {
        let pattern = line.trim();
        // A comment, a blank line, and a re-included path say nothing
        // about what the build writes.
        if pattern.is_empty() || pattern.starts_with('#') || pattern.starts_with('!') {
            continue;
        }
        let pattern = pattern.trim_end_matches('/');
        // gitignore anchors a pattern that holds a slash to the directory
        // its file sits in, and matches one without a slash at any depth.
        let anchored = pattern.trim_start_matches('/');
        let candidates = if pattern.contains('/') {
            vec![anchored.to_string(), format!("{anchored}/**")]
        } else {
            vec![
                anchored.to_string(),
                format!("**/{anchored}"),
                format!("**/{anchored}/**"),
            ]
        };
        if !anchored.contains(['*', '?', '[']) && anchored.contains('/') {
            literal_paths.push(anchored.to_string());
        }
        for candidate in candidates {
            if let Ok(glob) = Glob::new(&candidate) {
                builder.add(glob);
                any = true;
            }
        }
    }
    any.then(|| builder.build().ok())
        .flatten()
        .map(|globs| BuildProducts {
            globs,
            literal_paths,
        })
}

/// Whether an OCaml `open X` or `include X` names something the file binds
/// itself: a signature it states (`module type X`) or a functor parameter
/// (`module Make (X : S)`). Neither is another file's module, and dune's
/// vendored csexp opens its own functor parameter `Sexp` -- which the graph
/// answered with stdune's `sexp.ml`, closing a cycle that does not exist.
pub fn ocaml_module_bound_in_file(source: &str, label: &str) -> bool {
    let Some(name) = label
        .split_whitespace()
        .nth(1)
        .map(|name| name.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '\''))
    else {
        return false;
    };
    if name.is_empty() {
        return false;
    }
    source.lines().any(|line| {
        let line = line.trim();
        line.strip_prefix("module type ")
            .is_some_and(|rest| rest.split_whitespace().next() == Some(name))
            || line.contains(&format!("({name} :"))
            || line.contains(&format!("({name}:"))
    })
}
