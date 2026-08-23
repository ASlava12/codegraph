//! Benchmark harness: token/context savings and graph-query recall.
//!
//! `codegraph bench-context` answers two questions about a real corpus with
//! numbers instead of claims. First, how much reading does the graph save an
//! agent: for overview, symbol, and config investigations it compares the
//! bytes an agent would otherwise read (whole corpus for overviews, every
//! file mentioning a symbol for grep-style lookups) against the bytes of the
//! bounded graph slice that answers the same question. Second, how much the
//! graph actually captured: recall is measured against oracles derived from
//! plain text scanning of the sources — function definition headers and
//! environment-variable reads found independently of the parser pipeline —
//! so extraction gaps show up as recall below 100% instead of staying
//! invisible. Everything is deterministic: sampling is sorted, no clocks,
//! no randomness; token estimates use the common bytes/4 heuristic.

use anyhow::Result;
use codegraph_analysis::{ProjectReportLimits, project_report, query_graph};
use codegraph_core::{CodeGraph, NodeKind};
use codegraph_parser::adapter_for_path;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const BENCHMARK_SCHEMA: &str = "codegraph.benchmark.v1";
const MAX_ORACLE_FILE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Serialize)]
pub struct CorpusStats {
    pub files: usize,
    pub bytes: u64,
    pub est_tokens: u64,
    pub nodes: usize,
    pub edges: usize,
}

#[derive(Debug, Serialize)]
pub struct SavingsTask {
    pub task: String,
    pub samples: usize,
    pub baseline_bytes: u64,
    pub baseline_est_tokens: u64,
    pub graph_bytes: u64,
    pub graph_est_tokens: u64,
    /// Basis points saved versus the baseline; negative when the graph
    /// answer is larger than the raw reading it replaces.
    pub savings_basis_points: i64,
}

#[derive(Debug, Serialize)]
pub struct RecallTask {
    pub task: String,
    pub expected: usize,
    pub found: usize,
    pub recall_basis_points: u64,
    /// Recall over what a parser was actually given: the same count with
    /// the misses outside parsed source taken out of the denominator.
    /// Flask reads 79% raw and 100% here, because all 94 of its misses are
    /// code samples in prose, and the raw figure alone reads like an
    /// extraction gap that is not there.
    #[serde(default)]
    pub parsed_source_recall_basis_points: u64,
    /// Up to ten oracle items the graph missed, for direct follow-up.
    pub sample_misses: Vec<String>,
    /// Misses that live only in files no language adapter parses — code
    /// samples inside documentation, HTML templates. The graph never claims
    /// those, so they measure the corpus difference rather than extraction.
    pub misses_outside_parsed_source: usize,
}

#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub schema: String,
    pub root: String,
    pub corpus: CorpusStats,
    pub savings: Vec<SavingsTask>,
    pub recall: Vec<RecallTask>,
    pub total_baseline_bytes: u64,
    pub total_graph_bytes: u64,
    pub total_savings_basis_points: i64,
    pub mean_recall_basis_points: u64,
}

fn est_tokens(bytes: u64) -> u64 {
    bytes / 4
}

fn savings_bp(baseline: u64, graph: u64) -> i64 {
    if baseline == 0 {
        return 0;
    }
    10_000 - ((graph as i128 * 10_000) / baseline as i128) as i64
}

/// Source files present in the graph, with on-disk sizes and (bounded) text.
struct CorpusFile {
    path: String,
    bytes: u64,
    text: Option<String>,
}

