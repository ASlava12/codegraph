// 05-locale.js — translation helpers, locale/label-mode switching, panel hints.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

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
  renderPanelHints();
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

function renderPanelHints() {
  const hints = [
    [queryResult, t("hint.query")],
    [journeyResult, t("hint.journey")],
    [sourceSearchResult, t("hint.sourceSearch")],
    [prImpactResult, t("hint.prImpact")],
    [refactorResult, t("hint.refactor")],
    [cacheDiffResult, t("hint.cache")],
  ];
  hints.forEach(([container, text]) => {
    if (
      container &&
      (!container.innerHTML.trim() || container.querySelector(".panel-hint"))
    ) {
      container.innerHTML = `<p class="empty panel-hint">${escapeHtml(text)}</p>`;
    }
  });
}
