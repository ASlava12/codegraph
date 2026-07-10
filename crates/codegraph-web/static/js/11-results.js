// 11-results.js — path/trace/query result rendering and workflow-query actions.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function renderMergeBlocker(blocker) {
  return `
    <div class="query-item cache-diff-item">
      <span>${Number(blocker.count || 0)}</span>
      <strong>${escapeHtml(formatKind(blocker.kind || "blocker"))}</strong>
      <span>${escapeHtml(localizedMergeBlockerMessage(blocker))}</span>
    </div>
  `;
}

function localizedMergeWarning(merge) {
  const blockers = merge.completeness_blockers || [];
  return blockers.length ? localizedMergeBlockerMessage(blockers[0]) : merge.warning || "";
}

function localizedMergeBlockerMessage(blocker) {
  const key = `cache.blocker.${blocker?.kind || ""}`;
  const fallback = blocker?.message || "";
  return translate(key, fallback, { count: Number(blocker?.count || 0) });
}

function localizedIncrementalUpdateReason(update) {
  const cache = update?.cache || {};
  if (cache.stored) return t("cache.updateStored");
  const blockers = update?.preview?.merge?.completeness_blockers || [];
  if (blockers.length) return localizedMergeBlockerMessage(blockers[0]);
  switch (cache.reason) {
    case "cache already matches the current project fingerprint":
      return t("cache.updateAlreadyCurrent");
    case "stored complete graph under the current project fingerprint":
      return t("cache.updateStored");
    case "incremental merge is incomplete; cache record was not updated":
      return t("cache.updateIncomplete");
    default:
      return cache.reason || "";
  }
}

function renderCacheChunkEntry(chunk) {
  const nodePreview = (chunk.node_ids || []).slice(0, 6).join(", ");
  const edgePreview = (chunk.edge_indexes || []).slice(0, 6).join(", ");
  const preview = [nodePreview && `n ${nodePreview}`, edgePreview && `e ${edgePreview}`]
    .filter(Boolean)
    .join(" | ");
  return `
    <div class="query-item cache-diff-item">
      <span>${Number(chunk.nodes || 0)} ${escapeHtml(t("cache.nodes"))} / ${Number(chunk.edges || 0)} ${escapeHtml(t("cache.edges"))}</span>
      <strong>${escapeHtml(chunk.path || "")}</strong>
      ${preview ? `<span>${escapeHtml(preview)}</span>` : ""}
    </div>
  `;
}

function formatBasisPoints(value) {
  const points = Number(value);
  if (!Number.isFinite(points)) return "0%";
  return `${(Math.max(0, Math.min(10000, points)) / 100).toFixed(1)}%`;
}

function renderCacheDiffGroup(label, items, renderItem) {
  if (!items.length) return "";
  const rows = items.map((item) => `<li>${renderItem(item)}</li>`).join("");
  return `
    <section class="cache-diff-group">
      <h3>${escapeHtml(label)}</h3>
      <ul class="query-list">${rows}</ul>
    </section>
  `;
}

function graphQueryWithinClientLimit(expression, target) {
  const limit = Number(state.capabilities?.limits?.max_graph_query_length || 0);
  if (limit <= 0 || expression.length <= limit) return true;
  target.innerHTML = `<p class="error-text">${escapeHtml(t("query.tooLong", {
    count: formatNumber(expression.length),
    limit: formatNumber(limit),
  }))}</p>`;
  return false;
}

function renderCacheDiffEntry(entry) {
  return `
    <div class="query-item cache-diff-item">
      <span>${formatBytes(entry.bytes)}</span>
      <strong>${escapeHtml(entry.path || "")}</strong>
    </div>
  `;
}

function renderCacheDiffChange(change) {
  return `
    <div class="query-item cache-diff-item">
      <span>${formatBytes(change.previous_bytes)} -> ${formatBytes(change.current_bytes)}</span>
      <strong>${escapeHtml(change.path || "")}</strong>
    </div>
  `;
}

