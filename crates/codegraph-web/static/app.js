const DEFAULT_LOCALE = "en";
const DEFAULT_LABEL_MODE = "minimal";
const LABEL_MODES = new Set(["minimal", "focus", "auto"]);
const LABEL_MODE_STORAGE_KEY = "codegraph.labelMode";
const LABEL_MODE_STORAGE_VERSION_KEY = "codegraph.labelModeVersion";
const LABEL_MODE_STORAGE_VERSION = "3";

const I18N = {
  en: {
    "root.empty": "No project loaded",
    "project.currentRoot": "Current root",
    "selection.title": "Selection",
    "selection.noNode": "No node selected.",
    "selection.loading": "Loading node context...",
    "selection.node": "Node",
    "selection.kind": "Kind",
    "selection.id": "Id",
    "selection.path": "Path",
    "selection.lines": "Lines",
    "selection.summary": "Summary",
    "selection.dependencies": "Dependencies",
    "selection.risks": "Risks",
    "selection.source": "Source",
    "selection.metadata": "Metadata",
    "selection.noDependencies": "No neighboring edges.",
    "selection.noIssues": "No matching risks for this node.",
    "selection.contextEdges": "{count} edges",
    "selection.contextEdgesLimited": "{count} edges, first {limit}",
    "selection.issueHint": "Open finding",
    "selection.noSource": "No source span is attached to this node.",
    "selection.sourceTruncated": "preview truncated",
    "selection.incoming": "incoming",
    "selection.outgoing": "outgoing",
    "selection.from": "From",
    "selection.to": "To",
    "selection.configTrace": "Config Trace",
    "selection.errorTrace": "Error Trace",
    "selection.trace": "Trace",
    "selection.dependents": "Dependents",
    "selection.traceDepth": "Depth",
    "label.project": "Project",
    "label.path": "Path",
    "label.workLang": "Work Lang",
    "label.status": "Status",
    "label.capability": "Capability",
    "label.search": "Search",
    "label.depth": "Depth",
    "label.nodes": "Nodes",
    "label.edges": "Edges",
    "label.kind": "Kind",
    "label.item": "Item",
    "label.language": "Language",
    "label.edge": "Edge",
    "label.confidence": "Confidence",
    "label.relation": "Relation",
    "label.source": "Source",
    "label.query": "Query",
    "label.text": "Text",
    "label.limit": "Limit",
    "label.from": "From",
    "label.to": "To",
    "label.target": "Target",
    "label.severity": "Severity",
    "label.failOn": "Fail On",
    "label.format": "Format",
    "button.scan": "Scan",
    "button.cancel": "Cancel",
    "button.refresh": "Refresh",
    "button.apply": "Apply",
    "button.semanticEnrich": "Enrich",
    "button.traceEntrypoints": "Trace Entrypoints",
    "button.run": "Run",
    "button.searchSource": "Search Source",
    "button.explainCache": "Explain Cache",
    "button.cacheChunks": "Cache Chunks",
    "button.planIncremental": "Plan Incremental",
    "button.scanIncremental": "Scan Changed",
    "button.previewMerge": "Preview Merge",
    "button.updateCache": "Update Cache",
    "button.downloadGraph": "Download Graph",
    "button.download": "Download",
    "button.findPath": "Find Path",
    "button.traceConfig": "Trace Config",
    "button.traceErrors": "Trace Errors",
    "button.runCheck": "Run Check",
    "button.fit": "Fit",
    "button.reset": "Reset",
    "button.pause": "Pause",
    "button.resume": "Resume",
    "button.labelsMinimal": "Min",
    "button.labelsAuto": "Auto",
    "button.labelsFocus": "Focus",
    "button.explain": "Explain",
    "option.any": "Any",
    "option.ready": "Ready",
    "option.missing": "Missing",
    "option.unsupported": "Unsupported",
    "option.definitions": "Definitions",
    "option.diagnostics": "Diagnostics",
    "option.symbols": "Symbols",
    "option.workspaceSymbols": "Workspace Symbols",
    "option.references": "References",
    "option.server": "Server",
    "section.overview": "Overview",
    "section.jobs": "Jobs",
    "section.runtime": "Runtime",
    "section.entryFlows": "Entry Flows",
    "section.graphPage": "Graph Page",
    "section.sourceSearch": "Source Search",
    "section.cacheDiff": "Cache Diff",
    "section.export": "Export",
    "section.path": "Path",
    "section.configTrace": "Config Trace",
    "section.errorTrace": "Error Trace",
    "section.insights": "Insights",
    "stat.nodes": "Nodes",
    "stat.edges": "Edges",
    "stat.calls": "Calls",
    "stat.env": "Env",
    "stat.config": "Config",
    "stat.errors": "Errors",
    "stat.entrypoints": "Entrypoints",
    "stat.skipped": "Skipped",
    "empty.noLanguages": "No languages.",
    "empty.noEdgeConfidence": "No edge confidence.",
    "empty.noEdgeRelations": "No edge relations.",
    "empty.noEdgeSources": "No edge sources.",
    "empty.noInsights": "No matching insights.",
    "empty.noVisibleIssues": "No obvious issues in the visible graph.",
    "empty.noCapabilities": "No server capabilities.",
    "empty.noMetrics": "No runtime metrics.",
    "empty.loadingSource": "Loading...",
    "cap.api": "API",
    "cap.graph": "Graph",
    "cap.cache": "Cache",
    "cap.languages": "Languages",
    "cap.exports": "Exports",
    "cap.projects": "Projects",
    "cap.scanJobs": "Scan Jobs",
    "cap.semanticJobs": "Semantic Jobs",
    "cap.routes": "Routes",
    "cap.on": "on",
    "cap.off": "off",
    "runtime.uptime": "Uptime",
    "runtime.cache": "Cache",
    "runtime.scanSlots": "Scan Slots",
    "runtime.semanticSlots": "Semantic Slots",
    "runtime.scanJobs": "Scan Jobs",
    "runtime.semanticJobs": "Semantic Jobs",
    "export.report": "Report JSON",
    "trace.tracing": "Tracing...",
    "trace.tracingDependents": "Tracing dependents...",
    "trace.noDependents": "No incoming dependents.",
    "trace.dependents": "Dependents",
    "semantic.running": "Running semantic enrichment...",
    "job.scan": "Scan",
    "job.semantic": "Semantic",
    "job.empty": "No retained jobs.",
    "job.updated": "Updated",
    "job.status.queued": "queued",
    "job.status.running": "running",
    "job.status.complete": "complete",
    "job.status.failed": "failed",
    "job.status.canceled": "canceled",
    "job.scanCanceled": "Scan canceled.",
    "job.semanticCanceled": "Semantic enrichment canceled.",
    "semantic.report": "Semantic enrichment",
    "semantic.responses": "responses",
    "semantic.cache": "cache",
    "semantic.edges": "Semantic edges",
    "semantic.replaced": "Replaced",
    "semantic.added": "Added",
    "semantic.diagnostics": "Diagnostics",
    "semantic.errors": "Errors",
    "semantic.unmatched": "Unmatched",
    "status.idle": "idle",
    "status.queue": "queue",
    "status.scan": "scan",
    "status.page": "page",
    "status.semantic": "semantic",
    "status.ready": "ready",
    "status.error": "error",
    "kind.error": "error",
    "kind.warning": "warning",
    "kind.info": "info",
  },
  ru: {
    "root.empty": "Проект не загружен",
    "project.currentRoot": "Текущий каталог",
    "selection.title": "Выбор",
    "selection.noNode": "Узел не выбран.",
    "selection.loading": "Загружаю контекст узла...",
    "selection.node": "Узел",
    "selection.kind": "Тип",
    "selection.id": "Id",
    "selection.path": "Путь",
    "selection.lines": "Строки",
    "selection.summary": "Сводка",
    "selection.dependencies": "Связи",
    "selection.risks": "Риски",
    "selection.source": "Код",
    "selection.metadata": "Метаданные",
    "selection.noDependencies": "Соседних связей нет.",
    "selection.noIssues": "Для этого узла нет совпадающих рисков.",
    "selection.contextEdges": "{count} связей",
    "selection.contextEdgesLimited": "{count} связей, первые {limit}",
    "selection.issueHint": "Открыть находку",
    "selection.noSource": "К этому узлу не привязан фрагмент кода.",
    "selection.sourceTruncated": "фрагмент обрезан",
    "selection.incoming": "входящая",
    "selection.outgoing": "исходящая",
    "selection.from": "Отсюда",
    "selection.to": "Сюда",
    "selection.configTrace": "Трасса конфига",
    "selection.errorTrace": "Трасса ошибок",
    "selection.trace": "Трассировать",
    "selection.dependents": "Зависимые",
    "selection.traceDepth": "Глубина",
    "label.project": "Проект",
    "label.path": "Путь",
    "label.workLang": "Язык задач",
    "label.status": "Статус",
    "label.capability": "Возможность",
    "label.search": "Поиск",
    "label.depth": "Глубина",
    "label.nodes": "Узлы",
    "label.edges": "Связи",
    "label.kind": "Тип",
    "label.item": "Элемент",
    "label.language": "Язык",
    "label.edge": "Связь",
    "label.confidence": "Уверенность",
    "label.relation": "Отношение",
    "label.source": "Источник",
    "label.query": "Запрос",
    "label.text": "Текст",
    "label.limit": "Лимит",
    "label.from": "Откуда",
    "label.to": "Куда",
    "label.target": "Цель",
    "label.severity": "Важность",
    "label.failOn": "Порог",
    "label.format": "Формат",
    "button.scan": "Сканировать",
    "button.cancel": "Отменить",
    "button.refresh": "Обновить",
    "button.apply": "Применить",
    "button.semanticEnrich": "Обогатить",
    "button.traceEntrypoints": "Трассировать входы",
    "button.run": "Запустить",
    "button.searchSource": "Искать в коде",
    "button.explainCache": "Объяснить кеш",
    "button.cacheChunks": "Фрагменты кеша",
    "button.planIncremental": "План инкремента",
    "button.scanIncremental": "Скан изменений",
    "button.previewMerge": "Предпросмотр merge",
    "button.updateCache": "Обновить кеш",
    "button.downloadGraph": "Скачать граф",
    "button.download": "Скачать",
    "button.findPath": "Найти путь",
    "button.traceConfig": "Трассировать конфиг",
    "button.traceErrors": "Трассировать ошибки",
    "button.runCheck": "Проверить",
    "button.fit": "Вписать",
    "button.reset": "Сброс",
    "button.pause": "Пауза",
    "button.resume": "Продолжить",
    "button.labelsMinimal": "Мин",
    "button.labelsAuto": "Авто",
    "button.labelsFocus": "Фокус",
    "button.explain": "Пояснить",
    "option.any": "Любой",
    "option.ready": "Готово",
    "option.missing": "Нет сервера",
    "option.unsupported": "Не поддержано",
    "option.definitions": "Определения",
    "option.diagnostics": "Диагностика",
    "option.symbols": "Символы",
    "option.workspaceSymbols": "Символы workspace",
    "option.references": "Ссылки",
    "option.server": "Сервер",
    "section.overview": "Обзор",
    "section.jobs": "Задачи",
    "section.runtime": "Рантайм",
    "section.entryFlows": "Потоки входа",
    "section.graphPage": "Страница графа",
    "section.sourceSearch": "Поиск в коде",
    "section.cacheDiff": "Дифф кеша",
    "section.export": "Экспорт",
    "section.path": "Путь",
    "section.configTrace": "Трасса конфига",
    "section.errorTrace": "Трасса ошибок",
    "section.insights": "Находки",
    "stat.nodes": "Узлы",
    "stat.edges": "Связи",
    "stat.calls": "Вызовы",
    "stat.env": "Env",
    "stat.config": "Конфиг",
    "stat.errors": "Ошибки",
    "stat.entrypoints": "Точки входа",
    "stat.skipped": "Пропущено",
    "empty.noLanguages": "Языки не найдены.",
    "empty.noEdgeConfidence": "Нет данных об уверенности связей.",
    "empty.noEdgeRelations": "Нет отношений связей.",
    "empty.noEdgeSources": "Нет источников связей.",
    "empty.noInsights": "Совпадающих находок нет.",
    "empty.noVisibleIssues": "В видимом графе явных проблем нет.",
    "empty.noCapabilities": "Нет данных о сервере.",
    "empty.noMetrics": "Нет runtime-метрик.",
    "empty.loadingSource": "Загружаю...",
    "cap.api": "API",
    "cap.graph": "Граф",
    "cap.cache": "Кеш",
    "cap.languages": "Языки",
    "cap.exports": "Экспорты",
    "cap.projects": "Проекты",
    "cap.scanJobs": "Скан-задачи",
    "cap.semanticJobs": "Сем. задачи",
    "cap.routes": "Маршруты",
    "cap.on": "вкл",
    "cap.off": "выкл",
    "runtime.uptime": "Аптайм",
    "runtime.cache": "Кеш",
    "runtime.scanSlots": "Слоты скана",
    "runtime.semanticSlots": "Слоты сем.",
    "runtime.scanJobs": "Скан-задачи",
    "runtime.semanticJobs": "Сем. задачи",
    "export.report": "JSON-отчёт",
    "trace.tracing": "Трассирую...",
    "trace.tracingDependents": "Трассирую зависимые узлы...",
    "trace.noDependents": "Входящих зависимых нет.",
    "trace.dependents": "Зависимые",
    "semantic.running": "Запускаю семантическое обогащение...",
    "job.scan": "Скан",
    "job.semantic": "Семантика",
    "job.empty": "Сохранённых задач нет.",
    "job.updated": "Обновлено",
    "job.status.queued": "в очереди",
    "job.status.running": "в работе",
    "job.status.complete": "готово",
    "job.status.failed": "ошибка",
    "job.status.canceled": "отменено",
    "job.scanCanceled": "Сканирование отменено.",
    "job.semanticCanceled": "Семантическое обогащение отменено.",
    "semantic.report": "Семантическое обогащение",
    "semantic.responses": "ответов",
    "semantic.cache": "кеш",
    "semantic.edges": "Семантические связи",
    "semantic.replaced": "Заменено",
    "semantic.added": "Добавлено",
    "semantic.diagnostics": "Диагностика",
    "semantic.errors": "Ошибки",
    "semantic.unmatched": "Без совпадения",
    "status.idle": "ожидание",
    "status.queue": "очередь",
    "status.scan": "скан",
    "status.page": "страница",
    "status.semantic": "семантика",
    "status.ready": "готово",
    "status.error": "ошибка",
    "kind.error": "ошибка",
    "kind.warning": "предупреждение",
    "kind.info": "инфо",
    "kind.function": "функция",
    "kind.file": "файл",
    "kind.directory": "каталог",
    "kind.module": "модуль",
    "kind.type": "тип",
    "kind.config": "конфиг",
    "kind.environment": "окружение",
    "kind.entrypoint": "точка входа",
    "kind.external_dependency": "внешняя зависимость",
    "kind.repository": "репозиторий",
    "kind.unknown": "неизвестно",
    "kind.calls": "вызов",
    "kind.imports": "импорт",
    "kind.references": "ссылка",
    "kind.reads_config": "читает конфиг",
    "kind.reads_environment": "читает окружение",
    "kind.may_error": "может ошибиться",
    "kind.entrypoint_edge": "точка входа",
    "kind.unresolved_call": "неразрешённый вызов",
    "kind.parse_error": "ошибка парсинга",
    "kind.syntax_error": "синтаксическая ошибка",
    "kind.orphan_function": "изолированная функция",
    "kind.potential_error_flow": "потенциальный поток ошибки",
    "kind.undeclared_external_import": "импорт без зависимости",
  },
};

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

