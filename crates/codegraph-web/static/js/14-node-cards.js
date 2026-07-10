// 14-node-cards.js — node metadata and card rendering with focused query actions.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function renderNodeMetadataRows(node) {
  const summaryKeys = new Set(["language", "item_kind"]);
  return Object.entries(node.metadata || {})
    .filter(([key, value]) => !summaryKeys.has(key) && value != null && String(value).length > 0)
    .sort((left, right) => left[0].localeCompare(right[0]))
    .map(([key, value]) => [formatKind(key), value]);
}

function renderDefinitionRow([key, value]) {
  return `<dt>${escapeHtml(key)}</dt><dd>${escapeHtml(String(value))}</dd>`;
}

function renderFileSummary(summary) {
  if (!summary) return "";
  const total =
    Number(summary.contained_nodes || 0) +
    Number(summary.imports || 0) +
    Number(summary.trace_edges || 0);
  if (total === 0) return "";
  const totals = [
    [t("selection.contained"), summary.contained_nodes],
    [t("selection.symbols"), summary.code_symbols],
    [t("selection.imports"), summary.imports],
    [t("selection.calls"), summary.calls],
    [t("selection.configReads"), summary.config_reads],
    [t("selection.environmentReads"), summary.environment_reads],
    [t("selection.errorFacts"), summary.error_facts],
    [t("selection.unresolvedCalls"), summary.unresolved_calls],
  ]
    .filter(([, count]) => Number(count || 0) > 0)
    .map(
      ([label, count]) =>
        `<span>${escapeHtml(label)} <strong>${Number(count || 0)}</strong></span>`,
    )
    .join("");
  const groups = [
    [t("selection.containedKinds"), summary.contained_kinds],
    [t("label.item"), summary.contained_item_kinds],
    [t("selection.traceFacts"), summary.trace_edge_kinds],
    [t("selection.traceTargets"), summary.trace_target_kinds],
  ]
    .map(([label, values]) => renderDependencyFacetGroup(label, values))
    .filter(Boolean)
    .join("");

  return `
    <div class="file-summary" aria-label="${escapeHtml(t("selection.fileSummary"))}">
      <div class="dependency-totals">${totals}</div>
      ${groups}
    </div>
  `;
}

function renderDependencySummary(summary) {
  if (!summary) return "";
  const total = Number(summary.incoming || 0) + Number(summary.outgoing || 0);
  if (total === 0) return "";
  const groups = [
    [t("selection.edgeKinds"), summary.edge_kinds],
    [t("selection.confidences"), summary.confidences],
    [t("selection.neighborKinds"), summary.neighbor_kinds],
    [t("selection.neighborLanguages"), summary.neighbor_languages],
  ]
    .map(([label, values]) => renderDependencyFacetGroup(label, values))
    .filter(Boolean)
    .join("");

  return `
    <div class="dependency-summary" aria-label="${escapeHtml(t("selection.dependencySummary"))}">
      <div class="dependency-totals">
        <span>${escapeHtml(t("selection.incoming"))}: <strong>${Number(summary.incoming || 0)}</strong></span>
        <span>${escapeHtml(t("selection.outgoing"))}: <strong>${Number(summary.outgoing || 0)}</strong></span>
      </div>
      ${groups}
    </div>
  `;
}

function renderDependencyFacetGroup(label, values) {
  const entries = Object.entries(values || {})
    .filter(([, count]) => Number(count) > 0)
    .sort((left, right) => Number(right[1]) - Number(left[1]) || left[0].localeCompare(right[0]))
    .slice(0, 4);
  if (entries.length === 0) return "";
  return `
    <section>
      <h4>${escapeHtml(label)}</h4>
      <div>${entries
        .map(
          ([key, count]) =>
            `<span><em>${escapeHtml(formatKind(key))}</em><strong>${Number(count)}</strong></span>`,
        )
        .join("")}</div>
    </section>
  `;
}

