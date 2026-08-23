// 12-filters.js — local canvas filters, severity ranks, kind filter chips.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function applyFilters() {
  const query = state.search;
  const visibleIds = new Set();
  state.visibleNodes = state.graph.nodes.filter((node) => {
    const kindEnabled = state.enabledKinds.has(node.kind);
    const focusHit = !state.queryFocus || state.queryFocus.nodeIds.has(node.id);
    const nodeRiskSeverity = state.riskByNode.get(Number(node.id));
    const riskHit = !state.activeRiskSeverity || nodeRiskSeverity === state.activeRiskSeverity;
    const searchHit =
      !query ||
      node.label.toLowerCase().includes(query) ||
      node.kind.toLowerCase().includes(query) ||
      Object.values(node.metadata || {}).some((value) =>
        String(value).toLowerCase().includes(query),
      );
    if (kindEnabled && focusHit && riskHit && searchHit) visibleIds.add(node.id);
    return kindEnabled && focusHit && riskHit && searchHit;
  });

  // A changed visible set changes the forces; let the layout run again.
  state.layoutSettled = false;
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
  skippedCount.textContent = String(state.summary?.skipped_files || 0);
  renderViewportControls();
  renderInsights();

  let selectionChanged = false;
  if (state.selectedId != null && !visibleIds.has(state.selectedId)) {
    state.selectedId = null;
    selectionChanged = true;
  }
  if (state.selectedEdgeKey && !state.visibleEdges.some((edge) => edgeSelectionKey(edge) === state.selectedEdgeKey)) {
    state.selectedEdgeKey = null;
    selectionChanged = true;
  }
  if (state.hoveredEdgeKey && !state.visibleEdges.some((edge) => edgeSelectionKey(edge) === state.hoveredEdgeKey)) {
    state.hoveredEdgeKey = null;
  }
  if (selectionChanged) syncSelectionUrl();
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
      ? `${severitySummary}${kindSummary}<p class="empty">${escapeHtml(t("empty.noInsights"))}</p>`
      : `<p class="empty">${escapeHtml(t("empty.noVisibleIssues"))}</p>`;
    attachInsightSeverityFilters();
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
          <span class="insight-message">
            <strong>${escapeHtml(formatKind(insight.kind))}</strong>
            ${escapeHtml(insight.message)}
          </span>
          ${renderInsightEvidence(insight)}
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
  attachInsightSeverityFilters();
  attachInsightKindFilters();
}

function renderInsightEvidence(insight) {
  const nodeCount = Array.isArray(insight.nodes) ? insight.nodes.length : 0;
  const edgeCount = Array.isArray(insight.edges) ? insight.edges.length : 0;
  if (nodeCount === 0 && edgeCount === 0) return "";
  const chips = [];
  if (nodeCount > 0) chips.push(`<span>${nodeCount} ${escapeHtml(t("stat.nodes").toLowerCase())}</span>`);
  if (edgeCount > 0) chips.push(`<span>${edgeCount} ${escapeHtml(t("stat.edges").toLowerCase())}</span>`);
  return `<span class="insight-meta">${chips.join("")}</span>`;
}

