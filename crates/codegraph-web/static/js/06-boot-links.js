// 06-boot-links.js — startup flow and shareable deep-link read/write helpers.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

async function init() {
  renderPanelHints();
  await Promise.all([loadProjects(), loadCapabilities(), loadApiSchema(), loadMetrics()]);
  applyUrlState();
  loadJobQueue();
  scan();
}

function applyUrlState() {
  const link = readSelectionLinkFromUrl();
  if (link.path) {
    pathInput.value = link.path;
    if ([...projectSelect.options].some((option) => option.value === link.path)) {
      projectSelect.value = link.path;
    }
  }
  applyGraphPageLink(link.graphPage);
  if (link.nodeId != null || link.edgeIndex != null) {
    state.pendingSelectionLink = link;
  }
  if (link.query) {
    queryInput.value = link.query;
    state.pendingQueryLink = {
      query: link.query,
      focus: link.queryFocus,
    };
  }
  if (link.flowNodeId != null) {
    state.pendingFlowLink = link.flowNodeId;
  }
}

// The flow view is a shareable, reload-surviving deep link: `flow=<nodeId>`
// records the node the current flow is rooted on.
function syncFlowUrl(rootNodeId) {
  try {
    const url = new URL(window.location.href);
    writePathUrlParam(url);
    if (rootNodeId != null) {
      url.searchParams.set("flow", String(rootNodeId));
    } else {
      url.searchParams.delete("flow");
    }
    window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
  } catch (error) {
    // Flow links are best-effort; the flow view works without the History API.
  }
}

function flowShareUrl() {
  const url = new URL(window.location.href);
  writePathUrlParam(url);
  if (state.flow.rootNodeId != null) {
    url.searchParams.set("flow", String(state.flow.rootNodeId));
  }
  return url.toString();
}

async function copyFlowLink(button) {
  try {
    await writeClipboardText(flowShareUrl());
    const previous = button.textContent;
    button.textContent = t("button.copied");
    window.setTimeout(() => {
      button.textContent = previous || t("flow.copyLink");
    }, 1200);
  } catch (error) {
    // Clipboard is a convenience; the URL is still shareable from the address bar.
  }
}

async function restorePendingFlowLink() {
  const nodeId = state.pendingFlowLink;
  if (nodeId == null) return;
  const node = (state.graph?.nodes || []).find((candidate) => candidate.id === nodeId) || {
    id: nodeId,
    label: "",
  };
  setStageView("flow");
  state.pendingFlowLink = null;
  await buildFlowFromNode(node);
}

function readSelectionLinkFromUrl() {
  try {
    const params = new URLSearchParams(window.location.search);
    return {
      path: params.get("path") || "",
      nodeId: parseUrlInteger(params.get("node")),
      edgeIndex: parseUrlInteger(params.get("edge")),
      flowNodeId: parseUrlInteger(params.get("flow")),
      query: params.get("query") || "",
      queryFocus: params.get("query_focus") === "1",
      graphPage: readGraphPageLink(params),
    };
  } catch (error) {
    return {
      path: "",
      nodeId: null,
      edgeIndex: null,
      flowNodeId: null,
      query: "",
      queryFocus: false,
      graphPage: null,
    };
  }
}

function readGraphPageLink(params) {
  const values = {
    nodeOffset: parseUrlInteger(params.get("node_offset")),
    nodeLimit: parseUrlInteger(params.get("node_limit")),
    edgeOffset: parseUrlInteger(params.get("edge_offset")),
    edgeLimit: parseUrlInteger(params.get("edge_limit")),
    pathPrefix: params.get("path_prefix") || "",
    kind: params.get("kind") || "",
    itemKind: params.get("item_kind") || "",
    language: params.get("language") || "",
    search: params.get("search") || "",
    edgeKind: params.get("edge_kind") || "",
    confidence: params.get("confidence") || "",
    edgeRelation: params.get("edge_relation") || "",
    edgeSource: params.get("edge_source") || "",
  };
  const hasValue = Object.values(values).some((value) => value != null && value !== "");
  return hasValue ? values : null;
}

function applyGraphPageLink(link) {
  if (!link) return;
  state.pendingGraphPageLink = true;
  if (link.nodeOffset != null) state.graphPage.nodeOffset = link.nodeOffset;
  if (link.edgeOffset != null) state.graphPage.edgeOffset = link.edgeOffset;
  if (link.nodeLimit != null) nodeLimitInput.value = String(link.nodeLimit);
  if (link.edgeLimit != null) edgeLimitInput.value = String(link.edgeLimit);
  if (link.pathPrefix) state.architecturePathPrefix = link.pathPrefix;
  serverKindInput.value = link.kind || "";
  serverItemKindInput.value = link.itemKind || "";
  serverLanguageInput.value = link.language || "";
  serverSearchInput.value = link.search || "";
  serverEdgeKindInput.value = link.edgeKind || "";
  serverConfidenceInput.value = link.confidence || "";
  serverEdgeRelationInput.value = link.edgeRelation || "";
  serverEdgeSourceInput.value = link.edgeSource || "";
}

