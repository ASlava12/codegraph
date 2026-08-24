# CodeGraph Project Report

- Root: `.`
- Generated at unix: `1787554788`
- Graph schema version: `1`
- Quality gate: **passed** (`fail_on=error`, failing_insights=0)
- Risk: **low** (score 30, total 10923, errors 0, warnings 3, infos 10920)

## Summary
| Metric | Count |
| --- | ---: |
| Nodes | 20214 |
| Edges | 44707 |
| Entrypoints | 83 |
| Skipped files | 0 |

### Languages
| Language | Count |
| --- | ---: |
| `rust` | 15921 |
| `javascript` | 4011 |
| `markdown` | 147 |
| `sql` | 15 |

### Node Kinds
| Kind | Count |
| --- | ---: |
| `control_flow` | 10804 |
| `external_dependency` | 5470 |
| `function` | 2983 |
| `type` | 462 |
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
| `heuristic` | 36393 |
| `syntactic` | 7806 |
| `exact` | 508 |

## Compact Node Summaries
| Score | Node | Kind | Roles | In | Out | Risks | Edge kinds | Source |
| ---: | --- | --- | --- | ---: | ---: | --- | --- | --- |
| 4679 | `cg-545f575ded3c6cf1` `main` | `function` | entrypoint, risk, error_flow, hub | 4 | 328 | `info`=103, `warning`=1 | `calls`=125, `references`=110, `may_error`=95, `contains`=1, `entrypoint`=1 | `crates/codegraph-cli/src/main.rs:100-944` |
| 2666 | `cg-3a37cd9e02a71cf3` `project_report_markdown` | `function` | risk, error_flow, hub | 6 | 147 | `info`=79 | `may_error`=76, `calls`=42, `references`=34, `contains`=1 | `crates/codegraph-analysis/src/report.rs:98-545` |
| 2022 | `cg-3ecc0d8f2c161ea6` `from` | `function` | risk, hub | 224 | 1 | `info`=1 | `calls`=222, `references`=2, `contains`=1 | `crates/codegraph-cli/src/main.rs:947-953` |
| 1954 | `cg-d2950b3f329d2bf5` `new` | `function` | risk, hub | 214 | 3 | `info`=1 | `calls`=214, `contains`=1, `references`=1 | `crates/codegraph-core/src/lib.rs:461-475` |
| 1931 | `cg-ac9d30a355c78e5a` `index_file` | `function` | risk, error_flow, hub | 4 | 190 | `info`=7 | `calls`=139, `references`=53, `contains`=1, `may_error`=1 | `crates/codegraph-indexer/src/scan.rs:475-1040` |
| 1848 | `cg-7e3964d36c4506c3` `insights` | `function` | risk, hub | 118 | 66 | `info`=12 | `calls`=172, `references`=11, `contains`=1 | `crates/codegraph-analysis/src/insights.rs:13-91` |
| 1660 | `cg-d810f145303d349d` `add_node` | `function` | risk, hub | 182 | 2 | `info`=1 | `calls`=182, `contains`=1, `references`=1 | `crates/codegraph-core/src/lib.rs:477-479` |
| 1569 | `cg-f332f153115e540a` `add_node_with_metadata` | `function` | risk, hub | 170 | 4 | `info`=1 | `calls`=171, `references`=2, `contains`=1 | `crates/codegraph-core/src/lib.rs:490-506` |
| 1561 | `cg-30c87037268b3d30` `add_edge` | `function` | risk, hub | 171 | 2 | `info`=1 | `calls`=171, `contains`=1, `references`=1 | `crates/codegraph-core/src/lib.rs:508-516` |
| 1495 | `cg-be330dd92396b19a` `escapeHtml` | `function` | risk, hub | 163 | 3 | `info`=1 | `calls`=163, `references`=2, `contains`=1 | `crates/codegraph-web/static/js/16-flow.js:1174-1181` |
| 1452 | `cg-66a839d2982b5fa0` `t` | `function` | hub | 160 | 2 | - | `calls`=160, `contains`=1, `references`=1 | `crates/codegraph-web/static/js/05-locale.js:5-7` |
| 1432 | `cg-148240c31c28c061` `default` | `function` | risk, hub | 155 | 2 | `info`=2 | `calls`=156, `contains`=1 | `crates/codegraph-indexer/src/options.rs:98-107` |
| 1299 | `cg-c323b107fd5dc54c` `resolve_pending_calls` | `function` | risk, hub | 3 | 132 | `info`=3 | `calls`=93, `references`=41, `contains`=1 | `crates/codegraph-indexer/src/resolve.rs:1082-1651` |
| 1242 | `cg-b1c0c03e9f80fd44` `scan_project_with_scope` | `function` | risk, error_flow, hub | 4 | 106 | `info`=10 | `calls`=90, `references`=15, `may_error`=4, `contains`=1 | `crates/codegraph-indexer/src/scan.rs:96-287` |
| 1217 | `cg-c79b578b239bb0da` `scan_project` | `function` | risk, hub | 131 | 3 | `info`=2 | `calls`=131, `references`=2, `contains`=1 | `crates/codegraph-indexer/src/scan.rs:57-62` |
| 1140 | `cg-5ddd7ab359fb7be3` `pr_impact` | `function` | risk, hub | 7 | 107 | `info`=5 | `calls`=84, `references`=29, `contains`=1 | `crates/codegraph-analysis/src/pr_impact.rs:157-375` |
| 1079 | `cg-7231b9729e092e62` `export_wiki` | `function` | risk, error_flow, hub | 5 | 104 | `info`=5 | `calls`=73, `references`=33, `may_error`=2, `contains`=1 | `crates/codegraph-cli/src/wiki.rs:66-324` |
| 1053 | `cg-d511b273b15e893a` `impact_with_insights_mode` | `function` | risk, error_flow, hub | 4 | 97 | `info`=5 | `calls`=77, `references`=21, `may_error`=2, `contains`=1 | `crates/codegraph-analysis/src/refactoring.rs:636-809` |
| 1049 | `cg-cdb567578c39a099` `index_compose_entrypoints` | `function` | risk, hub | 3 | 107 | `info`=2 | `calls`=75, `references`=34, `contains`=1 | `crates/codegraph-indexer/src/runtime.rs:163-478` |
| 1037 | `cg-ab9b87e102457216` `parse_source` | `function` | risk, error_flow, hub | 71 | 28 | `info`=10 | `calls`=91, `references`=4, `may_error`=3, `contains`=1 | `crates/codegraph-parser/src/extract.rs:12-59` |

