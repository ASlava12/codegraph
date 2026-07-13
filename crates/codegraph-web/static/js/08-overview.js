// 08-overview.js — project overview and semantic work queue rendering.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function renderOverview() {
  const summary = state.summary;
  const entrypoints = state.entrypoints || [];
  const nodesLabel = t("stat.nodes").toLowerCase();
  const edgesLabel = t("stat.edges").toLowerCase();

  overviewTotals.textContent = summary
    ? `${summary.nodes} ${nodesLabel} · ${summary.edges} ${edgesLabel}`
    : `0 ${nodesLabel}`;
  skippedCount.textContent = String(summary?.skipped_files || 0);

  renderCapabilities(state.capabilities);

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
      : `<p class="empty">${escapeHtml(t("empty.noLanguages"))}</p>`;

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
      : `<p class="empty">${escapeHtml(t("empty.noEdgeConfidence"))}</p>`;

  const relations = Object.entries(summary?.edge_relations || {})
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 6);
  relationList.innerHTML =
    relations.length > 0
      ? relations.map(([relation, count]) => renderOverviewChip("relation", relation, count)).join("")
      : `<p class="empty">${escapeHtml(t("empty.noEdgeRelations"))}</p>`;

  const edgeSources = Object.entries(summary?.edge_sources || {})
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, 6);
  edgeSourceList.innerHTML =
    edgeSources.length > 0
      ? edgeSources.map(([source, count]) => renderOverviewChip("edge-source", source, count)).join("")
      : `<p class="empty">${escapeHtml(t("empty.noEdgeSources"))}</p>`;

  renderScanPolicy(state.scanOptions);
  renderCoverage(state.coverage);
  renderRiskSummary(state.report?.risk_summary, state.report?.quality_gate);
  renderLspStatus(state.lsp, state.semanticReadiness, state.semanticPlan);
  renderSemanticWorkFilterOptions(summary);
  renderSemanticWork(state.semanticPlan);
  renderArchitecture(state.architecture);
  renderLanguageDependencies(state.languageDependencies);
  renderSurprisingLinks(state.report?.surprising_links);
  renderCommunities(state.communities);
  renderHotspots(state.hotspots);

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
      : `<p class="empty">${escapeHtml(t("empty.noAnnotations"))}</p>`;

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
      : `<p class="empty">${escapeHtml(t("empty.noEntrypoints"))}</p>`;

  languageList.querySelectorAll("[data-language]").forEach((button) => {
    button.addEventListener("click", () => {
      serverKindInput.value = "";
      serverItemKindInput.value = "";
      serverLanguageInput.value = button.dataset.language || "";
      serverSearchInput.value = "";
      serverEdgeKindInput.value = "";
      serverConfidenceInput.value = "";
      serverEdgeRelationInput.value = "";
      serverEdgeSourceInput.value = "";
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

  relationList.querySelectorAll("[data-relation]").forEach((button) => {
    button.addEventListener("click", () => {
      const relation = button.dataset.relation || "";
      if (!relation) return;
      serverEdgeRelationInput.value = relation;
      serverEdgeSourceInput.value = "";
      loadGraphPage({ resetPage: true, resetLayout: true });
    });
  });

  edgeSourceList.querySelectorAll("[data-edge-source]").forEach((button) => {
    button.addEventListener("click", () => {
      const source = button.dataset.edgeSource || "";
      if (!source) return;
      serverEdgeSourceInput.value = source;
      serverEdgeRelationInput.value = "";
      loadGraphPage({ resetPage: true, resetLayout: true });
    });
  });

  entrypointList.querySelectorAll("[data-node-id]").forEach((button) => {
    button.addEventListener("click", () => {
      focusNodeId(Number(button.dataset.nodeId), t("focus.entrypoint"));
    });
  });

  // Keep the Flow entrypoint picker in sync with the loaded entrypoints.
  populateFlowEntrypointPicker();
}

function renderOverviewChip(kind, value, count) {
  const dataset = kind === "relation" ? "data-relation" : "data-edge-source";
  return `
    <button class="${kind}-chip" type="button" ${dataset}="${escapeHtml(value)}">
      <span>${escapeHtml(formatKind(value))}</span>
      <strong>${count}</strong>
    </button>
  `;
}

function renderCapabilities(capabilities) {
  if (!capabilities) {
    capabilitiesList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noCapabilities"))}</p>`;
    return;
  }

  const endpoints = Array.isArray(capabilities.endpoints) ? capabilities.endpoints : [];
  const languages = Array.isArray(capabilities.languages) ? capabilities.languages : [];
  const exportFormats = Array.isArray(capabilities.export_formats) ? capabilities.export_formats : [];
  const projects = Array.isArray(capabilities.projects) ? capabilities.projects : [];
  const limits = capabilities.limits || {};
  const cache = capabilities.cache || {};
  const commonHeaders = Array.isArray(state.apiSchema?.common_response_headers)
    ? state.apiSchema.common_response_headers
    : [];
  if (Number(limits.max_graph_query_length || 0) > 0) {
    queryInput.maxLength = String(Number(limits.max_graph_query_length));
    askInput.maxLength = String(Number(limits.max_graph_query_length));
  }
  if (Number(limits.max_source_search_query_length || 0) > 0) {
    sourceSearchInput.maxLength = String(Number(limits.max_source_search_query_length));
  }
  const chips = [
    [t("cap.server"), capabilities.server_version || "unknown"],
    [t("cap.api"), `v${Number(capabilities.api_version || 0)}`],
    [t("cap.graph"), `v${Number(capabilities.graph_schema_version || 0)}`],
    [t("cap.cache"), cache.enabled ? t("cap.on") : t("cap.off")],
    [t("cap.languages"), String(languages.length)],
    [t("cap.exports"), String(exportFormats.length)],
    [t("cap.projects"), String(projects.length)],
    [t("cap.scanJobs"), `${Number(limits.max_scan_concurrency || 0)}/${Number(limits.max_scan_jobs || 0)}`],
    [t("cap.semanticJobs"), `${Number(limits.max_semantic_concurrency || 0)}/${Number(limits.max_semantic_jobs || 0)}`],
    [t("cap.apiBody"), formatBytes(Number(limits.max_api_body_bytes || 0))],
    [t("cap.semanticWork"), String(Number(limits.max_semantic_work_item_limit || 0))],
    [t("cap.semanticTimeout"), String(Number(limits.max_semantic_request_timeout_ms || 0))],
    [t("cap.graphPage"), `${Number(limits.max_graph_node_limit || 0)}/${Number(limits.max_graph_edge_limit || 0)}`],
    [t("cap.nodeCard"), String(Number(limits.max_node_context_edge_limit || 0))],
    [t("cap.focus"), String(Number(limits.max_focus_edge_limit || 0))],
    [t("cap.queryLimit"), String(Number(limits.max_graph_query_limit || 0))],
    [t("cap.querySize"), String(Number(limits.max_graph_query_length || 0))],
    [t("cap.headers"), String(commonHeaders.length)],
    [
      t("cap.report"),
      [
        limits.max_report_architecture_group_limit,
        limits.max_report_architecture_edge_limit,
        limits.max_report_language_link_limit,
        limits.max_report_hotspot_limit,
        limits.max_report_community_limit,
        limits.max_report_insight_limit,
      ]
        .map((value) => String(Number(value || 0)))
        .join("/"),
    ],
    [t("cap.sourceSearchSize"), String(Number(limits.max_source_search_query_length || 0))],
    [t("cap.routes"), String(endpoints.length)],
  ];

  capabilitiesList.innerHTML = chips
    .map(
      ([label, value]) => `
        <div class="capability-chip">
          <span>${escapeHtml(label)}</span>
          <strong>${escapeHtml(value)}</strong>
        </div>
      `,
    )
    .join("");
}

function renderScanPolicy(options) {
  if (!options) {
    scanPolicyList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noScanPolicy"))}</p>`;
    return;
  }

  const ignoredNames = Array.isArray(options.ignored_names) ? options.ignored_names : [];
  const ignoredGlobs = Array.isArray(options.ignored_globs) ? options.ignored_globs : [];
  const chips = [
    [t("overview.maxFile"), formatBytes(Number(options.max_file_size || 0))],
    [t("overview.policy"), options.config_path ? ".codegraph" : t("overview.defaults")],
    [t("overview.ignoreNames"), String(ignoredNames.length)],
    [t("overview.ignoreGlobs"), String(ignoredGlobs.length)],
    [t("overview.hidden"), options.include_hidden ? t("overview.yes") : t("overview.no")],
    [t("overview.gitIgnored"), options.include_ignored ? t("overview.yes") : t("overview.no")],
  ];

  scanPolicyList.innerHTML = chips
    .map(
      ([label, value]) => `
        <div class="scan-policy-chip">
          <span>${escapeHtml(label)}</span>
          <strong>${escapeHtml(value)}</strong>
        </div>
      `,
    )
    .join("");
}

function renderCoverage(coverage) {
  if (!coverage) {
    coverageList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noCoverage"))}</p>`;
    return;
  }

  const chips = [
    [t("overview.indexed"), String(coverage.indexed_files || 0)],
    [t("overview.large"), String(coverage.skipped_large_files || 0)],
    [t("overview.policySkipped"), String(coverage.skipped_policy_entries || 0)],
    [t("overview.otherFiles"), String(coverage.non_index_files || 0)],
    [t("overview.indexedBytes"), formatBytes(Number(coverage.indexed_bytes || 0))],
  ];

  coverageList.innerHTML = chips
    .map(
      ([label, value]) => `
        <div class="coverage-chip">
          <span>${escapeHtml(label)}</span>
          <strong>${escapeHtml(value)}</strong>
        </div>
      `,
    )
    .join("");
}

function renderRiskSummary(risk, qualityGate = null) {
  if (!risk) {
    riskSummaryList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noInsights"))}</p>`;
    return;
  }

  const grade = String(risk.grade || "clean");
  const gate = qualityGate || {};
  const gateStatus = gate.passed === false ? "failed" : "passed";
  const gateValue =
    gate.passed === false
      ? `${Number(gate.failing_insights || 0)} ${formatKind(gate.fail_on || "error")}`
      : formatKind(gate.fail_on || "error");
  const chips = [
    riskGateChip(t("risk.gate"), gateValue, gateStatus, gate.fail_on || "error"),
    riskChip(t("risk.score"), formatCompactNumber(risk.score), `grade-${grade}`),
    riskChip(t("risk.grade"), formatKind(grade), `grade-${grade}`),
    riskChip(t("risk.errors"), Number(risk.errors || 0), "error", "error"),
    riskChip(t("risk.warnings"), Number(risk.warnings || 0), "warning", "warning"),
    riskChip(t("risk.infos"), Number(risk.infos || 0), "info", "info"),
  ];
  const topKinds = Array.isArray(risk.top_kinds) ? risk.top_kinds.slice(0, 5) : [];
  topKinds.forEach((item) => {
    chips.push(riskKindChip(item));
  });
  if (Number(risk.total || 0) === 0) {
    chips.push(riskChip(t("risk.clean"), 0, "clean"));
  }
  riskSummaryList.innerHTML = chips.join("");
  riskSummaryList.querySelectorAll("[data-risk-severity]").forEach((button) => {
    button.addEventListener("click", () => {
      const severity = button.dataset.riskSeverity || "";
      insightSeverityInput.value =
        insightSeverityInput.value.trim().toLowerCase() === severity ? "" : severity;
      insightKindInput.value = "";
      loadInsights();
    });
  });
  riskSummaryList.querySelectorAll("[data-risk-kind]").forEach((button) => {
    button.addEventListener("click", () => {
      insightKindInput.value = button.dataset.riskKind || "";
      insightSeverityInput.value = "";
      loadInsights();
    });
  });
  riskSummaryList.querySelectorAll("[data-risk-gate]").forEach((button) => {
    button.addEventListener("click", () => {
      checkFailOnInput.value = button.dataset.riskGate || "error";
      runCheck();
      checkResult.scrollIntoView({ block: "nearest" });
    });
  });
}

function riskChip(label, value, status, severity = "") {
  const dataset = severity ? `data-risk-severity="${escapeHtml(severity)}"` : "";
  const tag = severity ? "button" : "div";
  const type = severity ? 'type="button"' : "";
  return `
    <${tag} class="risk-summary-chip ${escapeHtml(status || "")}" ${type} ${dataset}>
      <span>${escapeHtml(label)}</span>
      <strong>${escapeHtml(String(value))}</strong>
    </${tag}>
  `;
}

function riskGateChip(label, value, status, failOn) {
  return `
    <button class="risk-summary-chip ${escapeHtml(status || "")}" type="button" data-risk-gate="${escapeHtml(failOn)}">
      <span>${escapeHtml(label)}</span>
      <strong>${escapeHtml(String(value))}</strong>
    </button>
  `;
}

function riskKindChip(item) {
  const kind = String(item.kind || "");
  const severity = String(item.severity || "info");
  return `
    <button class="risk-summary-chip ${escapeHtml(severity)}" type="button" data-risk-kind="${escapeHtml(kind)}">
      <span>${escapeHtml(formatKind(kind))}</span>
      <strong>${Number(item.count || 0)}</strong>
    </button>
  `;
}

function renderLspStatus(report, readiness, plan) {
  if (!report && !readiness && !plan) {
    lspList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noLspStatus"))}</p>`;
    return;
  }

  const servers = Array.isArray(report?.servers) ? report.servers : [];
  const chips = servers.slice(0, 8).map(
    (server) => `
      <div class="lsp-chip ${server.installed ? "available" : "missing"}">
        <span>${escapeHtml(server.id || "lsp")}</span>
        <strong>${escapeHtml(server.installed ? t("option.ready") : t("option.missing"))}</strong>
      </div>
    `,
  );
  chips.unshift(`
    <div class="lsp-chip">
      <span>${escapeHtml(t("semantic.coverage"))}</span>
      <strong>${Number(readiness?.covered_languages ?? report?.available_servers ?? 0)}/${Number(readiness?.total_languages ?? report?.total_servers ?? 0)}</strong>
    </div>
  `);
  if (readiness) {
    chips.splice(
      1,
      0,
      `
        <div class="lsp-chip ${Number(readiness.missing_languages || 0) === 0 ? "available" : "missing"}">
          <span>${escapeHtml(t("semantic.missing"))}</span>
          <strong>${Number(readiness.missing_languages || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>${escapeHtml(t("semantic.candidates"))}</span>
          <strong>${Number(readiness.semantic_candidate_nodes || 0)}</strong>
        </div>
      `,
    );
  }
  if (plan) {
    chips.splice(
      3,
      0,
      `
        <div class="lsp-chip ${Number(plan.blocked_languages || 0) === 0 ? "available" : "missing"}">
          <span>${escapeHtml(t("semantic.plan"))}</span>
          <strong>${Number(plan.ready_languages || 0)}/${Number(plan.total_languages || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>${escapeHtml(t("semantic.definitions"))}</span>
          <strong>${Number(plan.planned_requests?.definitions || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>${escapeHtml(t("semantic.diagnostics"))}</span>
          <strong>${Number(plan.planned_requests?.diagnostics || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>${escapeHtml(t("semantic.symbols"))}</span>
          <strong>${Number(plan.planned_requests?.document_symbols || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>${escapeHtml(t("semantic.references"))}</span>
          <strong>${Number(plan.planned_requests?.references || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>${escapeHtml(t("semantic.workspace"))}</span>
          <strong>${Number(plan.planned_requests?.workspace_symbols || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>${escapeHtml(t("semantic.workQueue"))}</span>
          <strong>${Number(plan.work_items?.length || 0)}/${Number(plan.total_work_items || 0)}</strong>
        </div>
      `,
    );
  }
  const missingServers = Array.isArray(readiness?.missing_servers)
    ? readiness.missing_servers.slice(0, 4)
    : [];
  missingServers.forEach((server) => {
    chips.push(`
      <div class="lsp-chip missing">
        <span>${escapeHtml(server)}</span>
        <strong>${escapeHtml(t("semantic.needed"))}</strong>
      </div>
    `);
  });
  const uncoveredLanguages = Array.isArray(readiness?.languages)
    ? readiness.languages.filter((language) => !language.server).slice(0, 4)
    : [];
  uncoveredLanguages.forEach((language) => {
    chips.push(`
      <div class="lsp-chip missing">
        <span>${escapeHtml(language.language || "language")}</span>
        <strong>${escapeHtml(t("semantic.noServer"))}</strong>
      </div>
    `);
  });
  const languagePlans = Array.isArray(plan?.languages) ? plan.languages.slice(0, 6) : [];
  languagePlans.forEach((language) => {
    const requests = language.planned_requests || {};
    const requestCount =
      Number(requests.document_symbols || 0) +
      Number(requests.workspace_symbols || 0) +
      Number(requests.definitions || 0) +
      Number(requests.references || 0) +
      Number(requests.diagnostics || 0);
    const status = language.status === "ready" ? "available" : "missing";
    chips.push(`
      <div class="lsp-chip ${status}">
        <span>${escapeHtml(language.language || "language")}</span>
        <strong>${language.status === "ready" ? `${requestCount} ${escapeHtml(t("semantic.ops"))}` : escapeHtml(formatKind(language.status || "blocked"))}</strong>
      </div>
    `);
  });
  lspList.innerHTML = chips.join("");
}

function renderSemanticWork(plan) {
  const items = Array.isArray(plan?.work_items) ? plan.work_items.slice(0, 8) : [];
  if (items.length === 0) {
    semanticWorkList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noSemanticWork"))}</p>`;
    return;
  }

  const filter = renderSemanticWorkFilterLabel(plan.work_item_filter);
  const truncated = plan.truncated_work_items
    ? `<span>${items.length}/${Number(plan.total_work_items || items.length)} ${escapeHtml(t("overview.shown"))}</span>`
    : `<span>${items.length} ${escapeHtml(t("overview.queued"))}</span>`;
  semanticWorkList.innerHTML = `
    <div class="semantic-work-summary">
      <strong>${escapeHtml(t("semantic.workQueue"))}</strong>
      <span>${filter}${truncated}</span>
    </div>
    <ul class="semantic-work-items">
      ${items.map(renderSemanticWorkItem).join("")}
    </ul>
  `;
  semanticWorkList.querySelectorAll("[data-semantic-work-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const item = items[Number(button.dataset.semanticWorkIndex)];
      if (item) focusSemanticWorkItem(item);
    });
  });
}