async function runPathQuery() {
  const from = pathFromInput.value.trim();
  const to = pathToInput.value.trim();
  if (!from || !to) {
    pathResult.innerHTML = `<p class="empty">${escapeHtml(t("path.enterEndpoints"))}</p>`;
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
  if (!graphQueryWithinClientLimit(expression, pathResult)) {
    clearLastPathResult();
    return;
  }

  state.pathRequest += 1;
  const requestId = state.pathRequest;
  pathButton.disabled = true;
  pathResult.innerHTML = `<p class="empty">${escapeHtml(t("path.finding"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: expression,
  });

  try {
    const response = await apiFetch(`/api/query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.pathRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("path.failedFallback")));
    }
    pathResult.innerHTML = renderQueryResult(body, { label: t("path.resultLabel") });
    state.lastPathResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      from,
      to,
      depth,
      edge_kind: edgeKind || null,
      query: expression,
      result: body,
    };
    renderPathExportState();
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

function renderPathExportState() {
  pathExportButton.disabled = !state.lastPathResult;
}

function clearLastPathResult() {
  state.lastPathResult = null;
  renderPathExportState();
}

function exportLastPathResult() {
  if (!state.lastPathResult) {
    pathResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noPathResult"))}</p>`;
    renderPathExportState();
    return;
  }

  const payload = {
    schema: "codegraph.path_result.v1",
    ...state.lastPathResult,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-path-result.json`;
  downloadBlob(blob, fileName);
  pathResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.pathResult"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(formatNumber(payload.result?.returned_nodes ?? payload.result?.nodes?.length ?? 0))} nodes</span>
        <span>${escapeHtml(formatNumber(payload.result?.returned_edges ?? payload.result?.edges?.length ?? 0))} edges</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

async function runConfigTrace() {
  const target = configTraceTargetInput.value.trim();
  if (!target) {
    configTraceResult.innerHTML = `<p class="empty">${escapeHtml(t("configTrace.enterTarget"))}</p>`;
    return;
  }

  const depth = clampNumber(Number(configTraceDepthInput.value || 6), 1, 32);
  configTraceDepthInput.value = String(depth);
  state.configTraceRequest += 1;
  const requestId = state.configTraceRequest;
  configTraceButton.disabled = true;
  clearLastConfigTraceReport();
  configTraceResult.innerHTML = `<p class="empty">${escapeHtml(t("configTrace.tracing"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    target,
    depth: String(depth),
    limit: "50",
  });

  try {
    const response = await apiFetch(`/api/trace-config?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.configTraceRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("configTrace.failedFallback")));
    }
    state.lastConfigTraceReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        target,
        depth,
        limit: 50,
      },
      report: body,
    };
    renderConfigTraceExportState();
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

function renderConfigTraceExportState() {
  configTraceExportButton.disabled = !state.lastConfigTraceReport;
}

function clearLastConfigTraceReport() {
  state.lastConfigTraceReport = null;
  renderConfigTraceExportState();
}

function exportLastConfigTraceReport() {
  if (!state.lastConfigTraceReport) {
    configTraceResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noConfigTrace"))}</p>`;
    renderConfigTraceExportState();
    return;
  }

  const payload = {
    schema: "codegraph.config_trace.v1",
    ...state.lastConfigTraceReport,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-config-trace.json`;
  downloadBlob(blob, fileName);
  configTraceResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.configTrace"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(t("configTrace.targetCount", { count: formatNumber(payload.report?.total_matches ?? payload.report?.matches?.length ?? 0) }))}</span>
        <span>${escapeHtml(t("trace.pathCount", { count: formatNumber(payload.report?.total_paths ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function renderConfigTrace(result) {
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("configTrace.targetCount", { count: formatNumber(result.total_matches || 0) }))}</span>
      <span>${escapeHtml(t("configTrace.readerCount", { count: formatNumber(result.total_readers || 0) }))}</span>
      <span>${escapeHtml(t("trace.pathCount", { count: formatNumber(result.total_paths || 0) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: result.max_depth }))}</span>
      <span class="query-expression">${escapeHtml(result.target)}</span>
    </div>
  `;

  if (!result.matches.length) {
    return `${summary}<p class="empty">${escapeHtml(t("configTrace.noMatches"))}</p>`;
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
      const truncated = match.truncated
        ? `<p class="empty">${escapeHtml(t("trace.traceTruncated"))}</p>`
        : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(match.target.label)}</h3>
          <div class="trace-summary">
            <span>${escapeHtml(t("configTrace.readerCount", { count: formatNumber(match.total_readers || 0) }))}</span>
            <span>${escapeHtml(t("trace.pathCount", { count: formatNumber(match.total_paths || 0) }))}</span>
            <span>${escapeHtml(formatKind(match.target.kind))}</span>
          </div>
          ${readers ? `<ul class="trace-list">${readers}</ul>` : `<p class="empty">${escapeHtml(t("configTrace.noReaders"))}</p>`}
          ${paths ? `<ul class="trace-list">${paths}</ul>` : ""}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = result.truncated
    ? `<p class="empty">${escapeHtml(t("trace.resultTruncated"))}</p>`
    : "";
  return `${summary}${rows}${truncated}`;
}

function renderConfigTracePath(path, matchIndex, pathIndex) {
  const labels = path.nodes.map((node) => node.label).join(" -> ");
  const kind = path.reached_entrypoint ? t("trace.entrypointPath") : t("configTrace.readerPath");
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
      const selectedId = path.nodes[path.nodes.length - 1]?.id ?? null;
      showFocusedGraph(
        focused,
        t("configTrace.focusTitle", { label: match.target.label }),
        selectedId,
      );
    });
  });
}

