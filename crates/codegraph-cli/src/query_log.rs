//! Local query audit logging with privacy controls.
//!
//! Query-like commands (CLI `query`/`ask`/`journey` and MCP tool calls) can be
//! appended to repository-owned `.codegraph/query-log.jsonl`, one JSON record
//! per line, for a local audit trail of how the graph was interrogated.
//! Privacy defaults are conservative: logging is disabled until opted in via
//! `[query_log]` in `.codegraph/config.toml` (or the `--log-queries` flag),
//! responses are never logged unless `log_responses = true`, and sensitive
//! `key=value`/`key:value` assignments plus configured redaction terms are
//! masked before anything reaches disk. Nothing ever leaves the repository.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const QUERY_LOG_FILE: &str = "query-log.jsonl";
pub const QUERY_LOG_SCHEMA: &str = "codegraph.query_log.v1";
pub const RESPONSE_PREVIEW_LIMIT: usize = 2_000;

const REDACTED: &str = "[redacted]";
const SENSITIVE_MARKERS: &[&str] = &[
    "token",
    "secret",
    "password",
    "passwd",
    "api_key",
    "apikey",
    "authorization",
    "credential",
    "private_key",
    "bearer",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryLogSettings {
    /// Logging is off until the repository opts in.
    pub enabled: bool,
    /// Response previews are a separate opt-in on top of query logging.
    pub log_responses: bool,
    /// Extra literal terms to mask in logged queries and responses.
    pub redact: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryLogRecord {
    pub recorded_at_unix: u64,
    pub surface: String,
    pub action: String,
    pub query: String,
    pub outcome: String,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_preview: Option<String>,
    #[serde(default)]
    pub response_truncated: bool,
}

#[derive(Debug)]
pub struct QueryLogEvent<'a> {
    pub surface: &'a str,
    pub action: &'a str,
    pub query: &'a str,
    pub outcome: &'a str,
    pub duration_ms: u64,
    pub response: Option<&'a str>,
    pub recorded_at_unix: u64,
}

#[derive(Debug, Serialize)]
pub struct QueryLogReport {
    pub schema: String,
    pub log_path: String,
    pub enabled: bool,
    pub log_responses: bool,
    pub total: usize,
    pub returned: usize,
    pub malformed_lines: usize,
    pub action_counts: BTreeMap<String, usize>,
    pub records: Vec<QueryLogRecord>,
}

pub fn query_log_path(root: &Path) -> PathBuf {
    root.join(".codegraph").join(QUERY_LOG_FILE)
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

/// Read `[query_log]` from `.codegraph/config.toml`; absent file or section
/// means the conservative defaults (logging disabled).
pub fn load_settings(root: &Path) -> Result<QueryLogSettings> {
    let mut settings = QueryLogSettings::default();
    let path = root.join(".codegraph").join("config.toml");
    if !path.is_file() {
        return Ok(settings);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let table = raw
        .parse::<toml::Table>()
        .with_context(|| format!("{} is not valid TOML", path.display()))?;
    let Some(section) = table.get("query_log") else {
        return Ok(settings);
    };
    let Some(section) = section.as_table() else {
        bail!("{}: [query_log] must be a table", path.display());
    };
    if let Some(value) = section.get("enabled") {
        settings.enabled = value
            .as_bool()
            .with_context(|| format!("{}: query_log.enabled must be a boolean", path.display()))?;
    }
    if let Some(value) = section.get("log_responses") {
        settings.log_responses = value.as_bool().with_context(|| {
            format!(
                "{}: query_log.log_responses must be a boolean",
                path.display()
            )
        })?;
    }
    if let Some(value) = section.get("redact") {
        let Some(items) = value.as_array() else {
            bail!(
                "{}: query_log.redact must be an array of strings",
                path.display()
            );
        };
        let mut redact = Vec::new();
        for item in items {
            let Some(term) = item.as_str() else {
                bail!(
                    "{}: query_log.redact must be an array of strings",
                    path.display()
                );
            };
            if !term.is_empty() {
                redact.push(term.to_string());
            }
        }
        settings.redact = redact;
    }
    Ok(settings)
}

/// Mask values assigned to sensitive-looking keys (`token=...`, `"password":
/// "..."`) and any configured literal redaction terms.
pub fn redact_text(text: &str, extra_terms: &[String]) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '=' || c == ':' {
            let sensitive = trailing_key_is_sensitive(&out);
            out.push(c);
            i += 1;
            if sensitive {
                while i < chars.len() && chars[i] == ' ' {
                    out.push(' ');
                    i += 1;
                }
                if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                    let quote = chars[i];
                    out.push(quote);
                    i += 1;
                    while i < chars.len() && chars[i] != quote {
                        i += 1;
                    }
                    out.push_str(REDACTED);
                    if i < chars.len() {
                        out.push(quote);
                        i += 1;
                    }
                } else if i < chars.len() && !chars[i].is_whitespace() {
                    while i < chars.len() && !chars[i].is_whitespace() && chars[i] != ',' {
                        i += 1;
                    }
                    out.push_str(REDACTED);
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    for term in extra_terms {
        out = out.replace(term.as_str(), REDACTED);
    }
    out
}

/// Look back over the emitted text for the identifier before `=`/`:`,
/// skipping one closing quote so JSON keys like `"token"` are recognized.
fn trailing_key_is_sensitive(emitted: &str) -> bool {
    let mut rev = emitted.chars().rev().peekable();
    if matches!(rev.peek(), Some('"') | Some('\'')) {
        rev.next();
    }
    let mut key: Vec<char> = Vec::new();
    for c in rev {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            key.push(c);
        } else {
            break;
        }
    }
    if key.is_empty() {
        return false;
    }
    let key: String = key.into_iter().rev().collect::<String>().to_lowercase();
    SENSITIVE_MARKERS.iter().any(|marker| key.contains(marker))
}

/// Append one audit record when logging is enabled; returns the stored record.
pub fn log_query(
    root: &Path,
    settings: &QueryLogSettings,
    event: QueryLogEvent<'_>,
) -> Result<Option<QueryLogRecord>> {
    if !settings.enabled {
        return Ok(None);
    }
    let (response_preview, response_truncated) = match (settings.log_responses, event.response) {
        (true, Some(response)) => {
            let redacted = redact_text(response, &settings.redact);
            let truncated = redacted.chars().count() > RESPONSE_PREVIEW_LIMIT;
            let preview: String = redacted.chars().take(RESPONSE_PREVIEW_LIMIT).collect();
            (Some(preview), truncated)
        }
        _ => (None, false),
    };
    let record = QueryLogRecord {
        recorded_at_unix: event.recorded_at_unix,
        surface: event.surface.to_string(),
        action: event.action.to_string(),
        query: redact_text(event.query, &settings.redact),
        outcome: event.outcome.to_string(),
        duration_ms: event.duration_ms,
        response_preview,
        response_truncated,
    };

    let path = query_log_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let line = serde_json::to_string(&record)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(Some(record))
}

pub fn load_records(root: &Path) -> Result<(Vec<QueryLogRecord>, usize)> {
    let path = query_log_path(root);
    if !path.exists() {
        return Ok((Vec::new(), 0));
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut records = Vec::new();
    let mut malformed = 0usize;
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<QueryLogRecord>(line) {
            Ok(record) => records.push(record),
            Err(_) => malformed += 1,
        }
    }
    Ok((records, malformed))
}

pub fn list_query_log(root: &Path, action: Option<&str>, limit: usize) -> Result<QueryLogReport> {
    let settings = load_settings(root)?;
    let (records, malformed_lines) = load_records(root)?;
    let filtered: Vec<QueryLogRecord> = records
        .into_iter()
        .filter(|record| action.is_none_or(|expected| record.action == expected))
        .collect();
    let mut action_counts = BTreeMap::new();
    for record in &filtered {
        *action_counts.entry(record.action.clone()).or_insert(0) += 1;
    }
    let total = filtered.len();
    let records: Vec<QueryLogRecord> = filtered
        .into_iter()
        .skip(total.saturating_sub(limit))
        .collect();
    Ok(QueryLogReport {
        schema: QUERY_LOG_SCHEMA.to_string(),
        log_path: query_log_path(root).display().to_string(),
        enabled: settings.enabled,
        log_responses: settings.log_responses,
        total,
        returned: records.len(),
        malformed_lines,
        action_counts,
        records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .subsec_nanos();
        let counter = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "codegraph-query-log-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temp root");
        root
    }

    fn write_config(root: &Path, body: &str) {
        let dir = root.join(".codegraph");
        fs::create_dir_all(&dir).expect("config dir");
        fs::write(dir.join("config.toml"), body).expect("config write");
    }

    fn event<'a>(query: &'a str, response: Option<&'a str>) -> QueryLogEvent<'a> {
        QueryLogEvent {
            surface: "cli",
            action: "query",
            query,
            outcome: "ok",
            duration_ms: 5,
            response,
            recorded_at_unix: 1_000,
        }
    }

    #[test]
    fn settings_default_to_disabled_and_parse_from_config() {
        let root = temp_root();
        let defaults = load_settings(&root).expect("defaults");
        assert!(!defaults.enabled);
        assert!(!defaults.log_responses);
        assert!(defaults.redact.is_empty());

        write_config(
            &root,
            "[scan]\nmax_file_size = 1000000\n\n[query_log]\nenabled = true\nlog_responses = true\nredact = [\"acme-internal\"]\n",
        );
        let settings = load_settings(&root).expect("configured");
        assert!(settings.enabled);
        assert!(settings.log_responses);
        assert_eq!(settings.redact, vec!["acme-internal".to_string()]);

        write_config(&root, "[query_log]\nenabled = \"yes\"\n");
        let error = load_settings(&root).expect_err("invalid type");
        assert!(error.to_string().contains("query_log.enabled"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn redaction_masks_sensitive_assignments_and_extra_terms() {
        assert_eq!(
            redact_text("configs target:DATABASE_URL", &[]),
            "configs target:DATABASE_URL",
            "non-sensitive keys stay intact"
        );
        assert_eq!(
            redact_text("API_TOKEN=abc123 kind:function", &[]),
            "API_TOKEN=[redacted] kind:function"
        );
        assert_eq!(
            redact_text(r#"{"query":"nodes","password":"hunter2"}"#, &[]),
            r#"{"query":"nodes","password":"[redacted]"}"#
        );
        assert_eq!(
            redact_text(
                "where is acme-internal used",
                &["acme-internal".to_string()]
            ),
            "where is [redacted] used"
        );
        assert_eq!(
            redact_text("label:main", &[]),
            "label:main",
            "plain query syntax survives"
        );
    }

    #[test]
    fn disabled_settings_write_nothing() {
        let root = temp_root();
        let settings = QueryLogSettings::default();
        let stored = log_query(&root, &settings, event("nodes kind:function", None))
            .expect("log call succeeds");
        assert!(stored.is_none());
        assert!(!query_log_path(&root).exists());
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn log_and_list_roundtrip_without_responses_by_default() {
        let root = temp_root();
        let settings = QueryLogSettings {
            enabled: true,
            ..QueryLogSettings::default()
        };
        log_query(
            &root,
            &settings,
            event("nodes kind:function", Some("{\"nodes\":[]}")),
        )
        .expect("first record")
        .expect("stored");
        let mut second = event("path from:main to:helper", None);
        second.action = "journey";
        second.outcome = "error";
        log_query(&root, &settings, second)
            .expect("second record")
            .expect("stored");

        let report = list_query_log(&root, None, 50).expect("list");
        assert_eq!(report.schema, QUERY_LOG_SCHEMA);
        assert_eq!(report.total, 2);
        assert_eq!(report.returned, 2);
        assert_eq!(report.malformed_lines, 0);
        assert_eq!(report.action_counts.get("query"), Some(&1));
        assert_eq!(report.action_counts.get("journey"), Some(&1));
        assert!(
            report.records[0].response_preview.is_none(),
            "responses stay out of the log unless opted in"
        );
        assert_eq!(report.records[1].outcome, "error");

        let filtered = list_query_log(&root, Some("journey"), 50).expect("filtered");
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.records[0].action, "journey");

        let limited = list_query_log(&root, None, 1).expect("limited");
        assert_eq!(limited.total, 2);
        assert_eq!(limited.returned, 1);
        assert_eq!(limited.records[0].action, "journey", "keeps most recent");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn response_previews_are_opt_in_redacted_and_truncated() {
        let root = temp_root();
        let settings = QueryLogSettings {
            enabled: true,
            log_responses: true,
            redact: Vec::new(),
        };
        let long_response = format!(
            "{{\"api_key\":\"12345\",\"body\":\"{}\"}}",
            "x".repeat(RESPONSE_PREVIEW_LIMIT * 2)
        );
        let record = log_query(&root, &settings, event("nodes", Some(&long_response)))
            .expect("log")
            .expect("stored");
        let preview = record.response_preview.expect("preview stored");
        assert!(record.response_truncated);
        assert_eq!(preview.chars().count(), RESPONSE_PREVIEW_LIMIT);
        assert!(preview.contains("\"api_key\":\"[redacted]\""));
        assert!(!preview.contains("12345"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn malformed_lines_are_counted_not_fatal() {
        let root = temp_root();
        let settings = QueryLogSettings {
            enabled: true,
            ..QueryLogSettings::default()
        };
        log_query(&root, &settings, event("nodes", None))
            .expect("log")
            .expect("stored");
        let path = query_log_path(&root);
        let mut raw = fs::read_to_string(&path).unwrap();
        raw.push_str("{not json}\n");
        fs::write(&path, raw).unwrap();

        let report = list_query_log(&root, None, 50).expect("list");
        assert_eq!(report.total, 1);
        assert_eq!(report.malformed_lines, 1);
        fs::remove_dir_all(root).ok();
    }
}
