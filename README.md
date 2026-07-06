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
- Manifest-defined entrypoints from Cargo, npm, Python, and Composer project metadata.
- Resolved manifest entrypoint targets for common file paths, command paths, and Python module callables.
- Approximate `calls` edges between functions when syntax-level names can be resolved.
- Manifest dependency extraction from Cargo, npm, Go, Python, and Composer projects.
- Heuristic config reads, environment reads, and potential error/exception constructs.
- CLI command that emits graph JSON.
- HTTP API and embedded web UI for interactive graph exploration.
- Async scan job API for long-running repository scans.
- SSE scan job status stream for live web progress updates.
- Source preview API and UI panel for graph nodes with source spans.
- Interactive UI trace panel for following outgoing dependency subgraphs from a selected node.
- Agent-friendly summary, entrypoint, and trace commands/endpoints.
- Agent-friendly graph query command and API for focused node, edge, call, dependency, and trace slices.
- Path queries for finding directed dependency paths between labels or node ids.
- Confidence-aware edge queries and UI edge labels for fact provenance.
- Server-side graph paging and filtering endpoint for large repository exploration.
- Web graph page controls backed by server-side paging, search, kind, item, language, edge, and confidence filters.
- Web project overview for language mix, edge confidence mix, and entrypoint launch points.
- Web path navigation for finding, focusing, and visually highlighting dependency paths between graph nodes.
- Node context API and detail-panel neighbor loading for paged graph exploration.
- Server-backed web insights for project-wide findings while browsing paged graph slices.
- Insight focus API and web interaction for turning findings into focused graph views.
- Web query panel for running focused graph queries, narrowing the canvas to query results, and jumping to matching nodes.
- DOT/Graphviz and NDJSON export formats for visualization and streaming agent use.
- Persistent server-side graph cache with project fingerprint invalidation.
- CI checks for formatting, clippy, tests, UI syntax, CLI scan, and server cache smoke tests.
- Investigation insights for unresolved calls, parse errors, duplicate labels, orphan functions, and error-flow facts.
- Dependency consistency insights for external imports that are not backed by declared manifest dependencies.
- Dependency consistency insights for package declarations with conflicting manifest constraints.
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

Export for Graphviz or streaming agent use:

```bash
cargo run -p codegraph-cli -- scan . --format dot
cargo run -p codegraph-cli -- scan . --format ndjson
```

Summarize a project:

```bash
cargo run -p codegraph-cli -- summary .
```

List entrypoint candidates:

```bash
cargo run -p codegraph-cli -- entrypoints .
```

List investigation insights:

```bash
cargo run -p codegraph-cli -- insights .
```

Query focused graph slices:

```bash
cargo run -p codegraph-cli -- query 'nodes kind:function label:main' .
cargo run -p codegraph-cli -- query 'edges kind:calls source:main' .
cargo run -p codegraph-cli -- query 'edges confidence:heuristic' .
cargo run -p codegraph-cli -- query 'calls(function:main)' .
cargo run -p codegraph-cli -- query 'trace label:main depth:3' .
cargo run -p codegraph-cli -- query 'path from:main to:load_config depth:6' .
```

Trace outgoing dependencies from a label:

```bash
cargo run -p codegraph-cli -- trace main . --depth 3
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

Scan API:

```bash
curl 'http://127.0.0.1:3765/api/scan?path=.'
```

The scan response includes `cache.status` as `hit`, `miss`, or `disabled`.

Export API:

```bash
curl 'http://127.0.0.1:3765/api/export?path=.&format=dot'
curl 'http://127.0.0.1:3765/api/export?path=.&format=ndjson'
```

Async scan job API:

```bash
curl -X POST 'http://127.0.0.1:3765/api/scan-jobs' \
  -H 'content-type: application/json' \
  -d '{"path":"."}'
curl 'http://127.0.0.1:3765/api/scan-jobs/scan-1'
curl -N 'http://127.0.0.1:3765/api/scan-jobs/scan-1/events'
curl 'http://127.0.0.1:3765/api/scan-jobs/scan-1/result'
```

Analysis APIs:

```bash
curl 'http://127.0.0.1:3765/api/graph?path=.&node_limit=250&kind=function'
curl 'http://127.0.0.1:3765/api/node-context?path=.&node_id=1&edge_limit=80'
curl 'http://127.0.0.1:3765/api/focus?path=.&node_ids=1,2&edge_indexes=0&edge_limit=200'
curl 'http://127.0.0.1:3765/api/summary?path=.'
curl 'http://127.0.0.1:3765/api/entrypoints?path=.'
curl 'http://127.0.0.1:3765/api/insights?path=.'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=nodes kind:function label:main'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=path from:main to:load_config depth:6'
curl 'http://127.0.0.1:3765/api/trace?path=.&label=main&depth=3'
```

`/api/graph` supports `node_offset`, `node_limit`, `edge_offset`,
`edge_limit`, `kind`, `search`, `language`, `item_kind`, `edge_kind`, and
`confidence`.
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
- Python: `requirements.txt`, `pyproject.toml`
- PHP/Composer: `composer.json`

Manifest dependencies are normalized into canonical package nodes with a stable
`package_id` metadata value such as `cargo:serde` or `python:fastapi`. Individual
manifest files connect to those package nodes with `depends_on` edges; the edge
metadata records whether the declaration is runtime, dev, optional, peer, or
build dependency data when the manifest format exposes that distinction, plus
the raw `dependency_version` constraint when the manifest declares one.

Manifest entrypoints are represented as `entrypoint` nodes linked from the
repository root with exact `entrypoint` edges. Examples include Cargo binaries,
npm scripts, Python project scripts, Composer scripts, and Composer binaries.
When a manifest target can be mapped back to code, the entrypoint node also
emits `references` edges with metadata such as `relation=entrypoint_file` or
`relation=entrypoint_function`; traces follow these edges before continuing into
regular call, import, config, environment, dependency, and error-flow edges.

## Workspace

```text
crates/
  codegraph-core/      graph schema and shared domain types
  codegraph-analysis/  summaries, entrypoints, and trace subgraphs
  codegraph-parser/    Tree-sitter syntax extraction
  codegraph-indexer/   project scanning and graph construction
  codegraph-storage/   persistent graph cache and project fingerprints
  codegraph-cli/       command-line interface
  codegraph-server/    HTTP API and embedded static web app
  codegraph-web/       browser UI assets
```

Expected future crates:

```text
crates/
  codegraph-lsp/       optional LSP enrichment
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
