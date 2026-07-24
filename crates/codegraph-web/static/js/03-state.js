// 03-state.js — initial settings, mutable app state, and the color palette.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

function getInitialLocale() {
  try {
    const saved = window.localStorage?.getItem("codegraph.locale");
    if (saved && I18N[saved]) return saved;
  } catch (error) {
    // Local storage can be disabled; falling back keeps the UI usable.
  }
  return DEFAULT_LOCALE;
}

function getInitialLabelMode() {
  try {
    const saved = window.localStorage?.getItem(LABEL_MODE_STORAGE_KEY);
    const version = window.localStorage?.getItem(LABEL_MODE_STORAGE_VERSION_KEY);
    if (version === LABEL_MODE_STORAGE_VERSION && saved && LABEL_MODES.has(saved)) return saved;
  } catch (error) {
    // Local storage can be disabled; the in-memory label mode still works.
  }
  return DEFAULT_LABEL_MODE;
}

function getInitialQueryHistory() {
  try {
    const raw = window.localStorage?.getItem(QUERY_HISTORY_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item) => typeof item === "string" && item.trim())
      .map((item) => item.trim())
      .slice(0, QUERY_HISTORY_LIMIT);
  } catch (error) {
    // Local storage can be disabled or user-edited; start with an empty history.
    return [];
  }
}

const state = {
  graph: { nodes: [], edges: [] },
  visibleNodes: [],
  visibleEdges: [],
  positions: new Map(),
  velocities: new Map(),
  selectedId: null,
  selectedEdgeKey: null,
  draggingId: null,
  hoveredId: null,
  hoveredEdgeKey: null,
  pan: { x: 0, y: 0 },
  zoom: 1,
  lastPointer: null,
  stageView: "graph",
  flow: {
    report: null,
    title: "",
    rootNodeId: null,
    focusMode: false,
    trail: [],
    positions: new Map(),
    pan: { x: 48, y: 48 },
    zoom: 1,
    selectedBlockId: null,
    hoveredBlockId: null,
    selectedTransitionId: null,
    hoveredTransitionId: null,
    lastPointer: null,
  },
  enabledKinds: new Set(),
  search: "",
  animationFrame: null,
  selectionRequest: 0,
  traceRequest: 0,
  workflowRequest: 0,
  dependentsRequest: 0,
  edgeExplainRequest: 0,
  entryFlowRequest: 0,
  journeyRequest: 0,
  queryWorkflowRequest: 0,
  queryRequest: 0,
  sourceSearchRequest: 0,
  cacheDiffRequest: 0,
  prImpactRequest: 0,
  prImpactReport: null,
  refactorRequest: 0,
  cacheChunksRequest: 0,
  incrementalPlanRequest: 0,
  incrementalScanRequest: 0,
  incrementalMergeRequest: 0,
  incrementalUpdateRequest: 0,
  exportRequest: 0,
  pathRequest: 0,
  configTraceRequest: 0,
  errorTraceRequest: 0,
  pageRequest: 0,
  overviewRequest: 0,
  insightRequest: 0,
  insightFocusRequest: 0,
  semanticEnrichRequest: 0,
  jobQueueRequest: 0,
  metricsRequest: 0,
  apiSchemaRequest: 0,
  checkRequest: 0,
  summary: null,
  scanOptions: null,
  coverage: null,
  capabilities: null,
  apiSchema: null,
  lsp: null,
  semanticReadiness: null,
  semanticPlan: null,
  report: null,
  architecture: null,
  languageDependencies: null,
  communities: null,
  hotspots: null,
  architecturePathPrefix: "",
  entrypoints: [],
  insightReport: null,
  riskByNode: new Map(),
  riskSeverities: new Set(),
  activeRiskSeverity: null,
  edgeSelectionCache: new Map(),
  edgeSelectionNodeCache: new Map(),
  projects: [],
  queryFocus: null,
  scanJobId: null,
  scanEvents: null,
  scanJobs: null,
  semanticJobId: null,
  semanticEvents: null,
  semanticJobs: null,
  metrics: null,
  lastApiResponse: null,
  layoutPaused: false,
  graphPage: {
    nodeOffset: 0,
    nodeLimit: 250,
    edgeOffset: 0,
    edgeLimit: 500,
    totalNodes: 0,
    totalEdges: 0,
    truncatedNodes: false,
    truncatedEdges: false,
    root: "",
  },
  locale: getInitialLocale(),
  labelMode: getInitialLabelMode(),
  queryHistory: getInitialQueryHistory(),
  lastQueryResult: null,
  lastCheckResult: null,
  lastSourceSearchResult: null,
  lastEntryFlowReport: null,
  lastJourneyReport: null,
  lastEntryWorkflowReport: null,
  lastPathResult: null,
  lastConfigTraceReport: null,
  lastErrorTraceReport: null,
  lastWorkflowReport: null,
  lastSelectionCard: null,
  pendingSelectionLink: null,
  pendingQueryLink: null,
  pendingGraphPageLink: false,
  pendingFlowLink: null,
};

// Full graphs above this node count start with control-flow facts hidden.
const LARGE_GRAPH_FILTER_THRESHOLD = 2000;

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
  control_flow: "#9aa7d8",
  unknown: "#a5adb3",
};

// Identity-cached node lookup for the current graph. Rebuilt automatically
// whenever state.graph is replaced, so per-frame code (edge drawing, client
// insights) does a Map get instead of a linear nodes.find per edge.
let graphNodeIndex = new Map();
let graphNodeIndexSource = null;

function graphNodeById(nodeId) {
  if (graphNodeIndexSource !== state.graph) {
    graphNodeIndexSource = state.graph;
    graphNodeIndex = new Map((state.graph.nodes || []).map((node) => [node.id, node]));
  }
  return graphNodeIndex.get(nodeId);
}
