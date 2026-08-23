# CodeGraph Project Report

- Root: `.`
- Generated at unix: `1787525258`
- Graph schema version: `1`
- Quality gate: **passed** (`fail_on=error`, failing_insights=0)
- Risk: **low** (score 60, total 10181, errors 0, warnings 6, infos 10175)

## Summary
| Metric | Count |
| --- | ---: |
| Nodes | 19331 |
| Edges | 42228 |
| Entrypoints | 83 |
| Skipped files | 0 |

### Languages
| Language | Count |
| --- | ---: |
| `rust` | 15189 |
| `javascript` | 3895 |
| `markdown` | 129 |

### Node Kinds
| Kind | Count |
| --- | ---: |
| `control_flow` | 10280 |
| `external_dependency` | 5298 |
| `function` | 2834 |
| `type` | 454 |
| `module` | 202 |
| `file` | 125 |
| `entrypoint` | 79 |
| `directory` | 26 |
| `config` | 24 |
| `environment` | 4 |
| `unknown` | 4 |
| `repository` | 1 |

### Edge Confidence
| Confidence | Count |
| --- | ---: |
| `heuristic` | 34363 |
| `syntactic` | 7437 |
| `exact` | 428 |

## Compact Node Summaries
| Score | Node | Kind | Roles | In | Out | Risks | Edge kinds | Source |
| ---: | --- | --- | --- | ---: | ---: | --- | --- | --- |
| 4903 | `n14683` `to_string` | `external_dependency` | risk, external_boundary, hub | 542 | 0 | `info`=1 | `calls`=542 | `crates/codegraph-analysis/src/ask.rs:40-40` |
| 4556 | `n3566` `main` | `function` | entrypoint, risk, error_flow, hub | 4 | 320 | `info`=100, `warning`=1 | `calls`=122, `references`=108, `may_error`=92, `contains`=1, `entrypoint`=1 | `crates/codegraph-cli/src/main.rs:99-931` |
| 4435 | `n14703` `map` | `external_dependency` | risk, external_boundary, hub | 490 | 0 | `info`=1 | `calls`=490 | `crates/codegraph-analysis/src/ask.rs:130-133` |
| 4101 | `n14714` `Some` | `external_dependency` | external_boundary, hub | 454 | 0 | - | `calls`=454 | `crates/codegraph-analysis/src/ask.rs:213-213` |
| 3741 | `n15350` `assert_eq` | `external_dependency` | external_boundary, hub | 414 | 0 | - | `calls`=414 | `crates/codegraph-analysis/src/mcp.rs:707-707` |
| 3633 | `n15352` `assert` | `external_dependency` | external_boundary, hub | 402 | 0 | - | `calls`=402 | `crates/codegraph-analysis/src/mcp.rs:710-710` |
| 3606 | `n14682` `format` | `external_dependency` | external_boundary, hub | 399 | 0 | - | `calls`=399 | `crates/codegraph-analysis/src/ask.rs:39-39` |
| 3220 | `n14763` `collect` | `external_dependency` | risk, external_boundary, hub | 355 | 0 | `info`=1 | `calls`=355 | `crates/codegraph-analysis/src/ask.rs:808-811` |
| 2860 | `n14702` `filter` | `external_dependency` | risk, external_boundary, hub | 315 | 0 | `info`=1 | `calls`=315 | `crates/codegraph-analysis/src/ask.rs:130-132` |
| 2653 | `n15348` `unwrap` | `external_dependency` | risk, external_boundary, hub | 292 | 0 | `info`=1 | `calls`=292 | `crates/codegraph-analysis/src/mcp.rs:701-704` |
| 2644 | `n14713` `find` | `external_dependency` | risk, external_boundary, hub | 291 | 0 | `info`=1 | `calls`=291 | `crates/codegraph-analysis/src/ask.rs:173-175` |
| 2627 | `n2252` `project_report_markdown` | `function` | risk, error_flow, hub | 4 | 146 | `info`=77 | `may_error`=76, `calls`=41, `references`=32, `contains`=1 | `crates/codegraph-analysis/src/report.rs:54-501` |
| 2157 | `n14689` `Ok` | `external_dependency` | external_boundary, hub | 238 | 0 | - | `calls`=238 | `crates/codegraph-analysis/src/ask.rs:65-74` |
| 2128 | `n10593` `api_schema_lists_agent_contracts` | `function` | risk, error_flow, hub | 1 | 74 | `info`=105 | `may_error`=52, `calls`=22, `contains`=1 | `crates/codegraph-server/src/tests.rs:1955-3165` |
| 2113 | `n15353` `expect` | `external_dependency` | risk, external_boundary, hub | 232 | 0 | `info`=1 | `calls`=232 | `crates/codegraph-analysis/src/mcp.rs:768-773` |
| 2050 | `n14676` `is_some_and` | `external_dependency` | risk, external_boundary, hub | 225 | 0 | `info`=1 | `calls`=225 | `crates/codegraph-analysis/src/ask.rs:22-25` |
| 2037 | `n7052` `scan_project_adds_manifest_dependency_edges` | `function` | risk, error_flow, hub | 1 | 73 | `info`=99 | `may_error`=49, `calls`=23, `contains`=1, `references`=1 | `crates/codegraph-indexer/src/tests.rs:4269-5232` |
| 2032 | `n14735` `Vec::new` | `external_dependency` | risk, external_boundary, hub | 223 | 0 | `info`=1 | `calls`=223 | `crates/codegraph-analysis/src/ask.rs:607-607` |
| 1951 | `n14693` `graph.nodes.iter` | `external_dependency` | risk, external_boundary, hub | 214 | 0 | `info`=1 | `calls`=214 | `crates/codegraph-analysis/src/ask.rs:83-85` |
| 1859 | `n6610` `index_file` | `function` | risk, error_flow, hub | 2 | 186 | `info`=5 | `calls`=135, `references`=51, `contains`=1, `may_error`=1 | `crates/codegraph-indexer/src/scan.rs:463-1022` |

