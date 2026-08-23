// Shared scaffolding for the browser-less smoke tests.
//
// `node --check` validates syntax and `check-defs.mjs` catches calls to helpers
// that are never defined, but neither exercises the code: a runtime error in a
// render path (a bad canvas call, a null deref, a logic slip) still ships. This
// loads the real concatenated bundle under DOM/canvas stubs and hands back the
// sandbox, so each smoke test only has to describe what it drives.
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

/// Load the bundle and expose the named globals under `globalThis.<key>`.
export function loadBundle(exportName, names) {
  const dir = dirname(fileURLToPath(import.meta.url));
  const jsDir = join(dir, "js");
  let source = readFileSync(join(dir, "label-policy.js"), "utf8") + "\n";
  source += readdirSync(jsDir)
    .filter((name) => name.endsWith(".js") && !name.endsWith(".test.js"))
    .sort()
    .map((name) => readFileSync(join(jsDir, name), "utf8"))
    .join("\n");
  source += `\n;globalThis.${exportName} = { ${names.join(", ")} };\n`;

  const ctx = new Proxy(
    {
      canvas: { width: 1400, height: 800 },
      measureText: () => ({ width: 10 }),
      createLinearGradient: () => ({ addColorStop() {} }),
      setLineDash() {},
    },
    { get: (t, p) => (p in t ? t[p] : () => {}), set: () => true },
  );
  const el = () => {
    const node = {
      width: 1400,
      height: 800,
      clientWidth: 1400,
      clientHeight: 800,
      textContent: "",
      innerHTML: "",
      value: "",
      checked: false,
      hidden: false,
      dataset: {},
      style: {},
      files: [],
      classList: { add() {}, remove() {}, toggle() {}, contains: () => false },
      addEventListener() {},
      removeEventListener() {},
      append() {},
      appendChild() {},
      remove() {},
      setAttribute() {},
      removeAttribute() {},
      getAttribute: () => null,
      insertAdjacentHTML() {},
      scrollIntoView() {},
      focus() {},
      closest: () => el(),
      matches: () => false,
      cloneNode: () => el(),
      querySelector: () => el(),
      querySelectorAll: () => [],
      getBoundingClientRect: () => ({ width: 1400, height: 800, left: 0, top: 0, right: 1400, bottom: 800 }),
      getContext: () => ctx,
    };
    return node;
  };
  const cache = new Map();
  const cached = (key) => (cache.has(key) ? cache.get(key) : (cache.set(key, el()), cache.get(key)));
  const storage = { getItem: () => null, setItem() {}, removeItem() {}, clear() {} };
  const documentStub = {
    documentElement: el(),
    body: el(),
    head: el(),
    querySelector: (selector) => cached(`q:${selector}`),
    querySelectorAll: () => [],
    getElementById: (id) => cached(`#${id}`),
    createElement: () => el(),
    createElementNS: () => el(),
    createDocumentFragment: () => el(),
    addEventListener() {},
  };
  const sandbox = {
    document: documentStub,
    console,
    addEventListener() {},
    removeEventListener() {},
    localStorage: storage,
    sessionStorage: storage,
    navigator: { language: "en", clipboard: { writeText: () => Promise.resolve() } },
    location: { href: "http://localhost/", search: "", pathname: "/", hash: "" },
    history: { replaceState() {}, pushState() {} },
    matchMedia: () => ({ matches: false, addEventListener() {}, addListener() {} }),
    getComputedStyle: () => ({ getPropertyValue: () => "" }),
    requestAnimationFrame: () => 0,
    cancelAnimationFrame() {},
    setTimeout: () => 0,
    clearTimeout() {},
    setInterval: () => 0,
    clearInterval() {},
    fetch: () => new Promise(() => {}),
    EventSource: class {
      close() {}
      addEventListener() {}
    },
    Headers,
    URL,
    URLSearchParams,
    Intl,
    structuredClone: (value) => JSON.parse(JSON.stringify(value)),
  };
  sandbox.window = sandbox;
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);
  vm.runInContext(source, sandbox);
  return sandbox[exportName];
}
