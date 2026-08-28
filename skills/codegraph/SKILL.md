---
name: codegraph
description: Analyze local source-code repositories with the installed CodeGraph CLI and MCP server. Use as the first-choice structural tool for questions about architecture, symbols, dependencies, call/config/error flows, entrypoints, workflows, impact or blast radius, refactoring context, PR changes, risks, and locating relevant source before broad grep or full-file reading.
---

# CodeGraph

Use CodeGraph to obtain small, evidence-bearing graph slices from a repository. Keep the workflow local and deterministic; CodeGraph does not call an LLM.

## Start safely

1. Confirm `command -v codegraph` succeeds. If it does not, report that the global binary is unavailable; do not silently substitute a different graph tool.
2. Resolve the repository root to an absolute path.
3. Treat analysis requests as read-only. Add `--no-cache` to graph commands when the user did not authorize repository writes or explicitly said not to change the project.
4. Do not run `install-agent`, `install-hooks`, `watch`, `memory-save`, `registry-add`, `export-wiki`, or commands that redirect output into the repository unless the user explicitly requests that mutation.
5. Keep generated/vendor trees excluded by default. Add `--include-ignored` or `--include-hidden` only when the question requires them.

## Default investigation workflow

Begin with a bounded natural-language query:

```bash
repo_root=/absolute/path/to/repository
codegraph ask --compact 'Where is DATABASE_URL read?' "$repo_root" --no-cache
```

Then:

1. Read the response schema, resolved node ids, ambiguity/confidence fields, source spans, `cli_snippet`, and `suggested_commands`.
2. Follow up with node ids rather than labels when labels are ambiguous.
3. Use one focused command for the next question: `node-card`, `impact`, `journey`, `workflow`, `component-dependencies`, `component-contract`, or `refactor-context`.
4. Read only the cited source spans needed to confirm the conclusion. Use `source-search` or `rg` when graph evidence is incomplete.
5. State when an edge is heuristic or when the graph cannot resolve the question. Never turn a plausible graph path into a claim of certainty.

Prefer `ask --compact` or a narrow `query` over dumping `scan` JSON. Keep limits modest so the host agent spends less context on tool output.

## Choose the command

- Repository orientation: `summary`, `architecture`, `report --format markdown`, `entrypoints`, `communities`, or `hotspots`.
- Natural-language investigation: `ask --compact`.
- Precise graph slice: `query`.
- Symbol evidence and nearby source: `node-card --node-id ID`.
- Call or dependency path: `journey --from SOURCE --to TARGET`; use `workflow TARGET --compact` for a block flow.
- Change safety: `impact TARGET` or `refactor-context TARGET`; use `pr-impact --base REF` for a diff.
- Boundaries: `component-dependencies TARGET` and `component-contract --source AREA --target AREA`.
- Risks and unresolved evidence: `insights`; use `check` only when its nonzero quality-gate exit is intentional.
- Text fallback: `source-search QUERY --limit N --context N`; use `rg` for regex or file discovery.
- Assistant integration: `codegraph mcp ROOT` is a long-lived stdio server, not a one-shot shell query.

Read [commands.md](references/commands.md) for query forms, copy-paste examples, cache behavior, and mutation boundaries.

## Output discipline

- Prefer JSON fields and source locations over prose guesses.
- Bound `--depth`, `--limit`, `--edge-limit`, `--paths`, and source context.
- Use `jq` only to select fields after verifying the response schema; do not discard ambiguity, confidence, evidence, or truncation fields.
- Avoid rerunning several full scans when one returned `suggested_commands` identifies the next focused operation.
- Mention that host-model input/output still consumes tokens even though CodeGraph itself makes no model call.

## Persistent project integration

Only when the user asks to integrate CodeGraph into a repository, run:

```bash
codegraph install-agent /absolute/path/to/repository --platform all
```

Explain before running that this writes `.mcp.json` plus marker-delimited `AGENTS.md` and `CLAUDE.md` guidance. Add `--hooks` only when hooks were requested. Inspect existing files first and verify the resulting diff afterward.
