// 10-refactor.js — refactoring reports, incremental cache panel, token handling, card exports.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

async function runRefactorReport(kind) {
  const target = refactorTargetInput.value.trim();
  const sourceArea = refactorSourceAreaInput.value.trim();
  const targetArea = refactorTargetAreaInput.value.trim();
  if ((kind === "impact" || kind === "dependencies" || kind === "context") && !target) {
    refactorResult.innerHTML = `<p class="empty">${escapeHtml(t("refactor.needTarget"))}</p>`;
    return;
  }
  if (kind === "contract" && (!sourceArea || !targetArea)) {
    refactorResult.innerHTML = `<p class="empty">${escapeHtml(t("refactor.needAreas"))}</p>`;
    return;
  }

  state.refactorRequest += 1;
  const requestId = state.refactorRequest;
  refactorStatus.textContent = t("status.loading");
  refactorResult.innerHTML = `<p class="empty">${escapeHtml(t("refactor.loading"))}</p>`;

  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  let endpoint = "";
  if (kind === "impact") {
    endpoint = "/api/impact";
    params.set("target", target);
  } else if (kind === "dependencies") {
    endpoint = "/api/component-dependencies";
    params.set("target", target);
  } else if (kind === "seams") {
    endpoint = "/api/seams";
  } else if (kind === "context") {
    endpoint = "/api/refactor-context";
    params.set("target", target);
  } else {
    endpoint = "/api/component-contract";
    params.set("source", sourceArea);
    params.set("target", targetArea);
  }

  try {
    const response = await apiFetch(`${endpoint}?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.refactorRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "refactor report failed"));
    }
    refactorStatus.textContent = formatKind(kind);
    if (kind === "context") {
      const blob = new Blob([JSON.stringify(body, null, 2)], { type: "application/json" });
      downloadBlob(blob, `${safeFilePart(target)}-refactor-context.json`);
      refactorResult.innerHTML = `<p>${escapeHtml(t("refactor.score"))}: ${Number(body.impact?.impact_score || 0)} · ${escapeHtml(t("refactor.dependents"))}: ${Number(body.impact?.total_dependents || 0)}</p>`;
    } else if (kind === "impact") {
      refactorResult.innerHTML = renderRefactorImpact(body);
    } else if (kind === "dependencies") {
      refactorResult.innerHTML = renderRefactorDependencies(body);
    } else if (kind === "seams") {
      refactorResult.innerHTML = renderRefactorSeams(body);
    } else {
      refactorResult.innerHTML = renderRefactorContract(body);
    }
  } catch (error) {
    if (requestId !== state.refactorRequest) return;
    refactorStatus.textContent = "error";
    refactorResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderRefactorImpact(report) {
  const summary = [
    `${escapeHtml(t("refactor.dependents"))}: ${Number(report.total_dependents || 0)}`,
    `${escapeHtml(t("refactor.entrypoints"))}: ${Array.isArray(report.affected_entrypoints) ? report.affected_entrypoints.length : 0}`,
    `${escapeHtml(t("refactor.tests"))}: ${Number(report.affected_tests || 0)}`,
    `${escapeHtml(t("refactor.routes"))}: ${Number(report.affected_routes || 0)}`,
    `${escapeHtml(t("refactor.score"))}: ${Number(report.impact_score || 0)}`,
  ];
  const parts = [
    `<p><code>${escapeHtml(report.target?.label || "")}</code>${report.area ? ` · ${escapeHtml(report.area)}` : ""}</p>`,
    `<p>${summary.join(" · ")}</p>`,
  ];
  const dependents = Array.isArray(report.dependents) ? report.dependents.slice(0, 8) : [];
  if (dependents.length) {
    const rows = dependents
      .map((entry) => `<li><code>${escapeHtml(entry.node?.label || "")}</code></li>`)
      .join("");
    parts.push(`<ul class="compact-list">${rows}</ul>`);
  }
  return parts.join("");
}

function renderRefactorDependencies(report) {
  const parts = [
    `<p><code>${escapeHtml(report.target?.label || "")}</code> · ${escapeHtml(t("refactor.incoming"))}: ${Number(report.total_incoming || 0)} · ${escapeHtml(t("refactor.outgoing"))}: ${Number(report.total_outgoing || 0)}</p>`,
  ];
  const group = (title, groups) => {
    const rows = (Array.isArray(groups) ? groups.slice(0, 6) : [])
      .map(
        (entry) =>
          `<li><code>${escapeHtml(entry.key || "")}</code> — ${Number(entry.incoming || 0)} / ${Number(entry.outgoing || 0)}</li>`,
      )
      .join("");
    if (rows) parts.push(`<p>${escapeHtml(title)}:</p><ul class="compact-list">${rows}</ul>`);
  };
  group(t("refactor.byArea"), report.areas);
  group(t("refactor.byPackage"), report.packages);
  return parts.join("");
}

function renderRefactorSeams(report) {
  const seamRows = (candidates) =>
    (Array.isArray(candidates) ? candidates.slice(0, 6) : [])
      .map(
        (seam) =>
          `<li><code>${escapeHtml(seam.source_area || "")}</code> → <code>${escapeHtml(seam.target_area || "")}</code> — ${Number(seam.edge_count || 0)} ${escapeHtml(t("refactor.edges"))}, ${escapeHtml(t("refactor.friction"))} ${Number(seam.friction_score || 0)}</li>`,
      )
      .join("");
  const parts = [];
  const safest = seamRows(report.safest);
  if (safest) parts.push(`<p>${escapeHtml(t("refactor.safest"))}:</p><ul class="compact-list">${safest}</ul>`);
  const needed = seamRows(report.most_needed);
  if (needed) parts.push(`<p>${escapeHtml(t("refactor.mostNeeded"))}:</p><ul class="compact-list">${needed}</ul>`);
  return parts.join("") || `<p class="empty">${escapeHtml(t("empty.noInsights"))}</p>`;
}

function renderRefactorContract(report) {
  const kinds = Object.entries(report.edge_kinds || {})
    .map(([kind, count]) => `${escapeHtml(kind)}: ${Number(count)}`)
    .join(" · ");
  const rows = (Array.isArray(report.edges) ? report.edges.slice(0, 8) : [])
    .map(
      (edge) =>
        `<li><code>${escapeHtml(edge.source_label || "")}</code> → <code>${escapeHtml(edge.target_label || "")}</code> (${escapeHtml(edge.edge?.kind || "")})</li>`,
    )
    .join("");
  const parts = [
    `<p><code>${escapeHtml(report.source_area || "")}</code> → <code>${escapeHtml(report.target_area || "")}</code> · ${escapeHtml(t("refactor.contractEdges"))}: ${Number(report.total_edges || 0)}</p>`,
  ];
  if (kinds) parts.push(`<p>${kinds}</p>`);
  if (rows) parts.push(`<ul class="compact-list">${rows}</ul>`);
  return parts.join("");
}

async function loadPrImpact() {
  state.prImpactRequest += 1;
  const requestId = state.prImpactRequest;
  prImpactButton.disabled = true;
  prImpactDownloadButton.disabled = true;
  state.prImpactReport = null;
  prImpactStatus.textContent = t("status.loading");
  prImpactResult.innerHTML = `<p class="empty">${escapeHtml(t("prImpact.loading"))}</p>`;

  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  const base = prImpactBaseInput.value.trim();
  if (base) params.set("base", base);
  const files = prImpactFilesInput.value.trim();
  if (files) params.set("files", files);

  try {
    const response = await apiFetch(`/api/pr-impact?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.prImpactRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "pr impact failed"));
    }
    state.prImpactReport = body;
    prImpactStatus.textContent = `${t("prImpact.riskScore")}: ${formatCompactNumber(body.risk_score || 0)}`;
    prImpactResult.innerHTML = renderPrImpact(body);
    prImpactDownloadButton.disabled = false;
  } catch (error) {
    if (requestId !== state.prImpactRequest) return;
    prImpactStatus.textContent = "error";
    prImpactResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.prImpactRequest) {
      prImpactButton.disabled = false;
    }
  }
}

