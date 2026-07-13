// 04-dom.js — DOM element references and top-level event wiring.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

const canvas = document.querySelector("#graphCanvas");
const ctx = canvas.getContext("2d");
const minimapCanvas = document.querySelector("#graphMinimap");
const minimapCtx = minimapCanvas.getContext("2d");
const flowCanvas = document.querySelector("#flowCanvas");
const flowCtx = flowCanvas.getContext("2d");
const flowMinimapCanvas = document.querySelector("#flowMinimap");
const flowMinimapCtx = flowMinimapCanvas.getContext("2d");
const flowHud = document.querySelector("#flowHud");
const graphStage = document.querySelector(".graph-stage");
const stageViewButtons = document.querySelectorAll("[data-stage-view]");
const scanButton = document.querySelector("#scanButton");
const scanCancelButton = document.querySelector("#scanCancelButton");
const projectSelect = document.querySelector("#projectSelect");
const pathInput = document.querySelector("#pathInput");
const localeSelect = document.querySelector("#localeSelect");
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
const skippedCount = document.querySelector("#skippedCount");
const jobRefreshButton = document.querySelector("#jobRefreshButton");
const metricsRefreshButton = document.querySelector("#metricsRefreshButton");
const scanJobSummary = document.querySelector("#scanJobSummary");
const semanticJobSummary = document.querySelector("#semanticJobSummary");
const runtimeMetricsList = document.querySelector("#runtimeMetricsList");
const scanJobList = document.querySelector("#scanJobList");
const semanticJobList = document.querySelector("#semanticJobList");
const overviewTotals = document.querySelector("#overviewTotals");
const capabilitiesList = document.querySelector("#capabilitiesList");
const languageList = document.querySelector("#languageList");
const confidenceList = document.querySelector("#confidenceList");
const relationList = document.querySelector("#relationList");
const edgeSourceList = document.querySelector("#edgeSourceList");
const scanPolicyList = document.querySelector("#scanPolicyList");
const coverageList = document.querySelector("#coverageList");
const riskSummaryList = document.querySelector("#riskSummaryList");
const lspList = document.querySelector("#lspList");
const semanticWorkLanguageInput = document.querySelector("#semanticWorkLanguageInput");
const semanticWorkStatusInput = document.querySelector("#semanticWorkStatusInput");
const semanticWorkCapabilityInput = document.querySelector("#semanticWorkCapabilityInput");
const semanticWorkFilterButton = document.querySelector("#semanticWorkFilterButton");
const semanticEnrichButton = document.querySelector("#semanticEnrichButton");
const semanticCancelButton = document.querySelector("#semanticCancelButton");
const semanticWorkList = document.querySelector("#semanticWorkList");
const architectureList = document.querySelector("#architectureList");
const languageDependencyList = document.querySelector("#languageDependencyList");
const surprisingLinkList = document.querySelector("#surprisingLinkList");
const communityList = document.querySelector("#communityList");
const hotspotList = document.querySelector("#hotspotList");
const annotationList = document.querySelector("#annotationList");
const entrypointList = document.querySelector("#entrypointList");
const entryFlowSearchInput = document.querySelector("#entryFlowSearchInput");
const entryFlowDepthInput = document.querySelector("#entryFlowDepthInput");
const entryWorkflowEntrypointKindInput = document.querySelector("#entryWorkflowEntrypointKindInput");
const entryWorkflowEdgeKindInput = document.querySelector("#entryWorkflowEdgeKindInput");
const entryWorkflowConfidenceInput = document.querySelector("#entryWorkflowConfidenceInput");
const entryWorkflowLanguageInput = document.querySelector("#entryWorkflowLanguageInput");
const entryWorkflowRiskSeverityInput = document.querySelector("#entryWorkflowRiskSeverityInput");
const entryWorkflowBlockKindInput = document.querySelector("#entryWorkflowBlockKindInput");
const journeyFromInput = document.querySelector("#journeyFromInput");
const journeyToInput = document.querySelector("#journeyToInput");
const journeyDepthInput = document.querySelector("#journeyDepthInput");
const journeyPathsInput = document.querySelector("#journeyPathsInput");
const journeyRunButton = document.querySelector("#journeyRunButton");
const journeyExportButton = document.querySelector("#journeyExportButton");
const journeyResult = document.querySelector("#journeyResult");
const entryFlowButton = document.querySelector("#entryFlowButton");
const entryFlowWorkflowButton = document.querySelector("#entryFlowWorkflowButton");
const entryFlowExportButton = document.querySelector("#entryFlowExportButton");
const entryFlowWorkflowExportButton = document.querySelector("#entryFlowWorkflowExportButton");
const entryFlowWorkflowMermaidExportButton = document.querySelector("#entryFlowWorkflowMermaidExportButton");
const entryFlowWorkflowDotExportButton = document.querySelector("#entryFlowWorkflowDotExportButton");
const entryFlowResult = document.querySelector("#entryFlowResult");
const pageInfo = document.querySelector("#pageInfo");
const pageScope = document.querySelector("#pageScope");
const edgePageInfo = document.querySelector("#edgePageInfo");
const nodeLimitInput = document.querySelector("#nodeLimitInput");
const edgeLimitInput = document.querySelector("#edgeLimitInput");
const serverKindInput = document.querySelector("#serverKindInput");
const serverItemKindInput = document.querySelector("#serverItemKindInput");
const serverLanguageInput = document.querySelector("#serverLanguageInput");
const serverSearchInput = document.querySelector("#serverSearchInput");
const serverEdgeKindInput = document.querySelector("#serverEdgeKindInput");
const serverConfidenceInput = document.querySelector("#serverConfidenceInput");
const serverEdgeRelationInput = document.querySelector("#serverEdgeRelationInput");
const serverEdgeSourceInput = document.querySelector("#serverEdgeSourceInput");
const pagePrevButton = document.querySelector("#pagePrevButton");
const pageReloadButton = document.querySelector("#pageReloadButton");
const pageNextButton = document.querySelector("#pageNextButton");
const edgePrevButton = document.querySelector("#edgePrevButton");
const edgeNextButton = document.querySelector("#edgeNextButton");
const pageCopyButton = document.querySelector("#pageCopyButton");
const pageClearButton = document.querySelector("#pageClearButton");
const queryInput = document.querySelector("#queryInput");
const queryButton = document.querySelector("#queryButton");
const queryCopyButton = document.querySelector("#queryCopyButton");
const queryExportButton = document.querySelector("#queryExportButton");
const askInput = document.querySelector("#askInput");
const askButton = document.querySelector("#askButton");
const queryHistory = document.querySelector("#queryHistory");
const queryHistoryList = document.querySelector("#queryHistoryList");
const clearQueryHistoryButton = document.querySelector("#clearQueryHistoryButton");
const queryResult = document.querySelector("#queryResult");
const sourceSearchInput = document.querySelector("#sourceSearchInput");
const sourcePathFilterInput = document.querySelector("#sourcePathFilterInput");
const sourceSearchButton = document.querySelector("#sourceSearchButton");
const sourceSearchExportButton = document.querySelector("#sourceSearchExportButton");
const sourceSearchResult = document.querySelector("#sourceSearchResult");
const cacheDiffStatus = document.querySelector("#cacheDiffStatus");
const cacheDiffLimitInput = document.querySelector("#cacheDiffLimitInput");
const cacheDiffButton = document.querySelector("#cacheDiffButton");
const cacheChunksButton = document.querySelector("#cacheChunksButton");
const incrementalPlanButton = document.querySelector("#incrementalPlanButton");
const incrementalScanButton = document.querySelector("#incrementalScanButton");
const incrementalMergeButton = document.querySelector("#incrementalMergeButton");
const incrementalUpdateButton = document.querySelector("#incrementalUpdateButton");
const cacheDiffResult = document.querySelector("#cacheDiffResult");
const prImpactStatus = document.querySelector("#prImpactStatus");
const prImpactBaseInput = document.querySelector("#prImpactBaseInput");
const prImpactFilesInput = document.querySelector("#prImpactFilesInput");
const prImpactButton = document.querySelector("#prImpactButton");
const prImpactDownloadButton = document.querySelector("#prImpactDownloadButton");
const prImpactResult = document.querySelector("#prImpactResult");
const refactorStatus = document.querySelector("#refactorStatus");
const refactorTargetInput = document.querySelector("#refactorTargetInput");
const refactorUseSelectionButton = document.querySelector("#refactorUseSelectionButton");
const refactorImpactButton = document.querySelector("#refactorImpactButton");
const refactorDepsButton = document.querySelector("#refactorDepsButton");
const refactorSeamsButton = document.querySelector("#refactorSeamsButton");
const refactorContextButton = document.querySelector("#refactorContextButton");
const refactorSourceAreaInput = document.querySelector("#refactorSourceAreaInput");
const refactorTargetAreaInput = document.querySelector("#refactorTargetAreaInput");
const refactorContractButton = document.querySelector("#refactorContractButton");
const refactorResult = document.querySelector("#refactorResult");
const exportFormatInput = document.querySelector("#exportFormatInput");
const exportButton = document.querySelector("#exportButton");
const exportSliceButton = document.querySelector("#exportSliceButton");
const exportResult = document.querySelector("#exportResult");
const pathFromInput = document.querySelector("#pathFromInput");
const pathToInput = document.querySelector("#pathToInput");
const pathDepthInput = document.querySelector("#pathDepthInput");
const pathEdgeKindInput = document.querySelector("#pathEdgeKindInput");
const pathButton = document.querySelector("#pathButton");
const pathExportButton = document.querySelector("#pathExportButton");
const pathResult = document.querySelector("#pathResult");
const configTraceTargetInput = document.querySelector("#configTraceTargetInput");
const configTraceDepthInput = document.querySelector("#configTraceDepthInput");
const configTraceButton = document.querySelector("#configTraceButton");
const configTraceExportButton = document.querySelector("#configTraceExportButton");
const configTraceResult = document.querySelector("#configTraceResult");
const errorTraceTargetInput = document.querySelector("#errorTraceTargetInput");
const errorTraceDepthInput = document.querySelector("#errorTraceDepthInput");
const errorTraceButton = document.querySelector("#errorTraceButton");
const errorTraceExportButton = document.querySelector("#errorTraceExportButton");
const errorTraceResult = document.querySelector("#errorTraceResult");
const insightCount = document.querySelector("#insightCount");
const insightList = document.querySelector("#insightList");
const insightSeverityInput = document.querySelector("#insightSeverityInput");
const checkFailOnInput = document.querySelector("#checkFailOnInput");
const insightKindInput = document.querySelector("#insightKindInput");
const insightSearchInput = document.querySelector("#insightSearchInput");
const insightFilterButton = document.querySelector("#insightFilterButton");
const insightExportButton = document.querySelector("#insightExportButton");
const checkButton = document.querySelector("#checkButton");
const checkExportButton = document.querySelector("#checkExportButton");
const checkResult = document.querySelector("#checkResult");
const kindFilters = document.querySelector("#kindFilters");
const clearCanvasFiltersButton = document.querySelector("#clearCanvasFiltersButton");
const selectionTitle = document.querySelector("#selectionTitle");
const selectionBody = document.querySelector("#selectionBody");
const legend = document.querySelector("#legend");
const graphHud = document.querySelector("#graphHud");
const zoomOutButton = document.querySelector("#zoomOutButton");
const zoomInButton = document.querySelector("#zoomInButton");
const fitGraphButton = document.querySelector("#fitGraphButton");
const resetLayoutButton = document.querySelector("#resetLayoutButton");
const toggleLayoutButton = document.querySelector("#toggleLayoutButton");
const viewportInfo = document.querySelector("#viewportInfo");
const labelModeButtons = Array.from(document.querySelectorAll("[data-label-mode]"));
const nodeKindOptions = document.querySelector("#nodeKindOptions");
const edgeKindOptions = document.querySelector("#edgeKindOptions");
const confidenceOptions = document.querySelector("#confidenceOptions");
const severityOptions = document.querySelector("#severityOptions");
const workflowBlockKindOptions = document.querySelector("#workflowBlockKindOptions");
const entrypointKindOptions = document.querySelector("#entrypointKindOptions");
const insightKindOptions = document.querySelector("#insightKindOptions");

