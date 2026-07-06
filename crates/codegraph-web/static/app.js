const state = {
  graph: { nodes: [], edges: [] },
  visibleNodes: [],
  visibleEdges: [],
  positions: new Map(),
  velocities: new Map(),
  selectedId: null,
  draggingId: null,
  hoveredId: null,
  pan: { x: 0, y: 0 },
  zoom: 1,
  lastPointer: null,
  enabledKinds: new Set(),
  search: "",
  animationFrame: null,
  selectionRequest: 0,
  traceRequest: 0,
};

const colors = {
  repository: "#5cc8a7",
  directory: "#7f9cff",
  file: "#67b7dc",
  module: "#8ccf7e",
  function: "#f2c14e",
  type: "#df7e7e",
  external_dependency: "#b88ee6",
  config: "#e5b454",
  environment: "#d8a657",
  unknown: "#a5adb3",
};

const canvas = document.querySelector("#graphCanvas");
const ctx = canvas.getContext("2d");
const scanButton = document.querySelector("#scanButton");
const pathInput = document.querySelector("#pathInput");
const searchInput = document.querySelector("#searchInput");
const statusEl = document.querySelector("#status");
const rootLabel = document.querySelector("#rootLabel");
const nodeCount = document.querySelector("#nodeCount");
const edgeCount = document.querySelector("#edgeCount");
const callCount = document.querySelector("#callCount");
const envCount = document.querySelector("#envCount");
const configCount = document.querySelector("#configCount");
const errorCount = document.querySelector("#errorCount");
const entryCount = document.querySelector("#entryCount");
const kindFilters = document.querySelector("#kindFilters");
const selectionTitle = document.querySelector("#selectionTitle");
const selectionBody = document.querySelector("#selectionBody");
const legend = document.querySelector("#legend");

scanButton.addEventListener("click", () => scan());
pathInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") scan();
});
searchInput.addEventListener("input", () => {
  state.search = searchInput.value.trim().toLowerCase();
  applyFilters();
});

canvas.addEventListener("pointerdown", onPointerDown);
canvas.addEventListener("pointermove", onPointerMove);
canvas.addEventListener("pointerup", onPointerUp);
canvas.addEventListener("pointerleave", onPointerUp);
canvas.addEventListener("wheel", onWheel, { passive: false });
window.addEventListener("resize", resizeCanvas);

resizeCanvas();
scan();

async function scan() {
  setStatus("scan", "busy");
  scanButton.disabled = true;
  selectionTitle.textContent = "Selection";
  selectionBody.innerHTML = "";

  try {
    const path = encodeURIComponent(pathInput.value.trim() || ".");
    const response = await fetch(`/api/scan?path=${path}`);
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "scan failed");
    }

    state.graph = body.graph;
    state.selectedId = null;
    state.hoveredId = null;
    state.positions.clear();
    state.velocities.clear();
    rootLabel.textContent = body.root;
    initializeGraph();
    setStatus("ready");
  } catch (error) {
    setStatus("error", "error");
    selectionTitle.textContent = "Error";
    selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    scanButton.disabled = false;
  }
}

function initializeGraph() {
  const kinds = [...new Set(state.graph.nodes.map((node) => node.kind))].sort();
  state.enabledKinds = new Set(kinds);
  renderKindFilters(kinds);
  renderLegend(kinds);

  const radius = Math.max(180, Math.min(canvas.width, canvas.height) * 0.28);
  state.graph.nodes.forEach((node, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(1, state.graph.nodes.length);
    state.positions.set(node.id, {
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
    });
    state.velocities.set(node.id, { x: 0, y: 0 });
  });

  state.pan = { x: canvas.width / 2, y: canvas.height / 2 };
  state.zoom = 1;
  applyFilters();
  startAnimation();
}

