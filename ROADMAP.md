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
- [x] Detect config and environment reads.
- [x] Detect basic error/exception constructs.
- [x] Detect manifest-defined entrypoints from project metadata.
- [x] Resolve manifest entrypoints to target files and functions where possible.
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

- Add `codegraph-lsp`.
- Use LSP definitions, references, document symbols, workspace symbols, and diagnostics.
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
- [x] Detect directed dependency cycles across calls, imports, references, and dependency edges.
- [x] Detect package manifest dependency boundaries.
- [x] Detect manifest entrypoints whose declared target cannot be resolved.
- [x] Detect external imports that are missing declared manifest dependencies.
- [x] Detect runtime manifest dependencies with no matching import.
- [x] Detect conflicting manifest dependency constraints for the same package.
- [x] Detect config and environment reads that are not reachable from entrypoints.
- [x] Back web insights with server analysis during paged graph exploration.
- [x] Add CLI, server-side, and web filters for insight severity, kind, and search.
- [x] Add insight severity and kind breakdowns for triage.
- [x] Add first graph query language for nodes, edges, calls, dependencies, and traces.
- [x] Add directed path queries between graph labels or node ids.
- [x] Add confidence-aware edge queries and UI provenance labels.
- [x] Add web path navigation with visual dependency-path highlighting.
- [x] Add richer query language with neighborhood expansion.

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
- [x] Add graph viewport controls for zoom, fit, layout reset, and layout pause.
- [x] Add web project overview for languages, edge confidence, and entrypoints.
- [x] Add web path navigation, focused graph views, and visual highlighting for dependency paths.
- [x] Add node context API for paged graph detail panels.
- [x] Add insight focus API and web interaction for opening findings as focused graph views.
- Add optional `codegraph-ui` with Tauri after the web UI stabilizes.
- [x] Add source preview.
- [x] Add trace panels.
- [ ] Support opening local repositories from a desktop shell.

Exit criteria:

- The UI can load a project graph and preview source spans.
- CLI, API, and UI expose the same underlying graph.

## Phase 5: Scale And Incrementality

Goal: handle real repositories efficiently.

- [x] Add first persistent graph cache for server scans.
- [x] Share persistent graph cache between server and CLI graph commands.
- Add persistent index storage with partial graph reuse.
- Incrementally update changed files.
- Cache parser and LSP facts.
- [x] Add first large graph filtering and paging endpoint.
- [x] Support first UI-driven large graph paging.
- [x] Add first CLI scan benchmarks.

Exit criteria:

- Medium-sized repositories are usable interactively.
- Re-scanning after a small edit is fast.

## Phase 6: Plugin System

Goal: make language and framework knowledge extensible.

- Add language adapter interface.
- Add framework detectors for config and entrypoints.
- Add custom rules for repositories.
- Support user-defined graph annotations.

Exit criteria:

- New language or framework support can be added without rewriting core graph logic.
