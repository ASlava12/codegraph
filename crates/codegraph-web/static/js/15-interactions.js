// 15-interactions.js — edge explanations and pointer/keyboard interaction.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function renderExplainEdgeButton(edge) {
  const edgeIndex = edgeIndexOf(edge);
  return `
    <button
      class="edge-explain-button"
      type="button"
      data-explain-edge
      ${edgeIndex == null ? "" : `data-edge-index="${edgeIndex}"`}
      data-edge-source="n${edge.source}"
      data-edge-target="n${edge.target}"
      data-edge-kind="${escapeHtml(edge.kind)}"
    >${escapeHtml(t("button.explain"))}</button>
  `;
}

function registerEdgeSelection(edge, source = null, target = null) {
  const selectionKey = edgeSelectionKey(edge);
  state.edgeSelectionCache.set(selectionKey, edge);
  if (source || target) {
    state.edgeSelectionNodeCache.set(selectionKey, { source, target });
  }
  return selectionKey;
}

function selectEdgeByKey(selectionKey, options = {}) {
  if (!selectionKey) return;
  const syncUrl = options.syncUrl !== false;
  state.selectedId = null;
  state.selectedEdgeKey = selectionKey;
  state.hoveredEdgeKey = null;
  if (syncUrl) syncSelectionUrl();
  renderSelection();
  draw();
}

async function explainEdge(button) {
  const container = button.closest("li") || button.closest(".edge-row")?.parentElement;
  const target = container?.querySelector("[data-edge-explanation]");
  if (!target) return;

  state.edgeExplainRequest += 1;
  const requestId = String(state.edgeExplainRequest);
  button.dataset.explainToken = requestId;
  target.hidden = false;
  target.innerHTML = '<p class="empty">Explaining edge...</p>';
  button.disabled = true;

  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  if (button.dataset.edgeIndex) {
    params.set("edge_index", button.dataset.edgeIndex);
  } else {
    params.set("source", button.dataset.edgeSource || "");
    params.set("target", button.dataset.edgeTarget || "");
    params.set("kind", button.dataset.edgeKind || "");
  }

  try {
    const response = await apiFetch(`/api/explain-edge?${params.toString()}`);
    const body = await response.json();
    if (button.dataset.explainToken !== requestId) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "edge explanation failed"));
    }
    target.innerHTML = renderEdgeExplanation(body);
    attachEdgeExplanationInsights(target, body);
  } catch (error) {
    if (button.dataset.explainToken !== requestId) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (button.dataset.explainToken === requestId) {
      button.disabled = false;
      delete button.dataset.explainToken;
    }
  }
}

function renderEdgeExplanation(explanation) {
  if (!explanation) {
    return '<p class="empty">No matching edge explanation.</p>';
  }
  const evidence = (explanation.evidence || [])
    .map((item) => `<li>${escapeHtml(item)}</li>`)
    .join("");
  const insights = Array.isArray(explanation.insights) ? explanation.insights.slice(0, 8) : [];
  const riskSummary = renderNodeRiskSummary(explanation.insight_summary);
  const riskRows = insights
    .map((insight, index) => renderEdgeExplanationInsight(insight, index))
    .join("");
  const totalInsights = Number(explanation.total_insights || insights.length);
  const insightLimit = Number(explanation.insight_limit || insights.length);
  const riskLimitNote =
    explanation.truncated_insights && totalInsights > insightLimit
      ? `<p class="empty">${escapeHtml(formatReturnedCount(insights.length, totalInsights))} ${escapeHtml(t("selection.risks").toLowerCase())}</p>`
      : "";
  const matchNote =
    explanation.total_matches > 1
      ? `<span>${explanation.total_matches} matches, showing first</span>`
      : "";

  return `
    <div class="edge-explanation-summary">
      <strong>${escapeHtml(explanation.summary)}</strong>
      <span>edge ${explanation.edge_index}</span>
      ${matchNote}
    </div>
    ${evidence ? `<ul>${evidence}</ul>` : '<p class="empty">No evidence metadata.</p>'}
    ${
      totalInsights > 0
        ? `<section class="edge-explanation-risks">
            <h4>${escapeHtml(t("selection.risks"))}</h4>
            ${riskSummary}
            <div class="node-issues">${riskRows}</div>
            ${riskLimitNote}
          </section>`
        : ""
    }
  `;
}

function renderEdgeExplanationInsight(insight, index) {
  const severity = insight.severity || "info";
  return `
    <button class="node-issue ${escapeHtml(severity)}" type="button" data-edge-insight-index="${index}">
      <span>${escapeHtml(formatKind(severity))} · ${escapeHtml(formatKind(insight.kind || "insight"))}</span>
      <strong>${escapeHtml(insight.message || "")}</strong>
      <em>${escapeHtml(t("selection.issueHint"))}</em>
    </button>
  `;
}

