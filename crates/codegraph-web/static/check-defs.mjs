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
console.log(`check-defs: ok (${defined.size} defs, ${calls.size} called names, 0 undefined)`);
