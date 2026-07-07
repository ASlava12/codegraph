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
- [x] Extract approximate call sites.
- [x] Resolve local import/include file dependencies where syntax-level paths are explicit.
- [x] Resolve Python project-local absolute imports when they match scanned files.
- [x] Resolve Go module-local imports from `go.mod` module paths when package files are scanned.
- [x] Resolve quoted C/C++ includes through CMake include directories when header files are scanned.
- [x] Resolve quoted C/C++ includes through `compile_commands.json` include directories.
- [x] Detect CommonJS `require(...)` imports for dependency and local-file analysis.
- [x] Detect config and environment reads.
- [x] Detect basic error/exception constructs.
- [x] Detect manifest-defined entrypoints from project metadata.
- [x] Resolve manifest entrypoints to target files and functions where possible.
- [x] Detect shebang script entrypoints for Bash, Python, Node.js, and PHP CLI files.
- [x] Detect Go module and `cmd/*` entrypoints from `go.mod`.
- [x] Detect CMake `add_executable` entrypoints for C and C++ projects.
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
- Add Rust enrichment through `rust-analyzer`.
- Add TypeScript/JavaScript enrichment through tsserver-compatible tooling.
- Add Go enrichment through `gopls`.
- Mark facts by confidence:
  - `exact`
  - `semantic`
  - `syntactic`
  - `heuristic`
  - `unknown`

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
- [x] Detect manifest entrypoints whose declared target cannot be resolved.
- [x] Detect external imports that are missing declared manifest dependencies.
- [x] Detect runtime manifest dependencies with no matching import.
- [x] Detect conflicting manifest dependency constraints for the same package.
- [x] Detect duplicate framework route method/path declarations.
- [x] Detect framework routes whose named handler cannot be resolved.
- [x] Detect local imports/includes whose target file cannot be resolved.
- [x] Detect custom architectural boundary violations on graph edges.
- [x] Detect config and environment reads that are not reachable from entrypoints.
- [x] Detect non-test source files with code symbols that are not reachable from entrypoints.
- [x] Detect config and environment keys that are read with conflicting fallback defaults across common inline Rust, Python, JavaScript/TypeScript, Go, C, C++, PHP, and Bash patterns.
- [x] Back web insights with server analysis during paged graph exploration.
- [x] Add CLI, server-side, and web filters for insight severity, kind, and search.
- [x] Add insight severity and kind breakdowns for triage.
- [x] Make web insight severity breakdown chips clickable triage filters.
- [x] Add CI/agent check command, API, and web quality gate for insight severity thresholds.
- [x] Add first graph query language for nodes, edges, calls, dependencies, and traces.
- [x] Add directed path queries between graph labels or node ids.
- [x] Add confidence-aware edge queries and UI provenance labels.
- [x] Warn on heuristic cross-language dependency edges.
- [x] Add web path navigation with visual dependency-path highlighting.
- [x] Add richer query language with neighborhood expansion.
- [x] Add reachability-aware query slices for unreachable source files and nodes.
- [x] Add semantic diagnostic query slices for focused LSP issue context.
- [x] Add reverse dependent traces for impact analysis.
- [x] Add source text search across CLI, API, and web for focused code snippets.
- [x] Add edge explanation for confidence and provenance evidence across CLI, API, and web.

Example commands:

```bash
codegraph entrypoints
codegraph trace main
codegraph trace-entrypoints --search server
codegraph trace-config DATABASE_URL
codegraph trace-errors "failed to load data"
codegraph query 'neighbors label:main direction:out depth:2 edge_kind:calls'
codegraph query 'calls(function:main)'
```

Exit criteria:

- A human can start from an entrypoint and follow meaningful execution paths.
- An agent can request focused subgraphs instead of reading a whole repository.

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
- [x] Add initial multi-language UI support with English and Russian.
- [x] Reduce graph label clutter with zoom thresholds and collision-aware label drawing.
- [x] Tighten graph label budgets and adaptive label placement so node captions do not cover dense graphs.
- [x] Make minimal node labels the default and keep dense captions behind explicit Focus/Auto modes.
- [x] Add clickable risk severity legend filters for graph triage.
- [x] Add bounded server retention for scan and semantic jobs with health counters.
- [x] Add configurable server concurrency limits for scan and semantic jobs with health counters.
- [x] Add cancelable scan and semantic jobs in API and web UI.
- [x] Add filterable scan and semantic job list APIs for retained job history.
- [x] Add web job monitor for scan and semantic job history.
- [x] Add API capabilities endpoint for agent and UI runtime discovery.
- [x] Add machine-readable API schema endpoint for agents and integrations.
- [x] Use API schema enum values as web graph filter suggestions.
- [x] Add runtime metrics endpoint for uptime, cache, job stores, and concurrency.
- [x] Surface runtime metrics in the web UI.
- [x] Surface server capabilities in the web overview.
- [x] Add project report snapshots across CLI, API, and web export.
- Add optional `codegraph-ui` with Tauri after the web UI stabilizes.
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
- Add persistent index storage with partial graph reuse.
- Incrementally update changed files.
- [x] Cache parser facts.
- [x] Cache LSP facts.
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
