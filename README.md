# CodeGraph

CodeGraph is a Rust-based code exploration tool for people and agents.

The project goal is to turn source code into a typed knowledge graph that can be inspected from a graphical UI, queried from a CLI, and consumed as structured JSON by automation.

## Current Status

This repository is an active production-oriented prototype.

Implemented now:

- Rust workspace layout.
- Core graph model.
- Stable graph confidence taxonomy for `exact`, `semantic`, `syntactic`, `heuristic`, and `unknown` facts with JSON round-trip coverage.
- Built-in language adapter registry for Rust, Python, JavaScript, TypeScript/TSX, Go, C, C++, PHP, and Bash parser support.
- LSP server discovery for semantic enrichment readiness across Rust, Go, JavaScript/TypeScript, Python, C/C++, PHP, and Bash.
- Semantic enrichment server contracts cover `rust-analyzer`, `gopls`, and `typescript-language-server --stdio` for primary Rust, Go, JavaScript, TypeScript, and TSX workflows.
- Project semantic readiness reports showing which scanned languages are covered by installed LSP servers.
- Semantic enrichment plans showing ready, blocked, and unsupported LSP work by language, including capped concrete work queues with stable ids and priorities for agents.
- Semantic execution batch reports that group filtered LSP work by language server command and include executable LSP request descriptors for semantic runners.
- CLI semantic runner that executes ready LSP batches over stdio and emits reusable response JSON for graph patching or enrichment.
- HTTP/API and web semantic enrichment action that can run ready LSP batches and render an enriched graph in the browser.
- Async semantic enrichment job API with status, SSE events, and result retrieval for long-running LSP work.
- Bounded in-memory scan and semantic job retention with job-store counters in the health API.
- Configurable scan and semantic job concurrency limits with active/available counters in the health API.
- Semantic LSP response patch reports that map definitions, references, and diagnostics back onto graph nodes.
- Semantic graph patch application that emits enriched graphs with semantic edges and diagnostic nodes.
- Filesystem scanner with default build/vendor ignore rules.
- Tree-sitter based syntax extraction for Rust, Python, JavaScript, TypeScript, TSX, Go, C, C++, PHP, and Bash.
- Function, type/class, module/namespace, import/include, and entrypoint candidate nodes.
- Manifest-defined entrypoints from Cargo, npm, Go, Python, setup.py, Composer, and CMake project metadata.
- Shebang-defined script entrypoints for Bash, Python, Node.js, and PHP scripts, including extensionless CLI files.
- Framework route entrypoints for common Python, JavaScript/TypeScript, Rust, Go, and PHP web route declarations.
- Rust/Axum route entrypoints handle multiline `.route(...)` calls and ignore string literal route markers.
- Resolved manifest entrypoint targets for common file paths, command paths, CMake executables, and Python module callables.
- Approximate `calls` edges between functions when syntax-level names can be resolved.
- Local import/include resolution for relative JavaScript/TypeScript imports and CommonJS requires, Python relative/absolute project imports, Go module-local imports, quoted C/C++ includes with CMake and compile database include directories, PHP include/require paths, Bash source paths, and common Rust module paths.
- Manifest dependency extraction from Cargo, npm, Go, Python, setup.py, Composer, vcpkg, and Conan projects.
- Heuristic config reads, environment reads, and potential error/exception constructs.
- CLI command that emits graph JSON.
- HTTP API and embedded web UI for interactive graph exploration.
- API capabilities endpoint for discovering supported languages, exports, features, limits, cache state, and route groups.
- API capabilities limits include graph page, node-card, focus, query, report, source preview, and source-search ceilings for production clients.
- API capabilities publish the effective maximum JSON API request body size for POST clients.
- API capabilities publish insight/check result limits and API schema parameters link overview/report limits to matching capability keys.
- API capabilities publish bounded project report snapshot limits for architecture groups, architecture edges, language links, hotspots, and returned insights.
- API and web UI enforce the published maximum graph query expression length before running repository scans.
- API and web UI enforce the published maximum source-search text length before scanning source files.
- Machine-readable API schema endpoint for agents and integrations.
- API schema parameters expose structured minimum, maximum, maximum string length, and matching capability-limit keys where runtime bounds exist.
- API schema POST endpoints expose structured body fields for scan jobs and semantic enrichment requests.
- API schema POST endpoints link to the published maximum API request body size enforced by the server.
- API schema system endpoints expose structured response fields for probes, health, and runtime metrics.
- API schema publishes common response headers for request correlation, latency diagnostics, cache policy, security policy, and static-asset ETags.
- API schema graph investigation endpoints expose structured response fields for graph slices, node cards, queries, edge explanations, and reports.
- API schema analysis and source endpoints expose structured response fields for topology, traces, insights, checks, source previews, and source search.
- Semantic work queues clamp and publish maximum work item limits across CLI, API, schema, and web capabilities.
- Semantic LSP request timeouts clamp and publish runtime limits across CLI, API, and web capabilities.
- API schema enum values document supported graph query commands and query terms, including exact edge index lookups.
- API schema enum values document graph node kinds, edge kinds, insight kinds, and confidence levels used by graph filters.
- API schema enum values document project report sections and risk grades used by report consumers.
- API schema enum values document web deep-link parameters used for shareable node, dependency, and query investigations.
- API schema enum values stay aligned with semantic work statuses and capabilities used by LSP work queues.
- Runtime metrics endpoint for uptime, API/schema versions, roots, language/feature counts, cache state, job stores, and concurrency.
- Lightweight liveness and readiness probe endpoints for deployment health checks.
- Server package version is published through runtime probes, health, metrics, capabilities, and API schema responses.
- Multi-stage Docker image definition for running the web/API server with a mounted repository and persistent cache volume.
- Built-in HTTP access logs with method, target, status, and latency for server operations.
- Per-response `x-request-id` correlation headers mirrored in access logs and JSON error bodies.
- Per-response `x-response-time-ms` timing headers for browser, proxy, and agent-side latency diagnostics.
- Graceful HTTP server shutdown on Ctrl-C and SIGTERM.
- Server-wide security headers for the embedded web UI and API responses.
- Server cache-control headers keep runtime/API responses uncacheable and force browser revalidation for unversioned embedded web assets.
- Embedded web asset ETags support conditional `304 Not Modified` responses during browser revalidation.
- Optional API bearer-token protection through `--api-token` or `CODEGRAPH_API_TOKEN`, with same-origin web UI token prompting.
- Project report snapshots in CLI, API, and web export for summary, full-risk scoring, quality gate, insights, topology reports, cache, and scan coverage.
- Web overview chips for server package version, server capabilities, API/schema versions, cache state, supported language/export counts, job limits, and route groups.
- Web overview surfaces the API schema common response-header contract for agent/client diagnostics.
- Web overview risk summary chips for report quality gate, grade, weighted score, severity counts, and top finding kinds with quick insight filtering plus one-click quality checks.
- Web overview hotspot, annotation, and entrypoint empty/focus states are localized for English and Russian UI sessions.
- Web overview scan policy, coverage, LSP, semantic work, architecture, and language-dependency diagnostics use localized status and focus text.
- Web overview reuses the project report snapshot for summary, coverage, topology, hotspots, and risks to avoid duplicate heavy scans.
- Web runtime panel for uptime, cache state, scan/semantic slots, and retained job-store totals.
- Web runtime panel surfaces the last API response latency from `x-response-time-ms` for quick slow-endpoint diagnosis.
- Async scan job API for long-running repository scans.
- SSE scan job status stream for live web progress updates.
- Cancelable scan and semantic enrichment jobs for stopping queued or running long work.
- Job listing APIs for inspecting retained scan and semantic job history by status.
- Web job monitor for retained scan and semantic jobs with refresh, status summaries, and cancellation actions.
- Source preview API and UI panel for parsed symbols plus framework route/config facts with source spans.
- File node cards include source previews and can jump into focused file graph slices.
- Node cards return suggested focused graph actions for files, symbols, packages, configs, and error facts.
- Node cards include dependency summaries with incoming/outgoing counts, edge kinds, confidence, and neighbor facets.
- File node cards include contained-symbol and in-file trace fact summaries for quick file-level triage.
- File node cards surface risks from contained symbols and facts, not only risks attached directly to the file node.
- Node cards include risk summaries by severity and insight kind alongside capped related risk lists.
- Enriched selected-node cards with summary metadata, source snippets, neighboring dependencies, trace actions, and related risks.
- Graph, query, focus, and node-card edges include stable `metadata.edge_index` values for exact dependency explanation and UI edge selection.
- Web canvas edges can be selected directly to open dependency cards with source, target, confidence, metadata, and provenance explanation actions.
- Web canvas edges highlight on hover so dependency paths are easier to inspect before opening a card.
- Edge explanations include related risk summaries and capped edge-scoped findings for dependency-level triage.
- Dependency cards can be opened from graph edges, query results, traces, and node neighbor lists.
- Dependency cards can focus or query their exact `edge_index` for fast canvas narrowing and agent handoff.
- Web node and dependency-card selections are reflected in shareable `node` and `edge` URL parameters with copy-link actions for exact human/agent handoff.
- Web node and dependency cards can be downloaded as JSON with source, dependency, and risk context for portable agent handoff.
- Web query presets include ambiguous calls, ambiguous entrypoints, dependency-scope/version/runtime-import/test-only issues, and sensitive defaults for fast logical inconsistency triage.
- Selected external dependency cards can open focused package graph slices that connect declarations and import sites.
- Initial English/Russian web UI localization with a persistent language selector.
- Static web landmarks and pagination controls expose localized ARIA labels for English/Russian accessibility.
- Web quality-check and source-search workflows use localized status, result, empty-state, and export summary text.
- Web path-query workflow uses localized validation, loading, error fallback, and result labels in English/Russian UI sessions.
- Web graph-query and path-query workflows enforce the published query-length limit before issuing API requests.
- Web path-query results can be downloaded as JSON with endpoints, query expression, counts, and returned graph slice for agent handoff.
- Web graph export workflow uses localized progress, error fallback, and node/edge count labels in English/Russian sessions.
- Web source-search match cards use localized titles and loading states for English/Russian UI sessions.
- Web entrypoint trace workflow uses localized status, counters, empty states, truncation notes, export summary text, and focused graph titles.
- Web config/error trace workflows use localized status, counters, empty states, truncation notes, and focused graph titles.
- Off-by-default graph labels with collision-aware, sparse Auto/Focus modes so node cards stay readable without captions covering the graph.
- Hover labels render only for the node under the pointer, selected-node details stay in the side card, and saved label modes reset when label-density rules change.
- Web graph viewport HUD for visible node/edge counts, zoom, and layout state during canvas exploration.
- Web graph edges connected to the hovered or selected node are softly highlighted for immediate local dependency context.
- Web graph selected/hovered-node neighborhoods emphasize adjacent nodes and dim unrelated graph noise while preserving the full canvas.
- Web graph legend node-kind chips can toggle canvas filters directly while staying synchronized with the sidebar kind filters.
- Keyboard-accessible graph canvas navigation for panning, zooming, fitting, resetting, and pausing layout.
- Dependency-free web label policy tests guard caption density, saved-mode resets, and interaction label behavior.
- Web API error messages include request ids when available so UI failures can be correlated with server access logs.
- Interactive UI trace panel for following outgoing dependency subgraphs from a selected node.
- Reverse dependency/dependent traces for impact analysis from CLI, API, query language, and web detail panels.
- Entrypoint trace API, CLI command, and web panel for comparing startup flows from manifest/code entrypoints.
- Web entrypoint trace reports can be downloaded as JSON with search, depth, and returned startup flows.
- Config trace API, CLI command, and web panel for finding config/environment readers and entrypoint paths.
- Web config trace reports can be downloaded as JSON with target, depth, matched readers, and dependency paths.
- Error trace API, CLI command, and web panel for following potential error/exception paths back to entrypoints.
- Web error trace reports can be downloaded as JSON with target, depth, source nodes, and exception-flow paths.
- Agent-friendly summary, entrypoint, and trace commands/endpoints.
- Agent-friendly graph query command and API for focused node, edge, call, dependency, trace, diagnostic, insight/risk, and unreachable-code slices.
- Agent-friendly annotation graph queries for focused user-owned metadata slices from `.codegraph/annotations.toml`.
- Focused query responses include returned counts and facets for node kinds, edge kinds, languages, item kinds, and confidence.
- Agent-friendly symbol graph queries for focused function/type/module context with containing files and nearby dependency edges.
- Agent-friendly file graph queries for focused source-file structure, imports, and contained-symbol dependency context.
- Agent-friendly entrypoint graph queries for focused startup slices with immediate trace edges.
- Agent-friendly route graph queries for focused HTTP/framework route and handler slices.
- Agent-friendly config graph queries for focused configuration/environment reader slices and entrypoint paths.
- Agent-friendly error graph queries for focused exception/error source slices and entrypoint paths.
- Agent-friendly cycle graph queries for focused circular dependency slices.
- Agent-friendly hotspot graph queries for focused high-degree dependency slices.
- Agent-friendly source search command, API, and web panel for compact matching snippets.
- Web source-search matches can open the matching file's graph slice directly for dependency exploration.
- Web source-search results can be downloaded as JSON with query, path filter, matches, and context snippets.
- Edge explanation command, API, and web controls for confidence/provenance evidence.
- Path queries for finding directed dependency paths between labels or node ids.
- Confidence-aware edge queries and UI edge labels for fact provenance.
- Server-side graph paging and filtering endpoint for large repository exploration.
- Web graph page controls backed by server-side paging, search, kind, item, language, edge, confidence, relation, and source filters.
- Web graph page controls can page dense edge slices independently with the `/api/graph` `edge_offset` contract.
- Web graph and insight filter inputs use API schema enum suggestions for node kinds, edge kinds, confidence levels, insight severities, and insight kinds.
- Web graph viewport controls for zooming, fitting visible nodes, restarting layout, and pausing layout simulation.
- Web graph page and viewport HUD show loaded-vs-total slice status so large-repository canvases are clearly marked as partial views.
- Web graph minimap shows graph position and supports click/drag recentering during large-canvas exploration.
- Web risk legend entries can filter the graph to nodes with matching insight severity.
- Web project overview for language mix, edge confidence/source/relation mix, and entrypoint launch points.
- Language dependency matrix reports in CLI, API, and web overview for mixed-language coupling.
- Architecture map reports in CLI, API, and web overview for top-level project areas and cross-area dependencies.
- Web semantic work queue with filters for reviewing prioritized LSP enrichment tasks and focusing their graph evidence.
- Web semantic overview counters for definitions, diagnostics, document symbols, references, workspace symbols, and queued work.
- Architecture overview chips can focus the paged graph by project area path prefix.
- Architecture dependency chips can focus the exact graph edges behind cross-area coupling.
- Hotspot reports in CLI, API, and web overview for high-degree files, functions, entrypoints, and config nodes.
- Web path navigation for finding, focusing, and visually highlighting dependency paths between graph nodes.
- Node context API and detail-panel neighbor loading for paged graph exploration.
- Server-backed web insights for project-wide findings while browsing paged graph slices.
- Insight reports include severity and kind breakdowns for triage.
- Web insight severity breakdown chips can apply and clear triage filters directly.
- Server-side insight filters for severity, kind, search, and capped agent/UI reads.
- Web insight findings can be exported as JSON with active filters and severity/kind counts for review or agent handoff.
- CI/agent check command, API, and web quality gate for failing on insight severity thresholds.
- Web quality-gate check results can be downloaded as JSON for CI handoff and review records.
- Insight focus API and web interaction for turning findings into focused graph views.
- Web query panel for running focused graph queries, narrowing the canvas to query results, and jumping to matching nodes.
- Web query panel supports shareable `query` deep-links and copy-link actions for reusable investigations.
- Web query panel keeps a local recent-query history so repeated investigations can be rerun quickly.
- Web query results can be downloaded as JSON with query, root, facets, nodes, and edges for agent handoff.
- Web graph page filters, node/edge offsets, and page limits can be copied as shareable deep-links for reproducible large-repository slices.
- Web graph page filters and offsets can be cleared in one action after opening focused or shared large-repository slices.
- Web canvas search, kind, risk, and query-focus filters show active-filter status in the HUD and can be cleared in one action.
- Web export panel for downloading full graph snapshots as JSON, DOT, or NDJSON.
- Web export panel can download the currently visible canvas slice with graph-page, filter, viewport, and layout metadata for compact handoff.
- Full graph exports publish response headers for node count, edge count, and serialized byte size.
- Web project selector backed by an explicit server-side allowlist for opening local repositories.
- DOT/Graphviz and NDJSON export formats for visualization and streaming agent use.
- Persistent server-side graph cache with project fingerprint invalidation.
- Persistent CLI graph cache using the same project fingerprinting and cache records as the server.
- Persistent per-file parser fact cache reused during graph-cache misses.
- Persistent graph impact index in cache records for fast incremental planning of affected nodes and edges.
- Persistent file graph chunk index in cache records for explicit per-file node and edge scopes.
- Cache fingerprint diff diagnostics in CLI, API, and web UI for explaining cache misses by added, removed, and modified files.
- Cache reuse estimates in CLI, API, and web UI for planning incremental scans from unchanged files and bytes.
- Incremental scan planning reports in CLI, API, and web UI with rescan, removed, reusable path sets, and cached impacted graph node/edge ids.
- Changed-scope incremental scan graphs in CLI, API, and web UI for inspecting only files that need rescanning.
- Incremental merge previews that use the persistent impact index to replace cached file scopes with changed-file rescans for fast review before a full scan.
- API schema enum values document cache status, reuse strategy, incremental actions, and merge blocker kinds for agent-safe incremental workflows.
- Web incremental cache diagnostics show localized completeness blockers and safe-update reasons.
- Scan coverage reports in CLI, API, and web overview for indexed files, policy skips, large-file skips, and non-indexed files.
- CLI scan benchmark reports with timing and graph-size metrics for regression tracking.
- Configurable scan file-size budget for CLI/server scans, with skipped large source files kept visible in summaries, insights, and the web stats panel.
- Repository-owned scan policy from `.codegraph/config.toml` for file-size budgets plus ignored names and globs.
- Effective scan policy API and web overview chips for explaining the active file-size, hidden-file, ignored-file, ignored-name, and ignored-glob rules.
- CI checks for formatting, clippy, tests, UI syntax, web label policy, Docker build and container smoke, embedded web assets, CLI scan, server cache, and safe incremental update smoke tests.
- Embedded web asset smoke checks cover shareable node, dependency, and query investigation links.
- Investigation insights for unresolved calls, parse errors, duplicate labels, orphan functions, and error-flow facts.
- Investigation insights for semantic LSP diagnostics, preserving language-server severity, source, code, file location, and the affected source node for node-card triage.
- Investigation insights for duplicate entrypoint labels that can make label-based startup traces ambiguous.
- Investigation insights for manifest entrypoints that resolve to multiple possible files or functions.
- Investigation insights for ambiguous call resolutions where one call label from the same caller points to multiple possible targets.
- Investigation insights for manifest entrypoints whose declared target cannot be resolved to a file or function.
- Investigation insights for entrypoints that have no outgoing code/config/dependency/error flow.
- Investigation insights for framework routes whose named handler cannot be linked to a scanned function.
- Investigation insights for heuristic cross-language dependency edges that deserve semantic review.
- Investigation insights for local imports/includes whose target file cannot be found.
- Investigation insights for config/environment reads that are not reachable from any detected entrypoint.
- Investigation insights for potential error/exception flows whose source is not reachable from any detected entrypoint.
- Investigation insights for non-test source files with code symbols that are not reachable from any detected entrypoint.
- Investigation insights for config/environment keys that are read with conflicting fallback defaults, including common inline Rust, Python, JavaScript/TypeScript, Go, C, C++, PHP, and Bash environment-read patterns.
- Investigation insights for config/environment keys that are read both as required and with fallback defaults.
- Investigation insights for sensitive config/environment keys, credential-like defaults, and placeholder secret fallbacks, without echoing fallback values in reports.
- Dependency consistency insights for external imports/CommonJS requires that are not backed by declared manifest dependencies.
- Dependency consistency insights for runtime manifest dependencies with no matching import.
- Dependency consistency insights for package declarations with conflicting manifest constraints.
- Dependency consistency insights for packages declared across multiple dependency scopes such as runtime and dev/build.
- Dependency consistency insights for production-like source files importing packages declared only in non-runtime scopes.
- Dependency consistency insights for runtime dependencies that are imported only from test-like source files.
- Focused package graph queries that connect manifest declarations, import sites, and source files for mixed-language dependency investigation.
- Focused file graph queries that connect source files to contained symbols, imports, config/environment reads, potential errors, and nearby dependency edges.
- Framework route insights for duplicate HTTP method/path declarations.
- Framework config convention facts for common web/service stacks across mixed-language repositories.
- Repository custom rule insights from `.codegraph/rules.toml`.
- Project-wide web overview facets for user graph annotations from `.codegraph/annotations.toml`.
- Project-wide web overview facets for edge provenance sources and relation metadata.
- Dependency cycle insights for circular calls, imports, references, and manifest dependency edges.

