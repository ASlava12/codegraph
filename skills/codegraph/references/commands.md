# CodeGraph command reference

Use an absolute repository path. For ordinary read-only analysis, append `--no-cache` to graph-building commands. `source-search` reads source directly and has no cache option.

## Fast routing

```bash
codegraph summary /repo --no-cache
codegraph architecture /repo --group-limit 25 --edge-limit 80 --no-cache
codegraph report /repo --format markdown --insight-limit 30 --no-cache
codegraph ask --compact 'Who calls load_config?' /repo --no-cache
codegraph node-card --node-id n42 /repo --edge-limit 16 --source-context 5 --no-cache
codegraph impact n42 /repo --depth 6 --limit 30 --no-cache
codegraph refactor-context n42 /repo --from main --depth 8 --paths 3 --no-cache
codegraph journey --from main --to n42 /repo --depth 8 --paths 3 --no-cache
codegraph workflow n42 /repo --depth 5 --max-fanout 8 --compact --no-cache
codegraph component-dependencies n42 /repo --group-limit 15 --edge-limit 8 --no-cache
codegraph component-contract --source web --target crates /repo --edge-limit 60 --no-cache
codegraph pr-impact /repo --base origin/main --no-cache
codegraph insights /repo --severity warning --limit 30 --no-cache
codegraph source-search DATABASE_URL /repo --path-filter src --limit 20 --context 3
```

## Query forms

Use a quoted expression and add `--compact` when supported:

```bash
codegraph query 'nodes kind:function label:main' /repo --compact --no-cache
codegraph query 'edges kind:calls source:main' /repo --compact --no-cache
codegraph query 'calls(function:main)' /repo --compact --no-cache
codegraph query 'trace label:main depth:3' /repo --compact --no-cache
codegraph query 'dependents label:load_config depth:3' /repo --compact --no-cache
codegraph query 'path from:main to:load_config depth:6' /repo --compact --no-cache
codegraph query 'configs target:DATABASE_URL depth:6' /repo --compact --no-cache
codegraph query 'routes method:GET path:/health depth:3 edge_limit:100' /repo --compact --no-cache
codegraph query 'errors target:panic depth:6' /repo --compact --no-cache
codegraph query 'cycles edge_kind:calls' /repo --compact --no-cache
codegraph query 'unreachable kind:function label:legacy_worker' /repo --compact --no-cache
codegraph query 'insights severity:error kind:dependency' /repo --compact --no-cache
```

If a label resolves to several nodes, use the returned id in subsequent calls. Keep heuristic edges visible in the answer and verify important claims against source.

## Cache and writes

Without `--no-cache`, graph commands may create or update persistent cache data. Prefer `--no-cache` when the user requested analysis only. For repeated investigations, ask before enabling repository-local persistence or use a user-approved `--cache-dir` outside the repository.

These operations deliberately write state and require an explicit request:

- `install-agent`, `install-hooks`, `watch`, and `hook-run`
- `memory-save` and repository memory artifacts
- `registry-add` and `registry-remove`
- `export-wiki` and any `--output` or shell redirection into the repository
- `semantic-apply` output files and project-specific configuration/rules/annotations

`pr-impact`, `ask`, `query`, `impact`, and report commands are analysis operations, but their default caching can still write cache data; add `--no-cache` for a strictly read-only run.

## MCP

The stdio command is:

```bash
codegraph mcp /absolute/path/to/repository
```

For project-scoped configuration, `codegraph install-agent /repo --platform all` creates the MCP entry and assistant guidance. Inspect `.mcp.json`, `AGENTS.md`, and `CLAUDE.md` before and after. Do not hand-edit or overwrite an existing conflicting MCP entry without user direction; use `--force` only after inspecting the conflict.
