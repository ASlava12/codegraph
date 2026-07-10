// 07-runtime.js — capabilities, scan/semantic job monitor, runtime metrics.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

async function loadCapabilities() {
  try {
    const response = await apiFetch("/api/capabilities");
    const body = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "capabilities failed"));
    }
    state.capabilities = body;
  } catch (error) {
    state.capabilities = null;
  }
  renderOverview();
}

async function loadApiSchema() {
  state.apiSchemaRequest += 1;
  const requestId = state.apiSchemaRequest;

  try {
    const response = await apiFetch("/api/schema");
    const body = await response.json();
    if (requestId !== state.apiSchemaRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "schema failed"));
    }
    state.apiSchema = body;
  } catch (error) {
    if (requestId !== state.apiSchemaRequest) return;
    state.apiSchema = null;
  }

  renderApiSchemaOptions();
  renderOverview();
}

function renderApiSchemaOptions() {
  const enums = state.apiSchema?.enum_values || {};
  renderDatalist(nodeKindOptions, enums.graph_node_kind || []);
  renderDatalist(edgeKindOptions, enums.graph_edge_kind || []);
  renderDatalist(confidenceOptions, enums.graph_confidence || []);
  renderDatalist(severityOptions, enums.insight_severity || []);
  renderDatalist(workflowBlockKindOptions, enums.workflow_block_kind || []);
  renderDatalist(entrypointKindOptions, enums.entrypoint_kind || []);
  renderDatalist(insightKindOptions, enums.insight_kind || []);
}

function renderDatalist(element, values) {
  if (!element) return;
  element.innerHTML = Array.isArray(values)
    ? values.map((value) => `<option value="${escapeHtml(String(value))}"></option>`).join("")
    : "";
}

async function loadMetrics() {
  state.metricsRequest += 1;
  const requestId = state.metricsRequest;
  metricsRefreshButton.disabled = true;

  try {
    const response = await apiFetch("/api/metrics");
    const body = await response.json();
    if (requestId !== state.metricsRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "metrics failed"));
    }
    state.metrics = body;
    renderRuntimeMetrics();
  } catch (error) {
    if (requestId !== state.metricsRequest) return;
    state.metrics = null;
    runtimeMetricsList.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.metricsRequest) {
      metricsRefreshButton.disabled = false;
    }
  }
}