function renderNodeRiskSummary(summary) {
  if (!summary) return "";
  const severityTotal = Object.values(summary.by_severity || {}).reduce(
    (total, count) => total + Number(count || 0),
    0,
  );
  const kindTotal = Object.values(summary.by_kind || {}).reduce(
    (total, count) => total + Number(count || 0),
    0,
  );
  if (severityTotal + kindTotal === 0) return "";
  const severities = ["error", "warning", "info"]
    .filter((severity) => Number(summary.by_severity?.[severity] || 0) > 0)
    .map(
      (severity) =>
        `<span class="${escapeHtml(severity)}"><em>${escapeHtml(formatKind(severity))}</em><strong>${Number(summary.by_severity[severity] || 0)}</strong></span>`,
    )
    .join("");
  const kinds = renderDependencyFacetGroup(t("selection.riskKinds"), summary.by_kind);
  return `
    <div class="risk-summary" aria-label="${escapeHtml(t("selection.riskSummary"))}">
      ${severities ? `<div class="risk-severities">${severities}</div>` : ""}
      ${kinds}
    </div>
  `;
}

function nodeInsightsForNode(nodeId) {
  const insights = [...(state.insightReport?.insights || []), ...buildClientInsights(state.graph)];
  const seen = new Set();
  return insights.filter((insight) => {
    const ids = insightNodeIds(insight).map((id) => Number(id));
    if (!ids.includes(Number(nodeId))) return false;
    const key = `${insight.severity || ""}:${insight.kind || ""}:${insight.message || ""}:${ids.join(",")}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function renderNodeIssue(insight, index) {
  const severity = insight.severity || "info";
  return `
    <button class="node-issue ${escapeHtml(severity)}" type="button" data-node-issue-index="${index}">
      <span>${escapeHtml(formatKind(severity))} · ${escapeHtml(formatKind(insight.kind || "insight"))}</span>
      <strong>${escapeHtml(insight.message || "")}</strong>
      <em>${escapeHtml(t("selection.issueHint"))}</em>
    </button>
  `;
}

function nodeCardActions(card, node) {
  if (Array.isArray(card?.actions)) {
    return card.actions.filter((action) => action?.query);
  }
  return localNodeCardActions(node);
}

function localNodeCardActions(node) {
  return [
    fileGraphQueryForNode(node) && {
      kind: "file_graph",
      label: "File graph",
      query: fileGraphQueryForNode(node),
    },
    documentGraphQueryForNode(node) && {
      kind: "document_graph",
      label: "Document graph",
      query: documentGraphQueryForNode(node),
    },
    sqlGraphQueryForNode(node) && {
      kind: "sql_graph",
      label: "SQL graph",
      query: sqlGraphQueryForNode(node),
    },
    symbolGraphQueryForNode(node) && {
      kind: "symbol_graph",
      label: "Symbol graph",
      query: symbolGraphQueryForNode(node),
    },
    packageGraphQueryForNode(node) && {
      kind: "package_graph",
      label: "Package graph",
      query: packageGraphQueryForNode(node),
    },
    configGraphQueryForNode(node) && {
      kind: "config_graph",
      label: "Config graph",
      query: configGraphQueryForNode(node),
    },
    errorGraphQueryForNode(node) && {
      kind: "error_graph",
      label: "Error graph",
      query: errorGraphQueryForNode(node),
    },
  ].filter(Boolean);
}

function nodeCardActionLabel(action) {
  const labels = {
    file_graph: "selection.fileGraph",
    document_graph: "selection.documentGraph",
    sql_graph: "selection.sqlGraph",
    symbol_graph: "selection.symbolGraph",
    package_graph: "selection.packageGraph",
    config_graph: "selection.configGraph",
    error_graph: "selection.errorGraph",
  };
  const key = labels[action?.kind || ""];
  return key ? t(key) : action?.label || formatKind(action?.kind || "query");
}

function packageGraphQueryForNode(node) {
  if (node.kind !== "external_dependency") return null;
  const packageId = node.metadata?.package_id || "";
  if (packageId.includes(":")) {
    const [ecosystem, ...packageParts] = packageId.split(":");
    const packageName = packageParts.join(":");
    if (ecosystem && packageName) {
      return `packages package:${quoteQueryValue(packageName)} ecosystem:${quoteQueryValue(ecosystem)} edge_limit:300`;
    }
  }

  const candidate = importPackageCandidate(node.metadata?.language, node.label);
  if (!candidate) return null;
  return `packages package:${quoteQueryValue(candidate.package)} ecosystem:${quoteQueryValue(candidate.ecosystem)} edge_limit:300`;
}

function fileGraphQueryForNode(node) {
  if (node.kind !== "file") return null;
  return `files path:${quoteQueryValue(node.label)} direction:out edge_limit:300`;
}

function documentGraphQueryForNode(node) {
  const itemKind = node.metadata?.item_kind || "";
  const language = node.metadata?.language || "";
  if (!["document", "document_section"].includes(itemKind) && language !== "markdown") return null;
  return `docs node_id:${node.id} edge_limit:300`;
}

function sqlGraphQueryForNode(node) {
  const itemKind = node.metadata?.item_kind || "";
  const language = node.metadata?.language || "";
  const source = node.metadata?.source || "";
  if (
    language !== "sql" &&
    source !== "sql" &&
    !["sql_schema", "sql_table", "sql_column", "sql_index", "sql_view", "app_sql_query"].includes(itemKind)
  ) {
    return null;
  }
  return `sql node_id:${node.id} edge_limit:300`;
}

function symbolGraphQueryForNode(node) {
  if (!["function", "type", "module", "entrypoint"].includes(node.kind)) return null;
  return `symbols node_id:${node.id} direction:out edge_limit:300`;
}

function configGraphQueryForNode(node) {
  if (!["config", "environment"].includes(node.kind)) return null;
  return `configs node_id:${node.id} depth:6`;
}

function errorGraphQueryForNode(node) {
  if (node.metadata?.item_kind !== "error") return null;
  return `errors node_id:${node.id} depth:6`;
}

async function runNodeCardQuery(expression) {
  queryInput.value = expression;
  await runGraphQuery({ focus: true });
  queryResult.scrollIntoView({ block: "nearest" });
}

async function loadTrace(node) {
  state.traceRequest += 1;
  state.workflowRequest += 1;
  state.dependentsRequest += 1;
  clearLastWorkflowReport();
  const requestId = state.traceRequest;
  const target = document.querySelector("#traceResult");
  if (!target) return;

  target.innerHTML = `<p class="empty">${escapeHtml(t("trace.tracing"))}</p>`;
  const depthInput = document.querySelector("#traceDepthInput");
  const depth = clampNumber(Number(depthInput?.value || 3), 1, 8);
  if (depthInput) depthInput.value = String(depth);
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: String(depth),
  });

  try {
    const response = await apiFetch(`/api/trace?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.traceRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "trace failed"));
    }
    target.innerHTML = renderTrace(body);
    attachTraceNavigation(target);
    attachEdgeExplainActions(target);
  } catch (error) {
    if (requestId !== state.traceRequest) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function loadWorkflow(node) {
  state.traceRequest += 1;
  state.workflowRequest += 1;
  state.dependentsRequest += 1;
  clearLastWorkflowReport();
  const requestId = state.workflowRequest;
  const target = document.querySelector("#traceResult");
  if (!target) return;

  target.innerHTML = `<p class="empty">${escapeHtml(t("workflow.loading"))}</p>`;
  const depthInput = document.querySelector("#traceDepthInput");
  const depth = clampNumber(Number(depthInput?.value || 4), 1, 8);
  if (depthInput) depthInput.value = String(depth);
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: String(depth),
    block_limit: "120",
    compact: "true",
  });
  const workflowFilters = readWorkflowFilters("workflow");
  appendWorkflowFilterParams(params, workflowFilters);

  try {
    const response = await apiFetch(`/api/workflow?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.workflowRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "workflow failed"));
    }
    state.lastWorkflowReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      request: {
        node_id: node.id,
        label: node.label,
        depth,
        block_limit: 120,
        compact: true,
        ...workflowFilters,
      },
      report: body,
    };
    renderWorkflowExportState();
    target.innerHTML = renderWorkflow(body);
    attachWorkflowNavigation(target);
    attachEdgeExplainActions(target);
    attachFlowViewActions(
      target,
      () => body,
      () => node.label,
    );
  } catch (error) {
    if (requestId !== state.workflowRequest) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function loadDependents(node) {
  state.traceRequest += 1;
  state.workflowRequest += 1;
  state.dependentsRequest += 1;
  clearLastWorkflowReport();
  const requestId = state.dependentsRequest;
  const target = document.querySelector("#traceResult");
  if (!target) return;

  target.innerHTML = `<p class="empty">${escapeHtml(t("trace.tracingDependents"))}</p>`;
  const depthInput = document.querySelector("#traceDepthInput");
  const depth = clampNumber(Number(depthInput?.value || 3), 1, 16);
  if (depthInput) depthInput.value = String(depth);
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(node.id),
    depth: String(depth),
  });

  try {
    const response = await apiFetch(`/api/dependents?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.dependentsRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "dependents trace failed"));
    }
    target.innerHTML = renderTrace(body, {
      empty: t("trace.noDependents"),
      label: t("trace.dependents"),
    });
    attachTraceNavigation(target);
    attachEdgeExplainActions(target);
  } catch (error) {
    if (requestId !== state.dependentsRequest) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderWorkflowExportState() {
  const jsonButton = document.querySelector("#workflowJsonExportButton");
  const mermaidButton = document.querySelector("#workflowMermaidExportButton");
  const dotButton = document.querySelector("#workflowDotExportButton");
  if (jsonButton) jsonButton.disabled = !state.lastWorkflowReport;
  if (mermaidButton) mermaidButton.disabled = !state.lastWorkflowReport;
  if (dotButton) dotButton.disabled = !state.lastWorkflowReport;
}

function clearLastWorkflowReport() {
  state.lastWorkflowReport = null;
  renderWorkflowExportState();
}

function exportLastWorkflowReport(format) {
  if (!state.lastWorkflowReport) {
    const target = document.querySelector("#traceResult");
    if (target) {
      target.insertAdjacentHTML(
        "afterbegin",
        `<p class="empty" data-workflow-export-note>${escapeHtml(t("export.noWorkflow"))}</p>`,
      );
    }
    renderWorkflowExportState();
    return;
  }

  const payload = {
    schema: "codegraph.workflow.v1",
    ...state.lastWorkflowReport,
  };
  const root = safeFilePart(payload.root);
  const label = safeFilePart(payload.request?.label || payload.report?.start?.label || "workflow");
  if (format === "mermaid") {
    const mermaid = workflowReportToMermaid(payload.report);
    const blob = new Blob([mermaid], { type: "text/vnd.mermaid;charset=utf-8" });
    const fileName = `codegraph-${root}-${label}-workflow.mmd`;
    downloadBlob(blob, fileName);
    renderWorkflowExportNote(fileName, blob.size, t("export.workflowMermaid"));
    return;
  }
  if (format === "dot") {
    const dot = workflowReportToDot(payload.report);
    const blob = new Blob([dot], { type: "text/vnd.graphviz;charset=utf-8" });
    const fileName = `codegraph-${root}-${label}-workflow.dot`;
    downloadBlob(blob, fileName);
    renderWorkflowExportNote(fileName, blob.size, t("export.workflowDot"));
    return;
  }

  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${root}-${label}-workflow.json`;
  downloadBlob(blob, fileName);
  renderWorkflowExportNote(fileName, blob.size, t("export.workflow"));
}

function renderWorkflowExportNote(fileName, size, label) {
  const target = document.querySelector("#traceResult");
  if (!target) return;
  target.querySelector("[data-workflow-export-note]")?.remove();
  target.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary" data-workflow-export-note>
        <span>${escapeHtml(label)}</span>
        <span>${escapeHtml(formatBytes(size))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function workflowReportToMermaid(report) {
  const blocks = Array.isArray(report?.blocks) ? report.blocks : [];
  const transitions = Array.isArray(report?.transitions) ? report.transitions : [];
  const lines = ["flowchart TD"];
  blocks.forEach((block) => {
    const node = block.node || {};
    lines.push(
      `  ${workflowMermaidNodeId(block.id || node.id)}["${mermaidEscape(`${formatKind(block.kind || "unknown")}: ${node.label || node.id || ""}`)}"]`,
    );
  });
  transitions.forEach((transition) => {
    const edge = transition.edge || {};
    const source = transition.source || transition.source_node_id || edge.source;
    const target = transition.target || transition.target_node_id || edge.target;
    const label = `${formatKind(edge.kind || "unknown")}/${formatKind(edge.confidence || "unknown")}`;
    lines.push(
      `  ${workflowMermaidNodeId(source)} -->|${mermaidEscape(label)}| ${workflowMermaidNodeId(target)}`,
    );
  });
  return `${lines.join("\n")}\n`;
}

function entryWorkflowReportToMermaid(report) {
  const workflows = Array.isArray(report?.workflows) ? report.workflows : [];
  return workflows
    .map((workflow) => `%% ${mermaidEscape(workflow?.start?.label || "entrypoint")}\n${workflowReportToMermaid(workflow)}`)
    .join("\n");
}

function workflowReportToDot(report, graphName = "workflow") {
  const blocks = Array.isArray(report?.blocks) ? report.blocks : [];
  const transitions = Array.isArray(report?.transitions) ? report.transitions : [];
  const lines = [
    `digraph ${workflowDotId(graphName)} {`,
    "  graph [rankdir=TB, overlap=false, splines=true];",
    '  node [shape=box, style="rounded,filled", fillcolor="#20262a", color="#5cc8a7", fontname="Inter", fontcolor="#f2f4f3"];',
    '  edge [color="#6c7680", fontname="Inter", fontcolor="#a5adb3"];',
  ];

  blocks.forEach((block) => {
    const node = block.node || {};
    const parts = [
      formatKind(block.kind || "unknown"),
      node.label || node.id || "",
      node.span?.path ? `${node.span.path}:${node.span.start_line || 1}` : "",
    ].filter(Boolean);
    lines.push(
      `  ${workflowDotId(block.id || node.id)} [label="${dotEscape(parts.join("\\n"))}"];`,
    );
  });

  transitions.forEach((transition) => {
    const edge = transition.edge || {};
    const source = transition.source || transition.source_node_id || edge.source;
    const target = transition.target || transition.target_node_id || edge.target;
    if (source == null || target == null) return;
    const label = `${formatKind(edge.kind || "unknown")}/${formatKind(edge.confidence || "unknown")}`;
    lines.push(
      `  ${workflowDotId(source)} -> ${workflowDotId(target)} [label="${dotEscape(label)}"];`,
    );
  });

  lines.push("}");
  return `${lines.join("\n")}\n`;
}

function entryWorkflowReportToDot(report) {
  const workflows = Array.isArray(report?.workflows) ? report.workflows : [];
  const lines = [
    "digraph entrypoint_workflows {",
    "  graph [rankdir=TB, overlap=false, splines=true, compound=true];",
    '  node [shape=box, style="rounded,filled", fillcolor="#20262a", color="#5cc8a7", fontname="Inter", fontcolor="#f2f4f3"];',
    '  edge [color="#6c7680", fontname="Inter", fontcolor="#a5adb3"];',
  ];

  workflows.forEach((workflow, index) => {
    const clusterId = workflowDotId(`cluster_${index}`);
    lines.push(`  subgraph ${clusterId} {`);
    lines.push(`    label="${dotEscape(workflow?.start?.label || `entrypoint ${index + 1}`)}";`);
    lines.push('    color="#2f4f4a";');
    const dot = workflowReportToDot(workflow, `workflow_${index}`);
    dot
      .split(/\r?\n/)
      .slice(4, -1)
      .forEach((line) => lines.push(`  ${line}`));
    lines.push("  }");
  });

  lines.push("}");
  return `${lines.join("\n")}\n`;
}

function workflowMermaidNodeId(id) {
  return `B${String(id || "unknown").replace(/[^A-Za-z0-9_]/g, "_")}`;
}

function workflowDotId(id) {
  const normalized = String(id || "unknown").replace(/[^A-Za-z0-9_]/g, "_");
  return /^[A-Za-z_]/.test(normalized) ? normalized : `N${normalized}`;
}

function dotEscape(value) {
  return String(value)
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\r?\n/g, "\\n");
}

function mermaidEscape(value) {
  return String(value)
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\|/g, " ")
    .replace(/\r?\n/g, " ");
}

function renderWorkflow(report) {
  if (!report) {
    return `<p class="empty">${escapeHtml(t("trace.noStart"))}</p>`;
  }
  const blocks = Array.isArray(report.blocks) ? report.blocks : [];
  const transitions = Array.isArray(report.transitions) ? report.transitions : [];
  if (blocks.length === 0) {
    return `<p class="empty">${escapeHtml(t("workflow.noBlocks"))}</p>`;
  }

  const orderedBlocks = [...blocks].sort(
    (left, right) =>
      Number(left.depth || 0) - Number(right.depth || 0) ||
      String(left.node?.label || "").localeCompare(String(right.node?.label || "")),
  );
  const nodeMap = new Map();
  orderedBlocks.forEach((block) => {
    if (block.node?.id != null) nodeMap.set(block.node.id, block.node);
    if (block.id) nodeMap.set(block.id, block.node);
  });
  const blockRows = orderedBlocks.map((block) => renderWorkflowBlock(block)).join("");
  const transitionRows = transitions
    .map((transition) => renderWorkflowTransition(transition, nodeMap))
    .join("");
  const truncated = report.truncated
    ? `<p class="empty">${escapeHtml(t("workflow.truncated"))}</p>`
    : "";

  return `
    <div class="trace-summary">
      <span>${escapeHtml(t("workflow.blockCount", { count: formatNumber(blocks.length) }))}</span>
      <span>${escapeHtml(t("workflow.transitionCount", { count: formatNumber(transitions.length) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: formatNumber(report.max_depth || 0) }))}</span>
      ${renderWorkflowFilterSummary(report.filters)}
      <button type="button" class="flow-open" data-flow-view>${escapeHtml(t("flow.openView"))}</button>
    </div>
    <div class="workflow-diagram" aria-label="${escapeHtml(t("selection.flow"))}">
      <ol class="workflow-blocks">${blockRows}</ol>
    </div>
    ${
      transitionRows
        ? `<section class="workflow-transitions">
            <h3>${escapeHtml(t("label.edges"))}</h3>
            <ul class="trace-list trace-edge-list">${transitionRows}</ul>
          </section>`
        : ""
    }
    ${truncated}
  `;
}

function renderWorkflowBlock(block) {
  const node = block.node || {};
  const risks = Array.isArray(block.risk_refs) ? block.risk_refs : [];
  const compacted = block.compacted
    ? `<span class="workflow-risk-count">${escapeHtml(t("workflow.compacted", { count: formatNumber(block.compacted_count || 0) }))}</span>`
    : "";
  const riskSummary =
    risks.length > 0
      ? `<span class="workflow-risk-count">${escapeHtml(t("workflow.risks", { count: risks.length }))}</span>`
      : "";
  const riskRows = risks
    .slice(0, 3)
    .map((risk) => {
      const severity = risk.severity || "info";
      return `<span class="workflow-risk ${escapeHtml(severity)}">${escapeHtml(formatKind(severity))} · ${escapeHtml(formatKind(risk.kind || "insight"))}</span>`;
    })
    .join("");
  return `
    <li class="workflow-block ${escapeHtml(block.kind || "unknown")}" style="--depth:${Number(block.depth || 0)}">
      <button type="button" data-node-id="${node.id || ""}">
        <span>${escapeHtml(formatKind(block.kind || "unknown"))}</span>
        <strong>${escapeHtml(node.label || String(node.id || ""))}</strong>
        <em>#${escapeHtml(String(node.id || ""))}</em>
        ${compacted}
        ${riskSummary}
      </button>
      ${riskRows ? `<div class="workflow-risk-list">${riskRows}</div>` : ""}
    </li>
  `;
}

function renderWorkflowTransition(transition, nodeMap) {
  const edge = transition.edge || {};
  const source = nodeMap.get(transition.source) || nodeMap.get(transition.source_node_id) || nodeMap.get(edge.source);
  const target = nodeMap.get(transition.target) || nodeMap.get(transition.target_node_id) || nodeMap.get(edge.target);
  const riskCount = Array.isArray(transition.risk_refs) ? transition.risk_refs.length : 0;
  const riskBadge =
    riskCount > 0
      ? `<span class="edge-facts">${escapeHtml(t("workflow.risks", { count: riskCount }))}</span>`
      : "";
  return `
    <li>
      <div class="edge-row">
        <button class="trace-edge" type="button" data-node-id="${edge.target || transition.target_node_id || ""}">
          <span>${escapeHtml(formatKind(edge.kind || "unknown"))}</span>
          <strong>${escapeHtml(source?.label || String(edge.source || transition.source_node_id || ""))}</strong>
          <em>${escapeHtml(target?.label || String(edge.target || transition.target_node_id || ""))}</em>
          ${renderEdgeFacts(edge)}
          ${riskBadge}
        </button>
        ${renderEdgeActions(edge, source, target)}
      </div>
      <div class="edge-explanation" data-edge-explanation hidden></div>
    </li>
  `;
}

function attachWorkflowNavigation(container) {
  attachTraceNavigation(container);
}

function renderTrace(trace, options = {}) {
  if (!trace) {
    return `<p class="empty">${escapeHtml(t("trace.noStart"))}</p>`;
  }
  if (trace.nodes.length <= 1 && trace.edges.length === 0) {
    return `<p class="empty">${escapeHtml(options.empty || t("trace.noOutgoing"))}</p>`;
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

  const suffix = trace.truncated ? `<p class="empty">${escapeHtml(t("trace.traceTruncated"))}</p>` : "";
  return `
    <div class="trace-summary">
      ${options.label ? `<span>${escapeHtml(options.label)}</span>` : ""}
      <span>${trace.nodes.length} ${escapeHtml(t("stat.nodes").toLowerCase())}</span>
      <span>${trace.edges.length} ${escapeHtml(t("stat.edges").toLowerCase())}</span>
      <span>${escapeHtml(t("label.depth").toLowerCase())} ${trace.max_depth}</span>
    </div>
    <div class="trace-columns">
      <section>
        <h3>${escapeHtml(t("label.nodes"))}</h3>
        <ul class="trace-list">${nodeRows}</ul>
      </section>
      <section>
        <h3>${escapeHtml(t("label.edges"))}</h3>
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
        ${renderEdgeActions(edge, source, target)}
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
      selectNodeById(nodeId);
    });
  });
}

