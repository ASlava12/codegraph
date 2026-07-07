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
  dependentsRequest: 0,
  edgeExplainRequest: 0,
  entryFlowRequest: 0,
  queryRequest: 0,
  pathRequest: 0,
  configTraceRequest: 0,
  errorTraceRequest: 0,
  pageRequest: 0,
  overviewRequest: 0,
  insightRequest: 0,
  insightFocusRequest: 0,
  summary: null,
  entrypoints: [],
  insightReport: null,
  projects: [],
  queryFocus: null,
  scanJobId: null,
  scanEvents: null,
  layoutPaused: false,
  graphPage: {
    nodeOffset: 0,
    nodeLimit: 250,
    edgeLimit: 500,
    totalNodes: 0,
    totalEdges: 0,
    truncatedNodes: false,
    root: "",
  },
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
const projectSelect = document.querySelector("#projectSelect");
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
const overviewTotals = document.querySelector("#overviewTotals");
const languageList = document.querySelector("#languageList");
const confidenceList = document.querySelector("#confidenceList");
const annotationList = document.querySelector("#annotationList");
const entrypointList = document.querySelector("#entrypointList");
const entryFlowSearchInput = document.querySelector("#entryFlowSearchInput");
const entryFlowDepthInput = document.querySelector("#entryFlowDepthInput");
const entryFlowButton = document.querySelector("#entryFlowButton");
const entryFlowResult = document.querySelector("#entryFlowResult");
const pageInfo = document.querySelector("#pageInfo");
const nodeLimitInput = document.querySelector("#nodeLimitInput");
const edgeLimitInput = document.querySelector("#edgeLimitInput");
const serverKindInput = document.querySelector("#serverKindInput");
const serverItemKindInput = document.querySelector("#serverItemKindInput");
const serverLanguageInput = document.querySelector("#serverLanguageInput");
const serverSearchInput = document.querySelector("#serverSearchInput");
const serverEdgeKindInput = document.querySelector("#serverEdgeKindInput");
const serverConfidenceInput = document.querySelector("#serverConfidenceInput");
const pagePrevButton = document.querySelector("#pagePrevButton");
const pageReloadButton = document.querySelector("#pageReloadButton");
const pageNextButton = document.querySelector("#pageNextButton");
const queryInput = document.querySelector("#queryInput");
const queryButton = document.querySelector("#queryButton");
const queryResult = document.querySelector("#queryResult");
const pathFromInput = document.querySelector("#pathFromInput");
const pathToInput = document.querySelector("#pathToInput");
const pathDepthInput = document.querySelector("#pathDepthInput");
const pathEdgeKindInput = document.querySelector("#pathEdgeKindInput");
const pathButton = document.querySelector("#pathButton");
const pathResult = document.querySelector("#pathResult");
const configTraceTargetInput = document.querySelector("#configTraceTargetInput");
const configTraceDepthInput = document.querySelector("#configTraceDepthInput");
const configTraceButton = document.querySelector("#configTraceButton");
const configTraceResult = document.querySelector("#configTraceResult");
const errorTraceTargetInput = document.querySelector("#errorTraceTargetInput");
const errorTraceDepthInput = document.querySelector("#errorTraceDepthInput");
const errorTraceButton = document.querySelector("#errorTraceButton");
const errorTraceResult = document.querySelector("#errorTraceResult");
const insightCount = document.querySelector("#insightCount");
const insightList = document.querySelector("#insightList");
const insightSeverityInput = document.querySelector("#insightSeverityInput");
const insightKindInput = document.querySelector("#insightKindInput");
const insightSearchInput = document.querySelector("#insightSearchInput");
const insightFilterButton = document.querySelector("#insightFilterButton");
const kindFilters = document.querySelector("#kindFilters");
const selectionTitle = document.querySelector("#selectionTitle");
const selectionBody = document.querySelector("#selectionBody");
const legend = document.querySelector("#legend");
const zoomOutButton = document.querySelector("#zoomOutButton");
const zoomInButton = document.querySelector("#zoomInButton");
const fitGraphButton = document.querySelector("#fitGraphButton");
const resetLayoutButton = document.querySelector("#resetLayoutButton");
const toggleLayoutButton = document.querySelector("#toggleLayoutButton");
const viewportInfo = document.querySelector("#viewportInfo");

scanButton.addEventListener("click", () => scan());
projectSelect.addEventListener("change", () => {
  const selected = projectSelect.value;
  if (selected) {
    pathInput.value = selected;
    scan();
  }
});
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
entryFlowButton.addEventListener("click", () => runEntryFlowTrace());
for (const input of [entryFlowSearchInput, entryFlowDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runEntryFlowTrace();
  });
}
pathButton.addEventListener("click", () => runPathQuery());
for (const input of [pathFromInput, pathToInput, pathDepthInput, pathEdgeKindInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runPathQuery();
  });
}
configTraceButton.addEventListener("click", () => runConfigTrace());
for (const input of [configTraceTargetInput, configTraceDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runConfigTrace();
  });
}
errorTraceButton.addEventListener("click", () => runErrorTrace());
for (const input of [errorTraceTargetInput, errorTraceDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runErrorTrace();
  });
}
insightFilterButton.addEventListener("click", () => loadInsights());
for (const input of [insightSeverityInput, insightKindInput, insightSearchInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") loadInsights();
  });
}
pagePrevButton.addEventListener("click", () => shiftGraphPage(-1));
pageNextButton.addEventListener("click", () => shiftGraphPage(1));
pageReloadButton.addEventListener("click", () => loadGraphPage({ resetPage: true }));
zoomOutButton.addEventListener("click", () => zoomAtCanvasCenter(0.82));
zoomInButton.addEventListener("click", () => zoomAtCanvasCenter(1.18));
fitGraphButton.addEventListener("click", () => fitVisibleGraph());
resetLayoutButton.addEventListener("click", () => resetGraphLayout());
toggleLayoutButton.addEventListener("click", () => toggleLayout());
for (const input of [
  nodeLimitInput,
  edgeLimitInput,
  serverKindInput,
  serverItemKindInput,
  serverLanguageInput,
  serverSearchInput,
  serverEdgeKindInput,
  serverConfidenceInput,
]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") loadGraphPage({ resetPage: true });
  });
}

canvas.addEventListener("pointerdown", onPointerDown);
canvas.addEventListener("pointermove", onPointerMove);
canvas.addEventListener("pointerup", onPointerUp);
canvas.addEventListener("pointerleave", onPointerUp);
canvas.addEventListener("wheel", onWheel, { passive: false });
window.addEventListener("resize", resizeCanvas);

resizeCanvas();
init();

async function init() {
  await loadProjects();
  scan();
}

async function loadProjects() {
  try {
    const response = await fetch("/api/projects");
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "projects failed");
    }
    state.projects = body;
    renderProjects();
  } catch (error) {
    state.projects = [];
    projectSelect.innerHTML = '<option value=".">Current root</option>';
  }
}