localeSelect.value = state.locale;
localeSelect.addEventListener("change", () => setLocale(localeSelect.value));
scanButton.addEventListener("click", () => scan());
scanCancelButton.addEventListener("click", () => cancelScanJob());
jobRefreshButton.addEventListener("click", () => loadJobQueue());
metricsRefreshButton.addEventListener("click", () => loadMetrics());
scanJobList.addEventListener("click", (event) => onJobListClick(event, "scan"));
semanticJobList.addEventListener("click", (event) => onJobListClick(event, "semantic"));
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
clearCanvasFiltersButton.addEventListener("click", () => clearCanvasFilters());
queryButton.addEventListener("click", () => runGraphQuery());
queryCopyButton.addEventListener("click", () => copyCurrentQueryLink(queryCopyButton));
queryExportButton.addEventListener("click", () => exportLastQueryResult());
askButton.addEventListener("click", () => runNaturalQuery());
clearQueryHistoryButton.addEventListener("click", () => clearQueryHistory());
queryInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") runGraphQuery();
});
askInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") runNaturalQuery();
});
document.querySelectorAll("[data-query-preset]").forEach((button) => {
  button.addEventListener("click", () => {
    queryInput.value = button.dataset.queryPreset || "";
    runGraphQuery();
  });
});
sourceSearchButton.addEventListener("click", () => runSourceSearch());
sourceSearchExportButton.addEventListener("click", () => exportLastSourceSearchResult());
for (const input of [sourceSearchInput, sourcePathFilterInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runSourceSearch();
  });
}
cacheDiffButton.addEventListener("click", () => loadCacheDiff());
prImpactButton.addEventListener("click", () => loadPrImpact());
refactorUseSelectionButton.addEventListener("click", () => {
  const node = state.lastSelectionCard?.selection?.node;
  if (node) {
    refactorTargetInput.value = node.label || `n${node.id}`;
  }
});
refactorImpactButton.addEventListener("click", () => runRefactorReport("impact"));
refactorDepsButton.addEventListener("click", () => runRefactorReport("dependencies"));
refactorSeamsButton.addEventListener("click", () => runRefactorReport("seams"));
refactorContextButton.addEventListener("click", () => runRefactorReport("context"));
refactorContractButton.addEventListener("click", () => runRefactorReport("contract"));
prImpactDownloadButton.addEventListener("click", () => {
  if (!state.prImpactReport) return;
  const blob = new Blob([JSON.stringify(state.prImpactReport, null, 2)], {
    type: "application/json",
  });
  downloadBlob(blob, `${safeFilePart(selectionCardRoot())}-pr-impact.json`);
});
cacheChunksButton.addEventListener("click", () => loadCacheChunks());
incrementalPlanButton.addEventListener("click", () => loadIncrementalPlan());
incrementalScanButton.addEventListener("click", () => loadIncrementalScan());
incrementalMergeButton.addEventListener("click", () => loadIncrementalMergePreview());
incrementalUpdateButton.addEventListener("click", () => loadIncrementalUpdate());
cacheDiffLimitInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") loadCacheDiff();
});
exportButton.addEventListener("click", () => runGraphExport());
exportSliceButton.addEventListener("click", () => exportVisibleGraphSlice());
journeyRunButton.addEventListener("click", () => runJourney());
journeyExportButton.addEventListener("click", () => exportLastJourneyReport());
for (const input of [journeyFromInput, journeyToInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runJourney();
  });
}
entryFlowButton.addEventListener("click", () => runEntryFlowTrace());
entryFlowWorkflowButton.addEventListener("click", () => runEntryFlowWorkflows());
entryFlowExportButton.addEventListener("click", () => exportLastEntryFlowReport());
entryFlowWorkflowExportButton.addEventListener("click", () => exportLastEntryWorkflowReport("json"));
entryFlowWorkflowMermaidExportButton.addEventListener("click", () => exportLastEntryWorkflowReport("mermaid"));
entryFlowWorkflowDotExportButton.addEventListener("click", () => exportLastEntryWorkflowReport("dot"));
for (const input of [entryFlowSearchInput, entryFlowDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runEntryFlowTrace();
  });
}
for (const input of [
  entryWorkflowEdgeKindInput,
  entryWorkflowConfidenceInput,
  entryWorkflowLanguageInput,
  entryWorkflowRiskSeverityInput,
  entryWorkflowBlockKindInput,
]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runEntryFlowWorkflows();
  });
}
pathButton.addEventListener("click", () => runPathQuery());
pathExportButton.addEventListener("click", () => exportLastPathResult());
for (const input of [pathFromInput, pathToInput, pathDepthInput, pathEdgeKindInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runPathQuery();
  });
}
configTraceButton.addEventListener("click", () => runConfigTrace());
configTraceExportButton.addEventListener("click", () => exportLastConfigTraceReport());
for (const input of [configTraceTargetInput, configTraceDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runConfigTrace();
  });
}
errorTraceButton.addEventListener("click", () => runErrorTrace());
errorTraceExportButton.addEventListener("click", () => exportLastErrorTraceReport());
for (const input of [errorTraceTargetInput, errorTraceDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runErrorTrace();
  });
}
insightFilterButton.addEventListener("click", () => loadInsights());
insightExportButton.addEventListener("click", () => exportCurrentInsights());
for (const input of [insightSeverityInput, insightKindInput, insightSearchInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") loadInsights();
  });
}
checkButton.addEventListener("click", () => runCheck());
checkExportButton.addEventListener("click", () => exportLastCheckResult());
checkFailOnInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") runCheck();
});
semanticWorkFilterButton.addEventListener("click", () => loadProjectOverview());
semanticEnrichButton.addEventListener("click", () => runSemanticEnrich());
semanticCancelButton.addEventListener("click", () => cancelSemanticJob());
for (const input of [semanticWorkLanguageInput, semanticWorkStatusInput, semanticWorkCapabilityInput]) {
  input.addEventListener("change", () => loadProjectOverview());
}
pagePrevButton.addEventListener("click", () => shiftGraphPage(-1));
pageNextButton.addEventListener("click", () => shiftGraphPage(1));
pageReloadButton.addEventListener("click", () => loadGraphPage({ resetPage: true }));
edgePrevButton.addEventListener("click", () => shiftEdgePage(-1));
edgeNextButton.addEventListener("click", () => shiftEdgePage(1));
pageCopyButton.addEventListener("click", () => copyGraphPageLink(pageCopyButton));
pageClearButton.addEventListener("click", () => clearGraphPageFilters());
zoomOutButton.addEventListener("click", () => {
  if (state.stageView === "flow") flowZoomAtCenter(0.82);
  else zoomAtCanvasCenter(0.82);
});
zoomInButton.addEventListener("click", () => {
  if (state.stageView === "flow") flowZoomAtCenter(1.18);
  else zoomAtCanvasCenter(1.18);
});
fitGraphButton.addEventListener("click", () => {
  if (state.stageView === "flow") fitFlowView();
  else fitVisibleGraph();
});
stageViewButtons.forEach((button) => {
  button.addEventListener("click", () => setStageView(button.dataset.stageView));
});
resetLayoutButton.addEventListener("click", () => resetGraphLayout());
toggleLayoutButton.addEventListener("click", () => toggleLayout());
labelModeButtons.forEach((button) => {
  button.addEventListener("click", () => setLabelMode(button.dataset.labelMode));
});
for (const input of [
  nodeLimitInput,
  edgeLimitInput,
  serverKindInput,
  serverItemKindInput,
  serverLanguageInput,
  serverSearchInput,
  serverEdgeKindInput,
  serverConfidenceInput,
  serverEdgeRelationInput,
  serverEdgeSourceInput,
]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") loadGraphPage({ resetPage: true });
  });
}

