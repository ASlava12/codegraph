# Roadmap

## Phase 0: Bootstrap

Status: mostly complete.

- [x] Initialize git repository.
- [x] Create Rust workspace.
- [x] Add core graph schema.
- [x] Add graph analysis crate.
- [x] Add basic project scanner.
- [x] Add first CLI command.
- [x] Add parser crate.
- [x] Add CI checks.
- [x] Add contribution and architecture docs.

Exit criteria:

- `cargo test --workspace` passes.
- `cargo run -p codegraph-cli -- scan .` emits valid graph JSON.
- The graph schema is stable enough for the first parser integration.

## Phase 1: Universal Syntax Graph

Goal: extract language-independent structure from source files.

- [x] Add `codegraph-parser`.
- [x] Integrate Tree-sitter.
- [x] Detect languages by extension and file name.
- [x] Extract files, modules, imports, top-level declarations, functions, and classes/types.
- [x] Support Rust, Python, JavaScript, TypeScript/TSX, Go, C, C++, PHP, and Bash at syntax level.
- [ ] Add Dart syntax support for `.dart` files with files, libraries, imports/exports/parts, classes, mixins, extensions, functions, methods, constructors, and approximate call sites.
- [x] Extract approximate call sites.
- [x] Resolve local import/include file dependencies where syntax-level paths are explicit.
- [ ] Resolve Dart relative imports, package imports from `pubspec.yaml`, `part`/`part of` relationships, and generated-file conventions.
- [x] Resolve Python project-local absolute imports when they match scanned files.
- [x] Resolve Go module-local imports from `go.mod` module paths when package files are scanned.
- [x] Resolve quoted C/C++ includes through CMake include directories when header files are scanned.
- [x] Resolve quoted C/C++ includes through `compile_commands.json` include directories.
- [x] Detect CommonJS `require(...)` imports for dependency and local-file analysis.
- [x] Detect config and environment reads.
- [ ] Detect Dart/Flutter config reads, environment reads, asset references, platform-channel boundaries, and common exception/error constructs.
- [x] Detect basic error/exception constructs.
- [x] Detect manifest-defined entrypoints from project metadata.
- [ ] Detect Dart and Flutter entrypoints from `main()`, Flutter `runApp(...)`, `bin/*.dart`, `test/*_test.dart`, and `pubspec.yaml` package metadata.
- [x] Resolve manifest entrypoints to target files and functions where possible.
- [x] Detect shebang script entrypoints for Bash, Python, Node.js, and PHP CLI files.
- [x] Detect Go module and `cmd/*` entrypoints from `go.mod`.
- [x] Detect CMake `add_executable` entrypoints for C and C++ projects.
- [x] Detect Makefile task targets as build/test/run entrypoints.
- [x] Detect Dockerfile `ENTRYPOINT` and `CMD` instructions as runtime entrypoints.
- [x] Detect Docker Compose services as runtime entrypoints with service dependency edges.
- [x] Detect Docker Compose service `environment` and `env_file` runtime config inputs.
- [x] Detect Docker Compose published ports as runtime surface facts.
- [x] Detect Docker Compose bind/local volumes as runtime dependency facts.
- [x] Detect GitHub Actions workflow jobs as CI/CD entrypoints.
- [x] Link GitHub Actions workflow job `needs`, `uses`, local actions, and local run-script paths.
- [x] Detect GitHub Actions workflow and job `env` variables as environment inputs.
- [x] Detect GitLab CI pipeline jobs as CI/CD entrypoints.
- [x] Link GitLab CI job `needs`, `dependencies`, and local script command paths.
- [x] Detect GitLab CI pipeline and job `variables` as environment inputs.
- [x] Detect Kubernetes workloads, services, and ConfigMap/Secret runtime config references from YAML manifests.
- [x] Detect Kubernetes Ingress entrypoints and backend Service references.
- [x] Link Kubernetes Services to workloads through selector and pod-template label matching.
- Add graph export formats:
  - [x] JSON
  - [x] DOT/Graphviz
  - [x] NDJSON for streaming agent use