Node summaries are truncated: showing 25 of 8895 important nodes.

## Compact File Summaries
| Score | File | Symbols | Trace | Imports | Config | Env | Errors | Unresolved | Risks | Trace kinds |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| 86348 | `n6969` `crates/codegraph-indexer/src/tests.rs` | 118 | 2679 | 8 | 0 | 0 | 833 | 1204 | `info`=1814, `warning`=2 | `calls`=1799, `may_error`=833, `references`=47 |
| 51063 | `n389` `crates/codegraph-analysis/src/insights.rs` | 136 | 2070 | 3 | 0 | 0 | 11 | 1288 | `info`=370 | `calls`=1701, `references`=358, `may_error`=11 |
| 51033 | `n1465` `crates/codegraph-analysis/src/query.rs` | 154 | 2277 | 3 | 0 | 0 | 56 | 1197 | `info`=243 | `calls`=1834, `references`=387, `may_error`=56 |
| 48029 | `n2638` `crates/codegraph-analysis/src/tests.rs` | 195 | 2334 | 4 | 0 | 0 | 217 | 623 | `info`=691 | `calls`=2065, `may_error`=217, `references`=52 |
| 35184 | `n5075` `crates/codegraph-indexer/src/manifests.rs` | 112 | 1535 | 8 | 2 | 0 | 28 | 837 | `info`=195 | `calls`=1159, `references`=346, `may_error`=28, `reads_config`=2 |
| 34424 | `n8029` `crates/codegraph-lsp/src/lib.rs` | 183 | 1415 | 21 | 0 | 1 | 87 | 663 | `info`=433 | `calls`=1094, `references`=233, `may_error`=87, `reads_environment`=1 |
| 32189 | `n6076` `crates/codegraph-indexer/src/runtime.rs` | 97 | 1447 | 4 | 0 | 0 | 8 | 703 | `info`=301 | `calls`=1042, `references`=397, `may_error`=8 |
| 30803 | `n10856` `crates/codegraph-storage/src/lib.rs` | 102 | 1169 | 13 | 0 | 0 | 195 | 480 | `info`=509 | `calls`=835, `may_error`=195, `references`=139 |
| 23471 | `n10511` `crates/codegraph-server/src/tests.rs` | 70 | 851 | 31 | 0 | 0 | 166 | 329 | `info`=468 | `calls`=600, `may_error`=166, `references`=85 |
| 21534 | `n5687` `crates/codegraph-indexer/src/resolve.rs` | 72 | 924 | 4 | 0 | 0 | 4 | 505 | `info`=175 | `calls`=713, `references`=207, `may_error`=4 |
| 20310 | `n9424` `crates/codegraph-server/src/analysis_handlers.rs` | 46 | 922 | 10 | 0 | 0 | 112 | 345 | `info`=171 | `calls`=616, `references`=194, `may_error`=112 |
| 18215 | `n8741` `crates/codegraph-parser/src/extract.rs` | 54 | 855 | 5 | 0 | 0 | 24 | 365 | `info`=161 | `calls`=545, `references`=286, `may_error`=24 |
| 17918 | `n9169` `crates/codegraph-parser/src/tests.rs` | 65 | 615 | 3 | 0 | 0 | 96 | 336 | `info`=280 | `calls`=503, `may_error`=96, `references`=16 |
| 17093 | `n2065` `crates/codegraph-analysis/src/refactoring.rs` | 28 | 689 | 3 | 0 | 0 | 18 | 379 | `info`=212 | `calls`=539, `references`=132, `may_error`=18 |
| 14575 | `n6736` `crates/codegraph-indexer/src/sql.rs` | 44 | 627 | 5 | 0 | 0 | 24 | 317 | `info`=126 | `calls`=444, `references`=159, `may_error`=24 |
| 12209 | `n2246` `crates/codegraph-analysis/src/report.rs` | 27 | 448 | 4 | 0 | 0 | 83 | 208 | `info`=177 | `calls`=286, `may_error`=83, `references`=79 |
| 11390 | `n13041` `crates/codegraph-web/static/js/12-filters.js` | 50 | 597 | 0 | 0 | 0 | 3 | 199 | `info`=109, `warning`=1 | `calls`=360, `references`=234, `may_error`=3 |
| 10978 | `n6583` `crates/codegraph-indexer/src/scan.rs` | 24 | 508 | 8 | 0 | 0 | 8 | 230 | `info`=101 | `calls`=388, `references`=112, `may_error`=8 |
| 10590 | `n12203` `crates/codegraph-web/static/js/09-investigate.js` | 48 | 637 | 0 | 0 | 0 | 9 | 147 | `info`=82, `warning`=1 | `calls`=410, `references`=218, `may_error`=9 |
| 10545 | `n13980` `crates/codegraph-web/static/js/16-flow.js` | 61 | 647 | 0 | 0 | 0 | 0 | 143 | `info`=91 | `references`=327, `calls`=320 |

