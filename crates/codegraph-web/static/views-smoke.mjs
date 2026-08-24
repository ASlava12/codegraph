// Browser-less smoke test for the render paths outside the Flow view.
//
// The Flow view has had a smoke test since it was built; the rest of the UI —
// the overview, query results, the insight list, journeys, the check report —
// had none, so a runtime error there shipped as easily as the ones this class
// of test was written to catch. Each view is driven twice: once over a normal
// payload and once over what the API really returns at the edges (no matches,
// an edge whose nodes are outside the slice, a node with no span, a report
// that passed). The DOM/canvas scaffolding lives in smoke-harness.mjs.
// Run: node crates/codegraph-web/static/views-smoke.mjs
import { loadBundle } from "./smoke-harness.mjs";

const VIEW_EXPORTS = [
  "state", "renderOverview", "renderQueryResult", "renderInsights",
  "renderJourneyReport", "renderCheckReport", "clientEntrypointReachableIds",
  "buildClientInsights", "buildSelectionUrl", "parseUrlNodeReference", "resolveNodeReference",
];
const api = loadBundle("__views", VIEW_EXPORTS);
const fail = (label, error) => {
  console.error(`FAIL ${label}: ${error && error.stack ? error.stack.split("\n").slice(0, 2).join(" | ") : error}`);
  process.exitCode = 1;
};
const ok = [];
const drive = (label, run) => { try { run(); ok.push(label); } catch (error) { fail(label, error); } };

// A link somebody keeps outlives the scan that produced it: the positional
// id names a different node once a file above it is edited, and every
// surface takes the durable one.
drive("a shared link carries the durable node id", () => {
  api.state.graph = {
    nodes: [
      { id: 7, kind: "function", label: "load", metadata: { stable_id: "cg-1234567890abcdef" } },
      { id: 8, kind: "function", label: "helper", metadata: {} },
    ],
    edges: [],
  };
  const link = api.buildSelectionUrl({ nodeId: 7 });
  if (!link.includes("node=cg-1234567890abcdef")) {
    throw new Error(`link names the durable id: ${link}`);
  }
  // A node the scan stamped nothing on keeps the positional form.
  const fallback = api.buildSelectionUrl({ nodeId: 8 });
  if (!fallback.includes("node=8")) throw new Error(`fallback link: ${fallback}`);
  // And both forms read back to the same node.
  if (api.resolveNodeReference(api.parseUrlNodeReference("cg-1234567890abcdef")) !== 7) {
    throw new Error("a durable id in a link resolves to its node");
  }
  if (api.resolveNodeReference(api.parseUrlNodeReference("8")) !== 8) {
    throw new Error("a positional id still resolves");
  }
  if (api.resolveNodeReference(api.parseUrlNodeReference("cg-gone")) !== null) {
    throw new Error("an id from another project resolves to nothing");
  }
});

drive("renderOverview", () => {
  api.state.summary = { nodes: 12, edges: 20, node_kinds: { function: 8, file: 4 }, edge_kinds: { calls: 12 }, languages: { rust: 4 } };
  api.state.entrypoints = [{ id: 1, kind: "function", label: "main", metadata: { entrypoint_kind: "program" } }];
  api.state.insights = { total: 2, by_severity: { warning: 1, info: 1 }, by_kind: { unresolved_call: 2 },
    insights: [{ kind: "unresolved_call", severity: "warning", message: "call `x` unresolved", nodes: [1], edges: [0] }] };
  api.renderOverview();
});

drive("renderQueryResult", () => api.renderQueryResult({
  query: "nodes kind:function",
  nodes: [{ id: 1, kind: "function", label: "main", span: { path: "src/main.rs", start_line: 1 }, metadata: {} }],
  edges: [{ source: 1, target: 1, kind: "calls", confidence: "heuristic", metadata: { line: "3", column: "5" } }],
  total_nodes: 1, total_edges: 1,
  facets: { node_kinds: { function: 1 }, edge_kinds: { calls: 1 } },
}));

