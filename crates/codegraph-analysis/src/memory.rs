//! Saved investigation memory: query outcomes linked to graph nodes.
//!
//! Records live in repository-owned `.codegraph/memory.jsonl`, one JSON
//! record per line, so investigation lessons survive between sessions and can
//! be committed alongside rules and annotations. Every record stores the
//! project fingerprint hash at save time; listings compare it against the
//! current fingerprint and mark records from a changed source tree as stale
//! instead of silently trusting outdated conclusions.

use crate::QueryError;

type Result<T> = std::result::Result<T, QueryError>;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const MEMORY_FILE: &str = "memory.jsonl";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOutcome {
    /// The query answered the investigation question.
    Useful,
    /// The query led nowhere; avoid repeating this direction.
    DeadEnd,
    /// The original conclusion was wrong and this record corrects it.
    Corrected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub recorded_at_unix: u64,
    pub query: String,
    pub outcome: MemoryOutcome,
    #[serde(default)]
    pub note: Option<String>,
    /// The nodes the investigation was about, as the caller named them: a
    /// durable `cg-*` id, or the positional `n42`/`42` form. A memory
    /// outlives the scan that produced it, and only the durable id still
    /// points at the same definition after an edit above it.
    #[serde(default, deserialize_with = "node_references")]
    pub node_ids: Vec<String>,
    pub fingerprint: String,
}

/// Node references from a record, accepting the numbers older records wrote.
fn node_references<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Reference {
        Text(String),
        Number(u64),
    }

    Ok(Vec::<Reference>::deserialize(deserializer)?
        .into_iter()
        .map(|reference| match reference {
            Reference::Text(value) => value,
            Reference::Number(value) => format!("n{value}"),
        })
        .collect())
}

#[derive(Debug, Serialize)]
pub struct MemoryListEntry {
    #[serde(flatten)]
    pub record: MemoryRecord,
    pub stale: bool,
}

#[derive(Debug, Serialize)]
pub struct MemoryListReport {
    pub total: usize,
    pub stale: usize,
    pub malformed_lines: usize,
    pub current_fingerprint: String,
    pub records: Vec<MemoryListEntry>,
}

impl std::str::FromStr for MemoryOutcome {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "useful" => Ok(Self::Useful),
            "dead_end" => Ok(Self::DeadEnd),
            "corrected" => Ok(Self::Corrected),
            other => Err(format!(
                "unknown outcome `{other}`; expected useful, dead_end, or corrected"
            )),
        }
    }
}

pub(crate) fn memory_path(root: &Path) -> PathBuf {
    root.join(".codegraph").join(MEMORY_FILE)
}

/// One saved investigation outcome, shared by the CLI `memory-save`
/// command and the MCP `memory_save` tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySaveRequest {
    pub query: String,
    pub outcome: MemoryOutcome,
    pub note: Option<String>,
    pub node_ids: Vec<String>,
    pub fingerprint: String,
    pub recorded_at_unix: u64,
}

pub fn save_memory(root: &Path, request: MemorySaveRequest) -> Result<MemoryRecord> {
    let MemorySaveRequest {
        query,
        outcome,
        note,
        node_ids,
        fingerprint,
        recorded_at_unix,
    } = request;
    let (existing, _) = load_records(root)?;
    let record = MemoryRecord {
        id: format!("mem-{}", existing.len() + 1),
        recorded_at_unix,
        query,
        outcome,
        note,
        node_ids,
        fingerprint,
    };

    let path = memory_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            QueryError::new(format!("failed to create {}: {error}", parent.display()))
        })?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| QueryError::new(format!("failed to open {}: {error}", path.display())))?;
    let line = serde_json::to_string(&record)
        .map_err(|error| QueryError::new(format!("failed to encode memory record: {error}")))?;
    file.write_all(line.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .map_err(|error| QueryError::new(format!("failed to write {}: {error}", path.display())))?;
    Ok(record)
}

