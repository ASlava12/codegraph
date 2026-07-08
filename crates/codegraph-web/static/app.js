const DEFAULT_LOCALE = "en";
const DEFAULT_LABEL_MODE = "minimal";
const LABEL_MODES = new Set(["minimal", "hover", "focus", "auto"]);
const LABEL_MODE_STORAGE_KEY = "codegraph.labelMode";
const LABEL_MODE_STORAGE_VERSION_KEY = "codegraph.labelModeVersion";
const LABEL_MODE_STORAGE_VERSION = "17";
const API_TOKEN_STORAGE_KEY = "codegraph.apiToken";
const QUERY_HISTORY_STORAGE_KEY = "codegraph.queryHistory";
const QUERY_HISTORY_LIMIT = 8;

const I18N = {
  en: {
    "root.empty": "No project loaded",
    "project.currentRoot": "Current root",
    "auth.prompt": "CodeGraph API token",
    "selection.title": "Selection",
    "selection.noNode": "No node selected.",
    "selection.edge": "Dependency",
    "selection.noEdge": "Selected edge is no longer visible.",
    "selection.loading": "Loading node context...",
    "selection.sourceMatch": "Source Match",
    "selection.sourceLoading": "Loading...",
    "selection.node": "Node",
    "selection.kind": "Kind",
    "selection.id": "Id",
    "selection.path": "Path",
    "selection.lines": "Lines",
    "selection.summary": "Summary",
    "selection.dependencies": "Dependencies",
    "selection.dependencySummary": "Dependency Summary",
    "selection.fileSummary": "File Summary",
    "selection.risks": "Risks",
    "selection.riskSummary": "Risk Summary",
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
    "selection.edgeKinds": "edge kinds",
    "selection.confidences": "confidence",
    "selection.neighborKinds": "neighbor kinds",
    "selection.neighborLanguages": "neighbor languages",
    "selection.contained": "contained",
    "selection.symbols": "symbols",
    "selection.imports": "imports",
    "selection.calls": "calls",
    "selection.configReads": "config",
    "selection.environmentReads": "env",
    "selection.errorFacts": "errors",
    "selection.unresolvedCalls": "unresolved",
    "selection.containedKinds": "contained kinds",
    "selection.traceFacts": "trace facts",
    "selection.traceTargets": "trace targets",
    "selection.riskKinds": "risk kinds",
    "selection.from": "From",
    "selection.to": "To",
    "selection.configTrace": "Config Trace",
    "selection.errorTrace": "Error Trace",
    "selection.packageGraph": "Packages",
    "selection.fileGraph": "File Graph",
    "selection.symbolGraph": "Symbol Graph",
    "selection.configGraph": "Config Graph",
    "selection.errorGraph": "Error Graph",
    "selection.trace": "Trace",
    "selection.flow": "Flow",
    "selection.dependents": "Dependents",
    "selection.traceDepth": "Depth",
    "selection.openSource": "Open Source",
    "selection.openTarget": "Open Target",
    "selection.edgeIndex": "Edge Index",
    "label.project": "Project",
    "label.path": "Path",
    "label.workLang": "Work Lang",
    "label.status": "Status",
    "label.capability": "Capability",
    "label.search": "Search",
    "label.depth": "Depth",
    "label.nodes": "Nodes",
    "label.edges": "Edges",
    "graph.zoom": "Zoom",
    "graph.layout": "Layout",
    "graph.slice": "Slice",
    "graph.filters": "Filters",
    "graph.filtersActive": "{count} active",
    "graph.filtersNone": "None",
    "graph.running": "Running",
    "graph.paused": "Paused",
    "graph.scopeLoaded": "Loaded {nodes} of {totalNodes} nodes and {edges} of {totalEdges} edges.",
    "graph.scopeComplete": "Loaded complete visible graph: {nodes} nodes and {edges} edges.",
    "graph.scopeNodesTruncated": "More nodes are available; use next page, filters, or a higher node limit.",
    "graph.scopeEdgesTruncated": "More edges are available; raise the edge limit or narrow the filters.",
    "graph.scopeFocused": "Focused slice: {nodes} nodes and {edges} edges.",
    "aria.graphStage": "Code graph",
    "aria.graphControls": "Graph viewport controls",
    "aria.graphCanvas": "Interactive code graph. Use arrow keys to pan, plus and minus to zoom, Home to fit, 0 to reset, and Space to pause or resume layout.",
    "aria.zoomOut": "Zoom out",
    "aria.zoomIn": "Zoom in",
    "aria.fitGraph": "Fit visible graph",
    "aria.restartLayout": "Restart graph layout",
    "aria.pauseLayout": "Pause graph layout",
    "aria.resumeLayout": "Resume graph layout",
    "aria.graphMinimap": "Graph minimap. Click or drag to recenter the viewport.",
    "aria.nodeLabels": "Node labels",
    "aria.interfaceLanguage": "Interface language",
    "aria.scanControls": "Scan controls",
    "aria.graphSummary": "Graph summary",
    "aria.runtimeStatus": "Runtime status",
    "aria.jobs": "Jobs",
    "aria.projectOverview": "Project overview",
    "aria.entrypointFlows": "Entrypoint flows",
    "aria.graphPage": "Graph page",
    "aria.previousGraphPage": "Previous graph page",
    "aria.nextGraphPage": "Next graph page",
    "aria.edgePage": "Edge page",
    "aria.previousEdgePage": "Previous edge page",
    "aria.nextEdgePage": "Next edge page",
    "aria.graphQuery": "Graph query",
    "aria.queryPresets": "Query presets",
    "aria.sourceSearch": "Source search",
    "aria.cacheDiagnostics": "Cache diagnostics",
    "aria.graphExport": "Graph export",
    "aria.graphPath": "Graph path",
    "aria.configurationTrace": "Configuration trace",
    "aria.errorTrace": "Error trace",
    "aria.graphInsights": "Graph insights",
    "aria.graphFilters": "Graph filters",
    "aria.selectedNode": "Selected node",
    "label.kind": "Kind",
    "label.item": "Item",
    "label.language": "Language",
    "label.edge": "Edge",
    "label.confidence": "Confidence",
    "label.riskSeverity": "Risk",
    "label.block": "Block",
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
    "button.labelsMinimal": "Off",
    "button.labelsHover": "Hover",
    "button.labelsAuto": "Auto",
    "button.labelsFocus": "Focus",
    "button.card": "Card",
    "button.copyLink": "Copy Link",
    "button.copyQueryLink": "Copy Query Link",
    "button.copyPageLink": "Copy Page Link",
    "button.clearFilters": "Clear Filters",
    "button.clearCanvasFilters": "Clear Canvas Filters",
    "button.downloadSlice": "Download Slice",
    "button.downloadQueryResult": "Download Result",
    "button.buildQueryWorkflows": "Build Flows",
    "button.downloadInsights": "Download Insights",
    "button.downloadCheck": "Download Check",
    "button.downloadSourceResults": "Download Results",
    "button.downloadEntryFlows": "Download Flows",
    "button.buildEntryWorkflows": "Build Workflows",
    "button.downloadEntryWorkflows": "Download Workflows",
    "button.downloadEntryWorkflowMermaid": "Download Mermaid",
    "button.downloadPathResult": "Download Path",
    "button.downloadConfigTrace": "Download Config Trace",
    "button.downloadErrorTrace": "Download Error Trace",
    "button.downloadCard": "Download Card",
    "button.downloadWorkflow": "Download Flow",
    "button.downloadWorkflowMermaid": "Download Mermaid",
    "button.graphFile": "Graph File",
    "button.copied": "Copied",
    "button.focusEdge": "Focus",
    "button.queryEdge": "Query",
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
    "queryPreset.unreachable": "Unreachable",
    "queryPreset.unreachableConfig": "Unreachable Config",
    "queryPreset.unreachableErrors": "Unreachable Errors",
    "queryPreset.diagnostics": "Diagnostics",
    "queryPreset.ambiguousCalls": "Ambiguous Calls",
    "queryPreset.ambiguousEntrypoints": "Ambiguous Entrypoints",
    "queryPreset.dependencyScopes": "Dependency Scopes",
    "queryPreset.dependencyVersions": "Dependency Versions",
    "queryPreset.runtimeImports": "Runtime Imports",
    "queryPreset.testOnlyRuntime": "Test-only Runtime",
    "queryPreset.sensitiveDefaults": "Sensitive Defaults",
    "queryPreset.annotations": "Annotations",
    "queryPreset.risks": "Risks",
    "queryPreset.symbols": "Symbols",
    "queryPreset.files": "Files",
    "queryPreset.entrypoints": "Entrypoints",
    "queryPreset.routes": "Routes",
    "queryPreset.configs": "Configs",
    "queryPreset.errors": "Errors",
    "queryPreset.cycles": "Cycles",
    "queryPreset.hotspots": "Hotspots",
    "queryPreset.deadFunctions": "Dead Functions",
    "queryPreset.mainCalls": "Main Calls",
    "queryPreset.heuristic": "Heuristic",
    "queryPreset.dependencies": "Dependencies",
    "queryPreset.packages": "Packages",
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
    "cache.loadingDiff": "Loading cache diagnostics...",
    "cache.loadingChunks": "Loading cache chunks...",
    "cache.planningIncremental": "Planning incremental scan...",
    "cache.scanningChanged": "Scanning changed files...",
    "cache.buildingMerge": "Building merge preview...",
    "cache.updating": "Updating graph cache...",
    "cache.noFingerprint": "no previous fingerprint",
    "cache.noChanges": "No file fingerprint changes detected.",
    "cache.noChunks": "No cached graph chunks available.",
    "cache.noIncrementalWork": "No incremental scan work needed.",
    "cache.previous": "previous",
    "cache.current": "current",
    "cache.files": "files",
    "cache.changed": "changed",
    "cache.rescan": "rescan",
    "cache.removed": "removed",
    "cache.reusable": "reusable",
    "cache.listed": "listed",
    "cache.truncated": "truncated",
    "cache.limitedScope": "limited scope",
    "cache.fileReuse": "file reuse",
    "cache.byteReuse": "byte reuse",
    "cache.changedCurrent": "changed current",
    "cache.graphNodes": "graph nodes",
    "cache.graphEdges": "graph edges",
    "cache.previewNodes": "preview nodes",
    "cache.previewEdges": "preview edges",
    "cache.reusedNodes": "reused nodes",
    "cache.reusedEdges": "reused edges",
    "cache.removedCachedNodes": "removed cached nodes",
    "cache.removedCachedEdges": "removed cached edges",
    "cache.chunkNodes": "chunk nodes",
    "cache.chunkEdges": "chunk edges",
    "cache.scannedNodes": "scanned nodes",
    "cache.scannedEdges": "scanned edges",
    "cache.replacedPaths": "replaced paths",
    "cache.incomingBlockers": "incoming blockers",
    "cache.surfaceAdded": "surface added",
    "cache.surfaceRemoved": "surface removed",
    "cache.removedPaths": "removed paths",
    "cache.blockers": "blockers",
    "cache.complete": "complete",
    "cache.preview": "preview",
    "cache.stored": "stored",
    "cache.notStored": "not stored",
    "cache.none": "none",
    "cache.updateAlreadyCurrent": "Cache already matches the current project fingerprint.",
    "cache.updateStored": "Stored a complete graph under the current project fingerprint.",
    "cache.updateIncomplete": "Incremental merge is incomplete; cache record was not updated.",
    "cache.addedGroup": "Added",
    "cache.modifiedGroup": "Modified",
    "cache.removedGroup": "Removed",
    "cache.chunksGroup": "Chunks",
    "cache.scanGroup": "Scan",
    "cache.reusableGroup": "Reusable",
    "cache.nodeIdsGroup": "Node IDs",
    "cache.edgeIndexesGroup": "Edge Indexes",
    "cache.completenessBlockersGroup": "Completeness blockers",
    "cache.path": "path",
    "cache.id": "id",
    "cache.nodes": "nodes",
    "cache.edges": "edges",
    "cache.changedGraph": "Changed: {action}",
    "cache.incrementalComplete": "Incremental: complete",
    "cache.incrementalPreview": "Incremental: merge preview",
    "cache.changedPageInfo": "changed {nodes} / {files}",
    "cache.previewPageInfo": "preview {nodes} / {reused} reused",
    "cache.blocker.removed_paths": "Partial merge includes removed files; run a full scan before storing the graph cache.",
    "cache.blocker.incoming_cross_file_edges":
      "Partial merge would drop {count} incoming cross-file edge(s); run a full scan before storing the graph cache.",
    "cache.blocker.graph_surface_added":
      "Partial merge adds graph-surface nodes; run a full scan before storing the graph cache.",
    "cache.blocker.graph_surface_removed":
      "Partial merge removes graph-surface nodes; run a full scan before storing the graph cache.",
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
    "empty.noHotspots": "No hotspots.",
    "empty.noCommunities": "No communities.",
    "empty.noAnnotations": "No annotations.",
    "empty.noEntrypoints": "No entrypoints.",
    "empty.noScanPolicy": "No scan policy.",
    "empty.noCoverage": "No coverage.",
    "empty.noLspStatus": "No LSP status.",
    "empty.noSemanticWork": "No semantic work items.",
    "empty.noArchitecture": "No architecture map.",
    "empty.noLanguageDependencies": "No language dependencies.",
    "empty.loadingSource": "Loading...",
    "focus.entrypoint": "Focus: entrypoint",
    "focus.hotspot": "Focus: hotspot",
    "focus.community": "Focus: {label}",
    "focus.architectureEdge": "Focus: {source} -> {target}",
    "focus.languageDependency": "Focus: {source} -> {target}",
    "focus.semantic": "Semantic: {label}",
    "overview.maxFile": "Max file",
    "overview.policy": "Policy",
    "overview.defaults": "defaults",
    "overview.ignoreNames": "Ignore names",
    "overview.ignoreGlobs": "Ignore globs",
    "overview.hidden": "Hidden",
    "overview.gitIgnored": "Git ignored",
    "overview.indexed": "Indexed",
    "overview.large": "Large",
    "overview.policySkipped": "Policy skipped",
    "overview.otherFiles": "Other files",
    "overview.indexedBytes": "Indexed bytes",
    "overview.yes": "yes",
    "overview.no": "no",
    "overview.shown": "shown",
    "overview.queued": "queued",
    "overview.allAreas": "All areas",
    "overview.reset": "reset",
    "overview.areaEdges": "Area edges",
    "overview.crossLanguage": "Cross-language",
    "overview.communities": "Communities",
    "cap.server": "Server",
    "cap.api": "API",
    "cap.graph": "Graph",
    "cap.cache": "Cache",
    "cap.languages": "Languages",
    "cap.exports": "Exports",
    "cap.projects": "Projects",
    "cap.scanJobs": "Scan Jobs",
    "cap.semanticJobs": "Semantic Jobs",
    "cap.semanticWork": "Semantic Work",
    "cap.semanticTimeout": "LSP Timeout",
    "cap.apiBody": "API Body",
    "cap.graphPage": "Graph Page",
    "cap.nodeCard": "Node Card",
    "cap.focus": "Focus",
    "cap.queryLimit": "Query Limit",
    "cap.querySize": "Query Size",
    "cap.report": "Report",
    "cap.sourceSearchSize": "Source Search",
    "cap.headers": "Headers",
    "cap.routes": "Routes",
    "cap.on": "on",
    "cap.off": "off",
    "runtime.uptime": "Uptime",
    "runtime.cache": "Cache",
    "runtime.scanSlots": "Scan Slots",
    "runtime.semanticSlots": "Semantic Slots",
    "runtime.scanJobs": "Scan Jobs",
    "runtime.semanticJobs": "Semantic Jobs",
    "runtime.lastApi": "Last API",
    "risk.score": "Risk Score",
    "risk.grade": "Grade",
    "risk.errors": "Errors",
    "risk.warnings": "Warnings",
    "risk.infos": "Info",
    "risk.gate": "Gate",
    "risk.clean": "Clean",
    "export.report": "Report JSON",
    "export.reportMarkdown": "Report Markdown",
    "export.slice": "Visible Slice JSON",
    "export.queryResult": "Query Result JSON",
    "export.insights": "Insights JSON",
    "export.check": "Check Result JSON",
    "export.sourceSearch": "Source Search JSON",
    "export.entryFlows": "Entrypoint Traces JSON",
    "export.entryWorkflows": "Entrypoint Workflows JSON",
    "export.entryWorkflowMermaid": "Entrypoint Workflows Mermaid",
    "export.pathResult": "Path Result JSON",
    "export.configTrace": "Config Trace JSON",
    "export.errorTrace": "Error Trace JSON",
    "export.selectionCard": "Selection Card JSON",
    "export.workflow": "Workflow JSON",
    "export.workflowMermaid": "Workflow Mermaid",
    "export.noEntryFlows": "Trace entrypoints before exporting flows.",
    "export.noEntryWorkflows": "Build entrypoint workflows before exporting.",
    "export.noPathResult": "Find a path before exporting its result.",
    "export.noConfigTrace": "Trace config before exporting results.",
    "export.noErrorTrace": "Trace errors before exporting results.",
    "export.noSelectionCard": "Select a node or edge card before exporting.",
    "export.noWorkflow": "Load Flow before exporting.",
    "export.noSourceSearch": "Run source search before exporting results.",
    "export.noCheck": "Run a quality check before exporting its result.",
    "export.noQueryResult": "Run a graph query before exporting its result.",
    "export.noSlice": "No visible graph slice to export.",
    "export.exporting": "Exporting...",
    "export.failedFallback": "export failed",
    "check.running": "Running check...",
    "check.failedFallback": "check failed",
    "check.passed": "Passed",
    "check.failed": "Failed",
    "check.failOn": "fail on {severity}",
    "check.failingCount": "{count} failing",
    "check.matchedCount": "{count} matched",
    "insights.count": "{count} insights",
    "sourceSearch.enterText": "Enter source text.",
    "sourceSearch.searching": "Searching source...",
    "sourceSearch.failedFallback": "source search failed",
    "sourceSearch.matchCount": "{count} matches",
    "sourceSearch.truncated": "truncated",
    "sourceSearch.noMatches": "No source matches.",
    "query.enterExpression": "Enter a query expression.",
    "query.running": "Running query...",
    "query.tooLong": "Graph query is too long: {count} characters, maximum {limit}.",
    "path.enterEndpoints": "Enter both path endpoints.",
    "path.finding": "Finding path...",
    "path.failedFallback": "path query failed",
    "path.resultLabel": "Path",
    "trace.depth": "depth {depth}",
    "trace.pathCount": "{count} paths",
    "trace.traceTruncated": "Trace truncated.",
    "trace.resultTruncated": "Result truncated by limit.",
    "trace.entrypointPath": "entrypoint path",
    "trace.noStart": "No matching start node.",
    "trace.noOutgoing": "No outgoing dependency edges.",
    "entryFlows.tracing": "Tracing entrypoints...",
    "entryFlows.buildingWorkflows": "Building entrypoint workflows...",
    "entryFlows.failedFallback": "entrypoint trace failed",
    "entryFlows.workflowFailedFallback": "entrypoint workflow failed",
    "entryFlows.entrypointCount": "{count} entrypoints",
    "entryFlows.traceCount": "{count} traces",
    "entryFlows.workflowCount": "{count} workflows",
    "query.workflowStarts": "{count} starts",
    "entryFlows.noMatches": "No matching entrypoint flows.",
    "entryFlows.noWorkflowMatches": "No matching entrypoint workflows.",
    "entryFlows.traceTruncated": "Trace truncated by depth.",
    "entryFlows.reportTruncated": "Report truncated by limit or depth.",
    "entryFlows.focusFlow": "Focus flow",
    "entryFlows.focusWorkflow": "Focus workflow",
    "entryFlows.focusTitle": "Entry: {label}",
    "configTrace.enterTarget": "Enter a config file or environment variable.",
    "configTrace.tracing": "Tracing config...",
    "configTrace.failedFallback": "config trace failed",
    "configTrace.targetCount": "{count} targets",
    "configTrace.readerCount": "{count} readers",
    "configTrace.noMatches": "No matching config or environment nodes.",
    "configTrace.noReaders": "No direct readers.",
    "configTrace.readerPath": "reader path",
    "configTrace.focusTitle": "Config: {label}",
    "errorTrace.enterTarget": "Enter an error or exception label.",
    "errorTrace.tracing": "Tracing errors...",
    "errorTrace.failedFallback": "error trace failed",
    "errorTrace.errorCount": "{count} errors",
    "errorTrace.sourceCount": "{count} sources",
    "errorTrace.noMatches": "No matching error nodes.",
    "errorTrace.noSources": "No direct sources.",
    "errorTrace.sourcePath": "source path",
    "errorTrace.focusTitle": "Error: {label}",
    "trace.tracing": "Tracing...",
    "workflow.loading": "Loading workflow...",
    "workflow.noBlocks": "No workflow blocks.",
    "workflow.blockCount": "{count} blocks",
    "workflow.transitionCount": "{count} transitions",
    "workflow.truncated": "Workflow truncated by depth or block limit.",
    "workflow.risks": "{count} risks",
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
    "semantic.coverage": "Semantic",
    "semantic.missing": "Missing semantic",
    "semantic.candidates": "Candidate nodes",
    "semantic.plan": "Semantic plan",
    "semantic.definitions": "Definitions",
    "semantic.symbols": "Symbols",
    "semantic.workspace": "Workspace",
    "semantic.references": "References",
    "semantic.workQueue": "Work queue",
    "semantic.needed": "needed",
    "semantic.ops": "ops",
    "semantic.noServer": "no server",
    "semantic.errors": "Errors",
    "semantic.unmatched": "Unmatched",
    "legend.kindFilter": "Toggle {kind} nodes",
    "legend.riskFilter": "Filter graph by {severity} risks",
    "queryHistory.recent": "Recent Queries",
    "queryHistory.clear": "Clear",
    "queryHistory.run": "Run recent query: {query}",
    "status.idle": "idle",
    "status.queue": "queue",
    "status.scan": "scan",
    "status.page": "page",
    "status.semantic": "semantic",
    "status.ready": "ready",
    "status.error": "error",
    "status.loading": "loading",
    "status.chunks": "chunks",
    "status.planning": "planning",
    "status.scanning": "scanning",
    "status.merging": "merging",
    "status.updating": "updating",
    "status.stored": "stored",
    "status.skipped": "skipped",
    "kind.error": "error",
    "kind.warning": "warning",
    "kind.info": "info",
    "kind.start": "start",
    "kind.call": "call",
    "kind.config_read": "config read",
    "kind.environment_read": "env read",
    "kind.dependency": "dependency",
    "kind.import": "import",
    "kind.reference": "reference",
    "kind.external_boundary": "external boundary",
    "kind.full_scan": "full scan",
    "kind.partial_rescan": "partial rescan",
    "kind.noop": "no changes",
    "kind.full_reuse": "full reuse",
    "kind.partial_reuse": "partial reuse",
    "kind.no_changes": "no changes",
    "kind.removed_paths": "removed paths",
    "kind.incoming_cross_file_edges": "incoming cross-file edges",
    "kind.graph_surface_added": "graph surface added",
    "kind.graph_surface_removed": "graph surface removed",
    "kind.ambiguous_call_resolution": "ambiguous call resolution",
    "kind.ambiguous_entrypoint_target": "ambiguous entrypoint target",
    "kind.conflicting_config_default": "conflicting config default",
    "kind.conflicting_dependency_declaration": "conflicting dependency declaration",
    "kind.mixed_config_requirement": "mixed config requirement",
    "kind.mixed_dependency_scope": "mixed dependency scope",
    "kind.non_runtime_dependency_import": "non-runtime dependency import",
    "kind.test_only_runtime_dependency": "test-only runtime dependency",
    "kind.semantic_diagnostic": "semantic diagnostic",
    "kind.sensitive_config_default": "sensitive config default",
    "kind.undeclared_external_import": "undeclared external import",
    "kind.unresolved_local_import": "unresolved local import",
    "kind.unused_declared_dependency": "unused declared dependency",
    "kind.unreachable_error_flow": "unreachable error flow",
    "kind.unreachable_source_file": "unreachable source file",
  },
  ru: {
    "root.empty": "Проект не загружен",
    "project.currentRoot": "Текущий каталог",
    "auth.prompt": "Токен CodeGraph API",
    "selection.title": "Выбор",
    "selection.noNode": "Узел не выбран.",
    "selection.edge": "Зависимость",
    "selection.noEdge": "Выбранная связь больше не видна.",
    "selection.loading": "Загружаю контекст узла...",
    "selection.sourceMatch": "Совпадение в коде",
    "selection.sourceLoading": "Загружаю...",
    "selection.node": "Узел",
    "selection.kind": "Тип",
    "selection.id": "Id",
    "selection.path": "Путь",
    "selection.lines": "Строки",
    "selection.summary": "Сводка",
    "selection.dependencies": "Связи",
    "selection.dependencySummary": "Сводка связей",
    "selection.fileSummary": "Сводка файла",
    "selection.risks": "Риски",
    "selection.riskSummary": "Сводка рисков",
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
    "selection.edgeKinds": "типы связей",
    "selection.confidences": "уверенность",
    "selection.neighborKinds": "типы соседей",
    "selection.neighborLanguages": "языки соседей",
    "selection.contained": "внутри",
    "selection.symbols": "символы",
    "selection.imports": "импорты",
    "selection.calls": "вызовы",
    "selection.configReads": "конфиг",
    "selection.environmentReads": "env",
    "selection.errorFacts": "ошибки",
    "selection.unresolvedCalls": "не разрешено",
    "selection.containedKinds": "типы внутри",
    "selection.traceFacts": "факты потока",
    "selection.traceTargets": "цели потока",
    "selection.riskKinds": "типы рисков",
    "selection.from": "Отсюда",
    "selection.to": "Сюда",
    "selection.configTrace": "Трасса конфига",
    "selection.errorTrace": "Трасса ошибок",
    "selection.packageGraph": "Пакеты",
    "selection.fileGraph": "Граф файла",
    "selection.symbolGraph": "Граф символа",
    "selection.configGraph": "Граф конфига",
    "selection.errorGraph": "Граф ошибок",
    "selection.trace": "Трассировать",
    "selection.flow": "Блок-схема",
    "selection.dependents": "Зависимые",
    "selection.traceDepth": "Глубина",
    "selection.openSource": "Открыть источник",
    "selection.openTarget": "Открыть цель",
    "selection.edgeIndex": "Индекс связи",
    "label.project": "Проект",
    "label.path": "Путь",
    "label.workLang": "Язык задач",
    "label.status": "Статус",
    "label.capability": "Возможность",
    "label.search": "Поиск",
    "label.depth": "Глубина",
    "label.nodes": "Узлы",
    "label.edges": "Связи",
    "graph.zoom": "Масштаб",
    "graph.layout": "Раскладка",
    "graph.slice": "Срез",
    "graph.filters": "Фильтры",
    "graph.filtersActive": "Активно: {count}",
    "graph.filtersNone": "Нет",
    "graph.running": "Идет",
    "graph.paused": "Пауза",
    "graph.scopeLoaded": "Загружено {nodes} из {totalNodes} узлов и {edges} из {totalEdges} связей.",
    "graph.scopeComplete": "Загружен полный видимый граф: {nodes} узлов и {edges} связей.",
    "graph.scopeNodesTruncated": "Доступны ещё узлы; используйте следующую страницу, фильтры или больший лимит узлов.",
    "graph.scopeEdgesTruncated": "Доступны ещё связи; увеличьте лимит связей или сузьте фильтры.",
    "graph.scopeFocused": "Фокусный срез: {nodes} узлов и {edges} связей.",
    "aria.graphStage": "Граф кода",
    "aria.graphControls": "Управление областью графа",
    "aria.graphCanvas": "Интерактивный граф кода. Используйте стрелки для сдвига, плюс и минус для масштаба, Home чтобы вписать граф, 0 для сброса и пробел для паузы или продолжения раскладки.",
    "aria.zoomOut": "Уменьшить масштаб",
    "aria.zoomIn": "Увеличить масштаб",
    "aria.fitGraph": "Вписать видимый граф",
    "aria.restartLayout": "Перезапустить раскладку графа",
    "aria.pauseLayout": "Поставить раскладку графа на паузу",
    "aria.resumeLayout": "Продолжить раскладку графа",
    "aria.graphMinimap": "Мини-карта графа. Нажмите или перетащите, чтобы сместить область просмотра.",
    "aria.nodeLabels": "Подписи узлов",
    "aria.interfaceLanguage": "Язык интерфейса",
    "aria.scanControls": "Управление сканированием",
    "aria.graphSummary": "Сводка графа",
    "aria.runtimeStatus": "Статус рантайма",
    "aria.jobs": "Задачи",
    "aria.projectOverview": "Обзор проекта",
    "aria.entrypointFlows": "Потоки точек входа",
    "aria.graphPage": "Страница графа",
    "aria.previousGraphPage": "Предыдущая страница графа",
    "aria.nextGraphPage": "Следующая страница графа",
    "aria.edgePage": "Страница связей",
    "aria.previousEdgePage": "Предыдущая страница связей",
    "aria.nextEdgePage": "Следующая страница связей",
    "aria.graphQuery": "Запрос к графу",
    "aria.queryPresets": "Шаблоны запросов",
    "aria.sourceSearch": "Поиск в коде",
    "aria.cacheDiagnostics": "Диагностика кеша",
    "aria.graphExport": "Экспорт графа",
    "aria.graphPath": "Путь по графу",
    "aria.configurationTrace": "Трасса конфигурации",
    "aria.errorTrace": "Трасса ошибок",
    "aria.graphInsights": "Находки графа",
    "aria.graphFilters": "Фильтры графа",
    "aria.selectedNode": "Выбранный узел",
    "label.kind": "Тип",
    "label.item": "Элемент",
    "label.language": "Язык",
    "label.edge": "Связь",
    "label.confidence": "Уверенность",
    "label.riskSeverity": "Риск",
    "label.block": "Блок",
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
    "button.labelsMinimal": "Выкл",
    "button.labelsHover": "Наведение",
    "button.labelsAuto": "Авто",
    "button.labelsFocus": "Фокус",
    "button.card": "Карточка",
    "button.copyLink": "Скопировать ссылку",
    "button.copyQueryLink": "Ссылка на запрос",
    "button.copyPageLink": "Ссылка на страницу",
    "button.clearFilters": "Сбросить фильтры",
    "button.clearCanvasFilters": "Сбросить фильтры графа",
    "button.downloadSlice": "Скачать срез",
    "button.downloadQueryResult": "Скачать результат",
    "button.buildQueryWorkflows": "Собрать Flow",
    "button.downloadInsights": "Скачать insights",
    "button.downloadCheck": "Скачать проверку",
    "button.downloadSourceResults": "Скачать результаты",
    "button.downloadEntryFlows": "Скачать потоки",
    "button.buildEntryWorkflows": "Собрать блок-схемы",
    "button.downloadEntryWorkflows": "Скачать блок-схемы",
    "button.downloadEntryWorkflowMermaid": "Скачать Mermaid",
    "button.downloadPathResult": "Скачать путь",
    "button.downloadConfigTrace": "Скачать трассу конфига",
    "button.downloadErrorTrace": "Скачать трассу ошибок",
    "button.downloadCard": "Скачать карточку",
    "button.downloadWorkflow": "Скачать Flow",
    "button.downloadWorkflowMermaid": "Скачать Mermaid",
    "button.graphFile": "Граф файла",
    "button.copied": "Скопировано",
    "button.focusEdge": "Фокус",
    "button.queryEdge": "Запрос",
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
    "queryPreset.unreachable": "Недостижимые",
    "queryPreset.unreachableConfig": "Недостижимые конфиги",
    "queryPreset.unreachableErrors": "Недостижимые ошибки",
    "queryPreset.diagnostics": "Диагностика",
    "queryPreset.ambiguousCalls": "Неоднозначные вызовы",
    "queryPreset.ambiguousEntrypoints": "Неоднозначные точки входа",
    "queryPreset.dependencyScopes": "Scope зависимостей",
    "queryPreset.dependencyVersions": "Версии зависимостей",
    "queryPreset.runtimeImports": "Runtime-импорты",
    "queryPreset.testOnlyRuntime": "Тестовый runtime",
    "queryPreset.sensitiveDefaults": "Секретные defaults",
    "queryPreset.annotations": "Аннотации",
    "queryPreset.risks": "Риски",
    "queryPreset.symbols": "Символы",
    "queryPreset.files": "Файлы",
    "queryPreset.entrypoints": "Точки входа",
    "queryPreset.routes": "Маршруты",
    "queryPreset.configs": "Конфиги",
    "queryPreset.errors": "Ошибки",
    "queryPreset.cycles": "Циклы",
    "queryPreset.hotspots": "Горячие узлы",
    "queryPreset.deadFunctions": "Мёртвые функции",
    "queryPreset.mainCalls": "Вызовы main",
    "queryPreset.heuristic": "Эвристика",
    "queryPreset.dependencies": "Зависимости",
    "queryPreset.packages": "Пакеты",
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
    "cache.loadingDiff": "Загружаю диагностику кеша...",
    "cache.loadingChunks": "Загружаю фрагменты кеша...",
    "cache.planningIncremental": "Планирую инкрементальный скан...",
    "cache.scanningChanged": "Сканирую изменённые файлы...",
    "cache.buildingMerge": "Собираю предпросмотр merge...",
    "cache.updating": "Обновляю кеш графа...",
    "cache.noFingerprint": "предыдущего fingerprint нет",
    "cache.noChanges": "Изменений fingerprint файлов нет.",
    "cache.noChunks": "Фрагменты графа в кеше не найдены.",
    "cache.noIncrementalWork": "Инкрементальный скан не нужен.",
    "cache.previous": "предыдущий",
    "cache.current": "текущий",
    "cache.files": "файлов",
    "cache.changed": "изменено",
    "cache.rescan": "перескан",
    "cache.removed": "удалено",
    "cache.reusable": "повторно",
    "cache.listed": "показано",
    "cache.truncated": "обрезано",
    "cache.limitedScope": "ограниченная область",
    "cache.fileReuse": "reuse файлов",
    "cache.byteReuse": "reuse байт",
    "cache.changedCurrent": "изменено сейчас",
    "cache.graphNodes": "узлов графа",
    "cache.graphEdges": "связей графа",
    "cache.previewNodes": "узлов preview",
    "cache.previewEdges": "связей preview",
    "cache.reusedNodes": "узлов повторно",
    "cache.reusedEdges": "связей повторно",
    "cache.removedCachedNodes": "удалено кеш-узлов",
    "cache.removedCachedEdges": "удалено кеш-связей",
    "cache.chunkNodes": "узлов фрагмента",
    "cache.chunkEdges": "связей фрагмента",
    "cache.scannedNodes": "узлов скана",
    "cache.scannedEdges": "связей скана",
    "cache.replacedPaths": "заменено путей",
    "cache.incomingBlockers": "входящих блокеров",
    "cache.surfaceAdded": "surface добавлено",
    "cache.surfaceRemoved": "surface удалено",
    "cache.removedPaths": "удалённых путей",
    "cache.blockers": "блокеров",
    "cache.complete": "полный",
    "cache.preview": "preview",
    "cache.stored": "сохранено",
    "cache.notStored": "не сохранено",
    "cache.none": "нет",
    "cache.updateAlreadyCurrent": "Кеш уже соответствует текущему fingerprint проекта.",
    "cache.updateStored": "Полный граф сохранён под текущим fingerprint проекта.",
    "cache.updateIncomplete": "Инкрементальный merge неполный; запись кеша не обновлена.",
    "cache.addedGroup": "Добавлено",
    "cache.modifiedGroup": "Изменено",
    "cache.removedGroup": "Удалено",
    "cache.chunksGroup": "Фрагменты",
    "cache.scanGroup": "Скан",
    "cache.reusableGroup": "Повторно",
    "cache.nodeIdsGroup": "ID узлов",
    "cache.edgeIndexesGroup": "Индексы связей",
    "cache.completenessBlockersGroup": "Блокеры полноты",
    "cache.path": "путь",
    "cache.id": "id",
    "cache.nodes": "узлов",
    "cache.edges": "связей",
    "cache.changedGraph": "Изменения: {action}",
    "cache.incrementalComplete": "Инкремент: полный",
    "cache.incrementalPreview": "Инкремент: preview merge",
    "cache.changedPageInfo": "изменено {nodes} / {files}",
    "cache.previewPageInfo": "preview {nodes} / {reused} повторно",
    "cache.blocker.removed_paths": "Partial merge включает удалённые файлы; перед сохранением кеша графа нужен полный скан.",
    "cache.blocker.incoming_cross_file_edges":
      "Partial merge потеряет входящие межфайловые связи: {count}; перед сохранением кеша графа нужен полный скан.",
    "cache.blocker.graph_surface_added":
      "Partial merge добавляет узлы поверхности графа; перед сохранением кеша графа нужен полный скан.",
    "cache.blocker.graph_surface_removed":
      "Partial merge удаляет узлы поверхности графа; перед сохранением кеша графа нужен полный скан.",
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
    "empty.noHotspots": "Горячих узлов нет.",
    "empty.noCommunities": "Подсистем нет.",
    "empty.noAnnotations": "Аннотаций нет.",
    "empty.noEntrypoints": "Точки входа не найдены.",
    "empty.noScanPolicy": "Политика скана не получена.",
    "empty.noCoverage": "Покрытие скана не получено.",
    "empty.noLspStatus": "Статус LSP не получен.",
    "empty.noSemanticWork": "Семантических задач нет.",
    "empty.noArchitecture": "Карта архитектуры не получена.",
    "empty.noLanguageDependencies": "Межъязыковых зависимостей нет.",
    "empty.loadingSource": "Загружаю...",
    "focus.entrypoint": "Фокус: точка входа",
    "focus.hotspot": "Фокус: горячий узел",
    "focus.community": "Фокус: {label}",
    "focus.architectureEdge": "Фокус: {source} -> {target}",
    "focus.languageDependency": "Фокус: {source} -> {target}",
    "focus.semantic": "Семантика: {label}",
    "overview.maxFile": "Макс. файл",
    "overview.policy": "Политика",
    "overview.defaults": "по умолчанию",
    "overview.ignoreNames": "Имена ignore",
    "overview.ignoreGlobs": "Glob ignore",
    "overview.hidden": "Скрытые",
    "overview.gitIgnored": "Git ignored",
    "overview.indexed": "Индексировано",
    "overview.large": "Большие",
    "overview.policySkipped": "Пропущено политикой",
    "overview.otherFiles": "Прочие файлы",
    "overview.indexedBytes": "Индексировано байт",
    "overview.yes": "да",
    "overview.no": "нет",
    "overview.shown": "показано",
    "overview.queued": "в очереди",
    "overview.allAreas": "Все области",
    "overview.reset": "сброс",
    "overview.areaEdges": "Связи областей",
    "overview.crossLanguage": "Межъязыковые",
    "overview.communities": "Подсистемы",
    "cap.server": "Сервер",
    "cap.api": "API",
    "cap.graph": "Граф",
    "cap.cache": "Кеш",
    "cap.languages": "Языки",
    "cap.exports": "Экспорты",
    "cap.projects": "Проекты",
    "cap.scanJobs": "Скан-задачи",
    "cap.semanticJobs": "Сем. задачи",
    "cap.semanticWork": "Сем. работа",
    "cap.semanticTimeout": "LSP таймаут",
    "cap.apiBody": "Тело API",
    "cap.graphPage": "Страница графа",
    "cap.nodeCard": "Карточка узла",
    "cap.focus": "Фокус",
    "cap.queryLimit": "Лимит запроса",
    "cap.querySize": "Размер запроса",
    "cap.report": "Отчет",
    "cap.sourceSearchSize": "Размер поиска",
    "cap.headers": "Заголовки",
    "cap.routes": "Маршруты",
    "cap.on": "вкл",
    "cap.off": "выкл",
    "runtime.uptime": "Аптайм",
    "runtime.cache": "Кеш",
    "runtime.scanSlots": "Слоты скана",
    "runtime.semanticSlots": "Слоты сем.",
    "runtime.scanJobs": "Скан-задачи",
    "runtime.semanticJobs": "Сем. задачи",
    "runtime.lastApi": "Последний API",
    "risk.score": "Риск",
    "risk.grade": "Оценка",
    "risk.errors": "Ошибки",
    "risk.warnings": "Предупреждения",
    "risk.infos": "Инфо",
    "risk.gate": "Гейт",
    "risk.clean": "Чисто",
    "export.report": "JSON-отчёт",
    "export.reportMarkdown": "Markdown-отчёт",
    "export.slice": "JSON видимого среза",
    "export.queryResult": "JSON результата запроса",
    "export.insights": "JSON insights",
    "export.check": "JSON проверки",
    "export.sourceSearch": "JSON поиска в коде",
    "export.entryFlows": "JSON потоков входа",
    "export.entryWorkflows": "JSON блок-схем входа",
    "export.entryWorkflowMermaid": "Mermaid блок-схем входа",
    "export.pathResult": "JSON результата пути",
    "export.configTrace": "JSON трассы конфига",
    "export.errorTrace": "JSON трассы ошибок",
    "export.selectionCard": "JSON карточки выбора",
    "export.workflow": "JSON блок-схемы",
    "export.workflowMermaid": "Mermaid блок-схемы",
    "export.noEntryFlows": "Сначала трассируйте точки входа.",
    "export.noEntryWorkflows": "Сначала соберите блок-схемы точек входа.",
    "export.noPathResult": "Сначала найдите путь.",
    "export.noConfigTrace": "Сначала трассируйте конфиг.",
    "export.noErrorTrace": "Сначала трассируйте ошибки.",
    "export.noSelectionCard": "Сначала выберите карточку узла или связи.",
    "export.noWorkflow": "Сначала загрузите блок-схему.",
    "export.noSourceSearch": "Сначала выполните поиск в коде.",
    "export.noCheck": "Сначала запустите проверку качества.",
    "export.noQueryResult": "Сначала выполните запрос к графу.",
    "export.noSlice": "Нет видимого среза графа для экспорта.",
    "export.exporting": "Экспортирую...",
    "export.failedFallback": "экспорт не удался",
    "check.running": "Проверяю...",
    "check.failedFallback": "проверка не удалась",
    "check.passed": "Пройдено",
    "check.failed": "Провалено",
    "check.failOn": "порог: {severity}",
    "check.failingCount": "нарушений: {count}",
    "check.matchedCount": "совпадений: {count}",
    "insights.count": "находок: {count}",
    "sourceSearch.enterText": "Введите текст для поиска.",
    "sourceSearch.searching": "Ищу в коде...",
    "sourceSearch.failedFallback": "поиск в коде не удался",
    "sourceSearch.matchCount": "совпадений: {count}",
    "sourceSearch.truncated": "результат усечён",
    "sourceSearch.noMatches": "Совпадений в коде нет.",
    "query.enterExpression": "Введите выражение запроса.",
    "query.running": "Выполняю запрос...",
    "query.tooLong": "Запрос к графу слишком длинный: {count} символов, максимум {limit}.",
    "path.enterEndpoints": "Введите обе конечные точки пути.",
    "path.finding": "Ищу путь...",
    "path.failedFallback": "запрос пути не удался",
    "path.resultLabel": "Путь",
    "trace.depth": "глубина {depth}",
    "trace.pathCount": "путей: {count}",
    "trace.traceTruncated": "Трасса усечена.",
    "trace.resultTruncated": "Результат усечён лимитом.",
    "trace.entrypointPath": "путь от точки входа",
    "trace.noStart": "Начальный узел не найден.",
    "trace.noOutgoing": "Исходящих связей зависимостей нет.",
    "entryFlows.tracing": "Трассирую точки входа...",
    "entryFlows.buildingWorkflows": "Собираю блок-схемы точек входа...",
    "entryFlows.failedFallback": "трасса точек входа не удалась",
    "entryFlows.workflowFailedFallback": "блок-схема точек входа не удалась",
    "entryFlows.entrypointCount": "точек входа: {count}",
    "entryFlows.traceCount": "трасс: {count}",
    "entryFlows.workflowCount": "блок-схем: {count}",
    "query.workflowStarts": "стартов: {count}",
    "entryFlows.noMatches": "Подходящих потоков входа нет.",
    "entryFlows.noWorkflowMatches": "Подходящих блок-схем входа нет.",
    "entryFlows.traceTruncated": "Трасса усечена глубиной.",
    "entryFlows.reportTruncated": "Отчёт усечён лимитом или глубиной.",
    "entryFlows.focusFlow": "Фокус потока",
    "entryFlows.focusWorkflow": "Фокус блок-схемы",
    "entryFlows.focusTitle": "Вход: {label}",
    "configTrace.enterTarget": "Введите конфиг-файл или переменную окружения.",
    "configTrace.tracing": "Трассирую конфиг...",
    "configTrace.failedFallback": "трасса конфига не удалась",
    "configTrace.targetCount": "целей: {count}",
    "configTrace.readerCount": "читателей: {count}",
    "configTrace.noMatches": "Подходящих конфигов или переменных окружения нет.",
    "configTrace.noReaders": "Прямых читателей нет.",
    "configTrace.readerPath": "путь читателя",
    "configTrace.focusTitle": "Конфиг: {label}",
    "errorTrace.enterTarget": "Введите метку ошибки или исключения.",
    "errorTrace.tracing": "Трассирую ошибки...",
    "errorTrace.failedFallback": "трасса ошибок не удалась",
    "errorTrace.errorCount": "ошибок: {count}",
    "errorTrace.sourceCount": "источников: {count}",
    "errorTrace.noMatches": "Подходящих узлов ошибок нет.",
    "errorTrace.noSources": "Прямых источников нет.",
    "errorTrace.sourcePath": "путь источника",
    "errorTrace.focusTitle": "Ошибка: {label}",
    "trace.tracing": "Трассирую...",
    "workflow.loading": "Загружаю блок-схему...",
    "workflow.noBlocks": "Блоков потока нет.",
    "workflow.blockCount": "блоков: {count}",
    "workflow.transitionCount": "переходов: {count}",
    "workflow.truncated": "Блок-схема усечена глубиной или лимитом блоков.",
    "workflow.risks": "рисков: {count}",
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
    "semantic.coverage": "Семантика",
    "semantic.missing": "Нет семантики",
    "semantic.candidates": "Узлы-кандидаты",
    "semantic.plan": "План семантики",
    "semantic.definitions": "Определения",
    "semantic.symbols": "Символы",
    "semantic.workspace": "Раб. область",
    "semantic.references": "Ссылки",
    "semantic.workQueue": "Очередь работ",
    "semantic.needed": "нужен",
    "semantic.ops": "операций",
    "semantic.noServer": "нет сервера",
    "semantic.errors": "Ошибки",
    "semantic.unmatched": "Без совпадения",
    "legend.kindFilter": "Переключить узлы типа {kind}",
    "legend.riskFilter": "Фильтр графа по рискам: {severity}",
    "queryHistory.recent": "Недавние запросы",
    "queryHistory.clear": "Очистить",
    "queryHistory.run": "Запустить недавний запрос: {query}",
    "status.idle": "ожидание",
    "status.queue": "очередь",
    "status.scan": "скан",
    "status.page": "страница",
    "status.semantic": "семантика",
    "status.ready": "готово",
    "status.error": "ошибка",
    "status.loading": "загрузка",
    "status.chunks": "фрагменты",
    "status.planning": "планирование",
    "status.scanning": "сканирование",
    "status.merging": "merge",
    "status.updating": "обновление",
    "status.stored": "сохранено",
    "status.skipped": "пропущено",
    "kind.error": "ошибка",
    "kind.warning": "предупреждение",
    "kind.info": "инфо",
    "kind.start": "старт",
    "kind.call": "вызов",
    "kind.config_read": "чтение конфига",
    "kind.environment_read": "чтение env",
    "kind.dependency": "зависимость",
    "kind.import": "импорт",
    "kind.reference": "ссылка",
    "kind.external_boundary": "внешняя граница",
    "kind.full_scan": "полный скан",
    "kind.partial_rescan": "частичный перескан",
    "kind.noop": "без изменений",
    "kind.full_reuse": "полный reuse",
    "kind.partial_reuse": "частичный reuse",
    "kind.no_changes": "без изменений",
    "kind.removed_paths": "удалённые пути",
    "kind.incoming_cross_file_edges": "входящие межфайловые связи",
    "kind.graph_surface_added": "surface графа добавлен",
    "kind.graph_surface_removed": "surface графа удалён",
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
    "kind.ambiguous_call_resolution": "неоднозначное разрешение вызова",
    "kind.ambiguous_entrypoint_target": "неоднозначная цель точки входа",
    "kind.unresolved_call": "неразрешённый вызов",
    "kind.parse_error": "ошибка парсинга",
    "kind.syntax_error": "синтаксическая ошибка",
    "kind.semantic_diagnostic": "семантическая диагностика",
    "kind.orphan_function": "изолированная функция",
    "kind.potential_error_flow": "потенциальный поток ошибки",
    "kind.conflicting_config_default": "конфликт default конфига",
    "kind.conflicting_dependency_declaration": "конфликт версии зависимости",
    "kind.mixed_config_requirement": "смешанное требование конфига",
    "kind.mixed_dependency_scope": "смешанный scope зависимости",
    "kind.non_runtime_dependency_import": "non-runtime импорт зависимости",
    "kind.test_only_runtime_dependency": "тестовая runtime-зависимость",
    "kind.sensitive_config_default": "секретный default конфига",
    "kind.undeclared_external_import": "импорт без зависимости",
    "kind.unresolved_local_import": "неразрешённый локальный импорт",
    "kind.unused_declared_dependency": "неиспользуемая зависимость",
    "kind.unreachable_error_flow": "недостижимый поток ошибки",
    "kind.unreachable_source_file": "недостижимый файл с кодом",
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
  enabledKinds: new Set(),
  search: "",
  animationFrame: null,
  selectionRequest: 0,
  traceRequest: 0,
  workflowRequest: 0,
  dependentsRequest: 0,
  edgeExplainRequest: 0,
  entryFlowRequest: 0,
  queryWorkflowRequest: 0,
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
  lastEntryWorkflowReport: null,
  lastPathResult: null,
  lastConfigTraceReport: null,
  lastErrorTraceReport: null,
  lastWorkflowReport: null,
  lastSelectionCard: null,
  pendingSelectionLink: null,
  pendingQueryLink: null,
  pendingGraphPageLink: false,
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
const minimapCanvas = document.querySelector("#graphMinimap");
const minimapCtx = minimapCanvas.getContext("2d");
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
const communityList = document.querySelector("#communityList");
const hotspotList = document.querySelector("#hotspotList");
const annotationList = document.querySelector("#annotationList");
const entrypointList = document.querySelector("#entrypointList");
const entryFlowSearchInput = document.querySelector("#entryFlowSearchInput");
const entryFlowDepthInput = document.querySelector("#entryFlowDepthInput");
const entryWorkflowEdgeKindInput = document.querySelector("#entryWorkflowEdgeKindInput");
const entryWorkflowConfidenceInput = document.querySelector("#entryWorkflowConfidenceInput");
const entryWorkflowLanguageInput = document.querySelector("#entryWorkflowLanguageInput");
const entryWorkflowRiskSeverityInput = document.querySelector("#entryWorkflowRiskSeverityInput");
const entryWorkflowBlockKindInput = document.querySelector("#entryWorkflowBlockKindInput");
const entryFlowButton = document.querySelector("#entryFlowButton");
const entryFlowWorkflowButton = document.querySelector("#entryFlowWorkflowButton");
const entryFlowExportButton = document.querySelector("#entryFlowExportButton");
const entryFlowWorkflowExportButton = document.querySelector("#entryFlowWorkflowExportButton");
const entryFlowWorkflowMermaidExportButton = document.querySelector("#entryFlowWorkflowMermaidExportButton");
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
clearQueryHistoryButton.addEventListener("click", () => clearQueryHistory());
queryInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") runGraphQuery();
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
entryFlowButton.addEventListener("click", () => runEntryFlowTrace());
entryFlowWorkflowButton.addEventListener("click", () => runEntryFlowWorkflows());
entryFlowExportButton.addEventListener("click", () => exportLastEntryFlowReport());
entryFlowWorkflowExportButton.addEventListener("click", () => exportLastEntryWorkflowReport("json"));
entryFlowWorkflowMermaidExportButton.addEventListener("click", () => exportLastEntryWorkflowReport("mermaid"));
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
canvas.addEventListener("pointerleave", onPointerLeave);
canvas.addEventListener("keydown", onCanvasKeyDown);
canvas.addEventListener("wheel", onWheel, { passive: false });
minimapCanvas.addEventListener("pointerdown", onMinimapPointerDown);
minimapCanvas.addEventListener("pointermove", onMinimapPointerMove);
minimapCanvas.addEventListener("pointerup", onMinimapPointerUp);
minimapCanvas.addEventListener("pointerleave", onMinimapPointerUp);
minimapCanvas.addEventListener("keydown", onCanvasKeyDown);
window.addEventListener("resize", resizeCanvas);

syncApiTokenCookie();
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
  document.querySelectorAll("[data-i18n-aria-label]").forEach((element) => {
    const key = element.dataset.i18nAriaLabel;
    if (key) element.setAttribute("aria-label", t(key));
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
  if (state.selectedId == null && selectionTitle.dataset.i18nFallback) {
    selectionTitle.textContent = t(selectionTitle.dataset.i18nFallback);
  }
  renderViewportControls();
  renderGraphPageScope({ focused: Boolean(state.queryFocus) });
  renderOverview();
  renderRuntimeMetrics();
  renderJobQueue();
  renderInsights();
  renderKindFilters(graphKindList());
  renderLegend();
  renderQueryHistory();
  renderQueryExportState();
  renderCheckExportState();
  renderSourceSearchExportState();
  renderEntryFlowExportState();
  renderEntryWorkflowExportState();
  renderPathExportState();
  renderConfigTraceExportState();
  renderErrorTraceExportState();
  renderSelection();
  draw();
}

async function init() {
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
}

function readSelectionLinkFromUrl() {
  try {
    const params = new URLSearchParams(window.location.search);
    return {
      path: params.get("path") || "",
      nodeId: parseUrlInteger(params.get("node")),
      edgeIndex: parseUrlInteger(params.get("edge")),
      query: params.get("query") || "",
      queryFocus: params.get("query_focus") === "1",
      graphPage: readGraphPageLink(params),
    };
  } catch (error) {
    return { path: "", nodeId: null, edgeIndex: null, query: "", queryFocus: false, graphPage: null };
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
    initializeGraph({ preserveView: !resetLayout });
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
      : `<p class="empty">${escapeHtml(t("empty.noHotspots"))}</p>`;
  hotspotList.querySelectorAll("[data-hotspot-node-id]").forEach((button) => {
    button.addEventListener("click", () => {
      focusNodeId(Number(button.dataset.hotspotNodeId), t("focus.hotspot"));
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
  state.graphPage.edgeOffset = 0;
  loadGraphPage({ resetLayout: true });
}

function shiftEdgePage(direction) {
  const nextOffset = state.graphPage.edgeOffset + direction * state.graphPage.edgeLimit;
  state.graphPage.edgeOffset = Math.max(0, nextOffset);
  loadGraphPage({ resetLayout: true });
}

function clearGraphPageFilters() {
  state.architecturePathPrefix = "";
  state.graphPage.nodeOffset = 0;
  state.graphPage.edgeOffset = 0;
  serverKindInput.value = "";
  serverItemKindInput.value = "";
  serverLanguageInput.value = "";
  serverSearchInput.value = "";
  serverEdgeKindInput.value = "";
  serverConfidenceInput.value = "";
  serverEdgeRelationInput.value = "";
  serverEdgeSourceInput.value = "";
  loadGraphPage({ resetLayout: true });
}

function updateGraphPageControls() {
  const start = state.graphPage.totalNodes === 0 ? 0 : state.graphPage.nodeOffset + 1;
  const end = Math.min(
    state.graphPage.totalNodes,
    state.graphPage.nodeOffset + state.graphPage.nodeLimit,
  );
  const edgeStart = state.graphPage.totalEdges === 0 ? 0 : state.graphPage.edgeOffset + 1;
  const edgeEnd = Math.min(
    state.graphPage.totalEdges,
    state.graphPage.edgeOffset + state.graphPage.edgeLimit,
  );
  pageInfo.textContent = `${start}-${end} / ${state.graphPage.totalNodes}`;
  edgePageInfo.textContent = `${edgeStart}-${edgeEnd} / ${state.graphPage.totalEdges} ${t("label.edges").toLowerCase()}`;
  pagePrevButton.disabled = state.graphPage.nodeOffset === 0;
  pageNextButton.disabled = !state.graphPage.truncatedNodes;
  edgePrevButton.disabled = state.graphPage.edgeOffset === 0;
  edgeNextButton.disabled = !state.graphPage.truncatedEdges;
  pageReloadButton.disabled = false;
  pageCopyButton.disabled = state.graph.nodes.length === 0 && state.graph.edges.length === 0;
  pageClearButton.disabled = !graphPageHasFilters();
  renderGraphPageScope();
}

function graphPageHasFilters() {
  return Boolean(
    state.architecturePathPrefix ||
      serverKindInput.value.trim() ||
      serverItemKindInput.value.trim() ||
      serverLanguageInput.value.trim() ||
      serverSearchInput.value.trim() ||
      serverEdgeKindInput.value.trim() ||
      serverConfidenceInput.value.trim() ||
      serverEdgeRelationInput.value.trim() ||
      serverEdgeSourceInput.value.trim() ||
      state.graphPage.nodeOffset > 0 ||
      state.graphPage.edgeOffset > 0,
  );
}

function renderGraphPageScope(options = {}) {
  if (!state.graphPage.root && state.graph.nodes.length === 0 && state.graph.edges.length === 0) {
    pageScope.innerHTML = "";
    return;
  }
  const focused = options.focused || Boolean(state.queryFocus);
  const nodes = formatNumber(state.graph.nodes.length);
  const edges = formatNumber(state.graph.edges.length);
  const totalNodes = formatNumber(state.graphPage.totalNodes);
  const totalEdges = formatNumber(state.graphPage.totalEdges);
  const warnings = [];

  if (focused) {
    pageScope.innerHTML = `<span>${escapeHtml(t("graph.scopeFocused", { nodes, edges }))}</span>`;
    return;
  }

  const message =
    state.graphPage.truncatedNodes || state.graphPage.truncatedEdges
      ? t("graph.scopeLoaded", { nodes, totalNodes, edges, totalEdges })
      : t("graph.scopeComplete", { nodes, edges });
  if (state.graphPage.truncatedNodes) warnings.push(t("graph.scopeNodesTruncated"));
  if (state.graphPage.truncatedEdges) warnings.push(t("graph.scopeEdgesTruncated"));

  pageScope.innerHTML = `
    <span>${escapeHtml(message)}</span>
    ${warnings.map((warning) => `<strong>${escapeHtml(warning)}</strong>`).join("")}
  `;
}

const WORKFLOW_FILTER_FIELDS = [
  ["edge_kind", "EdgeKind"],
  ["confidence", "Confidence"],
  ["language", "Language"],
  ["risk_severity", "RiskSeverity"],
  ["block_kind", "BlockKind"],
];

function workflowFilterInputs(prefix) {
  return WORKFLOW_FILTER_FIELDS.map(([, suffix]) => document.querySelector(`#${prefix}${suffix}Input`)).filter(Boolean);
}

function readWorkflowFilters(prefix) {
  return Object.fromEntries(
    WORKFLOW_FILTER_FIELDS.map(([key, suffix]) => [
      key,
      document.querySelector(`#${prefix}${suffix}Input`)?.value.trim() || "",
    ]).filter(([, value]) => value),
  );
}

function appendWorkflowFilterParams(params, filters) {
  Object.entries(filters || {}).forEach(([key, value]) => {
    if (value) params.set(key, value);
  });
}

function renderWorkflowFilterSummary(filters) {
  const entries = Object.entries(filters || {}).filter(([, value]) => value);
  if (!entries.length) return "";
  return entries
    .map(([key, value]) => `<span>${escapeHtml(`${formatKind(key)}: ${value}`)}</span>`)
    .join("");
}

async function runEntryFlowTrace() {
  const depth = clampNumber(Number(entryFlowDepthInput.value || 3), 1, 32);
  entryFlowDepthInput.value = String(depth);
  state.entryFlowRequest += 1;
  const requestId = state.entryFlowRequest;
  entryFlowButton.disabled = true;
  entryFlowWorkflowButton.disabled = true;
  entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("entryFlows.tracing"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    depth: String(depth),
    limit: "25",
  });
  const search = entryFlowSearchInput.value.trim();
  if (search) params.set("search", search);

  try {
    const response = await apiFetch(`/api/entrypoint-traces?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.entryFlowRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("entryFlows.failedFallback")));
    }
    state.lastEntryFlowReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        search,
        depth,
        limit: 25,
      },
      report: body,
    };
    state.lastEntryWorkflowReport = null;
    renderEntryFlowExportState();
    renderEntryWorkflowExportState();
    entryFlowResult.innerHTML = renderEntryFlowReport(body);
    attachEntryFlowActions(entryFlowResult, body);
  } catch (error) {
    if (requestId !== state.entryFlowRequest) return;
    entryFlowResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.entryFlowRequest) {
      entryFlowButton.disabled = false;
      entryFlowWorkflowButton.disabled = false;
    }
  }
}

async function runEntryFlowWorkflows() {
  const depth = clampNumber(Number(entryFlowDepthInput.value || 3), 1, 32);
  entryFlowDepthInput.value = String(depth);
  state.entryFlowRequest += 1;
  const requestId = state.entryFlowRequest;
  entryFlowButton.disabled = true;
  entryFlowWorkflowButton.disabled = true;
  entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("entryFlows.buildingWorkflows"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    depth: String(depth),
    block_limit: "120",
    limit: "15",
  });
  const search = entryFlowSearchInput.value.trim();
  if (search) params.set("search", search);
  const workflowFilters = readWorkflowFilters("entryWorkflow");
  appendWorkflowFilterParams(params, workflowFilters);

  try {
    const response = await apiFetch(`/api/entrypoint-workflows?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.entryFlowRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("entryFlows.workflowFailedFallback")));
    }
    state.lastEntryWorkflowReport = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        search,
        depth,
        block_limit: 120,
        limit: 15,
        ...workflowFilters,
      },
      report: body,
    };
    state.lastEntryFlowReport = null;
    renderEntryFlowExportState();
    renderEntryWorkflowExportState();
    entryFlowResult.innerHTML = renderEntryWorkflowReport(body);
    attachEntryWorkflowActions(entryFlowResult, body);
  } catch (error) {
    if (requestId !== state.entryFlowRequest) return;
    entryFlowResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
  } finally {
    if (requestId === state.entryFlowRequest) {
      entryFlowButton.disabled = false;
      entryFlowWorkflowButton.disabled = false;
    }
  }
}

function renderEntryFlowExportState() {
  entryFlowExportButton.disabled = !state.lastEntryFlowReport;
}

function renderEntryWorkflowExportState() {
  entryFlowWorkflowExportButton.disabled = !state.lastEntryWorkflowReport;
  entryFlowWorkflowMermaidExportButton.disabled = !state.lastEntryWorkflowReport;
}

function clearLastEntryFlowReport() {
  state.lastEntryFlowReport = null;
  renderEntryFlowExportState();
}

function clearLastEntryWorkflowReport() {
  state.lastEntryWorkflowReport = null;
  renderEntryWorkflowExportState();
}

function exportLastEntryFlowReport() {
  if (!state.lastEntryFlowReport) {
    entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noEntryFlows"))}</p>`;
    renderEntryFlowExportState();
    return;
  }

  const payload = {
    schema: "codegraph.entrypoint_traces.v1",
    ...state.lastEntryFlowReport,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-entrypoint-traces.json`;
  downloadBlob(blob, fileName);
  entryFlowResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.entryFlows"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(t("entryFlows.traceCount", { count: formatNumber(payload.report?.traces?.length ?? 0) }))}</span>
        <span>${escapeHtml(t("entryFlows.entrypointCount", { count: formatNumber(payload.report?.total_entrypoints ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function exportLastEntryWorkflowReport(format) {
  if (!state.lastEntryWorkflowReport) {
    entryFlowResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noEntryWorkflows"))}</p>`;
    renderEntryWorkflowExportState();
    return;
  }

  const payload = {
    schema: "codegraph.entrypoint_workflows.v1",
    ...state.lastEntryWorkflowReport,
  };
  const root = safeFilePart(payload.root);
  if (format === "mermaid") {
    const mermaid = entryWorkflowReportToMermaid(payload.report);
    const blob = new Blob([mermaid], { type: "text/vnd.mermaid;charset=utf-8" });
    const fileName = `codegraph-${root}-entrypoint-workflows.mmd`;
    downloadBlob(blob, fileName);
    renderEntryFlowExportNote(fileName, blob.size, t("export.entryWorkflowMermaid"));
    return;
  }

  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${root}-entrypoint-workflows.json`;
  downloadBlob(blob, fileName);
  renderEntryFlowExportNote(fileName, blob.size, t("export.entryWorkflows"));
}

function renderEntryFlowExportNote(fileName, size, label) {
  entryFlowResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(label)}</span>
        <span>${escapeHtml(formatBytes(size))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function renderEntryFlowReport(report) {
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("entryFlows.entrypointCount", { count: formatNumber(report.total_entrypoints || 0) }))}</span>
      <span>${escapeHtml(t("entryFlows.traceCount", { count: formatNumber(report.traces.length) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: formatNumber(report.max_depth || 0) }))}</span>
    </div>
  `;
  if (!report.traces.length) {
    return `${summary}<p class="empty">${escapeHtml(t("entryFlows.noMatches"))}</p>`;
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
      const truncated = trace.truncated
        ? `<p class="empty">${escapeHtml(t("entryFlows.traceTruncated"))}</p>`
        : "";
      return `
        <section class="trace-columns">
          <h3>${escapeHtml(trace.start.label)}</h3>
          <div class="trace-summary">
            <span>${escapeHtml(t("stat.nodes"))}: ${escapeHtml(formatNumber(trace.nodes.length))}</span>
            <span>${escapeHtml(t("stat.edges"))}: ${escapeHtml(formatNumber(trace.edges.length))}</span>
            <span>${escapeHtml(formatKind(trace.start.metadata?.entrypoint_kind || trace.start.kind))}</span>
          </div>
          <div class="query-actions">
            <button type="button" data-entry-flow="${index}">${escapeHtml(t("entryFlows.focusFlow"))}</button>
          </div>
          ${nodes ? `<ul class="trace-list">${nodes}</ul>` : `<p class="empty">${escapeHtml(t("trace.noOutgoing"))}</p>`}
          ${truncated}
        </section>
      `;
    })
    .join("");
  const truncated = report.truncated
    ? `<p class="empty">${escapeHtml(t("entryFlows.reportTruncated"))}</p>`
    : "";
  return `${summary}${rows}${truncated}`;
}

function renderEntryWorkflowReport(report) {
  const workflows = Array.isArray(report.workflows) ? report.workflows : [];
  const summary = `
    <div class="query-summary">
      <span>${escapeHtml(t("entryFlows.entrypointCount", { count: formatNumber(report.total_entrypoints || 0) }))}</span>
      <span>${escapeHtml(t("entryFlows.workflowCount", { count: formatNumber(workflows.length) }))}</span>
      <span>${escapeHtml(t("trace.depth", { depth: formatNumber(report.max_depth || 0) }))}</span>
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
            <button type="button" data-entry-workflow="${index}">${escapeHtml(t("entryFlows.focusWorkflow"))}</button>
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
      showFocusedGraph(focused, t("entryFlows.focusTitle", { label: trace.start.label }), trace.start.id);
    });
  });
}

function attachEntryWorkflowActions(container, report) {
  attachWorkflowNavigation(container);
  attachEdgeExplainActions(container);
  container.querySelectorAll("[data-entry-workflow]").forEach((button) => {
    button.addEventListener("click", () => {
      const workflow = report.workflows?.[Number(button.dataset.entryWorkflow)];
      if (!workflow) return;
      const blocks = Array.isArray(workflow.blocks) ? workflow.blocks : [];
      const transitions = Array.isArray(workflow.transitions) ? workflow.transitions : [];
      const focused = {
        query: `workflow-entrypoints ${workflow.start?.label || ""}`,
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
    const response = await apiFetch(`/api/insights?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.insightRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "insights failed"));
    }
    state.insightReport = body;
    refreshRiskIndex();
    renderInsights();
    renderLegend();
    draw();
  } catch (error) {
    if (requestId !== state.insightRequest) return;
    state.insightReport = null;
    refreshRiskIndex();
    renderInsights();
    renderLegend();
    draw();
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
  checkResult.innerHTML = `<p class="empty">${escapeHtml(t("check.running"))}</p>`;

  try {
    const response = await apiFetch(`/api/check?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.checkRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("check.failedFallback")));
    }
    state.lastCheckResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      filters: {
        fail_on: failOn,
        kind,
        search,
      },
      result: body,
    };
    renderCheckExportState();
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

function renderCheckExportState() {
  checkExportButton.disabled = !state.lastCheckResult;
}

function clearLastCheckResult() {
  state.lastCheckResult = null;
  renderCheckExportState();
}

function exportLastCheckResult() {
  if (!state.lastCheckResult) {
    checkResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noCheck"))}</p>`;
    renderCheckExportState();
    return;
  }

  const payload = {
    schema: "codegraph.check_result.v1",
    ...state.lastCheckResult,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-check-result.json`;
  downloadBlob(blob, fileName);
  checkResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.check"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(payload.result?.passed ? t("check.passed") : t("check.failed"))}</span>
        <span>${escapeHtml(t("check.failingCount", { count: formatNumber(payload.result?.failing_insights ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
}

function exportCurrentInsights() {
  const clientInsights = state.insightReport ? [] : buildClientInsights(state.graph);
  const report = state.insightReport || {
    total: clientInsights.length,
    insights: clientInsights,
    severity_counts: countInsightField(clientInsights, "severity"),
    kind_counts: countInsightField(clientInsights, "kind"),
  };
  const payload = {
    schema: "codegraph.insights_export.v1",
    generated_at: new Date().toISOString(),
    root: state.graphPage.root || pathInput.value.trim() || ".",
    source: state.insightReport ? "server" : "client",
    filters: {
      severity: insightSeverityInput.value.trim(),
      kind: insightKindInput.value.trim(),
      search: insightSearchInput.value.trim(),
      fail_on: checkFailOnInput.value.trim() || "error",
    },
    report,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-insights.json`;
  downloadBlob(blob, fileName);
  checkResult.innerHTML = `
    <div class="query-summary">
      <span>${escapeHtml(t("export.insights"))}</span>
      <span>${escapeHtml(formatBytes(blob.size))}</span>
      <span>${escapeHtml(t("insights.count", { count: formatNumber(report.total ?? report.insights?.length ?? 0) }))}</span>
      <span class="query-expression">${escapeHtml(fileName)}</span>
    </div>
  `;
}

function countInsightField(insights, field) {
  return insights.reduce((counts, insight) => {
    const key = insight?.[field] || "unknown";
    counts[key] = (counts[key] || 0) + 1;
    return counts;
  }, {});
}

function renderCheckReport(check) {
  const stateClass = check.passed ? "passed" : "failed";
  const label = check.passed ? t("check.passed") : t("check.failed");
  return `
    <div class="check-card ${stateClass}">
      <strong>${escapeHtml(label)}</strong>
      <span>${escapeHtml(t("check.failOn", { severity: formatKind(check.fail_on || "error") }))}</span>
      <span>${escapeHtml(t("check.failingCount", { count: formatNumber(check.failing_insights || 0) }))}</span>
      <span>${escapeHtml(t("check.matchedCount", { count: formatNumber(check.report?.total || 0) }))}</span>
    </div>
  `;
}

function initializeGraph(options = {}) {
  const preserveView = Boolean(options.preserveView);
  const previousPan = { ...state.pan };
  const previousZoom = state.zoom;

  clearSelection({ syncUrl: false, render: false });
  state.edgeSelectionCache.clear();
  state.edgeSelectionNodeCache.clear();
  state.hoveredId = null;
  state.hoveredEdgeKey = null;
  state.positions.clear();
  state.velocities.clear();
  const kinds = [...new Set(state.graph.nodes.map((node) => node.kind))].sort();
  state.enabledKinds = new Set(kinds);
  refreshRiskIndex();
  renderKindFilters(kinds);
  renderLegend();
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

async function runGraphQuery(options = {}) {
  const expression = queryInput.value.trim();
  if (!expression) {
    queryResult.innerHTML = `<p class="empty">${escapeHtml(t("query.enterExpression"))}</p>`;
    return null;
  }
  if (!graphQueryWithinClientLimit(expression, queryResult)) {
    clearLastQueryResult();
    return null;
  }

  state.queryRequest += 1;
  const requestId = state.queryRequest;
  queryButton.disabled = true;
  queryResult.innerHTML = `<p class="empty">${escapeHtml(t("query.running"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: expression,
  });

  try {
    const response = await apiFetch(`/api/query?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.queryRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "query failed"));
    }
    if (options.syncUrl !== false) {
      syncQueryUrl(expression, { focus: Boolean(options.focus) });
    }
    queryResult.innerHTML = renderQueryResult(body);
    attachQueryNavigation(queryResult);
    attachEdgeExplainActions(queryResult);
    attachQueryFocusActions(queryResult, body);
    state.lastQueryResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      query: expression,
      result: body,
    };
    renderQueryExportState();
    rememberQuery(expression);
    if (options.focus) {
      focusQueryResult(body, queryResult);
      fitVisibleGraph();
    }
    return body;
  } catch (error) {
    if (requestId !== state.queryRequest) return;
    queryResult.innerHTML = `<p class="error-text">${escapeHtml(error.message)}</p>`;
    return null;
  } finally {
    if (requestId === state.queryRequest) {
      queryButton.disabled = false;
    }
  }
}

