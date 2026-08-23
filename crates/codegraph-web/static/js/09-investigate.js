// 09-investigate.js — facets, workflow filters, entry flows, insights and quality gate panels.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

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
  state.graphPage.edgeOffset = 0;
  loadGraphPage({ resetLayout: true });
}

function shiftEdgePage(direction) {
  const nextOffset = state.graphPage.edgeOffset + direction * state.graphPage.edgeLimit;
  state.graphPage.edgeOffset = Math.max(0, nextOffset);
  loadGraphPage({ resetLayout: true });
}

function clearGraphPageFilters() {
  state.architecturePathPrefix = "";
  state.graphPage.nodeOffset = 0;
  state.graphPage.edgeOffset = 0;
  serverKindInput.value = "";
  serverItemKindInput.value = "";
  serverLanguageInput.value = "";
  serverSearchInput.value = "";
  serverEdgeKindInput.value = "";
  serverConfidenceInput.value = "";
  serverEdgeRelationInput.value = "";
  serverEdgeSourceInput.value = "";
  loadGraphPage({ resetLayout: true });
}

function updateGraphPageControls() {
  const start = state.graphPage.totalNodes === 0 ? 0 : state.graphPage.nodeOffset + 1;
  const end = Math.min(
    state.graphPage.totalNodes,
    state.graphPage.nodeOffset + state.graphPage.nodeLimit,
  );
  const edgeStart = state.graphPage.totalEdges === 0 ? 0 : state.graphPage.edgeOffset + 1;
  const edgeEnd = Math.min(
    state.graphPage.totalEdges,
    state.graphPage.edgeOffset + state.graphPage.edgeLimit,
  );
  pageInfo.textContent = `${start}-${end} / ${state.graphPage.totalNodes}`;
  edgePageInfo.textContent = `${edgeStart}-${edgeEnd} / ${state.graphPage.totalEdges} ${t("label.edges").toLowerCase()}`;
  pagePrevButton.disabled = state.graphPage.nodeOffset === 0;
  pageNextButton.disabled = !state.graphPage.truncatedNodes;
  edgePrevButton.disabled = state.graphPage.edgeOffset === 0;
  edgeNextButton.disabled = !state.graphPage.truncatedEdges;
  pageReloadButton.disabled = false;
  pageCopyButton.disabled = state.graph.nodes.length === 0 && state.graph.edges.length === 0;
  pageClearButton.disabled = !graphPageHasFilters();
  renderGraphPageScope();
}

function graphPageHasFilters() {
  return Boolean(
    state.architecturePathPrefix ||
      serverKindInput.value.trim() ||
      serverItemKindInput.value.trim() ||
      serverLanguageInput.value.trim() ||
      serverSearchInput.value.trim() ||
      serverEdgeKindInput.value.trim() ||
      serverConfidenceInput.value.trim() ||
      serverEdgeRelationInput.value.trim() ||
      serverEdgeSourceInput.value.trim() ||
      state.graphPage.nodeOffset > 0 ||
      state.graphPage.edgeOffset > 0,
  );
}

function renderGraphPageScope(options = {}) {
  if (!state.graphPage.root && state.graph.nodes.length === 0 && state.graph.edges.length === 0) {
    pageScope.innerHTML = "";
    return;
  }
  const focused = options.focused || Boolean(state.queryFocus);
  const nodes = formatNumber(state.graph.nodes.length);
  const edges = formatNumber(state.graph.edges.length);
  const totalNodes = formatNumber(state.graphPage.totalNodes);
  const totalEdges = formatNumber(state.graphPage.totalEdges);
  const warnings = [];

  if (focused) {
    pageScope.innerHTML = `<span>${escapeHtml(t("graph.scopeFocused", { nodes, edges }))}</span>`;
    return;
  }

  const message =
    state.graphPage.truncatedNodes || state.graphPage.truncatedEdges
      ? t("graph.scopeLoaded", { nodes, totalNodes, edges, totalEdges })
      : t("graph.scopeComplete", { nodes, edges });
  if (state.graphPage.truncatedNodes) warnings.push(t("graph.scopeNodesTruncated"));
  if (state.graphPage.truncatedEdges) warnings.push(t("graph.scopeEdgesTruncated"));

  pageScope.innerHTML = `
    <span>${escapeHtml(message)}</span>
    ${warnings.map((warning) => `<strong>${escapeHtml(warning)}</strong>`).join("")}
  `;
}

const WORKFLOW_FILTER_FIELDS = [
  ["edge_kind", "EdgeKind"],
  ["confidence", "Confidence"],
  ["language", "Language"],
  ["risk_severity", "RiskSeverity"],
  ["block_kind", "BlockKind"],
];

