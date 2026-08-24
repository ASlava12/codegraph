# CodeGraph Project Report

- Root: `.`
- Generated at unix: `1787574682`
- Graph schema version: `1`
- Quality gate: **passed** (`fail_on=error`, failing_insights=0)
- Risk: **low** (score 30, total 11404, errors 0, warnings 3, infos 11401)

## Summary
| Metric | Count |
| --- | ---: |
| Nodes | 20930 |
| Edges | 46614 |
| Entrypoints | 83 |
| Skipped files | 0 |

### Languages
| Language | Count |
| --- | ---: |
| `rust` | 16605 |
| `javascript` | 4038 |
| `markdown` | 147 |
| `sql` | 15 |

### Node Kinds
| Kind | Count |
| --- | ---: |
| `control_flow` | 11245 |
| `external_dependency` | 5626 |
| `function` | 3097 |
| `type` | 467 |
| `module` | 219 |
| `file` | 127 |
| `entrypoint` | 79 |
| `config` | 39 |
| `directory` | 26 |
| `environment` | 4 |
| `repository` | 1 |

### Edge Confidence
| Confidence | Count |
| --- | ---: |
| `heuristic` | 38144 |
| `syntactic` | 7956 |
| `exact` | 514 |

## Compact Node Summaries
| Score | Node | Kind | Roles | In | Out | Risks | Edge kinds | Source |
| ---: | --- | --- | --- | ---: | ---: | --- | --- | --- |
| 4704 | `cg-545f575ded3c6cf1` `main` | `function` | entrypoint, risk, error_flow, hub | 4 | 333 | `info`=100, `warning`=1 | `calls`=130, `references`=110, `may_error`=95, `contains`=1, `entrypoint`=1 | `crates/codegraph-cli/src/main.rs:100-953` |
| 2675 | `cg-3a37cd9e02a71cf3` `project_report_markdown` | `function` | risk, error_flow, hub | 7 | 147 | `info`=79 | `may_error`=76, `calls`=43, `references`=34, `contains`=1 | `crates/codegraph-analysis/src/report.rs:98-545` |
| 2044 | `cg-d2950b3f329d2bf5` `new` | `function` | risk, hub | 224 | 3 | `info`=1 | `calls`=224, `contains`=1, `references`=1 | `crates/codegraph-core/src/lib.rs:466-480` |
| 2023 | `cg-ac9d30a355c78e5a` `index_file` | `function` | risk, error_flow, hub | 4 | 202 | `info`=6 | `calls`=145, `references`=59, `contains`=1, `may_error`=1 | `crates/codegraph-indexer/src/scan.rs:667-1301` |
| 1934 | `cg-7e3964d36c4506c3` `insights` | `function` | risk, hub | 127 | 66 | `info`=13 | `calls`=180, `references`=12, `contains`=1 | `crates/codegraph-analysis/src/insights.rs:13-91` |
| 1746 | `cg-d810f145303d349d` `add_node` | `function` | risk, hub | 191 | 2 | `info`=2 | `calls`=190, `references`=2, `contains`=1 | `crates/codegraph-core/src/lib.rs:482-484` |
| 1656 | `cg-30c87037268b3d30` `add_edge` | `function` | risk, hub | 181 | 2 | `info`=2 | `calls`=180, `references`=2, `contains`=1 | `crates/codegraph-core/src/lib.rs:513-521` |
| 1655 | `cg-f332f153115e540a` `add_node_with_metadata` | `function` | risk, hub | 179 | 4 | `info`=2 | `calls`=179, `references`=3, `contains`=1 | `crates/codegraph-core/src/lib.rs:495-511` |
| 1509 | `cg-be330dd92396b19a` `escapeHtml` | `function` | risk, hub | 164 | 3 | `info`=2 | `calls`=163, `references`=3, `contains`=1 | `crates/codegraph-web/static/js/16-flow.js:1174-1181` |
| 1452 | `cg-66a839d2982b5fa0` `t` | `function` | hub | 160 | 2 | - | `calls`=160, `contains`=1, `references`=1 | `crates/codegraph-web/static/js/05-locale.js:5-7` |
| 1441 | `cg-b1c0c03e9f80fd44` `scan_project_with_scope` | `function` | risk, error_flow, hub | 6 | 122 | `info`=13 | `calls`=101, `references`=21, `may_error`=5, `contains`=1 | `crates/codegraph-indexer/src/scan.rs:97-335` |
| 1429 | `cg-c79b578b239bb0da` `scan_project` | `function` | risk, hub | 154 | 3 | `info`=3 | `calls`=153, `references`=3, `contains`=1 | `crates/codegraph-indexer/src/scan.rs:58-63` |
| 1392 | `cg-c323b107fd5dc54c` `resolve_pending_calls` | `function` | risk, hub | 4 | 141 | `info`=4 | `calls`=98, `references`=46, `contains`=1 | `crates/codegraph-indexer/src/resolve.rs:1377-1981` |
| 1154 | `cg-5ddd7ab359fb7be3` `pr_impact` | `function` | risk, hub | 8 | 107 | `info`=6 | `calls`=84, `references`=30, `contains`=1 | `crates/codegraph-analysis/src/pr_impact.rs:157-375` |
| 1113 | `cg-ab9b87e102457216` `parse_source` | `function` | risk, error_flow, hub | 78 | 31 | `info`=9 | `calls`=99, `references`=6, `may_error`=3, `contains`=1 | `crates/codegraph-parser/src/extract.rs:12-65` |
| 1093 | `cg-7231b9729e092e62` `export_wiki` | `function` | risk, error_flow, hub | 6 | 104 | `info`=6 | `calls`=73, `references`=34, `may_error`=2, `contains`=1 | `crates/codegraph-cli/src/wiki.rs:66-324` |
| 1078 | `cg-d511b273b15e893a` `impact_with_insights_mode` | `function` | risk, error_flow, hub | 5 | 98 | `info`=6 | `calls`=78, `references`=22, `may_error`=2, `contains`=1 | `crates/codegraph-analysis/src/refactoring.rs:636-809` |
| 1063 | `cg-cdb567578c39a099` `index_compose_entrypoints` | `function` | risk, hub | 4 | 107 | `info`=3 | `calls`=75, `references`=35, `contains`=1 | `crates/codegraph-indexer/src/runtime.rs:163-478` |
| 1011 | `cg-24d4b563129daa8f` `kubernetes_document_from_lines` | `function` | risk, error_flow, hub | 3 | 109 | `info`=6 | `references`=56, `calls`=53, `may_error`=2, `contains`=1 | `crates/codegraph-indexer/src/runtime.rs:1628-1941` |
| 932 | `cg-cd025d02923a89f7` `cache_diff_reports_added_removed_and_modified_files` | `function` | risk, error_flow, hub | 1 | 38 | `info`=41 | `may_error`=20, `calls`=18, `contains`=1 | `crates/codegraph-storage/src/lib.rs:2348-2464` |

