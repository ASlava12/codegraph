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
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 4.9, visibleCount: 6 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 5, visibleCount: 6 }), 1);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 5, visibleCount: 7 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 3.5, visibleCount: 60 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 3.2, visibleCount: 15 }), 0);
});

test("hover mode only labels the current interaction target", () => {
  assert.equal(policy.nodeLabelBudget({ labelMode: "hover", zoom: 6, visibleCount: 2 }), 0);
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "hover",
      selected: true,
      zoom: 1,
      visibleCount: 200,
      priority: 9,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "hover",
      hovered: true,
      zoom: 1,
      visibleCount: 200,
      priority: 9,
    }),
    true,
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
      zoom: 5,
      visibleCount: 6,
      priority: entrypointPriority,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      zoom: 5,
      visibleCount: 6,
      priority: functionPriority,
    }),
    false,
  );
});

test("auto mode keeps hover labels out of very dense views", () => {
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
      zoom: 3.6,
      visibleCount: 8,
      priority: 1,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      hovered: true,
      zoom: 3.8,
      visibleCount: 9,
      priority: 1,
    }),
    false,
  );
});

test("selected labels stay out of dense low-zoom views", () => {
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      selected: true,
      zoom: 2,
      visibleCount: 20,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      selected: true,
      zoom: 2.6,
      visibleCount: 12,
      priority: 1,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      selected: true,
      zoom: 2.8,
      visibleCount: 13,
      priority: 1,
    }),
    false,
  );
});

test("focus mode only labels focused nodes at high zoom", () => {
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      focused: true,
      zoom: 3.7,
      visibleCount: 6,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      focused: true,
      zoom: 3.8,
      visibleCount: 6,
      priority: 1,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      focused: true,
      zoom: 4,
      visibleCount: 7,
      priority: 1,
    }),
    false,
  );
  assert.equal(policy.nodeLabelBudget({ labelMode: "focus", zoom: 3.7, visibleCount: 6 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "focus", zoom: 3.8, visibleCount: 6 }), 1);
  assert.equal(policy.nodeLabelBudget({ labelMode: "focus", zoom: 3.8, visibleCount: 7 }), 0);
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
