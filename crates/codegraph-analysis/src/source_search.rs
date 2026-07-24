//! Bounded source text search with compact context snippets.

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

#[allow(unused_imports)]
use crate::*;

pub fn search_source(root: &Path, request: &SourceSearchRequest) -> SourceSearchResult {
    let query = request.query.trim();
    if query.is_empty() {
        return SourceSearchResult {
            query: request.query.clone(),
            path_filter: request.path_filter.clone(),
            case_sensitive: request.case_sensitive,
            total_matches: 0,
            matches: Vec::new(),
            truncated: false,
        };
    }

    let needle = if request.case_sensitive {
        query.to_string()
    } else {
        query.to_ascii_lowercase()
    };
    let path_filter = request
        .path_filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if request.case_sensitive {
                value.to_string()
            } else {
                value.to_ascii_lowercase()
            }
        });
    let limit = request.limit.clamp(1, 1_000);
    let context = request.context.min(20);
    let ignored_globs = compile_ignored_globs(&request.ignored_globs);
    let mut matches = Vec::new();
    let mut total_matches = 0usize;

    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_search_entry(entry, root, request, &ignored_globs))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root || !entry.file_type().is_file() {
            continue;
        }
        if !is_searchable_file(path, request.max_file_size) {
            continue;
        }
        let label = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if path_filter.as_deref().is_some_and(|filter| {
            let haystack = if request.case_sensitive {
                label.clone()
            } else {
                label.to_ascii_lowercase()
            };
            !haystack.contains(filter)
        }) {
            continue;
        }

        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let lines = text.lines().collect::<Vec<_>>();
        for (line_index, line) in lines.iter().enumerate() {
            let haystack = if request.case_sensitive {
                line.to_string()
            } else {
                line.to_ascii_lowercase()
            };
            let mut search_from = 0usize;
            while let Some(offset) = haystack[search_from..].find(&needle) {
                total_matches += 1;
                if matches.len() < limit {
                    let column = search_from + offset + 1;
                    matches.push(SourceSearchMatch {
                        path: label.clone(),
                        line: line_index as u32 + 1,
                        column: column as u32,
                        line_text: (*line).to_string(),
                        context: source_search_context(&lines, line_index, context),
                    });
                }
                search_from += offset + needle.len().max(1);
            }
        }
    }

    SourceSearchResult {
        query: request.query.clone(),
        path_filter: request.path_filter.clone(),
        case_sensitive: request.case_sensitive,
        total_matches,
        truncated: total_matches > matches.len(),
        matches,
    }
}

pub(crate) fn source_search_context(
    lines: &[&str],
    line_index: usize,
    context: usize,
) -> Vec<SourceSearchLine> {
    let start = line_index.saturating_sub(context);
    let end = (line_index + context + 1).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(offset, line)| {
            let index = start + offset;
            SourceSearchLine {
                number: index as u32 + 1,
                text: (*line).to_string(),
                highlight: index == line_index,
            }
        })
        .collect()
}

pub(crate) fn should_search_entry(
    entry: &DirEntry,
    root: &Path,
    request: &SourceSearchRequest,
    ignored_globs: &Option<GlobSet>,
) -> bool {
    if entry.path() == root {
        return true;
    }

    if !request.include_hidden && is_hidden_entry(entry) {
        return false;
    }
    if !request.include_ignored
        && (is_ignored_name(entry, request) || is_ignored_glob(entry.path(), root, ignored_globs))
    {
        return false;
    }
    true
}

pub(crate) fn is_ignored_name(entry: &DirEntry, request: &SourceSearchRequest) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| request.ignored_names.contains(name))
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

pub(crate) fn compile_ignored_globs(patterns: &BTreeSet<String>) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        for expanded in expanded_ignored_glob_patterns(pattern) {
            builder.add(Glob::new(&expanded).ok()?);
        }
    }
    builder.build().ok()
}

pub(crate) fn expanded_ignored_glob_patterns(pattern: &str) -> Vec<String> {
    let normalized = normalize_glob_pattern(pattern);
    let mut patterns = vec![normalized.clone()];
    for suffix in ["/**", "/**/*"] {
        if let Some(prefix) = normalized.strip_suffix(suffix)
            && !prefix.is_empty()
        {
            patterns.push(prefix.to_string());
        }
    }
    patterns
}

pub(crate) fn normalize_glob_pattern(pattern: &str) -> String {
    let mut normalized = pattern.trim().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }
    normalized
}

pub(crate) fn is_hidden_entry(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.') && name != "." && name != "..")
}

pub(crate) fn is_searchable_file(path: &Path, max_file_size: u64) -> bool {
    path.metadata()
        .map(|metadata| metadata.len() <= max_file_size)
        .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsightReport {
    pub total: usize,
    pub by_severity: BTreeMap<String, usize>,
    pub by_kind: BTreeMap<String, usize>,
    pub insights: Vec<Insight>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsightFilter {
    pub severity: Option<InsightSeverity>,
    pub kind: Option<String>,
    pub search: Option<String>,
    pub limit: usize,
}