Exit criteria:

- A mixed-language repository produces a navigable graph.
- CLI can show entrypoint candidates and symbol summaries.
- Parser failures are reported without aborting the whole scan.
- Function calls are represented as heuristic edges when syntactic resolution is possible.
- Config reads, environment reads, and potential error constructs are represented as heuristic graph facts.

## Phase 2: Semantic Enrichment

Goal: improve precision where language tooling exists.

- [x] Add `codegraph-lsp`.
- [x] Add LSP server discovery report in CLI, API, and web overview.
- [x] Add project semantic readiness reports from scanned language mix and available LSP servers.
- [x] Add semantic enrichment planning reports for ready, blocked, and unsupported LSP work.
- [x] Add capped semantic work queues for concrete LSP file, symbol, and edge requests.
- [x] Add stable ids, priorities, and reasons to semantic work queue items.
- [x] Add semantic work queue filters for language, status, and capability.
- [x] Add semantic execution batch reports grouped by language server command.
- [x] Add executable LSP request descriptors for semantic batch runners.
- [x] Add workspace symbol work items and request planning.
- [x] Add semantic LSP response mapping into graph patch reports.
- [x] Add semantic graph patch application into enriched graph reports.
- [x] Add first CLI semantic LSP batch runner over stdio.
- [x] Use LSP definitions, references, document symbols, workspace symbols, and diagnostics.
- [x] Add Rust enrichment through `rust-analyzer`.
- [x] Add TypeScript/JavaScript enrichment through tsserver-compatible tooling.
- [x] Add Go enrichment through `gopls`.
- [ ] Add Dart semantic enrichment through the Dart analysis server or an LSP-compatible Dart language server.
- [x] Mark facts by confidence:
  - [x] `exact`
  - [x] `semantic`
  - [x] `syntactic`
  - [x] `heuristic`
  - [x] `unknown`

Exit criteria:

- Function call edges can be resolved to definitions where LSP supports it.
- Graph facts retain provenance and confidence.
- CLI can explain why a relationship exists.

## Phase 3: Code Flow Exploration

Goal: answer practical code investigation questions.

