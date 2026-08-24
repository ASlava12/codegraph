# Surface Parity (Phase 9 Verification)

Verification that every analysis reachable from one surface is reachable
from the other two — or is documented here as intentionally
surface-specific. Inventories regenerated on 2026-07-10 after the thirteen
`FEATURE_AUDIT.md` follow-ups landed: 64 CLI commands, 62 API routes, 45
web-called endpoints, 14 MCP tools.

## Analyses available on all applicable surfaces

| Analysis | CLI | API | Web | MCP |
| --- | --- | --- | --- | --- |
| Graph scan/summary/coverage | `scan`, `summary`, `coverage` | `/api/scan`, `/api/summary`, `/api/coverage` | overview (report snapshot) | `report` |
| Query language + facets | `query` | `/api/query` | Query panel + presets | `query_graph` |
| Natural-language ask | `ask` | `/api/ask` | Ask flow | `ask` |
| Node cards / neighbors | `node-card`, `trace` | `/api/node-card`, `/api/node-context` | selection cards | `get_node_card`, `get_neighbors` |
| Traces (deps/config/errors/entrypoints) | `trace*` commands | `/api/trace*`, `/api/dependents`, `/api/entrypoint-traces` | trace panels | `query_graph` slices |
| Workflows / flow view | `workflow*` commands | `/api/workflow*`, `/api/entrypoint-workflows` | Flow view | `workflow` |
| Journeys | `journey` | `/api/journey` | Journey panel | `shortest_path` |
| Impact / seams / contracts / refactor bundle | `impact`, `seams`, `component-*`, `refactor-context` | matching endpoints | Refactoring panel | `impact`, `refactor_context` |
| Insights + quality gate | `insights`, `check` | `/api/insights`, `/api/check` | Insights panel, gate chip | `insights` |
| Reports (knowledge/architecture/hotspots/communities) | `report`, `architecture`, `hotspots`, `communities`, `surprising-links`, `language-dependencies` | matching endpoints | overview (one report snapshot) | `report` |
| PR impact dashboard | `pr-impact` | `/api/pr-impact` | PR Impact panel | via `report`/`query_graph` |
| Source search / preview | `source-search` | `/api/source-search`, `/api/source` | Source Search panel, previews | `source_search` |
| Exports (JSON/DOT/NDJSON/GraphML/Mermaid/Cypher/FalkorDB) | `scan` + format flags | `/api/export` | Export panel | n/a (artifact writing) |
| Incremental cache workflow | `cache-*`, `incremental-*` | matching endpoints | Cache Diff panel | n/a |
| Semantic enrichment | `semantic-*` | `/api/semantic-*` + jobs | semantic work queue | n/a |
| Investigation memory | `memory-*` | — (see below) | — (see below) | `memory_save/list/reflect` |

## Intentionally surface-specific

**CLI-only.** These operate on the local filesystem, long-running local
processes, or multi-file artifacts, and gain nothing from HTTP transport:
`benchmark` and `bench-context` (local performance measurement),
`registry-*` (global local-repository registry), `merge` (combines local
graph artifacts), `export-wiki` (writes a multi-file Obsidian vault),
`watch` (long-running refresh loop), `install-hooks`/`hook-run` (git hook
integration), `install-agent` (writes local guidance files), `query-log`
(reads the local audit file), `mcp` (the stdio transport itself), and the
granular `semantic-run`/`semantic-patch`/`semantic-apply` steps (the API
covers the same flow with `/api/semantic-enrich` jobs).

**API-only.** Server-runtime and UI-backing endpoints with no CLI
equivalent by design: `/api/graph` paging and `/api/focus` (canvas
slices; the CLI expresses the same through `query`), `/api/node-context`
(superseded by node cards, kept for compatibility), scan/semantic job
stores and SSE, `/api/health`, `/api/live`, `/api/ready`, `/api/metrics`,
`/api/capabilities`, `/api/schema`, `/api/projects`, and
`/api/scan-options` (runtime discovery; the CLI equivalent is `--help`
plus `coverage`).

**Web report snapshot.** The web overview intentionally does not call
`/api/architecture`, `/api/communities`, `/api/hotspots`,
`/api/surprising-links`, `/api/language-dependencies`, `/api/summary`,
`/api/coverage`, or `/api/entrypoints` individually — it renders all of
them from one `/api/report` snapshot to avoid duplicate scans (a checked
Phase 4 item). The endpoints remain for agents and integrations.

**MCP.** The 14 tools cover agent workflows: ad-hoc analyses without a
dedicated tool (seams, component contracts, PR impact, exports) are
reachable through `query_graph`, `report`, and `refactor_context` (which
embeds component dependencies), keeping the tool list small enough for
assistants to reason about. Investigation memory is exposed over MCP and
the CLI but deliberately not over plain REST or the web UI: records are
written by agents during investigations, stamped with the local
fingerprint, and reviewed via `memory-list`/`memory-reflect` or the
`memory_*` tools.

## Verification method

Inventories were regenerated from source (CLI `--help`, server route
table, `app.js` fetch calls, MCP `tools/list`) and cross-checked against
the audit matrix in `FEATURE_AUDIT.md`. The previously unintentional gaps
— refactoring reports without web views (F5), the CLI-only PR impact
dashboard (F6), and the missing MCP tools (F7) — were closed by their
audit follow-ups; everything remaining in the lists above is intentional
and recorded here.

## Agreement between surfaces (2026-08-24)

Reachability is one half; the other is whether two surfaces answering the
same question say the same thing. Three checks, each repeatable:

**The browser against the CLI.** The web view recomputes eight findings on
the graph it holds rather than asking for them again. Driving the bundle
over a real scanned graph and diffing the per-kind counts against
`codegraph insights` found three disagreements: the view's Python
standard-library list had 41 names against the CLI's 193, it never learned
that a project does not depend on itself, and it saw only half the
ambiguity rule. Repeat it with `loadBundle` from `smoke-harness.mjs` and
`buildClientInsights(graph)`; the fixtures in `views-smoke.mjs` pin each
case, and `check-defs.mjs` now fails when the three lists the bundle
copies from the analysis crate drift (the Python standard-library set
lives in `codegraph-core`, which the indexer reads too).

**The schema against the responses.** Every documented endpoint carries an
example; calling each one and comparing the response with its
`response_fields` found seven examples that did not run at all, four
responses that described none of their fields, and three published
defaults that were not the ones the handlers applied. A test in
`codegraph-server` serializes five responses and fails on a field
described and not returned, or returned and described nowhere, and pins
one published default against the limit the handler reports applying.

**One scan path per surface.** A handler that scans on its own skips the
automatic semantic pass its neighbours run, and then answers from a
different graph: `/api/node-card`, `/api/scan`, `/api/report` and the CLI
`report` all did. `scan_graph_with_cache` and `scan_with_cache_status` are
the single paths, returning the cache status those callers reached past
them for. Scan jobs stay syntactic on purpose — cancellable, with the
semantic jobs beside them — and the schema says so at both ends.