const state = {
  graph: { nodes: [], edges: [] },
  visibleNodes: [],
  visibleEdges: [],
  positions: new Map(),
  velocities: new Map(),
  selectedId: null,
  draggingId: null,
  hoveredId: null,
  pan: { x: 0, y: 0 },
  zoom: 1,
  lastPointer: null,
  enabledKinds: new Set(),
  search: "",
  animationFrame: null,
  selectionRequest: 0,
  traceRequest: 0,
  dependentsRequest: 0,
  edgeExplainRequest: 0,
  entryFlowRequest: 0,
  queryRequest: 0,
  sourceSearchRequest: 0,
  cacheDiffRequest: 0,
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
  checkRequest: 0,
  summary: null,
  scanOptions: null,
  coverage: null,
  capabilities: null,
  lsp: null,
  semanticReadiness: null,
  semanticPlan: null,
  architecture: null,
  languageDependencies: null,
  hotspots: null,
  architecturePathPrefix: "",
  entrypoints: [],
  insightReport: null,
  projects: [],
  queryFocus: null,
  scanJobId: null,
  scanEvents: null,
  scanJobs: null,
  semanticJobId: null,
  semanticEvents: null,
  semanticJobs: null,
  metrics: null,
  layoutPaused: false,
  graphPage: {
    nodeOffset: 0,
    nodeLimit: 250,
    edgeLimit: 500,
    totalNodes: 0,
    totalEdges: 0,
    truncatedNodes: false,
    root: "",
  },
  locale: getInitialLocale(),
  labelMode: getInitialLabelMode(),
};

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
  unknown: "#a5adb3",
};

const canvas = document.querySelector("#graphCanvas");
const ctx = canvas.getContext("2d");
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
const hotspotList = document.querySelector("#hotspotList");
const annotationList = document.querySelector("#annotationList");
const entrypointList = document.querySelector("#entrypointList");
const entryFlowSearchInput = document.querySelector("#entryFlowSearchInput");
const entryFlowDepthInput = document.querySelector("#entryFlowDepthInput");
const entryFlowButton = document.querySelector("#entryFlowButton");
const entryFlowResult = document.querySelector("#entryFlowResult");
const pageInfo = document.querySelector("#pageInfo");
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
const queryInput = document.querySelector("#queryInput");
const queryButton = document.querySelector("#queryButton");
const queryResult = document.querySelector("#queryResult");
const sourceSearchInput = document.querySelector("#sourceSearchInput");
const sourcePathFilterInput = document.querySelector("#sourcePathFilterInput");
const sourceSearchButton = document.querySelector("#sourceSearchButton");
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
const exportFormatInput = document.querySelector("#exportFormatInput");
const exportButton = document.querySelector("#exportButton");
const exportResult = document.querySelector("#exportResult");
const pathFromInput = document.querySelector("#pathFromInput");
const pathToInput = document.querySelector("#pathToInput");
const pathDepthInput = document.querySelector("#pathDepthInput");
const pathEdgeKindInput = document.querySelector("#pathEdgeKindInput");
const pathButton = document.querySelector("#pathButton");
const pathResult = document.querySelector("#pathResult");
const configTraceTargetInput = document.querySelector("#configTraceTargetInput");
const configTraceDepthInput = document.querySelector("#configTraceDepthInput");
const configTraceButton = document.querySelector("#configTraceButton");
const configTraceResult = document.querySelector("#configTraceResult");
const errorTraceTargetInput = document.querySelector("#errorTraceTargetInput");
const errorTraceDepthInput = document.querySelector("#errorTraceDepthInput");
const errorTraceButton = document.querySelector("#errorTraceButton");
const errorTraceResult = document.querySelector("#errorTraceResult");
const insightCount = document.querySelector("#insightCount");
const insightList = document.querySelector("#insightList");
const insightSeverityInput = document.querySelector("#insightSeverityInput");
const checkFailOnInput = document.querySelector("#checkFailOnInput");
const insightKindInput = document.querySelector("#insightKindInput");
const insightSearchInput = document.querySelector("#insightSearchInput");
const insightFilterButton = document.querySelector("#insightFilterButton");
const checkButton = document.querySelector("#checkButton");
const checkResult = document.querySelector("#checkResult");
const kindFilters = document.querySelector("#kindFilters");
const selectionTitle = document.querySelector("#selectionTitle");
const selectionBody = document.querySelector("#selectionBody");
const legend = document.querySelector("#legend");
const zoomOutButton = document.querySelector("#zoomOutButton");
const zoomInButton = document.querySelector("#zoomInButton");
const fitGraphButton = document.querySelector("#fitGraphButton");
const resetLayoutButton = document.querySelector("#resetLayoutButton");
const toggleLayoutButton = document.querySelector("#toggleLayoutButton");
const viewportInfo = document.querySelector("#viewportInfo");
const labelModeButtons = Array.from(document.querySelectorAll("[data-label-mode]"));

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
queryButton.addEventListener("click", () => runGraphQuery());
queryInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") runGraphQuery();
});
sourceSearchButton.addEventListener("click", () => runSourceSearch());
for (const input of [sourceSearchInput, sourcePathFilterInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runSourceSearch();
  });
}
cacheDiffButton.addEventListener("click", () => loadCacheDiff());
cacheChunksButton.addEventListener("click", () => loadCacheChunks());
incrementalPlanButton.addEventListener("click", () => loadIncrementalPlan());
incrementalScanButton.addEventListener("click", () => loadIncrementalScan());
incrementalMergeButton.addEventListener("click", () => loadIncrementalMergePreview());
incrementalUpdateButton.addEventListener("click", () => loadIncrementalUpdate());
cacheDiffLimitInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") loadCacheDiff();
});
exportButton.addEventListener("click", () => runGraphExport());
entryFlowButton.addEventListener("click", () => runEntryFlowTrace());
for (const input of [entryFlowSearchInput, entryFlowDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runEntryFlowTrace();
  });
}
pathButton.addEventListener("click", () => runPathQuery());
for (const input of [pathFromInput, pathToInput, pathDepthInput, pathEdgeKindInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runPathQuery();
  });
}
configTraceButton.addEventListener("click", () => runConfigTrace());
for (const input of [configTraceTargetInput, configTraceDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runConfigTrace();
  });
}
errorTraceButton.addEventListener("click", () => runErrorTrace());
for (const input of [errorTraceTargetInput, errorTraceDepthInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") runErrorTrace();
  });
}
insightFilterButton.addEventListener("click", () => loadInsights());
for (const input of [insightSeverityInput, insightKindInput, insightSearchInput]) {
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") loadInsights();
  });
}
checkButton.addEventListener("click", () => runCheck());
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
zoomOutButton.addEventListener("click", () => zoomAtCanvasCenter(0.82));
zoomInButton.addEventListener("click", () => zoomAtCanvasCenter(1.18));
fitGraphButton.addEventListener("click", () => fitVisibleGraph());
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
canvas.addEventListener("pointerleave", onPointerUp);
canvas.addEventListener("wheel", onWheel, { passive: false });
window.addEventListener("resize", resizeCanvas);

applyLocale();
resizeCanvas();
init();

function t(key, vars = {}) {
  return translate(key, key, vars);
}

function translate(key, fallback, vars = {}) {
  const dictionary = I18N[state.locale] || I18N[DEFAULT_LOCALE] || {};
  const defaultDictionary = I18N[DEFAULT_LOCALE] || {};
  const template = dictionary[key] ?? defaultDictionary[key] ?? fallback;
  return String(template).replace(/\{([A-Za-z0-9_]+)\}/g, (_, name) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : `{${name}}`,
  );
}

function setLocale(locale) {
  state.locale = I18N[locale] ? locale : DEFAULT_LOCALE;
  try {
    window.localStorage?.setItem("codegraph.locale", state.locale);
  } catch (error) {
    // Local storage can be disabled; the in-memory locale still works.
  }
  applyLocale();
}

function setLabelMode(mode) {
  if (!LABEL_MODES.has(mode)) return;
  state.labelMode = mode;
  try {
    window.localStorage?.setItem(LABEL_MODE_STORAGE_KEY, mode);
    window.localStorage?.setItem(LABEL_MODE_STORAGE_VERSION_KEY, LABEL_MODE_STORAGE_VERSION);
  } catch (error) {
    // Local storage can be disabled; the in-memory label mode still works.
  }
  renderViewportControls();
  draw();
}

function applyLocale() {
  document.documentElement.lang = state.locale;
  if (localeSelect.value !== state.locale) localeSelect.value = state.locale;
  document.querySelectorAll("[data-i18n]").forEach((element) => {
    const key = element.dataset.i18n;
    if (key) element.textContent = t(key);
  });
  if (!state.graphPage.root && !state.graph.nodes.length) {
    rootLabel.textContent = t("root.empty");
  }
  if (statusEl.dataset.status) {
    statusEl.textContent = translate(`status.${statusEl.dataset.status}`, statusEl.dataset.status);
  } else {
    statusEl.textContent = t("status.idle");
  }
  if (!state.projects.length) renderProjects();
  if (!state.selectedId && selectionTitle.dataset.i18nFallback) {
    selectionTitle.textContent = t(selectionTitle.dataset.i18nFallback);
  }
  renderViewportControls();
  renderOverview();
  renderRuntimeMetrics();
  renderJobQueue();
  renderInsights();
  renderSelection();
  draw();
}

async function init() {
  await Promise.all([loadProjects(), loadCapabilities(), loadMetrics()]);
  loadJobQueue();
  scan();
}

async function loadProjects() {
  try {
    const response = await fetch("/api/projects");
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "projects failed");
    }
    state.projects = body;
    renderProjects();
  } catch (error) {
    state.projects = [];
    projectSelect.innerHTML = `<option value=".">${escapeHtml(t("project.currentRoot"))}</option>`;
  }
}

async function loadCapabilities() {
  try {
    const response = await fetch("/api/capabilities");
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "capabilities failed");
    }
    state.capabilities = body;
  } catch (error) {
    state.capabilities = null;
  }
  renderOverview();
}