- [x] Trace from entrypoints.
- [x] Trace config loading and environment variable reads.
- [x] Identify error and exception paths.
- [x] Add structured summary, entrypoints, and trace commands.
- [x] Expose structured summary, entrypoints, and trace APIs.
- [x] Add investigation insights for unresolved calls, parse/syntax issues, duplicate labels, orphan functions, and error-flow facts.
- [x] Add investigation insights for semantic LSP diagnostics.
- [x] Detect directed dependency cycles across calls, imports, references, and dependency edges.
- [x] Detect package manifest dependency boundaries.
- [x] Preserve Go `// indirect` dependency scope from `go.mod`.
- [x] Warn when production Go code directly imports a dependency declared only as `// indirect`.
- [x] Detect direct npm `package-lock.json` dependencies with locked versions.
- [x] Detect direct pnpm `pnpm-lock.yaml` importer dependencies with locked versions.
- [x] Distinguish locked dependency versions from manifest constraints in dependency conflict insights.
- [x] Detect legacy Python `setup.py`/`setup.cfg` dependencies and console script entrypoints.
- [x] Detect Python `Pipfile` runtime and dev package dependencies.
- [x] Detect modern Poetry dependency groups from `pyproject.toml`.
- [x] Detect Composer `composer.lock` runtime and dev locked dependencies.
- [x] Match PHP namespace imports to Composer dependencies for dependency consistency insights.
- [x] Keep web fallback dependency insights aligned with PHP/Composer namespace imports and local import scopes.
- [x] Detect C/C++ package manifest dependencies from `vcpkg.json` and `conanfile.txt`.
- [x] Detect CMake `find_package(...)` dependencies and match them to C/C++ include usage.
- [x] Detect duplicate entrypoint labels that make startup traces ambiguous.
- [x] Detect manifest entrypoints that resolve to multiple possible files or functions.
- [x] Detect ambiguous call resolutions where one call label points to multiple targets from the same caller.
- [x] Detect manifest entrypoints whose declared target cannot be resolved.
- [x] Detect entrypoints that do not lead to any known code/config/dependency/error flow.
- [x] Detect external imports that are missing declared manifest dependencies.
- [x] Detect runtime manifest dependencies with no matching import.
- [x] Detect conflicting manifest dependency constraints for the same package.
- [x] Detect packages declared across multiple dependency scopes such as runtime and dev/build.
- [x] Detect production-like source imports of packages declared only in non-runtime dependency scopes.
- [x] Detect runtime dependencies that are imported only from test-like source files.
- [x] Recognize common JS/TS, Go, Python, PHP, C/C++, and Bash test file conventions in dependency-scope insights.
- [ ] Recognize Dart and Flutter package scopes, dev dependencies, generated files, assets, test conventions, and package import consistency issues.
- [x] Detect duplicate framework route method/path declarations.
- [x] Detect framework routes whose named handler cannot be resolved.
- [x] Improve Rust/Axum route entrypoint labels for multiline routes and string literal false positives.
- [x] Detect local imports/includes whose target file cannot be resolved.
- [x] Detect Makefile task targets that reference missing local command paths.
- [x] Detect Dockerfile entrypoints that reference missing local command paths.
- [x] Detect Docker Compose services that reference missing local command paths.
- [x] Detect Docker Compose services that reference missing local `env_file` paths.
- [x] Detect Docker Compose services that publish conflicting host ports.
- [x] Detect Docker Compose services that mount missing local bind volume paths.
- [x] Detect GitHub Actions workflow jobs whose `needs` reference missing jobs.
- [x] Detect GitHub Actions workflow jobs that reference missing local actions.
- [x] Detect GitHub Actions workflow jobs that reference missing local run-script paths.
- [x] Detect GitLab CI jobs whose `needs` or `dependencies` reference missing jobs.
- [x] Detect GitLab CI jobs that reference missing local script paths.
- [x] Detect Kubernetes workloads that reference missing local ConfigMap or Secret manifests.
- [x] Detect Kubernetes Ingresses that reference missing local Service manifests.
- [x] Detect Kubernetes Services whose selectors do not match any scanned workload.
- [x] Detect custom architectural boundary violations on graph edges.
- [x] Detect config and environment reads that are not reachable from entrypoints.
- [x] Detect potential error/exception flows whose source is not reachable from entrypoints.
- [x] Detect non-test source files with code symbols that are not reachable from entrypoints.
- [x] Detect config and environment keys that are read with conflicting fallback defaults across common inline Rust, Python, JavaScript/TypeScript, Go, C, C++, PHP, and Bash patterns.
- [x] Detect config and environment keys that are read both as required and with fallback defaults.
- [x] Detect sensitive config and environment keys, credential-like values, and placeholder secret defaults without leaking fallback values in reports.
- [x] Detect literal sensitive CI environment assignments without leaking assigned values in reports.
- [x] Promote source comments such as `WHY`, `NOTE`, `TODO`, `FIXME`, `HACK`, `BUG`, `XXX`, and `SECURITY` into linked rationale graph nodes.
- [x] Emit risk insights for actionable rationale markers such as `SECURITY`, `FIXME`, `HACK`, `BUG`, and `XXX`.
- [x] Back web insights with server analysis during paged graph exploration.
- [x] Add CLI, server-side, and web filters for insight severity, kind, and search.
- [x] Add insight severity and kind breakdowns for triage.
- [x] Make web insight severity breakdown chips clickable triage filters.
- [x] Add web insight JSON export for filtered findings and triage handoff.
- [x] Add CI/agent check command, API, and web quality gate for insight severity thresholds.
- [x] Add web quality-gate check result JSON downloads.
- [x] Add first graph query language for nodes, edges, calls, dependencies, and traces.
- [x] Add directed path queries between graph labels or node ids.
- [x] Add confidence-aware edge queries and UI provenance labels.
- [x] Warn on heuristic cross-language dependency edges.
- [x] Add web path navigation with visual dependency-path highlighting.
- [x] Add richer query language with neighborhood expansion.
- [x] Add symbol query slices for focused function/type/module structure context.
- [x] Add file query slices for focused source-file structure and contained-symbol dependency context.
- [x] Add reachability-aware query slices for unreachable source files and nodes.
- [x] Add reachability-aware query scopes for unreachable config reads and error flows.
- [x] Add semantic diagnostic query slices for focused LSP issue context.
- [x] Add insight/risk query slices for focused investigation findings.
- [x] Add annotation query slices for focused user-owned metadata context.
- [x] Add entrypoint query slices for focused startup graph context.
- [x] Add route query slices for focused HTTP/framework handler context.
- [x] Add package query slices for focused external dependency declaration/import context.
- [x] Add config query slices for focused configuration reader context.
- [x] Add error query slices for focused exception/error source context.
- [x] Add cycle query slices for focused circular dependency context.
- [x] Add hotspot query slices for focused high-degree dependency context.
- [x] Add query result facets for agent and web triage summaries.
- [x] Add reverse dependent traces for impact analysis.
- [x] Add web entrypoint trace JSON downloads for startup-flow handoff.
- [x] Add web config/error trace JSON downloads for configuration and exception-flow handoff.
- [x] Add source text search across CLI, API, and web for focused code snippets.
- [x] Add web source-search actions for opening matching files as focused graph slices.
- [x] Add web source-search result JSON downloads for agent handoff.
- [x] Add edge explanation for confidence and provenance evidence across CLI, API, and web.
- [x] Add a derived workflow model that converts selected outgoing traces into block-style execution steps.
- [x] Add workflow JSON output for agents with stable block ids, source node ids, edge provenance, confidence, and risk references.
- [x] Add first Mermaid flowchart export for human-readable workflow diagrams.
- [x] Add first CLI workflow command for selected entrypoint/function/node labels.
- [x] Add first workflow API endpoint with depth and block-limit controls.
- [x] Add first web selected-node Flow panel with workflow blocks, transitions, risk badges, and node navigation.
- [x] Add first web workflow export downloads in JSON and Mermaid.
- [ ] Extend workflow generation from selected labels to entrypoints, routes, CI jobs, Makefile targets, Docker commands, and selected query slices.
- [ ] Classify workflow blocks as start, call, config/env read, dependency, branch, error, return, and external boundary.
- [ ] Add workflow graph compaction so repeated helper calls, import-only hops, and low-signal nodes collapse into readable blocks.
- [ ] Add CLI workflow commands for routes, CI jobs, Makefile targets, Docker commands, and selected nodes.
- [ ] Add workflow API filters for edge-kind, confidence, language, risk severity, and block kinds.
- [ ] Add workflow query slices so existing graph query results can open as block diagrams.
- [ ] Add full web Flow view next to Graph/Cards/Risks with pan, zoom, minimap, and selectable workflow blocks.
- [ ] Reuse node and dependency cards from workflow blocks, including source preview, related dependencies, risks, and edge explanations.
- [ ] Add branch extraction for common if/match/try/catch constructs where parser or LSP facts support it.
- [ ] Add loop and async/concurrency markers for common constructs where confidence is high.
- [ ] Add workflow regression fixtures for Rust, Python, JavaScript/TypeScript, Go, PHP, Bash, Dart/Flutter, CI, Docker, and Kubernetes runtime paths.
- [ ] Add remaining workflow export downloads for DOT and visible-slice JSON.
- [ ] Localize workflow UI, commands, schema descriptions, and block labels in English and Russian.