Node summaries are truncated: showing 25 of 4431 important nodes.

## Compact File Summaries
| Score | File | Symbols | Trace | Imports | Config | Env | Errors | Unresolved | Risks | Trace kinds |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| 53448 | `cg-ee3c92e03a9056b1` `crates/codegraph-analysis/src/insights.rs` | 145 | 2175 | 3 | 0 | 0 | 11 | 1345 | `info`=385 | `calls`=1789, `references`=375, `may_error`=11 |
| 51598 | `cg-fe63b53f56a7f7e9` `crates/codegraph-analysis/src/query.rs` | 157 | 2306 | 3 | 0 | 0 | 58 | 1206 | `info`=247 | `calls`=1855, `references`=393, `may_error`=58 |
| 35294 | `cg-30025cf8bc99e019` `crates/codegraph-indexer/src/manifests.rs` | 112 | 1543 | 8 | 2 | 0 | 28 | 838 | `info`=196 | `calls`=1162, `references`=351, `may_error`=28, `reads_config`=2 |
| 34424 | `cg-dceca1acc0b42f99` `crates/codegraph-lsp/src/lib.rs` | 183 | 1415 | 21 | 0 | 1 | 87 | 663 | `info`=433 | `calls`=1094, `references`=233, `may_error`=87, `reads_environment`=1 |
| 33139 | `cg-87f7489dde024ef9` `crates/codegraph-indexer/src/runtime.rs` | 99 | 1485 | 4 | 0 | 0 | 9 | 724 | `info`=313 | `calls`=1070, `references`=406, `may_error`=9 |
| 31948 | `cg-ce015f1d29bf7bf1` `crates/codegraph-storage/src/lib.rs` | 103 | 1202 | 13 | 0 | 0 | 205 | 496 | `info`=538 | `calls`=858, `may_error`=205, `references`=139 |
| 22719 | `cg-754972209269a8a9` `crates/codegraph-indexer/src/resolve.rs` | 77 | 973 | 4 | 0 | 0 | 5 | 532 | `info`=186 | `calls`=753, `references`=215, `may_error`=5 |
| 20260 | `cg-6dae84628d387775` `crates/codegraph-parser/src/extract.rs` | 63 | 948 | 5 | 0 | 0 | 29 | 405 | `info`=178 | `calls`=610, `references`=309, `may_error`=29 |
| 20055 | `cg-9137df38151e2bd3` `crates/codegraph-server/src/analysis_handlers.rs` | 47 | 920 | 10 | 0 | 0 | 109 | 337 | `info`=169 | `calls`=613, `references`=198, `may_error`=109 |
| 17083 | `cg-90f7d906380e7031` `crates/codegraph-analysis/src/refactoring.rs` | 28 | 689 | 3 | 0 | 0 | 18 | 379 | `info`=211 | `calls`=539, `references`=132, `may_error`=18 |
| 16948 | `cg-57bea67c8e67d61d` `crates/codegraph-indexer/src/imports.rs` | 47 | 728 | 3 | 0 | 0 | 49 | 356 | `info`=133 | `calls`=513, `references`=166, `may_error`=49 |
| 16385 | `cg-c2b9c8bce2026ed5` `crates/codegraph-indexer/src/sql.rs` | 54 | 713 | 5 | 0 | 0 | 26 | 354 | `info`=138 | `calls`=503, `references`=184, `may_error`=26 |
| 13049 | `cg-31b5fb33171f9669` `crates/codegraph-analysis/src/report.rs` | 31 | 478 | 4 | 0 | 0 | 83 | 231 | `info`=183 | `calls`=316, `may_error`=83, `references`=79 |
| 12700 | `cg-7eebb03d80185684` `crates/codegraph-web/static/js/12-filters.js` | 52 | 647 | 0 | 0 | 0 | 3 | 228 | `info`=131, `warning`=1 | `calls`=392, `references`=252, `may_error`=3 |
| 11298 | `cg-772dad414bd11949` `crates/codegraph-indexer/src/scan.rs` | 24 | 517 | 8 | 0 | 0 | 8 | 238 | `info`=108 | `calls`=397, `references`=112, `may_error`=8 |
| 11027 | `cg-41fb10ec0e71f4a9` `crates/codegraph-indexer/src/frameworks.rs` | 33 | 473 | 2 | 0 | 0 | 18 | 256 | `info`=65 | `calls`=357, `references`=98, `may_error`=18 |
| 10620 | `cg-d60d3ba74cd4a2a8` `crates/codegraph-web/static/js/09-investigate.js` | 48 | 637 | 0 | 0 | 0 | 9 | 147 | `info`=85, `warning`=1 | `calls`=410, `references`=218, `may_error`=9 |
| 10619 | `cg-bbcb12cdb710d95f` `crates/codegraph-cli/src/main.rs` | 38 | 466 | 19 | 0 | 0 | 106 | 101 | `info`=157, `warning`=1 | `calls`=229, `references`=131, `may_error`=106 |
| 10555 | `cg-7e64b542059a39c6` `crates/codegraph-web/static/js/16-flow.js` | 61 | 647 | 0 | 0 | 0 | 0 | 143 | `info`=92 | `references`=327, `calls`=320 |
| 10543 | `cg-257aa6327619678d` `crates/codegraph-analysis/src/overview.rs` | 16 | 398 | 3 | 0 | 0 | 2 | 257 | `info`=130 | `calls`=317, `references`=79, `may_error`=2 |

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
| 331 | `cg-545f575ded3c6cf1` `main` | `function` | `architectural` | 3 | 328 | `calls`=125, `references`=110, `may_error`=95, `entrypoint`=1 |
| 193 | `cg-ac9d30a355c78e5a` `index_file` | `function` | `architectural` | 3 | 190 | `calls`=139, `references`=53, `may_error`=1 |
| 183 | `cg-d810f145303d349d` `add_node` | `function` | `architectural` | 181 | 2 | `calls`=182, `references`=1 |
| 183 | `cg-7e3964d36c4506c3` `insights` | `function` | `architectural` | 117 | 66 | `calls`=172, `references`=11 |
| 173 | `cg-f332f153115e540a` `add_node_with_metadata` | `function` | `architectural` | 169 | 4 | `calls`=171, `references`=2 |
| 172 | `cg-30c87037268b3d30` `add_edge` | `function` | `architectural` | 170 | 2 | `calls`=171, `references`=1 |
| 165 | `cg-be330dd92396b19a` `escapeHtml` | `function` | `architectural` | 162 | 3 | `calls`=163, `references`=2 |
| 152 | `cg-3a37cd9e02a71cf3` `project_report_markdown` | `function` | `architectural` | 5 | 147 | `may_error`=76, `calls`=42, `references`=34 |
| 136 | `cg-ff8d959bb9aca1b0` `crates/codegraph-web/static/js/04-dom.js` | `file` | `architectural` | 1 | 135 | `calls`=94, `references`=42 |
| 134 | `cg-c323b107fd5dc54c` `resolve_pending_calls` | `function` | `architectural` | 2 | 132 | `calls`=93, `references`=41 |
| 133 | `cg-c79b578b239bb0da` `scan_project` | `function` | `architectural` | 130 | 3 | `calls`=131, `references`=2 |
| 113 | `cg-5ddd7ab359fb7be3` `pr_impact` | `function` | `architectural` | 6 | 107 | `calls`=84, `references`=29 |
| 111 | `cg-24d4b563129daa8f` `kubernetes_document_from_lines` | `function` | `architectural` | 2 | 109 | `references`=56, `calls`=53, `may_error`=2 |
| 109 | `cg-b1c0c03e9f80fd44` `scan_project_with_scope` | `function` | `architectural` | 3 | 106 | `calls`=90, `references`=15, `may_error`=4 |
| 109 | `cg-cdb567578c39a099` `index_compose_entrypoints` | `function` | `architectural` | 2 | 107 | `calls`=75, `references`=34 |

