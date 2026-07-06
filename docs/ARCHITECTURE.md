# Architecture

CodeGraph is organized around one shared artifact: the code graph.

Every interface, including the CLI, UI, and future agent API, should read from or write to the same graph model.

## Layers

### Core

`codegraph-core` owns the stable graph schema:

- node identifiers
- node kinds
- edge kinds
- confidence levels
- source spans
- metadata

This crate should stay small and dependency-light.

### Indexer

`codegraph-indexer` walks project roots, applies ignore rules, invokes parsers, and assembles graph fragments.

It currently builds file, symbol, import, call, config, environment, error-flow, manifest entrypoint, and package dependency graph facts from syntax-level parser output and project manifests. Manifest dependencies are normalized by ecosystem-specific package identity, so multiple manifest files can point at the same external package node while preserving declaration kind and raw version constraint details on each edge. Manifest entrypoints are resolved after the full scan so path and function targets can be linked regardless of filesystem walk order. Future implementations should merge stronger semantic facts from LSP and compiler layers.

### Storage

`codegraph-storage` owns persistent graph cache records and project fingerprints.

The first implementation stores whole-graph JSON cache records keyed by root path and scan options. A fingerprint records scanned relative paths, file sizes, and modification times; a cache hit is only used when the current fingerprint matches the stored record. This is deliberately conservative: it accelerates repeated API and UI scans without yet promising partial graph reuse.

### Analysis

`codegraph-analysis` turns a full graph into focused artifacts:

- summary counts
- entrypoint candidates
- outgoing dependency traces
- investigation insights
- dependency consistency checks
- conflicting manifest dependency constraint checks
- dependency cycle checks
- graph query results and directed path searches for agent and API clients
- insight focus subgraphs for findings with multiple nodes or edges
- project overview data for language mix, edge confidence mix, and entrypoints
- graph slices for paged UI, search/item/language/confidence-filtered exploration, and agent loading
- node context records for detail panels and focused agent reads
- DOT and NDJSON exports

This crate is shared by the CLI and server so humans, UI features, and agents receive the same structured answers.

### Parser

`codegraph-parser` extracts syntax-level code facts with Tree-sitter.

Responsibilities:

- language detection
- Tree-sitter parser management
- syntax-level graph extraction
- approximate call-site extraction with parent function context
- heuristic extraction of config reads, environment reads, and potential error constructs
- parser error recovery

Current language coverage:

- Rust
- Python
- JavaScript
- TypeScript/TSX
- Go
- C
- C++
- PHP
- Bash/shell

### LSP

Future crate: `codegraph-lsp`.

Responsibilities:

- launch and manage language servers
- request definitions, references, symbols, and diagnostics
- convert language-server facts into graph edges

### CLI

`codegraph-cli` is the primary automation interface.

CLI output must remain machine-friendly. Human-oriented formatting can be added, but JSON should stay stable.

### Server And Web UI

`codegraph-server` exposes the graph over HTTP and serves the embedded browser UI from `codegraph-web`.

Responsibilities:

- provide health and scan endpoints
- reuse persistent graph cache records when project fingerprints match
- provide JSON, DOT, and NDJSON export endpoints
- provide async scan job endpoints and SSE status streams for long-running scans
- provide summary, entrypoint, and trace endpoints
- provide investigation insight endpoints
- provide graph query endpoints
- provide graph slice endpoints with server-side paging and filtering
- provide node context endpoints for paged detail exploration
- provide source preview endpoints for graph spans
- provide export endpoints for Graphviz DOT and NDJSON
- constrain scan paths to a configured root by default
- serve the static web application
- keep UI graph pages, query focus, path navigation/highlighting, trace, source, insight, and agent clients on the same JSON graph model

Future crate: optionally `codegraph-ui`.

A Tauri shell can wrap the web UI later for desktop distribution. The UI should visualize the graph without owning analysis logic.

## Confidence Model

Graph facts should declare how they were discovered:

- `exact`: produced by a compiler or exact project metadata.
- `semantic`: produced by LSP or a semantic analyzer.
- `syntactic`: produced from syntax trees.
- `heuristic`: inferred from naming, framework conventions, or patterns.
- `unknown`: legacy or unclassified fact.

This is central to the project. A useful imperfect graph is acceptable when uncertainty is explicit.

Current call, config, environment, and error-flow facts are `heuristic`: they are resolved by syntax patterns and simple-name matching. Package manifest dependencies and manifest-defined entrypoints are `exact` because they come from declared project metadata; package nodes expose `package_id`, and `depends_on` edge metadata records the declaration kind and raw version constraint when available. Entrypoint target `references` edges carry their own confidence: direct manifest paths such as Cargo target files and Composer binaries are exact, syntax-level function matches are syntactic, and command parsing or Python module-to-path mapping is heuristic. Edge queries can filter by confidence, and the web UI displays confidence and provenance metadata on edge rows. Dependency consistency insights compare syntactic external imports with declared package nodes, report likely undeclared package usage, and warn when the same manifest package is declared with conflicting constraints. The web UI loads insight reports from server analysis, so project-wide findings remain visible while the canvas displays a paged graph slice. Future LSP, framework, and compiler integrations should upgrade resolvable code edges to `semantic` or `exact`.
