// Browser-less smoke test for the Flow view render path.
//
// `node --check` validates syntax and `check-defs.mjs` catches calls to helpers
// that are never defined, but neither exercises the code: a runtime error in
// layout/draw (a bad canvas call, a null deref, a logic slip) still ships. This
// loads the real concatenated bundle under DOM/canvas stubs and drives the flow
// render path over a synthetic workflow report — groups, hidden-child counts, a
// multi-level tree — asserting it lays out, draws, navigates, and refines
// without throwing. Run: node crates/codegraph-web/static/flow-smoke.mjs
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const dir = dirname(fileURLToPath(import.meta.url));
const jsDir = join(dir, "js");
let source = readFileSync(join(dir, "label-policy.js"), "utf8") + "\n";
source += readdirSync(jsDir)
  .filter((name) => name.endsWith(".js") && !name.endsWith(".test.js"))
  .sort()
  .map((name) => readFileSync(join(jsDir, name), "utf8"))
  .join("\n");
source +=
  "\n;globalThis.__flow = { state, layoutFlow, drawFlow, fitFlowView, renderFlowHud," +
  " renderFlowHelp, toggleFlowHelp, refineFlowReport, flowPathToBlock, flowNavigate," +
  " selectFlowBlock, openFlowView };\n";

const ctx = new Proxy(
  {
    canvas: { width: 1400, height: 800 },
    measureText: () => ({ width: 10 }),
    createLinearGradient: () => ({ addColorStop() {} }),
    setLineDash() {},
  },
  { get: (t, p) => (p in t ? t[p] : () => {}), set: () => true },
);
const el = () => {
  const node = {
    width: 1400,
    height: 800,
    clientWidth: 1400,
    clientHeight: 800,
    textContent: "",
    innerHTML: "",
    value: "",
    checked: false,
    hidden: false,
    dataset: {},
    style: {},
    files: [],
    classList: { add() {}, remove() {}, toggle() {}, contains: () => false },
    addEventListener() {},
    removeEventListener() {},
    append() {},
    appendChild() {},
    remove() {},
    setAttribute() {},
    removeAttribute() {},
    getAttribute: () => null,
    insertAdjacentHTML() {},
    scrollIntoView() {},
    focus() {},
    closest: () => el(),
    matches: () => false,
    cloneNode: () => el(),
    querySelector: () => el(),
    querySelectorAll: () => [],
    getBoundingClientRect: () => ({ width: 1400, height: 800, left: 0, top: 0, right: 1400, bottom: 800 }),
    getContext: () => ctx,
  };
  return node;
};
const cache = new Map();
const cached = (key) => (cache.has(key) ? cache.get(key) : (cache.set(key, el()), cache.get(key)));
const storage = { getItem: () => null, setItem() {}, removeItem() {}, clear() {} };
const documentStub = {
  documentElement: el(),
  body: el(),
  head: el(),
  querySelector: (selector) => cached(`q:${selector}`),
  querySelectorAll: () => [],
  getElementById: (id) => cached(`#${id}`),
  createElement: () => el(),
  createElementNS: () => el(),
  createDocumentFragment: () => el(),
  addEventListener() {},
};
const sandbox = {
  document: documentStub,
  console,
  addEventListener() {},
  removeEventListener() {},
  localStorage: storage,
  sessionStorage: storage,
  navigator: { language: "en", clipboard: { writeText: () => Promise.resolve() } },
  location: { href: "http://localhost/", search: "", pathname: "/", hash: "" },
  history: { replaceState() {}, pushState() {} },
  matchMedia: () => ({ matches: false, addEventListener() {}, addListener() {} }),
  getComputedStyle: () => ({ getPropertyValue: () => "" }),
  requestAnimationFrame: () => 0,
  cancelAnimationFrame() {},
  setTimeout: () => 0,
  clearTimeout() {},
  setInterval: () => 0,
  clearInterval() {},
  fetch: () => new Promise(() => {}),
  EventSource: class {
    close() {}
    addEventListener() {}
  },
  Headers,
  URL,
  URLSearchParams,
  Intl,
  structuredClone: (value) => JSON.parse(JSON.stringify(value)),
};
sandbox.window = sandbox;
sandbox.globalThis = sandbox;
vm.createContext(sandbox);

const fail = (message) => {
  console.error(`flow-smoke: FAIL — ${message}`);
  process.exit(1);
};

try {
  vm.runInContext(source, sandbox, { filename: "app.js" });
} catch (error) {
  fail(`bundle boot threw: ${error.stack || error.message}`);
}
const flow = sandbox.__flow;
if (!flow) fail("flow internals were not exposed (boot aborted early)");

// A synthetic workflow report: a start, a branch, a wide node whose fan-out was
// capped (+N hidden), a deeper chain, and a group of same-kind leaves.
const report = {
  start: { label: "main" },
  truncated: true,
  blocks: [
    { id: 1, kind: "start", depth: 0, node: { id: 100, label: "main" } },
    { id: 2, kind: "call", depth: 1, node: { id: 101, label: "run" }, truncated_children: 12 },
    { id: 3, kind: "call", depth: 2, node: { id: 102, label: "scan" } },
    { id: 4, kind: "call", depth: 3, node: { id: 103, label: "parse" }, risk_refs: [{ severity: "warning" }] },
    { id: 5, kind: "branch", depth: 2, node: { id: 104, label: "if a" } },
    { id: 6, kind: "branch", depth: 2, node: { id: 105, label: "if b" } },
    { id: 7, kind: "branch", depth: 2, node: { id: 106, label: "if c" } },
    { id: 8, kind: "branch", depth: 2, node: { id: 107, label: "if d" } },
  ],
  transitions: [
    { id: "t1", source: 1, target: 2, edge: { kind: "calls" } },
    { id: "t2", source: 2, target: 3, edge: { kind: "calls" } },
    { id: "t3", source: 3, target: 4, edge: { kind: "calls" } },
    { id: "t4", source: 2, target: 5, edge: { kind: "references" } },
    { id: "t5", source: 2, target: 6, edge: { kind: "references" } },
    { id: "t6", source: 2, target: 7, edge: { kind: "references" } },
    { id: "t7", source: 2, target: 8, edge: { kind: "references" } },
  ],
};

try {
  flow.state.graph = { nodes: report.blocks.map((b) => b.node), edges: [] };
  flow.state.stageView = "flow";

  const refined = flow.refineFlowReport(report);
  if (!refined.grouped) fail("refineFlowReport did not collapse the sibling branch group");
  const group = refined.blocks.find((b) => b.groupCount > 1);
  if (!group || group.groupCount !== 4) fail(`expected a 4-member branch group, got ${group?.groupCount}`);

  flow.openFlowView(report, "main");
  if (!flow.state.flow.report) fail("openFlowView did not set the report");
  flow.layoutFlow();
  if (flow.state.flow.positions.size === 0) fail("layoutFlow produced no positions");
  flow.drawFlow();
  flow.fitFlowView();
  flow.renderFlowHud();
  flow.renderFlowHelp();
  flow.toggleFlowHelp(true);

  // Path highlight from start down to the deepest node.
  const path = flow.flowPathToBlock(1);
  if (!path || !path.blockIds.has(1)) fail("flowPathToBlock did not resolve a path");

  // Keyboard walk: no selection -> start, then a step onward, without throwing.
  flow.state.flow.selectedBlockId = null;
  flow.flowNavigate("right");
  flow.flowNavigate("right");
  flow.drawFlow();
} catch (error) {
  fail(`flow render path threw: ${error.stack || error.message}`);
}

console.log("flow-smoke: ok (bundle boots, flow lays out, draws, refines, and navigates)");