// A result whose answer had to choose between same-named definitions
// carries a note; the view must show it rather than drop it.
drive("query result with a note", () => {
  const html = api.renderQueryResult({
    query: "dependents label:Blueprint depth:4",
    nodes: [{ id: 1, kind: "type", label: "Blueprint", span: { path: "src/blueprints.py", start_line: 18 }, metadata: {} }],
    edges: [],
    total_nodes: 1, total_edges: 0,
    facets: { node_kinds: { type: 1 }, edge_kinds: {} },
    notes: ["2 definitions are named `Blueprint`; this answer traces the one at src/blueprints.py:18"],
  });
  if (!html.includes("query-note") || !html.includes("2 definitions are named")) {
    throw new Error("the note is missing from the rendered result");
  }
});

// The browser recomputes reachability when no server report is available.
// It must follow `contains` out of a file exactly as the server does, or
// everything an entrypoint file holds reads as unreachable.
drive("client reachability walks into an entrypoint file", () => {
  const graph = {
    nodes: [
      { id: 1, kind: "file", label: "src/main.rs", metadata: {} },
      { id: 2, kind: "function", label: "main", metadata: {} },
      { id: 3, kind: "function", label: "helper", metadata: {} },
      { id: 4, kind: "directory", label: "src", metadata: {} },
      { id: 5, kind: "function", label: "elsewhere", metadata: {} },
    ],
    edges: [
      { source: 1, target: 2, kind: "contains", confidence: "syntactic", metadata: {} },
      { source: 2, target: 3, kind: "calls", confidence: "syntactic", metadata: {} },
      { source: 4, target: 5, kind: "contains", confidence: "syntactic", metadata: {} },
    ],
  };
  const reachable = api.clientEntrypointReachableIds(graph, new Set([1]));
  for (const id of [1, 2, 3]) {
    if (!reachable.has(id)) throw new Error(`node ${id} must be reachable from its file`);
  }
  // Containment out of a directory would make the whole tree reachable.
  if (reachable.has(5)) throw new Error("a directory must not reach through containment");
});

drive("renderInsights", () => api.renderInsights());

drive("renderJourneyReport", () => api.renderJourneyReport({
  from: { id: 1, label: "main" }, to: { id: 2, label: "helper" }, total_paths: 1,
  paths: [{ steps: [
    { index: 1, node: { id: 1, kind: "function", label: "main" } },
    { index: 2, node: { id: 2, kind: "function", label: "helper" }, edge: { kind: "calls", confidence: "heuristic" } },
  ] }],
}));

// A journey whose ends are ambiguous carries notes; the view must show them.
drive("journey with a note", () => {
  const html = api.renderJourneyReport({
    from: { id: 1, label: "handle" }, to: { id: 2, label: "helper" }, total_paths: 0,
    paths: [], max_depth: 4,
    notes: ["2 definitions are named `handle`; this answer is about the one at src/first.rs:4"],
  });
  if (!html.includes("query-note") || !html.includes("2 definitions are named")) {
    throw new Error("the journey note is missing from the rendered result");
  }
});

drive("renderCheckReport", () => api.renderCheckReport({
  passed: false, fail_on: "warning", failing_insights: 1,
  report: { total: 1, by_severity: { warning: 1 }, by_kind: { unresolved_call: 1 },
    insights: [{ kind: "unresolved_call", severity: "warning", message: "m", nodes: [], edges: [] }] },
}));


// --- граничные данные, которые реально возвращает сервер ---
drive("empty query result", () => api.renderQueryResult({
  query: "nodes kind:function label:nothing", nodes: [], edges: [], total_nodes: 0, total_edges: 0, facets: {},
}));

drive("edge without its nodes", () => api.renderQueryResult({
  query: "edges kind:calls", nodes: [], total_nodes: 0, total_edges: 1,
  edges: [{ source: 99, target: 100, kind: "calls", confidence: "heuristic" }],
}));