async function runErrorTrace() {
  const target = errorTraceTargetInput.value.trim();
  if (!target) {
    errorTraceResult.innerHTML = `<p class="empty">${escapeHtml(t("errorTrace.enterTarget"))}</p>`;
    return;
  }

  const depth = clampNumber(Number(errorTraceDepthInput.value || 6), 1, 32);
  errorTraceDepthInput.value = String(depth);
  state.errorTraceRequest += 1;
  const requestId = state.errorTraceRequest;
  errorTraceButton.disabled = true;
  clearLastErrorTraceReport();
  errorTraceResult.innerHTML = `<p class="empty">${escapeHtml(t("errorTrace.tracing"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    target,
    depth: String(depth),
    limit: "50",
  });

  try {
    const response = await apiFetch(`/api/trace-errors?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.errorTraceRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("errorTrace.failedFallback")));
    }
    state.lastErrorTraceReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        target,
        depth,
        limit: 50,
      },
      report: body,
    };
    renderErrorTraceExportState();
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

function renderErrorTraceExportState() {
  errorTraceExportButton.disabled = !state.lastErrorTraceReport;
}

function clearLastErrorTraceReport() {
  state.lastErrorTraceReport = null;
  renderErrorTraceExportState();
}

function exportLastErrorTraceReport() {
  if (!state.lastErrorTraceReport) {
    errorTraceResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noErrorTrace"))}</p>`;
    renderErrorTraceExportState();
    return;
  }

  const payload = {
    schema: "codegraph.error_trace.v1",
    ...state.lastErrorTraceReport,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-error-trace.json`;
  downloadBlob(blob, fileName);
  errorTraceResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.errorTrace"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(t("errorTrace.errorCount", { count: formatNumber(payload.report?.total_matches ?? payload.report?.matches?.length ?? 0) }))}</span>
        <span>${escapeHtml(t("trace.pathCount", { count: formatNumber(payload.report?.total_paths ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function renderErrorTrace(result) {
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("errorTrace.errorCount", { count: formatNumber(result.total_matches || 0) }))}</span>
      <span>${escapeHtml(t("errorTrace.sourceCount", { count: formatNumber(result.total_sources || 0) }))}</span>
      <span>${escapeHtml(t("trace.pathCount", { count: formatNumber(result.total_paths || 0) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: result.max_depth }))}</span>
      <span class="query-expression">${escapeHtml(result.target)}</span>
    </div>
  `;

  if (!result.matches.length) {
    return `${summary}<p class="empty">${escapeHtml(t("errorTrace.noMatches"))}</p>`;
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
      const truncated = match.truncated
        ? `<p class="empty">${escapeHtml(t("trace.traceTruncated"))}</p>`
        : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(match.error.label)}</h3>
          <div class="trace-summary">
            <span>${escapeHtml(t("errorTrace.sourceCount", { count: formatNumber(match.total_sources || 0) }))}</span>
            <span>${escapeHtml(t("trace.pathCount", { count: formatNumber(match.total_paths || 0) }))}</span>
            <span>${escapeHtml(formatKind(match.error.metadata?.language || match.error.kind))}</span>
          </div>
          ${sources ? `<ul class="trace-list">${sources}</ul>` : `<p class="empty">${escapeHtml(t("errorTrace.noSources"))}</p>`}
          ${paths ? `<ul class="trace-list">${paths}</ul>` : ""}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = result.truncated
    ? `<p class="empty">${escapeHtml(t("trace.resultTruncated"))}</p>`
    : "";
  return `${summary}${rows}${truncated}`;
}

function renderErrorTracePath(path, matchIndex, pathIndex) {
  const labels = path.nodes.map((node) => node.label).join(" -> ");
  const kind = path.reached_entrypoint ? t("trace.entrypointPath") : t("errorTrace.sourcePath");
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
      const selectedId = path.nodes[path.nodes.length - 1]?.id ?? null;
      showFocusedGraph(
        focused,
        t("errorTrace.focusTitle", { label: match.error.label }),
        selectedId,
      );
    });
  });
}

