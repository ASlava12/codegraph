const test = require("node:test");
const assert = require("node:assert/strict");

const policy = require("./label-policy.js");

test("minimal mode keeps labels hidden except direct interaction", () => {
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
    true,
  );
  assert.equal(policy.nodeLabelBudget({ labelMode: "minimal", zoom: 4, visibleCount: 5 }), 0);
});

test("auto mode only allows a tiny label budget in dense graphs", () => {
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 3.5, visibleCount: 20 }), 1);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 4.1, visibleCount: 10 }), 2);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 3.5, visibleCount: 60 }), 0);
  assert.equal(policy.nodeLabelBudget({ labelMode: "auto", zoom: 2.4, visibleCount: 15 }), 0);
});

test("auto mode prioritizes entrypoints and risks over low-signal nodes", () => {
  const entrypointPriority = policy.nodeLabelPriority({ kind: "entrypoint", metadata: {} });
  const functionPriority = policy.nodeLabelPriority({ kind: "function", metadata: {} });
  assert.ok(entrypointPriority < functionPriority);
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      zoom: 3.7,
      visibleCount: 20,
      priority: entrypointPriority,
    }),
    true,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "auto",
      zoom: 3.7,
      visibleCount: 20,
      priority: functionPriority,
    }),
    false,
  );
});

test("focus mode only labels focused nodes at high zoom", () => {
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      focused: true,
      zoom: 2.3,
      visibleCount: 10,
      priority: 1,
    }),
    false,
  );
  assert.equal(
    policy.shouldShowNodeLabel({
      labelMode: "focus",
      focused: true,
      zoom: 2.35,
      visibleCount: 10,
      priority: 1,
    }),
    true,
  );
  assert.equal(policy.nodeLabelBudget({ labelMode: "focus", zoom: 2.4, visibleCount: 80 }), 0);
});

test("graph labels are truncated to stable compact text", () => {
  assert.equal(policy.truncateGraphLabel("short", 10), "short");
  assert.equal(policy.truncateGraphLabel("a-very-long-node-label", 10), "a-very-...");
});