async function loadMetrics() {
  state.metricsRequest += 1;
  const requestId = state.metricsRequest;
  metricsRefreshButton.disabled = true;

  try {
    const response = await fetch("/api/metrics");
    const body = await response.json();
    if (requestId !== state.metricsRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "metrics failed");
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
  const response = await fetch(`${endpoint}?limit=8`);
  const body = await response.json();
  if (!response.ok) {
    throw new Error(body.error || `${kind} jobs failed`);
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
    const response = await fetch(`${endpoint}/${encodeURIComponent(jobId)}`, { method: "DELETE" });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "job cancel failed");
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
  state.insightRequest += 1;
  state.overviewRequest += 1;
  state.summary = null;
  state.scanOptions = null;
  state.coverage = null;
  state.lsp = null;
  state.semanticReadiness = null;
  state.semanticPlan = null;
  state.architecture = null;
  state.languageDependencies = null;
  state.hotspots = null;
  state.architecturePathPrefix = "";
  state.entrypoints = [];
  renderOverview();
  state.insightReport = null;
  renderInsights();
  checkResult.innerHTML = "";
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
    const response = await fetch("/api/scan-jobs", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ path: pathInput.value.trim() || "." }),
    });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "failed to start scan");
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
    const response = await fetch(`/api/scan-jobs/${encodeURIComponent(jobId)}`, { method: "DELETE" });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "failed to cancel scan");
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
    const events = new EventSource(`/api/scan-jobs/${encodeURIComponent(jobId)}/events`);
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
    const response = await fetch(`/api/scan-jobs/${encodeURIComponent(jobId)}`);
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "scan status failed");
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

  if (resetPage) {
    state.graphPage.nodeOffset = 0;
  }

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
    const response = await fetch(`/api/graph?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.pageRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "graph page failed");
    }

    state.graph = { nodes: body.nodes, edges: body.edges };
    state.graphPage.totalNodes = body.total_nodes;
    state.graphPage.totalEdges = body.total_edges;
    state.graphPage.nodeOffset = body.node_offset;
    state.graphPage.nodeLimit = body.node_limit;
    state.graphPage.edgeLimit = body.edge_limit;
    state.graphPage.truncatedNodes = body.truncated_nodes;
    state.graphPage.root = root || state.graphPage.root || pathInput.value.trim() || ".";
    state.selectedId = null;
    state.hoveredId = null;
    state.queryFocus = null;
    state.insightReport = null;
    queryResult.innerHTML = "";
    checkResult.innerHTML = "";
    exportResult.innerHTML = "";
    entryFlowResult.innerHTML = "";
    pathResult.innerHTML = "";
    configTraceResult.innerHTML = "";
    errorTraceResult.innerHTML = "";
    rootLabel.textContent = state.graphPage.root;
    initializeGraph({ preserveView: !resetLayout });
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

  try {
    const [
      summaryResponse,
      entrypointsResponse,
      scanOptionsResponse,
      coverageResponse,
      lspResponse,
      semanticReadinessResponse,
      semanticPlanResponse,
      architectureResponse,
      languageDependenciesResponse,
      hotspotsResponse,
    ] = await Promise.all([
      fetch(`/api/summary?${params.toString()}`),
      fetch(`/api/entrypoints?${params.toString()}`),
      fetch(`/api/scan-options?${params.toString()}`),
      fetch(`/api/coverage?${params.toString()}`),
      fetch("/api/lsp"),
      fetch(`/api/semantic-readiness?${params.toString()}`),
      fetch(`/api/semantic-plan?${semanticParams.toString()}`),
      fetch(`/api/architecture?${params.toString()}&group_limit=8&edge_limit=40`),
      fetch(`/api/language-dependencies?${params.toString()}&limit=8`),
      fetch(`/api/hotspots?${params.toString()}&limit=8`),
    ]);
    const summary = await summaryResponse.json();
    const entrypoints = await entrypointsResponse.json();
    const scanOptions = await scanOptionsResponse.json();
    const coverage = await coverageResponse.json();
    const lsp = await lspResponse.json();
    const semanticReadiness = await semanticReadinessResponse.json();
    const semanticPlan = await semanticPlanResponse.json();
    const architecture = await architectureResponse.json();
    const languageDependencies = await languageDependenciesResponse.json();
    const hotspots = await hotspotsResponse.json();
    if (requestId !== state.overviewRequest) return;
    if (!summaryResponse.ok) {
      throw new Error(summary.error || "summary failed");
    }
    if (!entrypointsResponse.ok) {
      throw new Error(entrypoints.error || "entrypoints failed");
    }
    if (!scanOptionsResponse.ok) {
      throw new Error(scanOptions.error || "scan options failed");
    }
    if (!coverageResponse.ok) {
      throw new Error(coverage.error || "coverage failed");
    }
    if (!lspResponse.ok) {
      throw new Error(lsp.error || "lsp status failed");
    }
    if (!semanticReadinessResponse.ok) {
      throw new Error(semanticReadiness.error || "semantic readiness failed");
    }
    if (!semanticPlanResponse.ok) {
      throw new Error(semanticPlan.error || "semantic plan failed");
    }
    if (!architectureResponse.ok) {
      throw new Error(architecture.error || "architecture failed");
    }
    if (!languageDependenciesResponse.ok) {
      throw new Error(languageDependencies.error || "language dependencies failed");
    }
    if (!hotspotsResponse.ok) {
      throw new Error(hotspots.error || "hotspots failed");
    }
    state.summary = summary;
    state.scanOptions = scanOptions;
    state.coverage = coverage;
    state.lsp = lsp;
    state.semanticReadiness = semanticReadiness;
    state.semanticPlan = semanticPlan;
    state.architecture = architecture;
    state.languageDependencies = languageDependencies;
    state.hotspots = hotspots;
    state.entrypoints = entrypoints;
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
    lspList.innerHTML = "";
    semanticWorkList.innerHTML = "";
    architectureList.innerHTML = "";
    languageDependencyList.innerHTML = "";
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
    request_timeout_ms: 30_000,
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
    const response = await fetch("/api/semantic-jobs", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    const job = await response.json();
    if (requestId !== state.semanticEnrichRequest) return;
    if (!response.ok) {
      throw new Error(job.error || "semantic enrichment failed");
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
    const response = await fetch(`/api/semantic-jobs/${encodeURIComponent(jobId)}`, { method: "DELETE" });
    const body = await response.json();
    if (!response.ok) {
      throw new Error(body.error || "failed to cancel semantic enrichment");
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
    const events = new EventSource(`/api/semantic-jobs/${encodeURIComponent(jobId)}/events`);
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
    const response = await fetch(`/api/semantic-jobs/${encodeURIComponent(jobId)}`);
    const job = await response.json();
    if (!response.ok) {
      throw new Error(job.error || "semantic status failed");
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
  const response = await fetch(`/api/semantic-jobs/${encodeURIComponent(jobId)}/result`);
  const body = await response.json();
  if (requestId !== state.semanticEnrichRequest || state.semanticJobId !== jobId) return;
  if (!response.ok) {
    throw new Error(body.error || "semantic result failed");
  }
  applySemanticEnrichResult(body.result, body.root || pathInput.value.trim() || ".");
}

function applySemanticEnrichResult(result, root) {
  state.graph = result.graph || { nodes: [], edges: [] };
  state.summary = result.summary || null;
  state.graphPage.root = root;
  state.graphPage.nodeOffset = 0;
  state.graphPage.totalNodes = state.graph.nodes.length;
  state.graphPage.totalEdges = state.graph.edges.length;
  state.graphPage.truncatedNodes = false;
  state.selectedId = null;
  state.hoveredId = null;
  state.queryFocus = null;
  state.insightReport = null;
  queryResult.innerHTML = "";
  checkResult.innerHTML = "";
  rootLabel.textContent = state.graphPage.root;
  initializeGraph({ preserveView: false });
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
  renderLspStatus(state.lsp, state.semanticReadiness, state.semanticPlan);
  renderSemanticWorkFilterOptions(summary);
  renderSemanticWork(state.semanticPlan);
  renderArchitecture(state.architecture);
  renderLanguageDependencies(state.languageDependencies);
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
      : '<p class="empty">No annotations.</p>';

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
      : '<p class="empty">No entrypoints.</p>';

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
      focusNodeId(Number(button.dataset.nodeId), "Focus: entrypoint");
    });
  });
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
  const chips = [
    [t("cap.api"), `v${Number(capabilities.api_version || 0)}`],
    [t("cap.graph"), `v${Number(capabilities.graph_schema_version || 0)}`],
    [t("cap.cache"), cache.enabled ? t("cap.on") : t("cap.off")],
    [t("cap.languages"), String(languages.length)],
    [t("cap.exports"), String(exportFormats.length)],
    [t("cap.projects"), String(projects.length)],
    [t("cap.scanJobs"), `${Number(limits.max_scan_concurrency || 0)}/${Number(limits.max_scan_jobs || 0)}`],
    [t("cap.semanticJobs"), `${Number(limits.max_semantic_concurrency || 0)}/${Number(limits.max_semantic_jobs || 0)}`],
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
    scanPolicyList.innerHTML = '<p class="empty">No scan policy.</p>';
    return;
  }

  const ignoredNames = Array.isArray(options.ignored_names) ? options.ignored_names : [];
  const ignoredGlobs = Array.isArray(options.ignored_globs) ? options.ignored_globs : [];
  const chips = [
    ["Max file", formatBytes(Number(options.max_file_size || 0))],
    ["Policy", options.config_path ? ".codegraph" : "defaults"],
    ["Ignore names", String(ignoredNames.length)],
    ["Ignore globs", String(ignoredGlobs.length)],
    ["Hidden", options.include_hidden ? "yes" : "no"],
    ["Git ignored", options.include_ignored ? "yes" : "no"],
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
    coverageList.innerHTML = '<p class="empty">No coverage.</p>';
    return;
  }

  const chips = [
    ["Indexed", String(coverage.indexed_files || 0)],
    ["Large", String(coverage.skipped_large_files || 0)],
    ["Policy skipped", String(coverage.skipped_policy_entries || 0)],
    ["Other files", String(coverage.non_index_files || 0)],
    ["Indexed bytes", formatBytes(Number(coverage.indexed_bytes || 0))],
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

function renderLspStatus(report, readiness, plan) {
  if (!report && !readiness && !plan) {
    lspList.innerHTML = '<p class="empty">No LSP status.</p>';
    return;
  }

  const servers = Array.isArray(report?.servers) ? report.servers : [];
  const chips = servers.slice(0, 8).map(
    (server) => `
      <div class="lsp-chip ${server.installed ? "available" : "missing"}">
        <span>${escapeHtml(server.id || "lsp")}</span>
        <strong>${server.installed ? "ready" : "missing"}</strong>
      </div>
    `,
  );
  chips.unshift(`
    <div class="lsp-chip">
      <span>Semantic</span>
      <strong>${Number(readiness?.covered_languages ?? report?.available_servers ?? 0)}/${Number(readiness?.total_languages ?? report?.total_servers ?? 0)}</strong>
    </div>
  `);
  if (readiness) {
    chips.splice(
      1,
      0,
      `
        <div class="lsp-chip ${Number(readiness.missing_languages || 0) === 0 ? "available" : "missing"}">
          <span>Missing semantic</span>
          <strong>${Number(readiness.missing_languages || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>Candidate nodes</span>
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
          <span>Semantic plan</span>
          <strong>${Number(plan.ready_languages || 0)}/${Number(plan.total_languages || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>Definitions</span>
          <strong>${Number(plan.planned_requests?.definitions || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>Symbols</span>
          <strong>${Number(plan.planned_requests?.document_symbols || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>Workspace</span>
          <strong>${Number(plan.planned_requests?.workspace_symbols || 0)}</strong>
        </div>
        <div class="lsp-chip">
          <span>Work queue</span>
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
        <strong>needed</strong>
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
        <strong>no server</strong>
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
        <strong>${language.status === "ready" ? `${requestCount} ops` : escapeHtml(formatKind(language.status || "blocked"))}</strong>
      </div>
    `);
  });
  lspList.innerHTML = chips.join("");
}

function renderSemanticWork(plan) {
  const items = Array.isArray(plan?.work_items) ? plan.work_items.slice(0, 8) : [];
  if (items.length === 0) {
    semanticWorkList.innerHTML = '<p class="empty">No semantic work items.</p>';
    return;
  }

  const filter = renderSemanticWorkFilterLabel(plan.work_item_filter);
  const truncated = plan.truncated_work_items
    ? `<span>${items.length}/${Number(plan.total_work_items || items.length)} shown</span>`
    : `<span>${items.length} queued</span>`;
  semanticWorkList.innerHTML = `
    <div class="semantic-work-summary">
      <strong>Semantic work</strong>
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
    const response = await fetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "focus failed");
    }
    const selectedId = item.node?.id || item.target?.id || null;
    showFocusedGraph(body, `Semantic: ${formatKind(item.capability || item.kind)}`, selectedId);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderArchitecture(architecture) {
  if (!architecture) {
    architectureList.innerHTML = '<p class="empty">No architecture map.</p>';
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
        <span>All areas</span>
        <strong>reset</strong>
      </button>
    `);
  }
  groupChips.push(`
    <div class="architecture-chip">
      <span>Area edges</span>
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
    const response = await fetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "focus failed");
    }
    showFocusedGraph(body, `Focus: ${edge.source || "area"} -> ${edge.target || "area"}`);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderLanguageDependencies(report) {
  if (!report) {
    languageDependencyList.innerHTML = '<p class="empty">No language dependencies.</p>';
    return;
  }

  const links = Array.isArray(report.links) ? report.links.slice(0, 8) : [];
  const chips = links.map(
    (link, index) => `
      <button class="language-dependency-chip" type="button" data-language-dependency-index="${index}">
        <span>${escapeHtml(link.source_language || "unknown")} -> ${escapeHtml(link.target_language || "unknown")}</span>
        <strong>${Number(link.count || 0)}</strong>
      </button>
    `,
  );
  chips.unshift(`
    <div class="language-dependency-chip">
      <span>Cross-language</span>
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
    const response = await fetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "focus failed");
    }
    showFocusedGraph(
      body,
      `Focus: ${link.source_language || "unknown"} -> ${link.target_language || "unknown"}`,
    );
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function renderHotspots(report) {
  if (!report) {
    hotspotList.innerHTML = '<p class="empty">No hotspots.</p>';
    return;
  }

  const hotspots = Array.isArray(report.hotspots) ? report.hotspots.slice(0, 8) : [];
  hotspotList.innerHTML =
    hotspots.length > 0
      ? hotspots
          .map(
            (hotspot) => `
              <button class="hotspot-chip" type="button" data-hotspot-node-id="${hotspot.node?.id || ""}">
                <span>${escapeHtml(hotspot.node?.label || "unknown")}</span>
                <strong>${Number(hotspot.score || 0)}</strong>
              </button>
            `,
          )
          .join("")
      : '<p class="empty">No hotspots.</p>';
  hotspotList.querySelectorAll("[data-hotspot-node-id]").forEach((button) => {
    button.addEventListener("click", () => {
      focusNodeId(Number(button.dataset.hotspotNodeId), "Focus: hotspot");
    });
  });
}

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
  loadGraphPage({ resetLayout: true });
}

function updateGraphPageControls() {
  const start = state.graphPage.totalNodes === 0 ? 0 : state.graphPage.nodeOffset + 1;
  const end = Math.min(
    state.graphPage.totalNodes,
    state.graphPage.nodeOffset + state.graphPage.nodeLimit,
  );
  pageInfo.textContent = `${start}-${end} / ${state.graphPage.totalNodes}`;
  pagePrevButton.disabled = state.graphPage.nodeOffset === 0;
  pageNextButton.disabled = !state.graphPage.truncatedNodes;
  pageReloadButton.disabled = false;
}

async function runEntryFlowTrace() {
  const depth = clampNumber(Number(entryFlowDepthInput.value || 3), 1, 32);
  entryFlowDepthInput.value = String(depth);
  state.entryFlowRequest += 1;
  const requestId = state.entryFlowRequest;
  entryFlowButton.disabled = true;
  entryFlowResult.innerHTML = '<p class="empty">Tracing entrypoints...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    depth: String(depth),
    limit: "25",
  });
  const search = entryFlowSearchInput.value.trim();
  if (search) params.set("search", search);

  try {
    const response = await fetch(`/api/entrypoint-traces?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.entryFlowRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "entrypoint trace failed");
    }
    entryFlowResult.innerHTML = renderEntryFlowReport(body);
    attachEntryFlowActions(entryFlowResult, body);
  } catch (error) {
    if (requestId !== state.entryFlowRequest) return;
    entryFlowResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.entryFlowRequest) {
      entryFlowButton.disabled = false;
    }
  }
}

function renderEntryFlowReport(report) {
  const summary = `
    <div class="query-summary">
      <span>${report.total_entrypoints} entrypoints</span>
      <span>${report.traces.length} traces</span>
      <span>depth ${report.max_depth}</span>
    </div>
  `;
  if (!report.traces.length) {
    return `${summary}<p class="empty">No matching entrypoint flows.</p>`;
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
      const truncated = trace.truncated ? '<p class="empty">Trace truncated by depth.</p>' : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(trace.start.label)}</h3>
          <div class="trace-summary">
            <span>${trace.nodes.length} nodes</span>
            <span>${trace.edges.length} edges</span>
            <span>${escapeHtml(formatKind(trace.start.metadata?.entrypoint_kind || trace.start.kind))}</span>
          </div>
          <div class="query-actions">
            <button type="button" data-entry-flow="${index}">Focus flow</button>
          </div>
          ${nodes ? `<ul class="trace-list">${nodes}</ul>` : '<p class="empty">No outgoing dependency edges.</p>'}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = report.truncated ? '<p class="empty">Report truncated by limit or depth.</p>' : "";
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
      showFocusedGraph(focused, `Entry: ${trace.start.label}`, trace.start.id);
    });
  });
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
    const response = await fetch(`/api/insights?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "insights failed");
    }
    state.insightReport = body;
    renderInsights();
  } catch (error) {
    if (requestId !== state.insightRequest) return;
    state.insightReport = null;
    renderInsights();
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
  checkResult.innerHTML = '<p class="empty">Running check...</p>';

  try {
    const response = await fetch(`/api/check?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.checkRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "check failed");
    }
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