Node summaries are truncated: showing 25 of 4556 important nodes.

## Compact File Summaries
| Score | File | Symbols | Trace | Imports | Config | Env | Errors | Unresolved | Risks | Trace kinds |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| 56993 | `cg-ee3c92e03a9056b1` `crates/codegraph-analysis/src/insights.rs` | 158 | 2350 | 3 | 0 | 0 | 14 | 1409 | `info`=424 | `calls`=1914, `references`=422, `may_error`=14 |
| 51908 | `cg-fe63b53f56a7f7e9` `crates/codegraph-analysis/src/query.rs` | 157 | 2310 | 3 | 0 | 0 | 58 | 1220 | `info`=246 | `calls`=1859, `references`=393, `may_error`=58 |
| 36454 | `cg-30025cf8bc99e019` `crates/codegraph-indexer/src/manifests.rs` | 114 | 1576 | 8 | 2 | 0 | 28 | 877 | `info`=200 | `calls`=1188, `references`=358, `may_error`=28, `reads_config`=2 |
| 34684 | `cg-dceca1acc0b42f99` `crates/codegraph-lsp/src/lib.rs` | 183 | 1415 | 21 | 0 | 1 | 87 | 676 | `info`=433 | `calls`=1094, `references`=233, `may_error`=87, `reads_environment`=1 |
| 33209 | `cg-87f7489dde024ef9` `crates/codegraph-indexer/src/runtime.rs` | 99 | 1485 | 4 | 0 | 0 | 9 | 728 | `info`=312 | `calls`=1070, `references`=406, `may_error`=9 |
| 32398 | `cg-ce015f1d29bf7bf1` `crates/codegraph-storage/src/lib.rs` | 103 | 1202 | 13 | 0 | 0 | 205 | 519 | `info`=537 | `calls`=858, `may_error`=205, `references`=139 |
| 27050 | `cg-6dae84628d387775` `crates/codegraph-parser/src/extract.rs` | 85 | 1255 | 5 | 0 | 0 | 41 | 549 | `info`=227 | `calls`=823, `references`=391, `may_error`=41 |
| 24724 | `cg-754972209269a8a9` `crates/codegraph-indexer/src/resolve.rs` | 84 | 1046 | 4 | 0 | 0 | 7 | 584 | `info`=202 | `calls`=806, `references`=233, `may_error`=7 |
| 20165 | `cg-9137df38151e2bd3` `crates/codegraph-server/src/analysis_handlers.rs` | 47 | 921 | 10 | 0 | 0 | 109 | 342 | `info`=169 | `calls`=614, `references`=198, `may_error`=109 |
| 19523 | `cg-57bea67c8e67d61d` `crates/codegraph-indexer/src/imports.rs` | 54 | 851 | 3 | 0 | 0 | 50 | 416 | `info`=142 | `calls`=599, `references`=202, `may_error`=50 |
| 17263 | `cg-90f7d906380e7031` `crates/codegraph-analysis/src/refactoring.rs` | 28 | 691 | 3 | 0 | 0 | 18 | 386 | `info`=213 | `calls`=541, `references`=132, `may_error`=18 |
| 16475 | `cg-c2b9c8bce2026ed5` `crates/codegraph-indexer/src/sql.rs` | 54 | 713 | 5 | 0 | 0 | 26 | 359 | `info`=137 | `calls`=503, `references`=184, `may_error`=26 |
| 13449 | `cg-772dad414bd11949` `crates/codegraph-indexer/src/scan.rs` | 30 | 613 | 9 | 0 | 0 | 11 | 283 | `info`=128 | `calls`=462, `references`=140, `may_error`=11 |
| 13039 | `cg-31b5fb33171f9669` `crates/codegraph-analysis/src/report.rs` | 31 | 478 | 4 | 0 | 0 | 83 | 231 | `info`=182 | `calls`=316, `may_error`=83, `references`=79 |
| 12690 | `cg-7eebb03d80185684` `crates/codegraph-web/static/js/12-filters.js` | 52 | 647 | 0 | 0 | 0 | 3 | 228 | `info`=130, `warning`=1 | `calls`=392, `references`=252, `may_error`=3 |
| 11027 | `cg-41fb10ec0e71f4a9` `crates/codegraph-indexer/src/frameworks.rs` | 33 | 473 | 2 | 0 | 0 | 18 | 256 | `info`=65 | `calls`=357, `references`=98, `may_error`=18 |
| 10989 | `cg-bbcb12cdb710d95f` `crates/codegraph-cli/src/main.rs` | 38 | 478 | 19 | 0 | 0 | 106 | 111 | `info`=162, `warning`=1 | `calls`=239, `references`=133, `may_error`=106 |
| 10620 | `cg-d60d3ba74cd4a2a8` `crates/codegraph-web/static/js/09-investigate.js` | 48 | 637 | 0 | 0 | 0 | 9 | 147 | `info`=85, `warning`=1 | `calls`=410, `references`=218, `may_error`=9 |
| 10565 | `cg-7e64b542059a39c6` `crates/codegraph-web/static/js/16-flow.js` | 61 | 647 | 0 | 0 | 0 | 0 | 143 | `info`=93 | `references`=327, `calls`=320 |
| 10537 | `cg-1f3fc24f1939d091` `crates/codegraph-analysis/src/mcp.rs` | 34 | 427 | 7 | 0 | 0 | 48 | 198 | `info`=117 | `calls`=323, `references`=56, `may_error`=48 |