Hotspots are truncated: showing 25 key hubs out of 3212 candidates (3180 architectural, 32 utility).

## Communities
| Community | Nodes | Files | Entrypoints | Internal edges | External edges | Languages | Evidence |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| `crates/codegraph-analysis` | 980 | 22 | 0 | 2384 | 1142 | `rust`=979 | #89, #90, #91, #92, #93, +95 more |
| `crates/codegraph-indexer` | 818 | 18 | 0 | 2237 | 192 | `rust`=802, `sql`=15 | #4611, #4612, #4613, #4614, #4615, +95 more |
| `crates/codegraph-web` | 609 | 25 | 0 | 2068 | 19 | `javascript`=607 | #12254, #12255, #12256, #12257, #12258, +95 more |
| `crates/codegraph-server` | 490 | 13 | 73 | 1054 | 127 | `rust`=488 | #10015, #10021, #10028, #10043, #10044, +95 more |
| `crates/codegraph-cli` | 264 | 16 | 2 | 494 | 385 | `rust`=261 | #3300, #3302, #3314, #3315, #3316, +95 more |
| `crates/codegraph-parser` | 228 | 8 | 0 | 540 | 35 | `rust`=227 | #9125, #9126, #9127, #9128, #9129, +95 more |
| `crates/codegraph-lsp` | 185 | 2 | 0 | 388 | 63 | `rust`=184 | #8575, #8576, #8577, #8578, #8579, +95 more |
| `crates/codegraph-storage` | 105 | 2 | 0 | 282 | 87 | `rust`=104 | #11692, #11693, #11694, #11695, #11696, +95 more |
| `docs` | 104 | 7 | 0 | 108 | 84 | `markdown`=104 | #15485, #15486, #15487, #15488, #15489, +95 more |
| `root` | 50 | 9 | 2 | 46 | 112 | `markdown`=43 | #23, #24, #25, #26, #27, +95 more |
| `crates/codegraph-core` | 28 | 2 | 0 | 39 | 865 | `rust`=27 | #4553, #4554, #4555, #4556, #4557, +95 more |
| `crates/codegraph-ui` | 16 | 2 | 1 | 26 | 4 | `rust`=14 | #12147, #12164, #12165, #12166, #12168, +25 more |