function renderProjects() {
  if (!state.projects.length) {
    projectSelect.innerHTML = '<option value=".">Current root</option>';
    return;
  }

  projectSelect.innerHTML = state.projects
    .map(
      (project) => `
        <option value="${escapeHtml(project.path)}" ${project.default ? "selected" : ""}>
          ${escapeHtml(project.name)}
        </option>
      `,
    )
    .join("");

  const selected = state.projects.find((project) => project.default) || state.projects[0];
  if (selected) {
    projectSelect.value = selected.path;
    pathInput.value = selected.path;
  }
}

async function scan() {
  setStatus("queue", "busy");
  scanButton.disabled = true;
  selectionTitle.textContent = "Selection";
  selectionBody.innerHTML = "";
  state.insightRequest += 1;
  state.overviewRequest += 1;
  state.summary = null;
  state.entrypoints = [];
  renderOverview();
  state.insightReport = null;
  renderInsights();
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

    const finish = async (job) => {
      if (settled) return;
      settled = true;
      events.close();
      if (state.scanEvents === events) state.scanEvents = null;
      try {
        await loadGraphPage({ root: job?.path, resetPage: true, resetLayout: true });
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
        finish(job);
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

    await loadGraphPage({ root: body.path, resetPage: true, resetLayout: true });
    return;
  }
}

async function loadGraphPage({ root = null, resetPage = false, resetLayout = false } = {}) {
  state.pageRequest += 1;
  state.insightRequest += 1;
  const requestId = state.pageRequest;
  setStatus("page", "busy");
  pageReloadButton.disabled = true;
  pagePrevButton.disabled = true;
  pageNextButton.disabled = true;

  if (resetPage) {
    state.graphPage.nodeOffset = 0;
  }

  const nodeLimit = clampNumber(Number(nodeLimitInput.value || 250), 20, 1000);
  const edgeLimit = clampNumber(Number(edgeLimitInput.value || 500), 1, 2000);
  nodeLimitInput.value = String(nodeLimit);
  edgeLimitInput.value = String(edgeLimit);
  state.graphPage.nodeLimit = nodeLimit;
  state.graphPage.edgeLimit = edgeLimit;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_offset: String(state.graphPage.nodeOffset),
    node_limit: String(nodeLimit),
    edge_limit: String(edgeLimit),
  });
  const kind = serverKindInput.value.trim();
  const itemKind = serverItemKindInput.value.trim();
  const language = serverLanguageInput.value.trim();
  const serverSearch = serverSearchInput.value.trim();
  const edgeKind = serverEdgeKindInput.value.trim();
  const confidence = serverConfidenceInput.value.trim();
  if (kind) params.set("kind", kind);
  if (itemKind) params.set("item_kind", itemKind);
  if (language) params.set("language", language);
  if (serverSearch) params.set("search", serverSearch);
  if (edgeKind) params.set("edge_kind", edgeKind);
  if (confidence) params.set("confidence", confidence);

  try {
    const response = await fetch(`/api/graph?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.pageRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "graph page failed");
    }

    state.graph = { nodes: body.nodes, edges: body.edges };
    state.graphPage.totalNodes = body.total_nodes;
    state.graphPage.totalEdges = body.total_edges;
    state.graphPage.nodeOffset = body.node_offset;
    state.graphPage.nodeLimit = body.node_limit;
    state.graphPage.edgeLimit = body.edge_limit;
    state.graphPage.truncatedNodes = body.truncated_nodes;
    state.graphPage.root = root || state.graphPage.root || pathInput.value.trim() || ".";
    state.selectedId = null;
    state.hoveredId = null;
    state.queryFocus = null;
    state.insightReport = null;
    queryResult.innerHTML = "";
    entryFlowResult.innerHTML = "";
    pathResult.innerHTML = "";
    configTraceResult.innerHTML = "";
    errorTraceResult.innerHTML = "";
    rootLabel.textContent = state.graphPage.root;
    initializeGraph({ preserveView: !resetLayout });
    loadProjectOverview();
    loadInsights();
    setStatus("ready");
  } catch (error) {
    if (requestId !== state.pageRequest) return;
    setStatus("error", "error");
    selectionTitle.textContent = "Error";
    selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.pageRequest) {
      updateGraphPageControls();
    }
  }
}

async function loadProjectOverview() {
  state.overviewRequest += 1;
  const requestId = state.overviewRequest;
  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });

  try {
    const [summaryResponse, entrypointsResponse] = await Promise.all([
      fetch(`/api/summary?${params.toString()}`),
      fetch(`/api/entrypoints?${params.toString()}`),
    ]);
    const summary = await summaryResponse.json();
    const entrypoints = await entrypointsResponse.json();
    if (requestId !== state.overviewRequest) return;
    if (!summaryResponse.ok) {
      throw new Error(summary.error || "summary failed");
    }
    if (!entrypointsResponse.ok) {
      throw new Error(entrypoints.error || "entrypoints failed");
    }
    state.summary = summary;
    state.entrypoints = entrypoints;
    renderOverview();
  } catch (error) {
    if (requestId !== state.overviewRequest) return;
    overviewTotals.textContent = "error";
    languageList.innerHTML = "";
    confidenceList.innerHTML = "";
    annotationList.innerHTML = "";
    entrypointList.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderOverview() {
  const summary = state.summary;
  const entrypoints = state.entrypoints || [];

  overviewTotals.textContent = summary
    ? `${summary.nodes} nodes · ${summary.edges} edges`
    : "0 nodes";

  const languages = Object.entries(summary?.languages || {})
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 8);
  languageList.innerHTML =
    languages.length > 0
      ? languages
          .map(
            ([language, count]) => `
              <button class="language-chip" type="button" data-language="${escapeHtml(language)}">
                <span>${escapeHtml(language)}</span>
                <strong>${count}</strong>
              </button>
            `,
          )
          .join("")
      : '<p class="empty">No languages.</p>';

  const confidences = Object.entries(summary?.edge_confidences || {})
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 5);
  confidenceList.innerHTML =
    confidences.length > 0
      ? confidences
          .map(
            ([confidence, count]) => `
              <button class="confidence-chip" type="button" data-confidence="${escapeHtml(confidence)}">
                <span>${escapeHtml(formatKind(confidence))}</span>
                <strong>${count}</strong>
              </button>
            `,
          )
          .join("")
      : '<p class="empty">No edge confidence.</p>';

  const annotations = annotationFacets(summary, state.graph.nodes);
  annotationList.innerHTML =
    annotations.length > 0
      ? annotations
          .map(
            (facet) => `
              <button class="annotation-chip" type="button" data-annotation-key="${escapeHtml(facet.key)}" data-annotation-value="${escapeHtml(facet.value)}">
                <span>${escapeHtml(annotationLabel(facet.key, facet.value))}</span>
                <strong>${facet.count}</strong>
              </button>
            `,
          )
          .join("")
      : '<p class="empty">No annotations.</p>';

  entrypointList.innerHTML =
    entrypoints.length > 0
      ? entrypoints
          .slice(0, 8)
          .map(
            (node) => `
              <button class="entrypoint-item" type="button" data-node-id="${node.id}">
                <span>${escapeHtml(formatKind(node.metadata?.entrypoint_kind || node.kind))}</span>
                <strong>${escapeHtml(node.label)}</strong>
              </button>
            `,
          )
          .join("")
      : '<p class="empty">No entrypoints.</p>';

  languageList.querySelectorAll("[data-language]").forEach((button) => {
    button.addEventListener("click", () => {
      serverKindInput.value = "";
      serverItemKindInput.value = "";
      serverLanguageInput.value = button.dataset.language || "";
      serverSearchInput.value = "";
      serverEdgeKindInput.value = "";
      serverConfidenceInput.value = "";
      searchInput.value = "";
      state.search = "";
      loadGraphPage({ resetPage: true, resetLayout: true });
    });
  });

  annotationList.querySelectorAll("[data-annotation-key]").forEach((button) => {
    button.addEventListener("click", () => {
      const key = button.dataset.annotationKey || "";
      const value = button.dataset.annotationValue || "";
      if (!key || !value) return;
      queryInput.value = `nodes metadata.${key}:${quoteQueryValue(value)}`;
      runGraphQuery();
    });
  });

  confidenceList.querySelectorAll("[data-confidence]").forEach((button) => {
    button.addEventListener("click", () => {
      serverConfidenceInput.value = button.dataset.confidence || "";
      loadGraphPage({ resetPage: true, resetLayout: true });
    });
  });

  entrypointList.querySelectorAll("[data-node-id]").forEach((button) => {
    button.addEventListener("click", () => {
      focusNodeId(Number(button.dataset.nodeId), "Focus: entrypoint");
    });
  });
}

function annotationFacets(summary, nodes) {
  const summaryFacets = summary?.annotation_facets || {};
  const fromSummary = Object.entries(summaryFacets).flatMap(([key, values]) =>
    Object.entries(values || {}).map(([value, count]) => ({
      key,
      value,
      count,
    })),
  );
  if (fromSummary.length > 0) {
    return sortAnnotationFacets(fromSummary).slice(0, 8);
  }

  const counts = new Map();
  for (const node of nodes || []) {
    for (const [key, value] of Object.entries(node.metadata || {})) {
      if (!key.startsWith("annotation.")) continue;
      const stringValue = String(value).trim();
      if (!stringValue) continue;
      const facetKey = `${key}\u0000${stringValue}`;
      counts.set(facetKey, {
        key,
        value: stringValue,
        count: (counts.get(facetKey)?.count || 0) + 1,
      });
    }
  }
  return sortAnnotationFacets([...counts.values()]).slice(0, 8);
}

function sortAnnotationFacets(facets) {
  return facets
    .sort(
      (left, right) =>
        right.count - left.count ||
        annotationLabel(left.key, left.value).localeCompare(
          annotationLabel(right.key, right.value),
        ),
    );
}

function annotationLabel(key, value) {
  return `${formatKind(key.replace(/^annotation\./, ""))}: ${value}`;
}

function shiftGraphPage(direction) {
  const nextOffset = state.graphPage.nodeOffset + direction * state.graphPage.nodeLimit;
  state.graphPage.nodeOffset = Math.max(0, nextOffset);
  loadGraphPage({ resetLayout: true });
}

function updateGraphPageControls() {
  const start = state.graphPage.totalNodes === 0 ? 0 : state.graphPage.nodeOffset + 1;
  const end = Math.min(
    state.graphPage.totalNodes,
    state.graphPage.nodeOffset + state.graphPage.nodeLimit,
  );
  pageInfo.textContent = `${start}-${end} / ${state.graphPage.totalNodes}`;
  pagePrevButton.disabled = state.graphPage.nodeOffset === 0;
  pageNextButton.disabled = !state.graphPage.truncatedNodes;
  pageReloadButton.disabled = false;
}

async function runEntryFlowTrace() {
  const depth = clampNumber(Number(entryFlowDepthInput.value || 3), 1, 32);
  entryFlowDepthInput.value = String(depth);
  state.entryFlowRequest += 1;
  const requestId = state.entryFlowRequest;
  entryFlowButton.disabled = true;
  entryFlowResult.innerHTML = '<p class="empty">Tracing entrypoints...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    depth: String(depth),
    limit: "25",
  });
  const search = entryFlowSearchInput.value.trim();
  if (search) params.set("search", search);

  try {
    const response = await fetch(`/api/entrypoint-traces?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.entryFlowRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "entrypoint trace failed");
    }
    entryFlowResult.innerHTML = renderEntryFlowReport(body);
    attachEntryFlowActions(entryFlowResult, body);
  } catch (error) {
    if (requestId !== state.entryFlowRequest) return;
    entryFlowResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.entryFlowRequest) {
      entryFlowButton.disabled = false;
    }
  }
}