File summaries are truncated: showing 25 of 127 files.

## Confidence Guide
| CodeGraph confidence | Report wording | How to read it |
| --- | --- | --- |
| `exact` | extracted | Exact project metadata, deterministic manifests, or compiler-like facts. |
| `semantic` | resolved | Resolved through a semantic analyzer or language server. |
| `syntactic` | extracted from syntax | Extracted directly from source syntax. |
| `heuristic` | inferred | Inferred by a named rule and worth reviewing at architectural or runtime boundaries. |
| `unknown` | ambiguous | Legacy, imported, or ambiguous evidence. |

## Key Concepts
| Score | Node | Kind | Hub kind | In | Out | Edge kinds |
| ---: | --- | --- | --- | ---: | ---: | --- |
| 336 | `cg-545f575ded3c6cf1` `main` | `function` | `architectural` | 3 | 333 | `calls`=130, `references`=110, `may_error`=95, `entrypoint`=1 |
| 205 | `cg-ac9d30a355c78e5a` `index_file` | `function` | `architectural` | 3 | 202 | `calls`=145, `references`=59, `may_error`=1 |
| 192 | `cg-d810f145303d349d` `add_node` | `function` | `architectural` | 190 | 2 | `calls`=190, `references`=2 |
| 192 | `cg-7e3964d36c4506c3` `insights` | `function` | `architectural` | 126 | 66 | `calls`=180, `references`=12 |
| 182 | `cg-30c87037268b3d30` `add_edge` | `function` | `architectural` | 180 | 2 | `calls`=180, `references`=2 |
| 182 | `cg-f332f153115e540a` `add_node_with_metadata` | `function` | `architectural` | 178 | 4 | `calls`=179, `references`=3 |
| 166 | `cg-be330dd92396b19a` `escapeHtml` | `function` | `architectural` | 163 | 3 | `calls`=163, `references`=3 |
| 156 | `cg-c79b578b239bb0da` `scan_project` | `function` | `architectural` | 153 | 3 | `calls`=153, `references`=3 |
| 153 | `cg-3a37cd9e02a71cf3` `project_report_markdown` | `function` | `architectural` | 6 | 147 | `may_error`=76, `calls`=43, `references`=34 |
| 144 | `cg-c323b107fd5dc54c` `resolve_pending_calls` | `function` | `architectural` | 3 | 141 | `calls`=98, `references`=46 |
| 136 | `cg-ff8d959bb9aca1b0` `crates/codegraph-web/static/js/04-dom.js` | `file` | `architectural` | 1 | 135 | `calls`=94, `references`=42 |
| 127 | `cg-b1c0c03e9f80fd44` `scan_project_with_scope` | `function` | `architectural` | 5 | 122 | `calls`=101, `references`=21, `may_error`=5 |
| 114 | `cg-5ddd7ab359fb7be3` `pr_impact` | `function` | `architectural` | 7 | 107 | `calls`=84, `references`=30 |
| 111 | `cg-24d4b563129daa8f` `kubernetes_document_from_lines` | `function` | `architectural` | 2 | 109 | `references`=56, `calls`=53, `may_error`=2 |
| 110 | `cg-cdb567578c39a099` `index_compose_entrypoints` | `function` | `architectural` | 3 | 107 | `calls`=75, `references`=35 |

Hotspots are truncated: showing 25 key hubs out of 3325 candidates (3294 architectural, 31 utility).

## Communities
| Community | Nodes | Files | Entrypoints | Internal edges | External edges | Languages | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| `crates/codegraph-analysis` | 1007 | 22 | 0 | 2490 | 1007 | `rust`=1006 | #89, #90, #91, #92, #93, +95 more |
| `crates/codegraph-indexer` | 870 | 18 | 0 | 2264 | 131 | `rust`=854, `sql`=15 | #4705, #4706, #4707, #4708, #4709, +95 more |
| `crates/codegraph-web` | 612 | 25 | 0 | 2078 | 20 | `javascript`=610 | #12794, #12795, #12796, #12797, #12798, +95 more |
| `crates/codegraph-server` | 491 | 13 | 73 | 1051 | 115 | `rust`=489 | #10552, #10558, #10565, #10580, #10581, +95 more |
| `crates/codegraph-parser` | 264 | 8 | 0 | 646 | 27 | `rust`=263 | #9497, #9498, #9499, #9500, #9501, +95 more |
| `crates/codegraph-cli` | 264 | 16 | 2 | 494 | 152 | `rust`=261 | #3392, #3394, #3406, #3407, #3408, +95 more |
| `crates/codegraph-lsp` | 185 | 2 | 0 | 388 | 48 | `rust`=184 | #8942, #8943, #8944, #8945, #8946, +95 more |
| `crates/codegraph-storage` | 105 | 2 | 0 | 282 | 61 | `rust`=104 | #12232, #12233, #12234, #12235, #12236, +95 more |
| `docs` | 104 | 7 | 0 | 108 | 84 | `markdown`=104 | #16051, #16052, #16053, #16054, #16055, +95 more |
| `root` | 50 | 9 | 2 | 46 | 128 | `markdown`=43 | #23, #24, #25, #26, #27, +95 more |
| `crates/codegraph-core` | 28 | 2 | 0 | 39 | 907 | `rust`=27 | #4647, #4648, #4649, #4650, #4651, +95 more |
| `crates/codegraph-ui` | 16 | 2 | 1 | 26 | 5 | `rust`=14 | #12687, #12704, #12705, #12706, #12708, +26 more |