function attachEdgeExplanationInsights(container, explanation) {
  const insights = Array.isArray(explanation?.insights) ? explanation.insights : [];
  container.querySelectorAll("[data-edge-insight-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const insight = insights[Number(button.dataset.edgeInsightIndex)];
      if (insight) focusInsight(insight);
    });
  });
}

function edgeFacts(edge) {
  const facts = [];
  if (edge.confidence) facts.push(formatKind(edge.confidence));
  const metadata = edge.metadata || {};
  for (const key of [
    "source",
    "relation",
    "resolution",
    "dependency_kind",
    "dependency_version",
    "target_symbol",
  ]) {
    if (metadata[key]) facts.push(`${formatKind(key)}: ${metadata[key]}`);
  }
  return facts;
}

function edgeKey(edge) {
  return `${edge.source}->${edge.target}:${edge.kind}`;
}

function edgeIndexOf(edge) {
  const value = edge?.metadata?.edge_index;
  if (value == null || value === "") return null;
  const index = Number(value);
  return Number.isInteger(index) && index >= 0 ? index : null;
}

function edgeSelectionKey(edge) {
  const index = edgeIndexOf(edge);
  return index == null ? edgeKey(edge) : `edge:${index}`;
}

function centerViewportOnWorld(point) {
  state.pan.x = canvas.width / 2 - point.x * state.zoom;
  state.pan.y = canvas.height / 2 - point.y * state.zoom;
  draw();
}

function onPointerDown(event) {
  canvas.setPointerCapture(event.pointerId);
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  state.lastPointer = { x: event.offsetX, y: event.offsetY };
  if (hit) {
    selectNodeById(hit.id);
    state.draggingId = hit.id;
  } else {
    const edgeHit = findEdgeAt(world);
    if (edgeHit) {
      selectEdgeByKey(edgeSelectionKey(edgeHit));
    }
    state.draggingId = null;
  }
}

function onPointerMove(event) {
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  const edgeHit = hit ? null : findEdgeAt(world);
  const nextHoveredEdgeKey = edgeHit ? edgeSelectionKey(edgeHit) : null;
  const hoverChanged = state.hoveredEdgeKey !== nextHoveredEdgeKey || state.hoveredId !== (hit ? hit.id : null);
  state.hoveredId = hit ? hit.id : null;
  state.hoveredEdgeKey = nextHoveredEdgeKey;
  canvas.style.cursor = hit || edgeHit ? "pointer" : event.buttons === 1 ? "grabbing" : "";
  if (hoverChanged) draw();

  if (!state.lastPointer) return;

  if (state.draggingId) {
    const position = state.positions.get(state.draggingId);
    position.x = world.x;
    position.y = world.y;
    const velocity = state.velocities.get(state.draggingId);
    velocity.x = 0;
    velocity.y = 0;
  } else if (event.buttons === 1) {
    state.pan.x += event.offsetX - state.lastPointer.x;
    state.pan.y += event.offsetY - state.lastPointer.y;
  }

  state.lastPointer = { x: event.offsetX, y: event.offsetY };
}

function onPointerUp() {
  state.draggingId = null;
  state.lastPointer = null;
}

function onPointerLeave() {
  onPointerUp();
  if (state.hoveredId != null || state.hoveredEdgeKey != null) {
    state.hoveredId = null;
    state.hoveredEdgeKey = null;
    draw();
  }
  canvas.style.cursor = "";
}

function recenterFromMinimapEvent(event) {
  const transform = minimapTransform();
  if (!transform) return;
  const world = minimapToWorld({ x: event.offsetX, y: event.offsetY }, transform);
  centerViewportOnWorld(world);
}

function onMinimapPointerDown(event) {
  event.preventDefault();
  event.stopPropagation();
  minimapCanvas.setPointerCapture(event.pointerId);
  recenterFromMinimapEvent(event);
}

function onMinimapPointerMove(event) {
  if (event.buttons !== 1) return;
  event.preventDefault();
  event.stopPropagation();
  recenterFromMinimapEvent(event);
}

function onMinimapPointerUp(event) {
  if (minimapCanvas.hasPointerCapture(event.pointerId)) {
    minimapCanvas.releasePointerCapture(event.pointerId);
  }
}

function onWheel(event) {
  event.preventDefault();
  const delta = event.deltaY > 0 ? 0.9 : 1.1;
  zoomAt(event.offsetX, event.offsetY, delta);
}

