//! Scan options, repository-owned scan policy from `.codegraph/config.toml`,
//! ignored-glob compilation, and indexer error types.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use thiserror::Error;

#[allow(unused_imports)]
use crate::*;

pub const DEFAULT_MAX_FILE_SIZE: u64 = 2 * 1024 * 1024;
// v3: module-level calls, Lua bindings, Haskell data constructors and the
// dropping of call labels that name nothing all changed what a parse of an
// unchanged file yields. The build identity below catches the changes
// nobody remembers to record here.
pub(crate) const PARSE_CACHE_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("failed to walk project tree at {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("failed to read project config at {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse project config at {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid project config at {path}: {message}")]
    ConfigInvalid { path: PathBuf, message: String },
    #[error("invalid scan options: {message}")]
    InvalidOptions { message: String },
    #[error("scan canceled")]
    Canceled,
}

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: u64,
    pub ignored_names: BTreeSet<String>,
    pub ignored_globs: BTreeSet<String>,
    pub parse_cache_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct IndexOptionOverrides {
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScanCoverageReport {
    pub root: String,
    pub include_hidden: bool,
    pub include_ignored: bool,
    pub max_file_size: u64,
    pub ignored_names: Vec<String>,
    pub ignored_globs: Vec<String>,
    pub directories_seen: usize,
    pub files_seen: usize,
    pub indexed_files: usize,
    pub skipped_large_files: usize,
    pub skipped_policy_entries: usize,
    pub skipped_hidden_entries: usize,
    pub skipped_ignored_name_entries: usize,
    pub skipped_ignored_glob_entries: usize,
    pub non_index_files: usize,
    pub seen_bytes: u64,
    pub indexed_bytes: u64,
    pub skipped_large_bytes: u64,
    pub languages: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryExclusion {
    Hidden,
    IgnoredName,
    IgnoredGlob,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_ignored: false,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            ignored_names: default_ignored_names(),
            ignored_globs: BTreeSet::new(),
            parse_cache_dir: None,
        }
    }
}

impl IndexOptions {
    pub fn with_parse_cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.parse_cache_dir = Some(dir.into());
        self
    }
}

pub fn configured_index_options(
    root: impl AsRef<Path>,
    overrides: &IndexOptionOverrides,
) -> Result<IndexOptions, IndexError> {
    let root = root.as_ref();
    let mut options = IndexOptions::default();
    let path = root.join(".codegraph").join("config.toml");
    if path.is_file() {
        let source = fs::read_to_string(&path).map_err(|source| IndexError::ConfigRead {
            path: path.clone(),
            source,
        })?;
        let value = toml::Value::Table(source.parse::<toml::Table>().map_err(|source| {
            IndexError::ConfigParse {
                path: path.clone(),
                source,
            }
        })?);
        apply_project_config(&mut options, &path, &value)?;
    }

    if overrides.include_hidden {
        options.include_hidden = true;
    }
    if overrides.include_ignored {
        options.include_ignored = true;
    }
    if let Some(max_file_size) = overrides.max_file_size {
        if max_file_size == 0 {
            return Err(IndexError::ConfigInvalid {
                path,
                message: "max_file_size must be greater than zero".to_string(),
            });
        }
        options.max_file_size = max_file_size;
    }

    Ok(options)
}

pub(crate) fn apply_project_config(
    options: &mut IndexOptions,
    path: &Path,
    value: &toml::Value,
) -> Result<(), IndexError> {
    let Some(scan) = value.get("scan") else {
        return Ok(());
    };
    let Some(scan) = scan.as_table() else {
        return Err(config_invalid(path, "[scan] must be a table"));
    };

    if let Some(include_hidden) = optional_bool(path, scan, "include_hidden")? {
        options.include_hidden = include_hidden;
    }
    if let Some(include_ignored) = optional_bool(path, scan, "include_ignored")? {
        options.include_ignored = include_ignored;
    }
    if let Some(max_file_size) = optional_u64(path, scan, "max_file_size")? {
        if max_file_size == 0 {
            return Err(config_invalid(
                path,
                "scan.max_file_size must be greater than zero",
            ));
        }
        options.max_file_size = max_file_size;
    }
    if let Some(ignored_names) = optional_string_array(path, scan, "ignored_names")? {
        options.ignored_names = ignored_names.into_iter().collect();
    }
    if let Some(extra_ignored_names) = optional_string_array(path, scan, "extra_ignored_names")? {
        options.ignored_names.extend(extra_ignored_names);
    }
    if let Some(ignored_globs) = optional_string_array(path, scan, "ignored_globs")? {
        validate_ignored_globs(path, "ignored_globs", &ignored_globs)?;
        options.ignored_globs = ignored_globs
            .into_iter()
            .map(|pattern| normalize_glob_pattern(&pattern))
            .collect();
    }
    if let Some(extra_ignored_globs) = optional_string_array(path, scan, "extra_ignored_globs")? {
        validate_ignored_globs(path, "extra_ignored_globs", &extra_ignored_globs)?;
        options.ignored_globs.extend(
            extra_ignored_globs
                .into_iter()
                .map(|pattern| normalize_glob_pattern(&pattern)),
        );
    }

    Ok(())
}

pub(crate) fn optional_bool(
    path: &Path,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<bool>, IndexError> {
    table
        .get(key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| config_invalid(path, &format!("scan.{key} must be a boolean")))
        })
        .transpose()
}