function renderSourceSearchResult(result) {
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("sourceSearch.matchCount", { count: formatNumber(result.total_matches || 0) }))}</span>
      ${result.truncated ? `<span>${escapeHtml(t("sourceSearch.truncated"))}</span>` : ""}
      <span class="query-expression">${escapeHtml(result.query)}</span>
    </div>
  `;
  const rows = (result.matches || [])
    .map((match, index) => renderSourceSearchMatch(match, index))
    .join("");
  return `
    ${summary}
    ${rows ? `<ul class="query-list">${rows}</ul>` : `<p class="empty">${escapeHtml(t("sourceSearch.noMatches"))}</p>`}
  `;
}

function renderSourceSearchMatch(match, index) {
  const context = (match.context || []).map(renderSourceLine).join("");
  return `
    <li class="source-match">
      <button class="query-item" type="button" data-source-match="${index}">
        <span>${escapeHtml(match.path)}:${match.line}:${match.column}</span>
        <strong>${escapeHtml(match.line_text || " ")}</strong>
      </button>
      <button class="query-inline-action" type="button" data-source-file-graph="${index}">
        ${escapeHtml(t("button.graphFile"))}
      </button>
      ${context ? `<pre class="source-context"><code>${context}</code></pre>` : ""}
    </li>
  `;
}

function attachSourceSearchActions(container, result) {
  container.querySelectorAll("[data-source-match]").forEach((button) => {
    button.addEventListener("click", () => {
      const match = result.matches?.[Number(button.dataset.sourceMatch)];
      if (match) openSourceSearchMatch(match);
    });
  });
  container.querySelectorAll("[data-source-file-graph]").forEach((button) => {
    button.addEventListener("click", () => {
      const match = result.matches?.[Number(button.dataset.sourceFileGraph)];
      if (match?.path) openSourceFileGraph(match.path);
    });
  });
}

async function openSourceFileGraph(path) {
  queryInput.value = `files path:${quoteQueryValue(path)} direction:out edge_limit:300`;
  await runGraphQuery({ focus: true });
  queryResult.scrollIntoView({ block: "nearest" });
}

async function openSourceSearchMatch(match) {
  state.selectionRequest += 1;
  const requestId = state.selectionRequest;
  clearSelection({ render: false });
  selectionTitle.textContent = t("selection.sourceMatch");
  selectionBody.innerHTML = `
    <section class="source-preview">
      <header>
        <span>${escapeHtml(t("selection.source"))}</span>
        <strong>${escapeHtml(match.path)}:${match.line}</strong>
      </header>
      <pre id="sourceMatchPreview"><code>${escapeHtml(t("selection.sourceLoading"))}</code></pre>
    </section>
  `;
  const preview = selectionBody.querySelector("#sourceMatchPreview code");
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    file: match.path,
    start_line: String(match.line),
    end_line: String(match.line),
    context: "5",
  });

  try {
    const response = await apiFetch(`/api/source?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "failed to load source"));
    }
    preview.innerHTML = body.lines.map(renderSourceLine).join("");
  } catch (error) {
    if (requestId !== state.selectionRequest) return;
    preview.innerHTML = `<span class="source-error">${escapeHtml(error.message)}</span>`;
  }
}

