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
- Manifest-defined entrypoints from Cargo, npm, Go, Python, Composer, and CMake project metadata.
- Shebang-defined script entrypoints for Bash, Python, Node.js, and PHP scripts, including extensionless CLI files.
- Framework route entrypoints for common Python, JavaScript/TypeScript, Rust, Go, and PHP web route declarations.
- Resolved manifest entrypoint targets for common file paths, command paths, CMake executables, and Python module callables.
- Approximate `calls` edges between functions when syntax-level names can be resolved.
- Local import/include resolution for relative JavaScript/TypeScript imports and CommonJS requires, Python relative/absolute project imports, quoted C/C++ includes, PHP include/require paths, Bash source paths, and common Rust module paths.
- Manifest dependency extraction from Cargo, npm, Go, Python, and Composer projects.
- Heuristic config reads, environment reads, and potential error/exception constructs.
- CLI command that emits graph JSON.
- HTTP API and embedded web UI for interactive graph exploration.
- Async scan job API for long-running repository scans.
- SSE scan job status stream for live web progress updates.
- Source preview API and UI panel for parsed symbols plus framework route/config facts with source spans.
- Interactive UI trace panel for following outgoing dependency subgraphs from a selected node.
- Reverse dependency/dependent traces for impact analysis from CLI, API, query language, and web detail panels.
- Entrypoint trace API, CLI command, and web panel for comparing startup flows from manifest/code entrypoints.
- Config trace API, CLI command, and web panel for finding config/environment readers and entrypoint paths.
- Error trace API, CLI command, and web panel for following potential error/exception paths back to entrypoints.
- Agent-friendly summary, entrypoint, and trace commands/endpoints.
- Agent-friendly graph query command and API for focused node, edge, call, dependency, and trace slices.
- Edge explanation command, API, and web controls for confidence/provenance evidence.
- Path queries for finding directed dependency paths between labels or node ids.
- Confidence-aware edge queries and UI edge labels for fact provenance.
- Server-side graph paging and filtering endpoint for large repository exploration.
- Web graph page controls backed by server-side paging, search, kind, item, language, edge, confidence, relation, and source filters.
- Web graph viewport controls for zooming, fitting visible nodes, restarting layout, and pausing layout simulation.
- Web project overview for language mix, edge confidence/source/relation mix, and entrypoint launch points.
- Web path navigation for finding, focusing, and visually highlighting dependency paths between graph nodes.
- Node context API and detail-panel neighbor loading for paged graph exploration.
- Server-backed web insights for project-wide findings while browsing paged graph slices.
- Insight reports include severity and kind breakdowns for triage.
- Server-side insight filters for severity, kind, search, and capped agent/UI reads.
- Insight focus API and web interaction for turning findings into focused graph views.
- Web query panel for running focused graph queries, narrowing the canvas to query results, and jumping to matching nodes.
- Web project selector backed by an explicit server-side allowlist for opening local repositories.
- DOT/Graphviz and NDJSON export formats for visualization and streaming agent use.
- Persistent server-side graph cache with project fingerprint invalidation.
- Persistent CLI graph cache using the same project fingerprinting and cache records as the server.
- CLI scan benchmark reports with timing and graph-size metrics for regression tracking.
- CI checks for formatting, clippy, tests, UI syntax, CLI scan, and server cache smoke tests.
- Investigation insights for unresolved calls, parse errors, duplicate labels, orphan functions, and error-flow facts.
- Investigation insights for manifest entrypoints whose declared target cannot be resolved to a file or function.
- Investigation insights for framework routes whose named handler cannot be linked to a scanned function.
- Investigation insights for local imports/includes whose target file cannot be found.
- Investigation insights for config/environment reads that are not reachable from any detected entrypoint.
- Dependency consistency insights for external imports/CommonJS requires that are not backed by declared manifest dependencies.
- Dependency consistency insights for runtime manifest dependencies with no matching import.
- Dependency consistency insights for package declarations with conflicting manifest constraints.
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

Export for Graphviz or streaming agent use:

```bash
cargo run -p codegraph-cli -- scan . --format dot
cargo run -p codegraph-cli -- scan . --format ndjson
```

Summarize a project:

```bash
cargo run -p codegraph-cli -- summary .
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
```

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
cargo run -p codegraph-cli -- query 'edges confidence:heuristic' .
cargo run -p codegraph-cli -- query 'calls(function:main)' .
cargo run -p codegraph-cli -- query 'trace label:main depth:3' .
cargo run -p codegraph-cli -- query 'dependents label:load_config depth:3' .
cargo run -p codegraph-cli -- query 'neighbors label:main direction:out depth:2 edge_kind:calls' .
cargo run -p codegraph-cli -- query 'path from:main to:load_config depth:6' .
cargo run -p codegraph-cli -- query 'nodes metadata.annotation.domain:payments' .
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

Expose multiple local repositories to the web project selector by repeating
`--project`. Requests remain constrained to the configured roots unless
`--allow-any-path` is set explicitly:

```bash
cargo run -p codegraph-server -- --root . --project ../service-a --project ../tooling
```

Scan API:

```bash
curl 'http://127.0.0.1:3765/api/projects'
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
curl 'http://127.0.0.1:3765/api/entrypoint-traces?path=.&search=server&depth=4'
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
`edge_limit`, `kind`, `search`, `language`, `item_kind`, `edge_kind`,
`confidence`, `edge_relation`, and `edge_source`.
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
the raw `dependency_version` constraint when the manifest declares one. Cargo
`workspace = true` dependencies resolve to the root workspace constraint when
one exists; path-only workspace dependencies omit `dependency_version`.

Manifest entrypoints are represented as `entrypoint` nodes linked from the
repository root with exact `entrypoint` edges. Examples include Cargo binaries,
npm scripts, Python project scripts, Composer scripts, and Composer binaries.
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