function renderSemanticWorkFilterOptions(summary) {
  const current = semanticWorkLanguageInput.value;
  const languages = Object.keys(summary?.languages || {}).sort((left, right) => left.localeCompare(right));
  semanticWorkLanguageInput.innerHTML = [
    `<option value="">${escapeHtml(t("option.any"))}</option>`,
    ...languages.map(
      (language) =>
        `<option value="${escapeHtml(language)}" ${language === current ? "selected" : ""}>${escapeHtml(language)}</option>`,
    ),
  ].join("");
  if (current && !languages.includes(current)) {
    semanticWorkLanguageInput.insertAdjacentHTML(
      "beforeend",
      `<option value="${escapeHtml(current)}" selected>${escapeHtml(current)}</option>`,
    );
  }
}

function renderSemanticWorkFilterLabel(filter) {
  const labels = [];
  if (filter?.language) labels.push(filter.language);
  if (filter?.status) labels.push(formatKind(filter.status));
  if (filter?.capability) labels.push(formatKind(filter.capability));
  return labels.length > 0 ? `${escapeHtml(labels.join(" / "))} · ` : "";
}

function renderSemanticWorkItem(item, index) {
  const target = item.target?.label
    ? ` -> ${item.target.label}`
    : item.node?.label
      ? ` ${item.node.label}`
      : "";
  const location = item.path ? ` · ${item.path}${item.line ? `:${item.line}` : ""}` : "";
  const disabled = item.edge_index == null && !item.node?.id ? "disabled" : "";
  return `
    <li>
      <button class="semantic-work-item" type="button" data-semantic-work-index="${index}" ${disabled}>
        <span>${Number(item.priority || 100)}</span>
        <strong>${escapeHtml(formatKind(item.capability || item.kind))}${escapeHtml(target)}</strong>
        <em>${escapeHtml(item.reason || formatKind(item.status || "work"))}${escapeHtml(location)}</em>
      </button>
    </li>
  `;
}