function renderInsightSeveritySummary(report) {
  if (!report?.by_severity) return "";
  const activeSeverity = insightSeverityInput.value.trim().toLowerCase();
  const rows = ["error", "warning", "info"]
    .map((severity) => {
      const count = report.by_severity[severity] || 0;
      const active = activeSeverity === severity;
      return `
        <button
          class="${severity}${active ? " active" : ""}"
          type="button"
          data-insight-severity="${severity}"
          aria-pressed="${active ? "true" : "false"}"
        >
          <span>${escapeHtml(formatKind(severity))}</span>
          <strong>${count}</strong>
        </button>
      `;
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

function attachInsightSeverityFilters() {
  insightList.querySelectorAll("[data-insight-severity]").forEach((button) => {
    button.addEventListener("click", () => {
      const severity = button.dataset.insightSeverity || "";
      insightSeverityInput.value =
        insightSeverityInput.value.trim().toLowerCase() === severity ? "" : severity;
      loadInsights();
    });
  });
}

function insightNodeIds(insight) {
  if (Array.isArray(insight.nodes) && insight.nodes.length > 0) return insight.nodes;
  if (insight.nodeId) return [insight.nodeId];
  return [];
}

function insightEdgeIndexes(insight) {
  return Array.isArray(insight.edges) ? insight.edges : [];
}

function refreshRiskIndex() {
  const riskByNode = new Map();
  const insights = state.insightReport?.insights || buildClientInsights(state.graph);
  insights.forEach((insight) => {
    const severity = insight.severity || "info";
    const rank = severityRank(severity);
    insightNodeIds(insight).forEach((nodeId) => {
      const key = Number(nodeId);
      const current = riskByNode.get(key);
      if (!current || rank > severityRank(current)) {
        riskByNode.set(key, severity);
      }
    });
  });
  state.riskByNode = riskByNode;
  state.riskSeverities = new Set(riskByNode.values());
  if (state.activeRiskSeverity && !state.riskSeverities.has(state.activeRiskSeverity)) {
    state.activeRiskSeverity = null;
  }
}

function visibleRiskSeverities() {
  return state.riskSeverities;
}

function severityRank(severity) {
  switch (severity) {
    case "error":
      return 3;
    case "warning":
      return 2;
    case "info":
      return 1;
    default:
      return 0;
  }
}

async function focusInsight(insight) {
  const nodeIds = insightNodeIds(insight);
  const edgeIndexes = insightEdgeIndexes(insight);
  const selectedId = nodeIds[0] ?? null;
  if (nodeIds.length === 0 && edgeIndexes.length === 0) return;

  if (selectedId != null) {
    selectNodeById(selectedId);
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
    const response = await apiFetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "focus failed"));
    }
    const label = t("insight.focusLabel", { kind: formatKind(insight.kind) });
    showFocusedGraph(body, label, selectedId);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function focusNodeId(nodeId, label) {
  if (!nodeId) return;
  selectNodeById(nodeId);

  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_ids: String(nodeId),
    edge_limit: "300",
  });

  try {
    const response = await apiFetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "focus failed"));
    }
    showFocusedGraph(body, label, nodeId);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function focusEdgeIndex(edgeIndex, options = {}) {
  if (!Number.isInteger(edgeIndex) || edgeIndex < 0) return;
  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    edge_indexes: String(edgeIndex),
    edge_limit: "20",
  });

  try {
    const response = await apiFetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "focus edge failed"));
    }
    showFocusedGraph(body, `Edge ${edgeIndex}`, null, { syncUrl: false });
    selectEdgeByKey(`edge:${edgeIndex}`, { syncUrl: options.syncUrl !== false });
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function runEdgeIndexQuery(edgeIndex) {
  if (!Number.isInteger(edgeIndex) || edgeIndex < 0) return;
  queryInput.value = `edges edge_index:${edgeIndex}`;
  await runGraphQuery({ focus: true });
  queryResult.scrollIntoView({ block: "nearest" });
}

function showFocusedGraph(result, label, selectedId = null, options = {}) {
  state.graph = { nodes: result.nodes, edges: result.edges };
  state.graphPage.nodeOffset = 0;
  state.graphPage.edgeOffset = 0;
  state.graphPage.totalNodes = result.total_nodes;
  state.graphPage.totalEdges = result.total_edges;
  state.graphPage.truncatedNodes = false;
  state.graphPage.truncatedEdges = Boolean(result.truncated_edges);
  state.selectedEdgeKey = null;
  state.queryFocus = null;
  queryResult.innerHTML = renderQueryResult(result);
  attachQueryNavigation(queryResult);
  attachEdgeExplainActions(queryResult);
  attachQueryFocusActions(queryResult, result);
  rootLabel.textContent = label;
  initializeGraph({ preserveView: false });
  pageInfo.textContent = t("page.focusInfo", {
    shown: result.nodes.length,
    total: result.total_nodes,
  });
  renderGraphPageScope({ focused: true });
  pagePrevButton.disabled = true;
  pageNextButton.disabled = true;
  edgePrevButton.disabled = true;
  edgeNextButton.disabled = true;
  renderStaticEdgePageInfo();
  pageCopyButton.disabled = false;
  pageClearButton.disabled = false;
  pageReloadButton.disabled = false;
  if (selectedId != null) {
    selectNodeById(selectedId, { syncUrl: options.syncUrl !== false });
  } else if (options.syncUrl !== false) {
    syncSelectionUrl();
  }
}