function onCanvasKeyDown(event) {
  if (state.graph.nodes.length === 0) return;
  const panStep = event.shiftKey ? 120 : 48;
  switch (event.key) {
    case "ArrowLeft":
      event.preventDefault();
      panGraphBy(panStep, 0);
      break;
    case "ArrowRight":
      event.preventDefault();
      panGraphBy(-panStep, 0);
      break;
    case "ArrowUp":
      event.preventDefault();
      panGraphBy(0, panStep);
      break;
    case "ArrowDown":
      event.preventDefault();
      panGraphBy(0, -panStep);
      break;
    case "+":
    case "=":
      event.preventDefault();
      zoomAtCanvasCenter(1.12);
      break;
    case "-":
    case "_":
      event.preventDefault();
      zoomAtCanvasCenter(0.88);
      break;
    case "Home":
      event.preventDefault();
      fitVisibleGraph();
      break;
    case "0":
      event.preventDefault();
      resetGraphLayout();
      break;
    case " ":
      event.preventDefault();
      toggleLayout();
      break;
    default:
      break;
  }
}

function screenToWorld(x, y) {
  return {
    x: (x - state.pan.x) / state.zoom,
    y: (y - state.pan.y) / state.zoom,
  };
}

function findNodeAt(point) {
  for (let i = state.visibleNodes.length - 1; i >= 0; i -= 1) {
    const node = state.visibleNodes[i];
    const position = state.positions.get(node.id);
    const radius = nodeRadius(node) + 5;
    const dx = point.x - position.x;
    const dy = point.y - position.y;
    if (dx * dx + dy * dy <= radius * radius) return node;
  }
  return null;
}

function findEdgeAt(point) {
  const tolerance = Math.max(7, 12 / Math.max(0.18, state.zoom));
  let best = null;
  let bestDistance = tolerance;
  for (const edge of state.visibleEdges) {
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    if (!source || !target) continue;
    const distance = distanceToSegment(point, source, target);
    if (distance <= bestDistance) {
      best = edge;
      bestDistance = distance;
    }
  }
  return best;
}

function distanceToSegment(point, start, end) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSq = dx * dx + dy * dy;
  if (lengthSq === 0) {
    const px = point.x - start.x;
    const py = point.y - start.y;
    return Math.sqrt(px * px + py * py);
  }
  const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSq));
  const x = start.x + t * dx;
  const y = start.y + t * dy;
  const px = point.x - x;
  const py = point.y - y;
  return Math.sqrt(px * px + py * py);
}

function resizeCanvas() {
  const previousWidth = canvas.width;
  const previousHeight = canvas.height;
  const rect = canvas.getBoundingClientRect();
  canvas.width = Math.max(1, Math.floor(rect.width));
  canvas.height = Math.max(1, Math.floor(rect.height));
  if (previousWidth > 1 && previousHeight > 1) {
    state.pan.x += (canvas.width - previousWidth) / 2;
    state.pan.y += (canvas.height - previousHeight) / 2;
  } else {
    state.pan = { x: canvas.width / 2, y: canvas.height / 2 };
  }
  draw();
}

const FLOW_BLOCK_WIDTH = 224;
const FLOW_BLOCK_HEIGHT = 58;
const FLOW_COLUMN_GAP = 120;
const FLOW_ROW_GAP = 26;
const flowKindColors = {
  start: "#5cc8a7",
  call: "#7aa2f7",
  branch: "#f2c14e",
  loop: "#e0af68",
  async: "#bb9af7",
  return: "#9ece6a",
  error: "#e06c75",
  config_read: "#73daca",
  environment_read: "#73daca",
  dependency: "#c0caf5",
  import: "#8a93b5",
  reference: "#a9b1d6",
  external_boundary: "#ff9e64",
  unknown: "#8b949e",
};

function flowKindColor(kind) {
  return flowKindColors[kind] || flowKindColors.unknown;
}

function setStageView(view) {
  const next = view === "flow" ? "flow" : "graph";
  state.stageView = next;
  if (graphStage) graphStage.dataset.view = next;
  const flowActive = next === "flow";
  flowCanvas.hidden = !flowActive;
  flowMinimapCanvas.hidden = !flowActive;
  flowHud.hidden = !flowActive;
  stageViewButtons.forEach((button) => {
    button.setAttribute("aria-pressed", String(button.dataset.stageView === next));
  });
  if (flowActive) {
    resizeFlowCanvas();
    if (!state.flow.report) ensureFlowReportForStage();
  }
}

