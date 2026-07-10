//! Watch mode: automatic local graph refresh while editing.
//!
//! Polls the project fingerprint (the same content hash the persistent cache
//! uses) and, whenever it changes, runs the safe incremental update path:
//! changed files are rescanned, unchanged file chunks are reused, and the
//! cache record is replaced only when the merged graph is complete. Events
//! stream to stdout as NDJSON so humans can follow refreshes and tools can
//! pipe them. Polling keeps the watcher dependency-free and editor-agnostic;
//! a warm cache that already matches the tree syncs silently instead of
//! emitting a noop refresh.

use anyhow::Result;
use codegraph_indexer::IndexOptions;
use codegraph_storage::{GraphCache, IncrementalPlanAction, scan_project_cached};
use serde::Serialize;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WatchOptions {
    pub interval_ms: u64,
    /// Stop after this many refresh events; `None` watches until interrupted.
    pub max_refreshes: Option<usize>,
    /// Maximum changed-file paths listed per refresh event.
    pub limit: usize,
}

#[derive(Debug, Serialize)]
pub struct WatchRefreshEvent {
    pub event: &'static str,
    pub recorded_at_unix: u64,
    pub action: &'static str,
    pub previous_hash: Option<String>,
    pub current_hash: String,
    pub changed_files: usize,
    pub rescanned_files: usize,
    pub removed_files: usize,
    pub scan_paths: Vec<String>,
    pub removed_paths: Vec<String>,
    pub nodes: usize,
    pub edges: usize,
    pub stored: bool,
    /// True when the incremental merge was not surface-stable and the watcher
    /// fell back to a full rescan so the cache still converges.
    pub fallback_full_scan: bool,
    pub reason: String,
    pub duration_ms: u64,
}

#[derive(Debug)]
pub struct WatchTick {
    pub current_hash: String,
    pub refresh: Option<WatchRefreshEvent>,
}

/// One poll step: compare the project fingerprint against the last seen hash
/// and run a safe incremental cache update when they differ. A `Noop` update
/// (warm cache already matching the tree) syncs the hash without an event.
pub fn watch_tick(
    cache: &GraphCache,
    root: &Path,
    options: &IndexOptions,
    last_hash: Option<&str>,
    limit: usize,
    recorded_at_unix: u64,
) -> Result<WatchTick> {
    let current = GraphCache::fingerprint_project(root, options)?;
    if last_hash == Some(current.hash.as_str()) {
        return Ok(WatchTick {
            current_hash: current.hash,
            refresh: None,
        });
    }

    let started = Instant::now();
    let update = cache.incremental_update(root, options, limit)?;
    if update.preview.plan.action == IncrementalPlanAction::Noop {
        return Ok(WatchTick {
            current_hash: update.cache.current_hash,
            refresh: None,
        });
    }

    let mut event = WatchRefreshEvent {
        event: "refreshed",
        recorded_at_unix,
        action: match update.preview.plan.action {
            IncrementalPlanAction::FullScan => "full_scan",
            IncrementalPlanAction::PartialRescan => "partial_rescan",
            IncrementalPlanAction::Noop => unreachable!("noop handled above"),
        },
        previous_hash: update.preview.plan.previous_hash.clone(),
        current_hash: update.cache.current_hash.clone(),
        changed_files: update.preview.plan.changed_files,
        rescanned_files: update.preview.plan.rescan_files,
        removed_files: update.preview.plan.removed_files,
        scan_paths: update.preview.plan.scan_paths.clone(),
        removed_paths: update.preview.plan.removed_paths.clone(),
        nodes: update.preview.graph.nodes.len(),
        edges: update.preview.graph.edges.len(),
        stored: update.cache.stored,
        fallback_full_scan: false,
        reason: update.cache.reason.clone(),
        duration_ms: started.elapsed().as_millis() as u64,
    };
    if !update.cache.stored {
        // The merge was not surface-stable (for example a new file added
        // top-level nodes), so the safe update refused to persist it. Watch
        // mode must still converge: run a storing full rescan instead.
        let output = scan_project_cached(root, options, Some(cache))?;
        event.action = "full_scan";
        event.fallback_full_scan = true;
        event.stored = true;
        event.nodes = output.graph.nodes.len();
        event.edges = output.graph.edges.len();
        event.reason = format!(
            "{}; ran a full rescan and stored the complete graph",
            update.cache.reason
        );
        event.duration_ms = started.elapsed().as_millis() as u64;
    }
    Ok(WatchTick {
        current_hash: update.cache.current_hash,
        refresh: Some(event),
    })
}