Communities are truncated: showing 25 of 47 communities.

## Entrypoints
| Node | Kind | Source |
| --- | --- | --- |
| `cg-59694d29262a271e` `cargo bin:codegraph-cli` | `entrypoint` | `crates/codegraph-cli/Cargo.toml:2-2` |
| `cg-7bf026642975ff05` `cargo binary:codegraph` | `entrypoint` | `crates/codegraph-cli/Cargo.toml:12-12` |
| `cg-d539e957b36c07ca` `cargo bin:codegraph-server` | `entrypoint` | `crates/codegraph-server/Cargo.toml:2-2` |
| `cg-366ad221263995b6` `cargo bin:codegraph-ui` | `entrypoint` | `crates/codegraph-ui/Cargo.toml:2-2` |
| `cg-1bd5704207c9a5fc` `main` | `function` | `crates/codegraph-server/build.rs:11-43` |
| `cg-545f575ded3c6cf1` `main` | `function` | `crates/codegraph-cli/src/main.rs:100-953` |
| `cg-4f3131af78a6c532` `main` | `function` | `crates/codegraph-server/src/main.rs:45-221` |
| `cg-034812dcf5c5d389` `main` | `function` | `crates/codegraph-ui/src/main.rs:89-113` |
| `cg-a8b9f5e2866c2b1d` `route GET /` | `entrypoint` | `crates/codegraph-server/src/main.rs:93-93` |
| `cg-0d2766ac0d269797` `route GET /label-policy.js` | `entrypoint` | `crates/codegraph-server/src/main.rs:94-94` |
| `cg-0f4c957e6ba9e6a1` `route GET /app.js` | `entrypoint` | `crates/codegraph-server/src/main.rs:95-95` |
| `cg-71724ccbe22ad824` `route GET /styles.css` | `entrypoint` | `crates/codegraph-server/src/main.rs:96-96` |
| `cg-20ac227a8b932784` `route GET /api/capabilities` | `entrypoint` | `crates/codegraph-server/src/main.rs:97-97` |
| `cg-8eefc8ddaef5c06f` `route GET /api/schema` | `entrypoint` | `crates/codegraph-server/src/main.rs:98-98` |
| `cg-148b19af1bde1d98` `route GET /api/live` | `entrypoint` | `crates/codegraph-server/src/main.rs:99-99` |
| `cg-2b154275baa8e133` `route GET /api/ready` | `entrypoint` | `crates/codegraph-server/src/main.rs:100-100` |
| `cg-73a1fcf87aab8968` `route GET /api/health` | `entrypoint` | `crates/codegraph-server/src/main.rs:101-101` |
| `cg-a9d0fe0caaadcb89` `route GET /api/metrics` | `entrypoint` | `crates/codegraph-server/src/main.rs:102-102` |
| `cg-6084df36914ff8b3` `route GET /api/languages` | `entrypoint` | `crates/codegraph-server/src/main.rs:103-103` |
| `cg-2a37815eab72bbfb` `route GET /api/lsp` | `entrypoint` | `crates/codegraph-server/src/main.rs:104-104` |

Entrypoints are truncated: showing 20 of 83.