Example commands:

```bash
codegraph entrypoints
codegraph trace main
codegraph trace-entrypoints --search server
codegraph trace-config DATABASE_URL
codegraph trace-errors "failed to load data"
codegraph workflow --entrypoint main --format json
codegraph workflow --entrypoint "POST /users" --format mermaid
codegraph workflow --entrypoint "github workflow:CI/deploy" --depth 4
codegraph query 'neighbors label:main direction:out depth:2 edge_kind:calls'
codegraph query 'calls(function:main)'
```

Exit criteria:

- A human can start from an entrypoint and follow meaningful execution paths.
- An agent can request focused subgraphs instead of reading a whole repository.
- A human can switch from a dense graph to a readable block workflow for a selected entrypoint.
- An agent can request machine-readable workflow blocks with source, confidence, provenance, and risk context.

## Phase 4: API And UI

Status: in progress.

Goal: make the graph explorable interactively in a modern web UI.

- [x] Add `codegraph-server`.
- [x] Serve graph data over HTTP.
- [x] Add `codegraph-web` for browser usage.
- [x] Implement first graph canvas, filters, search, stats, and detail panel.
- [x] Add web query panel for focused graph slices and canvas narrowing.
- [x] Add async scan job API for long-running scans.
- [x] Add SSE for live scan status updates.
- [x] Add first server-side graph paging and filtering API.
- [x] Add web controls for server-side graph paging.
- [x] Add web controls for server-side graph search and item filtering.
- [x] Add web controls for server-side language filtering.
- [x] Add web controls for server-side edge confidence filtering.
- [x] Add web controls for server-side edge relation and provenance source filtering.
- [x] Add web graph export downloads for JSON, DOT, and NDJSON.
- [x] Add visible canvas-slice JSON export with filter, paging, viewport, and layout metadata.
- [x] Publish full graph export node/edge/byte metadata headers.
- [x] Add graph viewport controls for zoom, fit, layout reset, and layout pause.
- [x] Add web project overview for languages, edge confidence, and entrypoints.
- [x] Add web semantic work queue for prioritized LSP enrichment tasks.
- [x] Add web filters for semantic work queue language, status, and capability.
- [x] Surface full semantic work counters for definitions, diagnostics, symbols, references, and workspace symbols.
- [x] Add API and web action for running ready semantic enrichment batches.
- [x] Add async semantic enrichment jobs with status, SSE, and result retrieval.
- [x] Add architecture map overview for top-level project areas and dependencies.
- [x] Add language dependency matrix overview for mixed-language coupling.
- [x] Add UI graph focusing from architecture overview areas.
- [x] Add focused graph views for architecture cross-area dependency edges.
- [x] Add hotspot reports for high-degree graph nodes.
- [x] Add web project overview facets for edge relation and provenance sources.
- [x] Add web path navigation, focused graph views, and visual highlighting for dependency paths.
- [x] Add node context API for paged graph detail panels.
- [x] Add unified node card contracts across CLI, API, and web UI with source preview and related risks.
- [x] Add insight focus API and web interaction for opening findings as focused graph views.
- [x] Add enriched node cards with summary, code preview, dependencies, and related risks.
- [x] Add shareable query deep-links and copy-link actions for reusable web investigations.
- [x] Add local recent-query history in the web query panel.
- [x] Add query-result JSON downloads for portable investigation handoff.
- [x] Add shareable graph-page deep-links for paged/filterable large-repository slices.
- [x] Add a one-click web reset for graph-page filters and offsets.
- [x] Add package graph actions from external dependency/import node cards.
- [x] Add selectable graph edges with dependency cards and exact edge explanation actions.
- [x] Highlight graph edges on hover before opening dependency cards.
- [x] Surface edge-scoped risks in dependency explanation cards.
- [x] Open dependency cards from query, trace, and node-neighbor edge rows.
- [x] Add exact focus/query actions to dependency cards.
- [x] Add shareable web deep-links and copy-link actions for selected node and dependency cards.
- [x] Add downloadable node/dependency card JSON snapshots for portable investigation handoff.
- [x] Add initial multi-language UI support with English and Russian.
- [x] Reduce graph label clutter with zoom thresholds and collision-aware label drawing.
- [x] Tighten graph label budgets and adaptive label placement so node captions do not cover dense graphs.
- [x] Make minimal node labels the default and keep dense captions behind explicit Focus/Auto modes.
- [x] Reset saved web label mode and make Auto labels sparse enough for dense graph exploration.
- [x] Keep hover/focus labels as compact side badges and leave selected-node details in the side card.
- [x] Suppress node captions in dense graph views and reset stale saved label modes.
- [x] Add a graph viewport HUD for visible size, zoom, and layout state.
- [x] Surface loaded-vs-total graph slice status for paged large-repository views.
- [x] Add visible canvas-filter status and one-click reset for local graph filters.
- [x] Add web controls for paging dense edge slices through the existing `edge_offset` API contract.
- [x] Add a graph minimap with click/drag viewport recentering.
- [x] Add keyboard-accessible graph canvas navigation.
- [x] Add web label policy regression tests for caption density and interaction labels.
- [x] Add embedded web asset smoke checks for script order, content types, and static JS validity.
- [x] Add embedded web asset smoke coverage for shareable card and query investigation links.
- [x] Add clickable risk severity legend filters for graph triage.
- [x] Add bounded server retention for scan and semantic jobs with health counters.
- [x] Add configurable server concurrency limits for scan and semantic jobs with health counters.
- [x] Add cancelable scan and semantic jobs in API and web UI.
- [x] Add filterable scan and semantic job list APIs for retained job history.
- [x] Add web job monitor for scan and semantic job history.
- [x] Add API capabilities endpoint for agent and UI runtime discovery.
- [x] Publish graph, query, report, source, and card runtime limits in API capabilities.
- [x] Publish bounded project report snapshot topology and risk list limits in API capabilities.
- [x] Add machine-readable API schema endpoint for agents and integrations.
- [x] Publish structured parameter bounds in the API schema for bounded agent requests.
- [x] Publish structured POST body fields in the API schema for scan and semantic job requests.
- [x] Enforce and publish configurable API request body limits.
- [x] Publish structured system response fields in the API schema for runtime clients.
- [x] Publish common HTTP response headers in the API schema for runtime clients and agents.
- [x] Publish structured graph investigation response fields in the API schema for agents.
- [x] Publish structured analysis and source response fields in the API schema for agents.
- [x] Publish insight/check limits and link overview/report API schema parameters to capability keys.
- [x] Clamp and publish semantic work item limits across CLI, API, schema, and web capabilities.
- [x] Clamp and publish semantic LSP request timeout limits across CLI, API, and web capabilities.
- [x] Publish known insight kinds in the API schema for agent discovery and web filter suggestions.
- [x] Use API schema enum values as web graph filter suggestions.
- [x] Enforce and publish graph query expression length limits for production clients.
- [x] Enforce and publish source-search query length limits for production clients.
- [x] Publish project report sections and risk grades in the API schema for agent discovery.
- [x] Publish web deep-link parameters in the API schema for agent discovery.
- [x] Add runtime metrics endpoint for uptime, cache, job stores, and concurrency.
- [x] Add lightweight liveness and readiness probes for production deployments.
- [x] Publish server package version in runtime and discovery responses.
- [x] Surface server package version in the web overview.
- [x] Add a multi-stage Docker image for web/API server deployment.
- [x] Add CI smoke coverage for the Docker web/API server image.
- [x] Surface runtime metrics in the web UI.
- [x] Surface last API response latency in the web runtime panel.
- [x] Add built-in HTTP access logs with latency and quiet-mode control.
- [x] Add `x-request-id` response headers, JSON error fields, and access-log correlation.
- [x] Add `x-response-time-ms` response headers for client-side latency diagnostics.
- [x] Surface request ids in web error messages for access-log correlation.
- [x] Add graceful HTTP server shutdown for Ctrl-C and SIGTERM.
- [x] Surface server capabilities in the web overview.
- [x] Surface API schema common response-header contracts in the web overview.
- [x] Add server-wide security headers for embedded web and API responses.
- [x] Add production cache-control headers for embedded web assets and runtime API responses.
- [x] Add ETag-backed conditional responses for embedded web assets.
- [x] Add optional API bearer-token protection with web UI token handling.
- [x] Add project report snapshots across CLI, API, and web export.
- [x] Add full-project risk summary and limit-independent quality gate evaluation to report snapshots.
- [x] Surface project report risk summary in the web overview with severity and kind quick filters.
- [x] Surface project report quality gate status in the web overview risk summary.
- [x] Let the web overview quality gate chip run the matching quality check.
- [x] Back web overview summary, coverage, topology, hotspots, and risks from one report snapshot to reduce duplicate scans.
- [x] Add optional `codegraph-ui` desktop launcher with a native WebView shell.
- [x] Add source preview.
- [x] Add trace panels.
- [x] Support opening local repositories from a desktop shell.

