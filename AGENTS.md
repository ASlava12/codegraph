<!-- codegraph:start -->
## CodeGraph

This repository is indexed by CodeGraph: a typed code knowledge graph with
confidence and provenance on every fact. Query the graph before broad file
reads or grep sweeps — it answers structural questions in one bounded call.

- Ask in natural language: `codegraph ask "Where is DATABASE_URL read?" .`
- Query slices: `codegraph query 'nodes kind:function label:main' .`
- Follow an execution flow: `codegraph journey --from <entrypoint> --to <target> .`
- Assess a change before making it: `codegraph impact <target> .`
- Get full refactor context in one call: `codegraph refactor-context <target> .`
- Project overview with risks: `codegraph report . --format markdown`

Over MCP, the `codegraph` server (see `.mcp.json`) exposes `query_graph`,
`get_node_card`, `get_neighbors`, `shortest_path`, `workflow`, `insights`,
`impact`, `report`, `ask`, `source_search`, `refactor_context`, and the
`memory_save`/`memory_list`/`memory_reflect` investigation-memory tools with
the same graph answers.
<!-- codegraph:end -->