async function focusSemanticWorkItem(item) {
  const nodeIds = [];
  if (item.node?.id) nodeIds.push(item.node.id);
  if (item.target?.id) nodeIds.push(item.target.id);
  const edgeIndexes = item.edge_index == null ? [] : [item.edge_index];
  if (nodeIds.length === 0 && edgeIndexes.length === 0) return;

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
    const selectedId = item.node?.id ?? item.target?.id ?? null;
    showFocusedGraph(
      body,
      t("focus.semantic", { label: formatKind(item.capability || item.kind) }),
      selectedId,
    );
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderArchitecture(architecture) {
  if (!architecture) {
    architectureList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noArchitecture"))}</p>`;
    return;
  }

  const groups = Array.isArray(architecture.groups) ? architecture.groups.slice(0, 8) : [];
  const groupChips = groups.map(
    (group) => `
      <button class="architecture-chip" type="button" data-architecture-prefix="${escapeHtml(group.id || "")}">
        <span>${escapeHtml(group.label || group.id || "root")}</span>
        <strong>${Number(group.files || 0)}f/${Number(group.symbols || 0)}s</strong>
      </button>
    `,
  );
  if (state.architecturePathPrefix) {
    groupChips.unshift(`
      <button class="architecture-chip" type="button" data-architecture-prefix="">
        <span>${escapeHtml(t("overview.allAreas"))}</span>
        <strong>${escapeHtml(t("overview.reset"))}</strong>
      </button>
    `);
  }
  groupChips.push(`
    <div class="architecture-chip">
      <span>${escapeHtml(t("overview.areaEdges"))}</span>
      <strong>${Number(architecture.total_edges || 0)}</strong>
    </div>
  `);
  const edgeChips = (Array.isArray(architecture.edges) ? architecture.edges.slice(0, 6) : [])
    .map((edge, index) => ({ edge, index }))
    .filter(({ edge }) => Array.isArray(edge.edge_indexes) && edge.edge_indexes.length > 0)
    .map(
      ({ edge, index }) => `
        <button class="architecture-edge-chip" type="button" data-architecture-edge-index="${index}">
          <span>${escapeHtml(edge.source || "root")} -> ${escapeHtml(edge.target || "root")}</span>
          <strong>${Number(edge.count || 0)}</strong>
        </button>
      `,
    );
  architectureList.innerHTML = groupChips.join("");
  if (edgeChips.length > 0) {
    architectureList.insertAdjacentHTML("beforeend", edgeChips.join(""));
  }
  architectureList.querySelectorAll("[data-architecture-prefix]").forEach((button) => {
    button.addEventListener("click", () => {
      state.architecturePathPrefix = button.dataset.architecturePrefix || "";
      loadGraphPage({ resetPage: true, resetLayout: true });
    });
  });
  architectureList.querySelectorAll("[data-architecture-edge-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const edge = architecture.edges?.[Number(button.dataset.architectureEdgeIndex)];
      if (!edge) return;
      focusArchitectureEdge(edge);
    });
  });
}

