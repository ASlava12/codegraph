# Next Features Roadmap

This is the near-term execution plan on top of `../ROADMAP.md`. The main
roadmap tracks everything ever planned; this file orders the next concrete
features into milestones so work can continue one focused commit at a time.

Priorities follow `GRAPHIFY_PARITY.md`: finish the in-flight workflow surface
first, then close the highest-leverage parity gap (agent interfaces), then
team workflows, exports, and deeper knowledge ingestion.

Each checkbox is intended to be one commit: implementation, tests, README and
ROADMAP updates together.

## Milestone 1: Workflow Surface Completion

Goal: close the remaining Phase 3 workflow items so block diagrams are a
finished, regression-guarded feature for humans and agents.

- [ ] Add a `return` workflow block kind classified from function exits where parser facts support it, wired through block-kind filters, API schema enums, and web suggestions.
- [ ] Add workflow regression fixtures for Rust, Python, JavaScript/TypeScript, Go, PHP, Bash, and Dart runtime paths with expected block/transition snapshots.
- [ ] Add workflow regression fixtures for CI, Docker, and Kubernetes entrypoint kinds using the new `entrypoint_kind` filters.
- [ ] Add a full web Flow view next to the graph canvas with pan, zoom, minimap, and selectable workflow blocks.
- [ ] Reuse node and dependency cards from selected workflow blocks, including source preview, related risks, and edge explanations.
- [ ] Localize workflow block labels, commands, and schema descriptions in English and Russian.

Exit criteria:

- Workflow output shape is guarded by fixtures across all supported languages and runtime surfaces.
- A human can explore a workflow visually without leaving the web UI.

## Milestone 2: Agent Interfaces (highest parity leverage)

Goal: let coding agents use CodeGraph before broad file reads, as persistent
repository memory rather than a fresh index.

- [ ] Add MCP stdio server mode exposing `query_graph`, `get_node_card`, `get_neighbors`, `shortest_path`, `workflow`, `insights`, and `report` tools over the existing analysis APIs.
- [ ] Add an `install-agent` command that generates project-scoped guidance files (AGENTS.md/CLAUDE.md snippets, `.mcp.json` entries) nudging agents to query CodeGraph first.
- [ ] Add saved query/result memory with outcomes (`useful`, `dead_end`, `corrected`) linked to graph node ids and invalidated by cache fingerprints.
- [ ] Add reflection reports that aggregate saved investigation outcomes into repository lessons with provenance and stale-source warnings.
- [ ] Add query logging with privacy controls and local JSONL audit output.
- [ ] Add optional authenticated HTTP MCP transport reusing the existing bearer-token protection.

Exit criteria:

- An external assistant can query the graph through MCP without shelling out to the CLI.
- Investigation outcomes survive between sessions and go stale safely when sources change.

## Milestone 3: Updates And Team Workflows

Goal: keep graphs fresh automatically and usable across repositories.

- [ ] Add watch mode for automatic local graph refresh while editing, reusing the incremental scan machinery.
- [ ] Add git hooks for post-commit/post-checkout incremental refresh and optional export regeneration.
- [ ] Add a global graph registry for multiple local repositories with cross-project path and query support.
- [ ] Add graph merge commands with source provenance and a conflict-safe strategy for committed graph artifacts.

Exit criteria:

- Re-scans happen without manual commands during normal development.
- Two repositories can be queried through one registry.

## Milestone 4: Exports And Dashboards

Goal: make graph knowledge portable into external tools.

- [ ] Add GraphML export.
- [ ] Add Mermaid/callflow HTML export for full graphs and workflow sets.
- [ ] Add Obsidian vault / Markdown wiki export for communities, entrypoints, hotspots, config flows, and risky insights.
- [ ] Add Neo4j Cypher and FalkorDB export targets.
- [ ] Add a PR impact dashboard from changed files, graph communities, CI/review state, and shared hotspots.
- [ ] Add a benchmark harness for token/context savings and graph-query recall on real mixed corpora.

Exit criteria:

- Graph artifacts open in at least one external graph tool and one wiki tool without conversion.

## Milestone 5: Deeper Knowledge Ingestion

Goal: extend deterministic extraction where it is still shallow.

- [ ] Resolve Dart `.dart_tool/package_config.json` package maps and generated-file conventions.
- [ ] Validate Dart semantic enrichment patches from the Dart analysis server, with parser/semantic cache invalidation for `pubspec.yaml` and generated files.
- [ ] Match Dart/Flutter platform-channel declarations to native Android/iOS handler implementations.
- [ ] Add deeper SQL query extraction: JOIN relationship semantics, migration ordering, and broader schema consistency insights.
- [ ] Link application code to SQL/schema nodes through migrations, ORM metadata, and database config.
- [ ] Index MCP configuration files (`.mcp.json`, `mcp.json`, `mcp_servers.json`, assistant desktop configs) as tool/server dependency facts.
- [ ] Add richer Markdown citations, backlinks, front matter, ownership metadata, and docs-to-code UI overlays.

Exit criteria:

- Dart/Flutter repositories reach the same fact depth as Rust/Go/TypeScript.
- SQL and docs facts explain not only what exists but how it is connected.

## Working Agreement

- One checkbox per commit, with tests and doc updates in the same commit.
- `cargo fmt --all --check`, `cargo test --workspace`, web JS checks, and a live CLI smoke run gate every commit.
- When a checkbox lands here, mirror the matching item in `../ROADMAP.md`.