function renderProjects() {
  if (!state.projects.length) {
    projectSelect.innerHTML = `<option value=".">${escapeHtml(t("project.currentRoot"))}</option>`;
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

async function loadJobQueue() {
  state.jobQueueRequest += 1;
  const requestId = state.jobQueueRequest;
  jobRefreshButton.disabled = true;

  try {
    const [scanJobs, semanticJobs] = await Promise.all([fetchJobList("scan"), fetchJobList("semantic")]);
    if (requestId !== state.jobQueueRequest) return;
    state.scanJobs = scanJobs;
    state.semanticJobs = semanticJobs;
    renderJobQueue();
    loadMetrics();
  } catch (error) {
    if (requestId !== state.jobQueueRequest) return;
    scanJobList.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    semanticJobList.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.jobQueueRequest) {
      jobRefreshButton.disabled = false;
    }
  }
}

async function fetchJobList(kind) {
  const endpoint = kind === "semantic" ? "/api/semantic-jobs" : "/api/scan-jobs";
  const response = await apiFetch(`${endpoint}?limit=8`);
  const body = await response.json();
  if (!response.ok) {
    throw new Error(apiErrorMessage(body, response, `${kind} jobs failed`));
  }
  return body;
}

function renderJobQueue() {
  renderJobSummary(scanJobSummary, state.scanJobs);
  renderJobSummary(semanticJobSummary, state.semanticJobs);
  scanJobList.innerHTML = renderJobList(state.scanJobs, "scan");
  semanticJobList.innerHTML = renderJobList(state.semanticJobs, "semantic");
}

function renderRuntimeMetrics() {
  const metrics = state.metrics;
  if (!metrics) {
    runtimeMetricsList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noMetrics"))}</p>`;
    return;
  }

  const scanConcurrency = metrics.scan_jobs?.concurrency || {};
  const semanticConcurrency = metrics.semantic_jobs?.concurrency || {};
  const chips = [
    [t("runtime.uptime"), formatDuration(Number(metrics.uptime_seconds || 0)), ""],
    [t("runtime.cache"), metrics.cache?.enabled ? t("cap.on") : t("cap.off"), metrics.cache?.enabled ? "" : "missing"],
    [t("runtime.scanSlots"), concurrencyValue(scanConcurrency), Number(scanConcurrency.active || 0) > 0 ? "busy" : ""],
    [
      t("runtime.semanticSlots"),
      concurrencyValue(semanticConcurrency),
      Number(semanticConcurrency.active || 0) > 0 ? "busy" : "",
    ],
    [t("runtime.scanJobs"), jobStoreValue(metrics.scan_jobs?.store), jobStoreBusyClass(metrics.scan_jobs?.store)],
    [
      t("runtime.semanticJobs"),
      jobStoreValue(metrics.semantic_jobs?.store),
      jobStoreBusyClass(metrics.semantic_jobs?.store),
    ],
  ];
  if (state.lastApiResponse) {
    chips.push([
      t("runtime.lastApi"),
      formatLastApiResponse(state.lastApiResponse),
      lastApiResponseClass(state.lastApiResponse),
    ]);
  }

  runtimeMetricsList.innerHTML = chips
    .map(
      ([label, value, status]) => `
        <div class="runtime-metric-chip ${escapeHtml(status)}">
          <span>${escapeHtml(label)}</span>
          <strong>${escapeHtml(value)}</strong>
        </div>
      `,
    )
    .join("");
}

function concurrencyValue(concurrency) {
  return `${Number(concurrency.active || 0)}/${Number(concurrency.limit || 0)}`;
}

function jobStoreValue(store) {
  const total = Number(store?.total || 0);
  const active = Number(store?.queued || 0) + Number(store?.running || 0);
  return active > 0 ? `${active}/${total}` : String(total);
}

function jobStoreBusyClass(store) {
  return Number(store?.queued || 0) + Number(store?.running || 0) > 0 ? "busy" : "";
}

function formatLastApiResponse(apiResponse) {
  const elapsed = Number(apiResponse?.elapsedMs);
  const status = Number(apiResponse?.status || 0);
  const latency = Number.isFinite(elapsed) ? `${elapsed} ms` : "? ms";
  return status > 0 ? `${latency} / ${status}` : latency;
}

function lastApiResponseClass(apiResponse) {
  const elapsed = Number(apiResponse?.elapsedMs);
  const status = Number(apiResponse?.status || 0);
  if (status >= 500) return "missing";
  return Number.isFinite(elapsed) && elapsed >= 1000 ? "busy" : "";
}

function renderJobSummary(target, list) {
  const summary = list?.summary;
  if (!summary) {
    target.textContent = "0";
    return;
  }
  const active = (summary.queued || 0) + (summary.running || 0);
  target.textContent = active ? `${active}/${summary.total}` : String(summary.total || 0);
}

function renderJobList(list, kind) {
  const jobs = list?.jobs || [];
  if (!jobs.length) {
    return `<p class="empty">${escapeHtml(t("job.empty"))}</p>`;
  }
  return jobs.map((job) => renderJobCard(job, kind)).join("");
}

function renderJobCard(job, kind) {
  const canCancel = job.status === "queued" || job.status === "running";
  const cancelButton = canCancel
    ? `<button class="job-cancel-button cancel-action" type="button" data-job-id="${escapeHtml(job.id)}">${escapeHtml(t("button.cancel"))}</button>`
    : "";
  const updated = formatJobTime(job.updated_at_unix);
  return `
    <article class="job-card ${escapeHtml(job.status || "unknown")}">
      <header>
        <strong>${escapeHtml(job.id)}</strong>
        <span>${escapeHtml(formatJobStatus(job.status))}</span>
      </header>
      <p>${escapeHtml(job.message || "")}</p>
      <footer>
        <span>${escapeHtml(kind === "semantic" ? t("job.semantic") : t("job.scan"))}</span>
        <span>${escapeHtml(t("job.updated"))}: ${escapeHtml(updated)}</span>
        ${cancelButton}
      </footer>
    </article>
  `;
}

async function onJobListClick(event, kind) {
  const button = event.target.closest("[data-job-id]");
  if (!button) return;
  await cancelJobFromList(kind, button.dataset.jobId, button);
}

async function cancelJobFromList(kind, jobId, button) {
  if (!jobId) return;
  button.disabled = true;
  const endpoint = kind === "semantic" ? "/api/semantic-jobs" : "/api/scan-jobs";
  try {
    const response = await apiFetch(`${endpoint}/${encodeURIComponent(jobId)}`, { method: "DELETE" });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "job cancel failed"));
    }
    if (kind === "scan" && state.scanJobId === jobId) {
      state.scanJobId = null;
      if (state.scanEvents) {
        state.scanEvents.close();
        state.scanEvents = null;
      }
      scanCancelButton.disabled = true;
      selectionTitle.textContent = t("status.ready");
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("job.scanCanceled"))}</p>`;
    }
    if (kind === "semantic" && state.semanticJobId === jobId) {
      state.semanticJobId = null;
      if (state.semanticEvents) {
        state.semanticEvents.close();
        state.semanticEvents = null;
      }
      semanticCancelButton.disabled = true;
      semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(t("job.semanticCanceled"))}</p>`;
    }
    setStatus("ready");
    await loadJobQueue();
  } catch (error) {
    button.disabled = false;
    setStatus("error", "error");
    button.closest(".job-card")?.insertAdjacentHTML(
      "beforeend",
      `<p class="error-text">${escapeHtml(error.message)}</p>`,
    );
  }
}