function renderCheckReport(check) {
  const stateClass = check.passed ? "passed" : "failed";
  const label = check.passed ? "Passed" : "Failed";
  return `
    <div class="check-card ${stateClass}">
      <strong>${label}</strong>
      <span>fail on ${escapeHtml(formatKind(check.fail_on || "error"))}</span>
      <span>${check.failing_insights || 0} failing</span>
      <span>${check.report?.total || 0} matched</span>
    </div>
  `;
}

function initializeGraph(options = {}) {
  const preserveView = Boolean(options.preserveView);
  const previousPan = { ...state.pan };
  const previousZoom = state.zoom;

  state.selectedId = null;
  state.hoveredId = null;
  state.positions.clear();
  state.velocities.clear();
  const kinds = [...new Set(state.graph.nodes.map((node) => node.kind))].sort();
  state.enabledKinds = new Set(kinds);
  renderKindFilters(kinds);
  renderLegend(kinds);
  state.layoutPaused = false;
  renderViewportControls();

  seedGraphLayout();

  state.pan = preserveView ? previousPan : { x: canvas.width / 2, y: canvas.height / 2 };
  state.zoom = preserveView ? previousZoom : 1;
  applyFilters();
  startAnimation();
}

function seedGraphLayout() {
  const radius = Math.max(180, Math.min(canvas.width, canvas.height) * 0.28);
  state.graph.nodes.forEach((node, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(1, state.graph.nodes.length);
    state.positions.set(node.id, {
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
    });
    state.velocities.set(node.id, { x: 0, y: 0 });
  });
}

async function runGraphQuery() {
  const expression = queryInput.value.trim();
  if (!expression) {
    queryResult.innerHTML = '<p class="empty">Enter a query expression.</p>';
    return;
  }

  state.queryRequest += 1;
  const requestId = state.queryRequest;
  queryButton.disabled = true;
  queryResult.innerHTML = '<p class="empty">Running query...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: expression,
  });

  try {
    const response = await fetch(`/api/query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.queryRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "query failed");
    }
    queryResult.innerHTML = renderQueryResult(body);
    attachQueryNavigation(queryResult);
    attachEdgeExplainActions(queryResult);
    attachQueryFocusActions(queryResult, body);
  } catch (error) {
    if (requestId !== state.queryRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.queryRequest) {
      queryButton.disabled = false;
    }
  }
}

async function runSourceSearch() {
  const query = sourceSearchInput.value.trim();
  if (!query) {
    sourceSearchResult.innerHTML = '<p class="empty">Enter source text.</p>';
    return;
  }

  state.sourceSearchRequest += 1;
  const requestId = state.sourceSearchRequest;
  sourceSearchButton.disabled = true;
  sourceSearchResult.innerHTML = '<p class="empty">Searching source...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: query,
    limit: "50",
    context: "2",
  });
  const pathFilter = sourcePathFilterInput.value.trim();
  if (pathFilter) params.set("path_filter", pathFilter);

  try {
    const response = await fetch(`/api/source-search?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.sourceSearchRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "source search failed");
    }
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

async function loadCacheDiff() {
  const limit = clampNumber(Number(cacheDiffLimitInput.value || 50), 1, 10000);
  cacheDiffLimitInput.value = String(limit);
  state.cacheDiffRequest += 1;
  const requestId = state.cacheDiffRequest;
  cacheDiffButton.disabled = true;
  cacheDiffStatus.textContent = "loading";
  cacheDiffResult.innerHTML = '<p class="empty">Loading cache diagnostics...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await fetch(`/api/cache-diff?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.cacheDiffRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "cache diff failed");
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
  cacheDiffStatus.textContent = "chunks";
  cacheDiffResult.innerHTML = '<p class="empty">Loading cache chunks...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await fetch(`/api/cache-chunks?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.cacheChunksRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "cache chunks failed");
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
  cacheDiffStatus.textContent = "planning";
  cacheDiffResult.innerHTML = '<p class="empty">Planning incremental scan...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await fetch(`/api/incremental-plan?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.incrementalPlanRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "incremental plan failed");
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
  cacheDiffStatus.textContent = "scanning";
  cacheDiffResult.innerHTML = '<p class="empty">Scanning changed files...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await fetch(`/api/incremental-scan?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.incrementalScanRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "incremental scan failed");
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
  cacheDiffStatus.textContent = "merging";
  cacheDiffResult.innerHTML = '<p class="empty">Building merge preview...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await fetch(`/api/incremental-merge-preview?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.incrementalMergeRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "incremental merge preview failed");
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
  cacheDiffStatus.textContent = "updating";
  cacheDiffResult.innerHTML = '<p class="empty">Updating graph cache...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    limit: String(limit),
  });

  try {
    const response = await fetch(`/api/incremental-update?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.incrementalUpdateRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "incremental update failed");
    }
    const plan = body.preview?.plan || {};
    cacheDiffStatus.textContent = body.cache?.stored ? "stored" : formatKind(plan.action || "skipped");
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
  exportResult.innerHTML = '<p class="empty">Exporting...</p>';

  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  if (metadata.endpoint === "/api/export") {
    params.set("format", metadata.format);
  }

  try {
    const response = await fetch(`${metadata.endpoint}?${params.toString()}`);
    if (requestId !== state.exportRequest) return;
    if (!response.ok) {
      throw new Error(await responseErrorMessage(response, "export failed"));
    }

    const blob = await response.blob();
    if (requestId !== state.exportRequest) return;
    const fileName = `codegraph-${safeFilePart(pathInput.value.trim() || state.graphPage.root || "project")}.${metadata.extension}`;
    downloadBlob(blob, fileName);
    exportResult.innerHTML = `
      <div class="query-summary">
        <span>${escapeHtml(metadata.label)}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
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

async function responseErrorMessage(response, fallback) {
  const contentType = response.headers.get("content-type") || "";
  if (contentType.includes("application/json")) {
    try {
      const body = await response.json();
      return body.error || fallback;
    } catch (error) {
      return fallback;
    }
  }
  const text = await response.text();
  return text.trim() || fallback;
}

function exportFormatMetadata(format) {
  switch (format) {
    case "dot":
      return { format: "dot", extension: "dot", label: "DOT", endpoint: "/api/export" };
    case "ndjson":
      return { format: "ndjson", extension: "ndjson", label: "NDJSON", endpoint: "/api/export" };
    case "report":
      return { format: "report", extension: "report.json", label: t("export.report"), endpoint: "/api/report" };
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

function renderCacheDiff(report) {
  const added = report.added || [];
  const modified = report.modified || [];
  const removed = report.removed || [];
  const previousHash = report.previous_hash || "no previous fingerprint";
  const changedCount = added.length + modified.length + removed.length;
  const totalChanged = Number(report.changed_files ?? changedCount);
  const reusableFiles = Number(report.reusable_files ?? report.unchanged ?? 0);
  const currentFiles = Number(report.current_files ?? 0);
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(formatKind(report.cache_record || "unknown"))}</span>
      <span>${escapeHtml(formatKind(report.reuse_strategy || "unknown"))}</span>
      <span>${report.previous_files ?? 0} -> ${report.current_files ?? 0} files</span>
      <span>${formatBytes(report.previous_bytes)} -> ${formatBytes(report.current_bytes)}</span>
      <span>${totalChanged} changed</span>
      <span>${reusableFiles}/${currentFiles} reusable</span>
      <span>${formatBasisPoints(report.reuse_file_ratio_basis_points)} file reuse</span>
      <span>${formatBasisPoints(report.reuse_byte_ratio_basis_points)} byte reuse</span>
      <span>${formatBytes(report.changed_current_bytes)} changed current</span>
      <span>${formatBytes(report.reusable_bytes)} reusable</span>
      <span>${changedCount} listed</span>
      ${report.truncated ? "<span>truncated</span>" : ""}
      <span class="query-expression">previous ${escapeHtml(previousHash)}</span>
      <span class="query-expression">current ${escapeHtml(report.current_hash || "unknown")}</span>
    </div>
  `;

  const groups = [
    renderCacheDiffGroup("Added", added, renderCacheDiffEntry),
    renderCacheDiffGroup("Modified", modified, renderCacheDiffChange),
    renderCacheDiffGroup("Removed", removed, renderCacheDiffEntry),
  ]
    .filter(Boolean)
    .join("");

  if (!groups) {
    return `${summary}<p class="empty">No file fingerprint changes detected.</p>`;
  }

  return `${summary}${groups}`;
}

function renderCacheChunks(report) {
  const chunks = report.chunks || [];
  const previousHash = report.previous_hash || "no previous fingerprint";
  const listed = chunks.length;
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(formatKind(report.cache_record || "unknown"))}</span>
      <span>${Number(report.total_chunks || 0)} chunks</span>
      <span>${Number(report.total_chunk_nodes || 0)} unique nodes</span>
      <span>${Number(report.total_chunk_edges || 0)} unique edges</span>
      <span>${listed}/${Number(report.total_chunks || 0)} listed</span>
      <span>${report.previous_files ?? 0} -> ${report.current_files ?? 0} files</span>
      ${report.truncated ? "<span>truncated</span>" : ""}
      <span class="query-expression">previous ${escapeHtml(previousHash)}</span>
      <span class="query-expression">current ${escapeHtml(report.current_hash || "unknown")}</span>
    </div>
  `;
  const groups = renderCacheDiffGroup("Chunks", chunks, renderCacheChunkEntry);
  if (!groups) {
    return `${summary}<p class="empty">No cached graph chunks available.</p>`;
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
      <span>${Number(plan.changed_files || 0)} changed</span>
      <span>${Number(plan.rescan_files || 0)} rescan</span>
      <span>${Number(plan.removed_files || 0)} removed</span>
      <span>${Number(plan.reusable_files || 0)} reusable</span>
      <span>${Number(plan.impacted_nodes || 0)} graph nodes</span>
      <span>${Number(plan.impacted_edges || 0)} graph edges</span>
      <span>${formatBasisPoints(plan.reuse_file_ratio_basis_points)} file reuse</span>
      <span>${formatBasisPoints(plan.reuse_byte_ratio_basis_points)} byte reuse</span>
      <span>${formatBytes(plan.changed_current_bytes)} changed current</span>
      <span>${formatBytes(plan.reusable_bytes)} reusable</span>
      ${plan.truncated ? "<span>truncated</span>" : ""}
      <span class="query-expression">${escapeHtml(plan.reason || "")}</span>
    </div>
  `;
  const groups = [
    renderCacheDiffGroup("Scan", scanPaths, renderPlanPath),
    renderCacheDiffGroup("Removed", removedPaths, renderPlanPath),
    renderCacheDiffGroup("Reusable", reusablePaths, renderPlanPath),
    renderCacheDiffGroup("Node IDs", impactedNodeIds, renderPlanScalar),
    renderCacheDiffGroup("Edge Indexes", impactedEdgeIndexes, renderPlanScalar),
  ]
    .filter(Boolean)
    .join("");

  if (!groups) {
    return `${summary}<p class="empty">No incremental scan work needed.</p>`;
  }

  return `${summary}${groups}`;
}

function renderIncrementalScan(scan) {
  const graph = scan.graph || { nodes: [], edges: [] };
  const plan = scan.plan || {};
  const graphSummary = `
    <div class="query-summary">
      <span>${Number(graph.nodes?.length || 0)} scanned nodes</span>
      <span>${Number(graph.edges?.length || 0)} scanned edges</span>
      <span>${Number(plan.scan_paths?.length || 0)} listed paths</span>
      ${plan.truncated ? "<span>limited scope</span>" : ""}
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
    ? `<p class="empty">${escapeHtml(merge.warning)}</p>`
    : "";
  const blockerGroup = renderCacheDiffGroup("Completeness blockers", blockers, renderMergeBlocker);
  const graphSummary = `
    <div class="query-summary">
      <span>${Number(graph.nodes?.length || 0)} preview nodes</span>
      <span>${Number(graph.edges?.length || 0)} preview edges</span>
      <span>${Number(merge.reused_nodes || 0)} reused nodes</span>
      <span>${Number(merge.reused_edges || 0)} reused edges</span>
      <span>${Number(merge.removed_cached_nodes || 0)} removed cached nodes</span>
      <span>${Number(merge.removed_cached_edges || 0)} removed cached edges</span>
      <span>${Number(merge.chunk_removed_nodes || 0)} chunk nodes</span>
      <span>${Number(merge.chunk_removed_edges || 0)} chunk edges</span>
      <span>${Number(merge.scanned_nodes || 0)} scanned nodes</span>
      <span>${Number(merge.scanned_edges || 0)} scanned edges</span>
      <span>${Number(merge.replaced_paths || 0)} replaced paths</span>
      <span>${Number(merge.incoming_cross_file_edges || 0)} incoming blockers</span>
      <span>${Number(merge.graph_surface_added || 0)} surface added</span>
      <span>${Number(merge.graph_surface_removed || 0)} surface removed</span>
      <span>${Number(merge.removed_paths_blocking || 0)} removed paths</span>
      <span>${Number(blockers.length || 0)} blockers</span>
      <span>${merge.complete_graph ? "complete" : "preview"}</span>
    </div>
  `;
  return `${graphSummary}${warning}${blockerGroup}${renderIncrementalPlan(plan)}`;
}

function renderIncrementalUpdate(update) {
  const cache = update.cache || {};
  const status = cache.stored ? "stored" : "not stored";
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(status)}</span>
      <span>${escapeHtml(cache.reason || "")}</span>
      <span class="query-expression">previous ${escapeHtml(cache.previous_hash || "none")}</span>
      <span class="query-expression">current ${escapeHtml(cache.current_hash || "unknown")}</span>
    </div>
  `;
  return `${summary}${renderIncrementalMergePreview(update.preview || {})}`;
}

function showIncrementalScanGraph(scan) {
  const graph = scan.graph || { nodes: [], edges: [] };
  const plan = scan.plan || {};
  state.graph = { nodes: graph.nodes || [], edges: graph.edges || [] };
  state.graphPage.nodeOffset = 0;
  state.graphPage.totalNodes = state.graph.nodes.length;
  state.graphPage.totalEdges = state.graph.edges.length;
  state.graphPage.truncatedNodes = false;
  state.selectedId = null;
  state.hoveredId = null;
  state.queryFocus = null;
  rootLabel.textContent = `Changed: ${formatKind(plan.action || "scan")}`;
  initializeGraph({ preserveView: false });
  pageInfo.textContent = `changed ${state.graph.nodes.length} / ${Number(plan.rescan_files || 0)}`;
  pagePrevButton.disabled = true;
  pageNextButton.disabled = true;
  pageReloadButton.disabled = false;
}

function showIncrementalMergePreviewGraph(preview) {
  const graph = preview.graph || { nodes: [], edges: [] };
  const merge = preview.merge || {};
  state.graph = { nodes: graph.nodes || [], edges: graph.edges || [] };
  state.graphPage.nodeOffset = 0;
  state.graphPage.totalNodes = state.graph.nodes.length;
  state.graphPage.totalEdges = state.graph.edges.length;
  state.graphPage.truncatedNodes = false;
  state.selectedId = null;
  state.hoveredId = null;
  state.queryFocus = null;
  rootLabel.textContent = merge.complete_graph ? "Incremental: complete" : "Incremental: merge preview";
  initializeGraph({ preserveView: false });
  pageInfo.textContent = `preview ${state.graph.nodes.length} / ${Number(merge.reused_nodes || 0)} reused`;
  pagePrevButton.disabled = true;
  pageNextButton.disabled = true;
  pageReloadButton.disabled = false;
}

function renderPlanPath(path) {
  return `
    <div class="query-item cache-diff-item">
      <span>path</span>
      <strong>${escapeHtml(path || "")}</strong>
    </div>
  `;
}

function renderPlanScalar(value) {
  return `
    <div class="query-item cache-diff-item">
      <span>id</span>
      <strong>${escapeHtml(String(value ?? ""))}</strong>
    </div>
  `;
}

function renderMergeBlocker(blocker) {
  return `
    <div class="query-item cache-diff-item">
      <span>${Number(blocker.count || 0)}</span>
      <strong>${escapeHtml(formatKind(blocker.kind || "blocker"))}</strong>
      <span>${escapeHtml(blocker.message || "")}</span>
    </div>
  `;
}

function renderCacheChunkEntry(chunk) {
  const nodePreview = (chunk.node_ids || []).slice(0, 6).join(", ");
  const edgePreview = (chunk.edge_indexes || []).slice(0, 6).join(", ");
  const preview = [nodePreview && `n ${nodePreview}`, edgePreview && `e ${edgePreview}`]
    .filter(Boolean)
    .join(" | ");
  return `
    <div class="query-item cache-diff-item">
      <span>${Number(chunk.nodes || 0)} nodes / ${Number(chunk.edges || 0)} edges</span>
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
    pathResult.innerHTML = '<p class="empty">Enter both path endpoints.</p>';
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

  state.pathRequest += 1;
  const requestId = state.pathRequest;
  pathButton.disabled = true;
  pathResult.innerHTML = '<p class="empty">Finding path...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: expression,
  });

  try {
    const response = await fetch(`/api/query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.pathRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "path query failed");
    }
    pathResult.innerHTML = renderQueryResult(body, { label: "Path" });
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

async function runConfigTrace() {
  const target = configTraceTargetInput.value.trim();
  if (!target) {
    configTraceResult.innerHTML = '<p class="empty">Enter a config file or environment variable.</p>';
    return;
  }

  const depth = clampNumber(Number(configTraceDepthInput.value || 6), 1, 32);
  configTraceDepthInput.value = String(depth);
  state.configTraceRequest += 1;
  const requestId = state.configTraceRequest;
  configTraceButton.disabled = true;
  configTraceResult.innerHTML = '<p class="empty">Tracing config...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    target,
    depth: String(depth),
    limit: "50",
  });

  try {
    const response = await fetch(`/api/trace-config?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.configTraceRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "config trace failed");
    }
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

function renderConfigTrace(result) {
  const summary = `
    <div class="query-summary">
      <span>${result.total_matches} targets</span>
      <span>${result.total_readers} readers</span>
      <span>${result.total_paths} paths</span>
      <span>depth ${result.max_depth}</span>
      <span class="query-expression">${escapeHtml(result.target)}</span>
    </div>
  `;

  if (!result.matches.length) {
    return `${summary}<p class="empty">No matching config or environment nodes.</p>`;
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
      const truncated = match.truncated ? '<p class="empty">Trace truncated.</p>' : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(match.target.label)}</h3>
          <div class="trace-summary">
            <span>${match.total_readers} readers</span>
            <span>${match.total_paths} paths</span>
            <span>${escapeHtml(formatKind(match.target.kind))}</span>
          </div>
          ${readers ? `<ul class="trace-list">${readers}</ul>` : '<p class="empty">No direct readers.</p>'}
          ${paths ? `<ul class="trace-list">${paths}</ul>` : ""}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = result.truncated ? '<p class="empty">Result truncated by limit.</p>' : "";
  return `${summary}${rows}${truncated}`;
}

function renderConfigTracePath(path, matchIndex, pathIndex) {
  const labels = path.nodes.map((node) => node.label).join(" -> ");
  const kind = path.reached_entrypoint ? "entrypoint path" : "reader path";
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
      const selectedId = path.nodes[path.nodes.length - 1]?.id || null;
      showFocusedGraph(focused, `Config: ${match.target.label}`, selectedId);
    });
  });
}

