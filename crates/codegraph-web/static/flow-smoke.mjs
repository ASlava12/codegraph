// Browser-less smoke test for the Flow view render path.
//
// Drives the real bundle over a synthetic workflow report — groups,
// hidden-child counts, a multi-level tree — asserting it lays out, draws,
// navigates, and refines without throwing. The DOM/canvas scaffolding lives in
// smoke-harness.mjs. Run: node crates/codegraph-web/static/flow-smoke.mjs
import { loadBundle } from "./smoke-harness.mjs";

const FLOW_EXPORTS = [
  "state", "layoutFlow", "drawFlow", "fitFlowView", "renderFlowHud",
  "renderFlowHelp", "toggleFlowHelp", "refineFlowReport", "flowPathToBlock",
  "flowNavigate", "selectFlowBlock", "openFlowView",
];


const fail = (message) => {
  console.error(`flow-smoke: FAIL — ${message}`);
  process.exit(1);
};

let flow;
try {
  flow = loadBundle("__flow", FLOW_EXPORTS);
} catch (error) {
  fail(`bundle boot threw: ${error.stack || error.message}`);
}
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

  // Focus mode dims all but the current thread — must draw without throwing.
  flow.state.flow.focusMode = true;
  flow.state.flow.selectedBlockId = 4;
  flow.drawFlow();
  flow.state.flow.focusMode = false;
} catch (error) {
  fail(`flow render path threw: ${error.stack || error.message}`);
}

console.log("flow-smoke: ok (bundle boots, flow lays out, draws, refines, and navigates)");