function formatJobStatus(status) {
  return translate(`job.status.${status}`, formatKind(status || "unknown"));
}

function formatJobTime(seconds) {
  if (!seconds) return "";
  const date = new Date(Number(seconds) * 1000);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString(state.locale, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function formatDuration(seconds) {
  const total = Math.max(0, Math.floor(Number(seconds || 0)));
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

async function scan() {
  setStatus("queue", "busy");
  scanButton.disabled = true;
  scanCancelButton.disabled = true;
  selectionTitle.textContent = t("selection.title");
  selectionBody.innerHTML = "";
  clearSelection({ syncUrl: !state.pendingSelectionLink, render: false });
  state.insightRequest += 1;
  state.overviewRequest += 1;
  state.summary = null;
  state.scanOptions = null;
  state.coverage = null;
  state.lsp = null;
  state.semanticReadiness = null;
  state.semanticPlan = null;
  state.report = null;
  state.architecture = null;
  state.languageDependencies = null;
  state.hotspots = null;
  if (!state.pendingGraphPageLink) state.architecturePathPrefix = "";
  state.entrypoints = [];
  renderOverview();
  state.insightReport = null;
  renderInsights();
  clearLastEntryFlowReport();
  clearLastEntryWorkflowReport();
  clearLastConfigTraceReport();
  clearLastErrorTraceReport();
  clearLastCheckResult();
  checkResult.innerHTML = "";
  clearLastQueryResult();
  clearLastSourceSearchResult();
  clearLastPathResult();
  exportResult.innerHTML = "";
  if (state.scanEvents) {
    state.scanEvents.close();
    state.scanEvents = null;
  }
  if (state.semanticEvents) {
    state.semanticEvents.close();
    state.semanticEvents = null;
  }

  try {
    const response = await apiFetch("/api/scan-jobs", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path: pathInput.value.trim() || "." }),
    });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "failed to start scan"));
    }

    state.scanJobId = body.id;
    scanCancelButton.disabled = false;
    loadJobQueue();
    await watchScanJob(body.id);
  } catch (error) {
    setStatus("error", "error");
    selectionTitle.textContent = t("status.error");
    selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    scanButton.disabled = false;
    scanCancelButton.disabled = true;
    loadJobQueue();
  }
}

