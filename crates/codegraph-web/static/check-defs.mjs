// Static guard for the concatenated web bundle: fail if a bare-identifier
// helper is CALLED but never DEFINED anywhere in static/js/*.js.
//
// `node --check` only validates syntax, so a call to a helper that does not
// exist (e.g. a rename that missed call sites, or a lost definition during a
// module split) passes the syntax gate yet throws ReferenceError at runtime on
// the first code path that reaches it. This check catches that class of bug
// without a browser by comparing defined names against called identifiers.
//
// Usage: node crates/codegraph-web/static/check-defs.mjs
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = join(dirname(fileURLToPath(import.meta.url)), "js");
const files = readdirSync(dir)
  .filter((name) => name.endsWith(".js") && !name.endsWith(".test.js"))
  .sort();
const raw = files.map((name) => readFileSync(join(dir, name), "utf8")).join("\n");

// Strip block comments and single-line quoted strings so their contents (e.g. a
// CSS `rgba(` literal) are not mistaken for calls. The string patterns forbid
// newlines so they never span a multi-line template literal (whose embedded
// HTML quotes would otherwise swallow real code); template literals are kept so
// real calls inside `${...}` interpolations are still checked.
const source = raw
  .replace(/\/\*[\s\S]*?\*\//g, " ")
  .replace(/(^|[^:"'`])\/\/[^\n]*/g, "$1")
  .replace(/'(?:[^'\\\n]|\\.)*'/g, "''")
  .replace(/"(?:[^"\\\n]|\\.)*"/g, '""');

const addNames = (text) => {
  for (const part of text.split(",")) {
    // Strip defaults, destructuring braces, and rest/spread.
    const name = part
      .replace(/[[\]{}]/g, " ")
      .split(/[:=]/)[0]
      .replace(/\.\.\./, "")
      .trim();
    if (/^[A-Za-z_$][\w$]*$/.test(name)) defined.add(name);
  }
};

// Names defined in the bundle: function declarations, bindings, and parameters.
const defined = new Set();
for (const match of source.matchAll(/\bfunction\s+([A-Za-z_$][\w$]*)/g)) {
  defined.add(match[1]);
}
for (const match of source.matchAll(/\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=/g)) {
  defined.add(match[1]);
}
for (const match of source.matchAll(/\b(?:const|let|var)\s*[[{]([^=;]+?)[\]}]\s*=/g)) {
  addNames(match[1]);
}
// Function parameters: `function name(params)` and `(params) =>` and `catch (e)`.
for (const match of source.matchAll(/\bfunction\s*[A-Za-z_$\w$]*\s*\(([^)]*)\)/g)) {
  addNames(match[1]);
}
for (const match of source.matchAll(/\(([^()]*)\)\s*=>/g)) {
  addNames(match[1]);
}
for (const match of source.matchAll(/\bcatch\s*\(([^)]*)\)/g)) {
  addNames(match[1]);
}
// Single-identifier arrow params without parentheses: `id => ...`.
for (const match of source.matchAll(/(^|[^.\w$])([A-Za-z_$][\w$]*)\s*=>/g)) {
  defined.add(match[2]);
}
// Object-method and shorthand: `name(params) {` inside object/class bodies.
for (const match of source.matchAll(/([A-Za-z_$][\w$]*)\s*\(([^)]*)\)\s*\{/g)) {
  addNames(match[2]);
}

// Language and browser globals the bundle legitimately calls.
const globals = new Set([
  "Number", "String", "Boolean", "Array", "Object", "Math", "JSON", "Date",
  "Set", "Map", "WeakMap", "WeakSet", "Promise", "RegExp", "Error", "Symbol",
  "Intl", "URL", "URLSearchParams", "Blob", "FormData", "Headers", "Request",
  "parseInt", "parseFloat", "isNaN", "isFinite", "encodeURIComponent",
  "decodeURIComponent", "encodeURI", "decodeURI", "structuredClone", "atob", "btoa",
  "setTimeout", "clearTimeout", "setInterval", "clearInterval",
  "requestAnimationFrame", "cancelAnimationFrame", "queueMicrotask",
  "fetch", "alert", "confirm", "prompt", "print",
  "document", "window", "console", "navigator", "location", "history",
  "localStorage", "sessionStorage", "EventSource", "WebSocket", "AbortController",
  "getComputedStyle", "matchMedia", "CustomEvent", "Event", "Image",
  "if", "for", "while", "switch", "catch", "return", "function", "super",
  "await", "typeof", "new", "delete", "void", "in", "of", "do", "else",
  "async", "yield", "case", "throw", "try", "finally", "instanceof",
]);

