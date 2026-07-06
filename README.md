# CodeGraph

CodeGraph is a Rust-based code exploration tool for people and agents.

The project goal is to turn source code into a typed knowledge graph that can be inspected from a graphical UI, queried from a CLI, and consumed as structured JSON by automation.

## Current Status

This repository is at the bootstrap stage.

Implemented now:

- Rust workspace layout.
- Core graph model.
- Filesystem scanner with default build/vendor ignore rules.
- Tree-sitter based syntax extraction for Rust, Python, JavaScript, TypeScript, TSX, Go, C, C++, PHP, and Bash.
- Function, type/class, module/namespace, import/include, and entrypoint candidate nodes.
- CLI command that emits graph JSON.

Planned next:

- Approximate call edges.
- Config, environment, and error-flow extraction.
- Graph query commands.
- HTTP API for web usage.
- Graph queries for humans and agents.
- Modern web interface built on top of the same graph API.

## Usage

Run the initial scanner:

```bash
cargo run -p codegraph-cli -- scan .
```

Include hidden paths:

```bash
cargo run -p codegraph-cli -- scan . --include-hidden
```

Include default ignored paths such as `target` and `node_modules`:

```bash
cargo run -p codegraph-cli -- scan . --include-ignored
```

The output is JSON using the shared graph schema from `codegraph-core`.

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

## Workspace

```text
crates/
  codegraph-core/      graph schema and shared domain types
  codegraph-parser/    Tree-sitter syntax extraction
  codegraph-indexer/   project scanning and graph construction
  codegraph-cli/       command-line interface
```

Expected future crates:

```text
crates/
  codegraph-lsp/       optional LSP enrichment
  codegraph-server/    HTTP/WebSocket API for UI and agents
  codegraph-web/       browser-based graph interface
  codegraph-ui/        optional Tauri desktop shell
```

## Design Principles

- The CLI and UI must use the same graph model.
- Agent-facing output must be stable, structured, and documented.
- Every extracted fact should carry a confidence level.
- Language support should degrade gracefully from semantic to syntactic to heuristic analysis.
- The project should prefer proven language tooling such as Tree-sitter and LSP servers instead of inventing parsers from scratch.