function renderEntryFlowReport(report) {
  const summary = `
    <div class="query-summary">
      <span>${report.total_entrypoints} entrypoints</span>
      <span>${report.traces.length} traces</span>
      <span>depth ${report.max_depth}</span>
    </div>
  `;
  if (!report.traces.length) {
    return `${summary}<p class="empty">No matching entrypoint flows.</p>`;
  }

  const rows = report.traces
    .slice(0, 25)
    .map((trace, index) => {
      const nodes = [...trace.nodes]
        .sort((left, right) => left.depth - right.depth || left.node.label.localeCompare(right.node.label))
        .slice(0, 10)
        .map(
          ({ node, depth }) => `
            <li>
              <button class="trace-node" type="button" data-node-id="${node.id}" style="--depth:${depth}">
                <span>${escapeHtml(formatKind(node.kind))}</span>
                <strong>${escapeHtml(node.label)}</strong>
              </button>
            </li>
          `,
        )
        .join("");
      const truncated = trace.truncated ? '<p class="empty">Trace truncated by depth.</p>' : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(trace.start.label)}</h3>
          <div class="trace-summary">
            <span>${trace.nodes.length} nodes</span>
            <span>${trace.edges.length} edges</span>
            <span>${escapeHtml(formatKind(trace.start.metadata?.entrypoint_kind || trace.start.kind))}</span>
          </div>
          <div class="query-actions">
            <button type="button" data-entry-flow="${index}">Focus flow</button>
          </div>
          ${nodes ? `<ul class="trace-list">${nodes}</ul>` : '<p class="empty">No outgoing dependency edges.</p>'}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = report.truncated ? '<p class="empty">Report truncated by limit or depth.</p>' : "";
  return `${summary}${rows}${truncated}`;
}

function attachEntryFlowActions(container, report) {
  attachQueryNavigation(container);
  container.querySelectorAll("[data-entry-flow]").forEach((button) => {
    button.addEventListener("click", () => {
      const trace = report.traces[Number(button.dataset.entryFlow)];
      if (!trace) return;
      const focused = {
        query: `trace-entrypoints ${trace.start.label}`,
        nodes: trace.nodes.map(({ node }) => node),
        edges: trace.edges,
        total_nodes: trace.nodes.length,
        total_edges: trace.edges.length,
        truncated: trace.truncated,
      };
      showFocusedGraph(focused, `Entry: ${trace.start.label}`, trace.start.id);
    });
  });
}

async function loadInsights() {
  state.insightRequest += 1;
  const requestId = state.insightRequest;
  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  const severity = insightSeverityInput.value.trim();
  const kind = insightKindInput.value.trim();
  const search = insightSearchInput.value.trim();
  if (severity) params.set("severity", severity);
  if (kind) params.set("kind", kind);
  if (search) params.set("search", search);
  params.set("limit", "50");
  insightFilterButton.disabled = true;

  try {
    const response = await fetch(`/api/insights?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "insights failed");
    }
    state.insightReport = body;
    renderInsights();
  } catch (error) {
    if (requestId !== state.insightRequest) return;
    state.insightReport = null;
    renderInsights();
  } finally {
    if (requestId === state.insightRequest) {
      insightFilterButton.disabled = false;
    }
  }
}

function initializeGraph(options = {}) {
  const preserveView = Boolean(options.preserveView);
  const previousPan = { ...state.pan };
  const previousZoom = state.zoom;

  state.selectedId = null;
  state.hoveredId = null;
  state.positions.clear();
  state.velocities.clear();
  const kinds = [...new Set(state.graph.nodes.map((node) => node.kind))].sort();
  state.enabledKinds = new Set(kinds);
  renderKindFilters(kinds);
  renderLegend(kinds);
  state.layoutPaused = false;
  renderViewportControls();

  seedGraphLayout();

  state.pan = preserveView ? previousPan : { x: canvas.width / 2, y: canvas.height / 2 };
  state.zoom = preserveView ? previousZoom : 1;
  applyFilters();
  startAnimation();
}

function seedGraphLayout() {
  const radius = Math.max(180, Math.min(canvas.width, canvas.height) * 0.28);
  state.graph.nodes.forEach((node, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(1, state.graph.nodes.length);
    state.positions.set(node.id, {
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
    });
    state.velocities.set(node.id, { x: 0, y: 0 });
  });
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
    attachEdgeExplainActions(queryResult);
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

async function runPathQuery() {
  const from = pathFromInput.value.trim();
  const to = pathToInput.value.trim();
  if (!from || !to) {
    pathResult.innerHTML = '<p class="empty">Enter both path endpoints.</p>';
    return;
  }

  const depth = clampNumber(Number(pathDepthInput.value || 8), 1, 32);
  pathDepthInput.value = String(depth);
  const edgeKind = pathEdgeKindInput.value.trim();
  const expression = [
    "path",
    `from:${quoteQueryValue(from)}`,
    `to:${quoteQueryValue(to)}`,
    `depth:${depth}`,
    edgeKind ? `edge_kind:${quoteQueryValue(edgeKind)}` : "",
  ]
    .filter(Boolean)
    .join(" ");

  state.pathRequest += 1;
  const requestId = state.pathRequest;
  pathButton.disabled = true;
  pathResult.innerHTML = '<p class="empty">Finding path...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: expression,
  });

  try {
    const response = await fetch(`/api/query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.pathRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "path query failed");
    }
    pathResult.innerHTML = renderQueryResult(body, { label: "Path" });
    attachQueryNavigation(pathResult);
    attachEdgeExplainActions(pathResult);
    attachQueryFocusActions(pathResult, body);
    if (body.nodes.length > 0 || body.edges.length > 0) {
      focusQueryResult(body, pathResult, { mode: "path" });
    }
  } catch (error) {
    if (requestId !== state.pathRequest) return;
    pathResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.pathRequest) {
      pathButton.disabled = false;
    }
  }
}