fn load_corpus(graph: &CodeGraph, root: &Path) -> Vec<CorpusFile> {
    let mut files = Vec::new();
    for node in &graph.nodes {
        if node.kind != NodeKind::File {
            continue;
        }
        let disk_path = root.join(&node.label);
        let Ok(metadata) = fs::metadata(&disk_path) else {
            continue;
        };
        let bytes = metadata.len();
        let text = (bytes <= MAX_ORACLE_FILE_BYTES)
            .then(|| fs::read_to_string(&disk_path).ok())
            .flatten();
        files.push(CorpusFile {
            path: node.label.clone(),
            bytes,
            text,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

/// Test-convention paths whose contents are fixtures, not application code
/// the graph is expected to model (audit F12).
fn is_test_fixture_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    normalized.contains("/tests/")
        || normalized.contains("/test/")
        || normalized.contains("/fixtures/")
        || normalized.contains("/__tests__/")
        || file_name.starts_with("test_")
        || file_name.contains("_test.")
        || file_name.contains(".test.")
        || file_name.contains(".spec.")
}

/// Oracle-visible portion of a source file: test-fixture files are skipped
/// entirely, and Rust files are truncated at the inline `#[cfg(test)]`
/// module so fixture strings inside unit tests stop counting as expected
/// oracle facts (audit F12).
fn oracle_text(file: &CorpusFile) -> Option<&str> {
    if is_test_fixture_path(&file.path) {
        return None;
    }
    let text = file.text.as_deref()?;
    if file.path.ends_with(".rs")
        && let Some(index) = text.find("#[cfg(test)]")
    {
        return Some(&text[..index]);
    }
    Some(text)
}

/// A match at `column` sits inside a string literal when an odd number of
/// unescaped quotes precede it on the line (audit F12: fixture code embedded
/// in string literals must not count as expected oracle facts).
fn inside_string_literal(line: &str, column: usize) -> bool {
    let mut quotes = 0usize;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if index >= column {
            break;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' | '\'' => quotes += 1,
            _ => {}
        }
    }
    quotes % 2 == 1
}

/// Oracle 1: function definition names from plain header scanning,
/// independent of the Tree-sitter pipeline.
/// Oracle names, plus the subset that only ever appears in files no language
/// adapter parses.
/// The lines a file's string literals and comments cover, from the same
/// parse the graph is built from. The oracles scan text, so without this
/// a code sample counts against extraction: codegraph's own test fixtures
/// hold `env::var("API_KEY")` inside a Rust raw string, and the benchmark
/// reported half its environment reads missing because of it.
fn quoted_lines(file: &CorpusFile, text: &str) -> Vec<(u32, u32)> {
    let path = Path::new(&file.path);
    let Some(adapter) = adapter_for_path(path) else {
        return Vec::new();
    };
    adapter
        .parse(path, text.as_bytes())
        .map(|parsed| parsed.quoted_line_ranges)
        .unwrap_or_default()
}

fn line_is_quoted(ranges: &[(u32, u32)], line: u32) -> bool {
    ranges
        .iter()
        .any(|(start, end)| line >= *start && line <= *end)
}

fn oracle_function_names_by_origin(files: &[CorpusFile]) -> (BTreeSet<String>, BTreeSet<String>) {
    const HEADS: &[&str] = &["fn ", "def ", "func ", "function "];
    let mut names = BTreeSet::new();
    let mut parsed_source_names = BTreeSet::new();
    for file in files {
        let Some(text) = oracle_text(file) else {
            continue;
        };
        // A `def` inside a docstring is a code sample, not a definition: the
        // single-line guard below already refuses matches inside a string
        // literal, and a triple-quoted block is the same thing spread over
        // lines. Flask's `ctx.py` and `sansio/app.py` document themselves with
        // `.. code-block:: python`, and every one of those samples counted
        // against extraction.
        let quoted = quoted_lines(file, text);
        let mut inside_block_string = false;
        for (index, line) in text.lines().enumerate() {
            let fences = line.matches("\"\"\"").count() + line.matches("'''").count();
            let opened_here = inside_block_string;
            if fences % 2 == 1 {
                inside_block_string = !inside_block_string;
            }
            // The parse answers for a language the project has an adapter
            // for; the fence counting stays for the rest.
            if opened_here || line_is_quoted(&quoted, index as u32 + 1) {
                continue;
            }
            let trimmed = line.trim_start();
            let stripped = trimmed
                .strip_prefix("pub ")
                .or_else(|| trimmed.strip_prefix("async "))
                .unwrap_or(trimmed);
            for head in HEADS {
                let Some(rest) = stripped.strip_prefix(head) else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if name.len() >= 2
                    && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && rest[name.len()..].trim_start().starts_with('(')
                {
                    if adapter_for_path(Path::new(&file.path)).is_some() {
                        parsed_source_names.insert(name.clone());
                    }
                    names.insert(name);
                }
            }
        }
    }
    let documentation_only = names.difference(&parsed_source_names).cloned().collect();
    (names, documentation_only)
}

/// Oracle 2: environment keys from common read patterns in source text.
/// Environment keys the text scan finds, and the subset that appears only
/// in files no adapter parses. Vue core reads `process.env.COMMENT` inside
/// a GitHub workflow's inline script: a real read for the workflow, but
/// not one any language adapter was ever given the chance to see.
fn oracle_env_keys_by_origin(files: &[CorpusFile]) -> (BTreeSet<String>, BTreeSet<String>) {
    const PATTERNS: &[&str] = &[
        "env::var(\"",
        "getenv(\"",
        "os.environ.get(\"",
        "os.environ[\"",
        "process.env.",
    ];
    let mut keys = BTreeSet::new();
    let mut parsed_source_keys = BTreeSet::new();
    for file in files {
        let Some(text) = oracle_text(file) else {
            continue;
        };
        let parsed_language = adapter_for_path(Path::new(&file.path)).is_some();
        let quoted = quoted_lines(file, text);
        for pattern in PATTERNS {
            let mut cursor = 0;
            while let Some(found) = text[cursor..].find(pattern) {
                let match_start = cursor + found;
                let start = match_start + pattern.len();
                let line_start = text[..match_start]
                    .rfind('\n')
                    .map(|index| index + 1)
                    .unwrap_or(0);
                let line_end = text[line_start..]
                    .find('\n')
                    .map(|index| line_start + index)
                    .unwrap_or(text.len());
                let line_number = text[..line_start].lines().count() as u32 + 1;
                if line_is_quoted(&quoted, line_number)
                    || inside_string_literal(&text[line_start..line_end], match_start - line_start)
                {
                    cursor = start;
                    continue;
                }
                let key: String = text[start..]
                    .chars()
                    .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                    .collect();
                if key.len() >= 3 {
                    if parsed_language {
                        parsed_source_keys.insert(key.clone());
                    }
                    keys.insert(key);
                }
                cursor = start;
            }
        }
    }
    let outside_parsed_source = keys.difference(&parsed_source_keys).cloned().collect();
    (keys, outside_parsed_source)
}

/// Recall over the items a parser could see, given the raw counts.
fn parsed_source_recall(expected: usize, found: usize, outside: usize) -> u64 {
    let reachable = expected.saturating_sub(outside);
    if reachable == 0 {
        return 10_000;
    }
    ((found.min(reachable) as u64) * 10_000) / reachable as u64
}

fn json_bytes<T: Serialize>(value: &T) -> u64 {
    serde_json::to_string(value)
        .map(|s| s.len() as u64)
        .unwrap_or(0)
}

/// Deterministic sample: every k-th element of the sorted set, capped.
fn sample<T: Clone + Ord>(set: &BTreeSet<T>, limit: usize) -> Vec<T> {
    if set.is_empty() || limit == 0 {
        return Vec::new();
    }
    let step = (set.len() / limit).max(1);
    set.iter().step_by(step).take(limit).cloned().collect()
}

pub fn run_benchmark(graph: &CodeGraph, root: &Path, samples: usize) -> Result<BenchmarkReport> {
    let files = load_corpus(graph, root);
    let corpus_bytes: u64 = files.iter().map(|file| file.bytes).sum();
    let corpus = CorpusStats {
        files: files.len(),
        bytes: corpus_bytes,
        est_tokens: est_tokens(corpus_bytes),
        nodes: graph.nodes.len(),
        edges: graph.edges.len(),
    };

    let function_labels: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Function)
        .map(|node| node.label.as_str())
        .collect();
    let config_labels: BTreeSet<&str> = graph
        .nodes
        .iter()
        .filter(|node| matches!(node.kind, NodeKind::Config | NodeKind::Environment))
        .map(|node| node.label.as_str())
        .collect();

    // Recall against text-scan oracles.
    let mut recall = Vec::new();
    let (oracle_functions, documentation_only_functions) = oracle_function_names_by_origin(&files);
    let missing_functions: Vec<String> = oracle_functions
        .iter()
        .filter(|name| !function_labels.contains(name.as_str()))
        .cloned()
        .collect();
    // A `def` inside an .rst tutorial or an .html template is a text-scan hit
    // the graph never claimed; counting it silently made extraction look worse
    // than it is (101 of Flask's 457 oracle names live only in prose).
    let misses_outside_parsed_source = missing_functions
        .iter()
        .filter(|name| documentation_only_functions.contains(name.as_str()))
        .count();
    let found_functions = oracle_functions.len() - missing_functions.len();
    recall.push(RecallTask {
        misses_outside_parsed_source,
        parsed_source_recall_basis_points: parsed_source_recall(
            oracle_functions.len(),
            found_functions,
            misses_outside_parsed_source,
        ),
        task: "function_definitions".to_string(),
        expected: oracle_functions.len(),
        found: found_functions,
        recall_basis_points: if oracle_functions.is_empty() {
            10_000
        } else {
            ((oracle_functions.len() - missing_functions.len()) as u64 * 10_000)
                / oracle_functions.len() as u64
        },
        sample_misses: missing_functions.into_iter().take(10).collect(),
    });

    let (oracle_keys, keys_outside_parsed_source) = oracle_env_keys_by_origin(&files);
    let missing_keys: Vec<String> = oracle_keys
        .iter()
        .filter(|key| {
            !config_labels.contains(key.as_str())
                && !config_labels
                    .iter()
                    .any(|label| label.contains(key.as_str()))
        })
        .cloned()
        .collect();
    // A key read by a workflow's inline script sits in a file no adapter
    // parses, so the graph never had the chance to claim it.
    let keys_outside = missing_keys
        .iter()
        .filter(|key| keys_outside_parsed_source.contains(key.as_str()))
        .count();
    let found_keys = oracle_keys.len() - missing_keys.len();
    recall.push(RecallTask {
        misses_outside_parsed_source: keys_outside,
        parsed_source_recall_basis_points: parsed_source_recall(
            oracle_keys.len(),
            found_keys,
            keys_outside,
        ),
        task: "environment_reads".to_string(),
        expected: oracle_keys.len(),
        found: found_keys,
        recall_basis_points: if oracle_keys.is_empty() {
            10_000
        } else {
            ((oracle_keys.len() - missing_keys.len()) as u64 * 10_000) / oracle_keys.len() as u64
        },
        sample_misses: missing_keys.into_iter().take(10).collect(),
    });

    // Token savings tasks.
    let mut savings = Vec::new();

    // 1. Project overview: report snapshot vs reading the whole corpus.
    let report = project_report(graph, ProjectReportLimits::default());
    savings.push(SavingsTask {
        task: "project_overview".to_string(),
        samples: 1,
        baseline_bytes: corpus_bytes,
        baseline_est_tokens: est_tokens(corpus_bytes),
        graph_bytes: json_bytes(&report),
        graph_est_tokens: est_tokens(json_bytes(&report)),
        savings_basis_points: savings_bp(corpus_bytes, json_bytes(&report)),
    });

    // 2. Symbol lookups: bounded symbol slice vs every file mentioning the
    // name (what a grep-following agent would read).
    let mut file_sizes: BTreeMap<&str, u64> = BTreeMap::new();
    for file in &files {
        file_sizes.insert(file.path.as_str(), file.bytes);
    }
    let symbol_names: BTreeSet<String> = oracle_functions
        .iter()
        .filter(|name| function_labels.contains(name.as_str()))
        .cloned()
        .collect();
    let symbol_sample = sample(&symbol_names, samples);
    let mut symbol_baseline = 0u64;
    let mut symbol_graph = 0u64;
    for name in &symbol_sample {
        for file in &files {
            if file
                .text
                .as_ref()
                .is_some_and(|text| text.contains(name.as_str()))
            {
                symbol_baseline += file.bytes;
            }
        }
        if let Ok(result) = query_graph(graph, &format!("symbols label:{name}")) {
            symbol_graph += json_bytes(&result);
        }
    }
    savings.push(SavingsTask {
        task: "symbol_slices".to_string(),
        samples: symbol_sample.len(),
        baseline_bytes: symbol_baseline,
        baseline_est_tokens: est_tokens(symbol_baseline),
        graph_bytes: symbol_graph,
        graph_est_tokens: est_tokens(symbol_graph),
        savings_basis_points: savings_bp(symbol_baseline, symbol_graph),
    });

    // 3. Config lookups: bounded config slice vs every file mentioning the key.
    let config_keys: BTreeSet<String> = oracle_keys
        .iter()
        .filter(|key| {
            config_labels.contains(key.as_str())
                || config_labels
                    .iter()
                    .any(|label| label.contains(key.as_str()))
        })
        .cloned()
        .collect();
    let config_sample = sample(&config_keys, samples);
    let mut config_baseline = 0u64;
    let mut config_graph = 0u64;
    for key in &config_sample {
        for file in &files {
            if file
                .text
                .as_ref()
                .is_some_and(|text| text.contains(key.as_str()))
            {
                config_baseline += file.bytes;
            }
        }
        if let Ok(result) = query_graph(graph, &format!("configs target:{key}")) {
            config_graph += json_bytes(&result);
        }
    }
    savings.push(SavingsTask {
        task: "config_slices".to_string(),
        samples: config_sample.len(),
        baseline_bytes: config_baseline,
        baseline_est_tokens: est_tokens(config_baseline),
        graph_bytes: config_graph,
        graph_est_tokens: est_tokens(config_graph),
        savings_basis_points: savings_bp(config_baseline, config_graph),
    });

    let total_baseline: u64 = savings.iter().map(|task| task.baseline_bytes).sum();
    let total_graph: u64 = savings.iter().map(|task| task.graph_bytes).sum();
    let mean_recall = if recall.is_empty() {
        10_000
    } else {
        recall
            .iter()
            .map(|task| task.recall_basis_points)
            .sum::<u64>()
            / recall.len() as u64
    };

    Ok(BenchmarkReport {
        schema: BENCHMARK_SCHEMA.to_string(),
        root: root.display().to_string(),
        corpus,
        savings,
        recall,
        total_baseline_bytes: total_baseline,
        total_graph_bytes: total_graph,
        total_savings_basis_points: savings_bp(total_baseline, total_graph),
        mean_recall_basis_points: mean_recall,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegraph_indexer::{IndexOptions, scan_project};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .subsec_nanos();
        let counter = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "codegraph-bench-{}-{nanos}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn a_sample_in_prose_is_not_a_missing_definition() {
        let root = temp_dir();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::write(
            root.join("app.py"),
            "def real_function():\n    \"\"\"Usage:\n\n    def docstring_sample():\n        pass\n    \"\"\"\n    return 1\n",
        )
        .unwrap();
        // A tutorial is not source: the scanner never claims what it shows.
        fs::write(
            root.join("docs").join("guide.rst"),
            "Guide\n=====\n\n.. code-block:: python\n\n    def tutorial_sample():\n        pass\n",
        )
        .unwrap();

        let graph = scan_project(&root, &IndexOptions::default()).expect("scan");
        let report = run_benchmark(&graph, &root, 10).expect("benchmark");
        let functions = report
            .recall
            .iter()
            .find(|task| task.task == "function_definitions")
            .expect("function recall task");

        // The docstring sample is not counted at all; the tutorial one is
        // counted but attributed to prose rather than to extraction.
        assert!(
            !functions
                .sample_misses
                .iter()
                .any(|name| name == "docstring_sample"),
            "a definition inside a string is not a definition: {:?}",
            functions.sample_misses
        );
        assert!(
            functions
                .sample_misses
                .iter()
                .any(|name| name == "tutorial_sample"),
            "{:?}",
            functions.sample_misses
        );
        assert_eq!(functions.misses_outside_parsed_source, 1);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn recall_separates_what_a_parser_could_see() {
        // A `def` inside an .rst tutorial is a name the graph never claimed
        // and never could: flask reads 79% raw against 100% of what was
        // offered to a parser, and only the second number says anything
        // about extraction.
        assert_eq!(parsed_source_recall(450, 356, 94), 10_000);
        assert_eq!(parsed_source_recall(1093, 1089, 3), 9_990);
        // Nothing outside parsed source leaves both figures equal.
        assert_eq!(parsed_source_recall(10, 8, 0), 8_000);
        // A corpus the parser saw nothing of cannot be judged.
        assert_eq!(parsed_source_recall(4, 0, 4), 10_000);
    }

    #[test]
    fn a_fixture_inside_a_string_is_not_missing_extraction() {
        // codegraph's own tests hold Rust source in raw strings. The
        // oracles scan text, so `env::var("API_KEY")` written inside one
        // counted as an environment read the graph had failed to find, and
        // the benchmark reported half of them missing.
        let root = temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            "fn main() {\n    let _real = std::env::var(\"REAL_KEY\").unwrap();\n}\n\nfn fixture() -> &\'static str {\n    r#\"fn sample() {\n    let _ = std::env::var(\"FIXTURE_KEY\");\n}\n\"#\n}\n",
        )
        .unwrap();

        let files = load_corpus(
            &scan_project(&root, &IndexOptions::default()).unwrap(),
            &root,
        );
        let keys = oracle_env_keys_by_origin(&files).0;
        assert!(keys.contains("REAL_KEY"), "{keys:?}");
        assert!(
            !keys.contains("FIXTURE_KEY"),
            "a key inside a string literal is a sample, not a read: {keys:?}"
        );

        let (names, _) = oracle_function_names_by_origin(&files);
        assert!(names.contains("fixture"), "{names:?}");
        assert!(
            !names.contains("sample"),
            "a definition inside a string literal is a sample too: {names:?}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn benchmark_measures_recall_and_savings_on_mixed_corpus() {
        let root = temp_dir();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src").join("main.rs"),
            format!(
                "fn main() {{\n    let _db = std::env::var(\"DATABASE_URL\").unwrap();\n    process_records();\n}}\n\npub fn process_records() {{\n    // {}\n}}\n",
                "padding ".repeat(4000)
            ),
        )
        .unwrap();
        fs::write(
            root.join("worker.py"),
            format!(
                "import os\n\ndef sync_inventory():\n    token = os.environ.get(\"API_TOKEN\")\n    return token\n\n# {}\n",
                "filler ".repeat(4000)
            ),
        )
        .unwrap();
        let graph = scan_project(&root, &IndexOptions::default()).expect("scan");

        let report = run_benchmark(&graph, &root, 10).expect("benchmark");
        assert_eq!(report.schema, BENCHMARK_SCHEMA);
        assert!(report.corpus.files >= 2);
        assert!(report.corpus.bytes > 0);

        let functions = report
            .recall
            .iter()
            .find(|task| task.task == "function_definitions")
            .expect("function recall task");
        assert!(functions.expected >= 3, "oracle must see planted functions");
        assert_eq!(
            functions.recall_basis_points, 10_000,
            "planted simple definitions must all be extracted: misses {:?}",
            functions.sample_misses
        );

        let env = report
            .recall
            .iter()
            .find(|task| task.task == "environment_reads")
            .expect("env recall task");
        assert_eq!(env.expected, 2, "DATABASE_URL and API_TOKEN");
        assert_eq!(
            env.recall_basis_points, 10_000,
            "planted env reads must be extracted: misses {:?}",
            env.sample_misses
        );

        let overview = report
            .savings
            .iter()
            .find(|task| task.task == "project_overview")
            .expect("overview savings");
        assert_eq!(overview.baseline_bytes, report.corpus.bytes);
        assert!(overview.graph_bytes > 0);

        let symbols = report
            .savings
            .iter()
            .find(|task| task.task == "symbol_slices")
            .expect("symbol savings");
        assert!(symbols.samples >= 3);
        assert!(
            symbols.savings_basis_points > 0,
            "padded files must make slices cheaper than grep reading: {symbols:?}"
        );

        let configs = report
            .savings
            .iter()
            .find(|task| task.task == "config_slices")
            .expect("config savings");
        assert!(configs.samples >= 1);
        assert!(configs.savings_basis_points > 0);

        assert!(report.total_savings_basis_points > 0);
        assert_eq!(report.mean_recall_basis_points, 10_000);

        // Determinism: identical rerun produces identical JSON.
        let second = run_benchmark(&graph, &root, 10).expect("second run");
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn oracles_scan_headers_and_env_patterns() {
        let files = vec![CorpusFile {
            path: "a.rs".to_string(),
            bytes: 10,
            text: Some(
                "pub fn alpha() {}\nasync fn beta(x: u8) {}\nfn 9bad() {}\nlet x = env::var(\"MY_KEY\");\nprocess.env.NODE_ENV\n"
                    .to_string(),
            ),
        }];
        let (names, _) = oracle_function_names_by_origin(&files);
        assert!(names.contains("alpha"));
        assert!(names.contains("beta"));
        assert!(!names.contains("9bad"), "digit-leading names rejected");
        let keys = oracle_env_keys_by_origin(&files).0;
        assert!(keys.contains("MY_KEY"));
        assert!(keys.contains("NODE_ENV"));
    }

    #[test]
    fn oracles_skip_string_literals_and_test_fixtures() {
        let files = vec![
            CorpusFile {
                path: "src/lib.rs".to_string(),
                bytes: 10,
                text: Some(
                    concat!(
                        "fn real_reader() {\n",
                        "    let _ = env::var(\"REAL_KEY\");\n",
                        "    let fixture = \"let _ = env::var(\\\"FAKE_KEY\\\");\";\n",
                        "}\n",
                        "#[cfg(test)]\n",
                        "mod tests {\n",
                        "    fn test_only_helper() {}\n",
                        "    const T: &str = \"env::var(\";\n",
                        "    // env::var(\"TEST_ONLY_KEY\") lives below cfg(test)\n",
                        "}\n",
                    )
                    .to_string(),
                ),
            },
            CorpusFile {
                path: "tests/integration_test.rs".to_string(),
                bytes: 10,
                text: Some("fn fixture_fn() {}\nlet _ = env::var(\"FIXTURE_KEY\");\n".to_string()),
            },
        ];

        let keys = oracle_env_keys_by_origin(&files).0;
        assert!(keys.contains("REAL_KEY"), "real reads still count");
        assert!(
            !keys.contains("FAKE_KEY"),
            "string-literal fixtures are excluded: {keys:?}"
        );
        assert!(
            !keys.contains("TEST_ONLY_KEY"),
            "cfg(test) regions are excluded: {keys:?}"
        );
        assert!(
            !keys.contains("FIXTURE_KEY"),
            "test-path files are excluded: {keys:?}"
        );

        let (names, _) = oracle_function_names_by_origin(&files);
        assert!(names.contains("real_reader"));
        assert!(
            !names.contains("test_only_helper"),
            "cfg(test) functions are excluded: {names:?}"
        );
        assert!(
            !names.contains("fixture_fn"),
            "test-path functions are excluded: {names:?}"
        );
    }
}