File summaries are truncated: showing 25 of 125 files.

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
| 323 | `n3566` `main` | `function` | `architectural` | 3 | 320 | `calls`=122, `references`=108, `may_error`=92, `entrypoint`=1 |
| 187 | `n6610` `index_file` | `function` | `architectural` | 1 | 186 | `calls`=135, `references`=51, `may_error`=1 |
| 171 | `n393` `insights` | `function` | `architectural` | 105 | 66 | `calls`=161, `references`=10 |
| 165 | `n4397` `add_node` | `function` | `architectural` | 163 | 2 | `calls`=165 |
| 164 | `n14041` `escapeHtml` | `function` | `architectural` | 161 | 3 | `calls`=163, `references`=1 |
| 160 | `n4399` `add_node_with_metadata` | `function` | `architectural` | 156 | 4 | `calls`=159, `references`=1 |
| 157 | `n4400` `add_edge` | `function` | `architectural` | 155 | 2 | `calls`=157 |
| 149 | `n2252` `project_report_markdown` | `function` | `architectural` | 3 | 146 | `may_error`=76, `calls`=41, `references`=32 |
| 135 | `n11473` `crates/codegraph-web/static/js/04-dom.js` | `file` | `architectural` | 0 | 135 | `calls`=94, `references`=41 |
| 125 | `n5698` `resolve_pending_calls` | `function` | `architectural` | 1 | 124 | `calls`=87, `references`=38 |
| 112 | `n7076` `temp_project_root` | `function` | `architectural` | 103 | 9 | `calls`=111, `may_error`=1 |
| 112 | `n1398` `pr_impact` | `function` | `architectural` | 5 | 107 | `calls`=84, `references`=28 |
| 111 | `n6598` `scan_project` | `function` | `architectural` | 108 | 3 | `calls`=110, `references`=1 |
| 110 | `n6107` `kubernetes_document_from_lines` | `function` | `architectural` | 1 | 109 | `references`=55, `calls`=53, `may_error`=2 |
| 107 | `n6083` `index_compose_entrypoints` | `function` | `architectural` | 1 | 106 | `calls`=74, `references`=33 |