async function loadSourcePreview(node, requestId) {
  const preview = document.querySelector("#sourcePreview code");
  if (!preview || !node.span) return;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    file: node.span.path,
    start_line: String(node.span.start_line),
    end_line: String(node.span.end_line),
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

function renderSourceLine(line) {
  const number = String(line.number).padStart(4, " ");
  const className = line.highlight ? "source-line highlighted" : "source-line";
  return `<span class="${className}"><span class="line-number">${number}</span><span class="line-text">${escapeHtml(line.text || " ")}</span></span>`;
}

function renderNeighbor(edge, selectedId, nodeMap = null) {
  const otherId = edge.source === selectedId ? edge.target : edge.source;
  const other = nodeMap?.get(otherId) || state.graph.nodes.find((node) => node.id === otherId);
  const direction = edge.source === selectedId ? t("selection.outgoing") : t("selection.incoming");
  const facts = renderEdgeFacts(edge);
  return `
    <div>
      <div class="edge-row">
        <button type="button" class="neighbor" data-node-id="${otherId}">
          <span>${escapeHtml(direction)} ${escapeHtml(formatKind(edge.kind))}</span>
          <span>${escapeHtml(other ? other.label : String(otherId))}</span>
          ${facts}
        </button>
        ${renderEdgeActions(edge, nodeMap?.get(edge.source), nodeMap?.get(edge.target))}
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

function renderEdgeActions(edge, source = null, target = null) {
  return `
    <div class="edge-actions">
      ${renderSelectEdgeButton(edge, source, target)}
      ${renderExplainEdgeButton(edge)}
    </div>
  `;
}

function renderSelectEdgeButton(edge, source = null, target = null) {
  const selectionKey = registerEdgeSelection(edge, source, target);
  return `
    <button
      class="edge-card-button"
      type="button"
      data-select-edge
      data-edge-selection-key="${escapeHtml(selectionKey)}"
    >${escapeHtml(t("button.card"))}</button>
  `;
}
