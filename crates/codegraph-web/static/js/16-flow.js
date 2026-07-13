// 16-flow.js — Flow view rendering, HUD, and shared drawing utilities.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function fitFlowView() {
  const bounds = flowBounds();
  if (!bounds) return;
  const width = Math.max(1, bounds.maxX - bounds.minX);
  const height = Math.max(1, bounds.maxY - bounds.minY);
  const padding = 64;
  const zoomX = (flowCanvas.width - padding * 2) / width;
  const zoomY = (flowCanvas.height - padding * 2) / height;
  state.flow.zoom = Math.max(0.18, Math.min(1.6, Math.min(zoomX, zoomY)));
  state.flow.pan = {
    x: flowCanvas.width / 2 - ((bounds.minX + bounds.maxX) / 2) * state.flow.zoom,
    y: flowCanvas.height / 2 - ((bounds.minY + bounds.maxY) / 2) * state.flow.zoom,
  };
  drawFlow();
}

function flowZoomAt(screenX, screenY, scale) {
  const before = flowScreenToWorld(screenX, screenY);
  state.flow.zoom = Math.max(0.18, Math.min(3, state.flow.zoom * scale));
  const after = flowScreenToWorld(screenX, screenY);
  state.flow.pan.x += (after.x - before.x) * state.flow.zoom;
  state.flow.pan.y += (after.y - before.y) * state.flow.zoom;
  drawFlow();
}

function flowZoomAtCenter(scale) {
  flowZoomAt(flowCanvas.width / 2, flowCanvas.height / 2, scale);
}

function flowBlockAt(world) {
  const blocks = flowBlocks();
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    const position = state.flow.positions.get(block.id);
    if (!position) continue;
    if (
      world.x >= position.x &&
      world.x <= position.x + FLOW_BLOCK_WIDTH &&
      world.y >= position.y &&
      world.y <= position.y + FLOW_BLOCK_HEIGHT
    ) {
      return block;
    }
  }
  return null;
}

function flowTransitionGeometry(transition) {
  const source = state.flow.positions.get(transition.source);
  const target = state.flow.positions.get(transition.target);
  if (!source || !target) return null;
  const from = { x: source.x + FLOW_BLOCK_WIDTH, y: source.y + FLOW_BLOCK_HEIGHT / 2 };
  const to = { x: target.x, y: target.y + FLOW_BLOCK_HEIGHT / 2 };
  const forward = to.x >= from.x;
  const bend = forward ? Math.max(36, (to.x - from.x) / 2) : 72;
  if (forward) {
    return {
      from,
      to,
      c1: { x: from.x + bend, y: from.y },
      c2: { x: to.x - bend, y: to.y },
    };
  }
  const lift = Math.min(from.y, to.y) - FLOW_BLOCK_HEIGHT;
  return {
    from,
    to,
    c1: { x: from.x + bend, y: lift },
    c2: { x: to.x - bend, y: lift },
  };
}

function flowBezierPoint(geometry, t) {
  const inverse = 1 - t;
  const a = inverse * inverse * inverse;
  const b = 3 * inverse * inverse * t;
  const c = 3 * inverse * t * t;
  const d = t * t * t;
  return {
    x: a * geometry.from.x + b * geometry.c1.x + c * geometry.c2.x + d * geometry.to.x,
    y: a * geometry.from.y + b * geometry.c1.y + c * geometry.c2.y + d * geometry.to.y,
  };
}

function flowTransitionAt(world) {
  const threshold = 7 / Math.max(0.2, state.flow.zoom);
  const thresholdSquared = threshold * threshold;
  const transitions = flowTransitions();
  for (let index = transitions.length - 1; index >= 0; index -= 1) {
    const transition = transitions[index];
    const geometry = flowTransitionGeometry(transition);
    if (!geometry) continue;
    const samples = 24;
    for (let step = 0; step <= samples; step += 1) {
      const point = flowBezierPoint(geometry, step / samples);
      const dx = world.x - point.x;
      const dy = world.y - point.y;
      if (dx * dx + dy * dy <= thresholdSquared) return transition;
    }
  }
  return null;
}

function flowBlockById(blockId) {
  return flowBlocks().find((block) => block.id === blockId) || null;
}

function selectFlowTransition(transition) {
  state.flow.selectedTransitionId = transition ? transition.id : null;
  state.flow.selectedBlockId = null;
  drawFlow();
  if (!transition || !transition.edge) return;
  const sourceBlock = flowBlockById(transition.source);
  const targetBlock = flowBlockById(transition.target);
  const selectionKey = registerEdgeSelection(
    transition.edge,
    sourceBlock?.node || null,
    targetBlock?.node || null,
  );
  selectEdgeByKey(selectionKey);
}