async function cancelScanJob() {
  const jobId = state.scanJobId;
  if (!jobId) return;

  scanCancelButton.disabled = true;
  try {
    const response = await apiFetch(`/api/scan-jobs/${encodeURIComponent(jobId)}`, { method: "DELETE" });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "failed to cancel scan"));
    }
    state.scanJobId = null;
    if (state.scanEvents) {
      state.scanEvents.close();
      state.scanEvents = null;
    }
    setStatus("ready");
    selectionTitle.textContent = t("status.ready");
    selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("job.scanCanceled"))}</p>`;
    loadJobQueue();
  } catch (error) {
    setStatus("error", "error");
    selectionTitle.textContent = t("status.error");
    selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    scanCancelButton.disabled = false;
  }
}

async function watchScanJob(jobId) {
  if (!window.EventSource) {
    return pollScanJob(jobId);
  }

  return new Promise((resolve, reject) => {
    let settled = false;
    const events = authenticatedEventSource(`/api/scan-jobs/${encodeURIComponent(jobId)}/events`);
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

      if (job.status === "canceled") {
        settled = true;
        events.close();
        if (state.scanEvents === events) state.scanEvents = null;
        if (state.scanJobId === jobId) state.scanJobId = null;
        setStatus("ready");
        selectionTitle.textContent = t("status.ready");
        selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("job.scanCanceled"))}</p>`;
        resolve();
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
    const response = await apiFetch(`/api/scan-jobs/${encodeURIComponent(jobId)}`);
    const body = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "scan status failed"));
    }

    if (body.status === "queued" || body.status === "running") {
      setStatus(body.status === "queued" ? "queue" : "scan", "busy");
      await sleep(350);
      continue;
    }

    if (body.status === "failed") {
      throw new Error(body.message || "scan failed");
    }

    if (body.status === "canceled") {
      if (state.scanJobId === jobId) state.scanJobId = null;
      setStatus("ready");
      selectionTitle.textContent = t("status.ready");
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("job.scanCanceled"))}</p>`;
      return;
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
  edgePrevButton.disabled = true;
  edgeNextButton.disabled = true;
  pageCopyButton.disabled = true;
  pageClearButton.disabled = true;

  const preserveGraphPageLink = resetPage && state.pendingGraphPageLink;
  const preserveInvestigationLink = Boolean(state.pendingSelectionLink || state.pendingQueryLink);
  if (resetPage && !preserveGraphPageLink) {
    state.graphPage.nodeOffset = 0;
    state.graphPage.edgeOffset = 0;
  }
  if (preserveGraphPageLink) state.pendingGraphPageLink = false;

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
    edge_offset: String(state.graphPage.edgeOffset),
    edge_limit: String(edgeLimit),
  });
  const kind = serverKindInput.value.trim();
  const itemKind = serverItemKindInput.value.trim();
  const language = serverLanguageInput.value.trim();
  const serverSearch = serverSearchInput.value.trim();
  const edgeKind = serverEdgeKindInput.value.trim();
  const confidence = serverConfidenceInput.value.trim();
  const edgeRelation = serverEdgeRelationInput.value.trim();
  const edgeSource = serverEdgeSourceInput.value.trim();
  if (state.architecturePathPrefix) params.set("path_prefix", state.architecturePathPrefix);
  if (kind) params.set("kind", kind);
  if (itemKind) params.set("item_kind", itemKind);
  if (language) params.set("language", language);
  if (serverSearch) params.set("search", serverSearch);
  if (edgeKind) params.set("edge_kind", edgeKind);
  if (confidence) params.set("confidence", confidence);
  if (edgeRelation) params.set("edge_relation", edgeRelation);
  if (edgeSource) params.set("edge_source", edgeSource);

  try {
    const response = await apiFetch(`/api/graph?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.pageRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "graph page failed"));
    }

    state.graph = { nodes: body.nodes, edges: body.edges };
    state.graphPage.totalNodes = body.total_nodes;
    state.graphPage.totalEdges = body.total_edges;
    state.graphPage.nodeOffset = body.node_offset;
    state.graphPage.nodeLimit = body.node_limit;
    state.graphPage.edgeOffset = body.edge_offset;
    state.graphPage.edgeLimit = body.edge_limit;
    state.graphPage.truncatedNodes = body.truncated_nodes;
    state.graphPage.truncatedEdges = body.truncated_edges;
    state.graphPage.root = root || state.graphPage.root || pathInput.value.trim() || ".";
    clearSelection({ syncUrl: false, render: false });
    state.hoveredId = null;
    state.hoveredEdgeKey = null;
    state.queryFocus = null;
    state.insightReport = null;
    queryResult.innerHTML = "";
    checkResult.innerHTML = "";
    exportResult.innerHTML = "";
    entryFlowResult.innerHTML = "";
    pathResult.innerHTML = "";
    clearLastConfigTraceReport();
    clearLastErrorTraceReport();
    configTraceResult.innerHTML = "";
    errorTraceResult.innerHTML = "";
    rootLabel.textContent = state.graphPage.root;
    initializeGraph({ preserveView: !resetLayout, largeGraphDefaults: true });
    await restorePendingQueryLink();
    await restorePendingSelectionLink();
    if (!preserveInvestigationLink && state.selectedId == null && !state.selectedEdgeKey && !state.queryFocus) {
      syncGraphPageUrl();
    }
    loadProjectOverview();
    loadInsights();
    setStatus("ready");
  } catch (error) {
    if (requestId !== state.pageRequest) return;
    setStatus("error", "error");
    selectionTitle.textContent = t("status.error");
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
  const semanticParams = new URLSearchParams(params);
  const workLanguage = semanticWorkLanguageInput.value.trim();
  const workStatus = semanticWorkStatusInput.value.trim();
  const workCapability = semanticWorkCapabilityInput.value.trim();
  if (workLanguage) semanticParams.set("work_language", workLanguage);
  if (workStatus) semanticParams.set("work_status", workStatus);
  if (workCapability) semanticParams.set("work_capability", workCapability);
  const reportParams = new URLSearchParams(params);
  reportParams.set("architecture_group_limit", "8");
  reportParams.set("architecture_edge_limit", "40");
  reportParams.set("language_link_limit", "8");
  reportParams.set("hotspot_limit", "8");
  reportParams.set("community_limit", "8");
  reportParams.set("insight_limit", "6");
  reportParams.set("fail_on", "warning");

  try {
    const [
      scanOptionsResponse,
      lspResponse,
      semanticReadinessResponse,
      semanticPlanResponse,
      reportResponse,
    ] = await Promise.all([
      apiFetch(`/api/scan-options?${params.toString()}`),
      apiFetch("/api/lsp"),
      apiFetch(`/api/semantic-readiness?${params.toString()}`),
      apiFetch(`/api/semantic-plan?${semanticParams.toString()}`),
      apiFetch(`/api/report?${reportParams.toString()}`),
    ]);
    const scanOptions = await scanOptionsResponse.json();
    const lsp = await lspResponse.json();
    const semanticReadiness = await semanticReadinessResponse.json();
    const semanticPlan = await semanticPlanResponse.json();
    const reportResponseBody = await reportResponse.json();
    if (requestId !== state.overviewRequest) return;
    if (!scanOptionsResponse.ok) {
      throw new Error(apiErrorMessage(scanOptions, scanOptionsResponse, "scan options failed"));
    }
    if (!lspResponse.ok) {
      throw new Error(apiErrorMessage(lsp, lspResponse, "lsp status failed"));
    }
    if (!semanticReadinessResponse.ok) {
      throw new Error(apiErrorMessage(semanticReadiness, semanticReadinessResponse, "semantic readiness failed"));
    }
    if (!semanticPlanResponse.ok) {
      throw new Error(apiErrorMessage(semanticPlan, semanticPlanResponse, "semantic plan failed"));
    }
    if (!reportResponse.ok) {
      throw new Error(apiErrorMessage(reportResponseBody, reportResponse, "report failed"));
    }
    const report = reportResponseBody.report || {};
    state.summary = report.summary || null;
    state.scanOptions = scanOptions;
    state.coverage = reportResponseBody.coverage || null;
    state.lsp = lsp;
    state.semanticReadiness = semanticReadiness;
    state.semanticPlan = semanticPlan;
    state.report = report;
    state.architecture = report.architecture || null;
    state.languageDependencies = report.language_dependencies || null;
    state.communities = report.communities || null;
    state.hotspots = report.hotspots || null;
    state.entrypoints = Array.isArray(report.entrypoints) ? report.entrypoints : [];
    renderOverview();
  } catch (error) {
    if (requestId !== state.overviewRequest) return;
    overviewTotals.textContent = "error";
    renderCapabilities(state.capabilities);
    languageList.innerHTML = "";
    confidenceList.innerHTML = "";
    relationList.innerHTML = "";
    edgeSourceList.innerHTML = "";
    scanPolicyList.innerHTML = "";
    coverageList.innerHTML = "";
    state.report = null;
    riskSummaryList.innerHTML = "";
    lspList.innerHTML = "";
    semanticWorkList.innerHTML = "";
    architectureList.innerHTML = "";
    languageDependencyList.innerHTML = "";
    communityList.innerHTML = "";
    hotspotList.innerHTML = "";
    annotationList.innerHTML = "";
    entrypointList.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function runSemanticEnrich() {
  state.semanticEnrichRequest += 1;
  const requestId = state.semanticEnrichRequest;
  const workLanguage = semanticWorkLanguageInput.value.trim();
  const workStatus = semanticWorkStatusInput.value.trim();
  const workCapability = semanticWorkCapabilityInput.value.trim();
  const body = {
    path: pathInput.value.trim() || ".",
    work_item_limit: Number(state.semanticPlan?.work_item_limit || 100),
    work_status: workStatus || "ready",
    request_timeout_ms: Number(
      state.capabilities?.limits?.default_semantic_request_timeout_ms || 30_000,
    ),
  };
  if (workLanguage) body.work_language = workLanguage;
  if (workCapability) body.work_capability = workCapability;

  setStatus("semantic", "busy");
  semanticEnrichButton.disabled = true;
  semanticCancelButton.disabled = true;
  semanticWorkFilterButton.disabled = true;
  semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(t("semantic.running"))}</p>`;
  if (state.semanticEvents) {
    state.semanticEvents.close();
    state.semanticEvents = null;
  }

  try {
    const response = await apiFetch("/api/semantic-jobs", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    const job = await response.json();
    if (requestId !== state.semanticEnrichRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(job, response, "semantic enrichment failed"));
    }

    state.semanticJobId = job.id;
    semanticCancelButton.disabled = false;
    loadJobQueue();
    await watchSemanticJob(job.id, requestId);
  } catch (error) {
    if (requestId !== state.semanticEnrichRequest) return;
    setStatus("error", "error");
    semanticWorkList.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.semanticEnrichRequest) {
      semanticEnrichButton.disabled = false;
      semanticCancelButton.disabled = true;
      semanticWorkFilterButton.disabled = false;
      loadJobQueue();
    }
  }
}

async function cancelSemanticJob() {
  const jobId = state.semanticJobId;
  if (!jobId) return;

  semanticCancelButton.disabled = true;
  try {
    const response = await apiFetch(`/api/semantic-jobs/${encodeURIComponent(jobId)}`, { method: "DELETE" });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "failed to cancel semantic enrichment"));
    }
    state.semanticJobId = null;
    if (state.semanticEvents) {
      state.semanticEvents.close();
      state.semanticEvents = null;
    }
    setStatus("ready");
    semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(t("job.semanticCanceled"))}</p>`;
    semanticEnrichButton.disabled = false;
    semanticWorkFilterButton.disabled = false;
    loadJobQueue();
  } catch (error) {
    setStatus("error", "error");
    semanticWorkList.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    semanticCancelButton.disabled = false;
  }
}