async function runErrorTrace() {
  const target = errorTraceTargetInput.value.trim();
  if (!target) {
    errorTraceResult.innerHTML = '<p class="empty">Enter an error or exception label.</p>';
    return;
  }

  const depth = clampNumber(Number(errorTraceDepthInput.value || 6), 1, 32);
  errorTraceDepthInput.value = String(depth);
  state.errorTraceRequest += 1;
  const requestId = state.errorTraceRequest;
  errorTraceButton.disabled = true;
  errorTraceResult.innerHTML = '<p class="empty">Tracing errors...</p>';

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    target,
    depth: String(depth),
    limit: "50",
  });

  try {
    const response = await fetch(`/api/trace-errors?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.errorTraceRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "error trace failed");
    }
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

function renderErrorTrace(result) {
  const summary = `
    <div class="query-summary">
      <span>${result.total_matches} errors</span>
      <span>${result.total_sources} sources</span>
      <span>${result.total_paths} paths</span>
      <span>depth ${result.max_depth}</span>
      <span class="query-expression">${escapeHtml(result.target)}</span>
    </div>
  `;

  if (!result.matches.length) {
    return `${summary}<p class="empty">No matching error nodes.</p>`;
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
      const truncated = match.truncated ? '<p class="empty">Trace truncated.</p>' : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(match.error.label)}</h3>
          <div class="trace-summary">
            <span>${match.total_sources} sources</span>
            <span>${match.total_paths} paths</span>
            <span>${escapeHtml(formatKind(match.error.metadata?.language || match.error.kind))}</span>
          </div>
          ${sources ? `<ul class="trace-list">${sources}</ul>` : '<p class="empty">No direct sources.</p>'}
          ${paths ? `<ul class="trace-list">${paths}</ul>` : ""}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = result.truncated ? '<p class="empty">Result truncated by limit.</p>' : "";
  return `${summary}${rows}${truncated}`;
}

function renderErrorTracePath(path, matchIndex, pathIndex) {
  const labels = path.nodes.map((node) => node.label).join(" -> ");
  const kind = path.reached_entrypoint ? "entrypoint path" : "source path";
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
      const selectedId = path.nodes[path.nodes.length - 1]?.id || null;
      showFocusedGraph(focused, `Error: ${match.error.label}`, selectedId);
    });
  });
}

function renderSourceSearchResult(result) {
  const summary = `
    <div class="query-summary">
      <span>${result.total_matches} matches</span>
      ${result.truncated ? "<span>truncated</span>" : ""}
      <span class="query-expression">${escapeHtml(result.query)}</span>
    </div>
  `;
  const rows = (result.matches || [])
    .map((match, index) => renderSourceSearchMatch(match, index))
    .join("");
  return `
    ${summary}
    ${rows ? `<ul class="query-list">${rows}</ul>` : '<p class="empty">No source matches.</p>'}
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
}

async function openSourceSearchMatch(match) {
  state.selectionRequest += 1;
  const requestId = state.selectionRequest;
  state.selectedId = null;
  selectionTitle.textContent = "Source Match";
  selectionBody.innerHTML = `
    <section class="source-preview">
      <header>
        <span>Source</span>
        <strong>${escapeHtml(match.path)}:${match.line}</strong>
      </header>
      <pre id="sourceMatchPreview"><code>Loading...</code></pre>
    </section>
  `;
  const preview = selectionBody.querySelector("#sourceMatchPreview code");
  const params = new URLSearchParams({
    root: pathInput.value.trim() || ".",
    path: match.path,
    start_line: String(match.line),
    end_line: String(match.line),
    context: "5",
  });

  try {
    const response = await fetch(`/api/source?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "failed to load source");
    }
    preview.innerHTML = body.lines.map(renderSourceLine).join("");
  } catch (error) {
    if (requestId !== state.selectionRequest) return;
    preview.innerHTML = `<span class="source-error">${escapeHtml(error.message)}</span>`;
  }
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

  return `
    <div class="query-summary">
      ${resultLabel}
      <span>${result.total_nodes} nodes</span>
      <span>${result.total_edges} edges</span>
      ${expression}
    </div>
    <div class="query-actions">
      <button data-focus-result type="button" ${hasResults ? "" : "disabled"}>Focus result</button>
      <button data-clear-focus type="button" ${state.queryFocus ? "" : "disabled"}>Clear focus</button>
    </div>
    ${nodeRows ? `<ul class="query-list">${nodeRows}</ul>` : ""}
    ${edgeRows ? `<ul class="query-list query-edge-list">${edgeRows}</ul>` : ""}
    ${!nodeRows && !edgeRows ? '<p class="empty">No query results.</p>' : ""}
    ${truncated}
  `;
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
        ${renderExplainEdgeButton(edge)}
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
      state.selectedId = nodeId;
      renderSelection();
    });
  });
}

function attachQueryFocusActions(container, result) {
  const focusButton = container.querySelector("[data-focus-result]");
  const clearButton = container.querySelector("[data-clear-focus]");
  if (focusButton) {
    focusButton.addEventListener("click", () => {
      focusQueryResult(result, container);
    });
  }
  if (clearButton) {
    clearButton.addEventListener("click", () => {
      clearQueryFocus();
    });
  }
}

function attachEdgeExplainActions(container) {
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

function quoteQueryValue(value) {
  if (/^[A-Za-z0-9._/@:+-]+$/.test(value)) return value;
  if (!value.includes('"')) return `"${value}"`;
  if (!value.includes("'")) return `'${value}'`;
  return `"${value.replaceAll('"', "'")}"`;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function applyFilters() {
  const query = state.search;
  const visibleIds = new Set();
  state.visibleNodes = state.graph.nodes.filter((node) => {
    const kindEnabled = state.enabledKinds.has(node.kind);
    const focusHit = !state.queryFocus || state.queryFocus.nodeIds.has(node.id);
    const searchHit =
      !query ||
      node.label.toLowerCase().includes(query) ||
      node.kind.toLowerCase().includes(query) ||
      Object.values(node.metadata || {}).some((value) =>
        String(value).toLowerCase().includes(query),
      );
    if (kindEnabled && focusHit && searchHit) visibleIds.add(node.id);
    return kindEnabled && focusHit && searchHit;
  });

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

  if (state.selectedId && !visibleIds.has(state.selectedId)) {
    state.selectedId = null;
  }
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
  const rows = ["error", "warning", "info"]
    .map((severity) => {
      const count = report.by_severity[severity] || 0;
      return `<span class="${severity}">${escapeHtml(formatKind(severity))}: ${count}</span>`;
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

function insightNodeId(insight) {
  return insightNodeIds(insight)[0] || null;
}

function insightNodeIds(insight) {
  if (Array.isArray(insight.nodes) && insight.nodes.length > 0) return insight.nodes;
  if (insight.nodeId) return [insight.nodeId];
  return [];
}

function insightEdgeIndexes(insight) {
  return Array.isArray(insight.edges) ? insight.edges : [];
}

async function focusInsight(insight) {
  const nodeIds = insightNodeIds(insight);
  const edgeIndexes = insightEdgeIndexes(insight);
  const selectedId = nodeIds[0] || null;
  if (nodeIds.length === 0 && edgeIndexes.length === 0) return;

  if (selectedId) {
    state.selectedId = selectedId;
    renderSelection();
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
    const response = await fetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "focus failed");
    }
    const label = `Focus: ${formatKind(insight.kind)}`;
    showFocusedGraph(body, label, selectedId);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function focusNodeId(nodeId, label) {
  if (!nodeId) return;
  state.selectedId = nodeId;
  renderSelection();

  state.insightFocusRequest += 1;
  const requestId = state.insightFocusRequest;
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_ids: String(nodeId),
    edge_limit: "300",
  });

  try {
    const response = await fetch(`/api/focus?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightFocusRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "focus failed");
    }
    showFocusedGraph(body, label, nodeId);
  } catch (error) {
    if (requestId !== state.insightFocusRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

function showFocusedGraph(result, label, selectedId = null) {
  state.graph = { nodes: result.nodes, edges: result.edges };
  state.graphPage.nodeOffset = 0;
  state.graphPage.totalNodes = result.total_nodes;
  state.graphPage.totalEdges = result.total_edges;
  state.graphPage.truncatedNodes = false;
  state.queryFocus = null;
  queryResult.innerHTML = renderQueryResult(result);
  attachQueryNavigation(queryResult);
  attachEdgeExplainActions(queryResult);
  attachQueryFocusActions(queryResult, result);
  rootLabel.textContent = label;
  initializeGraph({ preserveView: false });
  pageInfo.textContent = `focus ${result.nodes.length} / ${result.total_nodes}`;
  pagePrevButton.disabled = true;
  pageNextButton.disabled = true;
  pageReloadButton.disabled = false;
  if (selectedId) {
    state.selectedId = selectedId;
    renderSelection();
  }
}

function buildClientInsights(graph) {
  const insights = [];
  const entrypointIds = new Set(
    graph.edges.filter((edge) => edge.kind === "entrypoint").map((edge) => edge.target),
  );
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
        severity: "warning",
        message: `${node.label} contains syntax error nodes`,
        nodeId: node.id,
      });
    }

    if (node.metadata?.item_kind === "call" && node.metadata?.resolution === "unresolved") {
      insights.push({
        kind: "unresolved_call",
        severity: "warning",
        message: `Call target ${node.label} could not be resolved`,
        nodeId: node.id,
      });
    }

    if (node.kind === "function" && !entrypointIds.has(node.id) && !calledIds.has(node.id)) {
      insights.push({
        kind: "orphan_function",
        severity: "info",
        message: `${node.label} has no incoming call edge`,
        nodeId: node.id,
      });
    }
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

  graph.edges
    .filter((edge) => edge.kind === "may_error")
    .forEach((edge) => {
      const source = graph.nodes.find((node) => node.id === edge.source);
      const target = graph.nodes.find((node) => node.id === edge.target);
      insights.push({
        kind: "potential_error_flow",
        severity: "warning",
        message: `${source?.label || edge.source} may error via ${target?.label || edge.target}`,
        nodeId: source?.id || target?.id,
      });
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
  graph.edges
    .filter((edge) => edge.kind === "imports")
    .forEach((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      const candidate = importPackageCandidate(target?.metadata?.language, target?.label || "");
      if (!candidate) return;
      if (!declaredEcosystems.has(candidate.ecosystem)) return;
      if (isDeclaredPackage(declared, candidate.ecosystem, candidate.package)) return;

      insights.push({
        kind: "undeclared_external_import",
        severity: "warning",
        message: `${source?.label || edge.source} imports ${candidate.package} without a matching ${candidate.ecosystem} dependency`,
        nodeId: target?.id || source?.id,
      });
    });
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
  if (
    moduleName.startsWith(".") ||
    moduleName.startsWith("/") ||
    moduleName.startsWith("node:") ||
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

const nodeBuiltinModules = new Set([
  "assert",
  "buffer",
  "child_process",
  "cluster",
  "crypto",
  "dgram",
  "dns",
  "events",
  "fs",
  "http",
  "https",
  "module",
  "net",
  "os",
  "path",
  "process",
  "querystring",
  "readline",
  "stream",
  "string_decoder",
  "timers",
  "tls",
  "tty",
  "url",
  "util",
  "vm",
  "zlib",
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
    input.checked = true;
    input.addEventListener("change", () => {
      if (input.checked) state.enabledKinds.add(kind);
      else state.enabledKinds.delete(kind);
      applyFilters();
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

function renderLegend(kinds) {
  legend.innerHTML = "";
  kinds.forEach((kind) => {
    const item = document.createElement("span");
    item.className = "legend-item";
    const swatch = document.createElement("span");
    swatch.className = "swatch";
    swatch.style.background = colorFor(kind);
    const text = document.createElement("span");
    text.textContent = formatKind(kind);
    item.append(swatch, text);
    legend.append(item);
  });
}

function startAnimation() {
  if (state.animationFrame) cancelAnimationFrame(state.animationFrame);
  const tick = () => {
    if (!state.layoutPaused) {
      simulateLayout();
    }
    draw();
    state.animationFrame = requestAnimationFrame(tick);
  };
  tick();
}

function renderViewportControls() {
  viewportInfo.textContent = `${state.visibleNodes.length} ${t("stat.nodes").toLowerCase()} / ${state.visibleEdges.length} ${t("stat.edges").toLowerCase()}`;
  toggleLayoutButton.textContent = state.layoutPaused ? t("button.resume") : t("button.pause");
  toggleLayoutButton.setAttribute(
    "aria-label",
    state.layoutPaused ? "Resume graph layout" : "Pause graph layout",
  );
  fitGraphButton.disabled = state.visibleNodes.length === 0;
  resetLayoutButton.disabled = state.graph.nodes.length === 0;
  zoomInButton.disabled = state.graph.nodes.length === 0;
  zoomOutButton.disabled = state.graph.nodes.length === 0;
  toggleLayoutButton.disabled = state.graph.nodes.length === 0;
  labelModeButtons.forEach((button) => {
    const active = button.dataset.labelMode === state.labelMode;
    button.setAttribute("aria-pressed", active ? "true" : "false");
    button.disabled = state.graph.nodes.length === 0;
  });
}

function zoomAtCanvasCenter(scale) {
  zoomAt(canvas.width / 2, canvas.height / 2, scale);
}

function zoomAt(screenX, screenY, scale) {
  const before = screenToWorld(screenX, screenY);
  state.zoom = Math.max(0.18, Math.min(3.5, state.zoom * scale));
  const after = screenToWorld(screenX, screenY);
  state.pan.x += (after.x - before.x) * state.zoom;
  state.pan.y += (after.y - before.y) * state.zoom;
  draw();
}

function fitVisibleGraph() {
  if (state.visibleNodes.length === 0) return;

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    if (!position) return;
    const radius = nodeRadius(node) + 24;
    minX = Math.min(minX, position.x - radius);
    minY = Math.min(minY, position.y - radius);
    maxX = Math.max(maxX, position.x + radius);
    maxY = Math.max(maxY, position.y + radius);
  });

  if (!Number.isFinite(minX) || !Number.isFinite(minY)) return;

  const width = Math.max(1, maxX - minX);
  const height = Math.max(1, maxY - minY);
  const padding = 72;
  const zoomX = (canvas.width - padding * 2) / width;
  const zoomY = (canvas.height - padding * 2) / height;
  state.zoom = Math.max(0.18, Math.min(3.5, Math.min(zoomX, zoomY)));
  state.pan = {
    x: canvas.width / 2 - ((minX + maxX) / 2) * state.zoom,
    y: canvas.height / 2 - ((minY + maxY) / 2) * state.zoom,
  };
  draw();
}

function resetGraphLayout() {
  if (state.graph.nodes.length === 0) return;
  state.positions.clear();
  state.velocities.clear();
  seedGraphLayout();
  state.pan = { x: canvas.width / 2, y: canvas.height / 2 };
  state.zoom = 1;
  state.layoutPaused = false;
  renderViewportControls();
  draw();
}

function toggleLayout() {
  if (state.graph.nodes.length === 0) return;
  state.layoutPaused = !state.layoutPaused;
  renderViewportControls();
  draw();
}

function simulateLayout() {
  const nodes = state.visibleNodes;
  const edges = state.visibleEdges;
  if (nodes.length === 0) return;

  const visibleIds = new Set(nodes.map((node) => node.id));
  const centerPull = 0.004;
  const linkDistance = 112;
  const linkStrength = 0.012;
  const charge = 2800;

  for (let i = 0; i < nodes.length; i += 1) {
    const a = nodes[i];
    const pa = state.positions.get(a.id);
    const va = state.velocities.get(a.id);

    for (let j = i + 1; j < nodes.length; j += 1) {
      const b = nodes[j];
      const pb = state.positions.get(b.id);
      const vb = state.velocities.get(b.id);
      let dx = pa.x - pb.x;
      let dy = pa.y - pb.y;
      let distanceSq = dx * dx + dy * dy + 0.01;
      const distance = Math.sqrt(distanceSq);
      dx /= distance;
      dy /= distance;
      const force = Math.min(6, charge / distanceSq);
      va.x += dx * force;
      va.y += dy * force;
      vb.x -= dx * force;
      vb.y -= dy * force;
    }
  }

  edges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    const sourceVelocity = state.velocities.get(edge.source);
    const targetVelocity = state.velocities.get(edge.target);
    const dx = target.x - source.x;
    const dy = target.y - source.y;
    const distance = Math.max(1, Math.sqrt(dx * dx + dy * dy));
    const force = (distance - linkDistance) * linkStrength;
    const fx = (dx / distance) * force;
    const fy = (dy / distance) * force;
    sourceVelocity.x += fx;
    sourceVelocity.y += fy;
    targetVelocity.x -= fx;
    targetVelocity.y -= fy;
  });

  nodes.forEach((node) => {
    if (node.id === state.draggingId) return;
    const position = state.positions.get(node.id);
    const velocity = state.velocities.get(node.id);
    velocity.x += -position.x * centerPull;
    velocity.y += -position.y * centerPull;
    velocity.x *= 0.82;
    velocity.y *= 0.82;
    position.x += velocity.x;
    position.y += velocity.y;
  });
}

function draw() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.save();
  ctx.translate(state.pan.x, state.pan.y);
  ctx.scale(state.zoom, state.zoom);

  const visibleIds = new Set(state.visibleNodes.map((node) => node.id));
  const focusedEdges = [];
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    if (edgeIsFocused(edge)) {
      focusedEdges.push(edge);
      return;
    }
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    drawEdge(edge, source, target, false);
  });

  focusedEdges.forEach((edge) => {
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    drawEdge(edge, source, target, true);
  });

  const labelCandidates = [];
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    const selected = node.id === state.selectedId;
    const hovered = node.id === state.hoveredId;
    const focused = nodeIsFocused(node);
    const radius = nodeRadius(node);

    ctx.beginPath();
    ctx.arc(
      position.x,
      position.y,
      radius + (selected ? 6 : focused ? 5 : hovered ? 3 : 0),
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = selected
      ? "rgba(92, 200, 167, 0.26)"
      : focused
        ? "rgba(237, 241, 242, 0.16)"
        : hovered
          ? "rgba(255,255,255,0.12)"
          : "rgba(0,0,0,0.22)";
    ctx.fill();

    ctx.beginPath();
    ctx.arc(position.x, position.y, radius, 0, Math.PI * 2);
    ctx.fillStyle = colorFor(node.kind);
    ctx.fill();
    ctx.lineWidth = selected ? 2.6 / state.zoom : focused ? 2.2 / state.zoom : 1 / state.zoom;
    ctx.strokeStyle = selected ? "#ffffff" : focused ? "rgba(237, 241, 242, 0.92)" : "rgba(255,255,255,0.55)";
    ctx.stroke();

    if (shouldShowNodeLabel(node, selected, hovered, focused)) {
      labelCandidates.push({
        node,
        position,
        radius,
        selected,
        hovered,
        focused,
        forced: selected || hovered,
        priority: nodeLabelPriority(node),
      });
    }
  });

  drawNodeLabels(labelCandidates);
  ctx.restore();
}

