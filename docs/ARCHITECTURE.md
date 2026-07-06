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

Future crates: `codegraph-server`, `codegraph-web`, and optionally `codegraph-ui`.

The server should expose graph queries to the web UI and to agents. The UI should visualize the graph without owning analysis logic. A Tauri shell can wrap the web UI later for desktop distribution.

## Confidence Model

Graph facts should declare how they were discovered:

- `exact`: produced by a compiler or exact project metadata.
- `semantic`: produced by LSP or a semantic analyzer.
- `syntactic`: produced from syntax trees.
- `heuristic`: inferred from naming, framework conventions, or patterns.
- `unknown`: legacy or unclassified fact.

This is central to the project. A useful imperfect graph is acceptable when uncertainty is explicit.
