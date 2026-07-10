# Feature Audit (Phase 9)

End-to-end audit of shipped features across CLI, API, web, and MCP, run
against this repository itself. Every gap found here is filed as a new
unchecked item in its phase in `../ROADMAP.md`; this document records the
method, the evidence, and where each finding was filed.

Audit date: 2026-07-10. Surface inventory at audit time: 67 CLI commands,
60 API endpoints, 13 web panels plus graph/flow canvases, 8 MCP tools.

## Method

- Ran all 53 non-interactive CLI commands against this repository and
  recorded exit status and output size (interactive/serving commands —
  `watch`, `mcp`, `install-hooks`, `install-agent`, registry commands — were
  exercised separately or covered by workspace tests).
- Started `codegraph-server` and hit all 60 API endpoints with realistic
  parameters, verifying status codes and payload shapes against
  `/api/schema`.
- Compared the CLI, API, web (`app.js` fetch calls), and MCP (`tools/list`)
  surfaces for parity.
- Inspected the largest payloads and slowest requests for usability and
  self-description quality.

Result: every command and endpoint works; no shipped feature is broken
outright. All findings below are gaps in parity, output quality, noise
calibration, or consistency — filed as follow-up roadmap items.

## Findings

### F1. Report JSON embeds the full uncapped insight list (filed: Phase 4)

`report --format json` on this repository emits 12.3 MB, of which 6.1 MB is
`quality_gate.report.insights` — the complete unbounded insight list —
while the top-level `insights` section is correctly capped (17 KB). The
report artifact is ~2× the size of the entire codebase (3.2 MB), and
`bench-context` measures `project_overview` savings at **−98.9%**: reading
the whole repository is cheaper than reading the report. Gate evaluation
should stay limit-independent, but the gate payload only needs counts,
severity breakdowns, and a capped sample.

### F2. Unresolved calls create one placeholder node per call site (filed: Phase 1)

Unresolved call targets are materialized as `external_dependency` nodes
without label deduplication and without filtering language builtins. On
this repository: 27,162 of 38,333 nodes are `external_dependency`, including
1,680 separate nodes labeled `assert`, 1,182 `assert_eq`, 777 `Some`, 561
`format`, 292 `Ok`. This inflates every downstream surface (graph paging,
hotspots, exports, node counts) and misclassifies language builtins as
external dependencies.

### F3. `unresolved_call` insight flood buries all other findings (filed: Phase 3)

The same placeholder explosion produces 26,869 `unresolved_call` warnings
(of 31,858 total insights), grading this healthy repository's risk as
**critical** (score 313,603). On syntactic-only scans (no LSP enrichment),
unresolved calls are the expected default, not a warning-grade anomaly;
severity should be calibrated by scan depth and deduplicated by label.

### F4. Control-flow facts surface as node kind `unknown` (filed: Phase 3)

Branch/loop/async/return/error-flow facts are stored with
`NodeKind::Unknown` plus an `item_kind` metadata value (8,154 nodes here:
3,924 `branch`, 1,991 `error`, 1,225 `return`, 739 `loop`, 274 `async`).
Every kind facet, summary table, and web filter shows them as `unknown` —
the second-largest category on this repository — which reads as a defect
and hides what they are.

### F5. Refactoring-intelligence reports are not reachable from the web UI (filed: Phase 8)

`impact`, `seams`, `component-dependencies`, `component-contract`, and
`refactor-context` exist in CLI and API but have no web view or node-card
action (journey does). A human exploring in the browser cannot see blast
radius or seams without switching to the terminal.

### F6. PR impact dashboard is CLI-only (filed: Phase 7)

`pr-impact` has no API endpoint and no web view despite being a dashboard
by name and purpose (changed files → communities, hotspots, blast radius,
risk score).

### F7. MCP misses the highest-value agent tools (filed: Phase 7)

MCP exposes 8 tools (`query_graph`, `get_node_card`, `get_neighbors`,
`shortest_path`, `workflow`, `insights`, `impact`, `report`) but not
`refactor_context` (the one-shot agent bundle built for handoff), `ask`,
`source_search`, or the memory commands (`memory-save`/`list`/`reflect`).
Milestone 3's exit criterion — investigation outcomes survive between
sessions without shelling out to the CLI — is not reachable over MCP alone.

### F8. Node id formats are inconsistent across surfaces (filed: Phase 9)

Most commands accept `n42`-style ids ("label or node id, for example n42"),
and query results, web deep links, and examples print them. But CLI
`node-card --node-id`, API `/api/node-card`, and `/api/node-context` accept
only bare numeric ids and reject `n42` with a parse error.

### F9. API parameter naming and error contract inconsistencies (filed: Phase 9)

`path` means "project root" on 59 endpoints but "file inside root" on
`/api/source` (which uses `root` for the project root). Query-string
deserialization failures return plain text (`Failed to deserialize query
string: ...`) instead of the documented structured JSON error with
`request_id`, so agent clients get two error shapes from one API.

### F10. Incremental cache commands print the full graph to stdout (filed: Phase 9)

`incremental-update` (31 MB here) and `incremental-merge-preview` (29 MB)
embed the entire merged graph JSON in their output with no summary/compact
mode, making them unusable in hooks, logs, and agent pipelines without
external filtering. `hook-run` already demonstrates the compact alternative
(136 bytes).

### F11. `/api/refactor-context` takes minutes on a warm cache (filed: Phase 8)

A single warm-cache request for `target=main` took 2m59s on this
repository while sibling endpoints (`impact`, `seams`, `journey`) return in
seconds. The CLI path is similarly heavy. The bundle recomputes expensive
sections rather than reusing cached report/insight results, and its journey
search is unbounded by default.

### F12. Benchmark recall oracle counts fixture strings as expected keys (filed: Phase 7)

`bench-context`'s environment-read oracle greps raw text for patterns like
`env::var("`, which matches code embedded inside string literals of test
fixtures. On this repository it reports 40% env recall (missing
`DATABASE_URL`, `NODE_ENV`, `PORT`) even though extraction is correct —
those keys exist only inside fixture strings that no code reads.

### F13. `ask` misroutes environment-variable questions (filed: Phase 3)

"Where is CODEGRAPH_API_TOKEN read?" — the documented question shape —
maps to rule `route_or_endpoint` and generates
`routes handler:CODEGRAPH_API_TOKEN` (0 results), while
`trace-config CODEGRAPH_API_TOKEN` finds the reader immediately. The
config/environment rule should win for SCREAMING_SNAKE tokens combined
with read/set verbs.

## Non-findings worth recording

- All 53 audited CLI commands and all 60 API endpoints completed
  successfully with valid JSON (after correcting audit-side parameters to
  the documented contracts).
- Error messages for missing nodes are good: structured JSON with
  `request_id` on the API, clear one-line errors on the CLI.
- The hotspot report correctly classifies label-collision hubs (582
  incoming `calls` edges on one `new`) as `utility`, not architectural.
- Environment detection handles both `std::env::var` and bare `env::var`,
  `process.env.X`, indexed and fallback forms.
- `memory-save`/`memory-list`/`memory-reflect`, `query-log`, `export-wiki`,
  `pr-impact`, and `hook-run` all behave as documented with compact
  outputs.
