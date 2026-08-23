# Graphify Feature Parity Analysis

Date: 2026-07-08

Source reviewed: https://github.com/Graphify-Labs/graphify

This document maps the main Graphify ideas to CodeGraph implementation targets. The goal is not to clone Graphify feature-for-feature, but to make CodeGraph cover the same core jobs: local-first repository mapping, queryable graph memory, human exploration, and agent handoff.

## Upstream Snapshot

Reviewed against the public `Graphify-Labs/graphify` `v8` README and architecture notes on 2026-07-08.

Current Graphify positions itself as an assistant-installed repository memory layer:

- `graphify-out/graph.json` is the persistent graph artifact.
- `graphify-out/graph.html` is a static clickable graph for humans.
- `GRAPH_REPORT.md` summarizes god nodes, communities, surprising links, rationale, confidence, and suggested questions.
- CLI workflows include graph build/update, `query`, `path`, `explain`, `add`, `watch`, `hook`, `merge-graphs`, `prs`, MCP serving, wiki, Obsidian, SVG, GraphML, Neo4j, FalkorDB, and callflow HTML exports.
- Code extraction is local and deterministic through Tree-sitter; docs and media are optional semantic/model-backed ingestion paths with explicit backend configuration.
- Assistant integrations generate query-first instructions and hooks for Codex, Claude Code, Cursor, Gemini CLI, Copilot, and other tools.
- MCP serving exposes graph tools for repeated assistant access, including query, node, neighbor, path, PR, and impact workflows.
- Team workflows commit graph artifacts, install git hooks, use merge drivers for graph conflict handling, and can run a shared HTTP MCP server.
- Privacy/security constraints matter: query logging is configurable, external ingestion is validated, code-only scans stay offline, and shared HTTP service requires authentication when exposed.

## Core Ideas To Preserve

Graphify's main product shape is a persistent knowledge graph artifact with several synchronized surfaces:

- a graph JSON artifact for repeatable querying
- an interactive graph view for humans
- report output with key concepts, communities, surprising links, rationale, and suggested questions
- command-line `query`, `path`, and `explain` workflows
- confidence labels that distinguish explicit extracted facts from inferred or ambiguous facts
- deterministic local code parsing, with optional model-backed extraction only for non-code media
- assistant integration through installable guidance, hooks, and MCP access
- update/cache/watch workflows so agents do not re-read the same repository from scratch
- graph exports for external tools and team workflows

CodeGraph already has the right foundation: Rust workspace, typed graph schema, Tree-sitter parser layer, CLI/API/web surfaces, node cards, path/query/explain workflows, edge confidence/provenance, incremental cache, workflow diagrams, insight checks, and a local desktop wrapper. The remaining work is mostly expanding the graph beyond source code and turning analysis into durable knowledge artifacts.

## Feature Gap Map