Hotspots are truncated: showing 25 key hubs out of 3039 candidates (3007 architectural, 32 utility).

## Communities
| Community | Nodes | Files | Entrypoints | Internal edges | External edges | Languages | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| `crates/codegraph-analysis` | 935 | 22 | 0 | 2267 | 1019 | `rust`=934 | #70, #71, #72, #73, #74, +95 more |
| `crates/codegraph-indexer` | 747 | 18 | 0 | 2031 | 167 | `rust`=746 | #4451, #4452, #4453, #4454, #4455, +95 more |
| `crates/codegraph-web` | 600 | 24 | 0 | 2050 | 2 | `javascript`=598 | #11610, #11627, #11643, #11644, #11645, +95 more |
| `crates/codegraph-server` | 478 | 13 | 73 | 1002 | 126 | `rust`=476 | #9447, #9453, #9460, #9475, #9476, +95 more |
| `crates/codegraph-cli` | 259 | 16 | 2 | 479 | 364 | `rust`=256 | #3172, #3174, #3186, #3187, #3188, +95 more |
| `crates/codegraph-parser` | 207 | 8 | 0 | 488 | 28 | `rust`=206 | #8617, #8618, #8619, #8620, #8621, +95 more |
| `crates/codegraph-lsp` | 185 | 2 | 0 | 388 | 62 | `rust`=184 | #8067, #8068, #8069, #8070, #8071, +95 more |
| `crates/codegraph-storage` | 104 | 2 | 0 | 278 | 85 | `rust`=103 | #11059, #11060, #11061, #11062, #11063, +95 more |
| `docs` | 103 | 7 | 0 | 107 | 81 | `markdown`=103 | #14770, #14771, #14772, #14773, #14774, +95 more |
| `root` | 33 | 8 | 2 | 30 | 46 | `markdown`=26 | #21, #22, #23, #27, #29, +71 more |
| `crates/codegraph-core` | 25 | 2 | 0 | 36 | 775 | `rust`=24 | #4398, #4399, #4400, #4401, #4402, +95 more |
| `crates/codegraph-ui` | 16 | 2 | 1 | 26 | 3 | `rust`=14 | #11503, #11520, #11521, #11522, #11524, +24 more |

Communities are truncated: showing 25 of 41 communities.

