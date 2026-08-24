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
    if is_markdown_document(path) || is_rst_document(path) {
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