async function focusArchitectureEdge(edge) {
  const edgeIndexes = Array.isArray(edge.edge_indexes) ? edge.edge_indexes : [];
  if (edgeIndexes.length === 0) return;
  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    edge_indexes: edgeIndexes.join(","),
    edge_limit: "300",
  });

  try {
    const response = await apiFetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "focus failed"));
    }
    showFocusedGraph(
      body,
      t("focus.architectureEdge", {
        source: edge.source || "area",
        target: edge.target || "area",
      }),
    );
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderLanguageDependencies(report) {
  if (!report) {
    languageDependencyList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noLanguageDependencies"))}</p>`;
    return;
  }

  const links = Array.isArray(report.links) ? report.links.slice(0, 8) : [];
  const chips = links.map(
    (link, index) => `
      <button class="language-dependency-chip" type="button" data-language-dependency-index="${index}">
        <span>${escapeHtml(link.source_language || formatKind("unknown"))} -> ${escapeHtml(link.target_language || formatKind("unknown"))}</span>
        <strong>${Number(link.count || 0)}</strong>
      </button>
    `,
  );
  chips.unshift(`
    <div class="language-dependency-chip">
      <span>${escapeHtml(t("overview.crossLanguage"))}</span>
      <strong>${Number(report.cross_language_edges || 0)}</strong>
    </div>
  `);
  languageDependencyList.innerHTML = chips.join("");
  languageDependencyList.querySelectorAll("[data-language-dependency-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const link = report.links?.[Number(button.dataset.languageDependencyIndex)];
      if (!link) return;
      focusLanguageDependency(link);
    });
  });
}