function renderPrImpact(report) {
  const summary = [
    `${escapeHtml(t("prImpact.changed"))}: ${Number(report.total_changed_files || 0)}`,
    `${escapeHtml(t("prImpact.matched"))}: ${Number(report.matched_files || 0)}`,
    `${escapeHtml(t("prImpact.dependents"))}: ${Number(report.blast?.dependents || 0)}`,
    `${escapeHtml(t("prImpact.entrypoints"))}: ${Number(report.blast?.affected_entrypoints || 0)}`,
    `${escapeHtml(t("prImpact.tests"))}: ${Number(report.blast?.affected_tests || 0)}`,
    `${escapeHtml(t("prImpact.riskScore"))}: ${Number(report.risk_score || 0)}`,
  ];
  const meta = [report.branch, report.base, report.ci_state, report.review_state]
    .filter(Boolean)
    .map((value) => escapeHtml(String(value)))
    .join(" · ");
  const parts = [];
  if (meta) parts.push(`<p class="hint">${meta}</p>`);
  parts.push(`<p>${summary.join(" · ")}</p>`);

  const communities = Array.isArray(report.touched_communities)
    ? report.touched_communities.slice(0, 6)
    : [];
  if (communities.length) {
    const rows = communities
      .map(
        (community) =>
          `<li><code>${escapeHtml(community.id || "")}</code> — ${Number(community.changed_files || 0)}</li>`,
      )
      .join("");
    parts.push(
      `<p>${escapeHtml(t("prImpact.communities"))}:</p><ul class="compact-list">${rows}</ul>`,
    );
  }

  const risks = Array.isArray(report.risks) ? report.risks.slice(0, 10) : [];
  if (risks.length) {
    const rows = risks
      .map(
        (risk) =>
          `<li><span class="severity-${escapeHtml(risk.severity || "info")}">${escapeHtml(risk.severity || "")}</span> <code>${escapeHtml(risk.kind || "")}</code> ${escapeHtml(risk.message || "")}</li>`,
      )
      .join("");
    parts.push(`<p>${escapeHtml(t("prImpact.risks"))}:</p><ul class="compact-list">${rows}</ul>`);
  } else if (!Number(report.matched_files || 0)) {
    parts.push(`<p class="empty">${escapeHtml(t("prImpact.empty"))}</p>`);
  }
  return parts.join("");
}