Planned next:

- Better graph layout and richer visual graph affordances.
- Richer graph queries for humans and agents.
- Production hardening for large repositories.

## Usage

Run the initial scanner:

```bash
cargo run -p codegraph-cli -- scan .
```

List built-in language adapters and detection patterns:

```bash
cargo run -p codegraph-cli -- languages
```

Report available semantic language servers:

```bash
cargo run -p codegraph-cli -- lsp
cargo run -p codegraph-cli -- semantic-readiness .
cargo run -p codegraph-cli -- semantic-plan .
cargo run -p codegraph-cli -- semantic-plan . --work-item-limit 25
cargo run -p codegraph-cli -- semantic-plan . --work-status ready --work-capability definitions
cargo run -p codegraph-cli -- semantic-batch . --work-status ready --work-capability definitions
cargo run -p codegraph-cli -- semantic-batch . --work-status ready --work-capability workspace_symbols
cargo run -p codegraph-cli -- semantic-run . --work-status ready --work-capability definitions > responses.json
cargo run -p codegraph-cli -- semantic-patch . --work-status ready --work-capability definitions --responses responses.json
cargo run -p codegraph-cli -- semantic-apply . --work-status ready --work-capability definitions --responses responses.json
```

`semantic-run` requires the matching language server to be installed and startable. `responses.json` for CLI commands is a JSON array of LSP response objects. Semantic LSP responses are cached under the shared cache directory by default; pass `--no-cache` to force a fresh language-server run.