Communities are truncated: showing 25 of 47 communities.

## Entrypoints
| Node | Kind | Source |
| --- | --- | --- |
| `cg-250feb8ef22845f2` `cargo bin:codegraph-cli` | `entrypoint` | - |
| `cg-5ca5aa0067eabcc1` `cargo binary:codegraph` | `entrypoint` | - |
| `cg-f98de3b398a5ac65` `cargo bin:codegraph-server` | `entrypoint` | - |
| `cg-50f40d112d58dbaa` `cargo bin:codegraph-ui` | `entrypoint` | - |
| `cg-1bd5704207c9a5fc` `main` | `function` | `crates/codegraph-server/build.rs:11-43` |
| `cg-545f575ded3c6cf1` `main` | `function` | `crates/codegraph-cli/src/main.rs:100-944` |
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
| `crates/codegraph-analysis` | `crates/codegraph-core` | 763 | `calls`=750, `references`=13 | `heuristic`=551, `syntactic`=212 | #15784, #16023, #16209, #16239, #16258, +95 more |
| `crates/codegraph-analysis` | `crates/codegraph-cli` | 168 | `calls`=168 | `heuristic`=168 | #15764, #15792, #15998, #16043, #16054, +95 more |
| `crates/codegraph-server` | `crates/codegraph-analysis` | 57 | `calls`=57 | `heuristic`=56, `syntactic`=1 | #37303, #37304, #37305, #37306, #37307, +52 more |
| `crates/codegraph-cli` | `crates/codegraph-analysis` | 54 | `calls`=54 | `heuristic`=53, `syntactic`=1 | #24834, #24835, #24846, #25011, #25012, +49 more |
| `docs` | `crates/codegraph-analysis` | 49 | `references`=49 | `heuristic`=49 | #44635, #44636, #44639, #44640, #44641, +44 more |
| `crates/codegraph-indexer` | `crates/codegraph-core` | 47 | `calls`=47 | `heuristic`=47 | #26485, #26504, #26522, #26548, #26573, +42 more |
| `.` | `crates/codegraph-analysis` | 36 | `references`=36 | `heuristic`=31, `exact`=5 | #44485, #44486, #44487, #44497, #44499, +31 more |
| `crates/codegraph-indexer` | `crates/codegraph-cli` | 30 | `calls`=30 | `heuristic`=30 | #26681, #27765, #27826, #29190, #29232, +25 more |
| `crates/codegraph-storage` | `crates/codegraph-indexer` | 30 | `calls`=30 | `syntactic`=20, `heuristic`=10 | #39504, #39514, #39598, #39601, #39626, +25 more |
| `crates/codegraph-cli` | `crates/codegraph-core` | 24 | `calls`=24 | `heuristic`=17, `syntactic`=7 | #25464, #25465, #25498, #25499, #25550, +19 more |
| `crates/codegraph-cli` | `crates/codegraph-indexer` | 23 | `calls`=23 | `heuristic`=14, `syntactic`=9 | #24874, #24875, #24891, #24892, #24905, +18 more |
| `crates/codegraph-cli` | `crates/codegraph-storage` | 22 | `calls`=22 | `heuristic`=15, `syntactic`=7 | #25002, #25098, #25265, #25267, #25268, +17 more |
| `crates/codegraph-server` | `crates/codegraph-lsp` | 21 | `calls`=21 | `heuristic`=17, `syntactic`=4 | #37818, #37946, #37948, #37950, #37952, +16 more |
| `.` | `crates/codegraph-web` | 17 | `references`=17 | `heuristic`=12, `exact`=5 | #44500, #44502, #44503, #44504, #44522, +12 more |
| `crates/codegraph-server` | `crates/codegraph-cli` | 16 | `calls`=16 | `heuristic`=16 | #37143, #37819, #37849, #37873, #37944, +11 more |