function workflowFilterInputs(prefix) {
  return WORKFLOW_FILTER_FIELDS.map(([, suffix]) => document.querySelector(`#${prefix}${suffix}Input`)).filter(Boolean);
}

function readWorkflowFilters(prefix) {
  return Object.fromEntries(
    WORKFLOW_FILTER_FIELDS.map(([key, suffix]) => [
      key,
      document.querySelector(`#${prefix}${suffix}Input`)?.value.trim() || "",
    ]).filter(([, value]) => value),
  );
}

function appendWorkflowFilterParams(params, filters) {
  Object.entries(filters || {}).forEach(([key, value]) => {
    if (value) params.set(key, value);
  });
}

function renderWorkflowFilterSummary(filters) {
  const entries = Object.entries(filters || {}).filter(([, value]) => value);
  if (!entries.length) return "";
  return entries
    .map(([key, value]) => `<span>${escapeHtml(`${formatKind(key)}: ${value}`)}</span>`)
    .join("");
}

async function runEntryFlowTrace() {
  const depth = clampNumber(Number(entryFlowDepthInput.value || 3), 1, 32);
  entryFlowDepthInput.value = String(depth);
  state.entryFlowRequest += 1;
  const requestId = state.entryFlowRequest;
  entryFlowButton.disabled = true;
  entryFlowWorkflowButton.disabled = true;
  entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("entryFlows.tracing"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    depth: String(depth),
    limit: "25",
  });
  const search = entryFlowSearchInput.value.trim();
  if (search) params.set("search", search);

  try {
    const response = await apiFetch(`/api/entrypoint-traces?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.entryFlowRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("entryFlows.failedFallback")));
    }
    state.lastEntryFlowReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        search,
        depth,
        limit: 25,
      },
      report: body,
    };
    state.lastEntryWorkflowReport = null;
    renderEntryFlowExportState();
    renderEntryWorkflowExportState();
    entryFlowResult.innerHTML = renderEntryFlowReport(body);
    attachEntryFlowActions(entryFlowResult, body);
  } catch (error) {
    if (requestId !== state.entryFlowRequest) return;
    entryFlowResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.entryFlowRequest) {
      entryFlowButton.disabled = false;
      entryFlowWorkflowButton.disabled = false;
    }
  }
}

async function runEntryFlowWorkflows() {
  const depth = clampNumber(Number(entryFlowDepthInput.value || 3), 1, 32);
  entryFlowDepthInput.value = String(depth);
  state.entryFlowRequest += 1;
  const requestId = state.entryFlowRequest;
  entryFlowButton.disabled = true;
  entryFlowWorkflowButton.disabled = true;
  entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("entryFlows.buildingWorkflows"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    depth: String(depth),
    block_limit: "120",
    limit: "15",
    compact: "true",
    max_fanout: "8",
  });
  const search = entryFlowSearchInput.value.trim();
  if (search) params.set("search", search);
  const entrypointKind = entryWorkflowEntrypointKindInput.value.trim();
  if (entrypointKind) params.set("entrypoint_kind", entrypointKind);
  const workflowFilters = readWorkflowFilters("entryWorkflow");
  appendWorkflowFilterParams(params, workflowFilters);

  try {
    const response = await apiFetch(`/api/entrypoint-workflows?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.entryFlowRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("entryFlows.workflowFailedFallback")));
    }
    state.lastEntryWorkflowReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        search,
        entrypoint_kind: entrypointKind,
        depth,
        block_limit: 120,
        limit: 15,
        compact: true,
        ...workflowFilters,
      },
      report: body,
    };
    state.lastEntryFlowReport = null;
    renderEntryFlowExportState();
    renderEntryWorkflowExportState();
    entryFlowResult.innerHTML = renderEntryWorkflowReport(body);
    attachEntryWorkflowActions(entryFlowResult, body);
  } catch (error) {
    if (requestId !== state.entryFlowRequest) return;
    entryFlowResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.entryFlowRequest) {
      entryFlowButton.disabled = false;
      entryFlowWorkflowButton.disabled = false;
    }
  }
}

function renderEntryFlowExportState() {
  entryFlowExportButton.disabled = !state.lastEntryFlowReport;
}

function renderEntryWorkflowExportState() {
  entryFlowWorkflowExportButton.disabled = !state.lastEntryWorkflowReport;
  entryFlowWorkflowMermaidExportButton.disabled = !state.lastEntryWorkflowReport;
  entryFlowWorkflowDotExportButton.disabled = !state.lastEntryWorkflowReport;
}

function clearLastEntryFlowReport() {
  state.lastEntryFlowReport = null;
  renderEntryFlowExportState();
}

function clearLastEntryWorkflowReport() {
  state.lastEntryWorkflowReport = null;
  renderEntryWorkflowExportState();
}

