const test = require("node:test");
const assert = require("node:assert/strict");

const policy = require("./label-policy.js");

test("minimal mode keeps all graph labels hidden", () => {
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "minimal",
      zoom: 4,
      visibleCount: 5,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "minimal",
      hovered: true,
      zoom: 1,
      visibleCount: 200,
      priority: 9,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "minimal",
      selected: true,
      zoom: 1,
      visibleCount: 200,
      priority: 9,
    }),
    false,
  );
  assert.equal(policy.nodeLabelBudget({ labelMode: "minimal", zoom: 4, visibleCount: 5 }), 0);
});

test("auto mode only allows labels in very sparse, highly zoomed graphs", () => {
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 5.5, visibleCount: 4 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 5.6, visibleCount: 4 }), 1);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 5.6, visibleCount: 5 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 3.5, visibleCount: 60 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 3.2, visibleCount: 15 }), 0);
});

test("hover mode only labels the current hovered target in readable views", () => {
  assert.equal(policy.nodeLabelBudget({ labelMode: "hover", zoom: 6, visibleCount: 2 }), 0);
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "hover",
      selected: true,
      hovered: true,
      zoom: 1,
      visibleCount: 200,
      priority: 9,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "hover",
      hovered: true,
      zoom: 1,
      visibleCount: 200,
      priority: 9,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "hover",
      hovered: true,
      zoom: 2.4,
      visibleCount: 80,
      priority: 9,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "hover",
      hovered: true,
      zoom: 2.4,
      visibleCount: 81,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "hover",
      focused: true,
      zoom: 6,
      visibleCount: 2,
      priority: 1,
    }),
    false,
  );
});

test("auto mode prioritizes entrypoints and risks over low-signal nodes", () => {
  const entrypointPriority = policy.nodeLabelPriority({ kind: "entrypoint", metadata: {} });
  const functionPriority = policy.nodeLabelPriority({ kind: "function", metadata: {} });
  assert.ok(entrypointPriority < functionPriority);
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      zoom: 5.6,
      visibleCount: 4,
      priority: entrypointPriority,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      zoom: 5.6,
      visibleCount: 4,
      priority: functionPriority,
    }),
    false,
  );
});

test("auto mode keeps hover labels out of dense or low-signal views", () => {
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      hovered: true,
      zoom: 3.1,
      visibleCount: 80,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      hovered: true,
      zoom: 3.2,
      visibleCount: 12,
      priority: 9,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      hovered: true,
      zoom: 4.2,
      visibleCount: 4,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      hovered: true,
      zoom: 5.2,
      visibleCount: 3,
      priority: 1,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      hovered: true,
      zoom: 4.4,
      visibleCount: 5,
      priority: 1,
    }),
    false,
  );
});

test("auto mode leaves selected node details to the side card", () => {
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      selected: true,
      zoom: 5,
      visibleCount: 2,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      selected: true,
      hovered: true,
      zoom: 5,
      visibleCount: 2,
      priority: 1,
    }),
    false,
  );
});

test("focus mode labels focused nodes only in sparse high-zoom views", () => {
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      selected: true,
      zoom: 5,
      visibleCount: 3,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      focused: true,
      zoom: 4.8,
      visibleCount: 3,
      priority: 1,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      focused: true,
      zoom: 5,
      visibleCount: 4,
      priority: 1,
    }),
    false,
  );
  assert.equal(policy.nodeLabelBudget({ labelMode: "focus", zoom: 4.7, visibleCount: 3 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "focus", zoom: 4.8, visibleCount: 3 }), 1);
  assert.equal(policy.nodeLabelBudget({ labelMode: "focus", zoom: 4.8, visibleCount: 4 }), 0);
});

test("graph labels are truncated to stable compact text", () => {
  assert.equal(policy.truncateGraphLabel("short", 10), "short");
  assert.equal(policy.truncateGraphLabel("a-very-long-node-label", 10), "a-very-...");
});

test("forced graph labels wrap into compact side cards", () => {
  assert.deepEqual(policy.compactGraphLabelLines("src/crates/codegraph-analysis/src/lib.rs", 16, 2), [
    "src/crates",
    "codegraph-ana...",
  ]);
  assert.deepEqual(policy.compactGraphLabelLines("main", 16, 2), ["main"]);
});