async function watchSemanticJob(jobId, requestId) {
  if (!window.EventSource) {
    return pollSemanticJob(jobId, requestId);
  }

  return new Promise((resolve, reject) => {
    let settled = false;
    const events = authenticatedEventSource(`/api/semantic-jobs/${encodeURIComponent(jobId)}/events`);
    state.semanticEvents = events;

    const finish = async () => {
      if (settled) return;
      settled = true;
      events.close();
      if (state.semanticEvents === events) state.semanticEvents = null;
      try {
        await loadSemanticJobResult(jobId, requestId);
        resolve();
      } catch (error) {
        reject(error);
      }
    };

    events.addEventListener("status", (event) => {
      if (state.semanticJobId !== jobId || requestId !== state.semanticEnrichRequest) {
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
        if (state.semanticEvents === events) state.semanticEvents = null;
        reject(new Error(`invalid semantic event: ${error.message}`));
        return;
      }
      if (job.status === "queued" || job.status === "running") {
        setStatus("semantic", "busy");
        semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(job.message || t("semantic.running"))}</p>`;
        return;
      }
      if (job.status === "failed") {
        settled = true;
        events.close();
        if (state.semanticEvents === events) state.semanticEvents = null;
        reject(new Error(job.message || "semantic enrichment failed"));
        return;
      }
      if (job.status === "canceled") {
        settled = true;
        events.close();
        if (state.semanticEvents === events) state.semanticEvents = null;
        if (state.semanticJobId === jobId) state.semanticJobId = null;
        setStatus("ready");
        semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(t("job.semanticCanceled"))}</p>`;
        resolve();
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
      if (state.semanticEvents === events) state.semanticEvents = null;
      pollSemanticJob(jobId, requestId).then(resolve, reject);
    };
  });
}

async function pollSemanticJob(jobId, requestId) {
  while (state.semanticJobId === jobId && requestId === state.semanticEnrichRequest) {
    const response = await apiFetch(`/api/semantic-jobs/${encodeURIComponent(jobId)}`);
    const job = await response.json();
    if (!response.ok) {
      throw new Error(apiErrorMessage(job, response, "semantic status failed"));
    }
    if (job.status === "queued" || job.status === "running") {
      setStatus("semantic", "busy");
      semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(job.message || t("semantic.running"))}</p>`;
      await sleep(350);
      continue;
    }
    if (job.status === "failed") {
      throw new Error(job.message || "semantic enrichment failed");
    }
    if (job.status === "canceled") {
      if (state.semanticJobId === jobId) state.semanticJobId = null;
      setStatus("ready");
      semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(t("job.semanticCanceled"))}</p>`;
      return;
    }
    await loadSemanticJobResult(jobId, requestId);
    return;
  }
}