function exportLastEntryFlowReport() {
  if (!state.lastEntryFlowReport) {
    entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noEntryFlows"))}</p>`;
    renderEntryFlowExportState();
    return;
  }

  const payload = {
    schema: "codegraph.entrypoint_traces.v1",
    ...state.lastEntryFlowReport,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-entrypoint-traces.json`;
  downloadBlob(blob, fileName);
  entryFlowResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.entryFlows"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(t("entryFlows.traceCount", { count: formatNumber(payload.report?.traces?.length ?? 0) }))}</span>
        <span>${escapeHtml(t("entryFlows.entrypointCount", { count: formatNumber(payload.report?.total_entrypoints ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function exportLastEntryWorkflowReport(format) {
  if (!state.lastEntryWorkflowReport) {
    entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noEntryWorkflows"))}</p>`;
    renderEntryWorkflowExportState();
    return;
  }

  const payload = {
    schema: "codegraph.entrypoint_workflows.v1",
    ...state.lastEntryWorkflowReport,
  };
  const root = safeFilePart(payload.root);
  if (format === "mermaid") {
    const mermaid = entryWorkflowReportToMermaid(payload.report);
    const blob = new Blob([mermaid], { type: "text/vnd.mermaid;charset=utf-8" });
    const fileName = `codegraph-${root}-entrypoint-workflows.mmd`;
    downloadBlob(blob, fileName);
    renderEntryFlowExportNote(fileName, blob.size, t("export.entryWorkflowMermaid"));
    return;
  }
  if (format === "dot") {
    const dot = entryWorkflowReportToDot(payload.report);
    const blob = new Blob([dot], { type: "text/vnd.graphviz;charset=utf-8" });
    const fileName = `codegraph-${root}-entrypoint-workflows.dot`;
    downloadBlob(blob, fileName);
    renderEntryFlowExportNote(fileName, blob.size, t("export.entryWorkflowDot"));
    return;
  }

  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${root}-entrypoint-workflows.json`;
  downloadBlob(blob, fileName);
  renderEntryFlowExportNote(fileName, blob.size, t("export.entryWorkflows"));
}

function renderEntryFlowExportNote(fileName, size, label) {
  entryFlowResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(label)}</span>
        <span>${escapeHtml(formatBytes(size))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function renderEntryFlowReport(report) {
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("entryFlows.entrypointCount", { count: formatNumber(report.total_entrypoints || 0) }))}</span>
      <span>${escapeHtml(t("entryFlows.traceCount", { count: formatNumber(report.traces.length) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: formatNumber(report.max_depth || 0) }))}</span>
    </div>
  `;
  if (!report.traces.length) {
    return `${summary}<p class="empty">${escapeHtml(t("entryFlows.noMatches"))}</p>`;
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
      const truncated = trace.truncated
        ? `<p class="empty">${escapeHtml(t("entryFlows.traceTruncated"))}</p>`
        : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(trace.start.label)}</h3>
          <div class="trace-summary">
            <span>${escapeHtml(t("stat.nodes"))}: ${escapeHtml(formatNumber(trace.nodes.length))}</span>
            <span>${escapeHtml(t("stat.edges"))}: ${escapeHtml(formatNumber(trace.edges.length))}</span>
            <span>${escapeHtml(formatKind(trace.start.metadata?.entrypoint_kind || trace.start.kind))}</span>
          </div>
          <div class="query-actions">
            <button type="button" data-entry-flow="${index}">${escapeHtml(t("entryFlows.focusFlow"))}</button>
          </div>
          ${nodes ? `<ul class="trace-list">${nodes}</ul>` : `<p class="empty">${escapeHtml(t("trace.noOutgoing"))}</p>`}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = report.truncated
    ? `<p class="empty">${escapeHtml(t("entryFlows.reportTruncated"))}</p>`
    : "";
  return `${summary}${rows}${truncated}`;
}

function renderEntryWorkflowReport(report) {
  const workflows = Array.isArray(report.workflows) ? report.workflows : [];
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("entryFlows.entrypointCount", { count: formatNumber(report.total_entrypoints || 0) }))}</span>
      <span>${escapeHtml(t("entryFlows.workflowCount", { count: formatNumber(workflows.length) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: formatNumber(report.max_depth || 0) }))}</span>
      ${renderWorkflowFilterSummary(report.filters)}
    </div>
  `;
  if (!workflows.length) {
    return `${summary}<p class="empty">${escapeHtml(t("entryFlows.noWorkflowMatches"))}</p>`;
  }

  const rows = workflows
    .slice(0, 15)
    .map((workflow, index) => {
      const start = workflow.start || {};
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(start.label || String(start.id || ""))}</h3>
          <div class="query-actions">
            <button type="button" data-entry-workflow="${index}">${escapeHtml(t("entryFlows.focusWorkflow"))}</button>
          </div>
          ${renderWorkflow(workflow)}
        </section>
      `;
    })
    .join("");
  const truncated = report.truncated
    ? `<p class="empty">${escapeHtml(t("entryFlows.reportTruncated"))}</p>`
    : "";
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
      showFocusedGraph(focused, t("entryFlows.focusTitle", { label: trace.start.label }), trace.start.id);
    });
  });
}