drive("node without a span", () => api.renderQueryResult({
  query: "nodes", nodes: [{ id: 7, kind: "external_dependency", label: "serde", metadata: {} }],
  edges: [], total_nodes: 1, total_edges: 0,
}));

drive("overview without a summary", () => {
  api.state.summary = null; api.state.entrypoints = []; api.state.insights = null;
  api.renderOverview();
});

drive("journey with no path", () => api.renderJourneyReport({
  from: { id: 1, label: "main" }, to: { id: 2, label: "helper" }, total_paths: 0, paths: [],
}));

drive("insights without severities", () => {
  api.state.insights = { total: 0, insights: [] };
  api.renderInsights();
});

drive("check that passed", () => api.renderCheckReport({
  passed: true, fail_on: "error", failing_insights: 0,
  report: { total: 0, by_severity: {}, by_kind: {}, insights: [] },
}));

if (!process.exitCode) console.log("edge cases: ok");

// The browser recomputes a handful of findings itself when the server has
// not sent a report, so those rules have to say what the CLI says. A name
// missing from the built-in module list, or a specifier the CLI skips,
// shows the reader a dependency the project never forgot.
drive("client insights agree with the CLI on undeclared imports", () => {
  const dependency = (id, label, packageId) => ({
    id, kind: "external_dependency", label,
    metadata: { item_kind: "dependency", package_id: packageId },
  });
  const importNode = (id, label) => ({
    id, kind: "external_dependency", label,
    metadata: { item_kind: "import", language: "typescript" },
  });
  const graph = {
    nodes: [
      { id: 1, kind: "file", label: "package.json" },
      dependency(2, "@types/trusted-types", "npm:@types/trusted-types"),
      { id: 3, kind: "file", label: "src/a.ts" },
      { id: 4, kind: "file", label: "src/b.ts" },
      { id: 5, kind: "file", label: "src/c.ts" },
      { id: 6, kind: "file", label: "src/d.ts" },
      importNode(10, 'import http2 from "http2"'),
      importNode(11, 'import { test } from "bun:test"'),
      importNode(12, 'import type { TrustedHTML } from "trusted-types/lib"'),
      importNode(13, 'import { z } from "zod"'),
      importNode(14, 'import { z } from "zod"'),
      importNode(15, 'import { z } from "zod"'),
      importNode(16, 'import { z } from "zod"'),
    ],
    edges: [
      { kind: "imports", source: 3, target: 10 },
      { kind: "imports", source: 3, target: 11 },
      { kind: "imports", source: 3, target: 12 },
      { kind: "imports", source: 3, target: 13 },
      { kind: "imports", source: 4, target: 14 },
      { kind: "imports", source: 5, target: 15 },
      { kind: "imports", source: 6, target: 16 },
    ],
  };
  const undeclared = api
    .buildClientInsights(graph)
    .filter((insight) => insight.kind === "undeclared_external_import");
  if (undeclared.length !== 1) {
    throw new Error(`expected one finding, got ${JSON.stringify(undeclared)}`);
  }
  const [finding] = undeclared;
  if (!finding.message.startsWith("zod is imported from")) {
    throw new Error(`finding names the package first: ${finding.message}`);
  }
  if (!finding.message.includes("and 1 more")) {
    throw new Error(`finding counts the sources it leaves out: ${finding.message}`);
  }
});

