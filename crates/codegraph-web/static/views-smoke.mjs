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
  "renderJourneyReport", "renderCheckReport",
];
const api = loadBundle("__views", VIEW_EXPORTS);
const fail = (label, error) => {
  console.error(`FAIL ${label}: ${error && error.stack ? error.stack.split("\n").slice(0, 2).join(" | ") : error}`);
  process.exitCode = 1;
};
const ok = [];
const drive = (label, run) => { try { run(); ok.push(label); } catch (error) { fail(label, error); } };

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

drive("renderInsights", () => api.renderInsights());

drive("renderJourneyReport", () => api.renderJourneyReport({
  from: { id: 1, label: "main" }, to: { id: 2, label: "helper" }, total_paths: 1,
  paths: [{ steps: [
    { index: 1, node: { id: 1, kind: "function", label: "main" } },
    { index: 2, node: { id: 2, kind: "function", label: "helper" }, edge: { kind: "calls", confidence: "heuristic" } },
  ] }],
}));

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

if (!process.exitCode) console.log("views-smoke: ok (" + ok.join(", ") + ")");