Limit per-file scan reads for very large repositories:

```bash
cargo run -p codegraph-cli -- --max-file-size 1048576 scan .
```

Export for Graphviz or streaming agent use:

```bash
cargo run -p codegraph-cli -- scan . --format dot
cargo run -p codegraph-cli -- scan . --format ndjson
```

Summarize a project:

```bash
cargo run -p codegraph-cli -- summary .
```

Show an architecture map grouped by top-level project area:

```bash
cargo run -p codegraph-cli -- architecture .
```

Show language-to-language dependency links:

```bash
cargo run -p codegraph-cli -- language-dependencies .
```

Find high-degree graph hotspots:

```bash
cargo run -p codegraph-cli -- hotspots .
```

Create a production-oriented project report snapshot:

```bash
cargo run -p codegraph-cli -- report . --fail-on warning --insight-limit 100
```

The report includes a `risk_summary` with total findings, severity counts, a weighted score, a grade, and the top insight kinds. The quality gate is calculated from the full insight set even when the returned insight list is capped with `--insight-limit`.

Explain scan coverage before or after a full graph scan:

```bash
cargo run -p codegraph-cli -- coverage .
```

Benchmark scanner performance:

```bash
cargo run -p codegraph-cli -- benchmark . --runs 5
cargo run -p codegraph-cli -- bench . --runs 5
```

