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
  queryRequest: 0,
  queryFocus: null,
  scanJobId: null,
  scanEvents: null,
};

const colors = {
  repository: "#5cc8a7",
  directory: "#7f9cff",
  file: "#67b7dc",
  module: "#8ccf7e",
  function: "#f2c14e",
  entrypoint: "#5cc8a7",
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
const queryInput = document.querySelector("#queryInput");
const queryButton = document.querySelector("#queryButton");
const queryResult = document.querySelector("#queryResult");
const insightCount = document.querySelector("#insightCount");
const insightList = document.querySelector("#insightList");
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
queryButton.addEventListener("click", () => runGraphQuery());
queryInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") runGraphQuery();
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
  setStatus("queue", "busy");
  scanButton.disabled = true;
  selectionTitle.textContent = "Selection";
  selectionBody.innerHTML = "";
  if (state.scanEvents) {
    state.scanEvents.close();
    state.scanEvents = null;
  }

  try {
    const response = await fetch("/api/scan-jobs", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path: pathInput.value.trim() || "." }),
    });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "failed to start scan");
    }

    state.scanJobId = body.id;
    await watchScanJob(body.id);
  } catch (error) {
    setStatus("error", "error");
    selectionTitle.textContent = "Error";
    selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    scanButton.disabled = false;
  }
}

async function watchScanJob(jobId) {
  if (!window.EventSource) {
    return pollScanJob(jobId);
  }

  return new Promise((resolve, reject) => {
    let settled = false;
    const events = new EventSource(`/api/scan-jobs/${encodeURIComponent(jobId)}/events`);
    state.scanEvents = events;

    const finish = async () => {
      if (settled) return;
      settled = true;
      events.close();
      if (state.scanEvents === events) state.scanEvents = null;
      try {
        await loadScanJobResult(jobId);
        resolve();
      } catch (error) {
        reject(error);
      }
    };

    events.addEventListener("status", (event) => {
      if (state.scanJobId !== jobId) {
        events.close();
        if (!settled) resolve();
        settled = true;
        return;
      }

      let job;
      try {
        job = JSON.parse(event.data);
      } catch (error) {
        settled = true;
        events.close();
        if (state.scanEvents === events) state.scanEvents = null;
        reject(new Error(`invalid scan event: ${error.message}`));
        return;
      }
      if (job.status === "queued" || job.status === "running") {
        setStatus(job.status === "queued" ? "queue" : "scan", "busy");
        return;
      }

      if (job.status === "failed") {
        settled = true;
        events.close();
        if (state.scanEvents === events) state.scanEvents = null;
        reject(new Error(job.message || "scan failed"));
        return;
      }

      if (job.status === "complete") {
        finish();
      }
    });

    events.onerror = () => {
      if (settled) return;
      settled = true;
      events.close();
      if (state.scanEvents === events) state.scanEvents = null;
      pollScanJob(jobId).then(resolve, reject);
    };
  });
}

async function pollScanJob(jobId) {
  while (state.scanJobId === jobId) {
    const response = await fetch(`/api/scan-jobs/${encodeURIComponent(jobId)}`);
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "scan status failed");
    }

    if (body.status === "queued" || body.status === "running") {
      setStatus(body.status === "queued" ? "queue" : "scan", "busy");
      await sleep(350);
      continue;
    }

    if (body.status === "failed") {
      throw new Error(body.message || "scan failed");
    }

    await loadScanJobResult(jobId);
    return;
  }
}

async function loadScanJobResult(jobId) {
  const resultResponse = await fetch(`/api/scan-jobs/${encodeURIComponent(jobId)}/result`);
  const result = await resultResponse.json();
  if (!resultResponse.ok) {
    throw new Error(result.error || "scan result failed");
  }

  setStatus("load", "busy");
  state.graph = result.graph;
  state.selectedId = null;
  state.hoveredId = null;
  state.queryFocus = null;
  queryResult.innerHTML = "";
  state.positions.clear();
  state.velocities.clear();
  rootLabel.textContent = result.root;
  initializeGraph();
  setStatus("ready");
}