function drawEdge(edge, source, target, focused) {
  if (!source || !target) return;
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const distance = Math.max(1, Math.sqrt(dx * dx + dy * dy));
  const ux = dx / distance;
  const uy = dy / distance;
  const sourceRadius = nodeRadiusById(edge.source) + 2 / state.zoom;
  const targetRadius = nodeRadiusById(edge.target) + (focused ? 8 : 3) / state.zoom;
  const start = {
    x: source.x + ux * Math.min(sourceRadius, distance * 0.35),
    y: source.y + uy * Math.min(sourceRadius, distance * 0.35),
  };
  const end = {
    x: target.x - ux * Math.min(targetRadius, distance * 0.35),
    y: target.y - uy * Math.min(targetRadius, distance * 0.35),
  };

  if (focused) {
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    ctx.lineTo(end.x, end.y);
    ctx.lineWidth = 6 / state.zoom;
    ctx.strokeStyle = "rgba(13, 15, 16, 0.72)";
    ctx.stroke();
  }

  ctx.beginPath();
  ctx.moveTo(start.x, start.y);
  ctx.lineTo(end.x, end.y);
  ctx.lineWidth = (focused ? 3.2 : 1) / state.zoom;
  ctx.strokeStyle = focused ? focusEdgeColor() : edgeColor(edge);
  ctx.stroke();

  if (focused) {
    drawArrowHead(start, end, focusEdgeColor());
  }
}