List entrypoint candidates:

```bash
cargo run -p codegraph-cli -- entrypoints .
```

List investigation insights:

```bash
cargo run -p codegraph-cli -- insights .
cargo run -p codegraph-cli -- insights . --severity warning --kind dependency --limit 25
cargo run -p codegraph-cli -- insights . --kind custom_rule
cargo run -p codegraph-cli -- insights . --kind sensitive_config_default
```

Fail CI or agent workflows when findings meet a severity threshold:

```bash
cargo run -p codegraph-cli -- check . --fail-on error
cargo run -p codegraph-cli -- check . --fail-on warning --kind dependency
```

`check` prints a JSON report and exits with code `2` when matching insights are
at or above the configured severity.

Pin repository scan policy with `.codegraph/config.toml`:

```toml
[scan]
max_file_size = 1048576
include_hidden = false
include_ignored = false
extra_ignored_names = ["coverage", "generated"]
extra_ignored_globs = ["fixtures/**", "public/**/*.min.js"]
```

`ignored_names = [...]` replaces the default ignored directory list, while
`extra_ignored_names = [...]` extends it. `ignored_globs = [...]` replaces the
repository path-pattern ignore list, while `extra_ignored_globs = [...]` extends
it. Glob patterns are matched against normalized project-relative paths. CLI/server flags such as
`--include-hidden`, `--include-ignored`, and `--max-file-size` override the
repository config for that run.