pub fn load_records(root: &Path) -> Result<(Vec<MemoryRecord>, usize)> {
    let path = memory_path(root);
    if !path.exists() {
        return Ok((Vec::new(), 0));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| QueryError::new(format!("failed to read {}: {error}", path.display())))?;
    let mut records = Vec::new();
    let mut malformed = 0usize;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<MemoryRecord>(line) {
            Ok(record) => records.push(record),
            Err(_) => malformed += 1,
        }
    }
    Ok((records, malformed))
}

pub fn list_memory(
    root: &Path,
    current_fingerprint: &str,
    outcome: Option<MemoryOutcome>,
    only_stale: bool,
) -> Result<MemoryListReport> {
    let (records, malformed_lines) = load_records(root)?;
    let entries = records
        .into_iter()
        .filter(|record| outcome.is_none_or(|expected| record.outcome == expected))
        .map(|record| {
            let stale = record.fingerprint != current_fingerprint;
            MemoryListEntry { record, stale }
        })
        .filter(|entry| !only_stale || entry.stale)
        .collect::<Vec<_>>();
    Ok(MemoryListReport {
        total: entries.len(),
        stale: entries.iter().filter(|entry| entry.stale).count(),
        malformed_lines,
        current_fingerprint: current_fingerprint.to_string(),
        records: entries,
    })
}

pub const REFLECTION_SCHEMA: &str = "codegraph.reflection.v1";

#[derive(Debug, Clone, Serialize)]
pub struct LessonRecord {
    pub id: String,
    pub query: String,
    pub outcome: MemoryOutcome,
    #[serde(default)]
    pub note: Option<String>,
    pub recorded_at_unix: u64,
    pub stale: bool,
}

#[derive(Debug, Serialize)]
pub struct NodeLesson {
    /// The reference the memory stored, durable or positional.
    pub node_id: String,
    #[serde(default)]
    pub label: Option<String>,
    pub missing: bool,
    pub records: Vec<LessonRecord>,
}

#[derive(Debug, Serialize)]
pub struct ReflectionReport {
    pub schema: String,
    pub current_fingerprint: String,
    pub total_records: usize,
    pub stale_records: usize,
    pub malformed_lines: usize,
    pub outcome_counts: std::collections::BTreeMap<String, usize>,
    pub dead_ends: Vec<LessonRecord>,
    pub corrections: Vec<LessonRecord>,
    pub node_lessons: Vec<NodeLesson>,
    pub general_lessons: Vec<LessonRecord>,
    pub stale_warnings: Vec<String>,
}

fn outcome_name(outcome: MemoryOutcome) -> &'static str {
    match outcome {
        MemoryOutcome::Useful => "useful",
        MemoryOutcome::DeadEnd => "dead_end",
        MemoryOutcome::Corrected => "corrected",
    }
}