function drawArrowHead(start, end, color) {
  const angle = Math.atan2(end.y - start.y, end.x - start.x);
  const length = 11 / state.zoom;
  const spread = Math.PI / 7;
  ctx.beginPath();
  ctx.moveTo(end.x, end.y);
  ctx.lineTo(end.x - Math.cos(angle - spread) * length, end.y - Math.sin(angle - spread) * length);
  ctx.lineTo(end.x - Math.cos(angle + spread) * length, end.y - Math.sin(angle + spread) * length);
  ctx.closePath();
  ctx.fillStyle = color;
  ctx.fill();
}

function shouldShowNodeLabel(node, selected, hovered, focused) {
  if (selected || hovered) return true;
  if (state.labelMode === "minimal") return false;
  if (state.labelMode === "focus") return focused && state.zoom >= 1.35;
  if (focused) return state.zoom >= 1.45;

  const priority = nodeLabelPriority(node);
  const visibleCount = state.visibleNodes.length;
  if (state.search) {
    if (visibleCount <= 30) return state.zoom >= 1.55 && priority <= 5;
    if (visibleCount <= 120) return state.zoom >= 2.15 && priority <= 3;
    return state.zoom >= 2.7 && priority <= 2;
  }
  if (state.zoom < 1.95) return false;
  if (visibleCount > 220) return state.zoom >= 3.1 && priority <= 1;
  if (visibleCount > 120) return state.zoom >= 2.85 && priority <= 1;
  if (visibleCount > 60) return state.zoom >= 2.55 && priority <= 2;
  if (visibleCount > 25) return state.zoom >= 2.25 && priority <= 3;
  if (priority >= 8) return state.zoom >= 2.65;
  return state.zoom >= 1.95 && priority <= 4;
}

function drawNodeLabels(candidates) {
  const occupied = [];
  const nodeBoxes = nodeOcclusionBoxes();
  const budget = nodeLabelBudget();
  let drawnAutoLabels = 0;
  const ordered = candidates.sort((left, right) => {
    const leftPriority = left.selected ? 0 : left.hovered ? 1 : left.focused ? 2 : 3;
    const rightPriority = right.selected ? 0 : right.hovered ? 1 : right.focused ? 2 : 3;
    return (
      leftPriority - rightPriority ||
      left.priority - right.priority ||
      left.node.label.localeCompare(right.node.label)
    );
  });

  ordered.forEach((candidate) => {
    const forced = candidate.forced;
    if (!forced && drawnAutoLabels >= budget) return;
    const geometry = labelGeometry(candidate, occupied, nodeBoxes);
    if (!geometry) return;
    drawLabelGeometry(geometry);
    occupied.push(geometry);
    if (!forced) drawnAutoLabels += 1;
  });
}

function labelGeometry(candidate, occupied, nodeBoxes) {
  const { node, position, radius, forced } = candidate;
  const zoom = Math.max(0.18, state.zoom);
  const maxLength = forced ? 40 : state.zoom >= 2.4 ? 22 : 14;
  const label = truncateGraphLabel(node.label, maxLength);
  const padX = (forced ? 7 : 5) / zoom;
  const height = (forced ? 23 : 18) / zoom;
  const fontSize = (forced ? 12 : 11) / zoom;
  ctx.font = `${fontSize}px Inter, sans-serif`;
  const metrics = ctx.measureText(label);
  const width = metrics.width + padX * 2;
  const gap = (forced ? 10 : 8) / zoom;
  const placements = forced
    ? ["right", "left", "top", "bottom"]
    : ["top", "right", "left", "bottom"];
  const geometries = placements.map((placement) =>
    labelGeometryForPlacement({
      node,
      position,
      radius,
      label,
      width,
      height,
      padX,
      gap,
      font: ctx.font,
      forced,
      placement,
    }),
  );
  const usable = geometries.find(
    (geometry) =>
      boxIntersectsViewport(geometry) && !labelIntersectsScene(geometry, occupied, nodeBoxes),
  );
  if (usable) return usable;
  if (!forced) return null;
  return geometries.find((geometry) => boxIntersectsViewport(geometry)) || geometries[0];
}

function labelGeometryForPlacement(options) {
  const {
    node,
    position,
    radius,
    label,
    width,
    height,
    padX,
    gap,
    font,
    forced,
    placement,
  } = options;
  let x = position.x - width / 2;
  let y = position.y + radius + gap;

  if (placement === "top") {
    y = position.y - radius - gap - height;
  } else if (placement === "right") {
    x = position.x + radius + gap;
    y = position.y - height / 2;
  } else if (placement === "left") {
    x = position.x - radius - gap - width;
    y = position.y - height / 2;
  }

  return {
    nodeId: node.id,
    label,
    x,
    y,
    width,
    height,
    padX,
    textY: y + height / 2,
    radius: 5 / Math.max(0.18, state.zoom),
    font,
    forced,
  };
}

function drawLabelGeometry(geometry) {
  ctx.font = geometry.font;
  ctx.textBaseline = "middle";
  if (!geometry.forced) {
    ctx.lineWidth = 3 / Math.max(0.18, state.zoom);
    ctx.strokeStyle = "rgba(13, 15, 16, 0.78)";
    ctx.strokeText(geometry.label, geometry.x + geometry.padX, geometry.textY);
    ctx.fillStyle = "rgba(237, 241, 242, 0.84)";
    ctx.fillText(geometry.label, geometry.x + geometry.padX, geometry.textY);
    return;
  }
  ctx.fillStyle = geometry.forced
    ? "rgba(13, 15, 16, 0.84)"
    : "rgba(13, 15, 16, 0.58)";
  roundRect(ctx, geometry.x, geometry.y, geometry.width, geometry.height, geometry.radius);
  ctx.fill();
  if (geometry.forced) {
    ctx.lineWidth = 1 / Math.max(0.18, state.zoom);
    ctx.strokeStyle = "rgba(237, 241, 242, 0.22)";
    ctx.stroke();
  }
  ctx.fillStyle = "#edf1f2";
  ctx.fillText(geometry.label, geometry.x + geometry.padX, geometry.textY);
}

function nodeLabelPriority(node) {
  if (node.metadata?.item_kind === "diagnostic") return 1;
  switch (node.kind) {
    case "entrypoint":
      return 1;
    case "repository":
      return 2;
    case "config":
    case "environment":
      return 3;
    case "directory":
    case "file":
    case "module":
    case "type":
      return 5;
    case "function":
      return 7;
    case "external_dependency":
      return 9;
    default:
      return 8;
  }
}

function nodeLabelBudget() {
  const visibleCount = state.visibleNodes.length;
  if (state.labelMode === "minimal") return 0;
  if (state.labelMode === "focus") {
    if (state.zoom < 1.35) return 0;
    return visibleCount <= 40 ? 3 : 1;
  }
  if (state.zoom < 1.95 && !state.search) return 0;
  let budget = visibleCount <= 25
    ? 4
    : visibleCount <= 80
      ? 2
      : visibleCount <= 160
        ? 1
        : 1;
  if (state.zoom >= 2.9) budget += 2;
  else if (state.zoom >= 2.35) budget += 1;
  else if (state.zoom < 2.1 && visibleCount > 60) budget = Math.min(budget, 1);
  if (state.search && visibleCount <= 80) budget += 1;
  return Math.max(0, Math.min(6, budget));
}

function nodeOcclusionBoxes() {
  const pad = 14 / Math.max(0.18, state.zoom);
  return state.visibleNodes
    .map((node) => {
      const position = state.positions.get(node.id);
      if (!position) return null;
      const radius = nodeRadius(node) + pad;
      return {
        nodeId: node.id,
        x: position.x - radius,
        y: position.y - radius,
        width: radius * 2,
        height: radius * 2,
      };
    })
    .filter(Boolean);
}

function truncateGraphLabel(value, maxLength) {
  if (value.length <= maxLength) return value;
  return `${value.slice(0, Math.max(0, maxLength - 3))}...`;
}

function boxIntersectsViewport(box) {
  const left = box.x * state.zoom + state.pan.x;
  const right = (box.x + box.width) * state.zoom + state.pan.x;
  const top = box.y * state.zoom + state.pan.y;
  const bottom = (box.y + box.height) * state.zoom + state.pan.y;
  const margin = 24;
  return !(right < -margin || left > canvas.width + margin || bottom < -margin || top > canvas.height + margin);
}

function labelIntersectsScene(label, occupied, nodeBoxes) {
  return (
    occupied.some((box) => boxesIntersect(box, label)) ||
    nodeBoxes.some((box) => box.nodeId !== label.nodeId && boxesIntersect(box, label))
  );
}

function boxesIntersect(left, right) {
  const pad = 5 / Math.max(0.18, state.zoom);
  return !(
    left.x + left.width + pad < right.x ||
    right.x + right.width + pad < left.x ||
    left.y + left.height + pad < right.y ||
    right.y + right.height + pad < left.y
  );
}

function renderSelection() {
  state.selectionRequest += 1;
  const requestId = state.selectionRequest;
  const node = state.graph.nodes.find((candidate) => candidate.id === state.selectedId);
  if (!node) {
    if (state.selectedId) {
      selectionTitle.textContent = `${t("selection.node")} ${state.selectedId}`;
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("selection.loading"))}</p>`;
      loadNodeContext(state.selectedId, requestId);
    } else {
      selectionTitle.textContent = t("selection.title");
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("selection.noNode"))}</p>`;
    }
    return;
  }

  renderSelectionPanel(node, [], new Map([[node.id, node]]), requestId, true);
  loadNodeContext(node.id, requestId);
}