Add repository-specific architecture checks with `.codegraph/rules.toml`:

```toml
[[rules.forbidden_dependency]]
id = "no-left-pad"
ecosystem = "npm"
package = "left-pad"
severity = "error"
message = "left-pad is not allowed in production services"

[[rules.required_config]]
id = "needs-database-url"
target = "DATABASE_URL"
severity = "warning"

[[rules.forbidden_edge]]
id = "ui-cannot-call-db"
edge_kind = "calls"
severity = "error"
message = "UI layer must not call database layer directly"

[rules.forbidden_edge.source_metadata]
"annotation.layer" = "ui"

[rules.forbidden_edge.target_metadata]
"annotation.layer" = "database"
```

Custom rules currently support forbidden manifest dependencies, required
config/environment targets, and forbidden graph edges between annotated nodes.
Violations are emitted as normal graph facts and show up in CLI, API, and web
insight reports.

Add user-owned graph metadata with `.codegraph/annotations.toml`:

```toml
[[annotations.node]]
id = "payments-files"
kind = "file"
label = "payments"

[annotations.node.set]
domain = "payments"
owner = "team-payments"
critical = true
```

Node annotations match existing graph nodes by `kind`, `label`, `language`,
`item_kind`, and optional `[annotations.node.metadata]` conditions. Values from
`set` are stored as `annotation.*` metadata and can be queried by humans,
agents, API clients, and the web UI.

Query focused graph slices:

```bash
cargo run -p codegraph-cli -- query 'nodes kind:function label:main' .
cargo run -p codegraph-cli -- query 'edges kind:calls source:main' .
cargo run -p codegraph-cli -- query 'edges edge_index:0' .
cargo run -p codegraph-cli -- query 'edges confidence:heuristic' .
cargo run -p codegraph-cli -- query 'calls(function:main)' .
cargo run -p codegraph-cli -- query 'trace label:main depth:3' .
cargo run -p codegraph-cli -- query 'dependents label:load_config depth:3' .
cargo run -p codegraph-cli -- query 'neighbors label:main direction:out depth:2 edge_kind:calls' .
cargo run -p codegraph-cli -- query 'symbols label:load_config direction:out edge_limit:300' .
cargo run -p codegraph-cli -- query 'files path:src/main.rs direction:out edge_limit:300' .
cargo run -p codegraph-cli -- query 'entrypoints language:rust' .
cargo run -p codegraph-cli -- query 'routes method:GET path:/health depth:3 edge_limit:300' .
cargo run -p codegraph-cli -- query 'packages package:serde ecosystem:cargo edge_limit:300' .
cargo run -p codegraph-cli -- query 'configs target:DATABASE_URL depth:6' .
cargo run -p codegraph-cli -- query 'errors target:panic depth:6' .
cargo run -p codegraph-cli -- query 'cycles edge_kind:calls' .
cargo run -p codegraph-cli -- query 'hotspots language:rust min_score:5 edge_limit:300' .
cargo run -p codegraph-cli -- query 'path from:main to:load_config depth:6' .
cargo run -p codegraph-cli -- query 'unreachable language:rust' .
cargo run -p codegraph-cli -- query 'unreachable kind:function label:legacy_worker' .
cargo run -p codegraph-cli -- query 'unreachable scope:config search:LEGACY_TOKEN' .
cargo run -p codegraph-cli -- query 'unreachable scope:errors search:LegacyError' .
cargo run -p codegraph-cli -- query 'diagnostics severity:error language:rust' .
cargo run -p codegraph-cli -- query 'insights severity:error kind:dependency' .
cargo run -p codegraph-cli -- query 'insights kind:ambiguous_entrypoint_target' .
cargo run -p codegraph-cli -- query 'insights kind:ambiguous_call_resolution' .
cargo run -p codegraph-cli -- query 'insights kind:sensitive_config_default' .
cargo run -p codegraph-cli -- query 'annotations key:domain value:payments direction:out edge_limit:300' .
cargo run -p codegraph-cli -- query 'nodes metadata.annotation.domain:payments' .
```

Inspect a node card with context, source preview, and related risks:

```bash
cargo run -p codegraph-cli -- node-card . --node-id 1
```

Search source text with compact snippets:

```bash
cargo run -p codegraph-cli -- source-search DATABASE_URL . --path-filter src --limit 20
```

Trace outgoing dependencies from a label:

```bash
cargo run -p codegraph-cli -- trace main . --depth 3
```

Trace incoming dependents for impact analysis:

```bash
cargo run -p codegraph-cli -- trace-dependents load_config . --depth 3
```

Explain why a graph edge exists:

```bash
cargo run -p codegraph-cli -- explain-edge . --source main --target load_config --kind calls
cargo run -p codegraph-cli -- explain-edge . --edge-index 12
```

Trace startup flows from entrypoint candidates:

```bash
cargo run -p codegraph-cli -- trace-entrypoints . --depth 3
cargo run -p codegraph-cli -- trace-entrypoints . --search server --depth 4
```

Trace config files and environment variables back to readers and entrypoints:

```bash
cargo run -p codegraph-cli -- trace-config DATABASE_URL . --depth 6
```

Trace potential error and exception constructs back to sources and entrypoints:

```bash
cargo run -p codegraph-cli -- trace-errors 'failed to load data' . --depth 6
```

Include hidden paths:

```bash
cargo run -p codegraph-cli -- scan . --include-hidden
```

Include default ignored paths such as `target` and `node_modules`:

```bash
cargo run -p codegraph-cli -- scan . --include-ignored
```

CLI graph commands use the persistent graph cache by default. Disable it for a
single run or pin records to a specific directory:

```bash
cargo run -p codegraph-cli -- summary . --no-cache
cargo run -p codegraph-cli -- summary . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- cache-diff . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- cache-chunks . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-plan . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-scan . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-merge-preview . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-update . --cache-dir /tmp/codegraph-cache
```

The output is JSON using the shared graph schema from `codegraph-core`.

Run the web application:

```bash
cargo run -p codegraph-server -- --root .
```

Open:

```text
http://127.0.0.1:3765
```

The server stores persistent graph cache records outside the project by default
(`CODEGRAPH_CACHE_DIR`, `XDG_CACHE_HOME/codegraph`, `~/Library/Caches/codegraph`
on macOS, or a temp fallback). Use `--cache-dir <path>` to choose a directory or
`--no-cache` to force every request to rescan.

Build and run the container image:

```bash
docker build -t codegraph:local .
docker run --rm -p 3765:3765 \
  -v "$PWD:/workspace:ro" \
  -v codegraph-cache:/cache \
  codegraph:local
```

The image starts `codegraph-server` as a non-root user with `/workspace` as the
default project root and `/cache` as the persistent graph cache directory.
Use `--max-file-size <bytes>` to cap per-file reads. Source/manifest files above
the limit remain visible as skipped file nodes and produce `skipped_large_file`
insights. When a selected project has `.codegraph/config.toml`, the server uses
that repository-owned scan policy for API and web requests.
Completed scan and semantic enrichment jobs are retained in bounded in-memory
stores so large graph results do not accumulate without limit. Use
`--max-scan-jobs <count>` and `--max-semantic-jobs <count>` to tune the retained
history; active queued/running jobs are not pruned.
Scan and semantic enrichment jobs also have independent concurrency limits. Use
`--max-scan-concurrency <count>` and `--max-semantic-concurrency <count>` to tune
how many long-running scans and LSP enrichment runs may execute at once; excess
jobs stay queued until a slot is available. `/api/health` reports active and
available slots for both pools.
JSON API request bodies are capped by `--max-api-body-bytes <bytes>` and the
effective limit is published through `/api/capabilities` for clients and agents.

Expose multiple local repositories to the web project selector by repeating
`--project`. Requests remain constrained to the configured roots unless
`--allow-any-path` is set explicitly:

```bash
cargo run -p codegraph-server -- --root . --project ../service-a --project ../tooling
```

Protect API routes with a URL-safe token when binding beyond trusted local use:

```bash
CODEGRAPH_API_TOKEN=change-me cargo run -p codegraph-server -- --root . --host 127.0.0.1
curl -H 'authorization: Bearer change-me' 'http://127.0.0.1:3765/api/health'
```