function applyFilters() {
  const query = state.search;
  const visibleIds = new Set();
  state.visibleNodes = state.graph.nodes.filter((node) => {
    const kindEnabled = state.enabledKinds.has(node.kind);
    const searchHit =
      !query ||
      node.label.toLowerCase().includes(query) ||
      node.kind.toLowerCase().includes(query) ||
      Object.values(node.metadata || {}).some((value) =>
        String(value).toLowerCase().includes(query),
      );
    if (kindEnabled && searchHit) visibleIds.add(node.id);
    return kindEnabled && searchHit;
  });

  state.visibleEdges = state.graph.edges.filter(
    (edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target),
  );

  nodeCount.textContent = String(state.visibleNodes.length);
  edgeCount.textContent = String(state.visibleEdges.length);
  callCount.textContent = String(
    state.visibleEdges.filter((edge) => edge.kind === "calls").length,
  );
  envCount.textContent = String(
    state.visibleEdges.filter((edge) => edge.kind === "reads_environment").length,
  );
  configCount.textContent = String(
    state.visibleEdges.filter((edge) => edge.kind === "reads_config").length,
  );
  errorCount.textContent = String(
    state.visibleEdges.filter((edge) => edge.kind === "may_error").length,
  );
  entryCount.textContent = String(
    state.graph.edges.filter((edge) => edge.kind === "entrypoint").length,
  );

  if (state.selectedId && !visibleIds.has(state.selectedId)) {
    state.selectedId = null;
  }
  renderSelection();
}

function renderKindFilters(kinds) {
  kindFilters.innerHTML = "";
  kinds.forEach((kind) => {
    const label = document.createElement("label");
    label.className = "kind-filter";

    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = true;
    input.addEventListener("change", () => {
      if (input.checked) state.enabledKinds.add(kind);
      else state.enabledKinds.delete(kind);
      applyFilters();
    });

    const swatch = document.createElement("span");
    swatch.className = "swatch";
    swatch.style.background = colorFor(kind);

    const text = document.createElement("span");
    text.textContent = formatKind(kind);

    label.append(input, swatch, text);
    kindFilters.append(label);
  });
}

function renderLegend(kinds) {
  legend.innerHTML = "";
  kinds.forEach((kind) => {
    const item = document.createElement("span");
    item.className = "legend-item";
    const swatch = document.createElement("span");
    swatch.className = "swatch";
    swatch.style.background = colorFor(kind);
    const text = document.createElement("span");
    text.textContent = formatKind(kind);
    item.append(swatch, text);
    legend.append(item);
  });
}

function startAnimation() {
  if (state.animationFrame) cancelAnimationFrame(state.animationFrame);
  const tick = () => {
    simulateLayout();
    draw();
    state.animationFrame = requestAnimationFrame(tick);
  };
  tick();
}

function simulateLayout() {
  const nodes = state.visibleNodes;
  const edges = state.visibleEdges;
  if (nodes.length === 0) return;

  const visibleIds = new Set(nodes.map((node) => node.id));
  const centerPull = 0.004;
  const linkDistance = 112;
  const linkStrength = 0.012;
  const charge = 2800;

  for (let i = 0; i < nodes.length; i += 1) {
    const a = nodes[i];
    const pa = state.positions.get(a.id);
    const va = state.velocities.get(a.id);

    for (let j = i + 1; j < nodes.length; j += 1) {
      const b = nodes[j];
      const pb = state.positions.get(b.id);
      const vb = state.velocities.get(b.id);
      let dx = pa.x - pb.x;
      let dy = pa.y - pb.y;
      let distanceSq = dx * dx + dy * dy + 0.01;
      const distance = Math.sqrt(distanceSq);
      dx /= distance;
      dy /= distance;
      const force = Math.min(6, charge / distanceSq);
      va.x += dx * force;
      va.y += dy * force;
      vb.x -= dx * force;
      vb.y -= dy * force;
    }
  }

  edges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    const sourceVelocity = state.velocities.get(edge.source);
    const targetVelocity = state.velocities.get(edge.target);
    const dx = target.x - source.x;
    const dy = target.y - source.y;
    const distance = Math.max(1, Math.sqrt(dx * dx + dy * dy));
    const force = (distance - linkDistance) * linkStrength;
    const fx = (dx / distance) * force;
    const fy = (dy / distance) * force;
    sourceVelocity.x += fx;
    sourceVelocity.y += fy;
    targetVelocity.x -= fx;
    targetVelocity.y -= fy;
  });

  nodes.forEach((node) => {
    if (node.id === state.draggingId) return;
    const position = state.positions.get(node.id);
    const velocity = state.velocities.get(node.id);
    velocity.x += -position.x * centerPull;
    velocity.y += -position.y * centerPull;
    velocity.x *= 0.82;
    velocity.y *= 0.82;
    position.x += velocity.x;
    position.y += velocity.y;
  });
}