async function runConfigTrace() {
  const target = configTraceTargetInput.value.trim();
  if (!target) {
    configTraceResult.innerHTML = '<p class="empty">Enter a config file or environment variable.</p>';
    return;
  }

  const depth = clampNumber(Number(configTraceDepthInput.value || 6), 1, 32);
  configTraceDepthInput.value = String(depth);
  state.configTraceRequest += 1;
  const requestId = state.configTraceRequest;
  configTraceButton.disabled = true;
  configTraceResult.innerHTML = '<p class="empty">Tracing config...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    target,
    depth: String(depth),
    limit: "50",
  });

  try {
    const response = await fetch(`/api/trace-config?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.configTraceRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "config trace failed");
    }
    configTraceResult.innerHTML = renderConfigTrace(body);
    attachConfigTraceActions(configTraceResult, body);
  } catch (error) {
    if (requestId !== state.configTraceRequest) return;
    configTraceResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.configTraceRequest) {
      configTraceButton.disabled = false;
    }
  }
}

function renderConfigTrace(result) {
  const summary = `
    <div class="query-summary">
      <span>${result.total_matches} targets</span>
      <span>${result.total_readers} readers</span>
      <span>${result.total_paths} paths</span>
      <span>depth ${result.max_depth}</span>
      <span class="query-expression">${escapeHtml(result.target)}</span>
    </div>
  `;

  if (!result.matches.length) {
    return `${summary}<p class="empty">No matching config or environment nodes.</p>`;
  }

  const rows = result.matches
    .map((match, matchIndex) => {
      const readers = match.readers
        .slice(0, 8)
        .map(
          (reader) => `
            <li>
              <button class="query-item" type="button" data-node-id="${reader.node.id}">
                <span>${escapeHtml(formatKind(reader.edge.kind))}</span>
                <strong>${escapeHtml(reader.node.label)}</strong>
              </button>
            </li>
          `,
        )
        .join("");
      const paths = match.paths
        .slice(0, 8)
        .map((path, pathIndex) => renderConfigTracePath(path, matchIndex, pathIndex))
        .join("");
      const truncated = match.truncated ? '<p class="empty">Trace truncated.</p>' : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(match.target.label)}</h3>
          <div class="trace-summary">
            <span>${match.total_readers} readers</span>
            <span>${match.total_paths} paths</span>
            <span>${escapeHtml(formatKind(match.target.kind))}</span>
          </div>
          ${readers ? `<ul class="trace-list">${readers}</ul>` : '<p class="empty">No direct readers.</p>'}
          ${paths ? `<ul class="trace-list">${paths}</ul>` : ""}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = result.truncated ? '<p class="empty">Result truncated by limit.</p>' : "";
  return `${summary}${rows}${truncated}`;
}

function renderConfigTracePath(path, matchIndex, pathIndex) {
  const labels = path.nodes.map((node) => node.label).join(" -> ");
  const kind = path.reached_entrypoint ? "entrypoint path" : "reader path";
  return `
    <li>
      <button class="trace-edge" type="button" data-config-match="${matchIndex}" data-config-path="${pathIndex}">
        <span>${escapeHtml(kind)}</span>
        <strong>${escapeHtml(labels)}</strong>
      </button>
    </li>
  `;
}

function attachConfigTraceActions(container, result) {
  attachQueryNavigation(container);
  container.querySelectorAll("[data-config-match][data-config-path]").forEach((button) => {
    button.addEventListener("click", () => {
      const match = result.matches[Number(button.dataset.configMatch)];
      const path = match?.paths?.[Number(button.dataset.configPath)];
      if (!path) return;
      const focused = {
        query: `trace-config ${result.target}`,
        nodes: path.nodes,
        edges: path.edges,
        total_nodes: path.nodes.length,
        total_edges: path.edges.length,
        truncated: false,
      };
      const selectedId = path.nodes[path.nodes.length - 1]?.id || null;
      showFocusedGraph(focused, `Config: ${match.target.label}`, selectedId);
    });
  });
}

async function runErrorTrace() {
  const target = errorTraceTargetInput.value.trim();
  if (!target) {
    errorTraceResult.innerHTML = '<p class="empty">Enter an error or exception label.</p>';
    return;
  }

  const depth = clampNumber(Number(errorTraceDepthInput.value || 6), 1, 32);
  errorTraceDepthInput.value = String(depth);
  state.errorTraceRequest += 1;
  const requestId = state.errorTraceRequest;
  errorTraceButton.disabled = true;
  errorTraceResult.innerHTML = '<p class="empty">Tracing errors...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    target,
    depth: String(depth),
    limit: "50",
  });

  try {
    const response = await fetch(`/api/trace-errors?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.errorTraceRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "error trace failed");
    }
    errorTraceResult.innerHTML = renderErrorTrace(body);
    attachErrorTraceActions(errorTraceResult, body);
  } catch (error) {
    if (requestId !== state.errorTraceRequest) return;
    errorTraceResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.errorTraceRequest) {
      errorTraceButton.disabled = false;
    }
  }
}