## Entrypoints
| Node | Kind | Source |
| --- | --- | --- |
| `n3164` `cargo bin:codegraph-cli` | `entrypoint` | - |
| `n3165` `cargo binary:codegraph` | `entrypoint` | - |
| `n9409` `cargo bin:codegraph-server` | `entrypoint` | - |
| `n11309` `cargo bin:codegraph-ui` | `entrypoint` | - |
| `n9414` `main` | `function` | `crates/codegraph-server/build.rs:11-43` |
| `n3566` `main` | `function` | `crates/codegraph-cli/src/main.rs:99-931` |
| `n10061` `main` | `function` | `crates/codegraph-server/src/main.rs:45-221` |
| `n11327` `main` | `function` | `crates/codegraph-ui/src/main.rs:89-113` |
| `n10063` `route GET /` | `entrypoint` | `crates/codegraph-server/src/main.rs:93-93` |
| `n10064` `route GET /label-policy.js` | `entrypoint` | `crates/codegraph-server/src/main.rs:94-94` |
| `n10065` `route GET /app.js` | `entrypoint` | `crates/codegraph-server/src/main.rs:95-95` |
| `n10066` `route GET /styles.css` | `entrypoint` | `crates/codegraph-server/src/main.rs:96-96` |
| `n10067` `route GET /api/capabilities` | `entrypoint` | `crates/codegraph-server/src/main.rs:97-97` |
| `n10068` `route GET /api/schema` | `entrypoint` | `crates/codegraph-server/src/main.rs:98-98` |
| `n10069` `route GET /api/live` | `entrypoint` | `crates/codegraph-server/src/main.rs:99-99` |
| `n10070` `route GET /api/ready` | `entrypoint` | `crates/codegraph-server/src/main.rs:100-100` |
| `n10071` `route GET /api/health` | `entrypoint` | `crates/codegraph-server/src/main.rs:101-101` |
| `n10072` `route GET /api/metrics` | `entrypoint` | `crates/codegraph-server/src/main.rs:102-102` |
| `n10073` `route GET /api/languages` | `entrypoint` | `crates/codegraph-server/src/main.rs:103-103` |
| `n10074` `route GET /api/lsp` | `entrypoint` | `crates/codegraph-server/src/main.rs:104-104` |

Entrypoints are truncated: showing 20 of 83.

## Architecture Links
| Source | Target | Count | Edge kinds | Confidence | Evidence |
| --- | --- | ---: | --- | --- | --- |
| `crates/codegraph-analysis` | `crates/codegraph-core` | 675 | `calls`=662, `references`=13 | `heuristic`=484, `syntactic`=191 | #15051, #15290, #15476, #15506, #15525, +95 more |
| `crates/codegraph-analysis` | `crates/codegraph-cli` | 152 | `calls`=152 | `heuristic`=152 | #15031, #15059, #15265, #15310, #15321, +95 more |
| `crates/codegraph-server` | `crates/codegraph-analysis` | 56 | `calls`=56 | `heuristic`=55, `syntactic`=1 | #35215, #35216, #35217, #35218, #35219, +51 more |
| `crates/codegraph-cli` | `crates/codegraph-analysis` | 53 | `calls`=53 | `heuristic`=52, `syntactic`=1 | #23657, #23658, #23669, #23834, #23835, +48 more |
| `docs` | `crates/codegraph-analysis` | 49 | `references`=49 | `heuristic`=49 | #42169, #42170, #42173, #42174, #42175, +44 more |
| `crates/codegraph-indexer` | `crates/codegraph-core` | 47 | `calls`=47 | `heuristic`=47 | #25260, #25279, #25297, #25322, #25347, +42 more |
| `crates/codegraph-indexer` | `crates/codegraph-cli` | 30 | `calls`=30 | `heuristic`=30 | #25455, #26329, #26390, #27749, #27791, +25 more |
| `crates/codegraph-storage` | `crates/codegraph-indexer` | 29 | `calls`=29 | `syntactic`=19, `heuristic`=10 | #37245, #37255, #37339, #37342, #37367, +24 more |
| `crates/codegraph-cli` | `crates/codegraph-core` | 25 | `calls`=24, `references`=1 | `heuristic`=17, `syntactic`=8 | #24177, #24276, #24277, #24310, #24311, +20 more |
| `crates/codegraph-cli` | `crates/codegraph-storage` | 23 | `calls`=23 | `heuristic`=16, `syntactic`=7 | #23825, #23921, #24088, #24090, #24091, +18 more |
| `crates/codegraph-cli` | `crates/codegraph-indexer` | 21 | `calls`=21 | `heuristic`=13, `syntactic`=8 | #23697, #23698, #23714, #23715, #23728, +16 more |
| `crates/codegraph-server` | `crates/codegraph-lsp` | 21 | `calls`=21 | `heuristic`=17, `syntactic`=4 | #35724, #35852, #35854, #35856, #35858, +16 more |
| `.` | `crates/codegraph-analysis` | 19 | `references`=19 | `heuristic`=19 | #42146, #42147, #42148, #42149, #42153, +14 more |
| `crates/codegraph-server` | `crates/codegraph-cli` | 16 | `calls`=16 | `heuristic`=16 | #35046, #35725, #35755, #35779, #35850, +11 more |
| `crates/codegraph-server` | `crates/codegraph-storage` | 15 | `calls`=15 | `heuristic`=14, `syntactic`=1 | #35097, #35115, #35132, #35149, #35166, +10 more |