| Graphify idea | Current CodeGraph state | Target state |
| --- | --- | --- |
| Local AST code graph | Implemented for 20 languages (Rust, Python, JS/TS/TSX, Go, C, C++, Dart, PHP, Bash, Ruby, Java, C#, Kotlin, Swift, Scala, Lua, Elixir, Zig) | Continue expanding language adapters while preserving typed facts and provenance |
| Confidence tags | Implemented as `exact`, `semantic`, `syntactic`, `heuristic`, `unknown`, with Markdown report wording for extracted/resolved/inferred/ambiguous evidence | Keep wording aligned as new confidence sources are added |
| Interactive graph | Implemented in web UI with graph cards, filters, focused slices, source preview, workflows, and compact workflow/callflow blocks | Add community and hotspot overlays directly into the UI graph/card flows |
| Query/path/explain | Implemented across CLI/API/web, including deterministic English/Russian natural-language `ask` mapping to bounded graph slices without vector storage | Add saved query sessions and MCP tool wrappers next |
| God nodes and communities | First community reports exist; hotspot reports exist | Separate architectural hubs from noisy utility hubs and expose both in reports/UI |
| GRAPH_REPORT-style summary | Implemented through `codegraph report --format markdown --output CODEGRAPH_REPORT.md` with compact node/file summaries, key concepts, communities, risks, surprising links, suggested questions, and provenance | Add saved investigation memory and reflection reports next |
| Rationale and doc refs | Source rationale comments plus first Markdown/ADR/RFC section, local path, and symbol references are indexed | Add richer document citations, backlinks, and UI overlays that connect docs, decisions, and code cards |
| Beyond code | First-class manifests/config, deterministic Markdown docs, first SQL schema facts, source SQL query-string links, SQL graph slices, and missing SQL table insights are indexed | Add deeper SQL semantics, ORM links, and migration insights next, then optional model-backed PDFs/images/media sidecars |
| Wiki/agent-readable docs | Not yet implemented | Export a Markdown wiki with one index plus pages for communities, hotspots, entrypoints, and important nodes |
| Assistant install/hooks | Not yet implemented | Generate project-scoped Codex/generic agent instructions that prefer CodeGraph query/path/explain before broad raw-file reads |
| MCP server | Planned | Expose query, path, explain, node card, report, insight, workflow, and PR-impact tools over stdio first, then optional authenticated HTTP |
| Watch and git hooks | Planned | Add local watch mode plus post-commit/post-checkout refresh/export hooks |
| Graph merge/team graph | Planned | Merge graph artifacts from project/docs/incidents/external systems with stable provenance and conflict handling |
| External graph exports | DOT/NDJSON exist | Add GraphML, SVG, Mermaid/callflow HTML, Obsidian/Markdown wiki, Neo4j Cypher, and FalkorDB |
| PR impact dashboard | Planned | Use changed files, communities, CI/review state, and shared subsystem overlap to rank merge risk |
| Work memory and reflections | Planned | Store useful/dead-end/corrected query outcomes, aggregate lessons by community/node, and mark lessons stale when source changes |
| MCP config/package manifest graph facts | Partially covered through manifests and runtime configs | Index MCP server configs, tool package refs, env requirements, and canonical package hubs across more manifest formats |
| Benchmarks | Local scan benchmark exists | Add graph-query recall and context/token-saving benchmarks for mixed corpora |

## Implementation Priorities

### P0: Repository Knowledge Artifact

Implemented first durable report and query artifact for code-only repositories:

- `codegraph report . --format markdown --output CODEGRAPH_REPORT.md`
- key concepts, top communities, hotspots, risky insights, surprising cross-area links, and suggested questions
- report sections backed by graph node ids and edge indexes so agents can jump to exact evidence
- UI entry point for the same report data

This gives humans and agents the biggest Graphify-like benefit without adding external ingestion risk.

### P1: Agent Interfaces

Make CodeGraph easy for coding agents to use before broad file reads:

- `codegraph install-agent --platform codex|agents --project`
- optional local hooks that suggest graph queries before raw repository scans
- MCP stdio server with `query_graph`, `get_node_card`, `get_neighbors`, `shortest_path`, `workflow`, `insights`, `report`, `list_prs`, `get_pr_impact`, and `triage_prs`
- query/result logging with local privacy controls
- saved investigation outcomes plus reflection reports so the graph becomes reusable project memory, not only a fresh index

### P2: Docs, SQL, And Wiki

Expand the graph from code into repository knowledge:

- Markdown, ADR, RFC, and wiki-link extraction
- deterministic SQL schema extraction and app-to-schema links
- MCP config extraction for `.mcp.json`, `mcp.json`, `mcp_servers.json`, and assistant desktop config files
- broader package manifest canonicalization so repeated package names collapse into shared package hubs
- Markdown wiki export for communities, entrypoints, hotspots, config flows, and risky insights
- GraphML/SVG/Mermaid/Neo4j/FalkorDB export targets

### P3: Updates, Merge, And PR Impact

Support multi-agent and team workflows:

- watch mode for local refresh
- git hooks for post-commit/post-checkout refresh and export regeneration
- graph merge with source provenance
- merge-driver support or an equivalent conflict-safe strategy for committed graph artifacts
- PR impact dashboards from changed files, communities, review/CI status, conflicts, and shared hotspots
- optional shared authenticated HTTP MCP/API deployment for team graph access

### P4: Optional Non-Code Semantic Ingestion

Add opt-in ingestion for material that cannot be parsed deterministically:

- plain text and Markdown first, fully local
- PDFs and Office documents through deterministic text extraction when available
- image/audio/video sidecars with explicit configured-model or transcript inputs only
- strict URL/path validation, redirect blocking, size/time limits, label sanitization, and graph-path constraints

## Readiness Assessment

CodeGraph can implement the main Graphify ideas with the current architecture. The core graph model, parser/indexer split, analysis crate, CLI/API/web surfaces, desktop launcher, provenance model, and cache layer already map well onto Graphify's pipeline of detect, extract, build graph, cluster, analyze, report, export, serve, watch, and benchmark.

The biggest remaining work is not architectural feasibility; it is product completion:

- agent installation and MCP are the highest-leverage parity gaps because they make CodeGraph usable by coding agents before raw file reads
- wiki/Obsidian/GraphML/Neo4j/FalkorDB exports are straightforward graph serializers once the target schemas are chosen
- watch/hooks/merge are operational glue around the existing cache and incremental scan machinery
- saved query outcomes and reflection reports need a small persistent memory schema plus invalidation against graph/cache fingerprints
- optional docs/media ingestion must stay behind explicit configuration and strict provenance so code-only Rust scans remain deterministic and offline
- Graphify's breadth across around 40 grammars can be approached incrementally; CodeGraph should prioritize deeper, typed facts for current supported languages before chasing every extension

## Compatibility Principles

- Code-only scans stay fully local. They are deterministic by default in the
  sense that matters for CI: the syntactic graph depends only on the sources.
  Since 2026-08 a scan additionally runs a semantic pass when a matching
  language server is installed, which trades bit-for-bit reproducibility across
  machines for accuracy. That pass is always labeled on the root node
  (`semantic_enrichment`, `semantic_servers`), its edges carry
  `confidence: semantic`, and `--no-semantic` (CLI) / `--no-semantic` (server)
  restores a machine-independent graph — which is what CI must use.
- Optional model-backed extraction must be explicit, isolated, and provenance-marked.
- Every generated report claim must reference graph node ids, edge indexes, source spans, or insight ids.
- Agent workflows should prefer bounded graph slices over reading whole repositories.
- The same graph model must power CLI, API, web, desktop, and MCP.
- Confidence wording should be user-friendly, but the typed Rust schema remains the source of truth.