## Architecture Links
| Source | Target | Count | Edge kinds | Confidence | Evidence |
| --- | --- | ---: | --- | --- | --- |
| `crates/codegraph-analysis` | `crates/codegraph-core` | 799 | `calls`=786, `references`=13 | `heuristic`=578, `syntactic`=221 | #16350, #16589, #16775, #16805, #16824, +95 more |
| `crates/codegraph-server` | `crates/codegraph-analysis` | 57 | `calls`=57 | `heuristic`=56, `syntactic`=1 | #39172, #39173, #39174, #39175, #39176, +52 more |
| `crates/codegraph-cli` | `crates/codegraph-analysis` | 54 | `calls`=54 | `heuristic`=53, `syntactic`=1 | #25689, #25701, #25866, #25867, #26053, +49 more |
| `docs` | `crates/codegraph-analysis` | 49 | `references`=49 | `heuristic`=49 | #46542, #46543, #46546, #46547, #46548, +44 more |
| `crates/codegraph-indexer` | `crates/codegraph-core` | 47 | `calls`=47 | `heuristic`=47 | #27350, #27372, #27393, #27425, #27452, +42 more |
| `.` | `crates/codegraph-analysis` | 40 | `references`=40 | `heuristic`=35, `exact`=5 | #46374, #46375, #46383, #46386, #46393, +35 more |
| `crates/codegraph-cli` | `crates/codegraph-core` | 24 | `calls`=24 | `heuristic`=17, `syntactic`=7 | #26329, #26330, #26363, #26364, #26415, +19 more |
| `crates/codegraph-cli` | `crates/codegraph-storage` | 22 | `calls`=22 | `heuristic`=15, `syntactic`=7 | #25857, #25953, #26120, #26122, #26123, +17 more |
| `.` | `crates/codegraph-indexer` | 19 | `references`=19 | `heuristic`=12, `exact`=7 | #46376, #46378, #46380, #46384, #46385, +14 more |
| `.` | `crates/codegraph-web` | 18 | `references`=18 | `heuristic`=13, `exact`=5 | #46387, #46390, #46392, #46394, #46413, +13 more |
| `crates/codegraph-server` | `crates/codegraph-lsp` | 18 | `calls`=18 | `heuristic`=15, `syntactic`=3 | #39816, #39818, #39820, #39822, #39825, +13 more |
| `.` | `docs` | 14 | `references`=14 | `exact`=14 | #46421, #46422, #46423, #46424, #46425, +9 more |
| `crates/codegraph-cli` | `crates/codegraph-indexer` | 14 | `calls`=14 | `heuristic`=14 | #25729, #25746, #25760, #26117, #26262, +9 more |
| `crates/codegraph-cli` | `crates/codegraph-lsp` | 13 | `calls`=13 | `heuristic`=12, `syntactic`=1 | #26276, #26282, #26977, #26981, #26987, +8 more |
| `crates/codegraph-server` | `crates/codegraph-storage` | 12 | `calls`=12 | `heuristic`=11, `syntactic`=1 | #39072, #39089, #39106, #39123, #39140, +7 more |

## Surprising Links
| Score | Source | Target | Areas | Languages | Edge | Confidence | Reasons | Evidence |
| ---: | --- | --- | --- | --- | --- | --- | --- | --- |
| 12 | `cg-d6bec304462d89a8` `github workflow:CI/test` | `cg-897089bf45e84db8` `crates/codegraph-web/static/label-policy.test.js` | `.github` -> `crates/codegraph-web` | `unknown` -> `javascript` | `references` | `heuristic` | cross_area, rare_crossing, heuristic_confidence, entrypoint_boundary | #46040 |
| 11 | `cg-7d024d8d9f0bd954` `compile_ignored_globs` | `cg-9ca49e4fe97e74c0` `build` | `crates/codegraph-indexer` -> `crates/codegraph-analysis` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #30138 |
| 11 | `cg-ec58c69c51510812` `language_responses` | `cg-528c36fc4f6ab1d1` `language_adapters` | `crates/codegraph-server` -> `crates/codegraph-parser` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #40307 |
| 11 | `cg-242007d1ef4b944d` `metrics_api` | `cg-528c36fc4f6ab1d1` `language_adapters` | `crates/codegraph-server` -> `crates/codegraph-parser` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #40570 |
| 11 | `cg-d6ca5a4e89e1b014` `run_window` | `cg-9ca49e4fe97e74c0` `build` | `crates/codegraph-ui` -> `crates/codegraph-analysis` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #42257 |
| 11 | `cg-d6ca5a4e89e1b014` `run_window` | `cg-67e7950d46906aa0` `run` | `crates/codegraph-ui` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #42266 |
| 8 | `cg-d68259ea66397360` `node_context` | `cg-eeef8341e286012e` `is_test_like_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #16350 |
| 8 | `cg-5c959ee8e5feaa99` `add_unresolved_local_import_insights` | `cg-eeef8341e286012e` `is_test_like_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #16956 |
| 8 | `cg-5c959ee8e5feaa99` `add_unresolved_local_import_insights` | `cg-dc43688101c64863` `is_vendored_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #16957 |
| 8 | `cg-aba04ac0577458d1` `add_unresolved_sql_table_reference_insights` | `cg-eeef8341e286012e` `is_test_like_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #17023 |
| 8 | `cg-924923bc7a02e227` `starts_in_its_own_code` | `cg-eeef8341e286012e` `is_test_like_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #17574 |
| 8 | `cg-924923bc7a02e227` `starts_in_its_own_code` | `cg-dc43688101c64863` `is_vendored_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #17575 |
| 8 | `cg-41f54a2821014a58` `add_unreachable_config_read_insights` | `cg-eeef8341e286012e` `is_test_like_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #17610 |
| 8 | `cg-b4494a7d9df8cadb` `every_read_is_vendored` | `cg-dc43688101c64863` `is_vendored_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #17687 |
| 8 | `cg-4912ea1778ae6cfb` `add_rationale_risk_comment_insights` | `cg-dc43688101c64863` `is_vendored_source_path` | `crates/codegraph-analysis` -> `crates/codegraph-core` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #17855 |

Surprising links are truncated: showing 200 of 38518 candidates.

## Risks And Insights
| Kind | Severity | Count |
| --- | --- | ---: |
| `dependency_cycle` | `warning` | 2 |
| `low_entrypoint_coverage` | `warning` | 1 |
| `unresolved_call` | `info` | 4801 |
| `potential_error_flow` | `info` | 3146 |
| `unreachable_error_flow` | `info` | 2270 |
| `orphan_function` | `info` | 868 |
| `cross_language_heuristic_edge` | `info` | 155 |
| `ambiguous_call_resolution` | `info` | 57 |
| `duplicate_function_label` | `info` | 55 |
| `unreachable_source_file` | `info` | 35 |