async function focusLanguageDependency(link) {
  const edgeIndexes = Array.isArray(link.edge_indexes) ? link.edge_indexes : [];
  if (edgeIndexes.length === 0) return;
  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    edge_indexes: edgeIndexes.join(","),
    edge_limit: "300",
  });

  try {
    const response = await apiFetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "focus failed"));
    }
    showFocusedGraph(
      body,
      t("focus.languageDependency", {
        source: link.source_language || formatKind("unknown"),
        target: link.target_language || formatKind("unknown"),
      }),
    );
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderSurprisingLinks(report) {
  if (!report) {
    surprisingLinkList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noSurprisingLinks"))}</p>`;
    return;
  }
  const links = Array.isArray(report.links) ? report.links.slice(0, 6) : [];
  surprisingLinkList.innerHTML =
    links.length > 0
      ? links
          .map(
            (link, index) => `
              <button class="surprising-link-chip" type="button" data-surprising-link-index="${index}">
                <span>${escapeHtml(link.source_area || "area")} -> ${escapeHtml(link.target_area || "area")}</span>
                <strong>${Number(link.score || 0)}</strong>
              </button>
            `,
          )
          .join("")
      : `<p class="empty">${escapeHtml(t("empty.noSurprisingLinks"))}</p>`;
  surprisingLinkList.querySelectorAll("[data-surprising-link-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const link = report.links?.[Number(button.dataset.surprisingLinkIndex)];
      if (!link) return;
      focusSurprisingLink(link);
    });
  });
}