/// Poll loop: first tick runs immediately, then every `interval_ms`. Scan
/// errors become NDJSON `error` events and the watcher keeps running.
pub fn run_watch(
    cache: &GraphCache,
    root: &Path,
    options: &IndexOptions,
    watch_options: &WatchOptions,
) -> Result<()> {
    let started = serde_json::json!({
        "event": "watching",
        "path": root.display().to_string(),
        "interval_ms": watch_options.interval_ms,
        "cache_dir": cache.dir().display().to_string(),
    });
    println!("{started}");
    std::io::stdout().flush().ok();

    let mut last_hash: Option<String> = None;
    let mut refreshes = 0usize;
    loop {
        match watch_tick(
            cache,
            root,
            options,
            last_hash.as_deref(),
            watch_options.limit,
            crate::query_log::unix_now(),
        ) {
            Ok(tick) => {
                last_hash = Some(tick.current_hash);
                if let Some(refresh) = tick.refresh {
                    println!("{}", serde_json::to_string(&refresh)?);
                    std::io::stdout().flush().ok();
                    refreshes += 1;
                    if watch_options
                        .max_refreshes
                        .is_some_and(|max| refreshes >= max)
                    {
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                let event = serde_json::json!({
                    "event": "error",
                    "message": format!("{error:#}"),
                });
                println!("{event}");
                std::io::stdout().flush().ok();
            }
        }
        std::thread::sleep(Duration::from_millis(watch_options.interval_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(kind: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .subsec_nanos();
        let counter = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "codegraph-watch-{kind}-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn cold_start_refreshes_then_idles_then_sees_changes() {
        let root = temp_dir("root");
        let cache_dir = temp_dir("cache");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() { helper(); }\n\nfn helper() {}\n",
        )
        .unwrap();
        let cache = GraphCache::new(&cache_dir);
        let options = IndexOptions::default();

        let first = watch_tick(&cache, &root, &options, None, 100, 1).expect("first tick");
        let refresh = first.refresh.expect("cold cache should refresh");
        assert_eq!(refresh.action, "full_scan");
        assert!(refresh.stored, "complete graph should be stored");
        assert!(refresh.nodes > 0);

        let idle = watch_tick(&cache, &root, &options, Some(&first.current_hash), 100, 2)
            .expect("idle tick");
        assert_eq!(idle.current_hash, first.current_hash);
        assert!(
            idle.refresh.is_none(),
            "unchanged fingerprint must not refresh"
        );

        // Body-only edit: surface-stable, so the partial merge is stored.
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() { helper(); }\n\nfn helper() {\n    let _edited = 1;\n}\n",
        )
        .unwrap();
        let edited = watch_tick(&cache, &root, &options, Some(&first.current_hash), 100, 3)
            .expect("edited tick");
        let refresh = edited.refresh.expect("body edit should refresh");
        assert!(refresh.stored, "refresh must leave a stored cache record");
        assert_eq!(refresh.changed_files, 1);
        assert_eq!(
            refresh.previous_hash.as_deref(),
            Some(first.current_hash.as_str())
        );

        // New file adds graph surface: the safe update refuses to store, so
        // the watcher falls back to a storing full rescan.
        fs::write(root.join("src").join("extra.rs"), "pub fn extra() {}\n").unwrap();
        let added = watch_tick(&cache, &root, &options, Some(&edited.current_hash), 100, 4)
            .expect("added tick");
        let refresh = added.refresh.expect("new file should refresh");
        assert_eq!(refresh.action, "full_scan");
        assert!(refresh.fallback_full_scan);
        assert!(refresh.stored);
        assert!(refresh.reason.contains("full rescan"));

        let settled = watch_tick(&cache, &root, &options, Some(&added.current_hash), 100, 5)
            .expect("settled tick");
        assert!(
            settled.refresh.is_none(),
            "fallback full scan must leave the cache converged"
        );
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(cache_dir).ok();
    }

    #[test]
    fn warm_cache_start_syncs_silently() {
        let root = temp_dir("root");
        let cache_dir = temp_dir("cache");
        fs::write(root.join("lib.py"), "def worker():\n    return 1\n").unwrap();
        let cache = GraphCache::new(&cache_dir);
        let options = IndexOptions::default();

        let first = watch_tick(&cache, &root, &options, None, 100, 1).expect("first tick");
        assert!(first.refresh.is_some());

        // A restarted watcher has no last hash, but the cache already matches
        // the tree: sync the hash without emitting a noop refresh event.
        let restarted = watch_tick(&cache, &root, &options, None, 100, 2).expect("restart tick");
        assert_eq!(restarted.current_hash, first.current_hash);
        assert!(
            restarted.refresh.is_none(),
            "warm matching cache must not emit a refresh event"
        );
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(cache_dir).ok();
    }
}
