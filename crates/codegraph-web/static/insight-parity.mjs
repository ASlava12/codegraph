// The browser recomputes a handful of the CLI's findings itself when no
// report has been sent, and those copies drift: flask's ten client warnings
// against the CLI's one warning and eight notes went unnoticed until someone
// drove both over a real graph. `views-smoke.mjs` pins the rules on
// fixtures; this drives them over a scanned project and compares what each
// side counts.
//
//   codegraph scan . --no-semantic > graph.json
//   codegraph insights . --no-semantic --limit 1 > by-kind.json
//   codegraph insights . --no-semantic --severity warning --limit 500 > warnings.json
//   node insight-parity.mjs graph.json by-kind.json warnings.json
import { readFileSync } from "node:fs";
import { loadBundle } from "./smoke-harness.mjs";

const [graphPath, byKindPath, warningsPath] = process.argv.slice(2);
if (!graphPath || !byKindPath || !warningsPath) {
  console.error("usage: insight-parity.mjs <graph.json> <by-kind.json> <warnings.json>");
  process.exit(2);
}

const api = loadBundle("__parity", ["buildClientInsights"]);
const graph = JSON.parse(readFileSync(graphPath, "utf8"));
const byKind = JSON.parse(readFileSync(byKindPath, "utf8")).by_kind || {};
const warningReport = JSON.parse(readFileSync(warningsPath, "utf8"));
const warnings = warningReport.insights || [];

const client = api.buildClientInsights(graph);
const clientTotals = new Map();
const clientWarnings = new Map();
for (const insight of client) {
  clientTotals.set(insight.kind, (clientTotals.get(insight.kind) || 0) + 1);
  if (insight.severity === "warning") {
    clientWarnings.set(insight.kind, (clientWarnings.get(insight.kind) || 0) + 1);
  }
}
const cliWarnings = new Map();
for (const insight of warnings) {
  cliWarnings.set(insight.kind, (cliWarnings.get(insight.kind) || 0) + 1);
}

// `total` counts every warning the CLI found, `insights` holds as many as
// the limit allowed. Severities are only comparable when the two agree.
const warningsComplete = warnings.length === (warningReport.total ?? warnings.length);
const problems = [];
for (const [kind, total] of [...clientTotals].sort()) {
  const cliTotal = byKind[kind] || 0;
  if (total !== cliTotal) problems.push(`${kind}: browser found ${total}, CLI ${cliTotal}`);
  if (!warningsComplete) continue;
  const mine = clientWarnings.get(kind) || 0;
  const theirs = cliWarnings.get(kind) || 0;
  if (mine !== theirs) {
    problems.push(`${kind}: browser calls ${mine} of them warnings, CLI ${theirs}`);
  }
}

if (problems.length > 0) {
  console.error("insight-parity: the browser and the CLI disagree");
  for (const problem of problems) console.error(`  ${problem}`);
  process.exit(1);
}
console.log(
  `insight-parity: ok (${clientTotals.size} kinds, ${client.length} findings${
    warningsComplete ? ", severities compared" : ", severities skipped: listing truncated"
  })`,
);