async function runSourceSearch() {
  const query = sourceSearchInput.value.trim();
  if (!query) {
    sourceSearchResult.innerHTML = `<p class="empty">${escapeHtml(t("sourceSearch.enterText"))}</p>`;
    return;
  }

  state.sourceSearchRequest += 1;
  const requestId = state.sourceSearchRequest;
  sourceSearchButton.disabled = true;
  sourceSearchResult.innerHTML = `<p class="empty">${escapeHtml(t("sourceSearch.searching"))}</p>`;

  const params = new URLSearchParams({
    path: pathInput.value.trim() || ".",
    q: query,
    limit: "50",
    context: "2",
  });
  const pathFilter = sourcePathFilterInput.value.trim();
  if (pathFilter) params.set("path_filter", pathFilter);

  try {
    const response = await apiFetch(`/api/source-search?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.sourceSearchRequest) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, t("sourceSearch.failedFallback")));
    }
    state.lastSourceSearchResult = {
      generated_at: new Date().toISOString(),
      root: pathInput.value.trim() || ".",
      query,
      path_filter: pathFilter,
      result: body,
    };
    renderSourceSearchExportState();
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

function renderSourceSearchExportState() {
  sourceSearchExportButton.disabled = !state.lastSourceSearchResult;
}

function clearLastSourceSearchResult() {
  state.lastSourceSearchResult = null;
  renderSourceSearchExportState();
}

function exportLastSourceSearchResult() {
  if (!state.lastSourceSearchResult) {
    sourceSearchResult.innerHTML = `<p class="empty">${escapeHtml(t("export.noSourceSearch"))}</p>`;
    renderSourceSearchExportState();
    return;
  }

  const payload = {
    schema: "codegraph.source_search_result.v1",
    ...state.lastSourceSearchResult,
  };
  const serialized = JSON.stringify(payload, null, 2);
  const blob = new Blob([serialized], { type: "application/json" });
  const fileName = `codegraph-${safeFilePart(payload.root)}-source-search.json`;
  downloadBlob(blob, fileName);
  sourceSearchResult.insertAdjacentHTML(
    "afterbegin",
    `
      <div class="query-summary">
        <span>${escapeHtml(t("export.sourceSearch"))}</span>
        <span>${escapeHtml(formatBytes(blob.size))}</span>
        <span>${escapeHtml(t("sourceSearch.matchCount", { count: formatNumber(payload.result?.total_matches ?? payload.result?.matches?.length ?? 0) }))}</span>
        <span class="query-expression">${escapeHtml(fileName)}</span>
      </div>
    `,
  );
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
    root: pathInput.value.trim() || ".",
    path: match.path,
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
    const label = `Focus: ${formatKind(insight.kind)}`;
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
  pageInfo.textContent = `focus ${result.nodes.length} / ${result.total_nodes}`;
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

function buildClientInsights(graph) {
  const insights = [];
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
        severity: "warning",
        message: `${label} appears ${nodes.length} times and may make label-based traces ambiguous`,
        nodeId: nodes[0].id,
      });
    }
  });

  const callsByLabel = new Map();
  graph.edges
    .filter((edge) => edge.kind === "calls" && edge.metadata?.call_label)
    .forEach((edge) => {
      const key = `${edge.source}:${edge.metadata.call_label}`;
      const list = callsByLabel.get(key) || [];
      list.push(edge);
      callsByLabel.set(key, list);
    });
  callsByLabel.forEach((edges) => {
    const targets = Array.from(new Set(edges.map((edge) => edge.target)));
    if (targets.length < 2) return;
    const source = graph.nodes.find((node) => node.id === edges[0].source);
    const targetLabels = targets
      .map((id) => graph.nodes.find((node) => node.id === id)?.label || id)
      .slice(0, 5)
      .join(", ");
    insights.push({
      kind: "ambiguous_call_resolution",
      severity: "warning",
      message: `${source?.label || edges[0].source} calls ${edges[0].metadata.call_label} but it resolves to multiple targets: ${targetLabels}`,
      nodeId: source?.id || targets[0],
    });
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
      if (reachableIds.size > 0 && !reachableIds.has(edge.source)) {
        insights.push({
          kind: "unreachable_error_flow",
          severity: "warning",
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

  (graph.edges || []).forEach((edge) => {
    if (!clientTraceEdge(edge)) return;
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
  graph.edges
    .filter((edge) => edge.kind === "imports")
    .forEach((edge) => {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      if (target?.metadata?.import_scope === "local") return;
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
  zoomAt(canvas.width / 2, canvas.height / 2, scale);
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
  renderGraphHud();
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

function panGraphBy(dx, dy) {
  state.pan.x += dx;
  state.pan.y += dy;
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
  const neighborhood = graphNeighborhoodContext(visibleIds);
  const highlightedEdges = [];
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const emphasis = edgeEmphasis(edge);
    if (emphasis !== "normal") {
      highlightedEdges.push([edge, emphasis]);
      return;
    }
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    const alpha = edgeNeighborhoodAlpha(edge, neighborhood);
    if (alpha < 1) ctx.globalAlpha = alpha;
    drawEdge(edge, source, target, "normal");
    if (alpha < 1) ctx.globalAlpha = 1;
  });

  highlightedEdges.forEach(([edge, emphasis]) => {
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    drawEdge(edge, source, target, emphasis);
  });

  const labelCandidates = [];
  const riskByNode = state.riskByNode;
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    const selected = node.id === state.selectedId;
    const hovered = node.id === state.hoveredId;
    const focused = nodeIsFocused(node);
    const neighbor = nodeIsNeighborhoodNeighbor(node, neighborhood);
    const muted = nodeIsNeighborhoodMuted(node, neighborhood, selected, hovered, focused);
    const radius = nodeRadius(node);

    if (muted) ctx.globalAlpha = 0.34;
    ctx.beginPath();
    ctx.arc(
      position.x,
      position.y,
      radius + (selected ? 6 : focused ? 5 : hovered ? 3 : neighbor ? 4 : 0),
      0,
      Math.PI * 2,
    );
    ctx.fillStyle = selected
      ? "rgba(92, 200, 167, 0.26)"
      : focused
        ? "rgba(237, 241, 242, 0.16)"
        : hovered
          ? "rgba(255,255,255,0.12)"
          : neighbor
            ? "rgba(92, 200, 167, 0.13)"
          : "rgba(0,0,0,0.22)";
    ctx.fill();

    ctx.beginPath();
    ctx.arc(position.x, position.y, radius, 0, Math.PI * 2);
    ctx.fillStyle = colorFor(node.kind);
    ctx.fill();
    ctx.lineWidth = selected ? 2.6 / state.zoom : focused ? 2.2 / state.zoom : neighbor ? 1.8 / state.zoom : 1 / state.zoom;
    ctx.strokeStyle = selected
      ? "#ffffff"
      : focused
        ? "rgba(237, 241, 242, 0.92)"
        : neighbor
          ? "rgba(92, 200, 167, 0.84)"
          : "rgba(255,255,255,0.55)";
    ctx.stroke();

    const riskSeverity = riskByNode.get(Number(node.id));
    if (riskSeverity) {
      drawRiskHalo(position, radius, riskSeverity, selected || focused || hovered || neighbor);
    }
    if (muted) ctx.globalAlpha = 1;

    if (!muted && shouldShowNodeLabel(node, selected, hovered, focused)) {
      labelCandidates.push({
        node,
        position,
        radius,
        selected,
        hovered,
        focused,
        forced: false,
        priority: nodeLabelPriority(node),
        bypassBudget: state.labelMode === "hover" && hovered,
      });
    }
  });

  drawNodeLabels(labelCandidates);
  ctx.restore();
  drawGraphMinimap();
}

function graphWorldBounds(padding = 32) {
  if (state.visibleNodes.length === 0) return null;

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    if (!position) return;
    const radius = nodeRadius(node);
    minX = Math.min(minX, position.x - radius);
    minY = Math.min(minY, position.y - radius);
    maxX = Math.max(maxX, position.x + radius);
    maxY = Math.max(maxY, position.y + radius);
  });

  if (!Number.isFinite(minX) || !Number.isFinite(minY)) return null;

  if (maxX - minX < 1) {
    minX -= 40;
    maxX += 40;
  }
  if (maxY - minY < 1) {
    minY -= 40;
    maxY += 40;
  }

  return {
    minX: minX - padding,
    minY: minY - padding,
    maxX: maxX + padding,
    maxY: maxY + padding,
  };
}

function minimapTransform() {
  const rect = minimapCanvas.getBoundingClientRect();
  const width = Math.max(1, Math.floor(rect.width));
  const height = Math.max(1, Math.floor(rect.height));
  const bounds = graphWorldBounds(48);
  if (!bounds) return null;

  if (minimapCanvas.width !== width || minimapCanvas.height !== height) {
    minimapCanvas.width = width;
    minimapCanvas.height = height;
  }

  const padding = 10;
  const graphWidth = Math.max(1, bounds.maxX - bounds.minX);
  const graphHeight = Math.max(1, bounds.maxY - bounds.minY);
  const scale = Math.min(
    Math.max(1, width - padding * 2) / graphWidth,
    Math.max(1, height - padding * 2) / graphHeight,
  );
  const offsetX = (width - graphWidth * scale) / 2 - bounds.minX * scale;
  const offsetY = (height - graphHeight * scale) / 2 - bounds.minY * scale;
  return { bounds, width, height, scale, offsetX, offsetY };
}

function worldToMinimap(point, transform) {
  return {
    x: point.x * transform.scale + transform.offsetX,
    y: point.y * transform.scale + transform.offsetY,
  };
}

function minimapToWorld(point, transform) {
  return {
    x: (point.x - transform.offsetX) / transform.scale,
    y: (point.y - transform.offsetY) / transform.scale,
  };
}

function drawGraphMinimap() {
  if (state.visibleNodes.length === 0) {
    minimapCanvas.hidden = true;
    return;
  }
  minimapCanvas.hidden = false;

  const transform = minimapTransform();
  if (!transform) return;

  minimapCtx.clearRect(0, 0, transform.width, transform.height);
  minimapCtx.fillStyle = "rgba(16, 18, 20, 0.82)";
  minimapCtx.fillRect(0, 0, transform.width, transform.height);

  const visibleIds = new Set(state.visibleNodes.map((node) => node.id));
  minimapCtx.lineWidth = 1;
  minimapCtx.strokeStyle = "rgba(237, 241, 242, 0.16)";
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    if (!source || !target) return;
    const start = worldToMinimap(source, transform);
    const end = worldToMinimap(target, transform);
    minimapCtx.beginPath();
    minimapCtx.moveTo(start.x, start.y);
    minimapCtx.lineTo(end.x, end.y);
    minimapCtx.stroke();
  });

  state.visibleNodes.forEach((node) => {
    const position = state.positions.get(node.id);
    if (!position) return;
    const point = worldToMinimap(position, transform);
    const selected = node.id === state.selectedId;
    const focused = nodeIsFocused(node);
    minimapCtx.beginPath();
    minimapCtx.arc(point.x, point.y, selected || focused ? 3.5 : 2.4, 0, Math.PI * 2);
    minimapCtx.fillStyle = selected ? "#ffffff" : focused ? "#5cc8a7" : colorFor(node.kind);
    minimapCtx.fill();
  });

  const topLeft = worldToMinimap(screenToWorld(0, 0), transform);
  const bottomRight = worldToMinimap(screenToWorld(canvas.width, canvas.height), transform);
  const viewX = Math.min(topLeft.x, bottomRight.x);
  const viewY = Math.min(topLeft.y, bottomRight.y);
  const viewWidth = Math.abs(bottomRight.x - topLeft.x);
  const viewHeight = Math.abs(bottomRight.y - topLeft.y);
  minimapCtx.fillStyle = "rgba(92, 200, 167, 0.12)";
  minimapCtx.strokeStyle = "rgba(92, 200, 167, 0.92)";
  minimapCtx.lineWidth = 1.4;
  minimapCtx.fillRect(viewX, viewY, viewWidth, viewHeight);
  minimapCtx.strokeRect(viewX, viewY, viewWidth, viewHeight);
}

function drawRiskHalo(position, radius, severity, emphasized) {
  const zoom = Math.max(0.18, state.zoom);
  ctx.beginPath();
  ctx.arc(position.x, position.y, radius + (emphasized ? 8 : 5) / zoom, 0, Math.PI * 2);
  ctx.lineWidth = (emphasized ? 3.2 : 2.2) / zoom;
  ctx.strokeStyle = riskColor(severity);
  ctx.stroke();
}

function drawEdge(edge, source, target, emphasis) {
  if (!source || !target) return;
  const emphasized = emphasis !== "normal";
  const dx = target.x - source.x;
  const dy = target.y - source.y;
  const distance = Math.max(1, Math.sqrt(dx * dx + dy * dy));
  const ux = dx / distance;
  const uy = dy / distance;
  const sourceRadius = nodeRadiusById(edge.source) + 2 / state.zoom;
  const targetRadius = nodeRadiusById(edge.target) + (emphasized ? 8 : 3) / state.zoom;
  const start = {
    x: source.x + ux * Math.min(sourceRadius, distance * 0.35),
    y: source.y + uy * Math.min(sourceRadius, distance * 0.35),
  };
  const end = {
    x: target.x - ux * Math.min(targetRadius, distance * 0.35),
    y: target.y - uy * Math.min(targetRadius, distance * 0.35),
  };

  if (emphasized) {
    ctx.beginPath();
    ctx.moveTo(start.x, start.y);
    ctx.lineTo(end.x, end.y);
    ctx.lineWidth = edgeBackplateWidth(emphasis) / state.zoom;
    ctx.strokeStyle = "rgba(13, 15, 16, 0.76)";
    ctx.stroke();
  }

  ctx.beginPath();
  ctx.moveTo(start.x, start.y);
  ctx.lineTo(end.x, end.y);
  ctx.lineWidth = edgeStrokeWidth(emphasis) / state.zoom;
  ctx.strokeStyle = emphasis === "normal" ? edgeColor(edge) : edgeHighlightColor(emphasis);
  ctx.stroke();

  if (emphasized) {
    drawArrowHead(start, end, edgeHighlightColor(emphasis));
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
  return CodeGraphLabelPolicy.shouldShowNodeLabel({
    selected,
    hovered,
    focused,
    labelMode: state.labelMode,
    zoom: state.zoom,
    visibleCount: state.visibleNodes.length,
    hasSearch: Boolean(state.search),
    priority: nodeLabelPriority(node),
  });
}

function drawNodeLabels(candidates) {
  const occupied = [];
  const nodeBoxes = nodeOcclusionBoxes();
  const edgeBoxes = edgeOcclusionBoxes();
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
    if (!candidate.bypassBudget && drawnAutoLabels >= budget) return;
    const geometry = labelGeometry(candidate, occupied, nodeBoxes, edgeBoxes);
    if (!geometry) return;
    drawLabelGeometry(geometry);
    occupied.push(geometry);
    if (!candidate.bypassBudget) drawnAutoLabels += 1;
  });
}

function labelGeometry(candidate, occupied, nodeBoxes, edgeBoxes) {
  const { node, position, radius, forced, hovered, focused } = candidate;
  const zoom = Math.max(0.18, state.zoom);
  const textLength = hovered ? 12 : focused ? 11 : 8;
  const lines = forced
    ? compactGraphLabelLines(node.label, 14, 1)
    : [truncateGraphLabel(node.label, textLength)];
  const padX = (forced ? 6 : 4) / zoom;
  const padY = (forced ? 4 : 2.5) / zoom;
  const fontSize = (forced ? 11 : 8) / zoom;
  const lineHeight = (forced ? 12 : 9) / zoom;
  ctx.font = `${fontSize}px Inter, sans-serif`;
  const width = Math.max(...lines.map((line) => ctx.measureText(line).width)) + padX * 2;
  const height = lines.length * lineHeight + padY * 2;
  const gap = (forced ? 11 : 21) / zoom;
  const placements = forced ? ["right", "left", "top"] : ["right", "left"];
  const geometries = placements.map((placement) =>
    clampLabelGeometryToViewport(labelGeometryForPlacement({
      node,
      position,
      radius,
      lines,
      width,
      height,
      padX,
      gap,
      lineHeight,
      font: ctx.font,
      forced,
      placement,
    })),
  );
  const usable = geometries.find(
    (geometry) =>
      boxIntersectsViewport(geometry) && !labelIntersectsScene(geometry, occupied, nodeBoxes, edgeBoxes),
  );
  if (usable) return usable;
  return null;
}

function labelGeometryForPlacement(options) {
  const {
    node,
    position,
    radius,
    lines,
    width,
    height,
    padX,
    gap,
    lineHeight,
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
    lines,
    x,
    y,
    width,
    height,
    padX,
    textY: y + height / 2,
    lineHeight,
    radius: 5 / Math.max(0.18, state.zoom),
    font,
    forced,
  };
}

function drawLabelGeometry(geometry) {
  ctx.font = geometry.font;
  ctx.textBaseline = "middle";
  if (!geometry.forced) {
    ctx.fillStyle = "rgba(13, 15, 16, 0.78)";
    roundRect(ctx, geometry.x, geometry.y, geometry.width, geometry.height, geometry.radius);
    ctx.fill();
    ctx.lineWidth = 1 / Math.max(0.18, state.zoom);
    ctx.strokeStyle = "rgba(237, 241, 242, 0.16)";
    ctx.stroke();
    ctx.fillStyle = "rgba(237, 241, 242, 0.9)";
    drawLabelText(geometry, (line, x, y) => ctx.fillText(line, x, y));
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
  drawLabelText(geometry, (line, x, y) => ctx.fillText(line, x, y));
}

function clampLabelGeometryToViewport(geometry) {
  const zoom = Math.max(0.18, state.zoom);
  const margin = 8 / zoom;
  const minX = (-state.pan.x / zoom) + margin;
  const minY = (-state.pan.y / zoom) + margin;
  const maxX = ((canvas.width - state.pan.x) / zoom) - geometry.width - margin;
  const maxY = ((canvas.height - state.pan.y) / zoom) - geometry.height - margin;
  if (maxX >= minX) geometry.x = Math.max(minX, Math.min(maxX, geometry.x));
  if (maxY >= minY) geometry.y = Math.max(minY, Math.min(maxY, geometry.y));
  geometry.textY = geometry.y + geometry.height / 2;
  return geometry;
}

function drawLabelText(geometry, drawLine) {
  const firstY = geometry.textY - ((geometry.lines.length - 1) * geometry.lineHeight) / 2;
  geometry.lines.forEach((line, index) => {
    drawLine(line, geometry.x + geometry.padX, firstY + index * geometry.lineHeight);
  });
}

function nodeLabelPriority(node) {
  return CodeGraphLabelPolicy.nodeLabelPriority(node);
}

function nodeLabelBudget() {
  return CodeGraphLabelPolicy.nodeLabelBudget({
    labelMode: state.labelMode,
    zoom: state.zoom,
    visibleCount: state.visibleNodes.length,
    hasSearch: Boolean(state.search),
  });
}

function nodeOcclusionBoxes() {
  const pad = 32 / Math.max(0.18, state.zoom);
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

function edgeOcclusionBoxes() {
  const pad = 8 / Math.max(0.18, state.zoom);
  return state.visibleEdges
    .map((edge) => {
      const source = state.positions.get(edge.source);
      const target = state.positions.get(edge.target);
      if (!source || !target) return null;
      const minX = Math.min(source.x, target.x) - pad;
      const minY = Math.min(source.y, target.y) - pad;
      return {
        source: edge.source,
        target: edge.target,
        x: minX,
        y: minY,
        width: Math.abs(source.x - target.x) + pad * 2,
        height: Math.abs(source.y - target.y) + pad * 2,
      };
    })
    .filter(Boolean);
}

function truncateGraphLabel(value, maxLength) {
  return CodeGraphLabelPolicy.truncateGraphLabel(value, maxLength);
}

function compactGraphLabelLines(value, maxLength, maxLines) {
  return CodeGraphLabelPolicy.compactGraphLabelLines(value, maxLength, maxLines);
}

function boxIntersectsViewport(box) {
  const left = box.x * state.zoom + state.pan.x;
  const right = (box.x + box.width) * state.zoom + state.pan.x;
  const top = box.y * state.zoom + state.pan.y;
  const bottom = (box.y + box.height) * state.zoom + state.pan.y;
  const margin = 24;
  return !(right < -margin || left > canvas.width + margin || bottom < -margin || top > canvas.height + margin);
}

function labelIntersectsScene(label, occupied, nodeBoxes, edgeBoxes) {
  return (
    occupied.some((box) => boxesIntersect(box, label)) ||
    nodeBoxes.some((box) => box.nodeId !== label.nodeId && boxesIntersect(box, label)) ||
    edgeBoxes.some(
      (box) =>
        box.source !== label.nodeId &&
        box.target !== label.nodeId &&
        boxesIntersect(box, label),
    )
  );
}

function boxesIntersect(left, right) {
  const pad = 12 / Math.max(0.18, state.zoom);
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
  if (state.selectedEdgeKey) {
    const edgeRecord = selectedEdge();
    if (edgeRecord) {
      renderEdgeSelectionPanel(edgeRecord.edge, edgeRecord.source, edgeRecord.target);
    } else {
      clearLastSelectionCard();
      selectionTitle.textContent = t("selection.edge");
      selectionBody.innerHTML = `<p class="empty">${escapeHtml(t("selection.noEdge"))}</p>`;
    }
    return;
  }
  const node = state.graph.nodes.find((candidate) => candidate.id === state.selectedId);
  if (!node) {
    clearLastSelectionCard();
    if (state.selectedId != null) {
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

function selectedEdge() {
  const key = state.selectedEdgeKey;
  if (!key) return null;
  const edge =
    state.edgeSelectionCache.get(key) ||
    state.visibleEdges.find((edge) => edgeSelectionKey(edge) === key) ||
    state.graph.edges.find((edge) => edgeSelectionKey(edge) === key) ||
    null;
  if (!edge) return null;
  const cachedNodes = state.edgeSelectionNodeCache.get(key) || {};
  return {
    edge,
    source: cachedNodes.source || state.graph.nodes.find((node) => node.id === edge.source),
    target: cachedNodes.target || state.graph.nodes.find((node) => node.id === edge.target),
  };
}

function renderEdgeSelectionPanel(edge, source = null, target = null) {
  const edgeIndex = edgeIndexOf(edge);
  const metadataRows = Object.entries(edge.metadata || {})
    .filter(([key]) => key !== "edge_index")
    .map(([key, value]) => [formatKind(key), value]);

  setLastSelectionCard(buildEdgeSelectionCard(edge, source, target));
  selectionTitle.textContent = t("selection.edge");
  selectionBody.innerHTML = `
    <div class="node-card edge-card">
      <header class="node-card-header">
        <div class="node-card-title">
          <span>${escapeHtml(formatKind(edge.kind))}</span>
          <strong>${escapeHtml(source?.label || String(edge.source))}</strong>
          <em>${escapeHtml(target?.label || String(edge.target))}</em>
        </div>
        ${edgeIndex == null ? "" : `<span class="node-card-id">#${edgeIndex}</span>`}
      </header>
      <div class="selection-actions">
        <button type="button" data-node-id="${edge.source}">${escapeHtml(t("selection.openSource"))}</button>
        <button type="button" data-node-id="${edge.target}">${escapeHtml(t("selection.openTarget"))}</button>
        ${
          edgeIndex == null
            ? ""
            : `<button type="button" data-copy-selection-link="edge" data-selection-link-id="${edgeIndex}">${escapeHtml(t("button.copyLink"))}</button>`
        }
        <button type="button" data-export-selection-card>${escapeHtml(t("button.downloadCard"))}</button>
        ${
          edgeIndex == null
            ? ""
            : `<button type="button" data-focus-edge-index="${edgeIndex}">${escapeHtml(t("button.focusEdge"))}</button>
               <button type="button" data-query-edge-index="${edgeIndex}">${escapeHtml(t("button.queryEdge"))}</button>`
        }
      </div>
      <section class="node-card-section">
        <h3>${escapeHtml(t("selection.summary"))}</h3>
        <dl class="node-summary">
          ${[
            [t("selection.edgeIndex"), edgeIndex == null ? "" : String(edgeIndex)],
            [t("selection.kind"), formatKind(edge.kind)],
            [t("label.confidence"), formatKind(edge.confidence || "unknown")],
            [t("label.from"), source?.label || String(edge.source)],
            [t("label.to"), target?.label || String(edge.target)],
          ]
            .filter(([, value]) => value)
            .map(renderDefinitionRow)
            .join("")}
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
        <div class="edge-row">
          ${renderExplainEdgeButton(edge)}
        </div>
        <div class="edge-explanation" data-edge-explanation hidden></div>
      </section>
    </div>
  `;
  attachQueryNavigation(selectionBody);
  attachEdgeExplainActions(selectionBody);
  attachCopyLinkActions(selectionBody);
  attachSelectionCardExportAction();
  selectionBody.querySelectorAll("[data-focus-edge-index]").forEach((button) => {
    button.addEventListener("click", () => focusEdgeIndex(Number(button.dataset.focusEdgeIndex)));
  });
  selectionBody.querySelectorAll("[data-query-edge-index]").forEach((button) => {
    button.addEventListener("click", () => runEdgeIndexQuery(Number(button.dataset.queryEdgeIndex)));
  });
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
    const response = await apiFetch(`/api/node-card?${params.toString()}`);
    const body = await response.json();
    if (requestId !== state.selectionRequest || state.selectedId !== nodeId) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "node card failed"));
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
  state.lastWorkflowReport = null;
  if (loading) {
    clearLastSelectionCard();
  } else {
    setLastSelectionCard(buildNodeSelectionCard(node, edges, context, card));
  }
  selectionTitle.textContent = node.label;
  const summaryRows = renderNodeSummaryRows(node);
  const metadataRows = renderNodeMetadataRows(node);
  const nodeIssues = (card?.insights || nodeInsightsForNode(node.id)).slice(0, 8);
  const sourceLines = card?.source?.lines || null;
  const sourcePath = card?.source?.path || node.span?.path || "";
  const sourceLineSuffix = node.span ? `:${node.span.start_line}` : "";
  const cardActions = nodeCardActions(card, node);
  const fileSummary = renderFileSummary(card?.file_summary);
  const dependencySummary = renderDependencySummary(card?.dependency_summary);
  const riskSummary = renderNodeRiskSummary(card?.insight_summary);
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
        <button type="button" data-copy-selection-link="node" data-selection-link-id="${node.id}">${escapeHtml(t("button.copyLink"))}</button>
        <button type="button" data-export-selection-card ${loading ? "disabled" : ""}>${escapeHtml(t("button.downloadCard"))}</button>
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
        ${cardActions
          .map(
            (action, index) =>
              `<button type="button" data-card-query-action="${index}">${escapeHtml(nodeCardActionLabel(action))}</button>`,
          )
          .join("")}
      </div>
      <section class="node-card-section">
        <h3>${escapeHtml(t("selection.summary"))}</h3>
        <dl class="node-summary">
          ${summaryRows.map(renderDefinitionRow).join("")}
        </dl>
        ${fileSummary}
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
        ${dependencySummary}
        <div class="neighbors">${neighborRows}</div>
      </section>
      <section class="node-card-section">
        <div class="node-card-section-header">
          <h3>${escapeHtml(t("selection.risks"))}</h3>
          <span>${Number(card?.total_insights ?? nodeIssues.length)}</span>
        </div>
        ${riskSummary}
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
          <button id="workflowButton" type="button">${escapeHtml(t("selection.flow"))}</button>
          <button id="dependentsButton" type="button">${escapeHtml(t("selection.dependents"))}</button>
        </div>
        <div class="workflow-filter-fields">
          <label class="field compact">
            <span>${escapeHtml(t("label.edge"))}</span>
            <input id="workflowEdgeKindInput" type="text" list="edgeKindOptions" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.confidence"))}</span>
            <input id="workflowConfidenceInput" type="text" list="confidenceOptions" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.language"))}</span>
            <input id="workflowLanguageInput" type="text" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.riskSeverity"))}</span>
            <input id="workflowRiskSeverityInput" type="text" list="severityOptions" autocomplete="off" />
          </label>
          <label class="field compact">
            <span>${escapeHtml(t("label.block"))}</span>
            <input id="workflowBlockKindInput" type="text" list="workflowBlockKindOptions" autocomplete="off" />
          </label>
        </div>
        <div class="workflow-export-actions">
          <button id="workflowJsonExportButton" type="button" disabled>${escapeHtml(t("button.downloadWorkflow"))}</button>
          <button id="workflowMermaidExportButton" type="button" disabled>${escapeHtml(t("button.downloadWorkflowMermaid"))}</button>
        </div>
        <div id="traceResult" class="trace-result"></div>
      </section>
      ${
        sourceLines || node.span
          ? `<section class="source-preview">
            <header>
              <span>${escapeHtml(t("selection.source"))}</span>
              <strong>${escapeHtml(`${sourcePath}${sourceLineSuffix}`)}</strong>
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
      selectNodeById(button.dataset.nodeId);
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

  selectionBody.querySelectorAll("[data-card-query-action]").forEach((button) => {
    const action = cardActions[Number(button.dataset.cardQueryAction)];
    if (action?.query) {
      button.addEventListener("click", () => runNodeCardQuery(action.query));
    }
  });

  const traceButton = document.querySelector("#traceButton");
  if (traceButton) {
    traceButton.addEventListener("click", () => loadTrace(node));
  }
  const workflowButton = document.querySelector("#workflowButton");
  if (workflowButton) {
    workflowButton.addEventListener("click", () => loadWorkflow(node));
  }
  for (const input of workflowFilterInputs("workflow")) {
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") loadWorkflow(node);
    });
  }
  renderWorkflowExportState();
  const workflowJsonExportButton = document.querySelector("#workflowJsonExportButton");
  if (workflowJsonExportButton) {
    workflowJsonExportButton.addEventListener("click", () => exportLastWorkflowReport("json"));
  }
  const workflowMermaidExportButton = document.querySelector("#workflowMermaidExportButton");
  if (workflowMermaidExportButton) {
    workflowMermaidExportButton.addEventListener("click", () => exportLastWorkflowReport("mermaid"));
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
  attachCopyLinkActions(selectionBody);
  attachSelectionCardExportAction();
}

function renderNodeSummaryRows(node) {
  const rows = [
    [t("selection.kind"), formatKind(node.kind)],
    [t("selection.id"), String(node.id)],
  ];
  if (node.metadata?.language) rows.push([t("label.language"), node.metadata.language]);
  if (node.metadata?.item_kind) rows.push([t("label.item"), formatKind(node.metadata.item_kind)]);
  if (node.kind === "file") rows.push([t("selection.path"), node.label]);
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
        ...workflowFilters,
      },
      report: body,
    };
    renderWorkflowExportState();
    target.innerHTML = renderWorkflow(body);
    attachWorkflowNavigation(target);
    attachEdgeExplainActions(target);
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
  if (jsonButton) jsonButton.disabled = !state.lastWorkflowReport;
  if (mermaidButton) mermaidButton.disabled = !state.lastWorkflowReport;
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
      `  ${workflowMermaidNodeId(node.id)}["${mermaidEscape(`${formatKind(block.kind || "unknown")}: ${node.label || node.id || ""}`)}"]`,
    );
  });
  transitions.forEach((transition) => {
    const edge = transition.edge || {};
    const source = transition.source_node_id || edge.source;
    const target = transition.target_node_id || edge.target;
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

function workflowMermaidNodeId(id) {
  return `B${String(id || "unknown").replace(/[^A-Za-z0-9_]/g, "_")}`;
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
  const nodeMap = new Map(orderedBlocks.map((block) => [block.node?.id, block.node]).filter(([id]) => id != null));
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
        ${riskSummary}
      </button>
      ${riskRows ? `<div class="workflow-risk-list">${riskRows}</div>` : ""}
    </li>
  `;
}

function renderWorkflowTransition(transition, nodeMap) {
  const edge = transition.edge || {};
  const source = nodeMap.get(transition.source_node_id) || nodeMap.get(edge.source);
  const target = nodeMap.get(transition.target_node_id) || nodeMap.get(edge.target);
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
    root: pathInput.value.trim() || ".",
    path: node.span.path,
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

function renderExplainEdgeButton(edge) {
  const edgeIndex = edgeIndexOf(edge);
  return `
    <button
      class="edge-explain-button"
      type="button"
      data-explain-edge
      ${edgeIndex == null ? "" : `data-edge-index="${edgeIndex}"`}
      data-edge-source="n${edge.source}"
      data-edge-target="n${edge.target}"
      data-edge-kind="${escapeHtml(edge.kind)}"
    >${escapeHtml(t("button.explain"))}</button>
  `;
}

function registerEdgeSelection(edge, source = null, target = null) {
  const selectionKey = edgeSelectionKey(edge);
  state.edgeSelectionCache.set(selectionKey, edge);
  if (source || target) {
    state.edgeSelectionNodeCache.set(selectionKey, { source, target });
  }
  return selectionKey;
}

function selectEdgeByKey(selectionKey, options = {}) {
  if (!selectionKey) return;
  const syncUrl = options.syncUrl !== false;
  state.selectedId = null;
  state.selectedEdgeKey = selectionKey;
  state.hoveredEdgeKey = null;
  if (syncUrl) syncSelectionUrl();
  renderSelection();
  draw();
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

  const params = new URLSearchParams({ path: pathInput.value.trim() || "." });
  if (button.dataset.edgeIndex) {
    params.set("edge_index", button.dataset.edgeIndex);
  } else {
    params.set("source", button.dataset.edgeSource || "");
    params.set("target", button.dataset.edgeTarget || "");
    params.set("kind", button.dataset.edgeKind || "");
  }

  try {
    const response = await apiFetch(`/api/explain-edge?${params.toString()}`);
    const body = await response.json();
    if (button.dataset.explainToken !== requestId) return;
    if (!response.ok) {
      throw new Error(apiErrorMessage(body, response, "edge explanation failed"));
    }
    target.innerHTML = renderEdgeExplanation(body);
    attachEdgeExplanationInsights(target, body);
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
  const insights = Array.isArray(explanation.insights) ? explanation.insights.slice(0, 8) : [];
  const riskSummary = renderNodeRiskSummary(explanation.insight_summary);
  const riskRows = insights
    .map((insight, index) => renderEdgeExplanationInsight(insight, index))
    .join("");
  const totalInsights = Number(explanation.total_insights || insights.length);
  const insightLimit = Number(explanation.insight_limit || insights.length);
  const riskLimitNote =
    explanation.truncated_insights && totalInsights > insightLimit
      ? `<p class="empty">${escapeHtml(formatReturnedCount(insights.length, totalInsights))} ${escapeHtml(t("selection.risks").toLowerCase())}</p>`
      : "";
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
    ${
      totalInsights > 0
        ? `<section class="edge-explanation-risks">
            <h4>${escapeHtml(t("selection.risks"))}</h4>
            ${riskSummary}
            <div class="node-issues">${riskRows}</div>
            ${riskLimitNote}
          </section>`
        : ""
    }
  `;
}

function renderEdgeExplanationInsight(insight, index) {
  const severity = insight.severity || "info";
  return `
    <button class="node-issue ${escapeHtml(severity)}" type="button" data-edge-insight-index="${index}">
      <span>${escapeHtml(formatKind(severity))} · ${escapeHtml(formatKind(insight.kind || "insight"))}</span>
      <strong>${escapeHtml(insight.message || "")}</strong>
      <em>${escapeHtml(t("selection.issueHint"))}</em>
    </button>
  `;
}

function attachEdgeExplanationInsights(container, explanation) {
  const insights = Array.isArray(explanation?.insights) ? explanation.insights : [];
  container.querySelectorAll("[data-edge-insight-index]").forEach((button) => {
    button.addEventListener("click", () => {
      const insight = insights[Number(button.dataset.edgeInsightIndex)];
      if (insight) focusInsight(insight);
    });
  });
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

function edgeIndexOf(edge) {
  const value = edge?.metadata?.edge_index;
  if (value == null || value === "") return null;
  const index = Number(value);
  return Number.isInteger(index) && index >= 0 ? index : null;
}

function edgeSelectionKey(edge) {
  const index = edgeIndexOf(edge);
  return index == null ? edgeKey(edge) : `edge:${index}`;
}

function centerViewportOnWorld(point) {
  state.pan.x = canvas.width / 2 - point.x * state.zoom;
  state.pan.y = canvas.height / 2 - point.y * state.zoom;
  draw();
}

function onPointerDown(event) {
  canvas.setPointerCapture(event.pointerId);
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  state.lastPointer = { x: event.offsetX, y: event.offsetY };
  if (hit) {
    selectNodeById(hit.id);
    state.draggingId = hit.id;
  } else {
    const edgeHit = findEdgeAt(world);
    if (edgeHit) {
      selectEdgeByKey(edgeSelectionKey(edgeHit));
    }
    state.draggingId = null;
  }
}

function onPointerMove(event) {
  const world = screenToWorld(event.offsetX, event.offsetY);
  const hit = findNodeAt(world);
  const edgeHit = hit ? null : findEdgeAt(world);
  const nextHoveredEdgeKey = edgeHit ? edgeSelectionKey(edgeHit) : null;
  const hoverChanged = state.hoveredEdgeKey !== nextHoveredEdgeKey || state.hoveredId !== (hit ? hit.id : null);
  state.hoveredId = hit ? hit.id : null;
  state.hoveredEdgeKey = nextHoveredEdgeKey;
  canvas.style.cursor = hit || edgeHit ? "pointer" : event.buttons === 1 ? "grabbing" : "";
  if (hoverChanged) draw();

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

function onPointerLeave() {
  onPointerUp();
  if (state.hoveredId != null || state.hoveredEdgeKey != null) {
    state.hoveredId = null;
    state.hoveredEdgeKey = null;
    draw();
  }
  canvas.style.cursor = "";
}

function recenterFromMinimapEvent(event) {
  const transform = minimapTransform();
  if (!transform) return;
  const world = minimapToWorld({ x: event.offsetX, y: event.offsetY }, transform);
  centerViewportOnWorld(world);
}

function onMinimapPointerDown(event) {
  event.preventDefault();
  event.stopPropagation();
  minimapCanvas.setPointerCapture(event.pointerId);
  recenterFromMinimapEvent(event);
}

function onMinimapPointerMove(event) {
  if (event.buttons !== 1) return;
  event.preventDefault();
  event.stopPropagation();
  recenterFromMinimapEvent(event);
}

function onMinimapPointerUp(event) {
  if (minimapCanvas.hasPointerCapture(event.pointerId)) {
    minimapCanvas.releasePointerCapture(event.pointerId);
  }
}

function onWheel(event) {
  event.preventDefault();
  const delta = event.deltaY > 0 ? 0.9 : 1.1;
  zoomAt(event.offsetX, event.offsetY, delta);
}

function onCanvasKeyDown(event) {
  if (state.graph.nodes.length === 0) return;
  const panStep = event.shiftKey ? 120 : 48;
  switch (event.key) {
    case "ArrowLeft":
      event.preventDefault();
      panGraphBy(panStep, 0);
      break;
    case "ArrowRight":
      event.preventDefault();
      panGraphBy(-panStep, 0);
      break;
    case "ArrowUp":
      event.preventDefault();
      panGraphBy(0, panStep);
      break;
    case "ArrowDown":
      event.preventDefault();
      panGraphBy(0, -panStep);
      break;
    case "+":
    case "=":
      event.preventDefault();
      zoomAtCanvasCenter(1.12);
      break;
    case "-":
    case "_":
      event.preventDefault();
      zoomAtCanvasCenter(0.88);
      break;
    case "Home":
      event.preventDefault();
      fitVisibleGraph();
      break;
    case "0":
      event.preventDefault();
      resetGraphLayout();
      break;
    case " ":
      event.preventDefault();
      toggleLayout();
      break;
    default:
      break;
  }
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

function findEdgeAt(point) {
  const tolerance = Math.max(7, 12 / Math.max(0.18, state.zoom));
  let best = null;
  let bestDistance = tolerance;
  for (const edge of state.visibleEdges) {
    const source = state.positions.get(edge.source);
    const target = state.positions.get(edge.target);
    if (!source || !target) continue;
    const distance = distanceToSegment(point, source, target);
    if (distance <= bestDistance) {
      best = edge;
      bestDistance = distance;
    }
  }
  return best;
}

function distanceToSegment(point, start, end) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSq = dx * dx + dy * dy;
  if (lengthSq === 0) {
    const px = point.x - start.x;
    const py = point.y - start.y;
    return Math.sqrt(px * px + py * py);
  }
  const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSq));
  const x = start.x + t * dx;
  const y = start.y + t * dy;
  const px = point.x - x;
  const py = point.y - y;
  return Math.sqrt(px * px + py * py);
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

function riskColor(severity) {
  switch (severity) {
    case "error":
      return "rgba(224, 108, 117, 0.95)";
    case "warning":
      return "rgba(242, 193, 78, 0.95)";
    case "info":
      return "rgba(92, 200, 167, 0.82)";
    default:
      return "rgba(237, 241, 242, 0.72)";
  }
}

function nodeIsFocused(node) {
  return Boolean(state.queryFocus?.nodeIds?.has(node.id));
}

function graphNeighborhoodContext(visibleIds) {
  const anchorId = state.selectedId ?? state.hoveredId;
  if (anchorId == null || !visibleIds.has(anchorId)) return null;
  const nodeIds = new Set([anchorId]);
  const edgeKeys = new Set();
  state.visibleEdges.forEach((edge) => {
    if (!visibleIds.has(edge.source) || !visibleIds.has(edge.target)) return;
    if (!edgeTouchesNode(edge, anchorId)) return;
    nodeIds.add(edge.source);
    nodeIds.add(edge.target);
    edgeKeys.add(edgeKey(edge));
  });
  return {
    anchorId,
    nodeIds,
    edgeKeys,
    mode: state.selectedId != null ? "selected" : "hover",
  };
}

function nodeIsNeighborhoodNeighbor(node, neighborhood) {
  return Boolean(
    neighborhood &&
      node.id !== neighborhood.anchorId &&
      neighborhood.nodeIds.has(node.id),
  );
}

function nodeIsNeighborhoodMuted(node, neighborhood, selected, hovered, focused) {
  return Boolean(
    neighborhood &&
      !selected &&
      !hovered &&
      !focused &&
      !neighborhood.nodeIds.has(node.id),
  );
}

function edgeNeighborhoodAlpha(edge, neighborhood) {
  if (!neighborhood) return 1;
  return neighborhood.edgeKeys.has(edgeKey(edge)) ? 1 : 0.28;
}

function edgeEmphasis(edge) {
  if (edgeSelectionKey(edge) === state.selectedEdgeKey) return "selected";
  if (state.queryFocus?.edgeKeys?.has(edgeKey(edge))) return "focus";
  if (edgeSelectionKey(edge) === state.hoveredEdgeKey) return "hover";
  if (state.selectedId != null && edgeTouchesNode(edge, state.selectedId)) return "selected-node";
  if (state.hoveredId != null && edgeTouchesNode(edge, state.hoveredId)) return "hover-node";
  return "normal";
}

function edgeTouchesNode(edge, nodeId) {
  return edge.source === nodeId || edge.target === nodeId;
}

function edgeBackplateWidth(emphasis) {
  if (emphasis === "hover-node") return 4.2;
  if (emphasis === "selected-node") return 5;
  return emphasis === "hover" ? 5 : 6;
}

function edgeStrokeWidth(emphasis) {
  if (emphasis === "normal") return 1;
  if (emphasis === "hover-node") return 1.8;
  if (emphasis === "selected-node") return 2.4;
  if (emphasis === "hover") return 2.4;
  return 3.2;
}

function edgeHighlightColor(emphasis) {
  if (emphasis === "selected") return "rgba(92, 200, 167, 0.98)";
  if (emphasis === "hover") return "rgba(237, 241, 242, 0.92)";
  if (emphasis === "selected-node") return "rgba(92, 200, 167, 0.86)";
  if (emphasis === "hover-node") return "rgba(237, 241, 242, 0.72)";
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

function formatCompactNumber(value) {
  const number = Number(value || 0);
  if (!Number.isFinite(number)) return "0";
  return new Intl.NumberFormat(state.locale, {
    notation: Math.abs(number) >= 10_000 ? "compact" : "standard",
    maximumFractionDigits: 1,
  }).format(number);
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