function attachEntryWorkflowActions(container, report) {
  attachWorkflowNavigation(container);
  attachEdgeExplainActions(container);
  attachFlowViewActions(
    container,
    (index) => report.workflows?.[index],
    (workflow) => workflow.start?.label || "",
  );
  container.querySelectorAll("[data-entry-workflow]").forEach((button) => {
    button.addEventListener("click", () => {
      const workflow = report.workflows?.[Number(button.dataset.entryWorkflow)];
      if (!workflow) return;
      const blocks = Array.isArray(workflow.blocks) ? workflow.blocks : [];
      const transitions = Array.isArray(workflow.transitions) ? workflow.transitions : [];
      const focused = {
        query: `workflow-entrypoints ${workflow.start?.label || ""}`,
        nodes: blocks.map((block) => block.node).filter(Boolean),
        edges: transitions.map((transition) => transition.edge).filter(Boolean),
        total_nodes: blocks.length,
        total_edges: transitions.length,
        truncated: workflow.truncated,
      };
      showFocusedGraph(focused, t("entryFlows.focusTitle", { label: workflow.start?.label || "" }), workflow.start?.id);
    });
  });
}

async function runJourney() {
  const from = journeyFromInput.value.trim();
  const to = journeyToInput.value.trim();
  if (!from || !to) {
    journeyResult.innerHTML = `<p class="empty">${escapeHtml(t("journey.needEndpoints"))}</p>`;
    return;
  }
  const depth = clampNumber(Number(journeyDepthInput.value || 8), 1, 32);
  const paths = clampNumber(Number(journeyPathsInput.value || 3), 1, 10);
  journeyDepthInput.value = String(depth);
  journeyPathsInput.value = String(paths);
  state.journeyRequest += 1;
  const requestId = state.journeyRequest;
  journeyRunButton.disabled = true;
  journeyResult.innerHTML = `<p class="empty">${escapeHtml(t("journey.building"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    from,
    to,
    depth: String(depth),
    paths: String(paths),
  });

  try {
    const response = await apiFetch(`/api/journey?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.journeyRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("journey.failedFallback")));
    }
    state.lastJourneyReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      request: { from, to, depth, paths },
      report: body,
    };
    renderJourneyExportState();
    journeyResult.innerHTML = renderJourneyReport(body);
    attachJourneyActions(journeyResult, body);
  } catch (error) {
    if (requestId !== state.journeyRequest) return;
    state.lastJourneyReport = null;
    renderJourneyExportState();
    journeyResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.journeyRequest) {
      journeyRunButton.disabled = false;
    }
  }
}