## Surprising Links
| Score | Source | Target | Areas | Languages | Edge | Confidence | Reasons | Evidence |
| ---: | --- | --- | --- | --- | --- | --- | --- | --- |
| 12 | `n5` `github workflow:CI/test` | `n14457` `crates/codegraph-web/static/label-policy.test.js` | `.github` -> `crates/codegraph-web` | `unknown` -> `javascript` | `references` | `heuristic` | cross_area, rare_crossing, heuristic_confidence, entrypoint_boundary | #41774 |
| 11 | `n1276` `fmt` | `n10945` `write_str` | `crates/codegraph-analysis` -> `crates/codegraph-storage` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #17713 |
| 11 | `n8061` `fmt` | `n10945` `write_str` | `crates/codegraph-lsp` -> `crates/codegraph-storage` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #32450 |
| 11 | `n9136` `fmt` | `n10945` `write_str` | `crates/codegraph-parser` -> `crates/codegraph-storage` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #34423 |
| 11 | `n10906` `default_cache_dir` | `n3567` `from` | `crates/codegraph-storage` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #37439 |
| 11 | `n10942` `write_bool` | `n3567` `from` | `crates/codegraph-storage` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #37735 |
| 11 | `n10946` `write_bytes` | `n3567` `from` | `crates/codegraph-storage` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #37744 |
| 11 | `n11328` `run_window` | `n2550` `build` | `crates/codegraph-ui` -> `crates/codegraph-analysis` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #38093 |
| 11 | `n11328` `run_window` | `n3822` `run` | `crates/codegraph-ui` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #38102 |
| 8 | `n82` `query_compacted_node` | `n3567` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15031 |
| 8 | `n162` `node_context` | `n3567` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15059 |
| 8 | `n187` `query_edges` | `n4105` `remove` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15265 |
| 8 | `n305` `export_dot` | `n3567` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15310 |
| 8 | `n307` `export_graphml` | `n3567` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15321 |
| 8 | `n312` `export_cypher` | `n3567` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15357 |

Surprising links are truncated: showing 200 of 34791 candidates.

## Risks And Insights
| Kind | Severity | Count |
| --- | --- | ---: |
| `dependency_cycle` | `warning` | 3 |
| `rationale_risk_comment` | `warning` | 3 |
| `low_entrypoint_coverage` | `warning` | 1 |
| `unresolved_call` | `info` | 4448 |
| `potential_error_flow` | `info` | 2734 |
| `unreachable_error_flow` | `info` | 1902 |
| `orphan_function` | `info` | 804 |
| `ambiguous_call_resolution` | `info` | 100 |
| `cross_language_heuristic_edge` | `info` | 84 |
| `duplicate_function_label` | `info` | 55 |