async function focusSurprisingLink(link) {
  if (!Number.isFinite(Number(link.edge_index))) return;
  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    edge_indexes: String(link.edge_index),
    edge_limit: "80",
  });

  try {
    const response = await apiFetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "focus failed"));
    }
    showFocusedGraph(body, t("focus.surprisingLink"));
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderCommunities(report) {
  if (!report) {
    communityList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noCommunities"))}</p>`;
    return;
  }

  const communities = Array.isArray(report.communities) ? report.communities.slice(0, 8) : [];
  const chips = communities.map(
    (community, index) => `
      <button class="community-chip" type="button" data-community-index="${index}">
        <span>${escapeHtml(community.label || community.id || "community")}</span>
        <strong>${Number(community.node_count || 0)}n/${Number(community.outgoing_external_edges || 0) + Number(community.incoming_external_edges || 0)}x</strong>
      </button>
    `,
  );
  chips.unshift(`
    <div class="community-chip">
      <span>${escapeHtml(t("overview.communities"))}</span>
      <strong>${Number(report.total_communities || 0)}</strong>
    </div>
  `);
  communityList.innerHTML =
    chips.length > 1 ? chips.join("") : `<p class="empty">${escapeHtml(t("empty.noCommunities"))}</p>`;
  communityList.querySelectorAll("[data-community-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const community = communities[Number(button.dataset.communityIndex)];
      if (!community) return;
      focusCommunity(community);
    });
  });
}

