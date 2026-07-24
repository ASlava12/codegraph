//! Persistent per-file parser fact cache keyed by language, path, size,
//! and modification time.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use codegraph_parser::{Language, LanguageAdapter, ParsedFile};

#[allow(unused_imports)]
use crate::*;

pub(crate) fn parse_source_cached(
    options: &IndexOptions,
    stamp: Option<FileStamp>,
    label: &str,
    source: &[u8],
    adapter: &dyn LanguageAdapter,
) -> Result<ParsedFile, codegraph_parser::ParseError> {
    let Some(cache_dir) = options.parse_cache_dir.as_deref() else {
        return adapter.parse(Path::new(label), source);
    };
    // The stamp is taken by the caller BEFORE reading the content (see
    // index_file) so a mid-scan edit can only self-heal, never pin stale facts.
    let Some(stamp) = stamp else {
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
        // Atomic like the graph/semantic caches: a crash mid-write must not
        // leave a torn record (it would be silently reparsed, but tmp+rename
        // is as cheap and keeps the file always well-formed).
        let final_path = parse_cache_path(cache_dir, label, language);
        let temporary = final_path.with_extension(format!(
            "json.tmp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        if fs::write(&temporary, bytes).is_ok() && fs::rename(&temporary, &final_path).is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }
}

pub(crate) fn parse_cache_path(cache_dir: &Path, label: &str, language: Language) -> PathBuf {
    // Stable FNV-1a: DefaultHasher's output is not guaranteed across Rust
    // releases, so a toolchain upgrade orphaned every cache entry forever.
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in language.name().bytes().chain([0u8]).chain(label.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    cache_dir.join(format!("parse-{hash:016x}.json"))
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