function renderErrorTrace(result) {
  const summary = `
    <div class="query-summary">
      <span>${result.total_matches} errors</span>
      <span>${result.total_sources} sources</span>
      <span>${result.total_paths} paths</span>
      <span>depth ${result.max_depth}</span>
      <span class="query-expression">${escapeHtml(result.target)}</span>
    </div>
  `;

  if (!result.matches.length) {
    return `${summary}<p class="empty">No matching error nodes.</p>`;
  }

  const rows = result.matches
    .map((match, matchIndex) => {
      const sources = match.sources
        .slice(0, 8)
        .map(
          (source) => `
            <li>
              <button class="query-item" type="button" data-node-id="${source.node.id}">
                <span>${escapeHtml(formatKind(source.edge.kind))}</span>
                <strong>${escapeHtml(source.node.label)}</strong>
              </button>
            </li>
          `,
        )
        .join("");
      const paths = match.paths
        .slice(0, 8)
        .map((path, pathIndex) => renderErrorTracePath(path, matchIndex, pathIndex))
        .join("");
      const truncated = match.truncated ? '<p class="empty">Trace truncated.</p>' : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(match.error.label)}</h3>
          <div class="trace-summary">
            <span>${match.total_sources} sources</span>
            <span>${match.total_paths} paths</span>
            <span>${escapeHtml(formatKind(match.error.metadata?.language || match.error.kind))}</span>
          </div>
          ${sources ? `<ul class="trace-list">${sources}</ul>` : '<p class="empty">No direct sources.</p>'}
          ${paths ? `<ul class="trace-list">${paths}</ul>` : ""}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = result.truncated ? '<p class="empty">Result truncated by limit.</p>' : "";
  return `${summary}${rows}${truncated}`;
}

function renderErrorTracePath(path, matchIndex, pathIndex) {
  const labels = path.nodes.map((node) => node.label).join(" -> ");
  const kind = path.reached_entrypoint ? "entrypoint path" : "source path";
  return `
    <li>
      <button class="trace-edge" type="button" data-error-match="${matchIndex}" data-error-path="${pathIndex}">
        <span>${escapeHtml(kind)}</span>
        <strong>${escapeHtml(labels)}</strong>
      </button>
    </li>
  `;
}

function attachErrorTraceActions(container, result) {
  attachQueryNavigation(container);
  container.querySelectorAll("[data-error-match][data-error-path]").forEach((button) => {
    button.addEventListener("click", () => {
      const match = result.matches[Number(button.dataset.errorMatch)];
      const path = match?.paths?.[Number(button.dataset.errorPath)];
      if (!path) return;
      const focused = {
        query: `trace-errors ${result.target}`,
        nodes: path.nodes,
        edges: path.edges,
        total_nodes: path.nodes.length,
        total_edges: path.edges.length,
        truncated: false,
      };
      const selectedId = path.nodes[path.nodes.length - 1]?.id || null;
      showFocusedGraph(focused, `Error: ${match.error.label}`, selectedId);
    });
  });
}

function renderQueryResult(result, options = {}) {
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
  const resultLabel = options.label ? `<span>${escapeHtml(options.label)}</span>` : "";
  const expression = result.query
    ? `<span class="query-expression">${escapeHtml(result.query)}</span>`
    : "";

  return `
    <div class="query-summary">
      ${resultLabel}
      <span>${result.total_nodes} nodes</span>
      <span>${result.total_edges} edges</span>
      ${expression}
    </div>
    <div class="query-actions">
      <button data-focus-result type="button" ${hasResults ? "" : "disabled"}>Focus result</button>
      <button data-clear-focus type="button" ${state.queryFocus ? "" : "disabled"}>Clear focus</button>
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
  const facts = renderEdgeFacts(edge);
  return `
    <li>
      <div class="edge-row">
        <button class="query-item query-edge" type="button" data-node-id="${edge.target}">
          <span>${escapeHtml(formatKind(edge.kind))}</span>
          <strong>${escapeHtml(source?.label || String(edge.source))}</strong>
          <em>${escapeHtml(target?.label || String(edge.target))}</em>
          ${facts}
        </button>
        ${renderExplainEdgeButton(edge)}
      </div>
      <div class="edge-explanation" data-edge-explanation hidden></div>
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
  const focusButton = container.querySelector("[data-focus-result]");
  const clearButton = container.querySelector("[data-clear-focus]");
  if (focusButton) {
    focusButton.addEventListener("click", () => {
      focusQueryResult(result, container);
    });
  }
  if (clearButton) {
    clearButton.addEventListener("click", () => {
      clearQueryFocus();
    });
  }
}

function attachEdgeExplainActions(container) {
  container.querySelectorAll("[data-explain-edge]").forEach((button) => {
    button.addEventListener("click", () => explainEdge(button));
  });
}

function focusQueryResult(result, container = queryResult, options = {}) {
  const nodeIds = new Set(result.nodes.map((node) => node.id));
  const edgeKeys = new Set();
  result.edges.forEach((edge) => {
    nodeIds.add(edge.source);
    nodeIds.add(edge.target);
    edgeKeys.add(edgeKey(edge));
  });

  if (nodeIds.size === 0 && edgeKeys.size === 0) return;

  state.queryFocus = {
    nodeIds,
    edgeKeys,
    mode: options.mode || (edgeKeys.size > 0 ? "query" : "nodes"),
  };
  applyFilters();
  const clearButton = container.querySelector("[data-clear-focus]");
  if (clearButton) clearButton.disabled = false;
}

function clearQueryFocus() {
  state.queryFocus = null;
  applyFilters();
  document.querySelectorAll("[data-clear-focus]").forEach((button) => {
    button.disabled = true;
  });
}

function quoteQueryValue(value) {
  if (/^[A-Za-z0-9._/@:+-]+$/.test(value)) return value;
  if (!value.includes('"')) return `"${value}"`;
  if (!value.includes("'")) return `'${value}'`;
  return `"${value.replaceAll('"', "'")}"`;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
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
    return (
      !state.queryFocus ||
      state.queryFocus.edgeKeys.size === 0 ||
      state.queryFocus.edgeKeys.has(edgeKey(edge))
    );
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
  renderViewportControls();
  renderInsights();

  if (state.selectedId && !visibleIds.has(state.selectedId)) {
    state.selectedId = null;
  }
  renderSelection();
}

function renderInsights() {
  const report = state.insightReport;
  const sourceInsights = report?.insights || buildClientInsights(state.graph);
  const insights = sourceInsights.slice(0, report ? 50 : 30);
  const total = report?.total ?? insights.length;
  const severitySummary = renderInsightSeveritySummary(report);
  const kindSummary = renderInsightKindSummary(report);

  insightCount.textContent = String(total);
  if (insights.length === 0) {
    insightList.innerHTML = report
      ? `${severitySummary}${kindSummary}<p class="empty">No matching insights.</p>`
      : '<p class="empty">No obvious issues in the visible graph.</p>';
    attachInsightKindFilters();
    return;
  }

  insightList.innerHTML =
    severitySummary +
    kindSummary +
    insights
      .map(
        (insight, index) => `
        <button class="insight ${escapeHtml(insight.severity)}" type="button" data-insight-index="${index}">
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
      const insight = insights[Number(button.dataset.insightIndex)];
      if (insight) focusInsight(insight);
    });
  });
  attachInsightKindFilters();
}

function renderInsightSeveritySummary(report) {
  if (!report?.by_severity) return "";
  const rows = ["error", "warning", "info"]
    .map((severity) => {
      const count = report.by_severity[severity] || 0;
      return `<span class="${severity}">${escapeHtml(formatKind(severity))}: ${count}</span>`;
    })
    .join("");
  return `<div class="insight-summary">${rows}</div>`;
}

function renderInsightKindSummary(report) {
  if (!report?.by_kind) return "";
  const rows = Object.entries(report.by_kind)
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 6)
    .map(
      ([kind, count]) => `
        <button class="insight-kind-chip" type="button" data-insight-kind="${escapeHtml(kind)}">
          <span>${escapeHtml(formatKind(kind))}</span>
          <strong>${count}</strong>
        </button>
      `,
    )
    .join("");
  return rows ? `<div class="insight-kind-summary">${rows}</div>` : "";
}

function attachInsightKindFilters() {
  insightList.querySelectorAll("[data-insight-kind]").forEach((button) => {
    button.addEventListener("click", () => {
      insightKindInput.value = button.dataset.insightKind || "";
      loadInsights();
    });
  });
}

function insightNodeId(insight) {
  return insightNodeIds(insight)[0] || null;
}

function insightNodeIds(insight) {
  if (Array.isArray(insight.nodes) && insight.nodes.length > 0) return insight.nodes;
  if (insight.nodeId) return [insight.nodeId];
  return [];
}

function insightEdgeIndexes(insight) {
  return Array.isArray(insight.edges) ? insight.edges : [];
}

async function focusInsight(insight) {
  const nodeIds = insightNodeIds(insight);
  const edgeIndexes = insightEdgeIndexes(insight);
  const selectedId = nodeIds[0] || null;
  if (nodeIds.length === 0 && edgeIndexes.length === 0) return;

  if (selectedId) {
    state.selectedId = selectedId;
    renderSelection();
  }

  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    edge_limit: "300",
  });
  if (nodeIds.length > 0) params.set("node_ids", nodeIds.join(","));
  if (edgeIndexes.length > 0) params.set("edge_indexes", edgeIndexes.join(","));

  try {
    const response = await fetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "focus failed");
    }
    const label = `Focus: ${formatKind(insight.kind)}`;
    showFocusedGraph(body, label, selectedId);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function focusNodeId(nodeId, label) {
  if (!nodeId) return;
  state.selectedId = nodeId;
  renderSelection();

  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_ids: String(nodeId),
    edge_limit: "300",
  });

  try {
    const response = await fetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "focus failed");
    }
    showFocusedGraph(body, label, nodeId);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function showFocusedGraph(result, label, selectedId = null) {
  state.graph = { nodes: result.nodes, edges: result.edges };
  state.graphPage.nodeOffset = 0;
  state.graphPage.totalNodes = result.total_nodes;
  state.graphPage.totalEdges = result.total_edges;
  state.graphPage.truncatedNodes = false;
  state.queryFocus = null;
  queryResult.innerHTML = renderQueryResult(result);
  attachQueryNavigation(queryResult);
  attachEdgeExplainActions(queryResult);
  attachQueryFocusActions(queryResult, result);
  rootLabel.textContent = label;
  initializeGraph({ preserveView: false });
  pageInfo.textContent = `focus ${result.nodes.length} / ${result.total_nodes}`;
  pagePrevButton.disabled = true;
  pageNextButton.disabled = true;
  pageReloadButton.disabled = false;
  if (selectedId) {
    state.selectedId = selectedId;
    renderSelection();
  }
}

function buildClientInsights(graph) {
  const insights = [];
  const entrypointIds = new Set(
    graph.edges.filter((edge) => edge.kind === "entrypoint").map((edge) => edge.target),
  );
  const calledIds = new Set(
    graph.edges
      .filter(
        (edge) =>
          edge.kind === "calls" ||
          (edge.kind === "references" && edge.metadata?.relation === "entrypoint_function"),
      )
      .map((edge) => edge.target),
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
    if (!state.layoutPaused) {
      simulateLayout();
    }
    draw();
    state.animationFrame = requestAnimationFrame(tick);
  };
  tick();
}

function renderViewportControls() {
  viewportInfo.textContent = `${state.visibleNodes.length} nodes / ${state.visibleEdges.length} edges`;
  toggleLayoutButton.textContent = state.layoutPaused ? "Run" : "Pause";
  toggleLayoutButton.setAttribute(
    "aria-label",
    state.layoutPaused ? "Resume graph layout" : "Pause graph layout",
  );
  fitGraphButton.disabled = state.visibleNodes.length === 0;
  resetLayoutButton.disabled = state.graph.nodes.length === 0;
  zoomInButton.disabled = state.graph.nodes.length === 0;
  zoomOutButton.disabled = state.graph.nodes.length === 0;
  toggleLayoutButton.disabled = state.graph.nodes.length === 0;
}

function zoomAtCanvasCenter(scale) {
  zoomAt(canvas.width / 2, canvas.height / 2, scale);
}

function zoomAt(screenX, screenY, scale) {
  const before = screenToWorld(screenX, screenY);
  state.zoom = Math.max(0.18, Math.min(3.5, state.zoom * scale));
  const after = screenToWorld(screenX, screenY);
  state.pan.x += (after.x - before.x) * state.zoom;
  state.pan.y += (after.y - before.y) * state.zoom;
  draw();
}

function fitVisibleGraph() {
  if (state.visibleNodes.length === 0) return;

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    if (!position) return;
    const radius = nodeRadius(node) + 24;
    minX = Math.min(minX, position.x - radius);
    minY = Math.min(minY, position.y - radius);
    maxX = Math.max(maxX, position.x + radius);
    maxY = Math.max(maxY, position.y + radius);
  });

  if (!Number.isFinite(minX) || !Number.isFinite(minY)) return;

  const width = Math.max(1, maxX - minX);
  const height = Math.max(1, maxY - minY);
  const padding = 72;
  const zoomX = (canvas.width - padding * 2) / width;
  const zoomY = (canvas.height - padding * 2) / height;
  state.zoom = Math.max(0.18, Math.min(3.5, Math.min(zoomX, zoomY)));
  state.pan = {
    x: canvas.width / 2 - ((minX + maxX) / 2) * state.zoom,
    y: canvas.height / 2 - ((minY + maxY) / 2) * state.zoom,
  };
  draw();
}

function resetGraphLayout() {
  if (state.graph.nodes.length === 0) return;
  state.positions.clear();
  state.velocities.clear();
  seedGraphLayout();
  state.pan = { x: canvas.width / 2, y: canvas.height / 2 };
  state.zoom = 1;
  state.layoutPaused = false;
  renderViewportControls();
  draw();
}

function toggleLayout() {
  if (state.graph.nodes.length === 0) return;
  state.layoutPaused = !state.layoutPaused;
  renderViewportControls();
  draw();
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
  const focusedEdges = [];
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    if (edgeIsFocused(edge)) {
      focusedEdges.push(edge);
      return;
    }
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    drawEdge(edge, source, target, false);
  });

  focusedEdges.forEach((edge) => {
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    drawEdge(edge, source, target, true);
  });

  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    const selected = node.id === state.selectedId;
    const hovered = node.id === state.hoveredId;
    const focused = nodeIsFocused(node);
    const radius = nodeRadius(node);

    ctx.beginPath();
    ctx.arc(
      position.x,
      position.y,
      radius + (selected ? 6 : focused ? 5 : hovered ? 3 : 0),
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = selected
      ? "rgba(92, 200, 167, 0.26)"
      : focused
        ? "rgba(237, 241, 242, 0.16)"
        : hovered
          ? "rgba(255,255,255,0.12)"
          : "rgba(0,0,0,0.22)";
    ctx.fill();

    ctx.beginPath();
    ctx.arc(position.x, position.y, radius, 0, Math.PI * 2);
    ctx.fillStyle = colorFor(node.kind);
    ctx.fill();
    ctx.lineWidth = selected ? 2.6 / state.zoom : focused ? 2.2 / state.zoom : 1 / state.zoom;
    ctx.strokeStyle = selected ? "#ffffff" : focused ? "rgba(237, 241, 242, 0.92)" : "rgba(255,255,255,0.55)";
    ctx.stroke();

    if (state.zoom > 0.45 || selected || hovered || focused) {
      drawLabel(node, position, radius);
    }
  });

  ctx.restore();
}

function drawEdge(edge, source, target, focused) {
  if (!source || !target) return;
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const distance = Math.max(1, Math.sqrt(dx * dx + dy * dy));
  const ux = dx / distance;
  const uy = dy / distance;
  const sourceRadius = nodeRadiusById(edge.source) + 2 / state.zoom;
  const targetRadius = nodeRadiusById(edge.target) + (focused ? 8 : 3) / state.zoom;
  const start = {
    x: source.x + ux * Math.min(sourceRadius, distance * 0.35),
    y: source.y + uy * Math.min(sourceRadius, distance * 0.35),
  };
  const end = {
    x: target.x - ux * Math.min(targetRadius, distance * 0.35),
    y: target.y - uy * Math.min(targetRadius, distance * 0.35),
  };

  if (focused) {
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    ctx.lineTo(end.x, end.y);
    ctx.lineWidth = 6 / state.zoom;
    ctx.strokeStyle = "rgba(13, 15, 16, 0.72)";
    ctx.stroke();
  }

  ctx.beginPath();
  ctx.moveTo(start.x, start.y);
  ctx.lineTo(end.x, end.y);
  ctx.lineWidth = (focused ? 3.2 : 1) / state.zoom;
  ctx.strokeStyle = focused ? focusEdgeColor() : edgeColor(edge);
  ctx.stroke();

  if (focused) {
    drawArrowHead(start, end, focusEdgeColor());
  }
}

function drawArrowHead(start, end, color) {
  const angle = Math.atan2(end.y - start.y, end.x - start.x);
  const length = 11 / state.zoom;
  const spread = Math.PI / 7;
  ctx.beginPath();
  ctx.moveTo(end.x, end.y);
  ctx.lineTo(end.x - Math.cos(angle - spread) * length, end.y - Math.sin(angle - spread) * length);
  ctx.lineTo(end.x - Math.cos(angle + spread) * length, end.y - Math.sin(angle + spread) * length);
  ctx.closePath();
  ctx.fillStyle = color;
  ctx.fill();
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
    if (state.selectedId) {
      selectionTitle.textContent = `Node ${state.selectedId}`;
      selectionBody.innerHTML = '<p class="empty">Loading node context...</p>';
      loadNodeContext(state.selectedId, requestId);
    } else {
      selectionTitle.textContent = "Selection";
      selectionBody.innerHTML = '<p class="empty">No node selected.</p>';
    }
    return;
  }

  renderSelectionPanel(node, [], new Map([[node.id, node]]), requestId, true);
  loadNodeContext(node.id, requestId);
}

async function loadNodeContext(nodeId, requestId) {
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(nodeId),
    edge_limit: "80",
  });

  try {
    const response = await fetch(`/api/node-context?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest || state.selectedId !== nodeId) return;
    if (!response.ok) {
      throw new Error(body.error || "node context failed");
    }
    const nodeMap = new Map(body.nodes.map((node) => [node.id, node]));
    nodeMap.set(body.node.id, body.node);
    renderSelectionPanel(body.node, body.edges, nodeMap, requestId, false, body);
  } catch (error) {
    if (requestId !== state.selectionRequest || state.selectedId !== nodeId) return;
    const node = state.graph.nodes.find((candidate) => candidate.id === nodeId);
    if (node) {
      renderSelectionPanel(node, [], new Map([[node.id, node]]), requestId, false);
      const container = selectionBody.querySelector(".neighbors");
      if (container) {
        container.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
      }
    } else {
      selectionTitle.textContent = "Error";
      selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    }
  }
}