Exit criteria:

- The UI can load a project graph and preview source spans.
- CLI, API, and UI expose the same underlying graph.

## Phase 5: Scale And Incrementality

Goal: handle real repositories efficiently.

- [x] Add first persistent graph cache for server scans.
- [x] Share persistent graph cache between server and CLI graph commands.
- [x] Add cache fingerprint diff diagnostics across CLI, API, and web UI for explaining cache misses.
- [x] Add configurable scan file-size budgets with skipped large-file visibility.
- [x] Add repository-owned scan policy from `.codegraph/config.toml`.
- [x] Expose effective scan policy through API and web overview.
- [x] Add repository-owned glob path excludes for generated files and fixtures.
- [x] Add scan coverage reports for indexed files and skip reasons.
- [x] Add cache reuse estimates for incremental scan planning.
- [x] Add explicit incremental scan planning reports with cached graph impact scopes.
- [x] Add persistent graph impact index storage for incremental planning.
- [x] Add changed-scope incremental scan graphs across CLI, API, and web UI.
- [x] Add first partial graph merge preview from cached graph plus changed-file rescans.
- [x] Drive partial graph merge previews from the persistent graph impact index.
- [x] Add persistent file graph chunk index storage for per-file node and edge scopes.
- [x] Expose persistent file graph chunk reports across CLI, API, and web UI.
- [x] Add safe incremental cache update reports that persist only complete graph results.
- [x] Store surface-stable partial graph updates from cached graph plus changed-file rescans.
- [x] Use POST as the primary API method for safe incremental cache updates.
- [x] Add HTTP smoke coverage for safe body-only incremental cache updates.
- [x] Localize web incremental cache diagnostics and safe-update blockers.
- [x] Add persistent index storage with partial graph reuse.
- [x] Incrementally update changed files.
- [x] Publish cache and incremental workflow enum values in the API schema for agents.
- [x] Cache parser facts.
- [x] Cache LSP facts.
- [ ] Cache Dart parser and semantic facts with invalidation for `pubspec.yaml`, `.dart_tool/package_config.json`, and generated Dart files.
- [x] Add first large graph filtering and paging endpoint.
- [x] Support first UI-driven large graph paging.
- [x] Add first CLI scan benchmarks.