async function loadSemanticJobResult(jobId, requestId) {
  const response = await apiFetch(`/api/semantic-jobs/${encodeURIComponent(jobId)}/result`);
  const body = await response.json();
  if (requestId !== state.semanticEnrichRequest || state.semanticJobId !== jobId) return;
  if (!response.ok) {
    throw new Error(apiErrorMessage(body, response, "semantic result failed"));
  }
  applySemanticEnrichResult(body.result, body.root || pathInput.value.trim() || ".");
}

function applySemanticEnrichResult(result, root) {
  state.graph = result.graph || { nodes: [], edges: [] };
  state.summary = result.summary || null;
  state.graphPage.root = root;
  state.graphPage.nodeOffset = 0;
  state.graphPage.edgeOffset = 0;
  state.graphPage.totalNodes = state.graph.nodes.length;
  state.graphPage.totalEdges = state.graph.edges.length;
  state.graphPage.truncatedNodes = false;
  state.graphPage.truncatedEdges = false;
  clearSelection({ render: false });
  state.edgeSelectionCache.clear();
  state.edgeSelectionNodeCache.clear();
  state.hoveredId = null;
  state.hoveredEdgeKey = null;
  state.queryFocus = null;
  state.insightReport = null;
  queryResult.innerHTML = "";
  checkResult.innerHTML = "";
  rootLabel.textContent = state.graphPage.root;
  initializeGraph({ preserveView: false });
  updateGraphPageControls();
  renderOverview();
  renderSemanticEnrichReport(result);
  setStatus("ready");
}