When token protection is enabled, the embedded web UI prompts for the token and
stores it in browser local storage for later same-origin API and SSE requests.

Scan API:

```bash
curl 'http://127.0.0.1:3765/api/projects'
curl 'http://127.0.0.1:3765/api/capabilities'
curl 'http://127.0.0.1:3765/api/schema'
curl 'http://127.0.0.1:3765/api/health'
curl 'http://127.0.0.1:3765/api/metrics'
curl 'http://127.0.0.1:3765/api/scan-options?path=.'
curl 'http://127.0.0.1:3765/api/languages'
curl 'http://127.0.0.1:3765/api/lsp'
curl 'http://127.0.0.1:3765/api/semantic-readiness?path=.'
curl 'http://127.0.0.1:3765/api/semantic-plan?path=.'
curl 'http://127.0.0.1:3765/api/semantic-plan?path=.&work_item_limit=25'
curl 'http://127.0.0.1:3765/api/semantic-plan?path=.&work_status=ready&work_capability=definitions'
curl 'http://127.0.0.1:3765/api/semantic-batch?path=.&work_status=ready&work_capability=definitions'
curl 'http://127.0.0.1:3765/api/semantic-batch?path=.&work_status=ready&work_capability=workspace_symbols'
curl -X POST 'http://127.0.0.1:3765/api/semantic-patch' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_status":"ready","work_capability":"definitions","responses":[]}'
curl -X POST 'http://127.0.0.1:3765/api/semantic-apply' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_status":"ready","work_capability":"definitions","responses":[]}'
curl -X POST 'http://127.0.0.1:3765/api/semantic-enrich' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_item_limit":25,"work_status":"ready","work_capability":"definitions"}'
curl -X POST 'http://127.0.0.1:3765/api/semantic-jobs' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_item_limit":25,"work_status":"ready","work_capability":"definitions"}'
curl 'http://127.0.0.1:3765/api/semantic-jobs?status=running&limit=20'
curl 'http://127.0.0.1:3765/api/semantic-jobs/semantic-1/events'
curl 'http://127.0.0.1:3765/api/semantic-jobs/semantic-1/result'
curl -X DELETE 'http://127.0.0.1:3765/api/semantic-jobs/semantic-1'
curl 'http://127.0.0.1:3765/api/report?path=.&fail_on=warning&insight_limit=100'
curl 'http://127.0.0.1:3765/api/coverage?path=.'
curl 'http://127.0.0.1:3765/api/scan?path=.'
curl 'http://127.0.0.1:3765/api/cache-diff?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/cache-chunks?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/incremental-plan?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/incremental-scan?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/incremental-merge-preview?path=.&limit=50'
curl -X POST 'http://127.0.0.1:3765/api/incremental-update?path=.&limit=50'
```

The scan response includes `cache.status` as `hit`, `miss`, or `disabled`. Cache diff responses and the web Cache Diff panel explain the previous and current project fingerprints without performing a full graph scan, including reuse strategy, changed file counts, reusable file/byte counts, and reuse ratios for incremental-scan planning. Incremental scan responses include the plan plus a focused graph for changed current files, while full-scan actions still return a complete graph. Merge preview responses use persistent impact and chunk indexes to remove cached scopes for changed or removed files, then add changed-file rescans. Surface-stable partial previews, such as body-only edits with the same graph signatures and no incoming cross-file blockers, are marked complete and can be stored; structural changes remain incomplete until a full scan rebuilds cross-file incoming edges.
Incremental update responses use `POST` because they may persist the graph cache when the result is complete; incomplete partial previews report `stored: false` with the reason and leave the previous cache record untouched. Incomplete previews also include structured blocker counters for removed paths, incoming cross-file edges, and graph-surface additions/removals so agents and the web UI can explain why the cache was not updated.
Cache chunk responses list the persistent per-file node and edge scopes currently stored in the graph cache, including compact node/edge id previews for agent diagnostics.

Export API:

```bash
curl 'http://127.0.0.1:3765/api/export?path=.&format=dot'
curl 'http://127.0.0.1:3765/api/export?path=.&format=ndjson'
```

The web Export panel can also download `Report JSON`, which uses `/api/report`
instead of the raw graph export route.

Async scan job API:

```bash
curl -X POST 'http://127.0.0.1:3765/api/scan-jobs' \
  -H 'content-type: application/json' \
  -d '{"path":"."}'
curl 'http://127.0.0.1:3765/api/scan-jobs?status=complete&limit=20'
curl 'http://127.0.0.1:3765/api/scan-jobs/scan-1'
curl -N 'http://127.0.0.1:3765/api/scan-jobs/scan-1/events'
curl 'http://127.0.0.1:3765/api/scan-jobs/scan-1/result'
curl -X DELETE 'http://127.0.0.1:3765/api/scan-jobs/scan-1'
```

Analysis APIs:

```bash
curl 'http://127.0.0.1:3765/api/graph?path=.&node_limit=250&kind=function'
curl 'http://127.0.0.1:3765/api/node-context?path=.&node_id=1&edge_limit=80'
curl 'http://127.0.0.1:3765/api/node-card?path=.&node_id=1&edge_limit=80&source_context=5&insight_limit=8'
curl 'http://127.0.0.1:3765/api/focus?path=.&node_ids=1,2&edge_indexes=0&edge_limit=200'
curl 'http://127.0.0.1:3765/api/summary?path=.'
curl 'http://127.0.0.1:3765/api/architecture?path=.&group_limit=50&edge_limit=200'
curl 'http://127.0.0.1:3765/api/language-dependencies?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/hotspots?path=.&limit=25'
curl 'http://127.0.0.1:3765/api/entrypoints?path=.'
curl 'http://127.0.0.1:3765/api/entrypoint-traces?path=.&search=server&depth=4'
curl 'http://127.0.0.1:3765/api/check?path=.&fail_on=warning&kind=dependency'
curl 'http://127.0.0.1:3765/api/insights?path=.'
curl 'http://127.0.0.1:3765/api/insights?path=.&severity=warning&kind=dependency&limit=25'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=nodes kind:function label:main'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=neighbors label:main direction:out depth:2 edge_kind:calls'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=dependents label:load_config depth:3'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=path from:main to:load_config depth:6'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=unreachable language:rust'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=unreachable scope:errors search:LegacyError'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=diagnostics severity:error language:rust'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=annotations key:domain value:payments direction:out edge_limit:300'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=insights severity:error'
curl --get 'http://127.0.0.1:3765/api/source-search' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=DATABASE_URL' \
  --data-urlencode 'path_filter=src'
curl --get 'http://127.0.0.1:3765/api/explain-edge' \
  --data-urlencode 'path=.' \
  --data-urlencode 'source=main' \
  --data-urlencode 'target=load_config' \
  --data-urlencode 'kind=calls'
curl 'http://127.0.0.1:3765/api/trace?path=.&label=main&depth=3'
curl 'http://127.0.0.1:3765/api/dependents?path=.&label=load_config&depth=3'
curl 'http://127.0.0.1:3765/api/trace-config?path=.&target=DATABASE_URL&depth=6'
curl --get 'http://127.0.0.1:3765/api/trace-errors' \
  --data-urlencode 'path=.' \
  --data-urlencode 'target=failed to load data' \
  --data-urlencode 'depth=6'
```

`/api/graph` supports `node_offset`, `node_limit`, `edge_offset`,
`edge_limit`, `path_prefix`, `kind`, `search`, `language`, `item_kind`,
`edge_kind`, `confidence`, `edge_relation`, and `edge_source`.
Returned edges connect nodes in the returned node page.

Source preview API:

```bash
curl 'http://127.0.0.1:3765/api/source?path=crates/codegraph-cli/src/main.rs&start_line=1&end_line=20'
```

At the current stage, supported source languages are detected by extension:

- Rust: `rs`
- Python: `py`, `pyw`
- JavaScript: `js`, `mjs`, `cjs`
- TypeScript/TSX: `ts`, `mts`, `cts`, `tsx`
- Go: `go`
- C: `c`, `h`
- C++: `cc`, `cpp`, `cxx`, `hpp`, `hh`, `hxx`
- PHP: `php`, `phtml`
- Bash/shell: `sh`, `bash`, `zsh`, `ksh`, `Makefile`

Supported package manifests:

- Rust/Cargo: `Cargo.toml`
- JavaScript/TypeScript/npm-compatible: `package.json`
- Go modules: `go.mod`
- Python: `requirements.txt`, `pyproject.toml`, `setup.py`
- PHP/Composer: `composer.json`
- C/C++ package managers: `vcpkg.json`, `conanfile.txt`

Manifest dependencies are normalized into canonical package nodes with a stable
`package_id` metadata value such as `cargo:serde` or `python:fastapi`. Individual
manifest files connect to those package nodes with `depends_on` edges; the edge
metadata records whether the declaration is runtime, dev, optional, peer,
build, or test dependency data when the manifest format exposes that
distinction, plus the raw `dependency_version` constraint when the manifest declares one. Cargo
`workspace = true` dependencies resolve to the root workspace constraint when
one exists; path-only workspace dependencies omit `dependency_version`.

Manifest entrypoints are represented as `entrypoint` nodes linked from the
repository root with exact `entrypoint` edges. Examples include Cargo binaries,
npm scripts, Python project and setup.py console scripts, Composer scripts, and
Composer binaries.
When a manifest target can be mapped back to code, the entrypoint node also
emits `references` edges with metadata such as `relation=entrypoint_file` or
`relation=entrypoint_function`; traces follow these edges before continuing into
regular call, import, config, environment, dependency, and error-flow edges.
Entrypoint trace reports run this traversal for all matching entrypoints so a
project's startup flows can be compared without manually copying labels.
Config traces specialize that graph traversal by matching `config` and
`environment` nodes, listing direct readers, and returning shortest known paths
from manifest entrypoints to the reader and final read edge.
Error traces use the same upstream traversal for `may_error` edges, listing the
function or construct that may produce an error and returning the shortest known
entrypoint path to the final error edge.

## Workspace

```text
crates/
  codegraph-core/      graph schema and shared domain types
  codegraph-analysis/  summaries, entrypoints, and trace subgraphs
  codegraph-parser/    Tree-sitter syntax extraction
  codegraph-lsp/       LSP server discovery and semantic enrichment foundation
  codegraph-indexer/   project scanning and graph construction
  codegraph-storage/   persistent graph cache and project fingerprints
  codegraph-cli/       command-line interface
  codegraph-server/    HTTP API and embedded static web app
  codegraph-web/       browser UI assets
```

Expected future crates:

```text
crates/
  codegraph-ui/        optional Tauri desktop shell
```

## Development

Contribution guidance and local verification commands live in
[`CONTRIBUTING.md`](CONTRIBUTING.md). The GitHub Actions workflow runs the same
core checks for pushes and pull requests.

## Design Principles

- The CLI and UI must use the same graph model.
- Agent-facing output must be stable, structured, and documented.
- Every extracted fact should carry a confidence level.
- Language support should degrade gracefully from semantic to syntactic to heuristic analysis.
- The project should prefer proven language tooling such as Tree-sitter and LSP servers instead of inventing parsers from scratch.