function renderSelectionPanel(node, edges, nodeMap, requestId, loading = false, context = null) {
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

  const neighborRows = loading
    ? '<p class="empty">Loading node context...</p>'
    : edges.length > 0
      ? edges.map((edge) => renderNeighbor(edge, node.id, nodeMap)).join("")
      : '<p class="empty">No neighboring edges.</p>';
  const contextSummary = context
    ? `<p class="neighbor-summary">${context.total_edges} edges${context.truncated_edges ? `, first ${context.edge_limit}` : ""}</p>`
    : "";

  selectionBody.innerHTML = `
    <div class="selection-actions">
      <button type="button" data-path-endpoint="from">From</button>
      <button type="button" data-path-endpoint="to">To</button>
      ${
        node.kind === "config" || node.kind === "environment"
          ? '<button type="button" data-config-trace-target>Config Trace</button>'
          : ""
      }
      ${
        node.metadata?.item_kind === "error"
          ? '<button type="button" data-error-trace-target>Error Trace</button>'
          : ""
      }
    </div>
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
      ${contextSummary}
      ${neighborRows}
    </div>
    <section class="trace-panel">
      <div class="trace-controls">
        <label class="field compact">
          <span>Depth</span>
          <input id="traceDepthInput" type="number" min="1" max="8" value="3" />
        </label>
        <button id="traceButton" type="button">Trace</button>
        <button id="dependentsButton" type="button">Dependents</button>
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

  selectionBody.querySelectorAll("[data-path-endpoint]").forEach((button) => {
    button.addEventListener("click", () => {
      const target = button.dataset.pathEndpoint === "to" ? pathToInput : pathFromInput;
      target.value = String(node.id);
      target.focus();
    });
  });

  const configTraceTarget = selectionBody.querySelector("[data-config-trace-target]");
  if (configTraceTarget) {
    configTraceTarget.addEventListener("click", () => {
      configTraceTargetInput.value = node.label;
      runConfigTrace();
    });
  }

  const errorTraceTarget = selectionBody.querySelector("[data-error-trace-target]");
  if (errorTraceTarget) {
    errorTraceTarget.addEventListener("click", () => {
      errorTraceTargetInput.value = node.label;
      runErrorTrace();
    });
  }

  const traceButton = document.querySelector("#traceButton");
  if (traceButton) {
    traceButton.addEventListener("click", () => loadTrace(node));
  }
  const dependentsButton = document.querySelector("#dependentsButton");
  if (dependentsButton) {
    dependentsButton.addEventListener("click", () => loadDependents(node));
  }

  if (node.span) {
    loadSourcePreview(node, requestId);
  }

  attachEdgeExplainActions(selectionBody);
}

async function loadTrace(node) {
  state.traceRequest += 1;
  state.dependentsRequest += 1;
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
    attachEdgeExplainActions(target);
  } catch (error) {
    if (requestId !== state.traceRequest) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function loadDependents(node) {
  state.traceRequest += 1;
  state.dependentsRequest += 1;
  const requestId = state.dependentsRequest;
  const target = document.querySelector("#traceResult");
  if (!target) return;

  target.innerHTML = '<p class="empty">Tracing dependents...</p>';
  const depthInput = document.querySelector("#traceDepthInput");
  const depth = clampNumber(Number(depthInput?.value || 3), 1, 16);
  if (depthInput) depthInput.value = String(depth);
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: String(depth),
  });

  try {
    const response = await fetch(`/api/dependents?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.dependentsRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "dependents trace failed");
    }
    target.innerHTML = renderTrace(body, { empty: "No incoming dependents.", label: "Dependents" });
    attachTraceNavigation(target);
    attachEdgeExplainActions(target);
  } catch (error) {
    if (requestId !== state.dependentsRequest) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderTrace(trace, options = {}) {
  if (!trace) {
    return '<p class="empty">No matching start node.</p>';
  }
  if (trace.nodes.length <= 1 && trace.edges.length === 0) {
    return `<p class="empty">${escapeHtml(options.empty || "No outgoing dependency edges.")}</p>`;
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
      ${options.label ? `<span>${escapeHtml(options.label)}</span>` : ""}
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
  const facts = renderEdgeFacts(edge);
  return `
    <li>
      <div class="edge-row">
        <button class="trace-edge" type="button" data-node-id="${edge.target}">
          <span>${escapeHtml(formatKind(edge.kind))}</span>
          <strong>${escapeHtml(source?.label || String(edge.source))}</strong>
          <em>${escapeHtml(target?.label || String(edge.target))}</em>
          ${facts}
        </button>
        ${renderExplainEdgeButton(edge)}
      </div>
      <div class="edge-explanation" data-edge-explanation hidden></div>
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
    root: pathInput.value.trim() || ".",
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

function renderNeighbor(edge, selectedId, nodeMap = null) {
  const otherId = edge.source === selectedId ? edge.target : edge.source;
  const other = nodeMap?.get(otherId) || state.graph.nodes.find((node) => node.id === otherId);
  const direction = edge.source === selectedId ? "out" : "in";
  const facts = renderEdgeFacts(edge);
  return `
    <div>
      <div class="edge-row">
        <button type="button" class="neighbor" data-node-id="${otherId}">
          <span>${escapeHtml(direction)} ${escapeHtml(formatKind(edge.kind))}</span>
          <span>${escapeHtml(other ? other.label : String(otherId))}</span>
          ${facts}
        </button>
        ${renderExplainEdgeButton(edge)}
      </div>
      <div class="edge-explanation" data-edge-explanation hidden></div>
    </div>
  `;
}

function renderEdgeFacts(edge) {
  const facts = edgeFacts(edge);
  if (facts.length === 0) return "";
  return `<span class="edge-facts">${facts.map((fact) => escapeHtml(fact)).join(" · ")}</span>`;
}

function renderExplainEdgeButton(edge) {
  return `
    <button
      class="edge-explain-button"
      type="button"
      data-explain-edge
      data-edge-source="n${edge.source}"
      data-edge-target="n${edge.target}"
      data-edge-kind="${escapeHtml(edge.kind)}"
    >Explain</button>
  `;
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

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    source: button.dataset.edgeSource || "",
    target: button.dataset.edgeTarget || "",
    kind: button.dataset.edgeKind || "",
  });

  try {
    const response = await fetch(`/api/explain-edge?${params.toString()}`);
    const body = await response.json();
    if (button.dataset.explainToken !== requestId) return;
    if (!response.ok) {
      throw new Error(body.error || "edge explanation failed");
    }
    target.innerHTML = renderEdgeExplanation(body);
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
  `;
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
  const delta = event.deltaY > 0 ? 0.9 : 1.1;
  zoomAt(event.offsetX, event.offsetY, delta);
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

function nodeIsFocused(node) {
  return Boolean(state.queryFocus?.nodeIds?.has(node.id));
}

function edgeIsFocused(edge) {
  return Boolean(state.queryFocus?.edgeKeys?.has(edgeKey(edge)));
}

function focusEdgeColor() {
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