function renderSemanticEnrichReport(result) {
  const report = result.report || {};
  const semanticCache = result.semantic_cache || {};
  semanticWorkList.innerHTML = `
    <div class="semantic-work-summary">
      <strong>${escapeHtml(t("semantic.report"))}</strong>
      <span>${Number(result.responses || 0)} ${escapeHtml(t("semantic.responses"))}</span>
      <span>${escapeHtml(t("semantic.cache"))}: ${escapeHtml(formatKind(semanticCache.status || "unknown"))}</span>
    </div>
    <div class="semantic-enrich-report">
      <div><span>${escapeHtml(t("semantic.edges"))}</span><strong>${Number(report.semantic_edges || 0)}</strong></div>
      <div><span>${escapeHtml(t("semantic.replaced"))}</span><strong>${Number(report.replaced_edges || 0)}</strong></div>
      <div><span>${escapeHtml(t("semantic.added"))}</span><strong>${Number(report.added_edges || 0)}</strong></div>
      <div><span>${escapeHtml(t("semantic.diagnostics"))}</span><strong>${Number(report.diagnostic_nodes || 0)}</strong></div>
      <div><span>${escapeHtml(t("semantic.errors"))}</span><strong>${Number(result.response_errors || 0)}</strong></div>
      <div><span>${escapeHtml(t("semantic.unmatched"))}</span><strong>${Number(result.unmatched_locations || 0)}</strong></div>
    </div>
  `;
}