async function loadCacheDiff() {
  const limit = clampNumber(Number(cacheDiffLimitInput.value || 50), 1, 10000);
  cacheDiffLimitInput.value = String(limit);
  state.cacheDiffRequest += 1;
  const requestId = state.cacheDiffRequest;
  cacheDiffButton.disabled = true;
  cacheDiffStatus.textContent = t("status.loading");
  cacheDiffResult.innerHTML = `<p class="empty">${escapeHtml(t("cache.loadingDiff"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await apiFetch(`/api/cache-diff?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.cacheDiffRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "cache diff failed"));
    }
    cacheDiffStatus.textContent = formatKind(body.cache_record || "unknown");
    cacheDiffResult.innerHTML = renderCacheDiff(body);
  } catch (error) {
    if (requestId !== state.cacheDiffRequest) return;
    cacheDiffStatus.textContent = "error";
    cacheDiffResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.cacheDiffRequest) {
      cacheDiffButton.disabled = false;
    }
  }
}

async function loadCacheChunks() {
  const limit = clampNumber(Number(cacheDiffLimitInput.value || 50), 1, 10000);
  cacheDiffLimitInput.value = String(limit);
  state.cacheChunksRequest += 1;
  const requestId = state.cacheChunksRequest;
  cacheChunksButton.disabled = true;
  cacheDiffStatus.textContent = t("status.chunks");
  cacheDiffResult.innerHTML = `<p class="empty">${escapeHtml(t("cache.loadingChunks"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await apiFetch(`/api/cache-chunks?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.cacheChunksRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "cache chunks failed"));
    }
    cacheDiffStatus.textContent = formatKind(body.cache_record || "unknown");
    cacheDiffResult.innerHTML = renderCacheChunks(body);
  } catch (error) {
    if (requestId !== state.cacheChunksRequest) return;
    cacheDiffStatus.textContent = "error";
    cacheDiffResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.cacheChunksRequest) {
      cacheChunksButton.disabled = false;
    }
  }
}

async function loadIncrementalPlan() {
  const limit = clampNumber(Number(cacheDiffLimitInput.value || 50), 1, 10000);
  cacheDiffLimitInput.value = String(limit);
  state.incrementalPlanRequest += 1;
  const requestId = state.incrementalPlanRequest;
  incrementalPlanButton.disabled = true;
  cacheDiffStatus.textContent = t("status.planning");
  cacheDiffResult.innerHTML = `<p class="empty">${escapeHtml(t("cache.planningIncremental"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await apiFetch(`/api/incremental-plan?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.incrementalPlanRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "incremental plan failed"));
    }
    cacheDiffStatus.textContent = formatKind(body.action || "unknown");
    cacheDiffResult.innerHTML = renderIncrementalPlan(body);
  } catch (error) {
    if (requestId !== state.incrementalPlanRequest) return;
    cacheDiffStatus.textContent = "error";
    cacheDiffResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.incrementalPlanRequest) {
      incrementalPlanButton.disabled = false;
    }
  }
}

async function loadIncrementalScan() {
  const limit = clampNumber(Number(cacheDiffLimitInput.value || 50), 1, 10000);
  cacheDiffLimitInput.value = String(limit);
  state.incrementalScanRequest += 1;
  const requestId = state.incrementalScanRequest;
  incrementalScanButton.disabled = true;
  cacheDiffStatus.textContent = t("status.scanning");
  cacheDiffResult.innerHTML = `<p class="empty">${escapeHtml(t("cache.scanningChanged"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await apiFetch(`/api/incremental-scan?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.incrementalScanRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "incremental scan failed"));
    }
    const plan = body.plan || {};
    cacheDiffStatus.textContent = formatKind(plan.action || "unknown");
    cacheDiffResult.innerHTML = renderIncrementalScan(body);
    showIncrementalScanGraph(body);
  } catch (error) {
    if (requestId !== state.incrementalScanRequest) return;
    cacheDiffStatus.textContent = "error";
    cacheDiffResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.incrementalScanRequest) {
      incrementalScanButton.disabled = false;
    }
  }
}

async function loadIncrementalMergePreview() {
  const limit = clampNumber(Number(cacheDiffLimitInput.value || 50), 1, 10000);
  cacheDiffLimitInput.value = String(limit);
  state.incrementalMergeRequest += 1;
  const requestId = state.incrementalMergeRequest;
  incrementalMergeButton.disabled = true;
  cacheDiffStatus.textContent = t("status.merging");
  cacheDiffResult.innerHTML = `<p class="empty">${escapeHtml(t("cache.buildingMerge"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await apiFetch(`/api/incremental-merge-preview?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.incrementalMergeRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "incremental merge preview failed"));
    }
    const plan = body.plan || {};
    cacheDiffStatus.textContent = formatKind(plan.action || "unknown");
    cacheDiffResult.innerHTML = renderIncrementalMergePreview(body);
    showIncrementalMergePreviewGraph(body);
  } catch (error) {
    if (requestId !== state.incrementalMergeRequest) return;
    cacheDiffStatus.textContent = "error";
    cacheDiffResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.incrementalMergeRequest) {
      incrementalMergeButton.disabled = false;
    }
  }
}