canvas.addEventListener("pointerdown", onPointerDown);
canvas.addEventListener("pointermove", onPointerMove);
canvas.addEventListener("pointerup", onPointerUp);
canvas.addEventListener("pointerleave", onPointerLeave);
canvas.addEventListener("keydown", onCanvasKeyDown);
canvas.addEventListener("wheel", onWheel, { passive: false });
minimapCanvas.addEventListener("pointerdown", onMinimapPointerDown);
minimapCanvas.addEventListener("pointermove", onMinimapPointerMove);
minimapCanvas.addEventListener("pointerup", onMinimapPointerUp);
minimapCanvas.addEventListener("pointerleave", onMinimapPointerUp);
minimapCanvas.addEventListener("keydown", onCanvasKeyDown);
flowCanvas.addEventListener("pointerdown", onFlowPointerDown);
flowCanvas.addEventListener("pointermove", onFlowPointerMove);
flowCanvas.addEventListener("pointerup", onFlowPointerUp);
flowCanvas.addEventListener("pointerleave", onFlowPointerUp);
flowCanvas.addEventListener("keydown", onFlowKeyDown);
flowCanvas.addEventListener("dblclick", onFlowDoubleClick);
flowCanvas.addEventListener("wheel", onFlowWheel, { passive: false });
if (flowHud) {
  flowHud.addEventListener("click", (event) => {
    const crumb = event.target.closest("[data-flow-crumb]");
    if (crumb) restoreFlowLevel(Number(crumb.dataset.flowCrumb));
  });
}
flowMinimapCanvas.addEventListener("pointerdown", onFlowMinimapPointerDown);
flowMinimapCanvas.addEventListener("pointermove", onFlowMinimapPointerMove);
flowMinimapCanvas.addEventListener("keydown", onFlowKeyDown);
window.addEventListener("resize", resizeCanvas);
window.addEventListener("resize", resizeFlowCanvas);

syncApiTokenCookie();
applyLocale();
resizeCanvas();
init();
