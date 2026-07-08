# Graphify Feature Parity Analysis

Date: 2026-07-08

Source reviewed: https://github.com/Graphify-Labs/graphify

This document maps the main Graphify ideas to CodeGraph implementation targets. The goal is not to clone Graphify feature-for-feature, but to make CodeGraph cover the same core jobs: local-first repository mapping, queryable graph memory, human exploration, and agent handoff.

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
| Local AST code graph | Implemented for the current supported language set | Continue expanding language adapters while preserving typed facts and provenance |
| Confidence tags | Implemented as `exact`, `semantic`, `syntactic`, `heuristic`, `unknown`, with Markdown report wording for extracted/resolved/inferred/ambiguous evidence | Keep wording aligned as new confidence sources are added |
| Interactive graph | Implemented in web UI with graph cards, filters, focused slices, source preview, workflows | Add community and hotspot overlays directly into the UI graph/card flows |
| Query/path/explain | Implemented across CLI/API/web | Add natural-language-to-bounded-subgraph mode without vector storage as the default dependency |
| God nodes and communities | First community reports exist; hotspot reports exist | Separate architectural hubs from noisy utility hubs and expose both in reports/UI |
| GRAPH_REPORT-style summary | Implemented through `codegraph report --format markdown --output CODEGRAPH_REPORT.md` with compact file summaries, key concepts, communities, risks, surprising links, suggested questions, and provenance | Add saved investigation memory and reflection reports next |
| Rationale and doc refs | Source rationale comments plus first Markdown/ADR/RFC section, local path, and symbol references are indexed | Add richer document citations, backlinks, and UI overlays that connect docs, decisions, and code cards |
| Beyond code | First-class manifests/config, deterministic Markdown docs, first SQL schema facts, source SQL query-string links, SQL graph slices, and missing SQL table insights are indexed | Add deeper SQL semantics, ORM links, and migration insights next, then optional model-backed PDFs/images/media sidecars |
| Wiki/agent-readable docs | Not yet implemented | Export a Markdown wiki with one index plus pages for communities, hotspots, entrypoints, and important nodes |
| Assistant install/hooks | Not yet implemented | Generate project-scoped Codex/generic agent instructions that prefer CodeGraph query/path/explain before broad raw-file reads |
| MCP server | Planned | Expose query, path, explain, node card, report, and insight tools over stdio first |
| Watch and git hooks | Planned | Add local watch mode plus post-commit/post-checkout refresh/export hooks |
| Graph merge/team graph | Planned | Merge graph artifacts from project/docs/incidents/external systems with stable provenance and conflict handling |
| External graph exports | DOT/NDJSON exist | Add GraphML, SVG, Mermaid/callflow HTML, Obsidian/Markdown wiki, Neo4j Cypher, and FalkorDB |
| PR impact dashboard | Planned | Use changed files, communities, CI/review state, and shared subsystem overlap to rank merge risk |
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
- MCP stdio server with `query_graph`, `get_node_card`, `get_neighbors`, `shortest_path`, `workflow`, `insights`, and `report`
- query/result logging with local privacy controls

### P2: Docs, SQL, And Wiki

Expand the graph from code into repository knowledge:

- Markdown, ADR, RFC, and wiki-link extraction
- deterministic SQL schema extraction and app-to-schema links
- Markdown wiki export for communities, entrypoints, hotspots, config flows, and risky insights
- GraphML/SVG/Mermaid/Neo4j/FalkorDB export targets

### P3: Updates, Merge, And PR Impact

Support multi-agent and team workflows:

- watch mode for local refresh
- git hooks for post-commit/post-checkout refresh and export regeneration
- graph merge with source provenance
- PR impact dashboards from changed files, communities, review/CI status, conflicts, and shared hotspots

### P4: Optional Non-Code Semantic Ingestion

Add opt-in ingestion for material that cannot be parsed deterministically:

- plain text and Markdown first, fully local
- PDFs and Office documents through deterministic text extraction when available
- image/audio/video sidecars with explicit configured-model or transcript inputs only
- strict URL/path validation, redirect blocking, size/time limits, label sanitization, and graph-path constraints

## Compatibility Principles

- Code-only scans must remain fully local and deterministic.
- Optional model-backed extraction must be explicit, isolated, and provenance-marked.
- Every generated report claim must reference graph node ids, edge indexes, source spans, or insight ids.
- Agent workflows should prefer bounded graph slices over reading whole repositories.
- The same graph model must power CLI, API, web, desktop, and MCP.
- Confidence wording should be user-friendly, but the typed Rust schema remains the source of truth.