function rememberQuery(expression) {
  const query = expression.trim();
  if (!query) return;
  state.queryHistory = [
    query,
    ...state.queryHistory.filter((item) => item.toLowerCase() !== query.toLowerCase()),
  ].slice(0, QUERY_HISTORY_LIMIT);
  persistQueryHistory();
  renderQueryHistory();
}

function renderQueryExportState() {
  queryExportButton.disabled = !state.lastQueryResult;
}

function clearLastQueryResult() {
  state.lastQueryResult = null;
  renderQueryExportState();
}

function exportLastQueryResult() {
  if (!state.lastQueryResult) {
    queryResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noQueryResult"))}</p>`;
    renderQueryExportState();
    return;
  }

  const payload = {
    schema: "codegraph.query_result.v1",
    ...state.lastQueryResult,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-query-result.json`;
  downloadBlob(blob, fileName);
  queryResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.queryResult"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(formatNumber(payload.result?.returned_nodes ?? payload.result?.nodes?.length ?? 0))} nodes</span>
        <span>${escapeHtml(formatNumber(payload.result?.returned_edges ?? payload.result?.edges?.length ?? 0))} edges</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function persistQueryHistory() {
  try {
    window.localStorage?.setItem(QUERY_HISTORY_STORAGE_KEY, JSON.stringify(state.queryHistory));
  } catch (error) {
    // The in-memory history remains usable when storage is unavailable.
  }
}

function clearQueryHistory() {
  state.queryHistory = [];
  persistQueryHistory();
  renderQueryHistory();
}

function renderQueryHistory() {
  if (!state.queryHistory.length) {
    queryHistory.hidden = true;
    queryHistoryList.innerHTML = "";
    return;
  }

  queryHistory.hidden = false;
  queryHistoryList.innerHTML = state.queryHistory
    .map(
      (query) => `
        <button type="button" data-query-history="${escapeHtml(query)}" aria-label="${escapeHtml(t("queryHistory.run", { query }))}">
          ${escapeHtml(query)}
        </button>
      `,
    )
    .join("");
  queryHistoryList.querySelectorAll("[data-query-history]").forEach((button) => {
    button.addEventListener("click", () => {
      queryInput.value = button.dataset.queryHistory || "";
      runGraphQuery();
    });
  });
}

function renderNaturalQueryReport(report) {
  const result = report.result || { query: report.generated_query || "", nodes: [], edges: [] };
  const alternatives = Array.isArray(report.alternatives) ? report.alternatives : [];
  const alternativeButtons = alternatives
    .slice(0, 5)
    .map(
      (query) => `
        <button type="button" data-ask-alternative="${escapeHtml(query)}">
          ${escapeHtml(query)}
        </button>
      `,
    )
    .join("");
  const alternativesMarkup = alternativeButtons
    ? `
      <div class="query-presets ask-alternatives" aria-label="${escapeHtml(t("ask.alternatives"))}">
        ${alternativeButtons}
      </div>
    `
    : "";

  return `
    <div class="query-summary">
      <span>${escapeHtml(t("ask.resultLabel"))}</span>
      <span>${escapeHtml(t("ask.question"))}</span>
      <span class="query-expression">${escapeHtml(report.question || "")}</span>
      <span>${escapeHtml(t("ask.rule", { rule: report.rule || "unknown" }))}</span>
      <span>${escapeHtml(t("ask.confidence", { confidence: report.confidence || "unknown" }))}</span>
      <span>${escapeHtml(t("ask.generatedQuery"))}</span>
      <span class="query-expression">${escapeHtml(report.generated_query || result.query || "")}</span>
    </div>
    ${alternativesMarkup}
    ${renderQueryResult(result, { label: t("ask.generatedQuery") })}
  `;
}

function attachNaturalQueryActions(container) {
  container.querySelectorAll("[data-ask-alternative]").forEach((button) => {
    button.addEventListener("click", () => {
      queryInput.value = button.dataset.askAlternative || "";
      runGraphQuery({ focus: true });
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
  const shownNodes = Number(result.returned_nodes ?? result.nodes.length);
  const shownEdges = Number(result.returned_edges ?? result.edges.length);
  const nodeTotal = Number(result.total_nodes ?? shownNodes);
  const edgeTotal = Number(result.total_edges ?? shownEdges);

  return `
    <div class="query-summary">
      ${resultLabel}
      <span>${formatReturnedCount(shownNodes, nodeTotal)} nodes</span>
      <span>${formatReturnedCount(shownEdges, edgeTotal)} edges</span>
      ${expression}
    </div>
    ${renderQueryFacets(result.facets)}
    <div class="query-actions">
      <button data-focus-result type="button" ${hasResults ? "" : "disabled"}>Focus result</button>
      <button data-query-workflows type="button" ${hasResults ? "" : "disabled"}>${escapeHtml(t("button.buildQueryWorkflows"))}</button>
      <button data-clear-focus type="button" ${state.queryFocus ? "" : "disabled"}>Clear focus</button>
    </div>
    ${nodeRows ? `<ul class="query-list">${nodeRows}</ul>` : ""}
    ${edgeRows ? `<ul class="query-list query-edge-list">${edgeRows}</ul>` : ""}
    ${!nodeRows && !edgeRows ? '<p class="empty">No query results.</p>' : ""}
    ${truncated}
  `;
}

function formatReturnedCount(returned, total) {
  return returned === total ? String(total) : `${returned}/${total}`;
}

function renderQueryFacets(facets) {
  if (!facets) return "";
  const groups = [
    ["nodes", facets.node_kinds],
    ["edges", facets.edge_kinds],
    ["languages", facets.languages],
    ["items", facets.item_kinds],
    ["confidence", facets.edge_confidences],
  ]
    .map(([label, values]) => renderQueryFacetGroup(label, values))
    .filter(Boolean)
    .join("");
  return groups ? `<div class="query-facets">${groups}</div>` : "";
}

function renderQueryFacetGroup(label, values) {
  const entries = Object.entries(values || {})
    .filter(([, count]) => Number(count) > 0)
    .sort((left, right) => Number(right[1]) - Number(left[1]) || left[0].localeCompare(right[0]))
    .slice(0, 6);
  if (!entries.length) return "";
  const chips = entries
    .map(
      ([key, count]) =>
        `<span><strong>${escapeHtml(formatKind(key))}</strong>${Number(count)}</span>`,
    )
    .join("");
  return `<section><h3>${escapeHtml(label)}</h3>${chips}</section>`;
}

async function runQueryWorkflow(expression) {
  const query = String(expression || queryInput.value || "").trim();
  if (!query) {
    queryResult.innerHTML = `<p class="empty">${escapeHtml(t("query.enterExpression"))}</p>`;
    return;
  }
  if (!graphQueryWithinClientLimit(query, queryResult)) return;

  state.queryWorkflowRequest += 1;
  const requestId = state.queryWorkflowRequest;
  queryResult.innerHTML = `<p class="empty">${escapeHtml(t("workflow.loading"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: query,
    depth: "4",
    block_limit: "120",
    limit: "15",
    compact: "true",
  });

  try {
    const response = await apiFetch(`/api/workflow-query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.queryWorkflowRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "workflow query failed"));
    }
    queryResult.innerHTML = renderWorkflowQueryReport(body);
    attachWorkflowQueryActions(queryResult, body);
    attachEdgeExplainActions(queryResult);
  } catch (error) {
    if (requestId !== state.queryWorkflowRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderWorkflowQueryReport(report) {
  const workflows = Array.isArray(report.workflows) ? report.workflows : [];
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("entryFlows.workflowCount", { count: formatNumber(workflows.length) }))}</span>
      <span>${escapeHtml(t("query.workflowStarts", { count: formatReturnedCount(workflows.length, Number(report.total_candidates || workflows.length)) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: formatNumber(report.max_depth || 0) }))}</span>
      ${report.query ? `<span class="query-expression">${escapeHtml(report.query)}</span>` : ""}
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
            <button type="button" data-query-workflow="${index}">${escapeHtml(t("entryFlows.focusWorkflow"))}</button>
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

function attachWorkflowQueryActions(container, report) {
  attachWorkflowNavigation(container);
  attachFlowViewActions(
    container,
    (index) => report.workflows?.[index],
    (workflow) => workflow.start?.label || "",
  );
  container.querySelectorAll("[data-query-workflow]").forEach((button) => {
    button.addEventListener("click", () => {
      const workflow = report.workflows?.[Number(button.dataset.queryWorkflow)];
      if (!workflow) return;
      const blocks = Array.isArray(workflow.blocks) ? workflow.blocks : [];
      const transitions = Array.isArray(workflow.transitions) ? workflow.transitions : [];
      const focused = {
        query: `workflow-query ${report.query || ""}`,
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
        ${renderEdgeActions(edge, source, target)}
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
      selectNodeById(nodeId);
    });
  });
}

function attachQueryFocusActions(container, result) {
  const focusButton = container.querySelector("[data-focus-result]");
  const workflowButton = container.querySelector("[data-query-workflows]");
  const clearButton = container.querySelector("[data-clear-focus]");
  if (focusButton) {
    focusButton.addEventListener("click", () => {
      focusQueryResult(result, container);
      if (result.query) syncQueryUrl(result.query, { focus: true });
    });
  }
  if (workflowButton) {
    workflowButton.addEventListener("click", () => runQueryWorkflow(result.query || queryInput.value.trim()));
  }
  if (clearButton) {
    clearButton.addEventListener("click", () => {
      clearQueryFocus();
      if (result.query) syncQueryUrl(result.query, { focus: false });
    });
  }
}

function attachEdgeExplainActions(container) {
  container.querySelectorAll("[data-select-edge]").forEach((button) => {
    button.addEventListener("click", () => selectEdgeByKey(button.dataset.edgeSelectionKey));
  });
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

function clearCanvasFilters() {
  state.search = "";
  searchInput.value = "";
  state.queryFocus = null;
  state.activeRiskSeverity = null;
  state.enabledKinds = new Set(state.graph.nodes.map((node) => node.kind));
  renderKindFilters([...state.enabledKinds].sort());
  renderLegend();
  document.querySelectorAll("[data-clear-focus]").forEach((button) => {
    button.disabled = true;
  });
  applyFilters();
  draw();
}

function canvasFilterCount() {
  let count = 0;
  if (state.search) count += 1;
  if (state.queryFocus) count += 1;
  if (state.activeRiskSeverity) count += 1;

  const graphKinds = new Set(state.graph.nodes.map((node) => node.kind));
  if (graphKinds.size > 0) {
    const missingKind = [...graphKinds].some((kind) => !state.enabledKinds.has(kind));
    if (state.enabledKinds.size !== graphKinds.size || missingKind) count += 1;
  }

  return count;
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
