//! Workflow regression fixtures for language runtime paths.
//!
//! Each fixture scans a small single-language project and asserts that the
//! block-style workflow built from the `worker` function keeps its expected
//! block classification and transition shape. These tests guard the workflow
//! output contract used by CLI, API, and web consumers.

use codegraph_analysis::{
    TraceStart, WorkflowBlockKind, WorkflowFilters, WorkflowRequest, workflow,
};
use codegraph_indexer::{IndexOptions, scan_project};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct WorkflowFixture {
    name: &'static str,
    file: &'static str,
    source: &'static str,
    expected_blocks: &'static [(WorkflowBlockKind, &'static str)],
}

fn temp_fixture_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .subsec_nanos();
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!(
        "codegraph-workflow-fixture-{name}-{}-{nanos}-{counter}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("fixture root should be creatable");
    root
}

fn workflow_fixtures() -> Vec<WorkflowFixture> {
    vec![
        WorkflowFixture {
            name: "rust",
            file: "src/main.rs",
            source: r#"async fn worker() {
    if ready() {
        for item in items() {
            item.await;
        }
        return;
    }
    helper();
}

fn helper() {}

fn ready() -> bool {
    true
}

fn items() -> Vec<u8> {
    Vec::new()
}
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: for"),
                (WorkflowBlockKind::Async, "async: await"),
                (WorkflowBlockKind::Return, "return: return"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
        WorkflowFixture {
            name: "python",
            file: "app.py",
            source: r#"async def worker():
    if ready():
        for item in items():
            await item
        return
    helper()


def helper():
    pass
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: for"),
                (WorkflowBlockKind::Async, "async: await"),
                (WorkflowBlockKind::Return, "return: return"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
        WorkflowFixture {
            name: "javascript",
            file: "index.js",
            source: r#"async function worker() {
  if (ready()) {
    for (const item of items()) {
      await item;
    }
    return;
  }
  helper();
}

function helper() {}
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: for"),
                (WorkflowBlockKind::Async, "async: await"),
                (WorkflowBlockKind::Return, "return: return"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
        WorkflowFixture {
            name: "typescript",
            file: "worker.ts",
            source: r#"async function worker(): Promise<void> {
  if (ready()) {
    for (const item of items()) {
      await item;
    }
    return;
  }
  helper();
}

function helper(): void {}
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: for"),
                (WorkflowBlockKind::Async, "async: await"),
                (WorkflowBlockKind::Return, "return: return"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
        WorkflowFixture {
            name: "go",
            file: "main.go",
            source: r#"package main

func worker() {
	if ready() {
		for i := 0; i < 3; i++ {
			go helper()
		}
		return
	}
	helper()
}

func helper() {}

func ready() bool {
	return true
}
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: for"),
                (WorkflowBlockKind::Async, "async: go"),
                (WorkflowBlockKind::Return, "return: return"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
        WorkflowFixture {
            name: "php",
            file: "app.php",
            source: r#"<?php
function worker() {
    if (ready()) {
        foreach (items() as $item) {
            process($item);
        }
        return;
    }
    helper();
}

function helper() {}
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: foreach"),
                (WorkflowBlockKind::Return, "return: return"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
        WorkflowFixture {
            name: "bash",
            file: "script.sh",
            source: r#"#!/usr/bin/env bash

worker() {
  if true; then
    for item in a b; do
      echo "$item"
    done
  fi
  helper
}

helper() {
  echo done
}
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: for"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
        WorkflowFixture {
            name: "dart",
            file: "lib/main.dart",
            source: r#"void worker() async {
  if (ready) {
    for (final item in items) {
      await item;
    }
    return;
  }
  helper();
}

void helper() {}
"#,
            expected_blocks: &[
                (WorkflowBlockKind::Branch, "branch: if"),
                (WorkflowBlockKind::Loop, "loop: for"),
                (WorkflowBlockKind::Async, "async: await"),
                (WorkflowBlockKind::Return, "return: return"),
                (WorkflowBlockKind::Call, "helper"),
            ],
        },
    ]
}

#[test]
fn workflow_language_fixtures_keep_block_and_transition_shape() {
    for fixture in workflow_fixtures() {
        let root = temp_fixture_root(fixture.name);
        let file_path = root.join(fixture.file);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("fixture directory should be creatable");
        }
        fs::write(&file_path, fixture.source).expect("fixture source should be writable");

        let graph = scan_project(&root, &IndexOptions::default())
            .unwrap_or_else(|error| panic!("{} fixture should scan: {error}", fixture.name));
        let report = workflow(
            &graph,
            WorkflowRequest {
                start: TraceStart::Label("worker".to_string()),
                max_depth: 3,
                block_limit: 100,
                filters: WorkflowFilters::default(),
                compact: false,
            },
        )
        .unwrap_or_else(|| panic!("{} fixture should build a workflow report", fixture.name));

        let start_block = report
            .blocks
            .iter()
            .find(|block| block.kind == WorkflowBlockKind::Start)
            .unwrap_or_else(|| panic!("{} fixture should keep a start block", fixture.name));
        assert_eq!(
            start_block.node.label, "worker",
            "{} fixture start block should be the worker function",
            fixture.name
        );

        for (expected_kind, expected_label) in fixture.expected_blocks {
            let block = report
                .blocks
                .iter()
                .find(|block| block.kind == *expected_kind && block.node.label == *expected_label)
                .unwrap_or_else(|| {
                    panic!(
                        "{} fixture should keep block {expected_label} ({expected_kind:?}); got {:#?}",
                        fixture.name,
                        report
                            .blocks
                            .iter()
                            .map(|block| (block.kind.clone(), block.node.label.clone()))
                            .collect::<Vec<_>>()
                    )
                });
            assert!(
                report
                    .transitions
                    .iter()
                    .any(|transition| transition.target == block.id),
                "{} fixture block {expected_label} should be reachable through a transition",
                fixture.name
            );
        }

        assert!(
            report.transitions.iter().all(|transition| report
                .blocks
                .iter()
                .any(|block| block.id == transition.source)
                && report
                    .blocks
                    .iter()
                    .any(|block| block.id == transition.target)),
            "{} fixture transitions should connect returned blocks",
            fixture.name
        );
        assert!(
            !report.truncated,
            "{} fixture workflow should not be truncated at the fixture size",
            fixture.name
        );

        fs::remove_dir_all(root).ok();
    }
}