### Insight Evidence
| Severity | Kind | Message | Evidence |
| --- | --- | --- | --- |
| `warning` | `dependency_cycle` | Directed dependency cycle across files involving `loadGraphPage` -> `loadProjectOverview` -> `renderOverview` -> `renderArchitecture` | nodes: cg-684e3eae463bcec3, cg-f2b231b5ea64b822, cg-e10a11ecfd95dc05, cg-55acfa324cc9a811; edges: #42955, #42978, #43077, #43090, #43235 |
| `warning` | `dependency_cycle` | Directed dependency cycle across files involving `selectNodeById` -> `clearSelection` -> `loadInsights` -> `initializeGraph` -> `runGraphQuery` -> ... | nodes: cg-63bda3507d6149c3, cg-731f9fdc3f1f4046, cg-b58b9813b85c7dfe, cg-d3e8b6605ed317cd, cg-024299113695827e, cg-97d38f02647de38b, cg-ff6d6acc66195fc0, cg-c626e3998d6d79e9, cg-d860927c1e59932a, cg-ea90455ab9f7f4c2, cg-a5b073f2c92d7ca3, cg-419f1b05576ff745, cg-9cb19a2927075004, cg-e738564263ff8bd6, cg-3752d29a74f2dfab, cg-dac8c1e80dd4c7d5, cg-00f6a8b425370b02, cg-3a11f8adba3b325e, cg-28448807c1c1b920, cg-5092ca9a7ecd9e06, cg-913083101fac8bbb, cg-f1f6e3784028fcbc, cg-6aafb28ada2a6217, cg-f41e3d7f0a754e80, cg-9924c655c426b56b, cg-9e5303d867405549, cg-1580f7127eb123e6, cg-1a772a06712a7c9f, cg-ff4b01fd82072f5a, cg-dce7f8716d2748b7, cg-2de988f6eed8777c, cg-b54c22a78e3b7812, cg-88c4263e82ca94fd, cg-f9cf7253d2cb6a11, cg-292ece7dd955c7bb, cg-cdadeae6b11704cd, cg-cad372864ac80c74, cg-759a44bad0166a2e, cg-fdb90773a0799eea, cg-faa07277bd833053, cg-368ec03504d9bf56, cg-6930d7b0f8c8ffca; edges: #42747, #42750, #43587, #43588, #43641, #43652, #43653, #43658, +77 more |
| `warning` | `low_entrypoint_coverage` | entrypoints reach 1488 of 3097 functions (48%), and 23% of calls resolve to a scanned function — the rest name a dependency, the standard library, or a method the syntax cannot type; counting the 150 exported functions as starting points reaches 53% — treat `unreachable_*` findings as gaps in call resolution, or as a library reached through its API, before reading them as dead code | cg-d6bec304462d89a8, cg-b0c3dc19d1e7efc7, cg-430851bec33a61db, cg-59694d29262a271e, cg-7bf026642975ff05, cg-545f575ded3c6cf1, cg-d539e957b36c07ca, cg-1bd5704207c9a5fc |
| `info` | `ambiguous_call_resolution` | Call `Arc::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-4f3131af78a6c532, cg-86557cb65cc74bf5, cg-415016e31e7531ca; edges: #39897, #41331 |
| `info` | `ambiguous_call_resolution` | Call `AtomicU64::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-4d337951626f13c5, cg-dceca1acc0b42f99, cg-16d0862a35fc3799, cg-4f3131af78a6c532, cg-f80f321293580a81, cg-86557cb65cc74bf5, cg-ce015f1d29bf7bf1, cg-8ef7808a8eedb832; edges: #33287, #36663, #39891, #39914, #40599, #41339, #41891 |
| `info` | `ambiguous_call_resolution` | Call `AtomicUsize::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d8a25e6b6eb07831, cg-9cc152340550cf9f, cg-d9e51f6be3b21183, cg-09c6d583a4c95539, cg-675f711cfe3c6481, cg-504d24ec4dae8bf9, cg-fa999f05c4ccb68d, cg-b1d494a4a47bc21f, cg-9e1deb0debde49f1, cg-16921d49ea151475, cg-c0ba70a9695fd1fe, cg-232250fad41ecfc8; edges: #19169, #19693, #25485, #25712, #25881, #26031, #26728, #26921, +3 more |
| `info` | `ambiguous_call_resolution` | Call `BTreeMap::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-05fc8d21be606901, cg-e4c5e2ffe79f98a1, cg-974e24db1d7c020f, cg-01b81d13078ad763, cg-53ab9b3f7bad79fb, cg-c09a400a1f6d72aa, cg-7e3964d36c4506c3, cg-5c959ee8e5feaa99, cg-eee43e9d2e88eaf4, cg-2fdd7e8f1691c8b3, cg-b50a509944cdd3ce, cg-41f54a2821014a58, cg-ef2026648b6f3896, cg-0e20e70153105ffb, cg-3862e34397bed91f, cg-4f1891c5b52c3f34, cg-ce9921d0dc04f49c, cg-2d730cb571f4f16a, cg-ca6a08cd5b67f654, cg-02db150916a6b615, cg-ca7d9552ca5c8be2, cg-457c7fef4e90120e, cg-8058cf17aca33edc, cg-7cc73ae5e6a91ab1, cg-f33563548e5fa22a, cg-c253f3c6f288050a, cg-17ed7a98c9addcd6, cg-5fc17909eee4ec2d, cg-3641d8e645d60bc2, cg-23610d92dbd17fb7, cg-5ddd7ab359fb7be3, cg-4a576f3692b5f215, cg-134f1ea7a738f0bd, cg-c53e3cdfb8f487fc, cg-a9144a1316867846, cg-966721a6e65dfd52, cg-ba5d11541227474f, cg-a54b4312f4ff6907, cg-b73fcd28508d78d0, cg-bf04066f2b2a283c, cg-48a6db618c93b3f3, cg-07c503359e1dca34, cg-a5ba4daecc094ac6, cg-d511b273b15e893a, cg-f62fdaf5ac53da88, cg-97fedb701147723c, cg-9bd61c6eced39a51, cg-58ada47a3c417cb0, cg-c3b7ea4eefb2a897, cg-28a0220a6526bcba, cg-9ab74a21cf21b45f, cg-8c543f02534e9830, cg-9ca49e4fe97e74c0, cg-cb86a3f3ad766fbd, cg-613b6f21eb9b177b, cg-02cd3a8f708ec27f, cg-78c834979f0bb690, cg-7c7c25b079655dc5, cg-96cb20a949b4de68, cg-b16f9a03daf60b73, cg-381a7789c8c74043, cg-e7619356e970ee58, cg-219ebccf6378c615, cg-57a82c05a3a399d0, cg-641a862f778196b4, cg-2ffbf9eed9fc8f17, cg-369ecf1e6c4fbbc4, cg-e3483a5bf17cad7b, cg-4c1b656b963d638d, cg-0512498dbb889d89, cg-30f8fdee94f5a9e0, cg-6c9bd1a3f0eae8b4, cg-5a2df730e2fd0336, cg-a82e28db304d064b, cg-4f24f38280ace263, cg-87f9691ffcc71a30, cg-7231b9729e092e62, cg-68cf4bcec2fa42be, cg-dbb60c8df27fd51b, cg-883938e7e72bbd33, cg-9e00c9fd48fd3e66, cg-d0441dbbd41bf9e1, cg-aae1705db58b135c, cg-35e1253f22025866, cg-6c1d641ef33ba7cd, cg-40c3c9853f56cba7, cg-8d4a6a93429e9d78, cg-8bd44d85cfbcf848, cg-31135cdb260c1d05, cg-e31f14c943c97e90, cg-4082d0b56f291071, cg-28e4ea9de6435ce2, cg-7ccce39311355df4, cg-c323b107fd5dc54c, cg-5a9aceda85031a0c, cg-cd24d54da63d131f, cg-6f41e399339eb56f, cg-a2cdfffc97697c73, cg-3499b1625aac1d09, cg-81d616cdf9db5d25, cg-6e13ae39c60fdda9, cg-e3d509b01ea5289e, cg-44efab9cd87e3034, cg-ddf3bd476cffb603, cg-e6c0c49a463a3985, cg-359d3d4b827622b6, cg-e8b714e8aee3dd8f, cg-d61e77ffd6163dc4, cg-a4e2496fb36df481, cg-2cafe5c90c33d602, cg-10a25556e045b16d, cg-c60a3a8f43142d41, cg-527a0e274eccf377, cg-f1f4caab909649d3, cg-02479a01dd6244b1, cg-cdb567578c39a099, cg-e510e945031f7c40, cg-b5af9ae75787ad7c, cg-1a27e9d4d17ff931, cg-ca167f8c6280e72e, cg-7e9fd13adb100509, cg-82ef2bd9dfb4f3e5, cg-1317ce56e1aa4f0d, cg-cca91192d469d389, cg-307cad2bc7e067f2, cg-db4d2e368b350aa9, cg-a04548f57528737b, cg-fba4d5a661e53e1b, cg-eef82e0b1624cecd, cg-777368437d4f25c1, cg-24d4b563129daa8f, cg-b1c0c03e9f80fd44, cg-6c436c928df45ddf, cg-d021720523f743df, cg-ac9d30a355c78e5a, cg-1057fda6fbf8872e, cg-f92969dd41ce8611, cg-5bca781e1ced44ee, cg-d8d3b005044da83d, cg-69af59277189a6d2, cg-b188a01528a35449, cg-7793b12d1ed8a08b, cg-9845255f488c227d, cg-2c4eca0299b7c2a7, cg-ab9b87e102457216, cg-1fdf709090d27876, cg-8cfaed1d453f116a, cg-49d43056df95c62c, cg-d15235dab34a6ad3, cg-3062d44e7b9fd184, cg-807382bb61254178, cg-f911f33c0f5aaaba, cg-4f3131af78a6c532, cg-8840f4a5795fa74f, cg-4dfbbc6d170de4ae, cg-7c7eb73b8f42d87e, cg-9b7ac3848a35db9f, cg-d2a8207c3627ac0e, cg-9b19fffea754c372, cg-86557cb65cc74bf5, cg-9ac781cfe61d8469, cg-e1806ec715857159, cg-23137e02ebfe4a79, cg-b114f96fbcb423bf; edges: #16410, #16414, #16510, #16533, #16668, #16741, #16888, #16934, +155 more |
| `info` | `ambiguous_call_resolution` | Call `BTreeSet::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-98d5e44c151d6bba, cg-c09a400a1f6d72aa, cg-5c959ee8e5feaa99, cg-19f83dc8af3c944d, cg-83987d0780a69bf7, cg-3862e34397bed91f, cg-2d730cb571f4f16a, cg-36cdf34dfa33c52e, cg-457c7fef4e90120e, cg-16bf4ba26dd0226f, cg-5ddd7ab359fb7be3, cg-8f328fcffbc68721, cg-29a9e73dd5e473df, cg-aea4362a2e0d2a3d, cg-5b3ae3dfcd9f4ce4, cg-2d89225ecdf3013e, cg-ca926f8768023cf0, cg-e14b6ca071175c43, cg-18b55790cd3b984d, cg-571c75da0e82975d, cg-b48e6417ab8c8c55, cg-9187dbfd812e8b66, cg-8cfa18e6cc8c1878, cg-5ffd3bb4465fd3b7, cg-2c551e90e38bee68, cg-b54f14c42f14abb4, cg-d511b273b15e893a, cg-9bd61c6eced39a51, cg-28a0220a6526bcba, cg-9ab74a21cf21b45f, cg-bd5ada189f1a6e34, cg-6663be0b924ca8dc, cg-db93baa9d3d1ea4f, cg-b68c85528a3c6108, cg-9a80ef0fb9c3b3cf, cg-abe0053cd3eb9d8b, cg-c60bcbf4d7761479, cg-148240c31c28c061, cg-b2a23392fe710052, cg-5a9aceda85031a0c, cg-b1c0c03e9f80fd44, cg-1fe861bfda241ce2, cg-ac9d30a355c78e5a, cg-b9ff4fbff2c6cfbc, cg-b610b1e09abf651b, cg-dcd0997f0e80304a, cg-94a40b9462432e19, cg-81c22c51efd6f50e, cg-9e752e7ac344df9a, cg-69af59277189a6d2, cg-912abb8ee08feac2, cg-6925ffd68c6928dc, cg-af3548762c07307f, cg-20c7521953d5f658, cg-7d9a874a87a5a983, cg-854f2d5eaffd0269, cg-092d22ec02604922, cg-f145b4f4b7c2ad29, cg-d15235dab34a6ad3, cg-e5505aa7b6cfb774, cg-a60d3a8bd04213ff, cg-5f6ca1879ec842fa, cg-e1806ec715857159, cg-8b72aa81e08145d2, cg-e586cfed452ebc9b, cg-8f635593f5408cac, cg-23137e02ebfe4a79, cg-79e31488852b527a, cg-ed5d3ed6e3e41671; edges: #16422, #16740, #16962, #17782, #17800, #17958, #18161, #18229, +60 more |
| `info` | `ambiguous_call_resolution` | Call `BufReader::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: cg-6bca22d4a64b4ae6, cg-0b20b28f283bad1c, cg-55fdf0b6a5e1804e, cg-ff79682a2c35f230; edges: #36119, #36730, #36738 |
| `info` | `ambiguous_call_resolution` | Call `CodeGraph::new` has 2 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-indexer/src/scan.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-b1c0c03e9f80fd44, cg-71eb59d3e229da53, cg-9aaf7317cbea8c3c, cg-96a0ff292b3a060f, cg-972ef45379de2270, cg-0eba1324ec3fa849, cg-c2c49f33e8ee249d, cg-10ae5cd44675bdcd, cg-2b63f29c54326d40, cg-cae74fa5c85ac004, cg-7883b1d8b52afae1, cg-50c7baa60b0a1d39, cg-cac9cc6a421efee5, cg-78d0307272a963ed, cg-8d9fe48bde5e3e1d; edges: #32362, #36755, #36850, #36872, #36946, #36954, #36962, #41929, +6 more |
| `info` | `ambiguous_call_resolution` | Call `Command::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: cg-6925ffd68c6928dc, cg-ad813e449534e799, cg-1f713c9eec5391fc, cg-c14e065c9ed6bbd5, cg-4db1c84674b76a35; edges: #36086, #36636, #36664, #42272 |
| `info` | `ambiguous_call_resolution` | Call `Cursor::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: cg-0b20b28f283bad1c, cg-55fdf0b6a5e1804e, cg-019819c41c64ad1c; edges: #36731, #36739 |
| `info` | `ambiguous_call_resolution` | Call `EventLoop::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d6ca5a4e89e1b014, cg-6ea3c6a5995d056b; edges: #42255 |
| `info` | `ambiguous_call_resolution` | Call `Glob::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d215e7cc8e49c357, cg-9b0afe81db28e9a7, cg-7d024d8d9f0bd954, cg-94fc2b79229062e2; edges: #22643, #30124, #30134 |
| `info` | `ambiguous_call_resolution` | Call `GlobSetBuilder::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d215e7cc8e49c357, cg-7d024d8d9f0bd954, cg-c94d5a08d3e09743; edges: #22640, #30131 |
| `info` | `ambiguous_call_resolution` | Call `HeaderMap::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-9d21a399110331bd, cg-9c51a267e161d99e, cg-1c9cde3139204a71, cg-e7c84c036ceda63e, cg-ae3624c7fc80decf, cg-e941a13854796985; edges: #40643, #40652, #40655, #40670, #40678 |
| `info` | `ambiguous_call_resolution` | Call `KeepAlive::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-9b0ffb294f75a217, cg-003f00db01ec7e62, cg-7ef191fb066d191d; edges: #39734, #40515 |
| `info` | `ambiguous_call_resolution` | Call `LogicalSize::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d6ca5a4e89e1b014, cg-6b24b4d853c8f39b; edges: #42261 |
| `info` | `ambiguous_call_resolution` | Call `OpenOptions::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-bf04fc10ea6b7b95, cg-d5346c73eac90e8b, cg-75ce21266a9c11d1; edges: #19105, #26689 |
| `info` | `ambiguous_call_resolution` | Call `Parser::new` has 8 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-ab9b87e102457216, cg-a9df755b7149e2fb; edges: #37288 |

Insights are truncated: showing 50 of 11404.

## Suggested Questions
- What startup flow is reachable from cargo bin:codegraph-cli?
- Why is main a central graph hotspot?
- What responsibilities and external dependencies does the crates/codegraph-analysis community have?
- What evidence explains the architecture link from crates/codegraph-analysis to crates/codegraph-core?
- Why is the references edge from github workflow:CI/test to crates/codegraph-web/static/label-policy.test.js surprising?
- Which code paths are involved in dependency_cycle findings?