### Insight Evidence
| Severity | Kind | Message | Evidence |
| --- | --- | --- | --- |
| `warning` | `dependency_cycle` | Directed dependency cycle across files involving `loadGraphPage` -> `loadProjectOverview` -> `renderOverview` -> `renderArchitecture` | nodes: n11700, n11701, n12002, n12017; edges: #38719, #38742, #38841, #38854, #38999 |
| `warning` | `dependency_cycle` | Directed dependency cycle across files involving `selectNodeById` -> `clearSelection` -> `loadInsights` -> `initializeGraph` -> `runGraphQuery` -> ... | nodes: n11567, n11568, n12236, n12244, n12246, n12792, n12798, n12799, n12805, n12811, n12817, n12824, n12826, n12829, n12830, n12831, n12832, n12833, n13042, n13043, n13047, n13048, n13054, n13056, n13057, n13058, n13079, n13080, n13083, n13360, n13362, n13363, n13364, n13578, n13579, n13580, n13581, n13597, n13601, n13795, n13796, n13799; edges: #38511, #38514, #39351, #39352, #39405, #39416, #39417, #39422, +77 more |
| `warning` | `low_entrypoint_coverage` | entrypoints reach 1393 of 2834 functions (49%), and 24% of calls resolve to a scanned function; counting the 146 exported functions as starting points reaches 53% — treat `unreachable_*` findings as gaps in call resolution, or as a library reached through its API, before reading them as dead code | n5, n27, n28, n3164, n3165, n3566, n9409, n9414 |
| `warning` | `rationale_risk_comment` | FIXME comment `FIXME: handle errors instead of unwrapping` should be reviewed at crates/codegraph-cli/src/wiki.rs:385 | nodes: n4272, n4271; edges: #4285 |
| `warning` | `rationale_risk_comment` | HACK comment `HACK: we borrow the sensitive-value treatment from the decoder.` should be reviewed at crates/codegraph-indexer/src/tests.rs:4045 | nodes: n6970, n6969; edges: #6992 |
| `warning` | `rationale_risk_comment` | HACK comment `HACK: we shell out because the library has no batch mode` should be reviewed at crates/codegraph-indexer/src/tests.rs:4047 | nodes: n6972, n6969; edges: #6994 |
| `info` | `ambiguous_call_resolution` | Call `Arc::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: n10061, n10607, n18288; edges: #35933, #37190 |
| `info` | `ambiguous_call_resolution` | Call `Args::parse` has 3 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/cards.rs:parse,crates/codegraph-cli/src/mcp.rs:parse,crates/codegraph-parser/src/language.rs:parse | nodes: n10061, n11327, n18286; edges: #35929, #38073 |
| `info` | `ambiguous_call_resolution` | Call `AtomicU64::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: n6969, n8029, n10022, n10061, n10511, n10607, n10856, n17666; edges: #30601, #33232, #35927, #35951, #36623, #37198, #37746 |
| `info` | `ambiguous_call_resolution` | Call `AtomicUsize::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: n1099, n1381, n3119, n3167, n3316, n3430, n3953, n4087, n4215, n4271, n15394; edges: #17628, #18133, #23453, #23680, #23849, #23999, #24668, #24861, +2 more |
| `info` | `ambiguous_call_resolution` | Call `BTreeMap::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: n172, n173, n184, n185, n314, n374, n393, n399, n404, n405, n406, n449, n455, n457, n474, n478, n480, n486, n490, n492, n493, n498, n1260, n1285, n1286, n1287, n1288, n1290, n1291, n1295, n1398, n1471, n1491, n1578, n1606, n1609, n1621, n1622, n2071, n2072, n2074, n2077, n2079, n2081, n2084, n2087, n2092, n2093, n2263, n2267, n2269, n2276, n2550, n2555, n2566, n2570, n2646, n2661, n2662, n2673, n2674, n2695, n2696, n2724, n2739, n2809, n2811, n2812, n2813, n2817, n2818, n3194, n3880, n3889, n3973, n4284, n4501, n4502, n4504, n4506, n4509, n4512, n4561, n4563, n4980, n5085, n5087, n5088, n5128, n5150, n5168, n5695, n5698, n5706, n5711, n5713, n5714, n5715, n5716, n5717, n5718, n5719, n5720, n5721, n5722, n5724, n5752, n5760, n5982, n6003, n6081, n6082, n6083, n6084, n6087, n6088, n6089, n6090, n6091, n6092, n6093, n6094, n6095, n6096, n6097, n6098, n6099, n6107, n6602, n6608, n6609, n6610, n6753, n6754, n6755, n6756, n8087, n8090, n8091, n8162, n8596, n8755, n8768, n8771, n8772, n8774, n8777, n10061, n10595, n10596, n10603, n10604, n10605, n10606, n10607, n10612, n10914, n10921, n14802; edges: #15111, #15115, #15211, #15234, #15369, #15442, #15589, #15634, +150 more |
| `info` | `ambiguous_call_resolution` | Call `BTreeSet::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: n175, n374, n399, n461, n462, n474, n486, n488, n498, n1298, n1398, n1472, n1473, n1474, n1475, n1476, n1478, n1479, n1480, n1482, n1483, n1486, n1489, n1490, n1593, n2070, n2081, n2092, n2267, n2269, n2448, n2554, n2660, n3189, n3190, n4513, n4814, n5585, n5699, n5704, n6602, n6604, n6610, n6611, n6612, n6613, n6763, n8086, n8087, n8092, n8097, n8103, n8147, n8613, n8762, n8763, n8770, n8772, n8775, n10898, n10912, n10914, n10915, n10917, n10919, n10921, n10927, n14807; edges: #15123, #15441, #15652, #16402, #16420, #16572, #16712, #16778, +59 more |
| `info` | `ambiguous_call_resolution` | Call `BufReader::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: n8098, n8200, n8201, n17783; edges: #32688, #33299, #33307 |
| `info` | `ambiguous_call_resolution` | Call `Cli::parse` has 3 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/cards.rs:parse,crates/codegraph-cli/src/mcp.rs:parse,crates/codegraph-parser/src/language.rs:parse | nodes: n3566, n16326; edges: #24067 |
| `info` | `ambiguous_call_resolution` | Call `CodeGraph::new` has 2 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-indexer/src/scan.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: n6602, n8204, n8213, n8214, n8231, n8232, n8233, n10951, n10955, n10956, n10957, n10958, n10959, n10960, n17492; edges: #29804, #33324, #33419, #33441, #33515, #33523, #33531, #37765, +6 more |
| `info` | `ambiguous_call_resolution` | Call `CollectedFacts::default` has 6 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-indexer/src/options.rs:default,crates/codegraph-lsp/src/lib.rs:default | nodes: n8747, n17992; edges: #33833 |
| `info` | `ambiguous_call_resolution` | Call `Command::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: n8097, n8183, n8194, n11329, n17768; edges: #32655, #33205, #33233, #38108 |
| `info` | `ambiguous_call_resolution` | Call `Cursor::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: n8200, n8201, n17918; edges: #33300, #33308 |
| `info` | `ambiguous_call_resolution` | Call `CustomRules::default` has 6 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-indexer/src/options.rs:default,crates/codegraph-lsp/src/lib.rs:default | nodes: n6004, n17192; edges: #28643 |
| `info` | `ambiguous_call_resolution` | Call `DefinitionScope::default` has 6 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-indexer/src/options.rs:default,crates/codegraph-lsp/src/lib.rs:default | nodes: n8747, n17994; edges: #33837 |

Insights are truncated: showing 50 of 10181.

## Suggested Questions
- What startup flow is reachable from cargo bin:codegraph-cli?
- Why is main a central graph hotspot?
- What responsibilities and external dependencies does the crates/codegraph-analysis community have?
- What evidence explains the architecture link from crates/codegraph-analysis to crates/codegraph-core?
- Why is the references edge from github workflow:CI/test to crates/codegraph-web/static/label-policy.test.js surprising?
- Which code paths are involved in dependency_cycle findings?
