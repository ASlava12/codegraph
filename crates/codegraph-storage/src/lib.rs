use codegraph_core::{CODEGRAPH_SCHEMA_VERSION, CodeGraph};
use codegraph_indexer::IndexOptions;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

const CACHE_SCHEMA_VERSION: u32 = 1;
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone)]
pub struct GraphCache {
    dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFingerprint {
    pub hash: String,
    pub files: usize,
    pub bytes: u64,
    pub latest_modified_unix_nanos: u128,
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

        let mut files = 0;
        let mut bytes = 0;
        let mut latest_modified_unix_nanos = 0;

        for entry in WalkDir::new(root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|entry| should_enter(entry, options))
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
            if metadata.len() > options.max_file_size {
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
        }

        Ok(ProjectFingerprint {
            hash: format!("{:016x}", hasher.finish()),
            files,
            bytes,
            latest_modified_unix_nanos,
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

fn should_enter(entry: &DirEntry, options: &IndexOptions) -> bool {
    if !options.include_hidden && is_hidden(entry) {
        return false;
    }
    if !options.include_ignored && is_ignored_name(entry, &options.ignored_names) {
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

fn modified_unix_nanos(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
}

fn cache_root(root: &Path) -> String {
    root.to_string_lossy().replace('\\', "/")
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