async function focusCommunity(community) {
  const edgeIndexes = Array.isArray(community.edge_indexes) ? community.edge_indexes : [];
  if (edgeIndexes.length === 0) return;
  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    edge_indexes: edgeIndexes.join(","),
    edge_limit: "300",
  });

  try {
    const response = await apiFetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "focus failed"));
    }
    showFocusedGraph(
      body,
      t("focus.community", {
        label: community.label || community.id || "community",
      }),
    );
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderHotspots(report) {
  if (!report) {
    hotspotList.innerHTML = `<p class="empty">${escapeHtml(t("empty.noHotspots"))}</p>`;
    return;
  }

  const hotspots = Array.isArray(report.architectural_hubs) && report.architectural_hubs.length > 0
    ? report.architectural_hubs.slice(0, 8)
    : Array.isArray(report.hotspots)
      ? report.hotspots.slice(0, 8)
      : [];
  hotspotList.innerHTML =
    hotspots.length > 0
      ? hotspots
          .map(
            (hotspot) => `
              <button class="hotspot-chip" type="button" data-hotspot-node-id="${hotspot.node?.id || ""}" title="${escapeHtml(hotspot.hub_kind || "hotspot")}">
                <span>${escapeHtml(hotspot.node?.label || "unknown")}</span>
                <strong>${Number(hotspot.score || 0)}</strong>
              </button>
            `,
          )
          .join("")
      : `<p class="empty">${escapeHtml(t("empty.noHotspots"))}</p>`;
  hotspotList.querySelectorAll("[data-hotspot-node-id]").forEach((button) => {
    button.addEventListener("click", () => {
      focusNodeId(Number(button.dataset.hotspotNodeId), t("focus.hotspot"));
    });
  });
}