// What the server calls `heuristic_scan_severity`: on a syntax-only scan an
// unresolved or ambiguous call is the expected default and reads as info,
// and only becomes a warning once semantic enrichment has run and the
// target still cannot be found.
function clientHeuristicSeverity(graph) {
  return (graph.edges || []).some((edge) => edge.confidence === "semantic") ? "warning" : "info";
}

function buildClientInsights(graph) {
  const nodesById = new Map((graph.nodes || []).map((node) => [node.id, node]));
  const insights = [];
  const heuristicSeverity = clientHeuristicSeverity(graph);
  const entrypointIds = new Set(
    graph.edges.filter((edge) => edge.kind === "entrypoint").map((edge) => edge.target),
  );
  const reachableIds = clientEntrypointReachableIds(graph, entrypointIds);
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
        // A limit of the extraction rather than a defect in the code; kept
        // in step with the server's own severity.
        severity: "info",
        message: `${node.label} contains syntax error nodes`,
        nodeId: node.id,
      });
    }

    if (node.kind === "function" && !entrypointIds.has(node.id) && !calledIds.has(node.id)) {
      insights.push({
        kind: "orphan_function",
        severity: "info",
        // Dead or the API: terraform has 11406 exported functions with no
        // in-repo caller against 592 unexported ones, and the CLI says
        // which is which.
        message:
          node.metadata?.visibility === "public"
            ? `${node.label} has no incoming call edge; it is exported, so its callers may be outside this repository`
            : `${node.label} has no incoming call edge`,
        nodeId: node.id,
      });
    }
  });

  // A call through a value the body binds has nothing to find, so it is not
  // a gap in the extraction: terraform's `done()` comes from `runningCtx,
  // done := context.WithCancel(...)`.
  const unresolvedCallers = new Map();
  graph.edges
    .filter((edge) => edge.kind === "calls")
    .forEach((edge) => {
      const list = unresolvedCallers.get(edge.target) || [];
      list.push(edge);
      unresolvedCallers.set(edge.target, list);
    });
  const unresolvedByLabel = new Map();
  graph.nodes
    .filter(
      (node) =>
        node.metadata?.item_kind === "call" && node.metadata?.resolution === "unresolved",
    )
    .forEach((node) => {
      const list = unresolvedByLabel.get(node.label) || [];
      list.push(node);
      unresolvedByLabel.set(node.label, list);
    });
  unresolvedByLabel.forEach((nodes, label) => {
    const edges = nodes.flatMap((node) => unresolvedCallers.get(node.id) || []);
    if (
      edges.length > 0 &&
      edges.every((edge) => edge.metadata?.unresolved_reason === "local_value")
    ) {
      return;
    }
    insights.push({
      kind: "unresolved_call",
      severity: heuristicSeverity,
      message:
        nodes.length > 1
          ? `Call target ${label} could not be resolved syntactically, in ${nodes.length} placeholder nodes`
          : `Call target ${label} could not be resolved syntactically`,
      nodeId: nodes[0].id,
    });
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

  const entrypointLabels = new Map();
  graph.nodes
    .filter((node) => node.kind === "entrypoint")
    .forEach((node) => {
      const list = entrypointLabels.get(node.label) || [];
      list.push(node);
      entrypointLabels.set(node.label, list);
    });
  entrypointLabels.forEach((nodes, label) => {
    if (nodes.length > 1) {
      insights.push({
        kind: "duplicate_entrypoint_label",
        severity: "info",
        message: `${label} appears ${nodes.length} times, so a trace by that label has to say which one`,
        nodeId: nodes[0].id,
      });
    }
  });

  // Ambiguity is a placeholder node the resolver left behind, not two
  // edges from one caller: it stopped writing those the day it started
  // bounding the uncertainty in one node, and this rule went quiet with
  // them.
  graph.nodes
    .filter(
      (node) => node.metadata?.item_kind === "call" && node.metadata?.resolution === "ambiguous",
    )
    .forEach((node) => {
      const count = node.metadata?.candidate_count || "multiple";
      const sample = node.metadata?.candidate_sample;
      const callers = (unresolvedCallers.get(node.id) || [])
        .map((edge) => nodesById.get(edge.source)?.label)
        .filter(Boolean);
      const from = callers.length > 0 ? ` called from ${callers.slice(0, 3).join(", ")}` : "";
      insights.push({
        kind: "ambiguous_call_resolution",
        severity: heuristicSeverity,
        message: `Call ${node.label}${from} matches ${count} definitions${sample ? `; sample: ${sample}` : ""}`,
        nodeId: node.id,
      });
    });

  graph.edges
    .filter((edge) => edge.kind === "may_error")
    .forEach((edge) => {
      const source = nodesById.get(edge.source);
      const target = nodesById.get(edge.target);
      insights.push({
        kind: "potential_error_flow",
        // Raising is ordinary control flow in Result- and exception-shaped
        // code, so this is context rather than a defect -- the same
        // severity the server gives it.
        severity: "info",
        message: `${source?.label || edge.source} may error via ${target?.label || edge.target}`,
        nodeId: source?.id || target?.id,
      });
      if (reachableIds.size > 0 && !reachableIds.has(edge.source)) {
        insights.push({
          kind: "unreachable_error_flow",
          // Reachability is heuristic on a syntax-only scan and error
          // constructs are everywhere; context, not a defect.
          severity: "info",
          message: `${source?.label || edge.source} may error via ${target?.label || edge.target} but is not reachable from any entrypoint`,
          nodeId: source?.id || target?.id,
        });
      }
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

function clientEntrypointReachableIds(graph, entrypointIds) {
  const reachable = new Set();
  const queue = [];
  const outgoing = new Map();
  // Most entrypoints name a file, and a file holds its symbols through
  // `contains`; without that step the walk stops at the file and calls
  // everything inside it unreachable. `contains` is followed only out of a
  // file, so the repository root does not reach the whole project by
  // containment alone. Kept in step with `entrypoint_reachable_nodes_from`
  // on the server.
  const fileIds = new Set(
    (graph.nodes || []).filter((node) => node.kind === "file").map((node) => node.id),
  );

  (graph.edges || []).forEach((edge) => {
    if (!clientTraceEdge(edge) && !(edge?.kind === "contains" && fileIds.has(edge.source))) return;
    const list = outgoing.get(edge.source) || [];
    list.push(edge.target);
    outgoing.set(edge.source, list);
  });

  entrypointIds.forEach((id) => {
    if (reachable.has(id)) return;
    reachable.add(id);
    queue.push(id);
  });

  while (queue.length > 0) {
    const id = queue.shift();
    (outgoing.get(id) || []).forEach((target) => {
      if (reachable.has(target)) return;
      reachable.add(target);
      queue.push(target);
    });
  }

  return reachable;
}

function clientTraceEdge(edge) {
  return [
    "calls",
    "references",
    "imports",
    "reads_config",
    "reads_environment",
    "may_error",
    "depends_on",
  ].includes(edge?.kind);
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
  // One finding per package, as the CLI reports it: a package imported from
  // sixty files is one fact about the manifest, not sixty.
  const grouped = new Map();
  graph.edges
    .filter((edge) => edge.kind === "imports")
    .forEach((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      const scope = target?.metadata?.import_scope;
      if (scope === "local" || scope === "workspace") return;
      const label = target?.label || "";
      // `import(`${pkgDir}/package.json`)` names no package: the specifier
      // is built when it runs.
      if (label.includes("${")) return;
      const candidate = importPackageCandidate(target?.metadata?.language, label);
      if (!candidate) return;
      if (!declaredEcosystems.has(candidate.ecosystem)) return;
      if (isDeclaredPackage(declared, candidate.ecosystem, candidate.package)) return;
      // `import type { X } from 'trusted-types/lib'` is served by
      // `@types/trusted-types`, and needs nothing at run time.
      if (
        label.trimStart().startsWith("import type ") &&
        isDeclaredPackage(declared, candidate.ecosystem, typesPackageName(candidate.package))
      ) {
        return;
      }

      const key = `${candidate.ecosystem}:${candidate.package}`;
      const group = grouped.get(key) || { candidate, sources: [], nodeId: target?.id || source?.id };
      const sourceLabel = source?.label || String(edge.source);
      if (!group.sources.includes(sourceLabel)) group.sources.push(sourceLabel);
      grouped.set(key, group);
    });

  grouped.forEach((group) => {
    const shown = group.sources.slice(0, 3).join(", ");
    const rest = group.sources.length - 3;
    const from = rest > 0 ? `${shown}, and ${rest} more` : shown;
    insights.push({
      kind: "undeclared_external_import",
      severity: "warning",
      message: `${group.candidate.package} is imported from ${from} without a matching ${group.candidate.ecosystem} dependency`,
      nodeId: group.nodeId,
    });
  });
}

// The DefinitelyTyped package that carries types for another.
function typesPackageName(packageName) {
  if (packageName.startsWith("@")) {
    const [scope, name] = packageName.slice(1).split("/");
    if (scope && name) return `@types/${scope}__${name}`;
  }
  return `@types/${packageName}`;
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
    case "php":
      return phpImportPackage(label);
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
  // `node:fs`, `bun:test`, `jsr:@std/assert` and `https://deno.land/x/...`
  // say where the module comes from, not which package declares it.
  if (
    moduleName.startsWith(".") ||
    moduleName.startsWith("/") ||
    moduleName.split("/")[0].includes(":") ||
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

function phpImportPackage(label) {
  const namespaces = phpImportNamespaces(label);
  for (const namespace of namespaces) {
    const candidate = phpNamespacePackage(namespace);
    if (candidate) return { ecosystem: "composer", package: candidate };
  }
  return null;
}

function phpImportNamespaces(label) {
  let value = String(label || "").trim().replace(/;$/, "").trim();
  if (value.startsWith("use ")) value = value.slice(4).trim();
  if (value.startsWith("function ")) value = value.slice(9).trim();
  if (value.startsWith("const ")) value = value.slice(6).trim();

  const groupStart = value.indexOf("{");
  const groupEnd = value.indexOf("}");
  if (groupStart >= 0 && groupEnd > groupStart) {
    const prefix = value.slice(0, groupStart).trim().replace(/\\+$/, "");
    return value
      .slice(groupStart + 1, groupEnd)
      .split(",")
      .map((part) => phpNamespaceWithoutAlias(part))
      .filter(Boolean)
      .map((part) => (prefix ? `${prefix}\\${part}` : part));
  }

  const namespace = phpNamespaceWithoutAlias(value);
  return namespace ? [namespace] : [];
}

function phpNamespaceWithoutAlias(value) {
  return String(value || "")
    .split(/\s+as\s+/i)[0]
    .trim()
    .replace(/^\\+/, "");
}

function phpNamespacePackage(namespace) {
  const parts = String(namespace || "")
    .split("\\")
    .map((part) => part.trim())
    .filter(Boolean);
  if (parts.length < 2 || phpNonComposerNamespaceRoots.has(parts[0])) return null;

  if (parts[0] === "Monolog") return "monolog/monolog";
  if (parts[0] === "PHPUnit") return "phpunit/phpunit";
  if (parts[0] === "GuzzleHttp") return "guzzlehttp/guzzle";
  if (parts[0] === "Symfony" && parts[1] === "Component" && parts[2]) {
    return `symfony/${composerPackagePart(parts[2])}`;
  }
  if (parts[0] === "Psr" && parts[1]) return `psr/${composerPackagePart(parts[1])}`;

  return `${composerPackagePart(parts[0])}/${composerPackagePart(parts[1])}`;
}

function composerPackagePart(value) {
  return String(value || "")
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .replace(/[_.-]+/g, "-")
    .toLowerCase()
    .replace(/^-+|-+$/g, "");
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

// `module.builtinModules` in full, subpaths included, and the same list the
// CLI keeps: a name missing here becomes a warning that the project forgot
// a dependency.
const nodeBuiltinModules = new Set([
  "assert",
  "assert/strict",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "diagnostics_channel",
  "dns",
  "dns/promises",
  "domain",
  "events",
  "fs",
  "fs/promises",
  "http",
  "http2",
  "https",
  "inspector",
  "inspector/promises",
  "module",
  "net",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "punycode",
  "querystring",
  "readline",
  "readline/promises",
  "repl",
  "stream",
  "stream/consumers",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
  "timers",
  "timers/promises",
  "tls",
  "trace_events",
  "tty",
  "url",
  "util",
  "util/types",
  "v8",
  "vm",
  "wasi",
  "worker_threads",
  "zlib",
]);

const phpNonComposerNamespaceRoots = new Set([
  "App",
  "Tests",
  "Test",
  "Database",
  "Config",
  "DateTime",
  "DateTimeImmutable",
  "DateTimeInterface",
  "DateInterval",
  "DateTimeZone",
  "Exception",
  "RuntimeException",
  "InvalidArgumentException",
  "Throwable",
  "Closure",
  "ArrayObject",
  "Iterator",
  "IteratorAggregate",
  "Traversable",
  "Countable",
  "JsonSerializable",
  "PDO",
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
    input.checked = state.enabledKinds.has(kind);
    input.dataset.nodeKind = kind;
    input.addEventListener("change", () => {
      setNodeKindFilter(kind, input.checked);
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

function setNodeKindFilter(kind, enabled) {
  if (enabled) state.enabledKinds.add(kind);
  else state.enabledKinds.delete(kind);
  syncKindFilterControls();
  applyFilters();
  renderLegend();
  draw();
}

function syncKindFilterControls() {
  kindFilters.querySelectorAll("[data-node-kind]").forEach((input) => {
    input.checked = state.enabledKinds.has(input.dataset.nodeKind);
  });
}

function graphKindList() {
  return [...new Set(state.graph.nodes.map((node) => node.kind))].sort();
}

function renderLegend() {
  legend.innerHTML = "";
  graphKindList().forEach((kind) => {
    const active = state.enabledKinds.has(kind);
    const item = document.createElement("button");
    item.type = "button";
    item.className = `legend-item node-kind${active ? " active" : ""}`;
    item.dataset.nodeKind = kind;
    item.setAttribute("aria-pressed", active ? "true" : "false");
    item.setAttribute("aria-label", t("legend.kindFilter", { kind: formatKind(kind) }));
    const swatch = document.createElement("span");
    swatch.className = "swatch";
    swatch.style.background = colorFor(kind);
    const text = document.createElement("span");
    text.textContent = formatKind(kind);
    item.append(swatch, text);
    legend.append(item);
  });
  legend.querySelectorAll("[data-node-kind]").forEach((button) => {
    button.addEventListener("click", () => {
      const kind = button.dataset.nodeKind;
      setNodeKindFilter(kind, !state.enabledKinds.has(kind));
    });
  });
  const riskSeverities = visibleRiskSeverities();
  ["error", "warning", "info"].forEach((severity) => {
    if (!riskSeverities.has(severity)) return;
    const item = document.createElement("button");
    item.type = "button";
    item.className = `legend-item risk risk-filter${
      state.activeRiskSeverity === severity ? " active" : ""
    }`;
    item.dataset.riskSeverity = severity;
    item.setAttribute("aria-pressed", state.activeRiskSeverity === severity ? "true" : "false");
    item.setAttribute("aria-label", t("legend.riskFilter", { severity: formatKind(severity) }));
    const swatch = document.createElement("span");
    swatch.className = `swatch risk-swatch ${severity}`;
    const text = document.createElement("span");
    text.textContent = formatKind(severity);
    item.append(swatch, text);
    legend.append(item);
  });
  legend.querySelectorAll("[data-risk-severity]").forEach((button) => {
    button.addEventListener("click", () => {
      const severity = button.dataset.riskSeverity || "";
      state.activeRiskSeverity = state.activeRiskSeverity === severity ? null : severity;
      applyFilters();
      renderLegend();
      draw();
    });
  });
}

function startAnimation() {
  if (state.animationFrame) cancelAnimationFrame(state.animationFrame);
  let framesSinceSimulation = 0;
  const tick = () => {
    if (!state.layoutPaused) {
      // The force layout is the expensive part (pairwise repulsion). Once it
      // settles, running it every frame is pure heat — skip it and re-check
      // periodically so any disturbance still resumes the simulation even if
      // a caller forgets to clear layoutSettled.
      if (!state.layoutSettled || framesSinceSimulation >= LAYOUT_RECHECK_FRAMES) {
        framesSinceSimulation = 0;
        state.layoutSettled = simulateLayout() < LAYOUT_SETTLE_SPEED;
      } else {
        framesSinceSimulation += 1;
      }
    }
    draw();
    state.animationFrame = requestAnimationFrame(tick);
  };
  tick();
}

function renderViewportControls() {
  viewportInfo.textContent = `${state.visibleNodes.length} ${t("stat.nodes").toLowerCase()} / ${state.visibleEdges.length} ${t("stat.edges").toLowerCase()}`;
  renderGraphHud();
  toggleLayoutButton.textContent = state.layoutPaused ? t("button.resume") : t("button.pause");
  toggleLayoutButton.setAttribute(
    "aria-label",
    state.layoutPaused ? t("aria.resumeLayout") : t("aria.pauseLayout"),
  );
  fitGraphButton.disabled = state.visibleNodes.length === 0;
  resetLayoutButton.disabled = state.graph.nodes.length === 0;
  zoomInButton.disabled = state.graph.nodes.length === 0;
  zoomOutButton.disabled = state.graph.nodes.length === 0;
  toggleLayoutButton.disabled = state.graph.nodes.length === 0;
  clearCanvasFiltersButton.disabled = canvasFilterCount() === 0;
  exportSliceButton.disabled = state.visibleNodes.length === 0 && state.visibleEdges.length === 0;
  labelModeButtons.forEach((button) => {
    const active = button.dataset.labelMode === state.labelMode;
    button.setAttribute("aria-pressed", active ? "true" : "false");
    button.disabled = state.graph.nodes.length === 0;
  });
}

function renderGraphHud() {
  const zoom = `${Math.round(state.zoom * 100)}%`;
  const layout = state.layoutPaused ? t("graph.paused") : t("graph.running");
  const slice = graphSliceLabel();
  const filters = canvasFilterCount();
  const items = [
    [t("label.nodes"), formatNumber(state.visibleNodes.length)],
    [t("label.edges"), formatNumber(state.visibleEdges.length)],
    [t("graph.slice"), slice],
    [
      t("graph.filters"),
      filters > 0
        ? t("graph.filtersActive", { count: formatNumber(filters) })
        : t("graph.filtersNone"),
    ],
    [t("graph.zoom"), zoom],
    [t("graph.layout"), layout],
  ];
  graphHud.innerHTML = items
    .map(
      ([label, value]) => `
        <span>
          <em>${escapeHtml(label)}</em>
          <strong>${escapeHtml(value)}</strong>
        </span>
      `,
    )
    .join("");
}

function graphSliceLabel() {
  if (state.queryFocus) {
    return `${formatNumber(state.visibleNodes.length)} · ${formatNumber(state.visibleEdges.length)}`;
  }
  return `${formatNumber(state.graph.nodes.length)}/${formatNumber(state.graphPage.totalNodes)} · ${formatNumber(state.graph.edges.length)}/${formatNumber(state.graphPage.totalEdges)}`;
}

function renderStaticEdgePageInfo() {
  edgePageInfo.textContent = `${formatNumber(state.graph.edges.length)} ${t("label.edges").toLowerCase()}`;
}

function zoomAtCanvasCenter(scale) {
  zoomAt(cssWidth(canvas) / 2, cssHeight(canvas) / 2, scale);
}

function zoomAt(screenX, screenY, scale) {
  const before = screenToWorld(screenX, screenY);
  state.zoom = Math.max(0.18, Math.min(3.5, state.zoom * scale));
  const after = screenToWorld(screenX, screenY);
  state.pan.x += (after.x - before.x) * state.zoom;
  state.pan.y += (after.y - before.y) * state.zoom;
  renderGraphHud();
  draw();
}