function parseUrlInteger(value) {
  if (value == null || value === "") return null;
  const number = Number(value);
  return Number.isInteger(number) && number >= 0 ? number : null;
}

function syncSelectionUrl() {
  try {
    const edgeIndex = state.selectedEdgeKey ? selectedEdgeIndexFromKey(state.selectedEdgeKey) : null;
    const href = buildSelectionUrl({
      nodeId: state.selectedId,
      edgeIndex,
      absolute: false,
    });
    window.history.replaceState(null, "", href);
  } catch (error) {
    // URL state is a sharing convenience; selection still works without History API.
  }
}

function buildSelectionUrl({ nodeId = null, edgeIndex = null, absolute = true } = {}) {
  const url = new URL(window.location.href);
  writePathUrlParam(url);

  if (nodeId != null) {
    url.searchParams.set("node", String(nodeId));
    url.searchParams.delete("edge");
    url.searchParams.delete("query");
    url.searchParams.delete("query_focus");
  } else if (edgeIndex != null) {
    url.searchParams.set("edge", String(edgeIndex));
    url.searchParams.delete("node");
    url.searchParams.delete("query");
    url.searchParams.delete("query_focus");
  } else {
    url.searchParams.delete("node");
    url.searchParams.delete("edge");
  }

  return absolute ? url.toString() : `${url.pathname}${url.search}${url.hash}`;
}

function buildQueryUrl(expression, { focus = false, absolute = true } = {}) {
  const url = new URL(window.location.href);
  writePathUrlParam(url);
  url.searchParams.set("query", expression);
  if (focus) {
    url.searchParams.set("query_focus", "1");
  } else {
    url.searchParams.delete("query_focus");
  }
  url.searchParams.delete("node");
  url.searchParams.delete("edge");
  return absolute ? url.toString() : `${url.pathname}${url.search}${url.hash}`;
}

function buildGraphPageUrl({ absolute = true } = {}) {
  const url = new URL(window.location.href);
  writePathUrlParam(url);
  writeOptionalIntegerUrlParam(url, "node_offset", state.graphPage.nodeOffset, 0);
  writeOptionalIntegerUrlParam(url, "node_limit", Number(nodeLimitInput.value || 250), 250);
  writeOptionalIntegerUrlParam(url, "edge_offset", state.graphPage.edgeOffset, 0);
  writeOptionalIntegerUrlParam(url, "edge_limit", Number(edgeLimitInput.value || 500), 500);
  writeOptionalStringUrlParam(url, "path_prefix", state.architecturePathPrefix);
  writeOptionalStringUrlParam(url, "kind", serverKindInput.value.trim());
  writeOptionalStringUrlParam(url, "item_kind", serverItemKindInput.value.trim());
  writeOptionalStringUrlParam(url, "language", serverLanguageInput.value.trim());
  writeOptionalStringUrlParam(url, "search", serverSearchInput.value.trim());
  writeOptionalStringUrlParam(url, "edge_kind", serverEdgeKindInput.value.trim());
  writeOptionalStringUrlParam(url, "confidence", serverConfidenceInput.value.trim());
  writeOptionalStringUrlParam(url, "edge_relation", serverEdgeRelationInput.value.trim());
  writeOptionalStringUrlParam(url, "edge_source", serverEdgeSourceInput.value.trim());
  url.searchParams.delete("node");
  url.searchParams.delete("edge");
  url.searchParams.delete("query");
  url.searchParams.delete("query_focus");
  return absolute ? url.toString() : `${url.pathname}${url.search}${url.hash}`;
}

function syncGraphPageUrl() {
  try {
    window.history.replaceState(null, "", buildGraphPageUrl({ absolute: false }));
  } catch (error) {
    // Graph page URLs are best-effort; the graph page itself works without History API.
  }
}

function writePathUrlParam(url) {
  const root = pathInput.value.trim();
  if (root && root !== ".") {
    url.searchParams.set("path", root);
  } else {
    url.searchParams.delete("path");
  }
}

function writeOptionalIntegerUrlParam(url, name, value, defaultValue) {
  const number = Number(value);
  if (Number.isInteger(number) && number >= 0 && number !== defaultValue) {
    url.searchParams.set(name, String(number));
  } else {
    url.searchParams.delete(name);
  }
}

function writeOptionalStringUrlParam(url, name, value) {
  const text = String(value || "").trim();
  if (text) {
    url.searchParams.set(name, text);
  } else {
    url.searchParams.delete(name);
  }
}

function syncQueryUrl(expression, options = {}) {
  try {
    window.history.replaceState(null, "", buildQueryUrl(expression, { ...options, absolute: false }));
  } catch (error) {
    // Query links are best-effort; query execution itself does not depend on History API.
  }
}