function draw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.save();
  ctx.translate(state.pan.x, state.pan.y);
  ctx.scale(state.zoom, state.zoom);

  const visibleIds = new Set(state.visibleNodes.map((node) => node.id));
  ctx.lineWidth = 1 / state.zoom;
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    ctx.beginPath();
    ctx.moveTo(source.x, source.y);
    ctx.lineTo(target.x, target.y);
    ctx.strokeStyle = edgeColor(edge);
    ctx.stroke();
  });

  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    const selected = node.id === state.selectedId;
    const hovered = node.id === state.hoveredId;
    const radius = nodeRadius(node);

    ctx.beginPath();
    ctx.arc(position.x, position.y, radius + (selected ? 5 : hovered ? 3 : 0), 0, Math.PI * 2);
    ctx.fillStyle = selected ? "rgba(92, 200, 167, 0.24)" : hovered ? "rgba(255,255,255,0.12)" : "rgba(0,0,0,0.22)";
    ctx.fill();

    ctx.beginPath();
    ctx.arc(position.x, position.y, radius, 0, Math.PI * 2);
    ctx.fillStyle = colorFor(node.kind);
    ctx.fill();
    ctx.lineWidth = selected ? 2.5 / state.zoom : 1 / state.zoom;
    ctx.strokeStyle = selected ? "#ffffff" : "rgba(255,255,255,0.55)";
    ctx.stroke();

    if (state.zoom > 0.45 || selected || hovered) {
      drawLabel(node, position, radius);
    }
  });

  ctx.restore();
}

function drawLabel(node, position, radius) {
  const label = node.label.length > 34 ? `${node.label.slice(0, 31)}...` : node.label;
  ctx.font = `${Math.max(11, 12 / state.zoom)}px Inter, sans-serif`;
  const metrics = ctx.measureText(label);
  const width = metrics.width + 12;
  const x = position.x - width / 2;
  const y = position.y + radius + 7;
  ctx.fillStyle = "rgba(13, 15, 16, 0.82)";
  roundRect(ctx, x, y, width, 20 / state.zoom, 5 / state.zoom);
  ctx.fill();
  ctx.fillStyle = "#edf1f2";
  ctx.fillText(label, x + 6, y + 14 / state.zoom);
}

function renderSelection() {
  state.selectionRequest += 1;
  const requestId = state.selectionRequest;
  const node = state.graph.nodes.find((candidate) => candidate.id === state.selectedId);
  if (!node) {
    selectionTitle.textContent = "Selection";
    selectionBody.innerHTML = '<p class="empty">No node selected.</p>';
    return;
  }

  selectionTitle.textContent = node.label;
  const rows = [
    ["Kind", formatKind(node.kind)],
    ["Id", String(node.id)],
  ];
  if (node.span) {
    rows.push(["Path", node.span.path]);
    rows.push(["Lines", `${node.span.start_line}-${node.span.end_line}`]);
  }
  Object.entries(node.metadata || {}).forEach(([key, value]) => rows.push([formatKind(key), value]));

  const neighbors = state.graph.edges
    .filter((edge) => edge.source === node.id || edge.target === node.id)
    .slice(0, 40);

  selectionBody.innerHTML = `
    <table class="detail-table">
      <tbody>
        ${rows
          .map(
            ([key, value]) =>
              `<tr><th>${escapeHtml(key)}</th><td>${escapeHtml(String(value))}</td></tr>`,
          )
          .join("")}
      </tbody>
    </table>
    <div class="neighbors">
      ${neighbors.map((edge) => renderNeighbor(edge, node.id)).join("")}
    </div>
    <section class="trace-panel">
      <button id="traceButton" type="button">Trace dependencies</button>
      <div id="traceResult" class="trace-result"></div>
    </section>
    ${
      node.span
        ? `<section class="source-preview">
            <header>
              <span>Source</span>
              <strong>${escapeHtml(node.span.path)}:${node.span.start_line}</strong>
            </header>
            <pre id="sourcePreview"><code>Loading...</code></pre>
          </section>`
        : ""
    }
  `;

  selectionBody.querySelectorAll(".neighbor").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedId = Number(button.dataset.nodeId);
      renderSelection();
    });
  });

  const traceButton = document.querySelector("#traceButton");
  if (traceButton) {
    traceButton.addEventListener("click", () => loadTrace(node));
  }

  if (node.span) {
    loadSourcePreview(node, requestId);
  }
}