function renderJourneyReport(report) {
  const paths = Array.isArray(report.paths) ? report.paths : [];
  // Which definition each end is about, when the name named more than one.
  const notes = Array.isArray(report.notes)
    ? report.notes.map((note) => `<p class="query-note">${escapeHtml(String(note))}</p>`).join("")
    : "";
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(`${report.from?.label || ""} → ${report.to?.label || ""}`)}</span>
      <span>${escapeHtml(t("journey.pathCount", { count: formatNumber(report.total_paths || 0) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: formatNumber(report.max_depth || 0) }))}</span>
    </div>
    ${notes}
  `;
  if (!paths.length) {
    const key = report.truncated ? "journey.truncatedNoPath" : "journey.noPaths";
    return `${summary}<p class="empty">${escapeHtml(t(key))}</p>`;
  }

  const sections = paths
    .map((path, index) => {
      const risk = path.risk_summary || {};
      const riskChips = [
        risk.fragile_transitions
          ? `<span class="workflow-risk warning">${escapeHtml(t("journey.fragileCount", { count: risk.fragile_transitions }))}</span>`
          : "",
        risk.low_confidence_hops
          ? `<span class="workflow-risk info">${escapeHtml(t("journey.lowConfidenceCount", { count: risk.low_confidence_hops }))}</span>`
          : "",
        risk.risky_steps
          ? `<span class="workflow-risk warning">${escapeHtml(t("journey.riskyStepCount", { count: risk.risky_steps }))}</span>`
          : "",
        risk.cycle_back_edges
          ? `<span class="workflow-risk info">${escapeHtml(t("journey.cycleCount", { count: risk.cycle_back_edges }))}</span>`
          : "",
      ]
        .filter(Boolean)
        .join("");
      const steps = (path.steps || [])
        .map((step) => {
          const node = step.block?.node || {};
          const fragile = step.fragile
            ? `<span class="workflow-risk warning">${escapeHtml(
                t("journey.fragile", { reasons: (step.fragile_reasons || []).map((reason) => formatKind(reason)).join(", ") }),
              )}</span>`
            : "";
          const edge = step.transition?.edge;
          const explanation = step.explanation?.summary
            ? `<em class="journey-hop-note">${escapeHtml(step.explanation.summary)}</em>`
            : "";
          const edgeRow = edge
            ? `<div class="edge-row">
                <button class="trace-edge" type="button" data-node-id="${node.id ?? ""}">
                  <span>${escapeHtml(formatKind(edge.kind || "unknown"))}</span>
                  ${renderEdgeFacts(edge)}
                </button>
                ${renderEdgeActions(edge)}
              </div>
              <div class="edge-explanation" data-edge-explanation hidden></div>`
            : "";
          const expand =
            node.id != null
              ? `<button type="button" class="journey-expand" data-journey-expand data-node-id-expand="${node.id}" data-step="${step.step}" data-label="${escapeHtml(node.label || String(node.id))}">${escapeHtml(t("journey.expandStep"))}</button>`
              : "";
          return `
            <li class="workflow-block ${escapeHtml(step.block?.kind || "unknown")}">
              <button type="button" data-node-id="${node.id ?? ""}">
                <span>${escapeHtml(`${step.step}. ${formatKind(step.block?.kind || "unknown")}`)}</span>
                <strong>${escapeHtml(node.label || String(node.id ?? ""))}</strong>
                ${fragile}
              </button>
              ${explanation}
              ${edgeRow}
              ${expand}
              <div class="journey-subflow" data-journey-subflow hidden></div>
            </li>
          `;
        })
        .join("");
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(t("journey.pathTitle", { rank: path.rank, steps: formatNumber(path.total_steps || 0) }))}</h3>
          <div class="query-summary">
            <span>${escapeHtml(t("journey.score", { score: formatNumber(path.confidence_score || 0) }))}</span>
            ${path.lowest_confidence ? `<span>${escapeHtml(t("journey.lowest", { confidence: formatKind(path.lowest_confidence) }))}</span>` : ""}
            ${riskChips}
          </div>
          <div class="query-actions">
            <button type="button" data-journey-focus="${index}">${escapeHtml(t("journey.focusPath"))}</button>
            <button type="button" data-journey-flow="${index}">${escapeHtml(t("flow.openView"))}</button>
          </div>
          <ol class="workflow-blocks">${steps}</ol>
        </section>
      `;
    })
    .join("");
  return `${summary}${sections}`;
}

function attachJourneyActions(container, report) {
  attachTraceNavigation(container);
  attachEdgeExplainActions(container);
  container.querySelectorAll("[data-journey-expand]").forEach((button) => {
    button.addEventListener("click", () => expandJourneyStep(button, report));
  });
  container.querySelectorAll("[data-journey-flow]").forEach((button) => {
    button.addEventListener("click", () => {
      const path = report.paths?.[Number(button.dataset.journeyFlow)];
      if (!path) return;
      const flowReport = journeyPathToFlowReport(report, path);
      if (!flowReport.blocks.length) return;
      openFlowView(
        flowReport,
        `${report.from?.label || ""} → ${report.to?.label || ""}`,
      );
    });
  });
  container.querySelectorAll("[data-journey-focus]").forEach((button) => {
    button.addEventListener("click", () => {
      const path = report.paths?.[Number(button.dataset.journeyFocus)];
      if (!path) return;
      const steps = Array.isArray(path.steps) ? path.steps : [];
      const focused = {
        query: `journey ${report.from?.label || ""} -> ${report.to?.label || ""}`,
        nodes: steps.map((step) => step.block?.node).filter(Boolean),
        edges: steps.map((step) => step.transition?.edge).filter(Boolean),
        total_nodes: steps.length,
        total_edges: steps.filter((step) => step.transition).length,
        truncated: false,
      };
      showFocusedGraph(
        focused,
        t("journey.focusTitle", { from: report.from?.label || "", to: report.to?.label || "" }),
        report.from?.id,
      );
    });
  });
}