pub(crate) fn optional_u64(
    path: &Path,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<u64>, IndexError> {
    table
        .get(key)
        .map(|value| {
            value
                .as_integer()
                .and_then(|value| u64::try_from(value).ok())
                .ok_or_else(|| {
                    config_invalid(path, &format!("scan.{key} must be a positive integer"))
                })
        })
        .transpose()
}

pub(crate) fn optional_string_array(
    path: &Path,
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<Vec<String>>, IndexError> {
    table
        .get(key)
        .map(|value| {
            let Some(values) = value.as_array() else {
                return Err(config_invalid(
                    path,
                    &format!("scan.{key} must be an array"),
                ));
            };
            // Collect into a Result first so a non-string entry surfaces as a
            // config error; only then drop the empty strings. Filtering before
            // collecting would silently discard the Err alongside the empties.
            let collected = values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(|value| value.trim().to_string())
                        .ok_or_else(|| {
                            config_invalid(path, &format!("scan.{key} entries must be strings"))
                        })
                })
                .collect::<Result<Vec<String>, _>>()?;
            Ok(collected
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect())
        })
        .transpose()
}

pub(crate) fn validate_ignored_globs(
    path: &Path,
    key: &str,
    patterns: &[String],
) -> Result<(), IndexError> {
    for pattern in patterns {
        let normalized = normalize_glob_pattern(pattern);
        for expanded in expanded_ignored_glob_patterns(&normalized) {
            Glob::new(&expanded).map_err(|error| {
                config_invalid(path, &format!("scan.{key} contains invalid glob: {error}"))
            })?;
        }
    }
    Ok(())
}

/// Compile the ignore globs into a matcher. Shared with the cache fingerprint
/// (codegraph-storage) so both compile the patterns identically.
pub fn compile_ignored_globs(patterns: &BTreeSet<String>) -> Result<Option<GlobSet>, IndexError> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        for expanded in expanded_ignored_glob_patterns(pattern) {
            builder.add(
                Glob::new(&expanded).map_err(|error| IndexError::InvalidOptions {
                    message: format!("ignored glob `{pattern}` is invalid: {error}"),
                })?,
            );
        }
    }
    Ok(Some(builder.build().map_err(|error| {
        IndexError::InvalidOptions {
            message: format!("ignored glob set is invalid: {error}"),
        }
    })?))
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

pub(crate) fn config_invalid(path: &Path, message: &str) -> IndexError {
    IndexError::ConfigInvalid {
        path: path.to_path_buf(),
        message: message.to_string(),
    }
}
