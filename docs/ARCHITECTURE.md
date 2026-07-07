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

`codegraph-indexer` walks project roots, applies ignore rules and scan budgets, invokes parsers, and assembles graph fragments.

It currently builds file, symbol, import, local import resolution, call, config, environment, error-flow, manifest entrypoint, script entrypoint, framework route entrypoint, custom rule violation, user annotation, and package dependency graph facts from syntax-level parser output, project manifests, framework conventions, `.codegraph/config.toml`, `.codegraph/rules.toml`, and `.codegraph/annotations.toml`. Repository scan policy supports ignored names and ignored project-relative globs so generated files, fixtures, and build artifacts can be excluded before parsing, source search, and cache fingerprinting. Explicit local import/include paths are resolved after the full scan so relative JavaScript/TypeScript imports and CommonJS requires, Python relative imports, quoted C/C++ includes, PHP include/require paths, Bash source paths, and common Rust module paths can point at files regardless of filesystem walk order. Python absolute imports, Go module-local imports, and C/C++ includes through CMake or `compile_commands.json` include directories are also marked local only when they resolve to scanned project files, preserving dependency warnings for unresolved external packages. Manifest dependencies are normalized by ecosystem-specific package identity, so multiple manifest files can point at the same external package node while preserving declaration kind and raw version constraint details on each edge. Cargo workspace dependency declarations resolve `workspace = true` through root `[workspace.dependencies]` when a version constraint is declared, and path-only workspace dependencies omit version metadata. Manifest entrypoints include Cargo, npm, Go, Python, Composer, and CMake `add_executable` declarations. Shebang entrypoints cover Bash, Python, Node.js, and PHP scripts, including extensionless CLI files whose parser language is inferred from the shebang. Files above the configured scan file-size budget are not read, but source and manifest files remain visible as `file` nodes with `skipped_reason=max_file_size` metadata so missing symbol facts are explainable. Framework route entrypoints cover common FastAPI/Flask-style Python decorators, Express-style JavaScript/TypeScript routes, Axum-style Rust routes, Go `HandleFunc`/router calls, and PHP route attributes. User annotations attach `annotation.*` metadata to matching nodes so repository-specific ownership, domain, criticality, and review context can travel through CLI, API, web, and agent graph queries. Manifest entrypoints are resolved after the full scan so path and function targets can be linked regardless of filesystem walk order. Future implementations should merge stronger semantic facts from LSP and compiler layers.

### Storage

`codegraph-storage` owns persistent graph cache records and project fingerprints.

The first implementation stores whole-graph JSON cache records keyed by root path and effective scan options, including repository config and the file-size budget. A fingerprint records scanned relative paths, file sizes, and modification times; a cache hit is only used when the current fingerprint matches the stored record. Cache diff reports compare the stored and current fingerprints so CLI, API, and web users can see added, removed, and modified files behind a miss. Graph-cache misses use a persistent per-file parser fact cache under the cache directory, keyed by language, relative path, file size, and modification time, so unchanged files can reuse parsed syntax facts while the graph is rebuilt. This is deliberately conservative: it accelerates repeated API and UI scans without yet promising partial graph reuse.

### Analysis

`codegraph-analysis` turns a full graph into focused artifacts:

- summary counts
- architecture maps that group files and dependency edges by top-level project area
- architecture dependency records preserve edge indexes so UI and agents can focus exact coupling evidence
- hotspot reports for finding high-degree files, functions, entrypoints, and config nodes
- scan coverage reports for indexed files, policy skips, large-file skips, and non-indexed files
- entrypoint candidates
- outgoing dependency traces
- incoming dependent traces for impact analysis
- investigation insights
- skipped large-file insights for scan coverage gaps
- insight severity and kind summaries for triage
- shared insight filtering for CLI, API, and web workflows
- severity-threshold check reports for CLI, API, web, CI, and agent gates
- edge explanations for confidence and provenance evidence
- manifest entrypoint target resolution checks
- local import/include resolution checks
- dependency consistency checks
- conflicting manifest dependency constraint checks
- duplicate framework route checks
- unresolved framework route handler checks
- framework config convention facts
- repository custom rule violation facts, including forbidden edge boundary checks
- user graph annotation facts
- entrypoint reachability checks for config and environment reads
- dependency cycle checks
- graph query results and directed path searches for agent and API clients
- insight focus subgraphs for findings with multiple nodes or edges
- project overview data for language mix, edge confidence/source/relation mix, annotations, and entrypoints
- graph slices for paged UI, search/item/language/confidence/relation/source-filtered exploration, and agent loading
- node context records for detail panels and focused agent reads
- DOT and NDJSON exports

This crate is shared by the CLI and server so humans, UI features, and agents receive the same structured answers.

### Parser

`codegraph-parser` extracts syntax-level code facts with Tree-sitter.

Responsibilities:

- built-in language adapter registry for parser discovery and future extension points
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
The CLI also owns local scan benchmark reports: it runs the same indexer path repeatedly and emits timing, graph size, and summary metrics without changing the graph schema.
Graph-reading CLI commands use the shared storage cache by default and expose `--no-cache` plus `--cache-dir` for deterministic runs.
The CLI also exposes cache fingerprint diff diagnostics without running a full graph scan.
The CLI check command emits machine-readable insight reports and exits non-zero when findings meet a configured severity threshold, making repository-specific rules and built-in consistency checks usable in CI.

### Server And Web UI

The web UI renders the current graph slice on a canvas with explicit viewport controls for zooming, fitting, resetting, and pausing force layout simulation.

`codegraph-server` exposes the graph over HTTP and serves the embedded browser UI from `codegraph-web`.

Responsibilities:

- provide health and scan endpoints
- expose repository-owned scan policy consistently across API, web, and source search workflows
- return effective scan options for humans and agents before they trigger a full scan
- reuse persistent graph cache records when project fingerprints match
- provide cache fingerprint diff endpoints and web controls for explaining cache invalidation
- provide JSON, DOT, and NDJSON export endpoints
- provide async scan job endpoints and SSE status streams for long-running scans
- provide summary, entrypoint, and trace endpoints
- provide entrypoint trace endpoints for startup flow comparison
- provide config trace endpoints for config/environment readers and upstream entrypoint paths
- provide error trace endpoints for potential error/exception sources and upstream entrypoint paths
- provide investigation insight endpoints with severity, kind, search, and limit filters
- provide severity-threshold check endpoints for quality gates
- provide graph query endpoints
- provide edge explanation endpoints for confidence and provenance evidence
- provide neighborhood query expansion for local incoming/outgoing graph context
- provide source search endpoints for compact text matches and source-preview handoff
- provide graph slice endpoints with server-side paging and filtering
- support path-prefix graph slices for architecture-area focus
- provide node context endpoints for paged detail exploration
- provide source preview endpoints for graph spans
- provide export endpoints for Graphviz DOT and NDJSON
- constrain scan paths to configured project roots by default
- expose configured local project roots to the web UI for project switching
- serve the static web application
- keep UI graph pages, query focus, path navigation/highlighting, trace, entrypoint trace, config trace, error trace, source search, cache diagnostics, insight checks, and agent clients on the same JSON graph model

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

Current call, config, environment, and error-flow facts are `heuristic`: they are resolved by syntax patterns and simple-name matching. Explicit local import/include file edges are `syntactic` because they come from source-level paths. Package manifest dependencies and manifest-defined entrypoints are `exact` because they come from declared project metadata; package nodes expose `package_id`, and `depends_on` edge metadata records the declaration kind and raw version constraint when available, including resolved Cargo workspace constraints. Entrypoint target `references` edges carry their own confidence: direct manifest paths such as Cargo target files, Go `main.go` files, Composer binaries, CMake executable source files, and shebang script files are exact, syntax-level function matches are syntactic, and command parsing or Python module-to-path mapping is heuristic. Entrypoint trace reports run the normal outgoing traversal for all matching startup nodes so humans and agents can compare startup flows without manually copying labels. Dependent traces run the same dependency edge rules in reverse, giving humans and agents an impact-analysis subgraph for callers, importers, config readers, and other incoming dependents. Neighborhood queries expand local incoming, outgoing, or bidirectional context around a node with edge kind and confidence filters, giving agents a compact subgraph without reading a full repository graph. Config traces use those existing graph facts to match `config` and `environment` targets, list direct readers, and find shortest known upstream entrypoint paths before the final read edge. Framework config convention facts add syntactic `config` nodes and `reads_config` edges for common settings files and framework-specific config calls, with metadata recording the framework and config kind. Error traces do the same for `may_error` edges, connecting potential error/exception constructs back to their direct source and upstream entrypoint path when one is known. Edge queries can filter by confidence, relation, source, and other metadata; the web UI displays confidence and provenance metadata on edge rows and exposes edge relation/source facets from the overview. Entrypoint resolution insights warn when a manifest-declared startup target cannot be linked to any scanned file or function. Dependency consistency insights compare syntactic external imports with declared package nodes, report likely undeclared package usage, flag runtime manifest dependencies with no matching import, and warn when the same manifest package is declared with conflicting constraints. Local import insights warn when an explicit local import/include path cannot be linked to a scanned file. Framework route insights warn when the same HTTP method/path is declared by multiple detected route entrypoints and when a route names a handler that cannot be linked to a scanned function. Config reachability insights warn when a config or environment read exists in a reader that is not reachable from any detected entrypoint. The web UI loads insight reports from server analysis, so project-wide findings remain visible while the canvas displays a paged graph slice. Future LSP, framework, and compiler integrations should upgrade resolvable code edges to `semantic` or `exact`.
