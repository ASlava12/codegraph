# Task-Oriented Guides

Step-by-step walkthroughs for the most common investigation tasks:
[investigate a bug](#guide-1-investigate-a-bug),
[trace a config value](#guide-2-trace-a-config-value),
[plan a refactor](#guide-3-plan-a-refactor), and
[follow an entrypoint as a flow](#guide-4-follow-an-entrypoint-as-a-flow).
Each guide follows one scenario end to end with copy-paste-ready commands, the
output fields that matter, and the matching web UI and agent (MCP) surfaces.
The feature-by-feature reference stays in the [README](../README.md).

Every command in these guides was run against this repository; the sample
output fragments are real (trimmed for brevity). Node ids like `n5069` are
stable within one cache fingerprint but will differ in your checkout —
always take ids from your own command output.

## Setup

The guides use the short binary name `codegraph`. Inside this repository,
substitute `cargo run -p codegraph-cli --`:

```bash
# guide form
codegraph insights .
# workspace form
cargo run -p codegraph-cli -- insights .
```

The first command against a project scans it and warms the persistent cache
in `.codegraph/`; subsequent commands reuse the cache and answer in seconds.
For the web UI steps, start the server and open `http://127.0.0.1:3765`:

```bash
cargo run -p codegraph-server -- --root .
```

For agent (MCP) steps, the stdio transport is `codegraph mcp .` and the tool
names are listed in each guide; `codegraph install-agent .` writes the
`.mcp.json` entry and guidance snippets for you.

## Guide 1: Investigate a bug

Scenario: a log shows `failed to read semantic LSP cache`, and you need to
find where that error originates, which function raises it, and which
entrypoints can trigger it — without grepping through the whole repository.

### 1. Start from the symptom text

Search source for the message fragment. This finds the literal text and gives
you files and lines:

```bash
codegraph source-search "failed to read" . --limit 10
```

Output: `total_matches` plus `matches[]` with `path`, `line`, `line_text`,
and surrounding `context` lines. Narrow with `--path-filter crates/codegraph-lsp`
if the message appears in several crates.

### 2. Map the error into the graph

Error and exception constructs are graph facts, so instead of reading files,
trace the error back to its enclosing sources and the entrypoints that reach
them:

```bash
codegraph trace-errors "failed to read" . --limit 5
```

Each match carries the error node (`kind: "control_flow"`,
`metadata.item_kind: "error"`, with the file span), its `sources` — the
enclosing functions such as `open_request_document` — and upstream `paths`
toward entrypoints. `total_sources`/`total_paths` tell you how widespread the
error is before you commit to a hypothesis.

### 3. Inspect the suspect function

Take a node id from step 2 (numeric and `n`-prefixed both work) and open its
investigation card — summary, source preview, neighboring edges, and related
risks in one response:

```bash
codegraph node-card . --node-id n3080 --source-context 8
```

### 4. Find out who can trigger it

Trace incoming dependents of the suspect function to see every caller chain
that can reach it, including how far each entrypoint sits:

```bash
codegraph trace-dependents open_request_document . --depth 3
```

If a label is ambiguous or you don't know it exactly, ask in natural
language — `ask` maps the question to a bounded graph query and shows which
query it generated:

```bash
codegraph ask "Who calls open_request_document?" .
```

### 5. Cross-check known problem patterns

The insight scan may already have filed the root cause (dependency cycles,
unresolved calls, unreachable error flows). Filter by severity or kind
substring instead of reading all findings:

```bash
codegraph insights . --severity warning --limit 20
codegraph insights . --kind cycle
```

In CI, the same analysis is a gate: `codegraph check . --fail-on error`
exits non-zero when findings meet the threshold.

**Web UI:** Source Search panel (open matches as focused graph slices) →
click a node for its card → Insights panel with severity chips as filters.
**MCP tools:** `source_search`, `get_node_card`, `get_neighbors`,
`insights`, `ask`.

## Guide 2: Trace a config value

Scenario: you need to know where the `CODEGRAPH_API_TOKEN` environment
variable is read, which entrypoints depend on it, and whether its handling
has known problems.

### 1. Trace the value to its readers

```bash
codegraph trace-config CODEGRAPH_API_TOKEN .
```

Output: the environment node (with the exact file span where the read
happens), `readers[]` — the functions holding the read edge, e.g.
`configured_api_token` in `crates/codegraph-server/src/main.rs` — and
upstream `paths` from entrypoints down to the read. Config files work the
same way: `codegraph trace-config config/settings.toml .`.

### 2. Or ask in natural language

Environment and config questions route to the same trace rule; the response
shows the generated query so you can refine it by hand:

```bash
codegraph ask "Where is CODEGRAPH_API_TOKEN read?" .
# generated_query: "configs target:CODEGRAPH_API_TOKEN depth:6"
```

### 3. Enumerate every config surface

When you don't know the key name, list what the project reads:

```bash
codegraph query 'nodes kind:environment' .
codegraph query 'nodes kind:config limit:30' .
```

### 4. Check for config-specific problems

The insight scan flags conflicting fallback defaults, keys read both as
required and with defaults, reads not reachable from any entrypoint, and
sensitive keys (reported without leaking values):

```bash
codegraph insights . --kind config
codegraph insights . --kind unreachable_config_read
```

**Web UI:** Ask flow (type the question, get the focused slice), trace
panels with config trace JSON downloads, Insights panel.
**MCP tools:** `ask`, `query_graph`, `insights`.

## Guide 3: Plan a refactor

Scenario: you are about to change `scan_project` in `codegraph-indexer` and
want to know the blast radius, the execution path that leads to it, and the
fragile hops that are most likely to break — before touching the code.

### 1. Measure the blast radius

```bash
codegraph impact scan_project .
```

Output: the resolved target node, `dependents` (reverse dependency closure),
`affected_entrypoints` with their distance to the target (e.g.
`cargo bin:codegraph-cli` at distance 3), `affected_tests`, and a
risk-weighted `impact_score`. If the label is ambiguous, pass a node id
(`codegraph impact n5840 .`).

### 2. Read the execution path as a chain of events

A journey expands entrypoint-to-target paths into step-numbered chains built
from workflow blocks, ranked by confidence and length:

```bash
codegraph journey --from "cargo bin:codegraph-cli" --to scan_project .
```

Each step carries its `block` (kind: start/call/branch/…), the `transition`
edge with provenance, a human-readable `explanation`, and — critically for
refactor planning — `fragile: true` with `fragile_reasons` such as
`low_confidence_edge` on hops where behavior is most likely to break.
`risk_summary` on each path aggregates risky steps, low-confidence hops, and
cycles crossing the flow.

### 3. Understand the component's dependency surface

Group everything the target depends on (and what depends on it) by
architecture area, package, and language; then inspect the exact edges on a
boundary you plan to cut. Areas are top-level project directories — list
them with `codegraph architecture .`:

```bash
codegraph component-dependencies scan_project .
codegraph component-contract . --source docs --target crates
```

On this repository the `docs -> crates` contract lists 62 edges — every
place the documentation cites code — each with its confidence and related
risks, so you know what a rename will invalidate.

### 4. Find the safest seam

Seam ranking orders cross-area boundaries by coupling friction — `safest`
lists boundaries where extraction is cheapest, `most_needed` where the
tangle hurts most (`friction_score`, `low_confidence_edges`, `risk_count`
per boundary):

```bash
codegraph seams .
```

### 5. Hand an agent the whole bundle

One command combines impact, grouped dependencies, the optional journey,
related risks, and the target source span — enough context to plan the
refactor without raw repository reads (seconds on a warm cache):

```bash
codegraph refactor-context scan_project . --from "cargo bin:codegraph-cli"
```

### 6. Gate the change after editing

Once the refactor branch exists, map the changed files onto communities,
hotspots, and blast radius, and keep the insight gate green:

```bash
codegraph pr-impact . --base origin/main --ci-state passing
codegraph check . --fail-on error
```

**Web UI:** Refactoring panel (impact, dependencies, seams, contract, and
refactor-context download), Journey panel (pick start and target, read the
step-numbered chain with fragile chips), PR Impact panel.
**MCP tools:** `impact`, `shortest_path`, `refactor_context`, `report`.

## Guide 4: Follow an entrypoint as a flow

Scenario: understand how the program runs from an entrypoint onward — the
call chain, its branches, and where it can fail — as a readable flow you can
walk step by step, then drill into any step.

### 1. List entrypoints and pick a start

```bash
codegraph entrypoints .
```

```json
[
  { "id": 3792, "kind": "entrypoint", "label": "cargo bin:codegraph-cli",
    "metadata": { "entrypoint_kind": "binary", "target": "src/main.rs" } },
  { "id": 4546, "kind": "function", "label": "main" },
  { "id": 5623, "kind": "entrypoint", "label": "route GET /" }
]
```

The workflow below starts from a label, so you can pass `"cargo
bin:codegraph-cli"` directly; ids work too (`n3792`).

### 2. Build a flow that follows the call chain into depth

A workflow from a busy entrypoint fans out to everything it touches in one
shallow level. `--max-fanout` caps how many outgoing edges each node expands
(keeping calls first), so the block budget follows the call chain into depth
instead of one wide node:

```bash
codegraph workflow 'cargo bin:codegraph-cli' . --depth 10 --max-fanout 8 --compact
```

```text
start: cargo bin:codegraph-cli
total_blocks: 93   total_transitions: 162   truncated: true
max depth reached: 6
```

Without `--max-fanout` the same call saturates at depth 1–2; with it the flow
reaches depth 6. Each block also reports `truncated_children` — how many more
calls were trimmed from that node (e.g. `main` +405, `parse` +42) and are
reachable by drilling into it.

### 3. Read one focused path from entry to a target

When you have a specific target, a journey is a single step-numbered chain
rather than a fan:

```bash
codegraph journey --from node_card --to node_context .
```

```text
node_card -> node_context   (1 path, 2 steps)
  step 1  start  node_card
  step 2  call   node_context
```

### 4. Explore it interactively in the web Flow view

Start the server, open `http://127.0.0.1:3765`, and switch the stage to
**Flow**. Pick an entrypoint from the Flow picker and the call chain renders
left to right, deep, with same-kind branches collapsed into `×N` groups.
From there:

- **click** a block to open its source card; **double-click** or **Enter** to
  re-root the flow on that block and walk deeper; the breadcrumb trail (`›`)
  walks back.
- **arrows** step along the flow, **c** shows a node's callers (reverse flow),
  **f** is focus mode (dim all but the current thread), **?** lists every
  control.
- a **`+N`** marker flags nodes whose calls were trimmed (drill in for the
  rest); **⚠ trimmed** in the HUD says the view is bounded.
- the view deep-links as `?flow=<nodeId>`, so a flow survives reload and the
  **⧉ link** button copies it to share.

### 5. Agent (MCP) form

The same flows are available over MCP without shelling out:

```jsonc
// workflow tool — deep call chain from an entrypoint
{ "target": "cargo bin:codegraph-cli", "depth": 10, "max_fanout": 8, "compact": true }
// shortest_path / journey — a focused entry-to-target chain
```

**Web UI:** Flow view (entrypoint picker, drill-down, callers, focus,
group expansion), Entry Flows panel, Journey panel.
**MCP tools:** `workflow` (with `max_fanout`), `shortest_path`, `get_neighbors`.

## Where to go next

- The full feature reference, API examples, and export formats: [README](../README.md).
- Which analyses exist on which surface (CLI/API/web/MCP): [`SURFACE_PARITY.md`](SURFACE_PARITY.md).
- Graph design and crate layout: [`ARCHITECTURE.md`](ARCHITECTURE.md).