// The CLI names the distribution a module comes from and files a test's or
// a build script's undeclared import as a note. Driving the bundle over
// flask showed the browser reporting ten warnings where the CLI reported
// one warning and eight notes.
drive("client insights read undeclared imports the way the CLI does", () => {
  const graph = {
    nodes: [
      { id: 1, kind: "file", label: "pyproject.toml" },
      {
        id: 2,
        kind: "external_dependency",
        label: "python-dotenv",
        metadata: { item_kind: "dependency", package_id: "python:python-dotenv" },
      },
      { id: 3, kind: "file", label: "src/app/config.py" },
      { id: 4, kind: "file", label: "tests/test_config.py" },
      { id: 5, kind: "file", label: "scripts/release.py" },
      {
        id: 10,
        kind: "external_dependency",
        label: "from dotenv import load_dotenv",
        metadata: { item_kind: "import", language: "python" },
      },
      {
        id: 11,
        kind: "external_dependency",
        label: "import fixture_only",
        metadata: { item_kind: "import", language: "python" },
      },
      {
        id: 12,
        kind: "external_dependency",
        label: "import enquirer",
        metadata: { item_kind: "import", language: "python" },
      },
      {
        id: 13,
        kind: "external_dependency",
        label: "import yaml",
        metadata: { item_kind: "import", language: "python" },
      },
    ],
    edges: [
      { kind: "imports", source: 3, target: 10 },
      { kind: "imports", source: 4, target: 11 },
      { kind: "imports", source: 5, target: 12 },
      { kind: "imports", source: 3, target: 13 },
    ],
  };
  const findings = api
    .buildClientInsights(graph)
    .filter((insight) => insight.kind === "undeclared_external_import");
  const named = Object.fromEntries(
    findings.map((finding) => [finding.message.split(" ")[0], finding.severity]),
  );
  if (named.dotenv || named["python-dotenv"]) {
    throw new Error(`python-dotenv ships the dotenv module: ${JSON.stringify(findings)}`);
  }
  // An undeclared module is named by the distribution that ships it, as
  // the CLI names it.
  if (named.pyyaml !== "warning" || named.yaml) {
    throw new Error(`an undeclared module names its distribution: ${JSON.stringify(findings)}`);
  }
  if (named["fixture-only"] !== "info" || named.enquirer !== "info") {
    throw new Error(`a test's and a script's imports are notes: ${JSON.stringify(findings)}`);
  }
});

// An unresolved or ambiguous call is the expected default on a syntax-only
// scan, and the CLI reads it as info; the browser used to call both a
// warning, and looked for ambiguity in a shape the resolver stopped
// writing when it started bounding uncertainty in one placeholder node.
drive("a project does not depend on itself", () => {
  const graph = {
    nodes: [
      {
        id: 1,
        kind: "repository",
        label: "guzzle",
        metadata: { own_package_ids: "composer:guzzlehttp/guzzle" },
      },
      { id: 2, kind: "file", label: "src/Client.php" },
      {
        id: 3,
        kind: "external_dependency",
        label: "psr/http-client",
        metadata: { item_kind: "dependency", package_id: "composer:psr/http-client" },
      },
      {
        id: 4,
        kind: "external_dependency",
        label: "use GuzzleHttp\\Psr7\\Request;",
        metadata: { item_kind: "import", language: "php" },
      },
    ],
    edges: [{ kind: "imports", source: 2, target: 4 }],
  };
  const undeclared = api
    .buildClientInsights(graph)
    .filter((insight) => insight.kind === "undeclared_external_import");
  if (undeclared.length !== 0) {
    throw new Error(`expected none, got ${JSON.stringify(undeclared.map((i) => i.message))}`);
  }
});

drive("client insights see both halves of the CLI's ambiguity rule", () => {
  const graph = {
    nodes: [
      { id: 1, kind: "function", label: "caller" },
      { id: 2, kind: "function", label: "indexOf" },
      { id: 3, kind: "function", label: "indexOf" },
      {
        id: 4,
        kind: "external_dependency",
        label: "helper",
        metadata: { item_kind: "call", resolution: "ambiguous", candidate_count: "2" },
      },
    ],
    edges: [
      // One call written once, landing on two definitions.
      { kind: "calls", source: 1, target: 2, metadata: { call_label: "indexOf" } },
      { kind: "calls", source: 1, target: 3, metadata: { call_label: "indexOf" } },
      // ...and the placeholder the resolver leaves for a bounded ambiguity.
      { kind: "calls", source: 1, target: 4, metadata: { call_label: "helper" } },
    ],
  };
  const ambiguous = api
    .buildClientInsights(graph)
    .filter((insight) => insight.kind === "ambiguous_call_resolution");
  if (ambiguous.length !== 2) {
    throw new Error(`expected both halves, got ${JSON.stringify(ambiguous.map((i) => i.message))}`);
  }
});