// Auto-build a workflow when the user opens the Flow view with nothing loaded,
// so the stage is not a blank canvas. Prefer the selected node; otherwise fall
// back to the first project entrypoint. If neither exists, keep the hint.
async function ensureFlowReportForStage() {
  if (state.flow.report) return;
  let node = null;
  if (state.selectedId != null) {
    node = state.graph.nodes.find((candidate) => candidate.id === state.selectedId) || null;
  }
  if (!node) node = (state.entrypoints || [])[0] || null;
  if (!node) {
    renderFlowHud();
    return;
  }

  state.flowAutoRequest = (state.flowAutoRequest || 0) + 1;
  const requestId = state.flowAutoRequest;
  if (flowHud) flowHud.textContent = t("flow.building");
  const depthInput = document.querySelector("#traceDepthInput");
  const depth = clampNumber(Number(depthInput?.value || 4), 1, 8);
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: String(depth),
    block_limit: "120",
    compact: "true",
  });
  appendWorkflowFilterParams(params, readWorkflowFilters("workflow"));

  try {
    const response = await apiFetch(`/api/workflow?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.flowAutoRequest || state.stageView !== "flow") return;
    if (!response.ok || !Array.isArray(body.blocks) || body.blocks.length === 0) {
      if (flowHud) flowHud.textContent = t("flow.noWorkflow");
      return;
    }
    openFlowView(body, node.label);
  } catch (error) {
    console.error("flow: auto-build failed", error);
    if (requestId === state.flowAutoRequest && flowHud) {
      flowHud.textContent = t("flow.noWorkflow");
    }
  }
}

function openFlowView(report, title) {
  if (!report || !Array.isArray(report.blocks) || report.blocks.length === 0) return;
  state.flow.report = report;
  state.flow.title = title || report.start?.label || "";
  state.flow.selectedBlockId = null;
  state.flow.hoveredBlockId = null;
  state.flow.selectedTransitionId = null;
  state.flow.hoveredTransitionId = null;
  layoutFlow();
  setStageView("flow");
  fitFlowView();
  flowCanvas.focus({ preventScroll: true });
}

function flowBlocks() {
  return Array.isArray(state.flow.report?.blocks) ? state.flow.report.blocks : [];
}

function flowTransitions() {
  return Array.isArray(state.flow.report?.transitions) ? state.flow.report.transitions : [];
}

function layoutFlow() {
  const positions = new Map();
  const columns = new Map();
  flowBlocks().forEach((block) => {
    const depth = Number(block.depth || 0);
    if (!columns.has(depth)) columns.set(depth, []);
    columns.get(depth).push(block);
  });
  const depths = [...columns.keys()].sort((left, right) => left - right);
  const maxRows = Math.max(1, ...depths.map((depth) => columns.get(depth).length));
  const totalHeight = maxRows * (FLOW_BLOCK_HEIGHT + FLOW_ROW_GAP);
  depths.forEach((depth, columnIndex) => {
    const blocks = columns.get(depth);
    const columnHeight = blocks.length * (FLOW_BLOCK_HEIGHT + FLOW_ROW_GAP);
    const startY = (totalHeight - columnHeight) / 2;
    blocks.forEach((block, rowIndex) => {
      positions.set(block.id, {
        x: columnIndex * (FLOW_BLOCK_WIDTH + FLOW_COLUMN_GAP),
        y: startY + rowIndex * (FLOW_BLOCK_HEIGHT + FLOW_ROW_GAP),
      });
    });
  });
  state.flow.positions = positions;
}

function resizeFlowCanvas() {
  if (flowCanvas.hidden) return;
  const rect = flowCanvas.getBoundingClientRect();
  flowCanvas.width = Math.max(1, Math.floor(rect.width));
  flowCanvas.height = Math.max(1, Math.floor(rect.height));
  const minimapRect = flowMinimapCanvas.getBoundingClientRect();
  flowMinimapCanvas.width = Math.max(1, Math.floor(minimapRect.width));
  flowMinimapCanvas.height = Math.max(1, Math.floor(minimapRect.height));
  drawFlow();
}

function flowScreenToWorld(x, y) {
  return {
    x: (x - state.flow.pan.x) / state.flow.zoom,
    y: (y - state.flow.pan.y) / state.flow.zoom,
  };
}

function flowBounds() {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  state.flow.positions.forEach((position) => {
    minX = Math.min(minX, position.x);
    minY = Math.min(minY, position.y);
    maxX = Math.max(maxX, position.x + FLOW_BLOCK_WIDTH);
    maxY = Math.max(maxY, position.y + FLOW_BLOCK_HEIGHT);
  });
  if (!Number.isFinite(minX)) return null;
  return { minX, minY, maxX, maxY };
}