Exit criteria:

- Medium-sized repositories are usable interactively.
- Re-scanning after a small edit is fast.

## Phase 6: Plugin System

Goal: make language and framework knowledge extensible.

- [x] Add language adapter interface.
- [x] Add first framework route detectors for entrypoints.
- [x] Add framework detectors for config conventions.
- [x] Add custom rules for repositories.
- [x] Support user-defined graph annotations.

Exit criteria:

- New language or framework support can be added without rewriting core graph logic.

## Phase 7: Repository Knowledge Graph Parity

Goal: cover the main Graphify-style ideas while keeping CodeGraph Rust-first, local-first, provenance-aware, and useful for both humans and agents.

Already aligned:

- [x] Build local code graphs from deterministic Tree-sitter parser facts.
- [x] Preserve confidence and provenance on graph edges.
- [x] Expose query, path, explain, trace, and node-card workflows across CLI, API, and web.
- [x] Provide interactive graph exploration with search, filters, focused views, exports, and graph cards.
- [x] Maintain persistent caches and incremental scan/update paths.
- [x] Support local desktop launch through `codegraph-ui`.

Graphify-inspired gaps to close:

- [x] Add first deterministic graph community reports for subsystems with stable ids, local-first labels, sample nodes, and internal/external edge counts.
- [ ] Add god-node and hotspot reports that separate real architectural hubs from noisy utility hubs.
- [ ] Add Markdown/ADR/RFC extraction so design docs, decisions, and citations become first-class graph nodes.
- [x] Promote source comments such as `NOTE`, `WHY`, `TODO`, `FIXME`, and security/risk markers into linked rationale nodes.
- [ ] Add deterministic SQL schema extraction for tables, views, columns, foreign keys, indexes, queries, and JOIN relationships.
- [ ] Link application code to SQL/schema nodes through query strings, migrations, ORM metadata, and database config.
- [ ] Add document ingestion for Markdown, plain text, PDFs, Office files, and generated Markdown sidecars, with size limits and provenance.
- [ ] Add optional local or configured-model semantic extraction for non-code documents while keeping code-only scans fully offline.
- [ ] Add media ingestion hooks for transcripts from audio/video sidecars, with explicit opt-in for transcription backends.
- [ ] Add graph compaction and compact node/file summaries for agent navigation without reading raw files.
- [ ] Add project knowledge reports similar to `GRAPH_REPORT.md`: key concepts, communities, surprising links, risks, and suggested questions.
- [ ] Add agent-facing natural-language query mode that maps a question to a bounded graph slice without vector storage.
- [ ] Add saved query/result memory with outcomes such as useful, dead-end, and corrected, linked back to graph nodes and invalidated by source changes.
- [ ] Add reflection reports that aggregate saved investigation outcomes into repository lessons with provenance and stale-source warnings.
- [ ] Add assistant installation commands for Codex and generic agent-skill instructions, generating project-scoped guidance to query CodeGraph before broad file reads.
- [ ] Add optional local hooks that nudge agents toward CodeGraph query/path/explain before grep-heavy or raw-file-heavy workflows.
- [ ] Add git hooks for post-commit/post-checkout incremental refresh, cache invalidation, and optional graph export regeneration.
- [ ] Add watch mode for automatic local graph refresh while editing.
- [ ] Add global graph registry for multiple local repositories with cross-project path/query support.
- [ ] Add graph merge commands for combining project, docs, incident, and external-system graphs.
- [ ] Add GraphML, SVG, Mermaid/callflow HTML, Obsidian vault, Markdown wiki, Neo4j Cypher, and FalkorDB export targets.
- [ ] Add MCP stdio server mode for graph query/path/explain/report/card access from external assistants.
- [ ] Add PR impact dashboard using graph communities, changed files, CI/review state, conflicts, and risky shared subsystems.
- [ ] Add query logging with privacy controls, response logging opt-in, and local JSONL audit output.
- [ ] Add explicit security model for external ingestion: URL validation, redirect blocking, size/time limits, label sanitization, and graph path constraints.
- [ ] Add benchmark harness for token/context savings and graph-query recall on real mixed corpora.

Exit criteria:

- Code-only repositories remain fully local and deterministic.
- Mixed repositories can combine code, architecture docs, SQL schemas, and runtime metadata in one typed graph.
- Agents can use CodeGraph as persistent repository memory through CLI/API/MCP without re-reading whole projects.
- Humans can inspect communities, rationale, docs, schema links, and code flow from one UI.