async function expandJourneyStep(button, report) {
  const target = button.closest("li")?.querySelector("[data-journey-subflow]");
  if (!target) return;
  if (!target.hidden) {
    target.hidden = true;
    target.innerHTML = "";
    button.textContent = t("journey.expandStep");
    return;
  }
  const nodeId = Number(button.dataset.nodeIdExpand);
  if (!Number.isInteger(nodeId)) return;
  // Separate counter from runJourney: sharing state.journeyRequest made an
  // expanded step silently cancel an in-flight journey run (and leave its
  // button disabled, since the finally-guard token no longer matched).
  const requestToken = String((state.journeyExpandRequest = (state.journeyExpandRequest || 0) + 1));
  button.dataset.expandToken = requestToken;
  target.hidden = false;
  target.innerHTML = `<p class="empty">${escapeHtml(t("journey.subflowLoading"))}</p>`;
  button.disabled = true;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(nodeId),
    depth: "3",
    block_limit: "60",
    compact: "true",
  });

  try {
    const response = await apiFetch(`/api/workflow?${params.toString()}`);
    const body = await response.json();
    if (button.dataset.expandToken !== requestToken) return;
    if (!response.ok || !body) {
      throw new Error(apiErrorMessage(body, response, t("journey.subflowFailedFallback")));
    }
    const breadcrumb = `
      <div class="query-summary journey-breadcrumb">
        <span>${escapeHtml(
          t("journey.subflowTitle", {
            from: report.from?.label || "",
            to: report.to?.label || "",
            step: button.dataset.step || "",
            label: button.dataset.label || "",
          }),
        )}</span>
        <button type="button" data-journey-collapse>${escapeHtml(t("journey.collapseStep"))}</button>
      </div>
    `;
    target.innerHTML = `${breadcrumb}${renderWorkflow(body)}`;
    attachWorkflowNavigation(target);
    attachEdgeExplainActions(target);
    attachFlowViewActions(
      target,
      () => body,
      () => button.dataset.label || "",
    );
    target.querySelector("[data-journey-collapse]")?.addEventListener("click", () => {
      target.hidden = true;
      target.innerHTML = "";
      button.textContent = t("journey.expandStep");
    });
    button.textContent = t("journey.collapseStep");
  } catch (error) {
    if (button.dataset.expandToken !== requestToken) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (button.dataset.expandToken === requestToken) {
      button.disabled = false;
      delete button.dataset.expandToken;
    }
  }
}

function renderJourneyExportState() {
  journeyExportButton.disabled = !state.lastJourneyReport;
}

function exportLastJourneyReport() {
  if (!state.lastJourneyReport) return;
  const payload = {
    schema: "codegraph.journey.v1",
    ...state.lastJourneyReport,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const root = (state.lastJourneyReport.root || "project").replaceAll("/", "-").replaceAll(".", "") || "project";
  downloadBlob(blob, `codegraph-${root}-journey.json`);
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
    const response = await apiFetch(`/api/insights?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "insights failed"));
    }
    state.insightReport = body;
    refreshRiskIndex();
    renderInsights();
    renderLegend();
    draw();
  } catch (error) {
    if (requestId !== state.insightRequest) return;
    state.insightReport = null;
    refreshRiskIndex();
    renderInsights();
    renderLegend();
    draw();
  } finally {
    if (requestId === state.insightRequest) {
      insightFilterButton.disabled = false;
    }
  }
}

async function runCheck() {
  state.checkRequest += 1;
  const requestId = state.checkRequest;
  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  const failOn = checkFailOnInput.value.trim() || "error";
  const kind = insightKindInput.value.trim();
  const search = insightSearchInput.value.trim();
  params.set("fail_on", failOn);
  if (kind) params.set("kind", kind);
  if (search) params.set("search", search);
  params.set("limit", "50");

  checkButton.disabled = true;
  checkResult.innerHTML = `<p class="empty">${escapeHtml(t("check.running"))}</p>`;

  try {
    const response = await apiFetch(`/api/check?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.checkRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("check.failedFallback")));
    }
    state.lastCheckResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        fail_on: failOn,
        kind,
        search,
      },
      result: body,
    };
    renderCheckExportState();
    checkResult.innerHTML = renderCheckReport(body);
  } catch (error) {
    if (requestId !== state.checkRequest) return;
    checkResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.checkRequest) {
      checkButton.disabled = false;
    }
  }
}

function renderCheckExportState() {
  checkExportButton.disabled = !state.lastCheckResult;
}

function clearLastCheckResult() {
  state.lastCheckResult = null;
  renderCheckExportState();
}