pub fn reflect(
    root: &Path,
    current_fingerprint: &str,
    node_labels: &std::collections::BTreeMap<String, String>,
) -> Result<ReflectionReport> {
    let (records, malformed_lines) = load_records(root)?;
    let mut outcome_counts = std::collections::BTreeMap::new();
    let mut dead_ends = Vec::new();
    let mut corrections = Vec::new();
    let mut node_records: std::collections::BTreeMap<String, Vec<LessonRecord>> =
        std::collections::BTreeMap::new();
    let mut general_lessons = Vec::new();
    let mut stale_warnings = Vec::new();
    let mut stale_records = 0usize;

    for record in &records {
        let stale = record.fingerprint != current_fingerprint;
        if stale {
            stale_records += 1;
            stale_warnings.push(format!(
                "{}: recorded against fingerprint {} but the source tree changed; re-verify before trusting `{}`",
                record.id,
                &record.fingerprint[..record.fingerprint.len().min(12)],
                record.query
            ));
        }
        *outcome_counts
            .entry(outcome_name(record.outcome).to_string())
            .or_insert(0) += 1;
        let lesson = LessonRecord {
            id: record.id.clone(),
            query: record.query.clone(),
            outcome: record.outcome,
            note: record.note.clone(),
            recorded_at_unix: record.recorded_at_unix,
            stale,
        };
        match record.outcome {
            MemoryOutcome::DeadEnd => dead_ends.push(lesson.clone()),
            MemoryOutcome::Corrected => corrections.push(lesson.clone()),
            MemoryOutcome::Useful => {}
        }
        if record.node_ids.is_empty() {
            general_lessons.push(lesson);
        } else {
            for node_id in &record.node_ids {
                node_records
                    .entry(node_id.clone())
                    .or_default()
                    .push(lesson.clone());
            }
        }
    }

    let node_lessons = node_records
        .into_iter()
        .map(|(node_id, records)| {
            let label = node_labels.get(&node_id).cloned();
            NodeLesson {
                node_id,
                missing: label.is_none(),
                label,
                records,
            }
        })
        .collect();

    Ok(ReflectionReport {
        schema: REFLECTION_SCHEMA.to_string(),
        current_fingerprint: current_fingerprint.to_string(),
        total_records: records.len(),
        stale_records,
        malformed_lines,
        outcome_counts,
        dead_ends,
        corrections,
        node_lessons,
        general_lessons,
        stale_warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .subsec_nanos();
        let counter = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "codegraph-memory-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    #[test]
    fn save_and_list_roundtrip_with_staleness() {
        let root = temp_root();
        let first = save_memory(
            &root,
            MemorySaveRequest {
                query: "configs target:DATABASE_URL".to_string(),
                outcome: MemoryOutcome::Useful,
                note: Some("readers live in server config module".to_string()),
                node_ids: vec!["n12".to_string(), "cg-1234".to_string()],
                fingerprint: "fp-old".to_string(),
                recorded_at_unix: 1_000,
            },
        )
        .expect("save first");
        assert_eq!(first.id, "mem-1");
        let second = save_memory(
            &root,
            MemorySaveRequest {
                query: "calls(function:legacy_worker)".to_string(),
                outcome: MemoryOutcome::DeadEnd,
                note: None,
                node_ids: vec![],
                fingerprint: "fp-current".to_string(),
                recorded_at_unix: 2_000,
            },
        )
        .expect("save second");
        assert_eq!(second.id, "mem-2");

        let report = list_memory(&root, "fp-current", None, false).expect("list");
        assert_eq!(report.total, 2);
        assert_eq!(report.stale, 1);
        assert_eq!(report.malformed_lines, 0);
        let stale_entry = report
            .records
            .iter()
            .find(|entry| entry.record.id == "mem-1")
            .expect("first record listed");
        assert!(stale_entry.stale, "old fingerprint should be stale");
        assert!(
            !report
                .records
                .iter()
                .find(|entry| entry.record.id == "mem-2")
                .expect("second record listed")
                .stale
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn list_filters_by_outcome_and_staleness() {
        let root = temp_root();
        save_memory(
            &root,
            MemorySaveRequest {
                query: "q1".to_string(),
                outcome: MemoryOutcome::Useful,
                note: None,
                node_ids: vec![],
                fingerprint: "fp".to_string(),
                recorded_at_unix: 1,
            },
        )
        .unwrap();
        save_memory(
            &root,
            MemorySaveRequest {
                query: "q2".to_string(),
                outcome: MemoryOutcome::Corrected,
                note: Some("actual reader is load_config".to_string()),
                node_ids: vec!["cg-abcd".to_string()],
                fingerprint: "fp-stale".to_string(),
                recorded_at_unix: 2,
            },
        )
        .unwrap();

        let corrected = list_memory(&root, "fp", Some(MemoryOutcome::Corrected), false).unwrap();
        assert_eq!(corrected.total, 1);
        assert_eq!(corrected.records[0].record.query, "q2");

        let only_stale = list_memory(&root, "fp", None, true).unwrap();
        assert_eq!(only_stale.total, 1);
        assert_eq!(only_stale.records[0].record.id, "mem-2");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn reflect_groups_lessons_with_provenance_and_stale_warnings() {
        let root = temp_root();
        save_memory(
            &root,
            MemorySaveRequest {
                query: "configs target:DATABASE_URL".to_string(),
                outcome: MemoryOutcome::Useful,
                note: Some("reader lives in load_config".to_string()),
                node_ids: vec!["cg-abcd".to_string()],
                fingerprint: "fp-current".to_string(),
                recorded_at_unix: 10,
            },
        )
        .unwrap();
        save_memory(
            &root,
            MemorySaveRequest {
                query: "calls(function:legacy_worker)".to_string(),
                outcome: MemoryOutcome::DeadEnd,
                note: Some("legacy_worker is unreachable".to_string()),
                node_ids: vec![],
                fingerprint: "fp-old".to_string(),
                recorded_at_unix: 20,
            },
        )
        .unwrap();
        save_memory(
            &root,
            MemorySaveRequest {
                query: "configs target:API_KEY".to_string(),
                outcome: MemoryOutcome::Corrected,
                note: Some("actual reader is auth module, not server".to_string()),
                node_ids: vec!["cg-abcd".to_string(), "n999".to_string()],
                fingerprint: "fp-current".to_string(),
                recorded_at_unix: 30,
            },
        )
        .unwrap();

        let mut labels = std::collections::BTreeMap::new();
        labels.insert("cg-abcd".to_string(), "load_config".to_string());
        let report = reflect(&root, "fp-current", &labels).expect("reflection report");

        assert_eq!(report.schema, REFLECTION_SCHEMA);
        assert_eq!(report.total_records, 3);
        assert_eq!(report.stale_records, 1);
        assert_eq!(report.outcome_counts.get("useful"), Some(&1));
        assert_eq!(report.outcome_counts.get("dead_end"), Some(&1));
        assert_eq!(report.outcome_counts.get("corrected"), Some(&1));
        assert_eq!(report.dead_ends.len(), 1);
        assert!(report.dead_ends[0].stale);
        assert_eq!(report.corrections.len(), 1);
        assert_eq!(report.stale_warnings.len(), 1);
        assert!(report.stale_warnings[0].contains("mem-2"));
        assert!(report.stale_warnings[0].contains("re-verify"));

        let known = report
            .node_lessons
            .iter()
            .find(|lesson| lesson.node_id == "cg-abcd")
            .expect("node lesson for the durable id");
        assert_eq!(known.label.as_deref(), Some("load_config"));
        assert!(!known.missing);
        assert_eq!(known.records.len(), 2);
        let missing = report
            .node_lessons
            .iter()
            .find(|lesson| lesson.node_id == "n999")
            .expect("node lesson for a node that is gone");
        assert!(missing.missing, "unknown node id should be flagged missing");
        assert_eq!(report.general_lessons.len(), 1);
        assert_eq!(report.general_lessons[0].id, "mem-2");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_record_written_with_numeric_ids_still_reads() {
        let root = temp_root();
        let path = memory_path(&root);
        fs::create_dir_all(path.parent().expect("parent")).expect("memory dir");
        // What older builds wrote: the positional id as a number.
        fs::write(
            &path,
            "{\"id\":\"mem-old\",\"recorded_at_unix\":1000,\"query\":\"old\",\"outcome\":\"useful\",\"node_ids\":[12,40],\"fingerprint\":\"fp-old\"}\n",
        )
        .expect("write record");

        let report = list_memory(&root, "fp-old", None, false).expect("list");
        assert_eq!(report.malformed_lines, 0);
        assert_eq!(report.total, 1);
        assert_eq!(
            report.records[0].record.node_ids,
            vec!["n12".to_string(), "n40".to_string()],
            "a number reads as the positional reference it was"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn malformed_lines_are_counted_not_fatal() {
        let root = temp_root();
        save_memory(
            &root,
            MemorySaveRequest {
                query: "q1".to_string(),
                outcome: MemoryOutcome::Useful,
                note: None,
                node_ids: vec![],
                fingerprint: "fp".to_string(),
                recorded_at_unix: 1,
            },
        )
        .unwrap();
        let path = memory_path(&root);
        let mut raw = fs::read_to_string(&path).unwrap();
        raw.push_str("{not json}\n");
        fs::write(&path, raw).unwrap();

        let report = list_memory(&root, "fp", None, false).unwrap();
        assert_eq!(report.total, 1);
        assert_eq!(report.malformed_lines, 1);
        fs::remove_dir_all(root).ok();
    }
}