async function restorePendingQueryLink() {
  const link = state.pendingQueryLink;
  state.pendingQueryLink = null;
  if (!link?.query) return;
  queryInput.value = link.query;
  await runGraphQuery({ focus: link.focus, syncUrl: false });
}

function selectedEdgeIndexFromKey(selectionKey) {
  const edgeRecord = selectedEdge();
  const edgeIndex = edgeIndexOf(edgeRecord?.edge);
  if (edgeIndex != null) return edgeIndex;
  const match = String(selectionKey || "").match(/^edge:(\d+)$/);
  return match ? Number(match[1]) : null;
}

async function copySelectionLink(kind, value, button) {
  const id = Number(value);
  if (!Number.isInteger(id) || id < 0) return;
  const href = buildSelectionUrl({
    nodeId: kind === "node" ? id : null,
    edgeIndex: kind === "edge" ? id : null,
  });
  await writeClipboardText(href);
  const previous = button.textContent;
  button.textContent = t("button.copied");
  window.setTimeout(() => {
    button.textContent = previous || t("button.copyLink");
  }, 1200);
}

async function writeClipboardText(value) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) throw new Error("clipboard copy failed");
}

function attachCopyLinkActions(container) {
  container.querySelectorAll("[data-copy-selection-link]").forEach((button) => {
    button.addEventListener("click", async () => {
      button.disabled = true;
      try {
        await copySelectionLink(button.dataset.copySelectionLink, button.dataset.selectionLinkId, button);
      } catch (error) {
        button.textContent = error.message;
        window.setTimeout(() => {
          button.textContent = t("button.copyLink");
        }, 1600);
      } finally {
        button.disabled = false;
      }
    });
  });
}

async function copyCurrentQueryLink(button) {
  const expression = queryInput.value.trim();
  if (!expression) return;
  button.disabled = true;
  try {
    await writeClipboardText(buildQueryUrl(expression, { focus: Boolean(state.queryFocus) }));
    const previous = button.textContent;
    button.textContent = t("button.copied");
    window.setTimeout(() => {
      button.textContent = previous || t("button.copyQueryLink");
    }, 1200);
  } catch (error) {
    button.textContent = error.message;
    window.setTimeout(() => {
      button.textContent = t("button.copyQueryLink");
    }, 1600);
  } finally {
    button.disabled = false;
  }
}

async function copyGraphPageLink(button) {
  button.disabled = true;
  try {
    const href = buildGraphPageUrl();
    await writeClipboardText(href);
    syncGraphPageUrl();
    const previous = button.textContent;
    button.textContent = t("button.copied");
    window.setTimeout(() => {
      button.textContent = previous || t("button.copyPageLink");
    }, 1200);
  } catch (error) {
    button.textContent = error.message;
    window.setTimeout(() => {
      button.textContent = t("button.copyPageLink");
    }, 1600);
  } finally {
    button.disabled = false;
  }
}

function selectNodeById(nodeId, options = {}) {
  const id = Number(nodeId);
  if (!Number.isInteger(id) || id < 0) return;
  const syncUrl = options.syncUrl !== false;
  state.selectedEdgeKey = null;
  state.hoveredEdgeKey = null;
  state.selectedId = id;
  if (syncUrl) syncSelectionUrl();
  renderSelection();
  draw();
}

function clearSelection(options = {}) {
  const syncUrl = options.syncUrl !== false;
  state.selectedId = null;
  state.selectedEdgeKey = null;
  state.hoveredEdgeKey = null;
  if (syncUrl) syncSelectionUrl();
  if (options.render !== false) renderSelection();
}

async function restorePendingSelectionLink() {
  const link = state.pendingSelectionLink || readSelectionLinkFromUrl();
  state.pendingSelectionLink = null;
  if (link.edgeIndex != null) {
    const edge = findEdgeByIndex(link.edgeIndex);
    if (edge) {
      selectEdgeByKey(edgeSelectionKey(edge), { syncUrl: false });
    } else {
      await focusEdgeIndex(link.edgeIndex, { syncUrl: false });
    }
    return;
  }
  if (link.nodeId != null) {
    selectNodeById(link.nodeId, { syncUrl: false });
  }
}

function findEdgeByIndex(edgeIndex) {
  return (
    state.visibleEdges.find((edge) => edgeIndexOf(edge) === edgeIndex) ||
    state.graph.edges.find((edge) => edgeIndexOf(edge) === edgeIndex) ||
    null
  );
}

async function loadProjects() {
  try {
    const response = await apiFetch("/api/projects");
    const body = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "projects failed"));
    }
    state.projects = body;
    renderProjects();
  } catch (error) {
    state.projects = [];
    projectSelect.innerHTML = `<option value=".">${escapeHtml(t("project.currentRoot"))}</option>`;
  }
}
