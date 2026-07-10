//! Persistent per-file parser fact cache keyed by language, path, size,
//! and modification time.

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use codegraph_parser::{Language, LanguageAdapter, ParsedFile};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn parse_source_cached(
    options: &IndexOptions,
    path: &Path,
    label: &str,
    source: &[u8],
    adapter: &dyn LanguageAdapter,
) -> Result<ParsedFile, codegraph_parser::ParseError> {
    let Some(cache_dir) = options.parse_cache_dir.as_deref() else {
        return adapter.parse(Path::new(label), source);
    };
    let Some(stamp) = file_stamp(path) else {
        return adapter.parse(Path::new(label), source);
    };
    let language = adapter.language();

    if let Some(parsed) = load_cached_parse(cache_dir, label, language, stamp) {
        return Ok(parsed);
    }

    let parsed = adapter.parse(Path::new(label), source)?;
    store_cached_parse(cache_dir, label, language, stamp, &parsed);
    Ok(parsed)
}

pub(crate) fn load_cached_parse(
    cache_dir: &Path,
    label: &str,
    language: Language,
    stamp: FileStamp,
) -> Option<ParsedFile> {
    let path = parse_cache_path(cache_dir, label, language);
    let bytes = fs::read(path).ok()?;
    let record: ParseCacheRecord = serde_json::from_slice(&bytes).ok()?;
    if record.cache_schema_version == PARSE_CACHE_SCHEMA_VERSION
        && record.language == language
        && record.stamp == stamp
    {
        Some(record.parsed)
    } else {
        None
    }
}

pub(crate) fn store_cached_parse(
    cache_dir: &Path,
    label: &str,
    language: Language,
    stamp: FileStamp,
    parsed: &ParsedFile,
) {
    let record = ParseCacheRecord {
        cache_schema_version: PARSE_CACHE_SCHEMA_VERSION,
        language,
        stamp,
        parsed: parsed.clone(),
    };
    if fs::create_dir_all(cache_dir).is_err() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(&record) {
        let _ = fs::write(parse_cache_path(cache_dir, label, language), bytes);
    }
}

pub(crate) fn parse_cache_path(cache_dir: &Path, label: &str, language: Language) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    language.hash(&mut hasher);
    label.hash(&mut hasher);
    cache_dir.join(format!("parse-{:016x}.json", hasher.finish()))
}

pub(crate) fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());
    Some(FileStamp {
        len: metadata.len(),
        modified_ns,
    })
}