async function loadIncrementalUpdate() {
  const limit = clampNumber(Number(cacheDiffLimitInput.value || 50), 1, 10000);
  cacheDiffLimitInput.value = String(limit);
  state.incrementalUpdateRequest += 1;
  const requestId = state.incrementalUpdateRequest;
  incrementalUpdateButton.disabled = true;
  cacheDiffStatus.textContent = t("status.updating");
  cacheDiffResult.innerHTML = `<p class="empty">${escapeHtml(t("cache.updating"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await apiFetch(`/api/incremental-update?${params.toString()}`, { method: "POST" });
    const body = await response.json();
    if (requestId !== state.incrementalUpdateRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "incremental update failed"));
    }
    const plan = body.preview?.plan || {};
    cacheDiffStatus.textContent = body.cache?.stored
      ? t("status.stored")
      : plan.action
        ? formatKind(plan.action)
        : t("status.skipped");
    cacheDiffResult.innerHTML = renderIncrementalUpdate(body);
    if (body.preview?.graph) {
      showIncrementalMergePreviewGraph(body.preview);
    }
  } catch (error) {
    if (requestId !== state.incrementalUpdateRequest) return;
    cacheDiffStatus.textContent = "error";
    cacheDiffResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.incrementalUpdateRequest) {
      incrementalUpdateButton.disabled = false;
    }
  }
}

async function runGraphExport() {
  const metadata = exportFormatMetadata(exportFormatInput.value);
  exportFormatInput.value = metadata.format;
  state.exportRequest += 1;
  const requestId = state.exportRequest;
  exportButton.disabled = true;
  exportResult.innerHTML = `<p class="empty">${escapeHtml(t("export.exporting"))}</p>`;

  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  if (metadata.endpoint === "/api/export") {
    params.set("format", metadata.format);
  } else if (metadata.reportFormat) {
    params.set("format", metadata.reportFormat);
  }

  try {
    const response = await apiFetch(`${metadata.endpoint}?${params.toString()}`);
    if (requestId !== state.exportRequest) return;
    if (!response.ok) {
      throw new Error(await responseErrorMessage(response, t("export.failedFallback")));
    }

    const blob = await response.blob();
    if (requestId !== state.exportRequest) return;
    const fileName = `codegraph-${safeFilePart(pathInput.value.trim() || state.graphPage.root || "project")}.${metadata.extension}`;
    const exportNodes = response.headers.get("x-codegraph-export-nodes") || "";
    const exportEdges = response.headers.get("x-codegraph-export-edges") || "";
    const exportBytes = response.headers.get("x-codegraph-export-bytes") || "";
    downloadBlob(blob, fileName);
    exportResult.innerHTML = `
      <div class="query-summary">
        <span>${escapeHtml(metadata.label)}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        ${exportNodes ? `<span>${escapeHtml(exportNodes)} ${escapeHtml(t("stat.nodes").toLowerCase())}</span>` : ""}
        ${exportEdges ? `<span>${escapeHtml(exportEdges)} ${escapeHtml(t("stat.edges").toLowerCase())}</span>` : ""}
        ${exportBytes && Number(exportBytes) !== blob.size ? `<span>${escapeHtml(formatBytes(Number(exportBytes)))}</span>` : ""}
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `;
  } catch (error) {
    if (requestId !== state.exportRequest) return;
    exportResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.exportRequest) {
      exportButton.disabled = false;
    }
  }
}

function exportVisibleGraphSlice() {
  if (state.visibleNodes.length === 0 && state.visibleEdges.length === 0) {
    exportResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noSlice"))}</p>`;
    return;
  }

  const visibleNodeIds = new Set(state.visibleNodes.map((node) => node.id));
  const edges = state.visibleEdges.filter(
    (edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target),
  );
  const slice = {
    schema: "codegraph.visible_slice.v1",
    generated_at: new Date().toISOString(),
    root: state.graphPage.root || pathInput.value.trim() || ".",
    nodes: state.visibleNodes,
    edges,
    counts: {
      nodes: state.visibleNodes.length,
      edges: edges.length,
      loaded_nodes: state.graph.nodes.length,
      loaded_edges: state.graph.edges.length,
      total_nodes: state.graphPage.totalNodes,
      total_edges: state.graphPage.totalEdges,
      truncated_nodes: Boolean(state.graphPage.truncatedNodes),
      truncated_edges: Boolean(state.graphPage.truncatedEdges),
    },
    graph_page: {
      node_offset: state.graphPage.nodeOffset,
      node_limit: state.graphPage.nodeLimit,
      edge_offset: state.graphPage.edgeOffset,
      edge_limit: state.graphPage.edgeLimit,
      path_prefix: state.graphPage.pathPrefix,
    },
    server_filters: {
      kind: serverKindInput.value.trim(),
      item_kind: serverItemKindInput.value.trim(),
      language: serverLanguageInput.value.trim(),
      search: serverSearchInput.value.trim(),
      edge_kind: serverEdgeKindInput.value.trim(),
      confidence: serverConfidenceInput.value.trim(),
      relation: serverEdgeRelationInput.value.trim(),
      source: serverEdgeSourceInput.value.trim(),
    },
    canvas_filters: {
      search: state.search,
      enabled_kinds: [...state.enabledKinds].sort(),
      active_risk_severity: state.activeRiskSeverity,
      query_focus: Boolean(state.queryFocus),
    },
    viewport: {
      zoom: state.zoom,
      pan: state.pan,
      label_mode: state.labelMode,
      layout_paused: state.layoutPaused,
    },
    layout: {
      positions: Object.fromEntries(
        state.visibleNodes
          .map((node) => [node.id, state.positions.get(node.id)])
          .filter(([, position]) => Boolean(position)),
      ),
    },
  };
  const serialized = JSON.stringify(slice, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(slice.root)}-visible-slice.json`;
  downloadBlob(blob, fileName);
  exportResult.innerHTML = `
    <div class="query-summary">
      <span>${escapeHtml(t("export.slice"))}</span>
      <span>${escapeHtml(formatBytes(blob.size))}</span>
      <span>${escapeHtml(formatNumber(slice.counts.nodes))} nodes</span>
      <span>${escapeHtml(formatNumber(slice.counts.edges))} edges</span>
      <span class="query-expression">${escapeHtml(fileName)}</span>
    </div>
  `;
}

async function responseErrorMessage(response, fallback) {
  const contentType = response.headers.get("content-type") || "";
  if (contentType.includes("application/json")) {
    try {
      const body = await response.json();
      return apiErrorMessage(body, response, fallback);
    } catch (error) {
      return apiErrorMessage(null, response, fallback);
    }
  }
  const text = await response.text();
  return apiErrorMessage(null, response, text.trim() || fallback);
}

async function apiFetch(input, init = {}) {
  let response = await window.fetch(input, withApiAuth(init));
  if (response.status !== 401 || !isApiRequest(input)) {
    recordApiResponse(input, response);
    return response;
  }

  const token = requestApiToken();
  if (!token) {
    recordApiResponse(input, response);
    return response;
  }
  response = await window.fetch(input, withApiAuth(init));
  recordApiResponse(input, response);
  return response;
}

function recordApiResponse(input, response) {
  if (!isApiRequest(input)) return;
  const elapsedHeader = response.headers.get("x-response-time-ms");
  const elapsedMs = elapsedHeader == null ? null : Number(elapsedHeader);
  state.lastApiResponse = {
    elapsedMs: Number.isFinite(elapsedMs) ? elapsedMs : null,
    requestId: response.headers.get("x-request-id") || "",
    path: apiRequestPath(input),
    status: response.status,
  };
  if (state.metrics) renderRuntimeMetrics();
}

function apiRequestPath(input) {
  const url = typeof input === "string" ? input : input?.url || "";
  try {
    const parsed = new URL(url, window.location.href);
    return `${parsed.pathname}${parsed.search}`;
  } catch (error) {
    return String(url || "");
  }
}

function withApiAuth(init = {}) {
  const headers = new Headers(init.headers || {});
  const token = storedApiToken();
  if (token && !headers.has("authorization") && !headers.has("x-codegraph-token")) {
    headers.set("authorization", `Bearer ${token}`);
  }
  return { ...init, headers };
}

function isApiRequest(input) {
  const url = typeof input === "string" ? input : input?.url || "";
  if (url.startsWith("/api/")) return true;
  try {
    const parsed = new URL(url, window.location.href);
    return parsed.origin === window.location.origin && parsed.pathname.startsWith("/api/");
  } catch (error) {
    return false;
  }
}

function requestApiToken() {
  const token = window.prompt(t("auth.prompt"), storedApiToken() || "");
  if (!token) return null;
  storeApiToken(token);
  return token;
}

function storedApiToken() {
  try {
    return window.localStorage?.getItem(API_TOKEN_STORAGE_KEY) || "";
  } catch (error) {
    return "";
  }
}

function storeApiToken(token) {
  try {
    window.localStorage?.setItem(API_TOKEN_STORAGE_KEY, token);
  } catch (error) {
    // Local storage can be disabled; the cookie still covers same-origin requests.
  }
  syncApiTokenCookie(token);
}

function syncApiTokenCookie(token = storedApiToken()) {
  if (!token) return;
  document.cookie = `codegraph_api_token=${encodeURIComponent(token)}; path=/; SameSite=Strict`;
}

function authenticatedEventSource(url) {
  syncApiTokenCookie();
  return new EventSource(url);
}

function apiErrorMessage(body, response, fallback) {
  const message = body?.error || fallback;
  const requestId = body?.request_id || response?.headers?.get?.("x-request-id") || "";
  return requestId ? `${message} [request ${requestId}]` : message;
}

function exportFormatMetadata(format) {
  switch (format) {
    case "dot":
      return { format: "dot", extension: "dot", label: "DOT", endpoint: "/api/export" };
    case "ndjson":
      return { format: "ndjson", extension: "ndjson", label: "NDJSON", endpoint: "/api/export" };
    case "graphml":
      return { format: "graphml", extension: "graphml", label: "GraphML", endpoint: "/api/export" };
    case "mermaid_html":
      return { format: "mermaid_html", extension: "html", label: "Mermaid HTML", endpoint: "/api/export" };
    case "cypher":
      return { format: "cypher", extension: "cypher", label: "Neo4j Cypher", endpoint: "/api/export" };
    case "falkordb":
      return { format: "falkordb", extension: "falkordb", label: "FalkorDB", endpoint: "/api/export" };
    case "report":
      return { format: "report", extension: "report.json", label: t("export.report"), endpoint: "/api/report" };
    case "reportMarkdown":
      return { format: "reportMarkdown", extension: "report.md", label: t("export.reportMarkdown"), endpoint: "/api/report", reportFormat: "markdown" };
    case "json":
    default:
      return { format: "json", extension: "json", label: "JSON", endpoint: "/api/export" };
  }
}

function downloadBlob(blob, fileName) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function safeFilePart(value) {
  return String(value)
    .trim()
    .replace(/[/\\:*?"<>|]+/g, "-")
    .replace(/\s+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80) || "project";
}

function selectionCardRoot() {
  return pathInput.value.trim() || state.graphPage.root || ".";
}

function setLastSelectionCard(card) {
  state.lastSelectionCard = card;
}

function clearLastSelectionCard() {
  state.lastSelectionCard = null;
}

function buildNodeSelectionCard(node, edges, context, card) {
  return {
    generated_at: new Date().toISOString(),
    root: selectionCardRoot(),
    card_type: "node",
    selection: {
      node,
      context: context || null,
      fallback_edges: context ? [] : edges,
      card: card || null,
    },
  };
}

function buildEdgeSelectionCard(edge, source, target) {
  return {
    generated_at: new Date().toISOString(),
    root: selectionCardRoot(),
    card_type: "edge",
    selection: {
      edge_index: edgeIndexOf(edge),
      edge,
      source: source || null,
      target: target || null,
    },
  };
}

function renderSelectionCardExportNote(fileName, size) {
  selectionBody.querySelector("[data-selection-card-export-note]")?.remove();
  selectionBody
    .querySelector(".selection-actions")
    ?.insertAdjacentHTML(
      "afterend",
      `<div class="query-summary" data-selection-card-export-note>
        <span>${escapeHtml(t("export.selectionCard"))}</span>
        <span>${escapeHtml(formatBytes(size))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>`,
    );
}

function exportLastSelectionCard() {
  if (!state.lastSelectionCard) {
    selectionBody.querySelector("[data-selection-card-export-note]")?.remove();
    selectionBody.insertAdjacentHTML(
      "afterbegin",
      `<p class="empty" data-selection-card-export-note>${escapeHtml(t("export.noSelectionCard"))}</p>`,
    );
    return;
  }
  const payload = {
    schema: "codegraph.selection_card.v1",
    ...state.lastSelectionCard,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const label =
    payload.card_type === "edge"
      ? `edge-${payload.selection?.edge_index ?? "selected"}`
      : payload.selection?.node?.label || "selection";
  const fileName = `codegraph-${safeFilePart(payload.root)}-${safeFilePart(label)}-card.json`;
  downloadBlob(blob, fileName);
  renderSelectionCardExportNote(fileName, blob.size);
}

function attachSelectionCardExportAction() {
  selectionBody.querySelectorAll("[data-export-selection-card]").forEach((button) => {
    button.addEventListener("click", () => exportLastSelectionCard());
  });
}

function renderCacheDiff(report) {
  const added = report.added || [];
  const modified = report.modified || [];
  const removed = report.removed || [];
  const previousHash = report.previous_hash || t("cache.noFingerprint");
  const changedCount = added.length + modified.length + removed.length;
  const totalChanged = Number(report.changed_files ?? changedCount);
  const reusableFiles = Number(report.reusable_files ?? report.unchanged ?? 0);
  const currentFiles = Number(report.current_files ?? 0);
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(formatKind(report.cache_record || "unknown"))}</span>
      <span>${escapeHtml(formatKind(report.reuse_strategy || "unknown"))}</span>
      <span>${report.previous_files ?? 0} -> ${report.current_files ?? 0} ${escapeHtml(t("cache.files"))}</span>
      <span>${formatBytes(report.previous_bytes)} -> ${formatBytes(report.current_bytes)}</span>
      <span>${totalChanged} ${escapeHtml(t("cache.changed"))}</span>
      <span>${reusableFiles}/${currentFiles} ${escapeHtml(t("cache.reusable"))}</span>
      <span>${formatBasisPoints(report.reuse_file_ratio_basis_points)} ${escapeHtml(t("cache.fileReuse"))}</span>
      <span>${formatBasisPoints(report.reuse_byte_ratio_basis_points)} ${escapeHtml(t("cache.byteReuse"))}</span>
      <span>${formatBytes(report.changed_current_bytes)} ${escapeHtml(t("cache.changedCurrent"))}</span>
      <span>${formatBytes(report.reusable_bytes)} ${escapeHtml(t("cache.reusable"))}</span>
      <span>${changedCount} ${escapeHtml(t("cache.listed"))}</span>
      ${report.truncated ? `<span>${escapeHtml(t("cache.truncated"))}</span>` : ""}
      <span class="query-expression">${escapeHtml(t("cache.previous"))} ${escapeHtml(previousHash)}</span>
      <span class="query-expression">${escapeHtml(t("cache.current"))} ${escapeHtml(report.current_hash || "unknown")}</span>
    </div>
  `;

  const groups = [
    renderCacheDiffGroup(t("cache.addedGroup"), added, renderCacheDiffEntry),
    renderCacheDiffGroup(t("cache.modifiedGroup"), modified, renderCacheDiffChange),
    renderCacheDiffGroup(t("cache.removedGroup"), removed, renderCacheDiffEntry),
  ]
    .filter(Boolean)
    .join("");

  if (!groups) {
    return `${summary}<p class="empty">${escapeHtml(t("cache.noChanges"))}</p>`;
  }

  return `${summary}${groups}`;
}