function exportLastCheckResult() {
  if (!state.lastCheckResult) {
    checkResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noCheck"))}</p>`;
    renderCheckExportState();
    return;
  }

  const payload = {
    schema: "codegraph.check_result.v1",
    ...state.lastCheckResult,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-check-result.json`;
  downloadBlob(blob, fileName);
  checkResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.check"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(payload.result?.passed ? t("check.passed") : t("check.failed"))}</span>
        <span>${escapeHtml(t("check.failingCount", { count: formatNumber(payload.result?.failing_insights ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function exportCurrentInsights() {
  const clientInsights = state.insightReport ? [] : buildClientInsights(state.graph);
  const report = state.insightReport || {
    total: clientInsights.length,
    insights: clientInsights,
    severity_counts: countInsightField(clientInsights, "severity"),
    kind_counts: countInsightField(clientInsights, "kind"),
  };
  const payload = {
    schema: "codegraph.insights_export.v1",
    generated_at: new Date().toISOString(),
    root: state.graphPage.root || pathInput.value.trim() || ".",
    source: state.insightReport ? "server" : "client",
    filters: {
      severity: insightSeverityInput.value.trim(),
      kind: insightKindInput.value.trim(),
      search: insightSearchInput.value.trim(),
      fail_on: checkFailOnInput.value.trim() || "error",
    },
    report,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-insights.json`;
  downloadBlob(blob, fileName);
  checkResult.innerHTML = `
    <div class="query-summary">
      <span>${escapeHtml(t("export.insights"))}</span>
      <span>${escapeHtml(formatBytes(blob.size))}</span>
      <span>${escapeHtml(t("insights.count", { count: formatNumber(report.total ?? report.insights?.length ?? 0) }))}</span>
      <span class="query-expression">${escapeHtml(fileName)}</span>
    </div>
  `;
}

function countInsightField(insights, field) {
  return insights.reduce((counts, insight) => {
    const key = insight?.[field] || "unknown";
    counts[key] = (counts[key] || 0) + 1;
    return counts;
  }, {});
}

function renderCheckReport(check) {
  const stateClass = check.passed ? "passed" : "failed";
  const label = check.passed ? t("check.passed") : t("check.failed");
  return `
    <div class="check-card ${stateClass}">
      <strong>${escapeHtml(label)}</strong>
      <span>${escapeHtml(t("check.failOn", { severity: formatKind(check.fail_on || "error") }))}</span>
      <span>${escapeHtml(t("check.failingCount", { count: formatNumber(check.failing_insights || 0) }))}</span>
      <span>${escapeHtml(t("check.matchedCount", { count: formatNumber(check.report?.total || 0) }))}</span>
    </div>
  `;
}

function initializeGraph(options = {}) {
  const preserveView = Boolean(options.preserveView);
  const previousPan = { ...state.pan };
  const previousZoom = state.zoom;

  clearSelection({ syncUrl: false, render: false });
  state.edgeSelectionCache.clear();
  state.edgeSelectionNodeCache.clear();
  state.hoveredId = null;
  state.hoveredEdgeKey = null;
  state.positions.clear();
  state.velocities.clear();
  const kinds = [...new Set(state.graph.nodes.map((node) => node.kind))].sort();
  state.enabledKinds = new Set(kinds);
  // Sane default for large graphs: hide low-signal control-flow facts on
  // full graph pages until the user re-enables them via the legend. The
  // canvas-filter status shows the active filter with a one-click reset.
  if (
    options.largeGraphDefaults &&
    state.graphPage.totalNodes >= LARGE_GRAPH_FILTER_THRESHOLD &&
    kinds.length > 1 &&
    state.enabledKinds.has("control_flow")
  ) {
    state.enabledKinds.delete("control_flow");
  }
  refreshRiskIndex();
  renderKindFilters(kinds);
  renderLegend();
  state.layoutPaused = false;
  state.layoutSettled = false;
  renderViewportControls();

  seedGraphLayout();

  state.pan = preserveView ? previousPan : { x: cssWidth(canvas) / 2, y: cssHeight(canvas) / 2 };
  state.zoom = preserveView ? previousZoom : 1;
  applyFilters();
  startAnimation();
}

function seedGraphLayout() {
  const radius = Math.max(180, Math.min(cssWidth(canvas), cssHeight(canvas)) * 0.28);
  state.graph.nodes.forEach((node, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(1, state.graph.nodes.length);
    state.positions.set(node.id, {
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
    });
    state.velocities.set(node.id, { x: 0, y: 0 });
  });
}

async function runGraphQuery(options = {}) {
  const expression = queryInput.value.trim();
  if (!expression) {
    queryResult.innerHTML = `<p class="empty">${escapeHtml(t("query.enterExpression"))}</p>`;
    return null;
  }
  if (!graphQueryWithinClientLimit(expression, queryResult)) {
    clearLastQueryResult();
    return null;
  }

  state.queryRequest += 1;
  const requestId = state.queryRequest;
  queryButton.disabled = true;
  queryResult.innerHTML = `<p class="empty">${escapeHtml(t("query.running"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: expression,
  });

  try {
    const response = await apiFetch(`/api/query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.queryRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "query failed"));
    }
    if (options.syncUrl !== false) {
      syncQueryUrl(expression, { focus: Boolean(options.focus) });
    }
    queryResult.innerHTML = renderQueryResult(body);
    attachQueryNavigation(queryResult);
    attachEdgeExplainActions(queryResult);
    attachQueryFocusActions(queryResult, body);
    state.lastQueryResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      query: expression,
      result: body,
    };
    renderQueryExportState();
    rememberQuery(expression);
    if (options.focus) {
      focusQueryResult(body, queryResult);
      fitVisibleGraph();
    }
    return body;
  } catch (error) {
    if (requestId !== state.queryRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    return null;
  } finally {
    if (requestId === state.queryRequest) {
      queryButton.disabled = false;
    }
  }
}