// Re-root the Flow view on a block's node so exploration continues "onward":
// double-clicking a call block rebuilds the workflow from that node and pushes
// the current flow onto a breadcrumb trail to walk back.
async function drillIntoFlowBlock(block) {
  const node = block?.node;
  if (!node || node.id == null) return;
  // The start block is already the current root; a group folds many nodes.
  if (block.kind === "start" || Number(block.groupCount || 0) > 1) return;

  state.flowDrillRequest = (state.flowDrillRequest || 0) + 1;
  const requestId = state.flowDrillRequest;
  if (flowHud) flowHud.textContent = t("flow.building");
  const depthInput = document.querySelector("#traceDepthInput");
  const depth = clampNumber(Number(depthInput?.value || 4), 1, 8);
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: String(depth),
    block_limit: "250",
    compact: "true",
  });
  appendWorkflowFilterParams(params, readWorkflowFilters("workflow"));

  try {
    const response = await apiFetch(`/api/workflow?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.flowDrillRequest || state.stageView !== "flow") return;
    if (!response.ok || !Array.isArray(body.blocks) || body.blocks.length === 0) {
      renderFlowHud();
      return;
    }
    if (!Array.isArray(body.transitions) || body.transitions.length === 0) {
      // Leaf: nothing further to explore — keep the current view, just say so.
      if (flowHud) flowHud.textContent = t("flow.noFurther");
      return;
    }
    (state.flow.trail ||= []).push({ report: state.flow.report, title: state.flow.title });
    openFlowView(body, node.label, { keepTrail: true });
  } catch (error) {
    console.error("flow: drill failed", error);
    renderFlowHud();
  }
}

// Jump back to an ancestor flow via a breadcrumb click.
function restoreFlowLevel(index) {
  const trail = state.flow.trail || [];
  if (index < 0 || index >= trail.length) return;
  const level = trail[index];
  state.flow.trail = trail.slice(0, index);
  state.flow.report = level.report;
  state.flow.title = level.title;
  state.flow.selectedBlockId = null;
  state.flow.selectedTransitionId = null;
  layoutFlow();
  fitFlowView();
}

// Collapse a workflow into a readable shape: leaf blocks that share the same
// parent and kind (e.g. the dozens of branch/error/import leaves that hang off
// a CLI `main`) fold into one representative block carrying the group count, so
// a 200-block fan reads as a handful of grouped steps instead of a wall.
function refineFlowReport(report) {
  if (!report || !Array.isArray(report.blocks)) return report;
  const blocks = report.blocks;
  const transitions = Array.isArray(report.transitions) ? report.transitions : [];
  const GROUP_MIN = 3;

  const outgoing = new Map();
  transitions.forEach((t) => outgoing.set(t.source, (outgoing.get(t.source) || 0) + 1));
  const parentOf = new Map();
  transitions.forEach((t) => {
    if (!parentOf.has(t.target)) parentOf.set(t.target, t.source);
  });
  const isLeaf = (id) => !(outgoing.get(id) > 0);

  const groupsByKey = new Map();
  blocks.forEach((block) => {
    if (!isLeaf(block.id)) return;
    const parent = parentOf.get(block.id);
    if (parent == null) return;
    const key = `${parent}::${block.kind}`;
    if (!groupsByKey.has(key)) groupsByKey.set(key, []);
    groupsByKey.get(key).push(block);
  });

  const collapsed = new Map(); // member id -> representative id
  const groupCounts = new Map(); // representative id -> member count
  const groupMembers = new Map(); // representative id -> [node ids]
  groupsByKey.forEach((members) => {
    if (members.length < GROUP_MIN) return;
    const rep = members[0];
    groupCounts.set(rep.id, members.length);
    groupMembers.set(
      rep.id,
      members.map((m) => m.node?.id).filter((id) => id != null),
    );
    members.slice(1).forEach((m) => collapsed.set(m.id, rep.id));
  });

  if (collapsed.size === 0) return report;

  const refinedBlocks = blocks
    .filter((block) => !collapsed.has(block.id))
    .map((block) =>
      groupCounts.has(block.id)
        ? { ...block, groupCount: groupCounts.get(block.id), groupMembers: groupMembers.get(block.id) }
        : block,
    );
  const refinedTransitions = transitions.filter(
    (t) => !collapsed.has(t.source) && !collapsed.has(t.target),
  );

  return {
    ...report,
    blocks: refinedBlocks,
    transitions: refinedTransitions,
    grouped: true,
    raw_total_blocks: report.raw_total_blocks ?? blocks.length,
  };
}

// Convert one ranked journey path (entry -> target) into a flow report so it
// renders in the Flow canvas as a linear, numbered left-to-right chain instead
// of the sidebar list.
function journeyPathToFlowReport(report, path) {
  const steps = Array.isArray(path?.steps) ? path.steps : [];
  const blocks = [];
  const transitions = [];
  const seen = new Set();
  steps.forEach((step, index) => {
    const block = step.block;
    if (block && !seen.has(block.id)) {
      seen.add(block.id);
      blocks.push({ ...block, depth: index });
    }
    if (step.transition) transitions.push(step.transition);
  });
  return {
    start: report.from,
    blocks,
    transitions,
    total_blocks: blocks.length,
    total_transitions: transitions.length,
  };
}

// The chain of transitions/blocks from the flow's start down to a block, so the
// canvas can highlight "how you got here" when a block is selected.
function flowPathToBlock(blockId) {
  const parentEdge = new Map();
  flowTransitions().forEach((transition) => {
    if (!parentEdge.has(transition.target)) parentEdge.set(transition.target, transition);
  });
  const blockIds = new Set([blockId]);
  const transitionIds = new Set();
  let current = blockId;
  let guard = 0;
  while (parentEdge.has(current) && guard < 2000) {
    guard += 1;
    const edge = parentEdge.get(current);
    transitionIds.add(edge.id);
    blockIds.add(edge.source);
    current = edge.source;
  }
  return { blockIds, transitionIds };
}

function drawFlow() {
  if (flowCanvas.hidden) return;
  flowCtx.clearRect(0, 0, flowCanvas.width, flowCanvas.height);
  const report = state.flow.report;
  renderFlowHud();
  if (!report) return;

  const path =
    state.flow.selectedBlockId != null
      ? flowPathToBlock(state.flow.selectedBlockId)
      : { blockIds: new Set(), transitionIds: new Set() };

  flowCtx.save();
  flowCtx.translate(state.flow.pan.x, state.flow.pan.y);
  flowCtx.scale(state.flow.zoom, state.flow.zoom);

  flowTransitions().forEach((transition) => {
    const geometry = flowTransitionGeometry(transition);
    if (!geometry) return;
    const focused =
      state.flow.selectedTransitionId === transition.id ||
      state.flow.hoveredTransitionId === transition.id;
    const onPath = path.transitionIds.has(transition.id);
    const active =
      focused ||
      onPath ||
      state.flow.selectedBlockId === transition.source ||
      state.flow.selectedBlockId === transition.target ||
      state.flow.hoveredBlockId === transition.source ||
      state.flow.hoveredBlockId === transition.target;
    const risky = Array.isArray(transition.risk_refs) && transition.risk_refs.length > 0;
    const { from, c1, c2, to } = geometry;
    flowCtx.beginPath();
    flowCtx.moveTo(from.x, from.y);
    flowCtx.bezierCurveTo(c1.x, c1.y, c2.x, c2.y, to.x, to.y);
    flowCtx.strokeStyle = risky
      ? "rgba(224, 108, 117, 0.85)"
      : onPath
        ? "rgba(122, 162, 247, 0.95)"
        : active
          ? "rgba(92, 200, 167, 0.9)"
          : "rgba(169, 177, 214, 0.42)";
    flowCtx.lineWidth = onPath ? 3 : focused ? 2.8 : active ? 2.2 : 1.3;
    flowCtx.stroke();
    flowCtx.beginPath();
    flowCtx.moveTo(to.x, to.y);
    flowCtx.lineTo(to.x - 7, to.y - 4);
    flowCtx.lineTo(to.x - 7, to.y + 4);
    flowCtx.closePath();
    flowCtx.fillStyle = flowCtx.strokeStyle;
    flowCtx.fill();
  });

  flowBlocks().forEach((block) => {
    const position = state.flow.positions.get(block.id);
    if (!position) return;
    const selected = state.flow.selectedBlockId === block.id;
    const hovered = state.flow.hoveredBlockId === block.id;
    const onPath = !selected && path.blockIds.has(block.id);
    const accent = flowKindColor(block.kind || "unknown");
    const groupCount = Number(block.groupCount || 0);
    // Stacked shadow cards behind a grouped block signal that it folds many.
    if (groupCount > 1) {
      for (let layer = 2; layer >= 1; layer -= 1) {
        flowCtx.beginPath();
        flowCtx.roundRect(position.x + layer * 4, position.y + layer * 4, FLOW_BLOCK_WIDTH, FLOW_BLOCK_HEIGHT, 9);
        flowCtx.fillStyle = "rgba(24, 27, 32, 0.72)";
        flowCtx.fill();
        flowCtx.strokeStyle = "rgba(255, 255, 255, 0.08)";
        flowCtx.lineWidth = 1;
        flowCtx.stroke();
      }
    }
    flowCtx.beginPath();
    flowCtx.roundRect(position.x, position.y, FLOW_BLOCK_WIDTH, FLOW_BLOCK_HEIGHT, 9);
    flowCtx.fillStyle = selected ? "rgba(38, 44, 52, 0.98)" : "rgba(24, 27, 32, 0.96)";
    flowCtx.fill();
    flowCtx.strokeStyle = selected
      ? "#5cc8a7"
      : onPath
        ? "rgba(122, 162, 247, 0.85)"
        : hovered
          ? "rgba(92, 200, 167, 0.6)"
          : "rgba(255, 255, 255, 0.14)";
    flowCtx.lineWidth = selected ? 2 : onPath ? 1.8 : 1.2;
    flowCtx.stroke();
    flowCtx.fillStyle = accent;
    flowCtx.fillRect(position.x, position.y + 6, 3.5, FLOW_BLOCK_HEIGHT - 12);

    flowCtx.fillStyle = accent;
    flowCtx.font = "600 10px 'JetBrains Mono', ui-monospace, monospace";
    flowCtx.textBaseline = "alphabetic";
    const kindText = formatKind(block.kind || "unknown").toUpperCase();
    flowCtx.fillText(
      groupCount > 1 ? `${kindText}  ×${groupCount}` : kindText,
      position.x + 14,
      position.y + 20,
      FLOW_BLOCK_WIDTH - 28,
    );
    flowCtx.fillStyle = "#edf1f2";
    flowCtx.font = "12.5px 'JetBrains Mono', ui-monospace, monospace";
    const label =
      groupCount > 1
        ? t("flow.groupLabel", { count: groupCount, kind: formatKind(block.kind || "unknown") })
        : String(block.node?.label || block.id || "");
    const shortLabel = label.length > 30 ? `${label.slice(0, 29)}…` : label;
    flowCtx.fillText(shortLabel, position.x + 14, position.y + 40, FLOW_BLOCK_WIDTH - 28);

    // Step badge (depth) so the left-to-right execution order reads clearly.
    const step = Number(block.depth || 0);
    const badgeX = position.x + FLOW_BLOCK_WIDTH - 15;
    const badgeY = position.y + FLOW_BLOCK_HEIGHT - 13;
    flowCtx.beginPath();
    flowCtx.arc(badgeX, badgeY, 9, 0, Math.PI * 2);
    flowCtx.fillStyle = "rgba(122, 162, 247, 0.2)";
    flowCtx.fill();
    flowCtx.fillStyle = "rgba(178, 201, 255, 0.95)";
    flowCtx.font = "600 10px 'JetBrains Mono', ui-monospace, monospace";
    flowCtx.textAlign = "center";
    flowCtx.textBaseline = "middle";
    flowCtx.fillText(String(step), badgeX, badgeY + 0.5);
    flowCtx.textAlign = "left";
    flowCtx.textBaseline = "alphabetic";

    const risks = Array.isArray(block.risk_refs) ? block.risk_refs : [];
    if (risks.length > 0) {
      const severity = risks.some((risk) => risk.severity === "error")
        ? "error"
        : risks.some((risk) => risk.severity === "warning")
          ? "warning"
          : "info";
      flowCtx.beginPath();
      flowCtx.arc(position.x + FLOW_BLOCK_WIDTH - 12, position.y + 12, 4.2, 0, Math.PI * 2);
      flowCtx.fillStyle = riskColor(severity);
      flowCtx.fill();
    }
  });

  flowCtx.restore();

  // A workflow with no transitions is a leaf: the single block is the node
  // itself with nothing downstream. Say so in screen space so it does not read
  // as a broken, half-drawn flow.
  if (report && flowTransitions().length === 0) {
    flowCtx.save();
    flowCtx.fillStyle = "rgba(237, 241, 242, 0.6)";
    flowCtx.font = "13px 'JetBrains Mono', ui-monospace, monospace";
    flowCtx.textAlign = "center";
    flowCtx.textBaseline = "middle";
    flowCtx.fillText(
      t("flow.noOutgoing"),
      flowCanvas.width / 2,
      flowCanvas.height - 54,
      flowCanvas.width - 40,
    );
    flowCtx.textAlign = "left";
    flowCtx.textBaseline = "alphabetic";
    flowCtx.restore();
  }

  drawFlowMinimap();
}

function flowMinimapTransform() {
  const bounds = flowBounds();
  if (!bounds) return null;
  const width = Math.max(1, bounds.maxX - bounds.minX);
  const height = Math.max(1, bounds.maxY - bounds.minY);
  const padding = 10;
  const scale = Math.min(
    (flowMinimapCanvas.width - padding * 2) / width,
    (flowMinimapCanvas.height - padding * 2) / height,
  );
  return {
    bounds,
    scale,
    offsetX: (flowMinimapCanvas.width - width * scale) / 2 - bounds.minX * scale,
    offsetY: (flowMinimapCanvas.height - height * scale) / 2 - bounds.minY * scale,
  };
}

function drawFlowMinimap() {
  if (flowMinimapCanvas.hidden) return;
  flowMinimapCtx.clearRect(0, 0, flowMinimapCanvas.width, flowMinimapCanvas.height);
  flowMinimapCtx.fillStyle = "rgba(16, 18, 20, 0.82)";
  flowMinimapCtx.fillRect(0, 0, flowMinimapCanvas.width, flowMinimapCanvas.height);
  const transform = flowMinimapTransform();
  if (!transform) return;

  flowBlocks().forEach((block) => {
    const position = state.flow.positions.get(block.id);
    if (!position) return;
    flowMinimapCtx.fillStyle =
      state.flow.selectedBlockId === block.id ? "#ffffff" : flowKindColor(block.kind || "unknown");
    flowMinimapCtx.fillRect(
      position.x * transform.scale + transform.offsetX,
      position.y * transform.scale + transform.offsetY,
      Math.max(2, FLOW_BLOCK_WIDTH * transform.scale),
      Math.max(1.5, FLOW_BLOCK_HEIGHT * transform.scale),
    );
  });

  const viewMinX = (0 - state.flow.pan.x) / state.flow.zoom;
  const viewMinY = (0 - state.flow.pan.y) / state.flow.zoom;
  const viewWidth = flowCanvas.width / state.flow.zoom;
  const viewHeight = flowCanvas.height / state.flow.zoom;
  flowMinimapCtx.strokeStyle = "rgba(92, 200, 167, 0.85)";
  flowMinimapCtx.lineWidth = 1;
  flowMinimapCtx.strokeRect(
    viewMinX * transform.scale + transform.offsetX,
    viewMinY * transform.scale + transform.offsetY,
    viewWidth * transform.scale,
    viewHeight * transform.scale,
  );
}

function renderFlowHud() {
  if (!flowHud) return;
  if (!state.flow.report) {
    flowHud.textContent = t("flow.empty");
    return;
  }
  const blocks = flowBlocks().length;
  const transitions = flowTransitions().length;
  const zoom = Math.round(state.flow.zoom * 100);
  const tail = transitions === 0 ? t("flow.noOutgoing") : t("flow.blockHint");
  const shorten = (value) => {
    const text = String(value || "");
    return text.length > 22 ? `${text.slice(0, 21)}…` : text;
  };
  const trail = state.flow.trail || [];
  const crumbs = [
    ...trail.map((level, index) => ({ label: level.title, index })),
    { label: state.flow.title, index: trail.length, current: true },
  ];
  const crumbHtml = crumbs
    .map((crumb) =>
      crumb.current
        ? `<strong>${escapeHtml(shorten(crumb.label))}</strong>`
        : `<button type="button" class="flow-crumb" data-flow-crumb="${crumb.index}">${escapeHtml(shorten(crumb.label))}</button>`,
    )
    .join('<span class="flow-crumb-sep">›</span>');
  const meta = `${t("workflow.blockCount", { count: formatNumber(blocks) })} · ${t(
    "workflow.transitionCount",
    { count: formatNumber(transitions) },
  )} · ${zoom}% · ${escapeHtml(tail)}`;
  flowHud.innerHTML = `${crumbHtml} · ${meta}`;
}

function selectFlowBlock(block) {
  state.flow.selectedBlockId = block ? block.id : null;
  state.flow.selectedTransitionId = null;
  drawFlow();
  const nodeId = block?.node?.id;
  if (nodeId != null) {
    selectNodeById(nodeId);
    // The node card with its source preview renders in the sidebar Selection
    // panel, far below the flow stage — scroll it into view so a block click
    // is a real drill-down into the implementation.
    document
      .querySelector(".selection")
      ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }
}

function onFlowPointerDown(event) {
  flowCanvas.setPointerCapture(event.pointerId);
  state.flow.lastPointer = { x: event.offsetX, y: event.offsetY };
  const world = flowScreenToWorld(event.offsetX, event.offsetY);
  const hit = flowBlockAt(world);
  if (hit) {
    selectFlowBlock(hit);
    return;
  }
  const transitionHit = flowTransitionAt(world);
  if (transitionHit) selectFlowTransition(transitionHit);
}

function onFlowPointerMove(event) {
  const world = flowScreenToWorld(event.offsetX, event.offsetY);
  const hit = flowBlockAt(world);
  const transitionHit = hit ? null : flowTransitionAt(world);
  const nextHoveredId = hit ? hit.id : null;
  const nextHoveredTransitionId = transitionHit ? transitionHit.id : null;
  if (
    state.flow.hoveredBlockId !== nextHoveredId ||
    state.flow.hoveredTransitionId !== nextHoveredTransitionId
  ) {
    state.flow.hoveredBlockId = nextHoveredId;
    state.flow.hoveredTransitionId = nextHoveredTransitionId;
    drawFlow();
  }
  flowCanvas.style.cursor =
    hit || transitionHit ? "pointer" : event.buttons === 1 ? "grabbing" : "grab";
  if (!state.flow.lastPointer || event.buttons !== 1) return;
  state.flow.pan.x += event.offsetX - state.flow.lastPointer.x;
  state.flow.pan.y += event.offsetY - state.flow.lastPointer.y;
  state.flow.lastPointer = { x: event.offsetX, y: event.offsetY };
  drawFlow();
}

function onFlowPointerUp() {
  state.flow.lastPointer = null;
}

function onFlowDoubleClick(event) {
  const world = flowScreenToWorld(event.offsetX, event.offsetY);
  const hit = flowBlockAt(world);
  if (hit) drillIntoFlowBlock(hit);
}

function onFlowWheel(event) {
  event.preventDefault();
  flowZoomAt(event.offsetX, event.offsetY, event.deltaY > 0 ? 0.9 : 1.1);
}

// Select a block and center the view on it — the landing action for keyboard
// walking so the current step stays in view with its source card open.
function selectAndCenterFlowBlock(block) {
  if (!block) return;
  selectFlowBlock(block);
  const position = state.flow.positions.get(block.id);
  if (position) {
    state.flow.pan.x = flowCanvas.width / 2 - (position.x + FLOW_BLOCK_WIDTH / 2) * state.flow.zoom;
    state.flow.pan.y = flowCanvas.height / 2 - (position.y + FLOW_BLOCK_HEIGHT / 2) * state.flow.zoom;
    drawFlow();
  }
}

// Walk the flow from the keyboard: left/right follow transitions
// upstream/downstream, up/down move between sibling blocks at the same depth.
function flowNavigate(direction) {
  const blocks = flowBlocks();
  if (blocks.length === 0) return;
  const currentId = state.flow.selectedBlockId;
  if (currentId == null) {
    selectAndCenterFlowBlock(blocks.find((block) => block.kind === "start") || blocks[0]);
    return;
  }
  const current = flowBlockById(currentId);
  if (!current) return;
  const transitions = flowTransitions();
  let nextId = null;
  if (direction === "right") {
    nextId = transitions.find((transition) => transition.source === currentId)?.target ?? null;
  } else if (direction === "left") {
    nextId = transitions.find((transition) => transition.target === currentId)?.source ?? null;
  } else {
    const depth = Number(current.depth || 0);
    const peers = blocks.filter((block) => Number(block.depth || 0) === depth);
    const index = peers.findIndex((block) => block.id === currentId);
    const nextIndex = direction === "down" ? index + 1 : index - 1;
    if (nextIndex >= 0 && nextIndex < peers.length) nextId = peers[nextIndex].id;
  }
  if (nextId != null) selectAndCenterFlowBlock(flowBlockById(nextId));
}

function onFlowKeyDown(event) {
  const panStep = event.shiftKey ? 120 : 48;
  // With a block selected the arrows walk the flow; otherwise they pan.
  const navigating = state.flow.selectedBlockId != null && !event.shiftKey;
  if (event.key === "Enter" && state.flow.selectedBlockId != null) {
    event.preventDefault();
    drillIntoFlowBlock(flowBlockById(state.flow.selectedBlockId));
    return;
  }
  if (event.key === "Escape" && state.flow.selectedBlockId != null) {
    event.preventDefault();
    state.flow.selectedBlockId = null;
    drawFlow();
    return;
  }
  if (navigating && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
    event.preventDefault();
    flowNavigate(
      { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" }[event.key],
    );
    return;
  }
  switch (event.key) {
    case "ArrowLeft":
      event.preventDefault();
      state.flow.pan.x += panStep;
      drawFlow();
      break;
    case "ArrowRight":
      event.preventDefault();
      state.flow.pan.x -= panStep;
      drawFlow();
      break;
    case "ArrowUp":
      event.preventDefault();
      state.flow.pan.y += panStep;
      drawFlow();
      break;
    case "ArrowDown":
      event.preventDefault();
      state.flow.pan.y -= panStep;
      drawFlow();
      break;
    case "+":
    case "=":
      event.preventDefault();
      flowZoomAtCenter(1.12);
      break;
    case "-":
    case "_":
      event.preventDefault();
      flowZoomAtCenter(0.88);
      break;
    case "Home":
      event.preventDefault();
      fitFlowView();
      break;
    default:
      break;
  }
}

function recenterFlowFromMinimap(event) {
  const transform = flowMinimapTransform();
  if (!transform) return;
  const world = {
    x: (event.offsetX - transform.offsetX) / transform.scale,
    y: (event.offsetY - transform.offsetY) / transform.scale,
  };
  state.flow.pan.x = flowCanvas.width / 2 - world.x * state.flow.zoom;
  state.flow.pan.y = flowCanvas.height / 2 - world.y * state.flow.zoom;
  drawFlow();
}

function onFlowMinimapPointerDown(event) {
  event.preventDefault();
  event.stopPropagation();
  flowMinimapCanvas.setPointerCapture(event.pointerId);
  recenterFlowFromMinimap(event);
}

function onFlowMinimapPointerMove(event) {
  if (event.buttons !== 1) return;
  event.preventDefault();
  event.stopPropagation();
  recenterFlowFromMinimap(event);
}

function attachFlowViewActions(container, resolveReport, titleFor) {
  container.querySelectorAll("[data-flow-view]").forEach((button, index) => {
    button.addEventListener("click", () => {
      const report = resolveReport(index);
      if (!report) return;
      openFlowView(report, titleFor ? titleFor(report, index) : "");
    });
  });
}

function nodeRadius(node) {
  switch (node.kind) {
    case "repository":
      return 15;
    case "file":
      return 10;
    case "function":
      return 8;
    case "entrypoint":
      return 10;
    case "type":
      return 9;
    default:
      return 7;
  }
}

function nodeRadiusById(nodeId) {
  const node = state.graph.nodes.find((candidate) => candidate.id === nodeId);
  return node ? nodeRadius(node) : 7;
}

function colorFor(kind) {
  return colors[kind] || colors.unknown;
}

function riskColor(severity) {
  switch (severity) {
    case "error":
      return "rgba(224, 108, 117, 0.95)";
    case "warning":
      return "rgba(242, 193, 78, 0.95)";
    case "info":
      return "rgba(92, 200, 167, 0.82)";
    default:
      return "rgba(237, 241, 242, 0.72)";
  }
}

function nodeIsFocused(node) {
  return Boolean(state.queryFocus?.nodeIds?.has(node.id));
}

function graphNeighborhoodContext(visibleIds) {
  const anchorId = state.selectedId ?? state.hoveredId;
  if (anchorId == null || !visibleIds.has(anchorId)) return null;
  const nodeIds = new Set([anchorId]);
  const edgeKeys = new Set();
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    if (!edgeTouchesNode(edge, anchorId)) return;
    nodeIds.add(edge.source);
    nodeIds.add(edge.target);
    edgeKeys.add(edgeKey(edge));
  });
  return {
    anchorId,
    nodeIds,
    edgeKeys,
    mode: state.selectedId != null ? "selected" : "hover",
  };
}

function nodeIsNeighborhoodNeighbor(node, neighborhood) {
  return Boolean(
    neighborhood &&
      node.id !== neighborhood.anchorId &&
      neighborhood.nodeIds.has(node.id),
  );
}

function nodeIsNeighborhoodMuted(node, neighborhood, selected, hovered, focused) {
  return Boolean(
    neighborhood &&
      !selected &&
      !hovered &&
      !focused &&
      !neighborhood.nodeIds.has(node.id),
  );
}

function edgeNeighborhoodAlpha(edge, neighborhood) {
  if (!neighborhood) return 1;
  return neighborhood.edgeKeys.has(edgeKey(edge)) ? 1 : 0.28;
}

function edgeEmphasis(edge) {
  if (edgeSelectionKey(edge) === state.selectedEdgeKey) return "selected";
  if (state.queryFocus?.edgeKeys?.has(edgeKey(edge))) return "focus";
  if (edgeSelectionKey(edge) === state.hoveredEdgeKey) return "hover";
  if (state.selectedId != null && edgeTouchesNode(edge, state.selectedId)) return "selected-node";
  if (state.hoveredId != null && edgeTouchesNode(edge, state.hoveredId)) return "hover-node";
  return "normal";
}

function edgeTouchesNode(edge, nodeId) {
  return edge.source === nodeId || edge.target === nodeId;
}

function edgeBackplateWidth(emphasis) {
  if (emphasis === "hover-node") return 4.2;
  if (emphasis === "selected-node") return 5;
  return emphasis === "hover" ? 5 : 6;
}

function edgeStrokeWidth(emphasis) {
  if (emphasis === "normal") return 1;
  if (emphasis === "hover-node") return 1.8;
  if (emphasis === "selected-node") return 2.4;
  if (emphasis === "hover") return 2.4;
  return 3.2;
}

function edgeHighlightColor(emphasis) {
  if (emphasis === "selected") return "rgba(92, 200, 167, 0.98)";
  if (emphasis === "hover") return "rgba(237, 241, 242, 0.92)";
  if (emphasis === "selected-node") return "rgba(92, 200, 167, 0.86)";
  if (emphasis === "hover-node") return "rgba(237, 241, 242, 0.72)";
  return state.queryFocus?.mode === "path" ? "rgba(92, 200, 167, 0.98)" : "rgba(237, 241, 242, 0.9)";
}

function edgeColor(edge) {
  switch (edge.kind) {
    case "calls":
      return "rgba(242, 193, 78, 0.72)";
    case "entrypoint":
      return "rgba(92, 200, 167, 0.82)";
    case "references":
      return "rgba(103, 183, 220, 0.58)";
    case "imports":
      return "rgba(184, 142, 230, 0.5)";
    case "depends_on":
      return "rgba(87, 178, 142, 0.68)";
    case "reads_environment":
      return "rgba(216, 166, 87, 0.72)";
    case "reads_config":
      return "rgba(229, 180, 84, 0.78)";
    case "may_error":
      return "rgba(224, 108, 117, 0.78)";
    default:
      return "rgba(170, 184, 190, 0.28)";
  }
}

function formatKind(value) {
  const raw = String(value);
  return translate(`kind.${raw}`, raw.replaceAll("_", " "));
}

function formatNumber(value) {
  const number = Number(value || 0);
  if (!Number.isFinite(number)) return "0";
  return new Intl.NumberFormat(state.locale).format(number);
}

function formatCompactNumber(value) {
  const number = Number(value || 0);
  if (!Number.isFinite(number)) return "0";
  return new Intl.NumberFormat(state.locale, {
    notation: Math.abs(number) >= 10_000 ? "compact" : "standard",
    maximumFractionDigits: 1,
  }).format(number);
}

function formatBytes(value) {
  const bytes = Number(value || 0);
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  let unitIndex = 0;
  let size = bytes;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const digits = unitIndex === 0 || size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}

function setStatus(text, className = "") {
  statusEl.textContent = translate(`status.${text}`, text);
  statusEl.dataset.status = text;
  statusEl.className = `status ${className}`.trim();
}

function clampNumber(value, min, max) {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, Math.trunc(value)));
}

function roundRect(context, x, y, width, height, radius) {
  context.beginPath();
  context.moveTo(x + radius, y);
  context.lineTo(x + width - radius, y);
  context.quadraticCurveTo(x + width, y, x + width, y + radius);
  context.lineTo(x + width, y + height - radius);
  context.quadraticCurveTo(x + width, y + height, x + width - radius, y + height);
  context.lineTo(x + radius, y + height);
  context.quadraticCurveTo(x, y + height, x, y + height - radius);
  context.lineTo(x, y + radius);
  context.quadraticCurveTo(x, y, x + radius, y);
  context.closePath();
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}