drive("client insights read unresolved and ambiguous calls as the CLI does", () => {
  const placeholder = (id, label, resolution, extra = {}) => ({
    id, kind: "function", label,
    metadata: { item_kind: "call", resolution, ...extra },
  });
  const graph = {
    nodes: [
      { id: 1, kind: "function", label: "run" },
      placeholder(2, "helper", "unresolved"),
      placeholder(3, "done", "unresolved"),
      placeholder(4, "build", "ambiguous", { candidate_count: "3", candidate_sample: "a.rs:build" }),
    ],
    edges: [
      { kind: "calls", source: 1, target: 2, metadata: { call_label: "helper" } },
      { kind: "calls", source: 1, target: 3, metadata: { call_label: "done", unresolved_reason: "local_value" } },
      { kind: "calls", source: 1, target: 4, metadata: { call_label: "build" } },
    ],
  };
  const insights = api.buildClientInsights(graph);
  const unresolved = insights.filter((insight) => insight.kind === "unresolved_call");
  const ambiguous = insights.filter((insight) => insight.kind === "ambiguous_call_resolution");
  if (unresolved.length !== 1 || !unresolved[0].message.includes("helper")) {
    throw new Error(`a call through a bound value has nothing to find: ${JSON.stringify(unresolved)}`);
  }
  if (unresolved[0].severity !== "info" || ambiguous[0]?.severity !== "info") {
    throw new Error("a syntax-only scan reads both as info");
  }
  if (ambiguous.length !== 1 || !ambiguous[0].message.includes("3 definitions")) {
    throw new Error(`ambiguity is a placeholder node: ${JSON.stringify(ambiguous)}`);
  }

  const enriched = { ...graph, edges: graph.edges.map((edge) => ({ ...edge, confidence: "semantic" })) };
  const after = api.buildClientInsights(enriched).filter((insight) => insight.kind === "unresolved_call");
  if (after[0]?.severity !== "warning") {
    throw new Error("after semantic enrichment an unresolved target is a warning");
  }
});

// Every kind the browser recomputes has to carry the severity the CLI
// gives it, or the same graph reads worse in the view than at the command
// line. Raising is ordinary control flow, and reachability is heuristic on
// a syntax-only scan: both are context.
drive("client insights carry the CLI's severities", () => {
  const graph = {
    nodes: [
      { id: 1, kind: "function", label: "run" },
      { id: 2, kind: "function", label: "raise" },
      { id: 3, kind: "function", label: "orphan" },
      { id: 4, kind: "function", label: "twin" },
      { id: 5, kind: "function", label: "twin" },
      { id: 6, kind: "file", label: "broken.py", metadata: { parse_error: "unexpected token" } },
      {
        id: 7,
        kind: "file",
        label: "secret.py",
        metadata: { read_error: "Permission denied (os error 13)" },
      },
    ],
    edges: [
      { kind: "may_error", source: 1, target: 2 },
      // An entrypoint that reaches nothing else, so `run` is out of reach
      // and its error flow is reported as such.
      { kind: "entrypoint", source: 6, target: 3 },
    ],
  };
  const bySeverity = new Map(
    api.buildClientInsights(graph).map((insight) => [insight.kind, insight.severity]),
  );
  const expected = {
    potential_error_flow: "info",
    unreachable_error_flow: "info",
    orphan_function: "info",
    duplicate_function_label: "info",
    parse_error: "error",
    unreadable_file: "error",
  };
  for (const [kind, severity] of Object.entries(expected)) {
    if (bySeverity.get(kind) !== severity) {
      throw new Error(`${kind}: expected ${severity}, got ${bySeverity.get(kind)}`);
    }
  }
});

if (!process.exitCode) console.log("views-smoke: ok (" + ok.join(", ") + ")");