function renderCacheChunks(report) {
  const chunks = report.chunks || [];
  const previousHash = report.previous_hash || t("cache.noFingerprint");
  const listed = chunks.length;
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(formatKind(report.cache_record || "unknown"))}</span>
      <span>${Number(report.total_chunks || 0)} ${escapeHtml(t("cache.chunksGroup").toLowerCase())}</span>
      <span>${Number(report.total_chunk_nodes || 0)} ${escapeHtml(t("cache.nodes"))}</span>
      <span>${Number(report.total_chunk_edges || 0)} ${escapeHtml(t("cache.edges"))}</span>
      <span>${listed}/${Number(report.total_chunks || 0)} ${escapeHtml(t("cache.listed"))}</span>
      <span>${report.previous_files ?? 0} -> ${report.current_files ?? 0} ${escapeHtml(t("cache.files"))}</span>
      ${report.truncated ? `<span>${escapeHtml(t("cache.truncated"))}</span>` : ""}
      <span class="query-expression">${escapeHtml(t("cache.previous"))} ${escapeHtml(previousHash)}</span>
      <span class="query-expression">${escapeHtml(t("cache.current"))} ${escapeHtml(report.current_hash || "unknown")}</span>
    </div>
  `;
  const groups = renderCacheDiffGroup(t("cache.chunksGroup"), chunks, renderCacheChunkEntry);
  if (!groups) {
    return `${summary}<p class="empty">${escapeHtml(t("cache.noChunks"))}</p>`;
  }
  return `${summary}${groups}`;
}

function renderIncrementalPlan(plan) {
  const scanPaths = plan.scan_paths || [];
  const removedPaths = plan.removed_paths || [];
  const reusablePaths = plan.reusable_paths || [];
  const impactedNodeIds = plan.impacted_node_ids || [];
  const impactedEdgeIndexes = plan.impacted_edge_indexes || [];
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(formatKind(plan.action || "unknown"))}</span>
      <span>${escapeHtml(formatKind(plan.cache_record || "unknown"))}</span>
      <span>${Number(plan.changed_files || 0)} ${escapeHtml(t("cache.changed"))}</span>
      <span>${Number(plan.rescan_files || 0)} ${escapeHtml(t("cache.rescan"))}</span>
      <span>${Number(plan.removed_files || 0)} ${escapeHtml(t("cache.removed"))}</span>
      <span>${Number(plan.reusable_files || 0)} ${escapeHtml(t("cache.reusable"))}</span>
      <span>${Number(plan.impacted_nodes || 0)} ${escapeHtml(t("cache.graphNodes"))}</span>
      <span>${Number(plan.impacted_edges || 0)} ${escapeHtml(t("cache.graphEdges"))}</span>
      <span>${formatBasisPoints(plan.reuse_file_ratio_basis_points)} ${escapeHtml(t("cache.fileReuse"))}</span>
      <span>${formatBasisPoints(plan.reuse_byte_ratio_basis_points)} ${escapeHtml(t("cache.byteReuse"))}</span>
      <span>${formatBytes(plan.changed_current_bytes)} ${escapeHtml(t("cache.changedCurrent"))}</span>
      <span>${formatBytes(plan.reusable_bytes)} ${escapeHtml(t("cache.reusable"))}</span>
      ${plan.truncated ? `<span>${escapeHtml(t("cache.truncated"))}</span>` : ""}
      <span class="query-expression">${escapeHtml(plan.reason || "")}</span>
    </div>
  `;
  const groups = [
    renderCacheDiffGroup(t("cache.scanGroup"), scanPaths, renderPlanPath),
    renderCacheDiffGroup(t("cache.removedGroup"), removedPaths, renderPlanPath),
    renderCacheDiffGroup(t("cache.reusableGroup"), reusablePaths, renderPlanPath),
    renderCacheDiffGroup(t("cache.nodeIdsGroup"), impactedNodeIds, renderPlanScalar),
    renderCacheDiffGroup(t("cache.edgeIndexesGroup"), impactedEdgeIndexes, renderPlanScalar),
  ]
    .filter(Boolean)
    .join("");

  if (!groups) {
    return `${summary}<p class="empty">${escapeHtml(t("cache.noIncrementalWork"))}</p>`;
  }

  return `${summary}${groups}`;
}

