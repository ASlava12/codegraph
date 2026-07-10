// 01-constants.js — top-level configuration constants and storage keys.
// Concatenated into app.js by codegraph-server's build script;
// file order (lexicographic) preserves the original single-file order.

const DEFAULT_LOCALE = "en";
const DEFAULT_LABEL_MODE = "minimal";
const LABEL_MODES = new Set(["minimal", "hover", "focus", "auto"]);
const LABEL_MODE_STORAGE_KEY = "codegraph.labelMode";
const LABEL_MODE_STORAGE_VERSION_KEY = "codegraph.labelModeVersion";
const LABEL_MODE_STORAGE_VERSION = "17";
const API_TOKEN_STORAGE_KEY = "codegraph.apiToken";
const QUERY_HISTORY_STORAGE_KEY = "codegraph.queryHistory";
const QUERY_HISTORY_LIMIT = 8;