async function loadNodeContext(nodeId, requestId) {
  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    node_id: String(nodeId),
    edge_limit: "80",
    source_context: "5",
    insight_limit: "8",
  });

  try {
    const response = await fetch(`/api/node-card?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest || state.selectedId !== nodeId) return;
    if (!response.ok) {
      throw new Error(body.error || "node card failed");
    }
    const context = body.context || {};
    const nodeMap = new Map((context.nodes || []).map((node) => [node.id, node]));
    nodeMap.set(context.node.id, context.node);
    renderSelectionPanel(context.node, context.edges || [], nodeMap, requestId, false, context, body);
  } catch (error) {
    if (requestId !== state.selectionRequest || state.selectedId !== nodeId) return;
    const node = state.graph.nodes.find((candidate) => candidate.id === nodeId);
    if (node) {
      renderSelectionPanel(node, [], new Map([[node.id, node]]), requestId, false);
      const container = selectionBody.querySelector(".neighbors");
      if (container) {
        container.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
      }
    } else {
      selectionTitle.textContent = t("status.error");
      selectionBody.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    }
  }
}

function renderSelectionPanel(node, edges, nodeMap, requestId, loading = false, context = null, card = null) {
  selectionTitle.textContent = node.label;
  const summaryRows = renderNodeSummaryRows(node);
  const metadataRows = renderNodeMetadataRows(node);
  const nodeIssues = (card?.insights || nodeInsightsForNode(node.id)).slice(0, 8);
  const sourceLines = card?.source?.lines || null;
  const neighborRows = loading
    ? `<p class="empty">${escapeHtml(t("selection.loading"))}</p>`
    : edges.length > 0
      ? edges.map((edge) => renderNeighbor(edge, node.id, nodeMap)).join("")
      : `<p class="empty">${escapeHtml(t("selection.noDependencies"))}</p>`;
  const contextSummary = context
    ? `<span class="neighbor-summary">${
        escapeHtml(
          context.truncated_edges
            ? t("selection.contextEdgesLimited", {
                count: context.total_edges,
                limit: context.edge_limit,
              })
            : t("selection.contextEdges", { count: context.total_edges }),
        )
      }</span>`
    : "";

  selectionBody.innerHTML = `
    <div class="node-card">
      <header class="node-card-header">
        <div class="node-card-title">
          <span>${escapeHtml(formatKind(node.kind))}</span>
          <strong>${escapeHtml(node.label)}</strong>
        </div>
        <span class="node-card-id">#${node.id}</span>
      </header>
      <div class="selection-actions">
        <button type="button" data-path-endpoint="from">${escapeHtml(t("selection.from"))}</button>
        <button type="button" data-path-endpoint="to">${escapeHtml(t("selection.to"))}</button>
        ${
          node.kind === "config" || node.kind === "environment"
            ? `<button type="button" data-config-trace-target>${escapeHtml(t("selection.configTrace"))}</button>`
            : ""
        }
        ${
          node.metadata?.item_kind === "error"
            ? `<button type="button" data-error-trace-target>${escapeHtml(t("selection.errorTrace"))}</button>`
            : ""
        }
      </div>
      <section class="node-card-section">
        <h3>${escapeHtml(t("selection.summary"))}</h3>
        <dl class="node-summary">
          ${summaryRows.map(renderDefinitionRow).join("")}
        </dl>
      </section>
      ${
        metadataRows.length > 0
          ? `<section class="node-card-section">
              <h3>${escapeHtml(t("selection.metadata"))}</h3>
              <dl class="node-summary">
                ${metadataRows.map(renderDefinitionRow).join("")}
              </dl>
            </section>`
          : ""
      }
      <section class="node-card-section">
        <div class="node-card-section-header">
          <h3>${escapeHtml(t("selection.dependencies"))}</h3>
          ${contextSummary}
        </div>
        <div class="neighbors">${neighborRows}</div>
      </section>
      <section class="node-card-section">
        <div class="node-card-section-header">
          <h3>${escapeHtml(t("selection.risks"))}</h3>
          <span>${nodeIssues.length}</span>
        </div>
        <div class="node-issues">
          ${
            nodeIssues.length > 0
              ? nodeIssues.map(renderNodeIssue).join("")
              : `<p class="empty">${escapeHtml(t("selection.noIssues"))}</p>`
          }
        </div>
      </section>
      <section class="trace-panel">
        <div class="trace-controls">
          <label class="field compact">
            <span>${escapeHtml(t("selection.traceDepth"))}</span>
            <input id="traceDepthInput" type="number" min="1" max="8" value="3" />
          </label>
          <button id="traceButton" type="button">${escapeHtml(t("selection.trace"))}</button>
          <button id="dependentsButton" type="button">${escapeHtml(t("selection.dependents"))}</button>
        </div>
        <div id="traceResult" class="trace-result"></div>
      </section>
      ${
        node.span
          ? `<section class="source-preview">
            <header>
              <span>${escapeHtml(t("selection.source"))}</span>
              <strong>${escapeHtml(node.span.path)}:${node.span.start_line}</strong>
              ${card?.source?.truncated ? `<span>${escapeHtml(t("selection.sourceTruncated"))}</span>` : ""}
            </header>
            <pre id="sourcePreview"><code>${
              sourceLines ? sourceLines.map(renderSourceLine).join("") : escapeHtml(t("empty.loadingSource"))
            }</code></pre>
          </section>`
          : `<section class="source-preview">
              <header>
                <span>${escapeHtml(t("selection.source"))}</span>
              </header>
              <p class="empty">${escapeHtml(t("selection.noSource"))}</p>
            </section>`
      }
    </div>
  `;

  selectionBody.querySelectorAll(".neighbor").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedId = Number(button.dataset.nodeId);
      renderSelection();
    });
  });

  selectionBody.querySelectorAll("[data-path-endpoint]").forEach((button) => {
    button.addEventListener("click", () => {
      const target = button.dataset.pathEndpoint === "to" ? pathToInput : pathFromInput;
      target.value = String(node.id);
      target.focus();
    });
  });

  const configTraceTarget = selectionBody.querySelector("[data-config-trace-target]");
  if (configTraceTarget) {
    configTraceTarget.addEventListener("click", () => {
      configTraceTargetInput.value = node.label;
      runConfigTrace();
    });
  }

  const errorTraceTarget = selectionBody.querySelector("[data-error-trace-target]");
  if (errorTraceTarget) {
    errorTraceTarget.addEventListener("click", () => {
      errorTraceTargetInput.value = node.label;
      runErrorTrace();
    });
  }

  const traceButton = document.querySelector("#traceButton");
  if (traceButton) {
    traceButton.addEventListener("click", () => loadTrace(node));
  }
  const dependentsButton = document.querySelector("#dependentsButton");
  if (dependentsButton) {
    dependentsButton.addEventListener("click", () => loadDependents(node));
  }

  selectionBody.querySelectorAll("[data-node-issue-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const issue = nodeIssues[Number(button.dataset.nodeIssueIndex)];
      if (issue) focusInsight(issue);
    });
  });

  if (node.span && !loading && !sourceLines) {
    loadSourcePreview(node, requestId);
  }

  attachEdgeExplainActions(selectionBody);
}

function renderNodeSummaryRows(node) {
  const rows = [
    [t("selection.kind"), formatKind(node.kind)],
    [t("selection.id"), String(node.id)],
  ];
  if (node.metadata?.language) rows.push([t("label.language"), node.metadata.language]);
  if (node.metadata?.item_kind) rows.push([t("label.item"), formatKind(node.metadata.item_kind)]);
  if (node.span) {
    rows.push([t("selection.path"), node.span.path]);
    rows.push([t("selection.lines"), `${node.span.start_line}-${node.span.end_line}`]);
  }
  return rows;
}

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

async function loadTrace(node) {
  state.traceRequest += 1;
  state.dependentsRequest += 1;
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
    const response = await fetch(`/api/trace?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.traceRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "trace failed");
    }
    target.innerHTML = renderTrace(body);
    attachTraceNavigation(target);
    attachEdgeExplainActions(target);
  } catch (error) {
    if (requestId !== state.traceRequest) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  }
}

async function loadDependents(node) {
  state.traceRequest += 1;
  state.dependentsRequest += 1;
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
    const response = await fetch(`/api/dependents?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.dependentsRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "dependents trace failed");
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

function renderTrace(trace, options = {}) {
  if (!trace) {
    return '<p class="empty">No matching start node.</p>';
  }
  if (trace.nodes.length <= 1 && trace.edges.length === 0) {
    return `<p class="empty">${escapeHtml(options.empty || "No outgoing dependency edges.")}</p>`;
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

  const suffix = trace.truncated ? '<p class="empty">Trace truncated by depth.</p>' : "";
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
        ${renderExplainEdgeButton(edge)}
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
      state.selectedId = nodeId;
      renderSelection();
    });
  });
}

async function loadSourcePreview(node, requestId) {
  const preview = document.querySelector("#sourcePreview code");
  if (!preview || !node.span) return;

  const params = new URLSearchParams({
    root: pathInput.value.trim() || ".",
    path: node.span.path,
    start_line: String(node.span.start_line),
    end_line: String(node.span.end_line),
    context: "5",
  });

  try {
    const response = await fetch(`/api/source?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest) return;
    if (!response.ok) {
      throw new Error(body.error || "failed to load source");
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
        ${renderExplainEdgeButton(edge)}
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

function renderExplainEdgeButton(edge) {
  return `
    <button
      class="edge-explain-button"
      type="button"
      data-explain-edge
      data-edge-source="n${edge.source}"
      data-edge-target="n${edge.target}"
      data-edge-kind="${escapeHtml(edge.kind)}"
    >${escapeHtml(t("button.explain"))}</button>
  `;
}

async function explainEdge(button) {
  const container = button.closest("li") || button.closest(".edge-row")?.parentElement;
  const target = container?.querySelector("[data-edge-explanation]");
  if (!target) return;

  state.edgeExplainRequest += 1;
  const requestId = String(state.edgeExplainRequest);
  button.dataset.explainToken = requestId;
  target.hidden = false;
  target.innerHTML = '<p class="empty">Explaining edge...</p>';
  button.disabled = true;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    source: button.dataset.edgeSource || "",
    target: button.dataset.edgeTarget || "",
    kind: button.dataset.edgeKind || "",
  });

  try {
    const response = await fetch(`/api/explain-edge?${params.toString()}`);
    const body = await response.json();
    if (button.dataset.explainToken !== requestId) return;
    if (!response.ok) {
      throw new Error(body.error || "edge explanation failed");
    }
    target.innerHTML = renderEdgeExplanation(body);
  } catch (error) {
    if (button.dataset.explainToken !== requestId) return;
    target.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (button.dataset.explainToken === requestId) {
      button.disabled = false;
      delete button.dataset.explainToken;
    }
  }
}

function renderEdgeExplanation(explanation) {
  if (!explanation) {
    return '<p class="empty">No matching edge explanation.</p>';
  }
  const evidence = (explanation.evidence || [])
    .map((item) => `<li>${escapeHtml(item)}</li>`)
    .join("");
  const matchNote =
    explanation.total_matches > 1
      ? `<span>${explanation.total_matches} matches, showing first</span>`
      : "";

  return `
    <div class="edge-explanation-summary">
      <strong>${escapeHtml(explanation.summary)}</strong>
      <span>edge ${explanation.edge_index}</span>
      ${matchNote}
    </div>
    ${evidence ? `<ul>${evidence}</ul>` : '<p class="empty">No evidence metadata.</p>'}
  `;
}

function edgeFacts(edge) {
  const facts = [];
  if (edge.confidence) facts.push(formatKind(edge.confidence));
  const metadata = edge.metadata || {};
  for (const key of [
    "source",
    "relation",
    "resolution",
    "dependency_kind",
    "dependency_version",
    "target_symbol",
  ]) {
    if (metadata[key]) facts.push(`${formatKind(key)}: ${metadata[key]}`);
  }
  return facts;
}

function edgeKey(edge) {
  return `${edge.source}->${edge.target}:${edge.kind}`;
}

function onPointerDown(event) {
  canvas.setPointerCapture(event.pointerId);
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  state.lastPointer = { x: event.offsetX, y: event.offsetY };
  if (hit) {
    state.selectedId = hit.id;
    state.draggingId = hit.id;
    renderSelection();
  } else {
    state.draggingId = null;
  }
}

function onPointerMove(event) {
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  state.hoveredId = hit ? hit.id : null;

  if (!state.lastPointer) return;

  if (state.draggingId) {
    const position = state.positions.get(state.draggingId);
    position.x = world.x;
    position.y = world.y;
    const velocity = state.velocities.get(state.draggingId);
    velocity.x = 0;
    velocity.y = 0;
  } else if (event.buttons === 1) {
    state.pan.x += event.offsetX - state.lastPointer.x;
    state.pan.y += event.offsetY - state.lastPointer.y;
  }

  state.lastPointer = { x: event.offsetX, y: event.offsetY };
}

function onPointerUp() {
  state.draggingId = null;
  state.lastPointer = null;
}

function onWheel(event) {
  event.preventDefault();
  const delta = event.deltaY > 0 ? 0.9 : 1.1;
  zoomAt(event.offsetX, event.offsetY, delta);
}

function screenToWorld(x, y) {
  return {
    x: (x - state.pan.x) / state.zoom,
    y: (y - state.pan.y) / state.zoom,
  };
}

function findNodeAt(point) {
  for (let i = state.visibleNodes.length - 1; i >= 0; i -= 1) {
    const node = state.visibleNodes[i];
    const position = state.positions.get(node.id);
    const radius = nodeRadius(node) + 5;
    const dx = point.x - position.x;
    const dy = point.y - position.y;
    if (dx * dx + dy * dy <= radius * radius) return node;
  }
  return null;
}

function resizeCanvas() {
  const previousWidth = canvas.width;
  const previousHeight = canvas.height;
  const rect = canvas.getBoundingClientRect();
  canvas.width = Math.max(1, Math.floor(rect.width));
  canvas.height = Math.max(1, Math.floor(rect.height));
  if (previousWidth > 1 && previousHeight > 1) {
    state.pan.x += (canvas.width - previousWidth) / 2;
    state.pan.y += (canvas.height - previousHeight) / 2;
  } else {
    state.pan = { x: canvas.width / 2, y: canvas.height / 2 };
  }
  draw();
}

function nodeRadius(node) {
  switch (node.kind) {
    case "repository":
      return 15;
    case "file":
      return 10;
    case "function":
      return 8;
    case "entrypoint":
      return 10;
    case "type":
      return 9;
    default:
      return 7;
  }
}

function nodeRadiusById(nodeId) {
  const node = state.graph.nodes.find((candidate) => candidate.id === nodeId);
  return node ? nodeRadius(node) : 7;
}

function colorFor(kind) {
  return colors[kind] || colors.unknown;
}

function nodeIsFocused(node) {
  return Boolean(state.queryFocus?.nodeIds?.has(node.id));
}

function edgeIsFocused(edge) {
  return Boolean(state.queryFocus?.edgeKeys?.has(edgeKey(edge)));
}

function focusEdgeColor() {
  return state.queryFocus?.mode === "path" ? "rgba(92, 200, 167, 0.98)" : "rgba(237, 241, 242, 0.9)";
}

function edgeColor(edge) {
  switch (edge.kind) {
    case "calls":
      return "rgba(242, 193, 78, 0.72)";
    case "entrypoint":
      return "rgba(92, 200, 167, 0.82)";
    case "references":
      return "rgba(103, 183, 220, 0.58)";
    case "imports":
      return "rgba(184, 142, 230, 0.5)";
    case "depends_on":
      return "rgba(87, 178, 142, 0.68)";
    case "reads_environment":
      return "rgba(216, 166, 87, 0.72)";
    case "reads_config":
      return "rgba(229, 180, 84, 0.78)";
    case "may_error":
      return "rgba(224, 108, 117, 0.78)";
    default:
      return "rgba(170, 184, 190, 0.28)";
  }
}

function formatKind(value) {
  const raw = String(value);
  return translate(`kind.${raw}`, raw.replaceAll("_", " "));
}

function formatBytes(value) {
  const bytes = Number(value || 0);
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  let unitIndex = 0;
  let size = bytes;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const digits = unitIndex === 0 || size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}

function setStatus(text, className = "") {
  statusEl.textContent = translate(`status.${text}`, text);
  statusEl.dataset.status = text;
  statusEl.className = `status ${className}`.trim();
}

function clampNumber(value, min, max) {
  if (!Number.isFinite(value)) return min;
  return Math.max(min, Math.min(max, Math.trunc(value)));
}

function roundRect(context, x, y, width, height, radius) {
  context.beginPath();
  context.moveTo(x + radius, y);
  context.lineTo(x + width - radius, y);
  context.quadraticCurveTo(x + width, y, x + width, y + radius);
  context.lineTo(x + width, y + height - radius);
  context.quadraticCurveTo(x + width, y + height, x + width - radius, y + height);
  context.lineTo(x + radius, y + height);
  context.quadraticCurveTo(x, y + height, x, y + height - radius);
  context.lineTo(x, y + radius);
  context.quadraticCurveTo(x, y, x + radius, y);
  context.closePath();
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}