async function runGraphQuery() {
  const expression = queryInput.value.trim();
  if (!expression) {
    queryResult.innerHTML = '<p class="empty">Enter a query expression.</p>';
    return;
  }

  state.queryRequest += 1;
  const requestId = state.queryRequest;
  queryButton.disabled = true;
  queryResult.innerHTML = '<p class="empty">Running query...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: expression,
  });

  try {
    const response = await fetch(`/api/query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.queryRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "query failed");
    }
    queryResult.innerHTML = renderQueryResult(body);
    attachQueryNavigation(queryResult);
    attachQueryFocusActions(queryResult, body);
  } catch (error) {
    if (requestId !== state.queryRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.queryRequest) {
      queryButton.disabled = false;
    }
  }
}

function renderQueryResult(result) {
  const nodeRows = result.nodes
    .slice(0, 40)
    .map((node) => renderQueryNode(node))
    .join("");
  const nodeMap = new Map(result.nodes.map((node) => [node.id, node]));
  const edgeRows = result.edges
    .slice(0, 40)
    .map((edge) => renderQueryEdge(edge, nodeMap))
    .join("");
  const truncated = result.truncated
    ? '<p class="empty">Result truncated by query limit.</p>'
    : "";
  const hasResults = result.nodes.length > 0 || result.edges.length > 0;

  return `
    <div class="query-summary">
      <span>${result.total_nodes} nodes</span>
      <span>${result.total_edges} edges</span>
    </div>
    <div class="query-actions">
      <button id="queryFocusButton" type="button" ${hasResults ? "" : "disabled"}>Focus result</button>
      <button id="queryClearButton" type="button" ${state.queryFocus ? "" : "disabled"}>Clear focus</button>
    </div>
    ${nodeRows ? `<ul class="query-list">${nodeRows}</ul>` : ""}
    ${edgeRows ? `<ul class="query-list query-edge-list">${edgeRows}</ul>` : ""}
    ${!nodeRows && !edgeRows ? '<p class="empty">No query results.</p>' : ""}
    ${truncated}
  `;
}

function renderQueryNode(node) {
  return `
    <li>
      <button class="query-item" type="button" data-node-id="${node.id}">
        <span>${escapeHtml(formatKind(node.kind))}</span>
        <strong>${escapeHtml(node.label)}</strong>
      </button>
    </li>
  `;
}

function renderQueryEdge(edge, nodeMap) {
  const source = nodeMap.get(edge.source) || state.graph.nodes.find((node) => node.id === edge.source);
  const target = nodeMap.get(edge.target) || state.graph.nodes.find((node) => node.id === edge.target);
  return `
    <li>
      <button class="query-item query-edge" type="button" data-node-id="${edge.target}">
        <span>${escapeHtml(formatKind(edge.kind))}</span>
        <strong>${escapeHtml(source?.label || String(edge.source))}</strong>
        <em>${escapeHtml(target?.label || String(edge.target))}</em>
      </button>
    </li>
  `;
}

function attachQueryNavigation(container) {
  container.querySelectorAll("[data-node-id]").forEach((button) => {
    button.addEventListener("click", () => {
      const nodeId = Number(button.dataset.nodeId);
      if (!nodeId) return;
      state.selectedId = nodeId;
      renderSelection();
    });
  });
}

function attachQueryFocusActions(container, result) {
  const focusButton = container.querySelector("#queryFocusButton");
  const clearButton = container.querySelector("#queryClearButton");
  if (focusButton) {
    focusButton.addEventListener("click", () => {
      focusQueryResult(result);
    });
  }
  if (clearButton) {
    clearButton.addEventListener("click", () => {
      clearQueryFocus();
    });
  }
}

function focusQueryResult(result) {
  const nodeIds = new Set(result.nodes.map((node) => node.id));
  const edgeKeys = new Set();
  result.edges.forEach((edge) => {
    nodeIds.add(edge.source);
    nodeIds.add(edge.target);
    edgeKeys.add(edgeKey(edge));
  });

  if (nodeIds.size === 0 && edgeKeys.size === 0) return;

  state.queryFocus = { nodeIds, edgeKeys };
  applyFilters();
  const clearButton = queryResult.querySelector("#queryClearButton");
  if (clearButton) clearButton.disabled = false;
}

function clearQueryFocus() {
  state.queryFocus = null;
  applyFilters();
  const clearButton = queryResult.querySelector("#queryClearButton");
  if (clearButton) clearButton.disabled = true;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
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
    const focusHit = !state.queryFocus || state.queryFocus.nodeIds.has(node.id);
    const searchHit =
      !query ||
      node.label.toLowerCase().includes(query) ||
      node.kind.toLowerCase().includes(query) ||
      Object.values(node.metadata || {}).some((value) =>
        String(value).toLowerCase().includes(query),
      );
    if (kindEnabled && focusHit && searchHit) visibleIds.add(node.id);
    return kindEnabled && focusHit && searchHit;
  });

  state.visibleEdges = state.graph.edges.filter((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) {
      return false;
    }
    return !state.queryFocus || state.queryFocus.edgeKeys.size === 0 || state.queryFocus.edgeKeys.has(edgeKey(edge));
  });

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
  renderInsights();

  if (state.selectedId && !visibleIds.has(state.selectedId)) {
    state.selectedId = null;
  }
  renderSelection();
}

function renderInsights() {
  const insights = buildClientInsights(state.graph).slice(0, 30);
  insightCount.textContent = String(insights.length);
  if (insights.length === 0) {
    insightList.innerHTML = '<p class="empty">No obvious issues in the visible graph.</p>';
    return;
  }

  insightList.innerHTML = insights
    .map(
      (insight) => `
        <button class="insight ${escapeHtml(insight.severity)}" type="button" data-node-id="${insight.nodeId || ""}">
          <span>
            <strong>${escapeHtml(formatKind(insight.kind))}</strong>
            ${escapeHtml(insight.message)}
          </span>
        </button>
      `,
    )
    .join("");

  insightList.querySelectorAll(".insight").forEach((button) => {
    button.addEventListener("click", () => {
      const nodeId = Number(button.dataset.nodeId);
      if (!nodeId) return;
      state.selectedId = nodeId;
      renderSelection();
    });
  });
}

function buildClientInsights(graph) {
  const insights = [];
  const entrypointIds = new Set(
    graph.edges.filter((edge) => edge.kind === "entrypoint").map((edge) => edge.target),
  );
  const calledIds = new Set(
    graph.edges.filter((edge) => edge.kind === "calls").map((edge) => edge.target),
  );

  graph.nodes.forEach((node) => {
    if (node.metadata?.parse_error) {
      insights.push({
        kind: "parse_error",
        severity: "error",
        message: `${node.label} failed to parse`,
        nodeId: node.id,
      });
    } else if (node.metadata?.syntax_errors === "true") {
      insights.push({
        kind: "syntax_error",
        severity: "warning",
        message: `${node.label} contains syntax error nodes`,
        nodeId: node.id,
      });
    }

    if (node.metadata?.item_kind === "call" && node.metadata?.resolution === "unresolved") {
      insights.push({
        kind: "unresolved_call",
        severity: "warning",
        message: `Call target ${node.label} could not be resolved`,
        nodeId: node.id,
      });
    }

    if (node.kind === "function" && !entrypointIds.has(node.id) && !calledIds.has(node.id)) {
      insights.push({
        kind: "orphan_function",
        severity: "info",
        message: `${node.label} has no incoming call edge`,
        nodeId: node.id,
      });
    }
  });

  const functionLabels = new Map();
  graph.nodes
    .filter((node) => node.kind === "function")
    .forEach((node) => {
      const list = functionLabels.get(node.label) || [];
      list.push(node);
      functionLabels.set(node.label, list);
    });
  functionLabels.forEach((nodes, label) => {
    if (nodes.length > 1) {
      insights.push({
        kind: "duplicate_function_label",
        severity: "info",
        message: `${label} appears ${nodes.length} times`,
        nodeId: nodes[0].id,
      });
    }
  });

  graph.edges
    .filter((edge) => edge.kind === "may_error")
    .forEach((edge) => {
      const source = graph.nodes.find((node) => node.id === edge.source);
      const target = graph.nodes.find((node) => node.id === edge.target);
      insights.push({
        kind: "potential_error_flow",
        severity: "warning",
        message: `${source?.label || edge.source} may error via ${target?.label || edge.target}`,
        nodeId: source?.id || target?.id,
      });
    });

  addUndeclaredImportInsights(graph, insights);

  const severityOrder = { error: 0, warning: 1, info: 2 };
  return insights.sort(
    (left, right) =>
      severityOrder[left.severity] - severityOrder[right.severity] ||
      left.kind.localeCompare(right.kind) ||
      left.message.localeCompare(right.message),
  );
}

function addUndeclaredImportInsights(graph, insights) {
  const declared = new Set(
    graph.nodes
      .filter((node) => node.metadata?.item_kind === "dependency" && node.metadata?.package_id)
      .map((node) => node.metadata.package_id),
  );
  const declaredEcosystems = new Set(
    Array.from(declared)
      .map((packageId) => packageId.split(":")[0])
      .filter(Boolean),
  );

  if (declaredEcosystems.size === 0) return;

  const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
  graph.edges
    .filter((edge) => edge.kind === "imports")
    .forEach((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      const candidate = importPackageCandidate(target?.metadata?.language, target?.label || "");
      if (!candidate) return;
      if (!declaredEcosystems.has(candidate.ecosystem)) return;
      if (isDeclaredPackage(declared, candidate.ecosystem, candidate.package)) return;

      insights.push({
        kind: "undeclared_external_import",
        severity: "warning",
        message: `${source?.label || edge.source} imports ${candidate.package} without a matching ${candidate.ecosystem} dependency`,
        nodeId: target?.id || source?.id,
      });
    });
}

function importPackageCandidate(language, label) {
  switch (language) {
    case "rust":
      return rustImportPackage(label);
    case "python":
      return pythonImportPackage(label);
    case "javascript":
    case "typescript":
    case "tsx":
      return jsImportPackage(label);
    case "go":
      return goImportPackage(label);
    default:
      return null;
  }
}

function rustImportPackage(label) {
  const match = label.trim().match(/^use\s+::?\s*([A-Za-z_][A-Za-z0-9_]*)/);
  if (!match) return null;
  const packageName = match[1].toLowerCase();
  if (["std", "core", "alloc", "crate", "self", "super"].includes(packageName)) return null;
  return { ecosystem: "cargo", package: packageName };
}

function pythonImportPackage(label) {
  const value = label.trim();
  const match = value.match(/^import\s+([A-Za-z_][A-Za-z0-9_.-]*)/) ||
    value.match(/^from\s+([A-Za-z_][A-Za-z0-9_.-]*)\s+import\b/);
  if (!match) return null;
  const packageName = normalizePythonPackageName(match[1].split(".")[0]);
  if (!packageName || pythonStdlibPackages.has(packageName)) return null;
  return { ecosystem: "python", package: packageName };
}

function jsImportPackage(label) {
  const moduleName = firstQuotedString(label);
  if (!moduleName) return null;
  if (
    moduleName.startsWith(".") ||
    moduleName.startsWith("/") ||
    moduleName.startsWith("node:") ||
    nodeBuiltinModules.has(moduleName)
  ) {
    return null;
  }

  if (moduleName.startsWith("@")) {
    const [scope, name] = moduleName.split("/");
    if (!scope || !name) return null;
    return { ecosystem: "npm", package: `${scope}/${name}`.toLowerCase() };
  }
  return { ecosystem: "npm", package: moduleName.split("/")[0].toLowerCase() };
}

function goImportPackage(label) {
  for (const moduleName of quotedStrings(label)) {
    if (moduleName.startsWith(".") || moduleName.startsWith("/")) continue;
    const firstSegment = moduleName.split("/")[0];
    if (firstSegment.includes(".")) {
      return { ecosystem: "go", package: moduleName };
    }
  }
  return null;
}

function isDeclaredPackage(declared, ecosystem, packageName) {
  if (ecosystem === "go") {
    return Array.from(declared).some((packageId) => {
      if (!packageId.startsWith("go:")) return false;
      const moduleName = packageId.slice(3);
      return packageName === moduleName || packageName.startsWith(`${moduleName}/`);
    });
  }
  if (ecosystem === "cargo") {
    const canonical = packageName.toLowerCase();
    return (
      declared.has(`cargo:${canonical}`) ||
      declared.has(`cargo:${canonical.replaceAll("_", "-")}`) ||
      declared.has(`cargo:${canonical.replaceAll("-", "_")}`)
    );
  }
  if (ecosystem === "python") {
    return declared.has(`python:${normalizePythonPackageName(packageName)}`);
  }
  return declared.has(`${ecosystem}:${packageName.toLowerCase()}`);
}

function normalizePythonPackageName(value) {
  return value.trim().toLowerCase().replaceAll(/[_.-]+/g, "-");
}

function firstQuotedString(value) {
  return quotedStrings(value)[0] || "";
}

function quotedStrings(value) {
  const matches = [];
  const pattern = /["'`]([^"'`]+)["'`]/g;
  let match = pattern.exec(value);
  while (match) {
    matches.push(match[1]);
    match = pattern.exec(value);
  }
  return matches;
}

const nodeBuiltinModules = new Set([
  "assert",
  "buffer",
  "child_process",
  "cluster",
  "crypto",
  "dgram",
  "dns",
  "events",
  "fs",
  "http",
  "https",
  "module",
  "net",
  "os",
  "path",
  "process",
  "querystring",
  "readline",
  "stream",
  "string_decoder",
  "timers",
  "tls",
  "tty",
  "url",
  "util",
  "vm",
  "zlib",
]);

const pythonStdlibPackages = new Set([
  "abc",
  "argparse",
  "asyncio",
  "base64",
  "collections",
  "contextlib",
  "csv",
  "dataclasses",
  "datetime",
  "functools",
  "glob",
  "hashlib",
  "http",
  "importlib",
  "inspect",
  "io",
  "itertools",
  "json",
  "logging",
  "math",
  "os",
  "pathlib",
  "pickle",
  "random",
  "re",
  "shutil",
  "sqlite3",
  "statistics",
  "string",
  "subprocess",
  "sys",
  "tempfile",
  "threading",
  "time",
  "typing",
  "unittest",
  "urllib",
  "uuid",
  "venv",
  "warnings",
  "xml",
]);

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
      <div class="trace-controls">
        <label class="field compact">
          <span>Depth</span>
          <input id="traceDepthInput" type="number" min="1" max="8" value="3" />
        </label>
        <button id="traceButton" type="button">Trace</button>
      </div>
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
  const depthInput = document.querySelector("#traceDepthInput");
  const depth = clampNumber(Number(depthInput?.value || 3), 1, 8);
  if (depthInput) depthInput.value = String(depth);
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: String(depth),
  });

  try {
    const response = await fetch(`/api/trace?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.traceRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "trace failed");
    }
    target.innerHTML = renderTrace(body);
    attachTraceNavigation(target);
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

  const nodes = [...trace.nodes]
    .sort((left, right) => left.depth - right.depth || left.node.label.localeCompare(right.node.label));
  const nodeMap = new Map(nodes.map(({ node }) => [node.id, node]));
  const nodeRows = nodes
    .map(({ node, depth }) => renderTraceNode(node, depth))
    .join("");
  const edgeRows = trace.edges
    .map((edge) => renderTraceEdge(edge, nodeMap))
    .join("");

  const suffix = trace.truncated ? '<p class="empty">Trace truncated by depth.</p>' : "";
  return `
    <div class="trace-summary">
      <span>${trace.nodes.length} nodes</span>
      <span>${trace.edges.length} edges</span>
      <span>depth ${trace.max_depth}</span>
    </div>
    <div class="trace-columns">
      <section>
        <h3>Nodes</h3>
        <ul class="trace-list">${nodeRows}</ul>
      </section>
      <section>
        <h3>Edges</h3>
        <ul class="trace-list trace-edge-list">${edgeRows}</ul>
      </section>
    </div>
    ${suffix}
  `;
}

function renderTraceNode(node, depth) {
  return `
    <li>
      <button class="trace-node" type="button" data-node-id="${node.id}" style="--depth:${depth}">
        <span>${escapeHtml(formatKind(node.kind))}</span>
        <strong>${escapeHtml(node.label)}</strong>
      </button>
    </li>
  `;
}

function renderTraceEdge(edge, nodeMap) {
  const source = nodeMap.get(edge.source);
  const target = nodeMap.get(edge.target);
  return `
    <li>
      <button class="trace-edge" type="button" data-node-id="${edge.target}">
        <span>${escapeHtml(formatKind(edge.kind))}</span>
        <strong>${escapeHtml(source?.label || String(edge.source))}</strong>
        <em>${escapeHtml(target?.label || String(edge.target))}</em>
      </button>
    </li>
  `;
}

function attachTraceNavigation(container) {
  container.querySelectorAll("[data-node-id]").forEach((button) => {
    button.addEventListener("click", () => {
      const nodeId = Number(button.dataset.nodeId);
      if (!nodeId) return;
      state.selectedId = nodeId;
      renderSelection();
    });
  });
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

function edgeKey(edge) {
  return `${edge.source}->${edge.target}:${edge.kind}`;
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
    case "entrypoint":
      return 10;
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
  return String(value).replaceAll("_", " ");
}

function setStatus(text, className = "") {
  statusEl.textContent = text;
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