async function loadTrace(node) {
  state.traceRequest += 1;
  const requestId = state.traceRequest;
  const target = document.querySelector("#traceResult");
  if (!target) return;

  target.innerHTML = '<p class="empty">Tracing...</p>';
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: "3",
  });

  try {
    const response = await fetch(`/api/trace?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.traceRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "trace failed");
    }
    target.innerHTML = renderTrace(body);
  } catch (error) {
    if (requestId !== state.traceRequest) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderTrace(trace) {
  if (!trace) {
    return '<p class="empty">No matching start node.</p>';
  }
  if (trace.nodes.length <= 1 && trace.edges.length === 0) {
    return '<p class="empty">No outgoing dependency edges.</p>';
  }

  const rows = trace.nodes
    .sort((left, right) => left.depth - right.depth || left.node.label.localeCompare(right.node.label))
    .map(({ node, depth }) => {
      const indent = "&nbsp;".repeat(depth * 3);
      return `<li>${indent}<span>${escapeHtml(formatKind(node.kind))}</span>${escapeHtml(node.label)}</li>`;
    })
    .join("");

  const suffix = trace.truncated ? '<p class="empty">Trace truncated by depth.</p>' : "";
  return `<ul class="trace-list">${rows}</ul>${suffix}`;
}

async function loadSourcePreview(node, requestId) {
  const preview = document.querySelector("#sourcePreview code");
  if (!preview || !node.span) return;

  const params = new URLSearchParams({
    path: node.span.path,
    start_line: String(node.span.start_line),
    end_line: String(node.span.end_line),
    context: "5",
  });

  try {
    const response = await fetch(`/api/source?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "failed to load source");
    }
    preview.innerHTML = body.lines.map(renderSourceLine).join("");
  } catch (error) {
    if (requestId !== state.selectionRequest) return;
    preview.innerHTML = `<span class="source-error">${escapeHtml(error.message)}</span>`;
  }
}

function renderSourceLine(line) {
  const number = String(line.number).padStart(4, " ");
  const className = line.highlight ? "source-line highlighted" : "source-line";
  return `<span class="${className}"><span class="line-number">${number}</span><span class="line-text">${escapeHtml(line.text || " ")}</span></span>`;
}

function renderNeighbor(edge, selectedId) {
  const otherId = edge.source === selectedId ? edge.target : edge.source;
  const other = state.graph.nodes.find((node) => node.id === otherId);
  const direction = edge.source === selectedId ? "out" : "in";
  return `
    <button type="button" class="neighbor" data-node-id="${otherId}">
      <span>${escapeHtml(direction)} ${escapeHtml(formatKind(edge.kind))}</span>
      <span>${escapeHtml(other ? other.label : String(otherId))}</span>
    </button>
  `;
}

function onPointerDown(event) {
  canvas.setPointerCapture(event.pointerId);
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  state.lastPointer = { x: event.offsetX, y: event.offsetY };
  if (hit) {
    state.selectedId = hit.id;
    state.draggingId = hit.id;
    renderSelection();
  } else {
    state.draggingId = null;
  }
}

function onPointerMove(event) {
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  state.hoveredId = hit ? hit.id : null;

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

function onWheel(event) {
  event.preventDefault();
  const before = screenToWorld(event.offsetX, event.offsetY);
  const delta = event.deltaY > 0 ? 0.9 : 1.1;
  state.zoom = Math.max(0.18, Math.min(3.5, state.zoom * delta));
  const after = screenToWorld(event.offsetX, event.offsetY);
  state.pan.x += (after.x - before.x) * state.zoom;
  state.pan.y += (after.y - before.y) * state.zoom;
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

function resizeCanvas() {
  const rect = canvas.getBoundingClientRect();
  canvas.width = Math.max(1, Math.floor(rect.width));
  canvas.height = Math.max(1, Math.floor(rect.height));
  state.pan = { x: rect.width / 2, y: rect.height / 2 };
}

function nodeRadius(node) {
  switch (node.kind) {
    case "repository":
      return 15;
    case "file":
      return 10;
    case "function":
      return 8;
    case "type":
      return 9;
    default:
      return 7;
  }
}

function colorFor(kind) {
  return colors[kind] || colors.unknown;
}

function edgeColor(edge) {
  switch (edge.kind) {
    case "calls":
      return "rgba(242, 193, 78, 0.72)";
    case "entrypoint":
      return "rgba(92, 200, 167, 0.82)";
    case "imports":
      return "rgba(184, 142, 230, 0.5)";
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
  return String(value).replaceAll("_", " ");
}

function setStatus(text, className = "") {
  statusEl.textContent = text;
  statusEl.className = `status ${className}`.trim();
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