// Called bare identifiers: `name(` not preceded by `.` (method call) or a word
// char (part of a longer identifier) or `function ` (a declaration).
const calls = new Set();
for (const match of source.matchAll(/(^|[^.\w$])([A-Za-z_$][\w$]*)\s*\(/g)) {
  const before = source.slice(Math.max(0, match.index - 9), match.index + match[1].length);
  if (/\bfunction\s*$/.test(before)) continue; // function declaration/expression
  calls.add(match[2]);
}

const missing = [...calls]
  .filter((name) => !defined.has(name) && !globals.has(name))
  .sort();

if (missing.length > 0) {
  console.error(
    `check-defs: ${missing.length} identifier(s) called but never defined in the bundle:`,
  );
  for (const name of missing) console.error(`  - ${name}`);
  console.error(
    "Define the helper (or add a genuine global to the allowlist in check-defs.mjs).",
  );
  process.exit(1);
}
// The browser recomputes the CLI's findings on the graph it holds, and a
// list it keeps its own copy of drifts silently: the Python standard
// library set held 41 names against the CLI's 193, so the view called
// `ast` and `code` dependencies flask forgot to declare.
const insightsRs = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "codegraph-analysis",
  "src",
  "insights.rs",
);
const rustSource = readFileSync(insightsRs, "utf8");
const namesIn = (text) => new Set([...text.matchAll(/"([A-Za-z_][A-Za-z_0-9.\\-]*)"/g)].map((m) => m[1]));
const rustNamesOf = (signature) => {
  const start = rustSource.indexOf(signature);
  if (start < 0) throw new Error(`check-defs: ${signature} is gone from insights.rs`);
  return namesIn(rustSource.slice(start, rustSource.indexOf("\n}", start)));
};
const jsNamesOf = (constant) => {
  const block = raw.match(new RegExp(`const ${constant} = new Set\\(\\[([\\s\\S]*?)\\]\\);`));
  if (!block) throw new Error(`check-defs: ${constant} is gone from the bundle`);
  return namesIn(block[1]);
};

const SHARED_LISTS = [
  ["pythonStdlibPackages", "fn is_python_stdlib_package"],
  ["nodeBuiltinModules", "fn is_node_builtin_module"],
  ["phpNonComposerNamespaceRoots", "fn is_php_non_composer_namespace_root"],
];
let sharedNames = 0;
for (const [constant, signature] of SHARED_LISTS) {
  const rustNames = rustNamesOf(signature);
  const jsNames = jsNamesOf(constant);
  const onlyRust = [...rustNames].filter((name) => !jsNames.has(name));
  const onlyJs = [...jsNames].filter((name) => !rustNames.has(name));
  if (onlyRust.length > 0 || onlyJs.length > 0) {
    console.error(`check-defs: ${constant} and ${signature} disagree`);
    if (onlyRust.length > 0) console.error(`  only in insights.rs: ${onlyRust.join(", ")}`);
    if (onlyJs.length > 0) console.error(`  only in the bundle: ${onlyJs.join(", ")}`);
    process.exit(1);
  }
  sharedNames += rustNames.size;
}

// Every finding the analysis can emit is rendered by name in the view. A
// kind with no Russian name shows English words in the Russian UI, and 30
// of the 52 did before anyone compared the two lists.
const limitsRs = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "codegraph-analysis",
  "src",
  "limits.rs",
);
const knownKinds = readFileSync(limitsRs, "utf8").match(
  /pub const KNOWN_INSIGHT_KINDS: &\[&str\] = &\[([\s\S]*?)\];/,
);
if (!knownKinds) throw new Error("check-defs: KNOWN_INSIGHT_KINDS is gone from limits.rs");
const kinds = [...knownKinds[1].matchAll(/"([a-z_*]+)"/g)]
  .map((match) => match[1])
  .filter((kind) => !kind.includes("*"));
const i18n = readFileSync(join(dir, "02-i18n-data.js"), "utf8");
const russian = i18n.slice(i18n.search(/\bru:\s*\{/));
const russianKinds = new Set(
  [...russian.matchAll(/"kind\.([a-z_]+)":/g)].map((match) => match[1]),
);
// The graph's own vocabulary is rendered the same way, so it needs the
// same names: `control_flow`, `contains`, `defines` and `depends_on` were
// shown in English.
const coreRs = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "codegraph-core",
  "src",
  "lib.rs",
);
const core = readFileSync(coreRs, "utf8");
const enumVariants = (name) => {
  const block = core.match(new RegExp(`pub enum ${name}\\s*\\{([\\s\\S]*?)\\n\\}`));
  if (!block) throw new Error(`check-defs: ${name} is gone from codegraph-core`);
  return [...block[1].matchAll(/\n {4}([A-Z][A-Za-z]*)/g)].map((match) =>
    match[1].replace(/(?<!^)([A-Z])/g, "_$1").toLowerCase(),
  );
};
// The entrypoint kinds the schema publishes are rendered the same way.
const schemaRs = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "..", "..", "codegraph-server", "src", "schema.rs"),
  "utf8",
);
// Values the view renders as labels rather than terms a reader types.
const RENDERED_ENUMS = [
  "entrypoint_kind",
  "workflow_block_kind",
  "cache_status",
  "graph_confidence",
  "risk_grade",
  "semantic_work_status",
  "semantic_work_capability",
];
const enumValues = (name) => {
  const block = schemaRs.match(new RegExp(`"${name}",\\s*vec!\\[([\\s\\S]*?)\\]`));
  if (!block) throw new Error(`check-defs: the ${name} enum is gone from schema.rs`);
  return [...block[1].matchAll(/"([a-z_]+)"/g)].map((match) => match[1]);
};
const vocabulary = [
  ...enumVariants("NodeKind"),
  ...enumVariants("EdgeKind"),
  ...RENDERED_ENUMS.flatMap(enumValues),
];
const untranslated = [...kinds, ...vocabulary].filter((kind) => !russianKinds.has(kind));
if (untranslated.length > 0) {
  console.error(
    `check-defs: ${untranslated.length} kind(s) have no Russian name: ${untranslated.join(", ")}`,
  );
  process.exit(1);
}

console.log(
  `check-defs: ok (${defined.size} defs, ${calls.size} called names, 0 undefined, ${sharedNames} shared names in step, ${kinds.length + vocabulary.length} kinds named)`,
);