function renderIncrementalScan(scan) {
  const graph = scan.graph || { nodes: [], edges: [] };
  const plan = scan.plan || {};
  const graphSummary = `
    <div class="query-summary">
      <span>${Number(graph.nodes?.length || 0)} ${escapeHtml(t("cache.scannedNodes"))}</span>
      <span>${Number(graph.edges?.length || 0)} ${escapeHtml(t("cache.scannedEdges"))}</span>
      <span>${Number(plan.scan_paths?.length || 0)} ${escapeHtml(t("cache.listed"))} ${escapeHtml(t("cache.path"))}</span>
      ${plan.truncated ? `<span>${escapeHtml(t("cache.limitedScope"))}</span>` : ""}
    </div>
  `;
  return `${graphSummary}${renderIncrementalPlan(plan)}`;
}

function renderIncrementalMergePreview(preview) {
  const graph = preview.graph || { nodes: [], edges: [] };
  const plan = preview.plan || {};
  const merge = preview.merge || {};
  const blockers = merge.completeness_blockers || [];
  const warning = merge.warning
    ? `<p class="empty">${escapeHtml(localizedMergeWarning(merge))}</p>`
    : "";
  const blockerGroup = renderCacheDiffGroup(t("cache.completenessBlockersGroup"), blockers, renderMergeBlocker);
  const graphSummary = `
    <div class="query-summary">
      <span>${Number(graph.nodes?.length || 0)} ${escapeHtml(t("cache.previewNodes"))}</span>
      <span>${Number(graph.edges?.length || 0)} ${escapeHtml(t("cache.previewEdges"))}</span>
      <span>${Number(merge.reused_nodes || 0)} ${escapeHtml(t("cache.reusedNodes"))}</span>
      <span>${Number(merge.reused_edges || 0)} ${escapeHtml(t("cache.reusedEdges"))}</span>
      <span>${Number(merge.removed_cached_nodes || 0)} ${escapeHtml(t("cache.removedCachedNodes"))}</span>
      <span>${Number(merge.removed_cached_edges || 0)} ${escapeHtml(t("cache.removedCachedEdges"))}</span>
      <span>${Number(merge.chunk_removed_nodes || 0)} ${escapeHtml(t("cache.chunkNodes"))}</span>
      <span>${Number(merge.chunk_removed_edges || 0)} ${escapeHtml(t("cache.chunkEdges"))}</span>
      <span>${Number(merge.scanned_nodes || 0)} ${escapeHtml(t("cache.scannedNodes"))}</span>
      <span>${Number(merge.scanned_edges || 0)} ${escapeHtml(t("cache.scannedEdges"))}</span>
      <span>${Number(merge.replaced_paths || 0)} ${escapeHtml(t("cache.replacedPaths"))}</span>
      <span>${Number(merge.incoming_cross_file_edges || 0)} ${escapeHtml(t("cache.incomingBlockers"))}</span>
      <span>${Number(merge.graph_surface_added || 0)} ${escapeHtml(t("cache.surfaceAdded"))}</span>
      <span>${Number(merge.graph_surface_removed || 0)} ${escapeHtml(t("cache.surfaceRemoved"))}</span>
      <span>${Number(merge.removed_paths_blocking || 0)} ${escapeHtml(t("cache.removedPaths"))}</span>
      <span>${Number(blockers.length || 0)} ${escapeHtml(t("cache.blockers"))}</span>
      <span>${merge.complete_graph ? escapeHtml(t("cache.complete")) : escapeHtml(t("cache.preview"))}</span>
    </div>
  `;
  return `${graphSummary}${warning}${blockerGroup}${renderIncrementalPlan(plan)}`;
}