async function runNaturalQuery(options = {}) {
  const question = askInput.value.trim();
  if (!question) {
    queryResult.innerHTML = `<p class="empty">${escapeHtml(t("ask.enterQuestion"))}</p>`;
    return null;
  }
  if (!graphQueryWithinClientLimit(question, queryResult)) {
    clearLastQueryResult();
    return null;
  }

  state.queryRequest += 1;
  const requestId = state.queryRequest;
  askButton.disabled = true;
  queryButton.disabled = true;
  queryResult.innerHTML = `<p class="empty">${escapeHtml(t("ask.running"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: question,
  });

  try {
    const response = await apiFetch(`/api/ask?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.queryRequest) return null;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("ask.failedFallback")));
    }
    const result = body.result || { query: body.generated_query || "", nodes: [], edges: [] };
    queryInput.value = body.generated_query || result.query || "";
    queryResult.innerHTML = renderNaturalQueryReport(body);
    attachQueryNavigation(queryResult);
    attachEdgeExplainActions(queryResult);
    attachQueryFocusActions(queryResult, result);
    attachNaturalQueryActions(queryResult);
    state.lastQueryResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      mode: "ask",
      question,
      query: body.generated_query || result.query || "",
      natural_query: {
        question: body.question || question,
        generated_query: body.generated_query || result.query || "",
        rule: body.rule || "",
        confidence: body.confidence || "",
        alternatives: body.alternatives || [],
      },
      result,
    };
    renderQueryExportState();
    rememberQuery(body.generated_query || result.query || "");
    if (options.focus !== false) {
      focusQueryResult(result, queryResult, { title: t("ask.resultLabel") });
      fitVisibleGraph();
      if (result.query) syncQueryUrl(result.query, { focus: true });
    }
    return body;
  } catch (error) {
    if (requestId !== state.queryRequest) return null;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    return null;
  } finally {
    if (requestId === state.queryRequest) {
      askButton.disabled = false;
      queryButton.disabled = false;
    }
  }
}

async function runSourceSearch() {
  const query = sourceSearchInput.value.trim();
  if (!query) {
    sourceSearchResult.innerHTML = `<p class="empty">${escapeHtml(t("sourceSearch.enterText"))}</p>`;
    return;
  }

  state.sourceSearchRequest += 1;
  const requestId = state.sourceSearchRequest;
  sourceSearchButton.disabled = true;
  sourceSearchResult.innerHTML = `<p class="empty">${escapeHtml(t("sourceSearch.searching"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: query,
    limit: "50",
    context: "2",
  });
  const pathFilter = sourcePathFilterInput.value.trim();
  if (pathFilter) params.set("path_filter", pathFilter);

  try {
    const response = await apiFetch(`/api/source-search?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.sourceSearchRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("sourceSearch.failedFallback")));
    }
    state.lastSourceSearchResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      query,
      path_filter: pathFilter,
      result: body,
    };
    renderSourceSearchExportState();
    sourceSearchResult.innerHTML = renderSourceSearchResult(body);
    attachSourceSearchActions(sourceSearchResult, body);
  } catch (error) {
    if (requestId !== state.sourceSearchRequest) return;
    sourceSearchResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.sourceSearchRequest) {
      sourceSearchButton.disabled = false;
    }
  }
}

function renderSourceSearchExportState() {
  sourceSearchExportButton.disabled = !state.lastSourceSearchResult;
}

function clearLastSourceSearchResult() {
  state.lastSourceSearchResult = null;
  renderSourceSearchExportState();
}

function exportLastSourceSearchResult() {
  if (!state.lastSourceSearchResult) {
    sourceSearchResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noSourceSearch"))}</p>`;
    renderSourceSearchExportState();
    return;
  }

  const payload = {
    schema: "codegraph.source_search_result.v1",
    ...state.lastSourceSearchResult,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-source-search.json`;
  downloadBlob(blob, fileName);
  sourceSearchResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.sourceSearch"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(t("sourceSearch.matchCount", { count: formatNumber(payload.result?.total_matches ?? payload.result?.matches?.length ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}
