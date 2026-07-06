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

The initial implementation only indexes directories and files. Future implementations should merge syntax and semantic facts from parser and LSP layers.

### Parser

`codegraph-parser` extracts syntax-level code facts with Tree-sitter.

Responsibilities:

- language detection
- Tree-sitter parser management
- syntax-level graph extraction
- approximate call-site extraction with parent function context
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
- constrain scan paths to a configured root by default
- serve the static web application
- keep UI and agent clients on the same JSON graph model

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

Current call edges are `heuristic`: they are resolved by syntactic function names and simple-name matching. Future LSP and compiler integrations should upgrade resolvable edges to `semantic` or `exact`.