function renderIncrementalUpdate(update) {
  const cache = update.cache || {};
  const status = cache.stored ? t("cache.stored") : t("cache.notStored");
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(status)}</span>
      <span>${escapeHtml(localizedIncrementalUpdateReason(update))}</span>
      <span class="query-expression">${escapeHtml(t("cache.previous"))} ${escapeHtml(cache.previous_hash || t("cache.none"))}</span>
      <span class="query-expression">${escapeHtml(t("cache.current"))} ${escapeHtml(cache.current_hash || "unknown")}</span>
    </div>
  `;
  return `${summary}${renderIncrementalMergePreview(update.preview || {})}`;
}

function showIncrementalScanGraph(scan) {
  const graph = scan.graph || { nodes: [], edges: [] };
  const plan = scan.plan || {};
  state.graph = { nodes: graph.nodes || [], edges: graph.edges || [] };
  state.graphPage.nodeOffset = 0;
  state.graphPage.edgeOffset = 0;
  state.graphPage.totalNodes = state.graph.nodes.length;
  state.graphPage.totalEdges = state.graph.edges.length;
  state.graphPage.truncatedNodes = false;
  state.graphPage.truncatedEdges = false;
  clearSelection({ render: false });
  state.hoveredId = null;
  state.hoveredEdgeKey = null;
  state.queryFocus = null;
  rootLabel.textContent = t("cache.changedGraph", { action: formatKind(plan.action || "scan") });
  initializeGraph({ preserveView: false });
  pageInfo.textContent = t("cache.changedPageInfo", {
    nodes: state.graph.nodes.length,
    files: Number(plan.rescan_files || 0),
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
}

function showIncrementalMergePreviewGraph(preview) {
  const graph = preview.graph || { nodes: [], edges: [] };
  const merge = preview.merge || {};
  state.graph = { nodes: graph.nodes || [], edges: graph.edges || [] };
  state.graphPage.nodeOffset = 0;
  state.graphPage.edgeOffset = 0;
  state.graphPage.totalNodes = state.graph.nodes.length;
  state.graphPage.totalEdges = state.graph.edges.length;
  state.graphPage.truncatedNodes = false;
  state.graphPage.truncatedEdges = false;
  clearSelection({ render: false });
  state.hoveredId = null;
  state.hoveredEdgeKey = null;
  state.queryFocus = null;
  rootLabel.textContent = merge.complete_graph ? t("cache.incrementalComplete") : t("cache.incrementalPreview");
  initializeGraph({ preserveView: false });
  pageInfo.textContent = t("cache.previewPageInfo", {
    nodes: state.graph.nodes.length,
    reused: Number(merge.reused_nodes || 0),
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
}

function renderPlanPath(path) {
  return `
    <div class="query-item cache-diff-item">
      <span>${escapeHtml(t("cache.path"))}</span>
      <strong>${escapeHtml(path || "")}</strong>
    </div>
  `;
}

function renderPlanScalar(value) {
  return `
    <div class="query-item cache-diff-item">
      <span>${escapeHtml(t("cache.id"))}</span>
      <strong>${escapeHtml(String(value ?? ""))}</strong>
    </div>
  `;
}
