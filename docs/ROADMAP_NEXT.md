# Next Features Roadmap

This is the near-term execution plan on top of `../ROADMAP.md`. The main
roadmap tracks everything ever planned; this file orders the next concrete
features into milestones so work can continue one focused commit at a time.

Priorities: finish the in-flight workflow surface first because it is the
foundation for execution journeys, then build the refactoring journey product
(ROADMAP Phase 8), then close the highest-leverage parity gap (agent
interfaces), then team workflows, exports, and deeper knowledge ingestion.

Each checkbox is intended to be one commit: implementation, tests, README and
ROADMAP updates together.

## Milestone 1: Workflow Surface Completion

Goal: close the remaining Phase 3 workflow items so block diagrams are a
finished, regression-guarded feature for humans and agents.

- [x] Add a `return` workflow block kind classified from function exits where parser facts support it, wired through block-kind filters, API schema enums, and web suggestions.
- [x] Add workflow regression fixtures for Rust, Python, JavaScript/TypeScript, Go, PHP, Bash, and Dart runtime paths with expected block/transition snapshots.
- [x] Add workflow regression fixtures for CI, Docker, and Kubernetes entrypoint kinds using the new `entrypoint_kind` filters.
- [x] Add a full web Flow view next to the graph canvas with pan, zoom, minimap, and selectable workflow blocks.
- [x] Reuse node and dependency cards from selected workflow blocks, including source preview, related risks, and edge explanations.
- [x] Localize workflow block labels and filter summaries in English and Russian (CLI help and API schema descriptions stay English as agent-facing contracts).

Exit criteria:

- Workflow output shape is guarded by fixtures across all supported languages and runtime surfaces.
- A human can explore a workflow visually without leaving the web UI.

## Milestone 2: Execution Journeys And Refactoring Intelligence

Goal: the core refactoring product from ROADMAP Phase 8 — follow one
execution flow from an entrypoint to a chosen target as an ordered chain of
events, understand component dependencies, see potential problems along the
way, and drill into the implementation of any step. Depends on the Milestone 1
Flow view and block-to-card actions.

- [x] Add target-directed journey reports (CLI `journey --from <entrypoint> --to <target>`, API `/api/journey`) that expand entrypoint-to-target paths into step-numbered execution chains built from workflow blocks.
- [x] Rank alternative journey paths by edge confidence and length, and attach per-hop provenance explanations for why each transition exists.
- [x] Add journey risk summaries: risky steps, unresolved or ambiguous calls, low-confidence hops, and cycles crossing the flow, with fragile-transition flags for refactor planning.
- [x] Add component dependency reports grouping a node's incoming/outgoing dependencies by architecture area, package, and language, plus contract views for the exact edges between two selected components.
- [ ] Add journey step drill-down: expand a step into a nested sub-flow with breadcrumbs back to the parent journey, and open node/dependency cards and source previews from steps.
- [ ] Add a web journey view: pick start and target, read the step-numbered chain with expandable branches, and jump between journey, graph, and cards.
- [ ] Add blast-radius reports for a selected node (CLI `impact`, API `/api/impact`): dependents, affected entrypoints/routes/tests, and a risk-weighted impact score.
- [ ] Add coupling/seam reports that suggest boundaries where extraction is safest and where it is most needed.
- [ ] Add a machine-readable refactor context bundle combining journey, dependencies, risks, and source spans for one-shot agent handoff.

Exit criteria:

- A human can pick an entrypoint and a target and read the program's path between them as a chain of events, expanding any step without losing context.
- Fragile hops and risky steps are visible before a refactor starts.
- An agent can request one bundle with enough context to plan a refactor without raw repository reads.

## Milestone 3: Agent Interfaces (highest parity leverage)

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

## Milestone 4: Updates And Team Workflows

Goal: keep graphs fresh automatically and usable across repositories.

- [ ] Add watch mode for automatic local graph refresh while editing, reusing the incremental scan machinery.
- [ ] Add git hooks for post-commit/post-checkout incremental refresh and optional export regeneration.
- [ ] Add a global graph registry for multiple local repositories with cross-project path and query support.
- [ ] Add graph merge commands with source provenance and a conflict-safe strategy for committed graph artifacts.

Exit criteria:

- Re-scans happen without manual commands during normal development.
- Two repositories can be queried through one registry.

## Milestone 5: Exports And Dashboards

Goal: make graph knowledge portable into external tools.

- [ ] Add GraphML export.
- [ ] Add Mermaid/callflow HTML export for full graphs and workflow sets.
- [ ] Add Obsidian vault / Markdown wiki export for communities, entrypoints, hotspots, config flows, and risky insights.
- [ ] Add Neo4j Cypher and FalkorDB export targets.
- [ ] Add a PR impact dashboard from changed files, graph communities, CI/review state, and shared hotspots.
- [ ] Add a benchmark harness for token/context savings and graph-query recall on real mixed corpora.

Exit criteria:

- Graph artifacts open in at least one external graph tool and one wiki tool without conversion.

## Milestone 6: Deeper Knowledge Ingestion

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

## Milestone 7: Feature Audit, Completion, And Usability

Goal: ROADMAP Phase 9 — walk through every shipped feature, finish or
explicitly close what is incomplete, and make existing features convenient
rather than merely present.

- [ ] Audit checked roadmap items end-to-end across CLI, API, and web; file every gap found as a new unchecked item in its phase.
- [ ] Sweep all phases for remaining unchecked items and finish, re-scope, or drop each with a recorded reason.
- [ ] Verify CLI/API/web feature parity and close or document intentional surface-specific gaps.
- [ ] Dogfood CodeGraph on itself and fix what its own reports make hard to understand.
- [ ] Unify CLI ergonomics: consistent flags, defaults, `--help` examples, actionable errors.
- [ ] Improve web discoverability: onboarding hints, explanatory empty states, sane default filters for large graphs.
- [ ] Add task-oriented guides (investigate a bug, trace a config value, plan a refactor).
- [ ] Make agent-facing outputs self-describing with schema examples and copy-paste-ready snippets.

Exit criteria:

- Nothing is silently half-implemented; every item is done, re-scoped, or dropped with a reason.
- Every major feature is reachable from `--help` and the web UI without reading source.

## Milestone 8: Code Refactoring And Internal Quality

Goal: ROADMAP Phase 10 — pay down structural debt behind green tests.

- [ ] Split monolithic files into focused modules: `codegraph-analysis/src/lib.rs`, `codegraph-indexer/src/lib.rs`, `codegraph-parser/src/lib.rs`, `codegraph-server/src/main.rs`, `codegraph-web/static/app.js`.
- [ ] Fix all clippy warnings on current stable and keep `-D warnings` green in CI without allow-listing.
- [ ] Extract shared report/filter/paging helpers and request structs to remove CLI/API/web duplication and long parameter lists.
- [ ] Add module-level docs and tighten crate public APIs to intended surfaces.
- [ ] Add regression fixtures first wherever a planned refactor lacks coverage, then land the refactor behind green workspace tests.

Exit criteria:

- No single source file dominates a crate; modules map to feature areas.
- Clippy is clean on stable and enforced in CI.

## Working Agreement

- One checkbox per commit, with tests and doc updates in the same commit.
- `cargo fmt --all --check`, `cargo test --workspace`, web JS checks, and a live CLI smoke run gate every commit.
- When a checkbox lands here, mirror the matching item in `../ROADMAP.md`.
