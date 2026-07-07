use codegraph_core::{CODEGRAPH_SCHEMA_VERSION, CodeGraph};
use codegraph_indexer::{IndexError, IndexOptions, is_index_relevant_file, scan_project};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

const CACHE_SCHEMA_VERSION: u32 = 2;
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone)]
pub struct GraphCache {
    dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct CachedScan {
    pub graph: CodeGraph,
    pub cache: CacheInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheInfo {
    pub status: CacheStatus,
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    Disabled,
    Hit,
    Miss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFingerprint {
    pub hash: String,
    pub files: usize,
    pub bytes: u64,
    pub latest_modified_unix_nanos: u128,
    pub entries: Vec<FingerprintEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FingerprintEntry {
    pub path: String,
    pub bytes: u64,
    pub modified_unix_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheDiffReport {
    pub cache_dir: String,
    pub cache_record: CacheRecordStatus,
    pub reuse_strategy: CacheReuseStrategy,
    pub previous_hash: Option<String>,
    pub current_hash: String,
    pub previous_files: Option<usize>,
    pub current_files: usize,
    pub previous_bytes: Option<u64>,
    pub current_bytes: u64,
    pub changed_files: usize,
    pub reusable_files: usize,
    pub changed_current_bytes: u64,
    pub reusable_bytes: u64,
    pub reuse_file_ratio_basis_points: u16,
    pub reuse_byte_ratio_basis_points: u16,
    pub added: Vec<FingerprintEntry>,
    pub removed: Vec<FingerprintEntry>,
    pub modified: Vec<FingerprintChange>,
    pub unchanged: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheReuseStrategy {
    FullScan,
    PartialReuse,
    NoChanges,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheRecordStatus {
    Missing,
    Present,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FingerprintChange {
    pub path: String,
    pub previous_bytes: u64,
    pub current_bytes: u64,
    pub previous_modified_unix_nanos: u128,
    pub current_modified_unix_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CacheRecord {
    cache_schema_version: u32,
    graph_schema_version: u32,
    root: String,
    options_hash: String,
    fingerprint: ProjectFingerprint,
    graph: CodeGraph,
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("failed to walk project tree at {path}: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to encode or decode cache record: {0}")]
    Codec(#[from] serde_json::Error),
    #[error("failed to index project: {0}")]
    Index(#[from] IndexError),
}

impl GraphCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn fingerprint_project(
        root: &Path,
        options: &IndexOptions,
    ) -> Result<ProjectFingerprint, CacheError> {
        let mut hasher = StableHasher::new();
        hash_index_options(&mut hasher, options);
        let ignored_globs = compile_ignored_globs(&options.ignored_globs)?;

        let mut files = 0;
        let mut bytes = 0;
        let mut latest_modified_unix_nanos = 0;
        let mut entries = Vec::new();

        for entry in WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| should_enter(entry, root, options, &ignored_globs))
        {
            let entry = entry.map_err(|source| CacheError::Walk {
                path: root.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            if path == root || !entry.file_type().is_file() {
                continue;
            }

            let metadata = match path.metadata() {
                Ok(metadata) => metadata,
                Err(source) => {
                    return Err(CacheError::Io {
                        path: path.to_path_buf(),
                        source,
                    });
                }
            };
            if metadata.len() > options.max_file_size && !is_index_relevant_file(path) {
                continue;
            }

            let relative_path = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let modified = modified_unix_nanos(&metadata).unwrap_or(0);

            files += 1;
            bytes += metadata.len();
            latest_modified_unix_nanos = latest_modified_unix_nanos.max(modified);
            hasher.write_str(&relative_path);
            hasher.write_u64(metadata.len());
            hasher.write_u128(modified);
            entries.push(FingerprintEntry {
                path: relative_path,
                bytes: metadata.len(),
                modified_unix_nanos: modified,
            });
        }

        Ok(ProjectFingerprint {
            hash: format!("{:016x}", hasher.finish()),
            files,
            bytes,
            latest_modified_unix_nanos,
            entries,
        })
    }

    pub fn load(
        &self,
        root: &Path,
        options: &IndexOptions,
        fingerprint: &ProjectFingerprint,
    ) -> Result<Option<CodeGraph>, CacheError> {
        let path = self.cache_path(root, options);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(CacheError::Io { path, source }),
        };
        let record: CacheRecord = serde_json::from_slice(&bytes)?;
        if record.cache_schema_version != CACHE_SCHEMA_VERSION
            || record.graph_schema_version != record.graph.schema_version
            || record.graph_schema_version != CODEGRAPH_SCHEMA_VERSION
            || record.root != cache_root(root)
            || record.options_hash != options_hash(options)
            || record.fingerprint != *fingerprint
        {
            return Ok(None);
        }
        Ok(Some(record.graph))
    }

    pub fn diff(
        &self,
        root: &Path,
        options: &IndexOptions,
        limit: usize,
    ) -> Result<CacheDiffReport, CacheError> {
        let current = Self::fingerprint_project(root, options)?;
        let path = self.cache_path(root, options);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(diff_without_record(
                    self,
                    current,
                    CacheRecordStatus::Missing,
                    limit,
                ));
            }
            Err(source) => return Err(CacheError::Io { path, source }),
        };
        let record: CacheRecord = serde_json::from_slice(&bytes)?;
        if record.cache_schema_version != CACHE_SCHEMA_VERSION
            || record.graph_schema_version != record.graph.schema_version
            || record.graph_schema_version != CODEGRAPH_SCHEMA_VERSION
            || record.root != cache_root(root)
            || record.options_hash != options_hash(options)
        {
            return Ok(diff_without_record(
                self,
                current,
                CacheRecordStatus::Incompatible,
                limit,
            ));
        }

        Ok(diff_fingerprints(self, &record.fingerprint, current, limit))
    }

    pub fn store(
        &self,
        root: &Path,
        options: &IndexOptions,
        fingerprint: ProjectFingerprint,
        graph: &CodeGraph,
    ) -> Result<(), CacheError> {
        fs::create_dir_all(&self.dir).map_err(|source| CacheError::Io {
            path: self.dir.clone(),
            source,
        })?;
        let path = self.cache_path(root, options);
        let temporary_path = path.with_extension(format!(
            "json.tmp-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let record = CacheRecord {
            cache_schema_version: CACHE_SCHEMA_VERSION,
            graph_schema_version: graph.schema_version,
            root: cache_root(root),
            options_hash: options_hash(options),
            fingerprint,
            graph: graph.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&record)?;
        fs::write(&temporary_path, bytes).map_err(|source| CacheError::Io {
            path: temporary_path.clone(),
            source,
        })?;
        fs::rename(&temporary_path, &path).map_err(|source| CacheError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(())
    }

    fn cache_path(&self, root: &Path, options: &IndexOptions) -> PathBuf {
        let mut hasher = StableHasher::new();
        hasher.write_str(&cache_root(root));
        hasher.write_str(&options_hash(options));
        self.dir
            .join(format!("graph-{:016x}.json", hasher.finish()))
    }
}

pub fn scan_project_cached(
    root: impl AsRef<Path>,
    options: &IndexOptions,
    cache: Option<&GraphCache>,
) -> Result<CachedScan, CacheError> {
    let root = root.as_ref();
    let Some(cache) = cache else {
        return Ok(CachedScan {
            graph: scan_project(root, options)?,
            cache: CacheInfo {
                status: CacheStatus::Disabled,
                dir: None,
            },
        });
    };

    let fingerprint = GraphCache::fingerprint_project(root, options)?;
    if let Ok(Some(graph)) = cache.load(root, options, &fingerprint) {
        return Ok(CachedScan {
            graph,
            cache: CacheInfo {
                status: CacheStatus::Hit,
                dir: Some(cache.dir().display().to_string()),
            },
        });
    }

    let scan_options = options
        .clone()
        .with_parse_cache_dir(cache.dir().join("parse-facts"));
    let graph = scan_project(root, &scan_options)?;
    let _ = cache.store(root, options, fingerprint, &graph);
    Ok(CachedScan {
        graph,
        cache: CacheInfo {
            status: CacheStatus::Miss,
            dir: Some(cache.dir().display().to_string()),
        },
    })
}

pub fn default_cache_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("CODEGRAPH_CACHE_DIR") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(path).join("codegraph");
    }
    if cfg!(target_os = "macos")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("codegraph");
    }
    std::env::temp_dir().join("codegraph-cache")
}

fn diff_without_record(
    cache: &GraphCache,
    current: ProjectFingerprint,
    status: CacheRecordStatus,
    limit: usize,
) -> CacheDiffReport {
    let limit = limit.clamp(1, 10_000);
    let truncated = current.entries.len() > limit;
    CacheDiffReport {
        cache_dir: cache.dir().display().to_string(),
        cache_record: status,
        reuse_strategy: CacheReuseStrategy::FullScan,
        previous_hash: None,
        current_hash: current.hash,
        previous_files: None,
        current_files: current.files,
        previous_bytes: None,
        current_bytes: current.bytes,
        changed_files: current.files,
        reusable_files: 0,
        changed_current_bytes: current.bytes,
        reusable_bytes: 0,
        reuse_file_ratio_basis_points: 0,
        reuse_byte_ratio_basis_points: 0,
        added: current.entries.into_iter().take(limit).collect(),
        removed: Vec::new(),
        modified: Vec::new(),
        unchanged: 0,
        truncated,
    }
}

fn diff_fingerprints(
    cache: &GraphCache,
    previous: &ProjectFingerprint,
    current: ProjectFingerprint,
    limit: usize,
) -> CacheDiffReport {
    let limit = limit.clamp(1, 10_000);
    let previous_entries = previous
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<std::collections::BTreeMap<_, _>>();
    let current_entries = current
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();
    let mut unchanged = 0usize;
    let mut reusable_bytes = 0u64;
    let mut changed_current_bytes = 0u64;
    let mut total_changes = 0usize;

    for (path, current_entry) in &current_entries {
        match previous_entries.get(path) {
            Some(previous_entry)
                if previous_entry.bytes == current_entry.bytes
                    && previous_entry.modified_unix_nanos == current_entry.modified_unix_nanos =>
            {
                unchanged += 1;
                reusable_bytes = reusable_bytes.saturating_add(current_entry.bytes);
            }
            Some(previous_entry) => {
                total_changes += 1;
                changed_current_bytes = changed_current_bytes.saturating_add(current_entry.bytes);
                if modified.len() < limit {
                    modified.push(FingerprintChange {
                        path: (*path).to_string(),
                        previous_bytes: previous_entry.bytes,
                        current_bytes: current_entry.bytes,
                        previous_modified_unix_nanos: previous_entry.modified_unix_nanos,
                        current_modified_unix_nanos: current_entry.modified_unix_nanos,
                    });
                }
            }
            None => {
                total_changes += 1;
                changed_current_bytes = changed_current_bytes.saturating_add(current_entry.bytes);
                if added.len() < limit {
                    added.push((*current_entry).clone());
                }
            }
        }
    }

    for (path, previous_entry) in &previous_entries {
        if !current_entries.contains_key(path) {
            total_changes += 1;
            if removed.len() < limit {
                removed.push((*previous_entry).clone());
            }
        }
    }
    let reuse_strategy = if total_changes == 0 {
        CacheReuseStrategy::NoChanges
    } else if unchanged > 0 {
        CacheReuseStrategy::PartialReuse
    } else {
        CacheReuseStrategy::FullScan
    };

    CacheDiffReport {
        cache_dir: cache.dir().display().to_string(),
        cache_record: CacheRecordStatus::Present,
        reuse_strategy,
        previous_hash: Some(previous.hash.clone()),
        current_hash: current.hash,
        previous_files: Some(previous.files),
        current_files: current.files,
        previous_bytes: Some(previous.bytes),
        current_bytes: current.bytes,
        changed_files: total_changes,
        reusable_files: unchanged,
        changed_current_bytes,
        reusable_bytes,
        reuse_file_ratio_basis_points: ratio_basis_points(unchanged as u64, current.files as u64),
        reuse_byte_ratio_basis_points: ratio_basis_points(reusable_bytes, current.bytes),
        added,
        removed,
        modified,
        unchanged,
        truncated: total_changes > limit,
    }
}

fn ratio_basis_points(part: u64, total: u64) -> u16 {
    if total == 0 {
        return 10_000;
    }
    ((part.saturating_mul(10_000)) / total).min(10_000) as u16
}

fn should_enter(
    entry: &DirEntry,
    root: &Path,
    options: &IndexOptions,
    ignored_globs: &Option<GlobSet>,
) -> bool {
    if entry.path() == root {
        return true;
    }

    if !options.include_hidden && is_hidden(entry) {
        return false;
    }
    if !options.include_ignored
        && (is_ignored_name(entry, &options.ignored_names)
            || is_ignored_glob(entry.path(), root, ignored_globs))
    {
        return false;
    }
    true
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.') && name != ".")
}

fn is_ignored_name(entry: &DirEntry, ignored_names: &BTreeSet<String>) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| ignored_names.contains(name))
}

fn is_ignored_glob(path: &Path, root: &Path, ignored_globs: &Option<GlobSet>) -> bool {
    let Some(ignored_globs) = ignored_globs else {
        return false;
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    !relative.is_empty() && ignored_globs.is_match(relative)
}

fn compile_ignored_globs(patterns: &BTreeSet<String>) -> Result<Option<GlobSet>, IndexError> {
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

fn expanded_ignored_glob_patterns(pattern: &str) -> Vec<String> {
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

fn normalize_glob_pattern(pattern: &str) -> String {
    let mut normalized = pattern.trim().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }
    normalized
}

fn modified_unix_nanos(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
}

fn cache_root(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

fn options_hash(options: &IndexOptions) -> String {
    let mut hasher = StableHasher::new();
    hash_index_options(&mut hasher, options);
    format!("{:016x}", hasher.finish())
}

fn hash_index_options(hasher: &mut StableHasher, options: &IndexOptions) {
    hasher.write_bool(options.include_hidden);
    hasher.write_bool(options.include_ignored);
    hasher.write_u64(options.max_file_size);
    for name in &options.ignored_names {
        hasher.write_str(name);
    }
    for pattern in &options.ignored_globs {
        hasher.write_str(pattern);
    }
}

struct StableHasher {
    value: u64,
}

impl StableHasher {
    fn new() -> Self {
        Self { value: FNV_OFFSET }
    }

    fn finish(&self) -> u64 {
        self.value
    }

    fn write_bool(&mut self, value: bool) {
        self.write_bytes(&[u8::from(value)]);
    }

    fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_u128(&mut self, value: u128) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_str(&mut self, value: &str) {
        self.write_u64(value.len() as u64);
        self.write_bytes(value.as_bytes());
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.value ^= u64::from(*byte);
            self.value = self.value.wrapping_mul(FNV_PRIME);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_core::NodeKind;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn cache_round_trips_matching_fingerprint() {
        let root = temp_project_root();
        let cache_dir = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        let options = IndexOptions::default();
        let fingerprint = GraphCache::fingerprint_project(&root, &options).unwrap();
        let mut graph = CodeGraph::new("demo");
        graph.add_node(NodeKind::File, "src/main.rs");
        let cache = GraphCache::new(&cache_dir);

        assert!(cache.load(&root, &options, &fingerprint).unwrap().is_none());
        cache
            .store(&root, &options, fingerprint.clone(), &graph)
            .unwrap();
        let loaded = cache.load(&root, &options, &fingerprint).unwrap().unwrap();

        assert_eq!(loaded, graph);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(cache_dir).unwrap();
    }

    #[test]
    fn cache_uses_canonical_root_paths() {
        let root = temp_project_root();
        let cache_dir = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        let equivalent_root = root.join(".");
        let options = IndexOptions::default();
        let fingerprint = GraphCache::fingerprint_project(&equivalent_root, &options).unwrap();
        let graph = CodeGraph::new("demo");
        let cache = GraphCache::new(&cache_dir);

        cache
            .store(&root, &options, fingerprint.clone(), &graph)
            .unwrap();
        let loaded = cache
            .load(&equivalent_root, &options, &fingerprint)
            .unwrap()
            .unwrap();

        assert_eq!(loaded, graph);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(cache_dir).unwrap();
    }

    #[test]
    fn cache_misses_after_project_changes() {
        let root = temp_project_root();
        let cache_dir = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        let options = IndexOptions::default();
        let fingerprint = GraphCache::fingerprint_project(&root, &options).unwrap();
        let graph = CodeGraph::new("demo");
        let cache = GraphCache::new(&cache_dir);
        cache.store(&root, &options, fingerprint, &graph).unwrap();

        fs::write(
            root.join("src").join("main.rs"),
            "fn main() {}\nfn helper() {}\n",
        )
        .unwrap();
        let changed_fingerprint = GraphCache::fingerprint_project(&root, &options).unwrap();

        assert!(
            cache
                .load(&root, &options, &changed_fingerprint)
                .unwrap()
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(cache_dir).unwrap();
    }

    #[test]
    fn cache_misses_after_skipped_large_source_changes() {
        let root = temp_project_root();
        let cache_dir = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("huge.rs"), "fn main() {}\n").unwrap();
        let options = IndexOptions {
            max_file_size: 4,
            ..IndexOptions::default()
        };
        let cache = GraphCache::new(&cache_dir);

        let first = scan_project_cached(&root, &options, Some(&cache)).unwrap();
        assert_eq!(first.cache.status, CacheStatus::Miss);
        assert!(first.graph.nodes.iter().any(|node| {
            node.label == "src/huge.rs"
                && node
                    .metadata
                    .get("skipped_reason")
                    .is_some_and(|reason| reason == "max_file_size")
        }));
        let second = scan_project_cached(&root, &options, Some(&cache)).unwrap();
        assert_eq!(second.cache.status, CacheStatus::Hit);

        std::thread::sleep(std::time::Duration::from_millis(2));
        fs::write(
            root.join("src").join("huge.rs"),
            "fn main() {}\nfn changed() {}\n",
        )
        .unwrap();
        let third = scan_project_cached(&root, &options, Some(&cache)).unwrap();
        assert_eq!(third.cache.status, CacheStatus::Miss);

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(cache_dir).unwrap();
    }

    #[test]
    fn cached_scan_reports_miss_then_hit() {
        let root = temp_project_root();
        let cache_dir = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        let options = IndexOptions::default();
        let cache = GraphCache::new(&cache_dir);

        let first = scan_project_cached(&root, &options, Some(&cache)).unwrap();
        let second = scan_project_cached(&root, &options, Some(&cache)).unwrap();

        assert_eq!(first.cache.status, CacheStatus::Miss);
        assert_eq!(second.cache.status, CacheStatus::Hit);
        assert_eq!(first.graph, second.graph);
        assert!(
            fs::read_dir(cache_dir.join("parse-facts"))
                .unwrap()
                .next()
                .is_some(),
            "graph cache misses should populate persistent per-file parse facts"
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(cache_dir).unwrap();
    }

    #[test]
    fn cache_diff_reports_added_removed_and_modified_files() {
        let root = temp_project_root();
        let cache_dir = temp_project_root();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        fs::write(root.join("src").join("old.rs"), "pub fn old() {}\n").unwrap();
        let options = IndexOptions::default();
        let cache = GraphCache::new(&cache_dir);

        let missing = cache.diff(&root, &options, 10).unwrap();
        assert_eq!(missing.cache_record, CacheRecordStatus::Missing);
        assert_eq!(missing.reuse_strategy, CacheReuseStrategy::FullScan);
        assert_eq!(missing.changed_files, 2);
        assert_eq!(missing.reusable_files, 0);
        assert_eq!(missing.reuse_file_ratio_basis_points, 0);
        assert_eq!(missing.added.len(), 2);

        let first = scan_project_cached(&root, &options, Some(&cache)).unwrap();
        assert_eq!(first.cache.status, CacheStatus::Miss);

        std::thread::sleep(std::time::Duration::from_millis(2));
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() {}\nfn changed() {}\n",
        )
        .unwrap();
        fs::write(root.join("src").join("new.rs"), "pub fn new() {}\n").unwrap();
        fs::remove_file(root.join("src").join("old.rs")).unwrap();
        fs::write(root.join("src").join("stable.rs"), "pub fn stable() {}\n").unwrap();

        let diff = cache.diff(&root, &options, 10).unwrap();
        assert_eq!(diff.cache_record, CacheRecordStatus::Present);
        assert_eq!(diff.previous_files, Some(2));
        assert_eq!(diff.current_files, 3);
        assert_eq!(diff.changed_files, 4);
        assert_eq!(diff.reusable_files, 0);
        assert_eq!(diff.reuse_strategy, CacheReuseStrategy::FullScan);
        assert!(diff.added.iter().any(|entry| entry.path == "src/new.rs"));
        assert!(diff.added.iter().any(|entry| entry.path == "src/stable.rs"));
        assert!(diff.removed.iter().any(|entry| entry.path == "src/old.rs"));
        assert!(
            diff.modified
                .iter()
                .any(|entry| entry.path == "src/main.rs")
        );
        assert!(!diff.truncated);

        let second = scan_project_cached(&root, &options, Some(&cache)).unwrap();
        assert_eq!(second.cache.status, CacheStatus::Miss);

        let clean = cache.diff(&root, &options, 10).unwrap();
        assert_eq!(clean.reuse_strategy, CacheReuseStrategy::NoChanges);
        assert_eq!(clean.changed_files, 0);
        assert_eq!(clean.reusable_files, 3);
        assert_eq!(clean.reuse_file_ratio_basis_points, 10_000);

        std::thread::sleep(std::time::Duration::from_millis(2));
        fs::write(root.join("src").join("new.rs"), "pub fn newer() {}\n").unwrap();

        let reuse = cache.diff(&root, &options, 10).unwrap();
        assert_eq!(reuse.cache_record, CacheRecordStatus::Present);
        assert_eq!(reuse.reuse_strategy, CacheReuseStrategy::PartialReuse);
        assert_eq!(reuse.changed_files, 1);
        assert_eq!(reuse.reusable_files, 2);
        assert_eq!(reuse.reuse_file_ratio_basis_points, 6666);
        assert!(reuse.reusable_bytes > 0);
        assert!(reuse.changed_current_bytes > 0);

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(cache_dir).unwrap();
    }

    fn temp_project_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "codegraph-storage-test-{}-{nanos}-{id}",
            std::process::id()
        ))
    }
}
