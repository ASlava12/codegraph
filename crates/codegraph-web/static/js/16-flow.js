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

function drawFlow() {
  if (flowCanvas.hidden) return;
  flowCtx.clearRect(0, 0, flowCanvas.width, flowCanvas.height);
  const report = state.flow.report;
  renderFlowHud();
  if (!report) return;

  flowCtx.save();
  flowCtx.translate(state.flow.pan.x, state.flow.pan.y);
  flowCtx.scale(state.flow.zoom, state.flow.zoom);

  flowTransitions().forEach((transition) => {
    const geometry = flowTransitionGeometry(transition);
    if (!geometry) return;
    const focused =
      state.flow.selectedTransitionId === transition.id ||
      state.flow.hoveredTransitionId === transition.id;
    const active =
      focused ||
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
      : active
        ? "rgba(92, 200, 167, 0.9)"
        : "rgba(169, 177, 214, 0.42)";
    flowCtx.lineWidth = focused ? 2.8 : active ? 2.2 : 1.3;
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
    const accent = flowKindColor(block.kind || "unknown");
    flowCtx.beginPath();
    flowCtx.roundRect(position.x, position.y, FLOW_BLOCK_WIDTH, FLOW_BLOCK_HEIGHT, 9);
    flowCtx.fillStyle = selected ? "rgba(38, 44, 52, 0.98)" : "rgba(24, 27, 32, 0.96)";
    flowCtx.fill();
    flowCtx.strokeStyle = selected
      ? "#5cc8a7"
      : hovered
        ? "rgba(92, 200, 167, 0.6)"
        : "rgba(255, 255, 255, 0.14)";
    flowCtx.lineWidth = selected ? 2 : 1.2;
    flowCtx.stroke();
    flowCtx.fillStyle = accent;
    flowCtx.fillRect(position.x, position.y + 6, 3.5, FLOW_BLOCK_HEIGHT - 12);

    flowCtx.fillStyle = accent;
    flowCtx.font = "600 10px 'JetBrains Mono', ui-monospace, monospace";
    flowCtx.textBaseline = "alphabetic";
    flowCtx.fillText(
      formatKind(block.kind || "unknown").toUpperCase(),
      position.x + 14,
      position.y + 20,
      FLOW_BLOCK_WIDTH - 28,
    );
    flowCtx.fillStyle = "#edf1f2";
    flowCtx.font = "12.5px 'JetBrains Mono', ui-monospace, monospace";
    const label = String(block.node?.label || block.id || "");
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
  const title = state.flow.title ? `${state.flow.title} · ` : "";
  flowHud.textContent = `${title}${t("workflow.blockCount", { count: formatNumber(blocks) })} · ${t(
    "workflow.transitionCount",
    { count: formatNumber(transitions) },
  )} · ${zoom}% · ${t("flow.blockHint")}`;
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

function onFlowWheel(event) {
  event.preventDefault();
  flowZoomAt(event.offsetX, event.offsetY, event.deltaY > 0 ? 0.9 : 1.1);
}

function onFlowKeyDown(event) {
  const panStep = event.shiftKey ? 120 : 48;
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