## Surprising Links
| Score | Source | Target | Areas | Languages | Edge | Confidence | Reasons | Evidence |
| ---: | --- | --- | --- | --- | --- | --- | --- | --- |
| 12 | `cg-d6bec304462d89a8` `github workflow:CI/test` | `cg-897089bf45e84db8` `crates/codegraph-web/static/label-policy.test.js` | `.github` -> `crates/codegraph-web` | `unknown` -> `javascript` | `references` | `heuristic` | cross_area, rare_crossing, heuristic_confidence, entrypoint_boundary | #44150 |
| 11 | `cg-353f3b935dde75dd` `fmt` | `cg-28b01d82c62bfb4e` `write_str` | `crates/codegraph-analysis` -> `crates/codegraph-storage` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #18547 |
| 11 | `cg-c0dde696ecf6000c` `fmt` | `cg-28b01d82c62bfb4e` `write_str` | `crates/codegraph-lsp` -> `crates/codegraph-storage` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #34385 |
| 11 | `cg-b578766ea50f99ad` `fmt` | `cg-28b01d82c62bfb4e` `write_str` | `crates/codegraph-parser` -> `crates/codegraph-storage` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #36437 |
| 11 | `cg-90a41d6c54ef87cd` `default_cache_dir` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-storage` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #39702 |
| 11 | `cg-59dabd4c488f60c3` `write_bool` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-storage` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #39998 |
| 11 | `cg-bf24cebefd4b0cb2` `write_bytes` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-storage` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #40007 |
| 11 | `cg-d6ca5a4e89e1b014` `run_window` | `cg-9ca49e4fe97e74c0` `build` | `crates/codegraph-ui` -> `crates/codegraph-analysis` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #40375 |
| 11 | `cg-d6ca5a4e89e1b014` `run_window` | `cg-67e7950d46906aa0` `run` | `crates/codegraph-ui` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, rare_crossing, heuristic_confidence | #40384 |
| 8 | `cg-76b9bc25b964a1a6` `query_compacted_node` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15764 |
| 8 | `cg-d68259ea66397360` `node_context` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15792 |
| 8 | `cg-e609fff75caf8925` `query_edges` | `cg-f02ccf0470c5ab68` `remove` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #15998 |
| 8 | `cg-7252c7417575fe2a` `export_dot` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #16043 |
| 8 | `cg-8f000e4f121cac44` `export_graphml` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #16054 |
| 8 | `cg-df3f3fb961222800` `export_cypher` | `cg-3ecc0d8f2c161ea6` `from` | `crates/codegraph-analysis` -> `crates/codegraph-cli` | `rust` -> `rust` | `calls` | `heuristic` | cross_area, heuristic_confidence | #16090 |

Surprising links are truncated: showing 200 of 36799 candidates.

## Risks And Insights
| Kind | Severity | Count |
| --- | --- | ---: |
| `dependency_cycle` | `warning` | 2 |
| `low_entrypoint_coverage` | `warning` | 1 |
| `unresolved_call` | `info` | 4615 |
| `potential_error_flow` | `info` | 2973 |
| `unreachable_error_flow` | `info` | 2111 |
| `orphan_function` | `info` | 877 |
| `cross_language_heuristic_edge` | `info` | 139 |
| `ambiguous_call_resolution` | `info` | 100 |
| `duplicate_function_label` | `info` | 55 |
| `unreachable_source_file` | `info` | 36 |

### Insight Evidence
| Severity | Kind | Message | Evidence |
| --- | --- | --- | --- |
| `warning` | `dependency_cycle` | Directed dependency cycle across files involving `loadGraphPage` -> `loadProjectOverview` -> `renderOverview` -> `renderArchitecture` | nodes: cg-684e3eae463bcec3, cg-f2b231b5ea64b822, cg-e10a11ecfd95dc05, cg-55acfa324cc9a811; edges: #41062, #41085, #41184, #41197, #41342 |
| `warning` | `dependency_cycle` | Directed dependency cycle across files involving `selectNodeById` -> `clearSelection` -> `loadInsights` -> `initializeGraph` -> `runGraphQuery` -> ... | nodes: cg-63bda3507d6149c3, cg-731f9fdc3f1f4046, cg-b58b9813b85c7dfe, cg-d3e8b6605ed317cd, cg-024299113695827e, cg-97d38f02647de38b, cg-ff6d6acc66195fc0, cg-c626e3998d6d79e9, cg-d860927c1e59932a, cg-ea90455ab9f7f4c2, cg-a5b073f2c92d7ca3, cg-419f1b05576ff745, cg-9cb19a2927075004, cg-e738564263ff8bd6, cg-3752d29a74f2dfab, cg-dac8c1e80dd4c7d5, cg-00f6a8b425370b02, cg-3a11f8adba3b325e, cg-28448807c1c1b920, cg-5092ca9a7ecd9e06, cg-913083101fac8bbb, cg-f1f6e3784028fcbc, cg-6aafb28ada2a6217, cg-f41e3d7f0a754e80, cg-9924c655c426b56b, cg-9e5303d867405549, cg-1580f7127eb123e6, cg-1a772a06712a7c9f, cg-ff4b01fd82072f5a, cg-dce7f8716d2748b7, cg-2de988f6eed8777c, cg-b54c22a78e3b7812, cg-88c4263e82ca94fd, cg-f9cf7253d2cb6a11, cg-292ece7dd955c7bb, cg-cdadeae6b11704cd, cg-cad372864ac80c74, cg-759a44bad0166a2e, cg-fdb90773a0799eea, cg-faa07277bd833053, cg-368ec03504d9bf56, cg-6930d7b0f8c8ffca; edges: #40854, #40857, #41694, #41695, #41748, #41759, #41760, #41765, +77 more |
| `warning` | `low_entrypoint_coverage` | entrypoints reach 1451 of 2983 functions (48%), and 24% of calls resolve to a scanned function — the rest name a dependency, the standard library, or a method the syntax cannot type; counting the 149 exported functions as starting points reaches 53% — treat `unreachable_*` findings as gaps in call resolution, or as a library reached through its API, before reading them as dead code | cg-d6bec304462d89a8, cg-b0c3dc19d1e7efc7, cg-430851bec33a61db, cg-250feb8ef22845f2, cg-5ca5aa0067eabcc1, cg-545f575ded3c6cf1, cg-f98de3b398a5ac65, cg-1bd5704207c9a5fc |
| `info` | `ambiguous_call_resolution` | Call `Arc::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-4f3131af78a6c532, cg-86557cb65cc74bf5, cg-415016e31e7531ca; edges: #38027, #39449 |
| `info` | `ambiguous_call_resolution` | Call `Args::parse` has 3 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/cards.rs:parse,crates/codegraph-cli/src/mcp.rs:parse,crates/codegraph-parser/src/language.rs:parse | nodes: cg-4f3131af78a6c532, cg-034812dcf5c5d389, cg-2b1e2ad5624b23c8; edges: #38023, #40355 |
| `info` | `ambiguous_call_resolution` | Call `AtomicU64::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-4d337951626f13c5, cg-dceca1acc0b42f99, cg-16d0862a35fc3799, cg-4f3131af78a6c532, cg-f80f321293580a81, cg-86557cb65cc74bf5, cg-ce015f1d29bf7bf1, cg-8ef7808a8eedb832; edges: #32178, #35167, #38021, #38045, #38728, #39457, #40009 |
| `info` | `ambiguous_call_resolution` | Call `AtomicUsize::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d8a25e6b6eb07831, cg-9cc152340550cf9f, cg-d9e51f6be3b21183, cg-09c6d583a4c95539, cg-675f711cfe3c6481, cg-504d24ec4dae8bf9, cg-fa999f05c4ccb68d, cg-b1d494a4a47bc21f, cg-9e1deb0debde49f1, cg-16921d49ea151475, cg-232250fad41ecfc8; edges: #18462, #18976, #24630, #24857, #25026, #25176, #25863, #26056, +2 more |
| `info` | `ambiguous_call_resolution` | Call `BTreeMap::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-05fc8d21be606901, cg-e4c5e2ffe79f98a1, cg-974e24db1d7c020f, cg-01b81d13078ad763, cg-53ab9b3f7bad79fb, cg-c09a400a1f6d72aa, cg-7e3964d36c4506c3, cg-5c959ee8e5feaa99, cg-eee43e9d2e88eaf4, cg-2fdd7e8f1691c8b3, cg-b50a509944cdd3ce, cg-41f54a2821014a58, cg-ef2026648b6f3896, cg-0e20e70153105ffb, cg-3862e34397bed91f, cg-4f1891c5b52c3f34, cg-ce9921d0dc04f49c, cg-2d730cb571f4f16a, cg-ca6a08cd5b67f654, cg-02db150916a6b615, cg-ca7d9552ca5c8be2, cg-457c7fef4e90120e, cg-8058cf17aca33edc, cg-7cc73ae5e6a91ab1, cg-f33563548e5fa22a, cg-c253f3c6f288050a, cg-17ed7a98c9addcd6, cg-5fc17909eee4ec2d, cg-3641d8e645d60bc2, cg-23610d92dbd17fb7, cg-5ddd7ab359fb7be3, cg-4a576f3692b5f215, cg-134f1ea7a738f0bd, cg-c53e3cdfb8f487fc, cg-a9144a1316867846, cg-966721a6e65dfd52, cg-ba5d11541227474f, cg-a54b4312f4ff6907, cg-b73fcd28508d78d0, cg-bf04066f2b2a283c, cg-48a6db618c93b3f3, cg-07c503359e1dca34, cg-a5ba4daecc094ac6, cg-d511b273b15e893a, cg-f62fdaf5ac53da88, cg-97fedb701147723c, cg-9bd61c6eced39a51, cg-58ada47a3c417cb0, cg-c3b7ea4eefb2a897, cg-28a0220a6526bcba, cg-9ab74a21cf21b45f, cg-8c543f02534e9830, cg-9ca49e4fe97e74c0, cg-cb86a3f3ad766fbd, cg-613b6f21eb9b177b, cg-02cd3a8f708ec27f, cg-78c834979f0bb690, cg-7c7c25b079655dc5, cg-96cb20a949b4de68, cg-b16f9a03daf60b73, cg-381a7789c8c74043, cg-e7619356e970ee58, cg-219ebccf6378c615, cg-57a82c05a3a399d0, cg-641a862f778196b4, cg-369ecf1e6c4fbbc4, cg-e3483a5bf17cad7b, cg-4c1b656b963d638d, cg-0512498dbb889d89, cg-30f8fdee94f5a9e0, cg-6c9bd1a3f0eae8b4, cg-5a2df730e2fd0336, cg-a82e28db304d064b, cg-4f24f38280ace263, cg-87f9691ffcc71a30, cg-7231b9729e092e62, cg-68cf4bcec2fa42be, cg-dbb60c8df27fd51b, cg-883938e7e72bbd33, cg-9e00c9fd48fd3e66, cg-d0441dbbd41bf9e1, cg-aae1705db58b135c, cg-35e1253f22025866, cg-6c1d641ef33ba7cd, cg-40c3c9853f56cba7, cg-8d4a6a93429e9d78, cg-8bd44d85cfbcf848, cg-31135cdb260c1d05, cg-e31f14c943c97e90, cg-4082d0b56f291071, cg-28e4ea9de6435ce2, cg-7ccce39311355df4, cg-c323b107fd5dc54c, cg-5a9aceda85031a0c, cg-6f41e399339eb56f, cg-a2cdfffc97697c73, cg-3499b1625aac1d09, cg-81d616cdf9db5d25, cg-6e13ae39c60fdda9, cg-e3d509b01ea5289e, cg-44efab9cd87e3034, cg-ddf3bd476cffb603, cg-e6c0c49a463a3985, cg-359d3d4b827622b6, cg-e8b714e8aee3dd8f, cg-d61e77ffd6163dc4, cg-a4e2496fb36df481, cg-2cafe5c90c33d602, cg-10a25556e045b16d, cg-c60a3a8f43142d41, cg-527a0e274eccf377, cg-f1f4caab909649d3, cg-02479a01dd6244b1, cg-cdb567578c39a099, cg-e510e945031f7c40, cg-b5af9ae75787ad7c, cg-1a27e9d4d17ff931, cg-ca167f8c6280e72e, cg-7e9fd13adb100509, cg-82ef2bd9dfb4f3e5, cg-1317ce56e1aa4f0d, cg-cca91192d469d389, cg-307cad2bc7e067f2, cg-db4d2e368b350aa9, cg-a04548f57528737b, cg-fba4d5a661e53e1b, cg-eef82e0b1624cecd, cg-777368437d4f25c1, cg-24d4b563129daa8f, cg-b1c0c03e9f80fd44, cg-6c436c928df45ddf, cg-d021720523f743df, cg-ac9d30a355c78e5a, cg-1057fda6fbf8872e, cg-f92969dd41ce8611, cg-5bca781e1ced44ee, cg-d8d3b005044da83d, cg-69af59277189a6d2, cg-b188a01528a35449, cg-7793b12d1ed8a08b, cg-9845255f488c227d, cg-2c4eca0299b7c2a7, cg-1fdf709090d27876, cg-8cfaed1d453f116a, cg-49d43056df95c62c, cg-d15235dab34a6ad3, cg-3062d44e7b9fd184, cg-807382bb61254178, cg-4f3131af78a6c532, cg-8840f4a5795fa74f, cg-4dfbbc6d170de4ae, cg-7c7eb73b8f42d87e, cg-9b7ac3848a35db9f, cg-d2a8207c3627ac0e, cg-9b19fffea754c372, cg-86557cb65cc74bf5, cg-9ac781cfe61d8469, cg-e1806ec715857159, cg-23137e02ebfe4a79, cg-b114f96fbcb423bf; edges: #15844, #15848, #15944, #15967, #16102, #16175, #16322, #16367, +151 more |
| `info` | `ambiguous_call_resolution` | Call `BTreeSet::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-98d5e44c151d6bba, cg-c09a400a1f6d72aa, cg-5c959ee8e5feaa99, cg-19f83dc8af3c944d, cg-83987d0780a69bf7, cg-3862e34397bed91f, cg-2d730cb571f4f16a, cg-36cdf34dfa33c52e, cg-457c7fef4e90120e, cg-16bf4ba26dd0226f, cg-5ddd7ab359fb7be3, cg-8f328fcffbc68721, cg-29a9e73dd5e473df, cg-aea4362a2e0d2a3d, cg-5b3ae3dfcd9f4ce4, cg-2d89225ecdf3013e, cg-ca926f8768023cf0, cg-e14b6ca071175c43, cg-18b55790cd3b984d, cg-571c75da0e82975d, cg-b48e6417ab8c8c55, cg-9187dbfd812e8b66, cg-8cfa18e6cc8c1878, cg-5ffd3bb4465fd3b7, cg-2c551e90e38bee68, cg-b54f14c42f14abb4, cg-d511b273b15e893a, cg-9bd61c6eced39a51, cg-28a0220a6526bcba, cg-9ab74a21cf21b45f, cg-bd5ada189f1a6e34, cg-6663be0b924ca8dc, cg-db93baa9d3d1ea4f, cg-b68c85528a3c6108, cg-9a80ef0fb9c3b3cf, cg-abe0053cd3eb9d8b, cg-c60bcbf4d7761479, cg-148240c31c28c061, cg-b2a23392fe710052, cg-5a9aceda85031a0c, cg-b1c0c03e9f80fd44, cg-1fe861bfda241ce2, cg-ac9d30a355c78e5a, cg-b9ff4fbff2c6cfbc, cg-b610b1e09abf651b, cg-dcd0997f0e80304a, cg-94a40b9462432e19, cg-81c22c51efd6f50e, cg-9e752e7ac344df9a, cg-69af59277189a6d2, cg-912abb8ee08feac2, cg-6925ffd68c6928dc, cg-af3548762c07307f, cg-20c7521953d5f658, cg-7d9a874a87a5a983, cg-854f2d5eaffd0269, cg-092d22ec02604922, cg-f145b4f4b7c2ad29, cg-d15235dab34a6ad3, cg-e5505aa7b6cfb774, cg-a60d3a8bd04213ff, cg-5f6ca1879ec842fa, cg-e1806ec715857159, cg-8b72aa81e08145d2, cg-e586cfed452ebc9b, cg-8f635593f5408cac, cg-23137e02ebfe4a79, cg-79e31488852b527a, cg-ed5d3ed6e3e41671; edges: #15856, #16174, #16389, #17191, #17209, #17365, #17522, #17590, +60 more |
| `info` | `ambiguous_call_resolution` | Call `BufReader::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: cg-6bca22d4a64b4ae6, cg-0b20b28f283bad1c, cg-55fdf0b6a5e1804e, cg-ff79682a2c35f230; edges: #34623, #35234, #35242 |
| `info` | `ambiguous_call_resolution` | Call `Cli::parse` has 3 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/cards.rs:parse,crates/codegraph-cli/src/mcp.rs:parse,crates/codegraph-parser/src/language.rs:parse | nodes: cg-545f575ded3c6cf1, cg-42a5942d596b6270; edges: #25244 |
| `info` | `ambiguous_call_resolution` | Call `CodeGraph::new` has 2 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-indexer/src/scan.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-b1c0c03e9f80fd44, cg-71eb59d3e229da53, cg-9aaf7317cbea8c3c, cg-96a0ff292b3a060f, cg-972ef45379de2270, cg-0eba1324ec3fa849, cg-c2c49f33e8ee249d, cg-10ae5cd44675bdcd, cg-2b63f29c54326d40, cg-cae74fa5c85ac004, cg-7883b1d8b52afae1, cg-50c7baa60b0a1d39, cg-cac9cc6a421efee5, cg-78d0307272a963ed, cg-8d9fe48bde5e3e1d; edges: #31318, #35259, #35354, #35376, #35450, #35458, #35466, #40047, +6 more |
| `info` | `ambiguous_call_resolution` | Call `CollectedFacts::default` has 6 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-indexer/src/options.rs:default,crates/codegraph-lsp/src/lib.rs:default | nodes: cg-ab9b87e102457216, cg-bd240e5398fa155f; edges: #35782 |
| `info` | `ambiguous_call_resolution` | Call `Command::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: cg-6925ffd68c6928dc, cg-ad813e449534e799, cg-1f713c9eec5391fc, cg-c14e065c9ed6bbd5, cg-4db1c84674b76a35; edges: #34590, #35140, #35168, #40390 |
| `info` | `ambiguous_call_resolution` | Call `Cursor::new` has 4 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new,crates/codegraph-lsp/src/lib.rs:new | nodes: cg-0b20b28f283bad1c, cg-55fdf0b6a5e1804e, cg-019819c41c64ad1c; edges: #35235, #35243 |
| `info` | `ambiguous_call_resolution` | Call `CustomRules::default` has 6 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-indexer/src/options.rs:default,crates/codegraph-lsp/src/lib.rs:default | nodes: cg-8191b92ffe130cb6, cg-e1c2d65f370ab1a7; edges: #30124 |
| `info` | `ambiguous_call_resolution` | Call `DefinitionScope::default` has 6 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-indexer/src/options.rs:default,crates/codegraph-lsp/src/lib.rs:default | nodes: cg-ab9b87e102457216, cg-41d5c91e79091088; edges: #35786 |
| `info` | `ambiguous_call_resolution` | Call `EventLoop::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d6ca5a4e89e1b014, cg-6ea3c6a5995d056b; edges: #40373 |
| `info` | `ambiguous_call_resolution` | Call `FileNodeSummary::default` has 6 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-analysis/src/limits.rs:default,crates/codegraph-indexer/src/options.rs:default,crates/codegraph-lsp/src/lib.rs:default | nodes: cg-98d5e44c151d6bba, cg-58a8f857694ef3dc; edges: #15855 |
| `info` | `ambiguous_call_resolution` | Call `Glob::new` has 13 same-language candidates and was kept as one bounded ambiguity; sample: crates/codegraph-analysis/src/model.rs:new,crates/codegraph-analysis/src/model.rs:new,crates/codegraph-cli/src/mcp.rs:new,crates/codegraph-core/src/lib.rs:new,crates/codegraph-indexer/src/scan.rs:new | nodes: cg-d215e7cc8e49c357, cg-9b0afe81db28e9a7, cg-7d024d8d9f0bd954, cg-94fc2b79229062e2; edges: #21920, #29133, #29143 |

Insights are truncated: showing 50 of 10923.

## Suggested Questions
- What startup flow is reachable from cargo bin:codegraph-cli?
- Why is main a central graph hotspot?
- What responsibilities and external dependencies does the crates/codegraph-analysis community have?
- What evidence explains the architecture link from crates/codegraph-analysis to crates/codegraph-core?
- Why is the references edge from github workflow:CI/test to crates/codegraph-web/static/label-policy.test.js surprising?
- Which code paths are involved in dependency_cycle findings?
