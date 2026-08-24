# CodeGraph

CodeGraph is a Rust-based code exploration tool for people and agents.

The project goal is to turn source code into a typed knowledge graph that can be inspected from a graphical UI, queried from a CLI, and consumed as structured JSON by automation.

## Current Status

This repository is an active production-oriented prototype.

Implemented now:

- Rust workspace layout.
- Core graph model.
- Stable graph confidence taxonomy for `exact`, `semantic`, `syntactic`, `heuristic`, and `unknown` facts with JSON round-trip coverage.
- Built-in language adapter registry for Rust, Python, JavaScript, TypeScript/TSX, Go, C, C++, Dart, PHP, Bash, Ruby, Java, C#, Kotlin, Swift, Scala, Lua, Elixir, Zig, Haskell, OCaml, Julia, Erlang, Nix, R, HCL (Terraform), Protobuf, GraphQL, Solidity, and Objective-C parser support.
- LSP server discovery for semantic enrichment readiness across Rust, Go, JavaScript/TypeScript, Python, C/C++, PHP, and Bash.
- Semantic enrichment server contracts cover `rust-analyzer`, `gopls`, and `typescript-language-server --stdio` for primary Rust, Go, JavaScript, TypeScript, and TSX workflows.
- Project semantic readiness reports showing which scanned languages are covered by installed LSP servers.
- Semantic enrichment plans showing ready, blocked, and unsupported LSP work by language, including capped concrete work queues with stable ids and priorities for agents.
- Semantic execution batch reports that group filtered LSP work by language server command and include executable LSP request descriptors for semantic runners.
- CLI semantic runner that executes ready LSP batches over stdio and emits reusable response JSON for graph patching or enrichment.
- HTTP/API and web semantic enrichment action that can run ready LSP batches and render an enriched graph in the browser.
- Async semantic enrichment job API with status, SSE events, and result retrieval for long-running LSP work.
- Bounded in-memory scan and semantic job retention with job-store counters in the health API.
- Configurable scan and semantic job concurrency limits with active/available counters in the health API.
- Semantic LSP response patch reports that map definitions, references, and diagnostics back onto graph nodes.
- Semantic graph patch application that emits enriched graphs with semantic edges and diagnostic nodes.
- Filesystem scanner with default build/vendor ignore rules.
- Automatic semantic enrichment: when a language server for a scanned language is installed, the scan asks it to resolve what syntax cannot and applies the result as `confidence: semantic` edges over the syntactic graph. The root node records the outcome either way (`semantic_enrichment=applied` with `semantic_servers`, or `semantic_enrichment=skipped` with `semantic_skip_reason`), a missing or failing server degrades to the syntactic graph instead of failing the scan, and `--no-semantic` restores a machine-independent scan (use it in CI).
- A `.properties` file states settings the program reads by name: each key becomes the configuration node the code reads, and a `${placeholder}` in a value is that file reading another setting (its default after the colon is not part of the name). Resource bundles — a program's words, one file per locale — are left out.
- A Terraform configuration's `required_providers` are dependencies like any manifest's, named the way their `source` names them; 212 of terraform's own `.tf` files hold nothing else.
- Jupyter notebooks (`.ipynb`) are read as the program their code cells hold: every fact points at the line of the notebook that holds it, and IPython's own lines (`%matplotlib inline`, `!pip install`) are left out — with them in, 69 of pytudes' 113 notebooks fail to parse, and without them none does.
- Tree-sitter based syntax extraction for Rust, Python, JavaScript, TypeScript, TSX, Go, C, C++, Dart, PHP, Bash, Ruby, Java, C#, Kotlin, Swift, Scala, Lua, Elixir, Zig, Haskell, OCaml, Julia, Erlang, Nix, R, HCL (Terraform, Packer, Nomad), Protobuf, GraphQL, Solidity, and Objective-C.
- Files are read into facts on every core, one round ahead of the walk that assembles the graph, so reading and assembling overlap; the graph is assembled in one order and is byte-identical run to run. Terraform's 49 MB first scan takes 3.6s and 0.58 GB on an 18-core machine, and 0.4s from a warm cache.
- Function, type/class, module/namespace, import/include, and entrypoint candidate nodes.
- What a definition lets others see, recorded as `visibility` wherever the language states it: a
  keyword (`pub`, `static`, `local`, `private`, `defp`, Solidity's `external`/`public`/`internal`/
  `private`), a name (`_helper` in Python and Dart, a capital in Go), an export list at the top of an
  Erlang or Haskell module, or the `.mli` beside an OCaml one. A library's coverage finding counts what it exports as starting points, an uncalled
  function says whether it is dead or the API, and a call from another file is not answered by a
  definition that file cannot name.
- Every call and every fact belongs to the definition whose body contains it. One file can write
  one name several times — Go declares `String()` once per type, and Python writes an overload stub
  above the implementation — so the line decides, not the name.
- Manifest-defined entrypoints from Cargo, npm, Go, Python, setup.py/setup.cfg, Composer, and CMake project metadata.
- Shebang-defined script entrypoints for Bash, Python, Node.js, and PHP scripts, including extensionless CLI files.
- Dockerfile, Docker Compose, GitHub Actions, GitLab CI, and Kubernetes runtime entrypoints, including Compose service dependencies, workflow/pipeline job dependencies, CI environment inputs with secret-safe value classification, CI job-reference and script-path checks, runtime config inputs, published ports, local bind volumes, Kubernetes workloads, Ingresses, services, Service selector links, and ConfigMap/Secret references.
- Framework route entrypoints for common Python, JavaScript/TypeScript, Rust, Go, PHP, Ruby, Java, Kotlin and C# web route declarations — ASP.NET attributes with `[controller]`/`[action]` filled in the way the framework fills them, Spring's `@GetMapping`/`@RequestMapping` (with the class's own prefix joined onto each method's path) and JAX-RS `@Path`, told apart from Retrofit's parameter `@Path("id")` by the leading slash a path has — Rails' `config/routes.rb` (a `resources :users` is the seven routes Rails generates for it, less what `only:`/`except:` take back and under the segment `path:` renames it to, a `namespace` prefixes what it holds, a nested `resources` block lives under its parent's member path (`/users/:account_username/statuses/:id`), a `concern` states routes to be mounted elsewhere rather than routes of its own, and `to: "health#show"` names the action on `HealthController`, which settles which of mastodon's 139 `show` methods it means), including Django's URLconf (`path`, `re_path`, `url`, with the pattern of a multi-line `re_path` read from the line below). A Django application mounts each URLconf under a prefix of its own, so the same written path in two applications is two URLs rather than a collision. Laravel's `Route::` file is read the same way: a group hands its prefix to what it holds, `[SongController::class, 'update']` and the invokable `SongController::class` both name the method that serves the route, and `apiResource` declares the set the framework expands it into (with `->only(..)`/`->except(..)` taken into account). Which class the route names settles which method it means, and the file's own imports settle which class -- koel has an API and a Subsonic `ScrobbleController`, and one of them is imported there.
- Rust/Axum route entrypoints handle multiline `.route(...)` calls and ignore string literal route markers.
- Resolved manifest entrypoint targets for common file paths, command paths, CMake executables, and Python module callables.
- Approximate `calls` edges between functions when syntax-level names can be resolved. Each edge records
  what settled it in `resolution_basis` — the file it sits in, the import that named the module, the
  package, the file a module is named after, the scope it is written in, the module's exports, the
  receiver's declared type, the type that owns the method, a set of overloads, or the name alone — and
  carries `syntactic` confidence for everything but the last. A call that sits outside
  any definition — a module initialiser, `bp.route(...)`, `if __name__ == "__main__": main()` — is attributed
  to the file that runs it; a call inside an unnamed callback is not, because it runs on invocation rather
  than on load.
- What a package publishes, read from npm's `files` field and kept as `published_paths` on the
  manifest: code a package states it does not ship cannot turn a dev dependency into a runtime one,
  and a dead import there reads as a note. The nearest manifest answers for a file, and a package
  that publishes only a build product the scan never held (`files: ["dist"]` over sources in `src/`)
  says nothing about its sources.
- Objective-C resolves through the frameworks it calls: NSObject's universal methods and the XCTest
  API by name, a bare function whose prefix belongs to a framework (`dispatch_`, `objc_`, a
  capitalised `NS`/`CF`/`CG`/`Sec`) as that framework's, and a message whose receiver names a
  framework class (`[NSURL URLWithString:]`) as Foundation's rather than as a selector nothing
  declares.
- The browser's own copy of the undeclared-import rule reads a package the way the CLI does: every
  spelling a PHP namespace could name (`Doctrine\CouchDB` is doctrine/couchdb as readily as
  doctrine/couch-db), the namespaces a composer lockfile says a package autoloads, the library a
  declared name already covers (`aws/aws-sdk-php` publishes `Aws\`), and a standard module written
  in mixed case (`cProfile`). Driving both sides over twelve scanned projects had the browser
  reporting 52 of koel's framework imports, 27 of monolog's optional handlers and one of pytudes'
  notebooks as undeclared where the CLI reported one, none and none.
- A syntax-error finding names the line the parser lost the thread on, which is what tells a broken
  file from syntax a grammar does not cover: koel's specs are valid TypeScript that
  tree-sitter-typescript cannot read (`await original<typeof import('./helpers')>()`), and dune's C
  stubs start with an OCaml macro where a return type belongs.
- A Makefile is read as make, not as shell. Its recipes are shell, and the file around them is not:
  reading the whole file with a shell grammar made terraform's Makefile a file that calls
  `protobuf:`, `.PHONY:` and `CURDIR`, and put a syntax error on every Makefile in the corpus. The
  targets and the commands they run are read by the makefile detector, which is what states them.
- A Dockerfile's command is resolved against the build context as well as the directory the
  Dockerfile sits in: mastodon keeps `streaming/Dockerfile` and runs `node ./streaming/index.js`
  from its `WORKDIR`, which is the repository, not `streaming/streaming/index.js`.
- A compose `env_file` the repository's own `.gitignore` keeps out is a note rather than a warning:
  `.env.production` is written by whoever deploys, from the `.sample` the project ships beside it.
- A ruby route is a declaration, not a call: sinatra states the block that serves the route on the
  line that opens it (`get '/x' do`, `get('/x') {`), so a request spec's `post '/accounts', params:
  { id: 1 }` is a call to a route rather than one of the program's own. 148 of mastodon's specs read
  as routes it serves, and 51 of sinatra's own test fixtures did.
- Rails routes are read the way Rails reads them: a `collection do` block holds the set's routes and
  takes back the id the enclosing `resources` hands down (`POST /notifications/requests/accept`, not
  `/requests/:request_id/accept`), a controller path states the modules its class sits in
  (`auth/registrations#new` is `Auth::RegistrationsController#new`, which mastodon declares beside
  `Admin::Fasp::RegistrationsController`), a route matches a class whose acronyms are capitalised the
  project's way (`oembed#show` is `Api::OEmbedController#show`), and a route that states
  `constraints:` shares its path by design rather than colliding with the route beneath it.
- A qualified call's import qualifier is read from the front of the name, not from the segment
  before the method: `protoimpl.X.MessageStateOf` comes from `protoimpl`, so terraform's generated
  protobuf code no longer reports 3234 calls as unresolved, and `providers.SchemaCache.Set` reaches
  the in-repo package that declares it. Go's predeclared types count as the language's own when they
  are written as conversions -- `string(b)`, `int64(n)` -- rather than as functions nothing declares.
- Ruby resolves through the constant a call is written through. A ruby call's label keeps only the
  method name, so `Addressable::URI.parse(href).normalize` and a project's own
  `HashtagNormalizer#normalize` looked like one call; the receiver's leading constant says which is
  meant, and one the project never declares belongs to a gem. On mastodon that is 145 methods no
  longer called by code that never calls them (`FastImage.size` answered by a connection pool's
  `size`, `Chewy::Stash::Specification.reset!` by a delivery tracker's), each recorded as a call that
  leaves the project rather than as one the resolver failed on.
- Single-file components are read as the programs they hold: a `.vue` or `.svelte` file states a
  template, a script and a style together, and the `<script>` block (TypeScript when it says `lang="ts"`)
  is parsed with every other line blanked, so a fact keeps the line of the component that holds it.
  koel's 337 components contribute 723 functions and 2460 imports that nothing held before.
- `//go:generate` says how a package's code is produced, and the directive reaches the program it
  names: gqlgen writes 58 of them and terraform 42, and 49 of gqlgen's resolve to
  `testdata/gqlgen.go`.
- A Rails schema declares tables: `db/schema.rb` is the dump Rails keeps of the database it built,
  and mastodon's states 127 of them.
- A Laravel migration declares tables: `Schema::create('songs', ..)` is the schema koel states, and
  without it every query in the project referenced a table "without a matching indexed schema table".
- A composer lockfile states which namespaces each package autoloads, and that is where the mapping
  is written down: `Illuminate\Broadcasting\Channel` comes from `laravel/framework` and
  `Spatie\Permission\..` from `spatie/laravel-permission`, which no rule about names could work out.
- An import written through a path alias reaches the file it names: `tsconfig.json`/`jsconfig.json`
  `compilerOptions.paths` is read the way every bundler reads it (comments and trailing commas
  included, `baseUrl` honoured, longest prefix winning), so koel's `@/stores/userStore` is
  `resources/assets/js/stores/userStore.ts` rather than a package nobody declared.
- Unresolved call targets share one placeholder node per language and label (every call site keeps its own `calls` edge), and language builtins/std macros (`Some`, `format!`, `println!`, `len`, `console.log`, `make`, …) are classified as `resolution: builtin` instead of counting as external dependencies or unresolved-call findings. A definition that patches a runtime namespace — kong replaces `ngx.exit` — answers only calls written through that namespace, and a qualified builtin (`Object.create`) is never answered by a project function that shares its tail. `unresolved_call` findings are grouped per call label and read as `info` on syntactic-only scans — they escalate to `warning` only after semantic (LSP) enrichment has run and the target still cannot be resolved.
- Source rationale comments such as `WHY`, `NOTE`, `TODO`, `FIXME`, `HACK`, `BUG`, `XXX`, and `SECURITY` are indexed as linked graph facts with source spans for human and agent review.
- Common branch, loop, async/concurrency, and return/exit constructs are indexed as source-spanned graph facts for workflow diagrams and node-card investigation, and workflow blocks classify them as branch, loop, async, and return steps. These facts carry the dedicated `control_flow` node kind, so kind facets, summaries, and web filters show them by name instead of `unknown`.
- Markdown, ADR, and RFC documents are indexed as repository knowledge facts with section nodes plus local file/directory and symbol references from Markdown links and inline code. AsciiDoc (`.adoc`, how the Java and Solidity worlds document themselves) and reStructuredText (`.rst`, the format Python projects document themselves in) join the same graph: over- and underlined titles become `document_section` nodes and ``literal`` paths resolve to the files they name.
- Plain-text knowledge files (`.txt`, `.text`) join the graph as `plain_text` documents with line-count provenance and path-shaped mentions resolved to scanned files (capped at 100 references per file; the scan file-size budget bounds how much text is read); manifest-convention files such as `requirements.txt` stay manifests. Generated Markdown sidecars like `report.pdf.md` carry `generated` and `sidecar_of` metadata pointing back to the binary document they transcribe.
- Rich Markdown citations: YAML front matter (`title`, `owner`, `status`, `tags`, `date`) becomes document metadata queryable via `docs owner:… status:… tag:…`, `[[wikilinks]]` resolve to sibling and root documents, `#L42`/`#L42-L50` link anchors are kept as `line_ref` citation metadata on reference edges, and every cited node gets a `doc_backlinks` count; the web query panel ships a `Docs → code` preset for the docs-to-code overlay.
- SQL schema files are indexed as repository knowledge facts for tables, views, columns, indexes, and foreign-key references with exact source spans. A schema written inside application code counts the same: Kong keeps `CREATE TABLE IF NOT EXISTS plugins (...)` in a Lua migration's `[[ ]]` string, and those 39 tables and 228 columns are declared at the span they are written at, so its queries reach them. SQL spelled out in a comment describes a statement rather than running one, and declares nothing.
- SQL query strings in application code are indexed as query cards and linked back to schema table nodes for common `SELECT`, `JOIN`, `INSERT`, `UPDATE`, and `DELETE` references. Only statement-shaped literals count: `SELECT` and `DELETE` must reach a `FROM` and `INSERT`/`REPLACE` an `INTO`, `UPDATE` needs a `SET` and `WITH` a `SELECT`, a statement keyword binds only where a statement can begin, an alias is never a qualified name, and a token that ends a sentence marks prose. So UI prose such as "Build a workflow from a selected node", a Julia docstring signature (`select(df::AbstractDataFrame, ...)`), a CLI help string and test names listing "insert, update, and delete" no longer produce phantom table references, and queries extracted from inline `#[cfg(test)]` modules or test-convention files carry `test_context` metadata and read as `info` findings, mirroring the benchmark-oracle test exclusions.
- Local import/include resolution for relative JavaScript/TypeScript imports and CommonJS requires, C# `using` declarations resolved to the namespace the project declares (Polly resolves 450 of its 742 usings that way; the rest name the framework), Python relative/absolute project imports, Go module-local imports, quoted C/C++ includes with CMake and compile database include directories, PHP include/require paths and namespace imports, Bash source paths, and common Rust module paths. Rust module paths are read inside the crate the file belongs to — a workspace has one per member, so serde_derive's `use crate::internals::ast` is `serde_derive/src/internals/mod.rs` — and a crate laid out without `src/` is found by walking up, as ripgrep keeps `crates/core/flags/mod.rs` beside `crates/core/main.rs`; a module a compiling crate names may be written inline or re-exported, so a Rust miss stays quiet rather than claiming a missing file, and a file in a sibling crate never answers a `crate::` path. Rust glob imports (`use super::*;`) and leading-uppercase item imports (`use crate::SomeType`) are item references, not module files, so they no longer raise `unresolved_local_import` findings; unresolved imports declared by test-convention files read as `info`, as do findings about code the project vendored — redis carries jemalloc under `deps/` and dune carries `re` under `vendor/`, and their notes and cycles are upstream's.
- Manifest dependency extraction from Cargo, npm/package-lock/pnpm-lock, Go including indirect requirements, Python/Poetry/PEP 735 dependency groups, setup.py/setup.cfg/Pipfile, Composer/composer.lock including `suggest` as optional packages, vcpkg, Conan, and CMake `find_package` projects. A package a manifest marks optional — composer's `suggest`, a Python extra — is the pattern behind monolog's optional handlers, so importing one from the code that needs it is not a finding. A manifest that does not parse declares nothing, and says so: `malformed_manifest` names the file and the reason, so a missing brace in `package.json` reads as one finding instead of as a project with no dependencies.
- Heuristic config reads, environment reads, and potential error/exception constructs. C and C++ wrap a throw in a macro as often as they write the keyword — json writes `JSON_THROW(...)`, spdlog `SPDLOG_THROW(...)`, the OCaml runtime `caml_raise_not_found(...)` — so a call whose name says it throws counts, while a test framework's assertion *about* throwing (`CHECK_THROWS_AS`, `REQUIRE_THROWS`) does not. A Python
  application that keeps its configuration in a mapping — `app.config["SECRET_KEY"]`, or
  `app.config.get("PORT", 8080)` with its fallback — has each key read indexed by name, so asking
  where a key is read answers with the code that reads it.
- CLI command that emits graph JSON.
- HTTP API and embedded web UI for interactive graph exploration.
- API capabilities endpoint for discovering supported languages, exports, features, limits, cache state, and route groups.
- API capabilities limits include graph page, node-card, focus, query, report, source preview, and source-search ceilings for production clients.
- API capabilities publish the effective maximum JSON API request body size for POST clients.
- API capabilities publish insight/check result limits and API schema parameters link overview/report limits to matching capability keys.
- API capabilities publish bounded project report snapshot limits for architecture groups, architecture edges, language links, hotspots, graph communities, compact node/file summaries, and returned insights.
- API and web UI enforce the published maximum graph query expression length before running repository scans.
- API and web UI enforce the published maximum source-search text length before scanning source files.
- Machine-readable API schema endpoint for agents and integrations.
- API schema parameters expose structured minimum, maximum, maximum string length, and matching capability-limit keys where runtime bounds exist.
- API schema POST endpoints expose structured body fields for scan jobs and semantic enrichment requests.
- API schema POST endpoints link to the published maximum API request body size enforced by the server.
- API schema system endpoints expose structured response fields for probes, health, and runtime metrics.
- API schema publishes common response headers for request correlation, latency diagnostics, cache policy, security policy, and static-asset ETags.
- API schema graph investigation endpoints expose structured response fields for graph slices, node cards, queries, edge explanations, and reports.
- API schema analysis and source endpoints expose structured response fields for topology, traces, insights, checks, source previews, and source search.
- Semantic work queues clamp and publish maximum work item limits across CLI, API, schema, and web capabilities.
- Semantic LSP request timeouts clamp and publish runtime limits across CLI, API, and web capabilities.
- API schema enum values document supported graph query commands and query terms, including exact edge index lookups.
- API schema enum values document graph node kinds, edge kinds, insight kinds, and confidence levels used by graph filters.
- API schema enum values document project report sections and risk grades used by report consumers.
- API schema enum values document web deep-link parameters used for shareable node, dependency, and query investigations.
- API schema enum values stay aligned with semantic work statuses and capabilities used by LSP work queues.
- Runtime metrics endpoint for uptime, API/schema versions, roots, language/feature counts, cache state, job stores, and concurrency.
- Lightweight liveness and readiness probe endpoints for deployment health checks.
- Server package version is published through runtime probes, health, metrics, capabilities, and API schema responses.
- Multi-stage Docker image definition for running the web/API server with a mounted repository and persistent cache volume.
- Native desktop launcher (`codegraph-ui`) that starts a local CodeGraph backend and opens the explorer in a system WebView window.
- Built-in HTTP access logs with method, target, status, and latency for server operations.
- Per-response `x-request-id` correlation headers mirrored in access logs and JSON error bodies.
- Per-response `x-response-time-ms` timing headers for browser, proxy, and agent-side latency diagnostics.
- Graceful HTTP server shutdown on Ctrl-C and SIGTERM.
- Server-wide security headers for the embedded web UI and API responses.
- Server cache-control headers keep runtime/API responses uncacheable and force browser revalidation for unversioned embedded web assets.
- Embedded web asset ETags support conditional `304 Not Modified` responses during browser revalidation.
- Optional API bearer-token protection through `--api-token` or `CODEGRAPH_API_TOKEN`, with same-origin web UI token prompting.
- Project report snapshots in CLI, API, and web export for summary, full-risk scoring, quality gate, insights, topology reports, cache, and scan coverage.
- Project report snapshots include compact node/file summaries with roles, dependency facets, symbol, trace, config, environment, error, unresolved-call, and related-risk counts for agent navigation without broad raw-file reads.
- Web overview chips for server package version, server capabilities, API/schema versions, cache state, supported language/export counts, job limits, and route groups.
- Web overview surfaces the API schema common response-header contract for agent/client diagnostics.
- Web overview risk summary chips for report quality gate, grade, weighted score, severity counts, and top finding kinds with quick insight filtering plus one-click quality checks.
- Web overview hotspot, annotation, and entrypoint empty/focus states are localized for English and Russian UI sessions.
- Web overview scan policy, coverage, LSP, semantic work, architecture, and language-dependency diagnostics use localized status and focus text.
- Web overview reuses the project report snapshot for summary, coverage, topology, hotspots, and risks to avoid duplicate heavy scans.
- Deterministic graph community reports group files and contained symbols into top-level subsystems with internal/external edge counts, sample nodes, languages, and provenance edge indexes.
- Web runtime panel for uptime, cache state, scan/semantic slots, and retained job-store totals.
- Web runtime panel surfaces the last API response latency from `x-response-time-ms` for quick slow-endpoint diagnosis.
- Async scan job API for long-running repository scans.
- SSE scan job status stream for live web progress updates.
- Cancelable scan and semantic enrichment jobs for stopping queued or running long work.
- Job listing APIs for inspecting retained scan and semantic job history by status.
- Web job monitor for retained scan and semantic jobs with refresh, status summaries, and cancellation actions.
- Source preview API and UI panel for parsed symbols plus framework route/config facts with source spans.
- File node cards include source previews and can jump into focused file graph slices.
- Node cards return suggested focused graph actions for files, documents, symbols, packages, configs, and error facts.
- Node cards include dependency summaries with incoming/outgoing counts, edge kinds, confidence, and neighbor facets.
- File node cards include contained-symbol and in-file trace fact summaries for quick file-level triage.
- File node cards surface risks from contained symbols and facts, not only risks attached directly to the file node.
- Node cards include risk summaries by severity and insight kind alongside capped related risk lists.
- Enriched selected-node cards with summary metadata, source snippets, neighboring dependencies, trace actions, and related risks.
- Graph, query, focus, and node-card edges include stable `metadata.edge_index` values for exact dependency explanation and UI edge selection.
- The web UI starts with onboarding hints in every investigation panel (query, journey, source search, PR impact, refactoring, cache) that state the next action and localize with the interface language, and full graphs above 2,000 nodes start with low-signal `control_flow` facts hidden — the canvas-filter status shows the active default with a one-click reset, and the legend re-enables the kind.
- Key CLI commands ship `--help` examples (`scan`, `query`, `journey`, `ask`, `impact`, `refactor-context`, `pr-impact`, `trace-config`, `memory-save`, `mcp`), and node-not-found errors suggest up to three near-matching labels (`did you mean \`scan_project\`?`) or point at `entrypoints`/`query` for discovery.
- API query-string errors always use the structured JSON contract with `request_id` (including deserialization failures), and `/api/source` takes the same `path` project-root parameter as every other endpoint plus a `file` parameter for the source file (the older `root`+`path` form stays accepted).
- Every scanned node carries a deterministic `metadata.stable_id` (`cg-*`) derived from what the node is — its kind, file, label, language and item kind — and not from where in the file it sits, so a handle an agent saved survives an edit above it; when one file declares a name more than once, the order they are declared in tells them apart. Every surface that names a node accepts these durable ids alongside numeric (`42`) and n-prefixed (`n42`) ids: graph queries, `node-card`, `impact`, `journey`, `workflow`, both traces, `explain-edge`, and MCP target resolution.
- Syntactic call resolution is language- and file-aware. A duplicated method label becomes one explicit bounded ambiguity with candidate count/sample instead of edges to every same-named function, preventing cross-language false links and quadratic edge fan-out.
- Dart type annotations and constructor invocations produce `type_reference` / `constructor_reference` edges to uniquely declared types, allowing impact analysis to recover service consumers that method-name-only call graphs miss.
- Web canvas edges can be selected directly to open dependency cards with source, target, confidence, metadata, and provenance explanation actions.
- Web canvas edges highlight on hover so dependency paths are easier to inspect before opening a card.
- Edge explanations include related risk summaries and capped edge-scoped findings for dependency-level triage.
- Dependency cards can be opened from graph edges, query results, traces, and node neighbor lists.
- Dependency cards can focus or query their exact `edge_index` for fast canvas narrowing and agent handoff.
- Web node and dependency-card selections are reflected in shareable `node` and `edge` URL parameters with copy-link actions for exact human/agent handoff. A node link carries the durable `cg-*` id the scan stamped, so a link somebody keeps still opens the same definition after the file is edited above it; a positional id in an older link still resolves.
- Web node and dependency cards can be downloaded as JSON with source, dependency, and risk context for portable agent handoff.
- Web query presets include ambiguous calls, ambiguous entrypoints, dependency-scope/version/runtime-import/test-only issues, sensitive defaults, SQL query cards, and missing SQL table references for fast logical inconsistency triage.
- Selected external dependency cards can open focused package graph slices that connect declarations and import sites.
- Initial English/Russian web UI localization with a persistent language selector.
- Static web landmarks and pagination controls expose localized ARIA labels for English/Russian accessibility.
- Web quality-check and source-search workflows use localized status, result, empty-state, and export summary text.
- Web path-query workflow uses localized validation, loading, error fallback, and result labels in English/Russian UI sessions.
- Web graph-query and path-query workflows enforce the published query-length limit before issuing API requests.
- Web path-query results can be downloaded as JSON with endpoints, query expression, counts, and returned graph slice for agent handoff.
- Web graph export workflow uses localized progress, error fallback, and node/edge count labels in English/Russian sessions.
- Web source-search match cards use localized titles and loading states for English/Russian UI sessions.
- Web entrypoint trace workflow uses localized status, counters, empty states, truncation notes, export summary text, and focused graph titles.
- Web config/error trace workflows use localized status, counters, empty states, truncation notes, and focused graph titles.
- Off-by-default graph labels with collision-aware, sparse Auto/Focus modes so node cards stay readable without captions covering the graph.
- Hover/Focus labels render as short side badges only when the graph is sparse enough, selected-node details stay in the side card, and saved label modes reset when label-density rules change.
- Web graph viewport HUD for visible node/edge counts, zoom, and layout state during canvas exploration.
- Web graph edges connected to the hovered or selected node are softly highlighted for immediate local dependency context.
- Web graph selected/hovered-node neighborhoods emphasize adjacent nodes and dim unrelated graph noise while preserving the full canvas.
- Web graph legend node-kind chips can toggle canvas filters directly while staying synchronized with the sidebar kind filters.
- Keyboard-accessible graph canvas navigation for panning, zooming, fitting, resetting, and pausing layout.
- Dependency-free web label policy tests guard caption density, saved-mode resets, and interaction label behavior.
- Web API error messages include request ids when available so UI failures can be correlated with server access logs.
- Interactive UI trace panel for following outgoing dependency subgraphs from a selected node.
- Reverse dependency/dependent traces for impact analysis from CLI, API, query language, and web detail panels.
- Entrypoint trace API, CLI command, and web panel for comparing startup flows from manifest/code entrypoints.
- Web entrypoint trace reports can be downloaded as JSON with search, depth, and returned startup flows.
- Block-style workflow reports from a selected entrypoint, matched entrypoint set, or node label in CLI and API, with stable block ids, source node ids, edge indexes, confidence metadata, risk references, optional low-signal block compaction, Mermaid/DOT flowchart output from CLI/web, and a selected-node web Flow panel with JSON/Mermaid/DOT downloads.
- Workflow reports support edge kind, confidence, language, risk severity, block kind filters, and compact mode across CLI and API for smaller human diagrams and agent handoffs.
- A workflow rooted on a file or directory expands into the symbols it holds, while a container the flow only arrives at -- the file a route names as its definition site, a module an import reaches -- stays a single landmark block. One mastodon route no longer answers with the 196 sibling routes declared beside it instead of its own handler chain.
- Entrypoint workflow reports can be restricted to an entrypoint kind such as routes, CI workflow/pipeline jobs, Makefile targets, Docker/Compose commands, and Kubernetes workloads across CLI, API, and web, with known kinds published as API schema enum suggestions.
- Web Entry Flows can build block-style workflow reports for matched entrypoints, focus a workflow slice on the graph, and download JSON, Mermaid, or DOT for agent handoff and external diagramming.
- Web workflow filters are available for selected-node Flow panels and Entry Flows using edge kind, confidence, language, risk severity, and block kind controls.
- Web workflow block-kind badges and applied-filter summaries are localized for English and Russian UI sessions.
- Full web Flow view next to the graph canvas renders any built workflow as a depth-layered block diagram with pan, zoom, wheel/keyboard navigation, a minimap, hover highlighting, risk badges, and click-to-select blocks that open the standard node card.
- Flow view transitions are selectable too: clicking a diagram edge opens the standard dependency card with confidence, provenance, edge-scoped risks, and exact edge explanation actions.
- Selected-node Flow panels, Entry Flows, and query workflow results include an Open Flow view action that jumps into the block-diagram canvas.
- Graph query results can be converted into block-style workflow reports from CLI, API, and web query result actions.
- Target-directed journey reports (CLI `journey --from <start> --to <target>`, API `/api/journey`) expand entrypoint-to-target paths into step-numbered execution chains of workflow blocks with control-flow markers, edge provenance, and risk references on every step.
- Journey reports rank up to `--paths` alternative routes by edge confidence and length, report per-path `confidence_score` and `lowest_confidence`, and attach structured per-hop explanations (confidence note, relation, provenance source) for why each transition exists.
- Web Journey panel builds ranked entrypoint-to-target chains from `/api/journey`: step-numbered blocks with localized kind badges, fragile/risk chips, per-hop provenance notes, node/dependency card actions, graph focus per path, and JSON export for agent handoff.
- Journey steps expand in place into nested sub-flows (bounded workflow slices from the step node) with breadcrumb context back to the parent journey, collapse actions, and the same block/edge card and Flow-view actions as regular workflows.
- Reflection reports (`codegraph memory-reflect`) aggregate saved outcomes into per-node repository lessons with resolved labels, dead-end and correction lists, outcome counts, and stale-source warnings.
- Saved investigation memory (`codegraph memory-save` / `memory-list`) records query outcomes with lessons and linked node ids in `.codegraph/memory.jsonl`, and flags records as stale when the project fingerprint changes.
- Agent installation (`codegraph install-agent`) writes idempotent `.mcp.json` server entries and marker-delimited CLAUDE.md/AGENTS.md guidance blocks so assistants query the graph before raw file reads; `--hooks` adds assistant hook configuration snippets (Claude Code `PreToolUse` nudge on Grep/Glob plus a portable pre-search shell hook) under `.codegraph/hooks/`.
- MCP stdio server (`codegraph mcp`) exposes query_graph, get_node_card, get_neighbors, shortest_path, workflow, insights, impact, report, refactor_context, ask, source_search, and memory_save/memory_list/memory_reflect tools over newline-delimited JSON-RPC so external assistants use the graph — and repository memory — without shelling out to the CLI. The server scans one root when it starts and says which one in the `initialize` instructions; a tool call that names a `path`, `root` or `project` is refused with that root rather than answered from it.
- HTTP MCP transport (`POST /api/mcp`) serves the same MCP tools from `codegraph-server` through the shared engine, authenticated by the existing optional API bearer token for shared team graph access.
- Opt-in query audit logging (`[query_log]` in `.codegraph/config.toml`, `codegraph query-log`) appends CLI query/ask/journey and MCP tool calls to local `.codegraph/query-log.jsonl` with sensitive-value redaction and response previews only behind a second opt-in.
- Refactor context bundles (CLI `refactor-context`, API `/api/refactor-context`) combine impact, component dependencies, optional ranked journey, related risks, and a target source preview into one `codegraph.refactor_context.v1` JSON for one-shot agent handoff.
- Coupling/seam reports (CLI `seams`, API `/api/seams`) rank cross-area boundaries by deterministic friction score both ways: safest thin seams for extraction and most tangled boundaries needing work, with edge-kind/confidence breakdowns and sample edge evidence.
- Blast-radius reports (CLI `impact`, API `/api/impact`) list transitive dependents with distances and test flags, extract affected entrypoints/routes/tests, and rank one representative per source file before repeated symbols. Agent-facing calls skip repository-wide risks by default; `--include-risks`, `include_risks=true`, or `refactor-context` enables risk counts and risk-weighted scoring.
- Component dependency reports (CLI `component-dependencies`, API `/api/component-dependencies`) group a node's incoming/outgoing dependencies by architecture area, package, and language; component contract views (CLI `component-contract`, API `/api/component-contract`) list the exact cross-area edges with confidence and risk counts.
- Journey paths carry a `risk_summary` (risky steps/transitions, low-confidence hops, unresolved and ambiguous calls, duplicate labels, cycle back edges, severity counts) and per-step `fragile` flags with reasons so refactor-breaking hops are visible before changes start.
- Config trace API, CLI command, and web panel for finding config/environment readers and entrypoint paths.
- Web config trace reports can be downloaded as JSON with target, depth, matched readers, and dependency paths.
- Error trace API, CLI command, and web panel for following potential error/exception paths back to entrypoints.
- Web error trace reports can be downloaded as JSON with target, depth, source nodes, and exception-flow paths.
- Agent-friendly summary, entrypoint, and trace commands/endpoints.
- Agent-friendly graph query command and API for focused node, edge, call, dependency, trace, diagnostic, insight/risk, and unreachable-code slices.
- Agent-facing natural-language `ask` command and API map English/Russian investigation questions to deterministic bounded graph queries with generated query, rule, confidence, alternatives, and optional compact results. Questions the graph answers outright reach the query that answers them rather than a text search: cycles (`show me the cycles`), dependencies (`what does this project depend on`), startup (`how do I run the tests`, `where is the entry point`), blast radius (`what would break if I change X`), coupling (`which modules are most coupled`), and the public surface (`what are the public APIs`, answered from recorded visibility). A word the routing rules match on is what the question is about rather than a name to filter by, so "what are the public APIs" is not a search for `APIs` (which answered with nothing at all), and neither is "what are the HTTP routes" a search for `HTTP`. Questions naming a SCREAMING_SNAKE identifier with a read/set verb (`Where is CODEGRAPH_API_TOKEN read?`) route straight to the config/environment trace rule, so identifier substrings like `API` cannot pull them into the route rule.
- Agent-facing reports are self-describing: `ask`, `impact`, and `journey` responses carry stable `schema` ids (`codegraph.ask.v1`, `codegraph.impact.v1`, `codegraph.journey.v1`, matching the existing `codegraph.refactor_context.v1`) plus copy-paste-ready CLI follow-ups — `ask` returns a `cli_snippet` with the equivalent `codegraph query` command, and `impact`/`journey` return `suggested_commands` (node-card, journey, impact, refactor-context) built from resolved node ids; every graph/analysis/source API endpoint publishes a copy-paste `example` request in `/api/schema`.
- SQL/schema graph query slices for tables, columns, indexes, views, app SQL query cards, code-to-query references, schema references, and missing-table triage.
- Agent-friendly annotation graph queries for focused user-owned metadata slices from `.codegraph/annotations.toml`.
- Focused query responses include returned counts and facets for node kinds, edge kinds, languages, item kinds, and confidence.
- Agent-friendly symbol graph queries for focused function/type/module context with containing files and nearby dependency edges.
- Agent-friendly file graph queries for focused source-file structure, imports, and contained-symbol dependency context.
- Agent-friendly entrypoint graph queries for focused startup slices with immediate trace edges.
- Agent-friendly route graph queries for focused HTTP/framework route and handler slices.
- Agent-friendly config graph queries for focused configuration/environment reader slices and entrypoint paths.
- Agent-friendly error graph queries for focused exception/error source slices and entrypoint paths.
- Agent-friendly cycle graph queries for focused circular dependency slices.
- Agent-friendly hotspot graph queries for focused high-degree dependency slices.
- Agent-friendly source search command, API, and web panel for compact matching snippets.
- Web source-search matches can open the matching file's graph slice directly for dependency exploration.
- Web source-search results can be downloaded as JSON with query, path filter, matches, and context snippets.
- Edge explanation command, API, and web controls for confidence/provenance evidence.
- Path queries for finding directed dependency paths between labels or node ids.
- Confidence-aware edge queries and UI edge labels for fact provenance.
- Server-side graph paging and filtering endpoint for large repository exploration.
- Web graph page controls backed by server-side paging, search, kind, item, language, edge, confidence, relation, and source filters.
- Web graph page controls can page dense edge slices independently with the `/api/graph` `edge_offset` contract.
- Web graph and insight filter inputs use API schema enum suggestions for node kinds, edge kinds, confidence levels, insight severities, and insight kinds.
- Web fallback dependency insights recognize local import scopes and common PHP/Composer namespace imports while server reports are loading or unavailable.
- Web graph viewport controls for zooming, fitting visible nodes, restarting layout, and pausing layout simulation.
- Web graph page and viewport HUD show loaded-vs-total slice status so large-repository canvases are clearly marked as partial views.
- Web graph minimap shows graph position and supports click/drag recentering during large-canvas exploration.
- Web risk legend entries can filter the graph to nodes with matching insight severity.
- Web project overview for language mix, edge confidence/source/relation mix, and entrypoint launch points.
- Language dependency matrix reports in CLI, API, and web overview for mixed-language coupling.
- Architecture map reports in CLI, API, and web overview for top-level project areas and cross-area dependencies.
- Web semantic work queue with filters for reviewing prioritized LSP enrichment tasks and focusing their graph evidence.
- Web semantic overview counters for definitions, diagnostics, document symbols, references, workspace symbols, and queued work.
- Architecture overview chips can focus the paged graph by project area path prefix.
- Architecture dependency chips can focus the exact graph edges behind cross-area coupling.
- Surprising-link reports in Markdown/API/web overview rank cross-area, cross-language, low-confidence, and config/error/dependency boundary edges with exact edge evidence.
- Hotspot reports in CLI, API, and web overview for high-degree files, functions, entrypoints, and config nodes, with architectural hubs separated from noisy utility hubs.
- Web path navigation for finding, focusing, and visually highlighting dependency paths between graph nodes.
- Node context API and detail-panel neighbor loading for paged graph exploration.
- Server-backed web insights for project-wide findings while browsing paged graph slices.
- Insight reports include severity and kind breakdowns for triage.
- Web insight severity breakdown chips can apply and clear triage filters directly.
- Server-side insight filters for severity, kind, search, and capped agent/UI reads.
- Web insight findings can be exported as JSON with active filters and severity/kind counts for review or agent handoff.
- CI/agent check command, API, and web quality gate for failing on insight severity thresholds.
- Web quality-gate check results can be downloaded as JSON for CI handoff and review records.
- Insight focus API and web interaction for turning findings into focused graph views.
- Web query panel for running focused graph queries, narrowing the canvas to query results, and jumping to matching nodes.
- Web Ask field maps English/Russian investigation questions to generated graph queries, focuses the canvas on the result, and keeps the generated query/export payload for agent handoff.
- Web query panel supports shareable `query` deep-links and copy-link actions for reusable investigations.
- Web query panel keeps a local recent-query history so repeated investigations can be rerun quickly.
- Web query results can be downloaded as JSON with query, root, facets, nodes, and edges for agent handoff.
- CLI/API graph query and focus results support optional compact mode for collapsing repeated low-signal nodes while preserving raw counts and compacted metadata.
- Web graph page filters, node/edge offsets, and page limits can be copied as shareable deep-links for reproducible large-repository slices.
- Web graph page filters and offsets can be cleared in one action after opening focused or shared large-repository slices.
- Web canvas search, kind, risk, and query-focus filters show active-filter status in the HUD and can be cleared in one action.
- Web export panel for downloading full graph snapshots as JSON, DOT, NDJSON, GraphML, SVG, Mermaid HTML, Neo4j Cypher, or FalkorDB.
- Web export panel can download the currently visible canvas slice with graph-page, filter, viewport, and layout metadata for compact handoff.
- Full graph exports publish response headers for node count, edge count, and serialized byte size.
- Web project selector backed by an explicit server-side allowlist for opening local repositories.
- DOT/Graphviz, NDJSON, GraphML, SVG, Mermaid HTML, Neo4j Cypher, and FalkorDB export formats for visualization, streaming agent use, external graph tools (yEd, Gephi, Cytoscape), shareable callflow pages, and graph databases. The SVG export renders a self-contained, deterministic image (highest-degree nodes on a circle, confidence-colored edges, kind legend, hover titles) with published node/edge limits and a truncation comment for larger graphs.
- Persistent server-side graph cache with project fingerprint invalidation.
- Persistent CLI graph cache using the same project fingerprinting and cache records as the server.
- Persistent per-file parser fact cache reused during graph-cache misses.
- Persistent graph impact index in cache records for fast incremental planning of affected nodes and edges.
- Persistent file graph chunk index in cache records for explicit per-file node and edge scopes.
- Cache fingerprint diff diagnostics in CLI, API, and web UI for explaining cache misses by added, removed, and modified files.
- Cache reuse estimates in CLI, API, and web UI for planning incremental scans from unchanged files and bytes.
- Incremental scan planning reports in CLI, API, and web UI with rescan, removed, reusable path sets, and cached impacted graph node/edge ids.
- Changed-scope incremental scan graphs in CLI, API, and web UI for inspecting only files that need rescanning.
- Incremental merge previews that use the persistent impact index to replace cached file scopes with changed-file rescans for fast review before a full scan.
- Watch mode (`codegraph watch`) polls the project fingerprint and refreshes the graph cache automatically on changes, streaming NDJSON refresh events and falling back to a storing full rescan when a partial merge is not surface-stable.
- Git hooks (`codegraph install-hooks`) refresh the graph cache after every commit and checkout through idempotent marker-delimited hook blocks, with optional export regeneration configured under `[hooks]` in `.codegraph/config.toml`.
- Global graph registry (`codegraph registry-add`/`registry-list`/`registry-remove`/`registry-query`) runs one query or path expression across several registered local repositories with per-project results and inline per-repository errors.
- Graph merge (`codegraph merge`) combines exported graph artifacts and registered projects into one deterministic byte-stable graph with `merge_sources` provenance on every merged node/edge and an explicit metadata-conflict report.
- Obsidian/Markdown wiki export (`codegraph export-wiki`) writes an interlinked vault of communities, entrypoints, hotspots, config flows, and risky findings that opens in Obsidian without conversion.
- PR impact dashboard (`codegraph pr-impact`, `GET /api/pr-impact`, and the web PR Impact panel) maps changed files from git onto touched communities, shared hotspots, reverse-dependent blast radius, and changed-code risks with a deterministic risk score for merge gates.
- The web Refactoring panel runs blast-radius impact, component dependency groupings, seam rankings, and area contracts against a typed-in or selected node, and downloads the one-shot refactor-context bundle as JSON — the same reports as `impact`, `component-dependencies`, `seams`, `component-contract`, and `refactor-context` in the CLI/API. Bundle requests evaluate insights once and reuse them across sections, so warm-cache refactor-context responses return in under a second on this repository.
- Benchmark harness (`codegraph bench-context`) quantifies token/context savings of bounded graph slices versus raw reading and measures extraction recall against independent text-scan oracles.
- API schema enum values document cache status, reuse strategy, incremental actions, and merge blocker kinds for agent-safe incremental workflows.
- Web incremental cache diagnostics show localized completeness blockers and safe-update reasons.
- Scan coverage reports in CLI, API, and web overview for indexed files, policy skips, large-file skips, and non-indexed files.
- CLI scan benchmark reports with timing and graph-size metrics for regression tracking.
- Configurable scan file-size budget for CLI/server scans, with skipped large source files kept visible in summaries, insights, and the web stats panel.
- Repository-owned scan policy from `.codegraph/config.toml` for file-size budgets plus ignored names and globs.
- Effective scan policy API and web overview chips for explaining the active file-size, hidden-file, ignored-file, ignored-name, and ignored-glob rules.
- CI checks for formatting, clippy, tests, UI syntax, web label policy, Docker build and container smoke, embedded web assets, CLI scan, server cache, and safe incremental update smoke tests.
- Workflow regression fixtures guard block classification and transition shape for Rust, Python, JavaScript/TypeScript, Go, PHP, Bash, and Dart runtime paths, plus entrypoint-kind workflow reports for CI jobs, Makefile targets, Docker/Compose commands, and Kubernetes workloads.
- Embedded web asset smoke checks cover shareable node, dependency, and query investigation links.
- Investigation insights for unresolved calls, parse errors, duplicate labels, orphan functions, and error-flow facts.
- Investigation insights for semantic LSP diagnostics, preserving language-server severity, source, code, file location, and the affected source node for node-card triage.
- Investigation insights for duplicate entrypoint labels that can make label-based startup traces ambiguous.
- Investigation insights for manifest entrypoints that resolve to multiple possible files or functions.
- Investigation insights for ambiguous call resolutions where one call label from the same caller points to multiple possible targets.
- Investigation insights for manifest entrypoints whose declared target cannot be resolved to a file or function.
- Investigation insights for entrypoints that have no outgoing code/config/dependency/error flow.
- Investigation insights for Dockerfile, Makefile, Docker Compose, and Kubernetes runtime paths/config refs that reference missing local files, missing local bind volumes, missing ConfigMap/Secret manifests, missing Ingress backend Services, unmatched Service selectors, plus duplicate Compose published ports.
- Investigation insights for framework routes whose named handler cannot be linked to a scanned function.
- Investigation insights for heuristic cross-language dependency edges that deserve semantic review.
- Investigation insights for local imports/includes whose target file cannot be found.
- Investigation insights for config/environment reads that are not reachable from any detected entrypoint.
- Investigation insights for potential error/exception flows whose source is not reachable from any detected entrypoint.
- Investigation insights for non-test source files with code symbols that are not reachable from any detected entrypoint.
- Investigation insights for config/environment keys that are read with conflicting fallback defaults, including common inline Rust, Python, JavaScript/TypeScript, Go, C, C++, Dart, PHP, and Bash environment-read patterns.
- Investigation insights for config/environment keys that are read both as required and with fallback defaults.
- Investigation insights for sensitive config/environment keys, credential-like defaults, placeholder secret fallbacks, and literal sensitive CI environment assignments, without echoing fallback values in reports.
- Investigation insights for risky source rationale markers such as `SECURITY`, `FIXME`, `HACK`, `BUG`, and `XXX`, while lower-noise `WHY`, `NOTE`, and `TODO` comments remain graph facts for context.
- Investigation insights for application SQL query strings that reference tables without matching indexed schema tables.
- Dependency consistency insights for external imports/CommonJS requires that are not backed by declared manifest dependencies.
- Dependency consistency insights for runtime manifest dependencies with no matching import.
- Dependency consistency insights for package declarations with conflicting manifest constraints. Two texts are not two requirements: a constraint is read as the range of versions it admits, so a Cargo workspace asking for `anyhow 1.0.75` in one crate and `1.0.103` in another installs one version and says nothing, while `blinker ==1.6.2` against `>=1.9.0` cannot be satisfied at once and does. A bare version is a range where cargo and pub read one and a pin where npm, composer, Go and Python do; an unreadable constraint is reported rather than assumed compatible.
- Dependency consistency insights for packages declared across multiple dependency scopes such as runtime and dev/build.
- Dependency consistency insights for production imports of packages declared only in non-runtime scopes, including Go `// indirect` requirements.
- Composer dependency consistency insights map common PHP namespace imports such as `Monolog\*`, `Symfony\Component\Console\*`, and `PHPUnit\Framework\*` back to package nodes.
- Dependency consistency insights for production-like source files importing packages declared only in non-runtime scopes. An import a language erases before anything runs is not one of them: TypeScript's `import type` and Python's `if TYPE_CHECKING:` block name what a type checker reads, so they neither make a dev dependency a runtime one nor close a dependency cycle — requests writes its `_types.py` that way.
- Dart/Flutter `pubspec.yaml` dependencies participate in undeclared, unused, dev-only-in-production, and test-only runtime dependency insights.
- Flutter `pubspec.yaml` assets are indexed, Dart asset reads are linked as config facts, missing asset declarations produce warnings, and Dart platform channels are surfaced as external boundary nodes.
- Dart `package:` imports resolve through `pubspec.yaml` and `.dart_tool/package_config.json` package maps (workspace-relative `rootUri` only, with escape and absolute-URI guards), so path dependencies and monorepo packages link to their scanned files; generated files (`.g.dart`, `.freezed.dart`, protobuf, mocks, `.gen.dart`) carry `generated` metadata with a `generated_from` link to the source that produces them.
- Dart analysis server semantic patches are validated end-to-end (definitions upgrade heuristic call edges to semantic confidence, diagnostics attach to Dart nodes), and the graph cache fingerprint tracks hidden `.dart_tool/package_config.json` files so regenerating a package map invalidates cached graphs alongside `pubspec.yaml` and generated-file edits.
- Flutter platform channels match their native handlers: Kotlin/Java/Swift/Objective-C channel registrations link to the Dart channel nodes by name and kind with per-platform `native_handler_*` metadata, and Dart channels with no native registration raise an `unmatched_platform_channel` warning when the repository contains native host sources.
- SQL depth: JOINs link table nodes with captured ON conditions (`sql_join` edges), migration files chain in sequence order per directory (`migration_order` edges with `migration_sequence` metadata, numeric and Flyway `V<digits>__` prefixes), ALTER/DROP TABLE statements link to their tables, and consistency insights flag duplicate migration sequence numbers plus ALTER/DROP targets missing from the indexed schema.
- Application code links to SQL tables through ORM metadata (`__tablename__`, Django `db_table`, TypeORM `@Entity`, Sequelize `tableName`, Prisma `@@map`, Laravel `$table`, Doctrine, Diesel, GORM) as `orm_table_mapping` edges, and migration runners plus database configs (`sqlx::migrate!`, `alembic.ini script_location`, `flyway.locations`, knex/phinx directories) link to the migration files they run as `runs_migrations` edges.
- MCP configuration files (`.mcp.json` including the hidden root convention, `mcp.json`, `mcp_servers.json`, `claude_desktop_config.json`) are indexed as tool/server dependency facts: each declared server becomes a node with transport (stdio/http), command, url, and args metadata, linked from its config file, with path-like commands and args matched to scanned source files.
- Dependency consistency insights for runtime dependencies that are imported only from test-like source files.
- Focused package graph queries that connect manifest declarations, import sites, and source files for mixed-language dependency investigation.
- Focused file graph queries that connect source files to contained symbols, imports, config/environment reads, potential errors, and nearby dependency edges.
- Framework route insights for duplicate HTTP method/path declarations.
- Framework config convention facts for common web/service stacks across mixed-language repositories.
- Repository custom rule insights from `.codegraph/rules.toml`.
- Project-wide web overview facets for user graph annotations from `.codegraph/annotations.toml`.
- Project-wide web overview facets for edge provenance sources and relation metadata.
- Dependency cycle insights for circular calls, imports, references, and manifest dependency edges.

Planned next:

- Better graph layout and richer visual graph affordances.
- Richer graph queries for humans and agents.
- Production hardening for large repositories.

## Usage

Task-oriented walkthroughs — investigate a bug, trace a config value, and
plan a refactor step by step with verified commands — live in
[`docs/GUIDES.md`](docs/GUIDES.md). The sections below are the
feature-by-feature reference.

Run the initial scanner:

```bash
cargo run -p codegraph-cli -- scan .
```

List built-in language adapters and detection patterns:

```bash
cargo run -p codegraph-cli -- languages
```

Report available semantic language servers:

```bash
cargo run -p codegraph-cli -- lsp
cargo run -p codegraph-cli -- semantic-readiness .
cargo run -p codegraph-cli -- semantic-plan .
cargo run -p codegraph-cli -- semantic-plan . --work-item-limit 25
cargo run -p codegraph-cli -- semantic-plan . --work-status ready --work-capability definitions
cargo run -p codegraph-cli -- semantic-batch . --work-status ready --work-capability definitions
cargo run -p codegraph-cli -- semantic-batch . --work-status ready --work-capability workspace_symbols
cargo run -p codegraph-cli -- semantic-run . --work-status ready --work-capability definitions > responses.json
cargo run -p codegraph-cli -- semantic-patch . --work-status ready --work-capability definitions --responses responses.json
cargo run -p codegraph-cli -- semantic-apply . --work-status ready --work-capability definitions --responses responses.json
```

`semantic-run` requires the matching language server to be installed and startable. `responses.json` for CLI commands is a JSON array of LSP response objects. Semantic LSP responses are cached under the shared cache directory by default; pass `--no-cache` to force a fresh language-server run.

Limit per-file scan reads for very large repositories:

```bash
cargo run -p codegraph-cli -- --max-file-size 1048576 scan .
```

Export for Graphviz or streaming agent use:

```bash
cargo run -p codegraph-cli -- scan . --format dot
cargo run -p codegraph-cli -- scan . --format ndjson
cargo run -p codegraph-cli -- scan . --format graphml
cargo run -p codegraph-cli -- scan . --format mermaid-html > graph.html
cargo run -p codegraph-cli -- workflow-entrypoints --entrypoint-kind route --format html > callflows.html
cargo run -p codegraph-cli -- scan . --format cypher > graph.cypher      # cypher-shell -f graph.cypher
cargo run -p codegraph-cli -- scan . --format falkordb > graph.falkordb  # redis-cli < graph.falkordb
```

Summarize a project:

```bash
cargo run -p codegraph-cli -- summary .
```

Show an architecture map grouped by top-level project area:

```bash
cargo run -p codegraph-cli -- architecture .
```

Show language-to-language dependency links:

```bash
cargo run -p codegraph-cli -- language-dependencies .
```

Rank surprising cross-area, cross-language, and low-confidence dependency links:

```bash
cargo run -p codegraph-cli -- surprising-links .
```

Find high-degree graph hotspots, including separated architectural and utility hub buckets:

```bash
cargo run -p codegraph-cli -- hotspots .
```

Find graph communities/subsystems:

```bash
cargo run -p codegraph-cli -- communities .
```

Create a production-oriented project report snapshot:

```bash
cargo run -p codegraph-cli -- report . --fail-on warning --insight-limit 100
cargo run -p codegraph-cli -- report . --format markdown --output CODEGRAPH_REPORT.md
```

**What each severity means.** A `warning` is about the program itself. A
finding reads as `info` when it is not: when it is about code the project
did not write (a vendored `vendor/`, `deps/`, `third_party/` or
`node_modules/` directory) or does not ship (tests, examples, docs, build
scripts); when nothing looked for the thing it names (a path inside an
install or build directory, a hidden path, or a name a template fills in);
when the project starts nothing of its own, so coverage cannot be low; or
when the subject is the resolution rather than the code, as with unresolved
and ambiguous calls. `error` is reserved for a file the scan could not read
or parse, a `SECURITY:` comment in the project's own code, and a diagnostic
a language server reported as an error. Every risk row in the report names
one kind at one severity, so the warning rows add up to the summary's
warning count.

The JSON report includes a `risk_summary` with total findings, severity counts, a weighted score (errors ×100 plus warnings ×10 — info findings are counted but do not affect the score or grade, so large healthy repositories read as healthy), a grade, and the top insight kinds. Error-flow facts and heuristic-resolution findings (ambiguous calls, cross-language heuristic edges) read as info on syntactic-only scans and escalate to warnings once semantic enrichment has run. The quality gate is calculated from the full insight set even when the returned insight list is capped with `--insight-limit`; the gate payload itself carries limit-independent totals and severity/kind breakdowns plus a failing-first sample of at most 25 findings, so report JSON stays compact on noisy repositories (the limit is published as `report_quality_gate_sample_limit` in `/api/capabilities`). The Markdown report is a Graphify-style handoff artifact for humans and agents, with summary, compact node/file summaries, key concepts, communities, surprising links, architecture links, risks, evidence ids, suggested questions, and confidence wording that maps CodeGraph evidence to extracted/resolved/inferred/ambiguous labels. It cites every node by the durable `cg-*` id, since the document is committed and read weeks later, while the positional `n42` moves when a file is edited above the node; the JSON report carries the same translation as `durable_node_ids`.

Explain scan coverage before or after a full graph scan:

```bash
cargo run -p codegraph-cli -- coverage .
```

Benchmark scanner performance:

```bash
cargo run -p codegraph-cli -- benchmark . --runs 5
cargo run -p codegraph-cli -- bench . --runs 5
```

List entrypoint candidates:

```bash
cargo run -p codegraph-cli -- entrypoints .
```

List investigation insights:

```bash
cargo run -p codegraph-cli -- insights .
cargo run -p codegraph-cli -- insights . --severity warning --kind dependency --limit 25
cargo run -p codegraph-cli -- insights . --kind custom_rule
cargo run -p codegraph-cli -- insights . --kind sensitive_config_default
cargo run -p codegraph-cli -- insights . --kind rationale_risk_comment
```

Fail CI or agent workflows when findings meet a severity threshold:

```bash
cargo run -p codegraph-cli -- check . --fail-on error
cargo run -p codegraph-cli -- check . --fail-on warning --kind dependency
```

`check` prints a JSON report and exits with code `2` when matching insights are
at or above the configured severity.

Pin repository scan policy with `.codegraph/config.toml`:

```toml
[scan]
max_file_size = 1048576
include_hidden = false
include_ignored = false
extra_ignored_names = ["coverage", "generated"]
extra_ignored_globs = ["fixtures/**", "public/**/*.min.js"]
```

`ignored_names = [...]` replaces the default ignored directory list, while
`extra_ignored_names = [...]` extends it. `ignored_globs = [...]` replaces the
repository path-pattern ignore list, while `extra_ignored_globs = [...]` extends
it. Generated graph directories such as `.codegraph` and `graphify-out` are in
the default ignore list, preventing one tool's index from being indexed as
source by another. Glob patterns are matched against normalized project-relative paths. CLI/server flags such as
`--include-hidden`, `--include-ignored`, and `--max-file-size` override the
repository config for that run.

Add repository-specific architecture checks with `.codegraph/rules.toml`:

```toml
[[rules.forbidden_dependency]]
id = "no-left-pad"
ecosystem = "npm"
package = "left-pad"
severity = "error"
message = "left-pad is not allowed in production services"

[[rules.required_config]]
id = "needs-database-url"
target = "DATABASE_URL"
severity = "warning"

[[rules.forbidden_edge]]
id = "ui-cannot-call-db"
edge_kind = "calls"
severity = "error"
message = "UI layer must not call database layer directly"

[rules.forbidden_edge.source_metadata]
"annotation.layer" = "ui"

[rules.forbidden_edge.target_metadata]
"annotation.layer" = "database"
```

Custom rules currently support forbidden manifest dependencies, required
config/environment targets, and forbidden graph edges between annotated nodes.
Violations are emitted as normal graph facts and show up in CLI, API, and web
insight reports.

Add user-owned graph metadata with `.codegraph/annotations.toml`:

```toml
[[annotations.node]]
id = "payments-files"
kind = "file"
label = "payments"

[annotations.node.set]
domain = "payments"
owner = "team-payments"
critical = true
```

Node annotations match existing graph nodes by `kind`, `label`, `language`,
`item_kind`, and optional `[annotations.node.metadata]` conditions. Values from
`set` are stored as `annotation.*` metadata and can be queried by humans,
agents, API clients, and the web UI.

Query focused graph slices:

```bash
cargo run -p codegraph-cli -- query 'nodes kind:function label:main' .
cargo run -p codegraph-cli -- query 'edges kind:calls source:main' .
cargo run -p codegraph-cli -- query 'edges edge_index:0' .
cargo run -p codegraph-cli -- query 'edges confidence:heuristic' .
cargo run -p codegraph-cli -- query 'calls(function:main)' .
cargo run -p codegraph-cli -- query 'trace label:main depth:3' .
cargo run -p codegraph-cli -- query 'dependents label:load_config depth:3' .
cargo run -p codegraph-cli -- query 'neighbors label:main direction:out depth:2 edge_kind:calls' .
cargo run -p codegraph-cli -- query 'symbols label:load_config direction:out edge_limit:300' .
cargo run -p codegraph-cli -- query 'files path:src/main.rs direction:out edge_limit:300' .
cargo run -p codegraph-cli -- query 'docs target:src/main.rs relation:markdown_link edge_limit:300' .
cargo run -p codegraph-cli -- query 'sql table:users operation:select edge_limit:300' .
cargo run -p codegraph-cli -- query 'entrypoints language:rust' .
cargo run -p codegraph-cli -- query 'routes method:GET path:/health depth:3 edge_limit:300' .
cargo run -p codegraph-cli -- query 'packages package:serde ecosystem:cargo edge_limit:300' .
cargo run -p codegraph-cli -- query 'configs target:DATABASE_URL depth:6' .
cargo run -p codegraph-cli -- ask 'Where is DATABASE_URL read from the environment?' .
cargo run -p codegraph-cli -- ask 'Кто вызывает load_config?' . --compact
cargo run -p codegraph-cli -- query 'errors target:panic depth:6' .
cargo run -p codegraph-cli -- query 'cycles edge_kind:calls' .
cargo run -p codegraph-cli -- query 'hotspots language:rust min_score:5 edge_limit:300' .
cargo run -p codegraph-cli -- query 'path from:main to:load_config depth:6' .
cargo run -p codegraph-cli -- query 'unreachable language:rust' .
cargo run -p codegraph-cli -- query 'unreachable kind:function label:legacy_worker' .
cargo run -p codegraph-cli -- query 'unreachable scope:config search:LEGACY_TOKEN' .
cargo run -p codegraph-cli -- query 'unreachable scope:errors search:LegacyError' .
cargo run -p codegraph-cli -- query 'diagnostics severity:error language:rust' .
cargo run -p codegraph-cli -- query 'insights severity:error kind:dependency' .
cargo run -p codegraph-cli -- query 'insights kind:ambiguous_entrypoint_target' .
cargo run -p codegraph-cli -- query 'insights kind:ambiguous_call_resolution' .
cargo run -p codegraph-cli -- query 'insights kind:sensitive_config_default' .
cargo run -p codegraph-cli -- query 'insights kind:rationale_risk_comment' .
cargo run -p codegraph-cli -- query 'annotations key:domain value:payments direction:out edge_limit:300' .
cargo run -p codegraph-cli -- query 'nodes metadata.annotation.domain:payments' .
```

Inspect a node card with context, source preview, and related risks:

```bash
cargo run -p codegraph-cli -- node-card . --node-id 1
```

Search source text with compact snippets:

```bash
cargo run -p codegraph-cli -- source-search DATABASE_URL . --path-filter src --limit 20
```

Trace outgoing dependencies from a label:

```bash
cargo run -p codegraph-cli -- trace main . --depth 3
cargo run -p codegraph-cli -- workflow main . --depth 4 --format mermaid
cargo run -p codegraph-cli -- workflow main . --edge-kind calls --confidence heuristic --block-kind call
cargo run -p codegraph-cli -- workflow main . --depth 10 --max-fanout 8 # follow the call chain into depth instead of a wide shallow fan
cargo run -p codegraph-cli -- workflow-entrypoints . --search server --depth 4
cargo run -p codegraph-cli -- workflow-entrypoints . --entrypoint-kind route --depth 4
cargo run -p codegraph-cli -- workflow-query 'nodes kind:function search:main' . --edge-kind calls
```

Follow one execution journey from an entrypoint to a target as a step-numbered chain:

```bash
cargo run -p codegraph-cli -- journey --from main --to load_config .
cargo run -p codegraph-cli -- journey --from "cargo bin:codegraph-server" --to scan_graph . --depth 12 --paths 5
```

Journey steps reuse workflow blocks, so each step keeps its block kind (start, call, branch, loop, async, return, error), node, incoming transition with edge provenance, and related risk references. Up to `--paths` alternative routes are returned, ranked by edge confidence and then length (exact evidence beats heuristic guesses even when the heuristic route is shorter); alternatives avoid edges already used by better-ranked paths, each path reports its `confidence_score` and `lowest_confidence`, and every hop carries a structured explanation of why the transition exists.

Understand what a component depends on and what depends on it:

```bash
cargo run -p codegraph-cli -- component-dependencies load_config .
cargo run -p codegraph-cli -- component-contract --source web --target crates .
```

`component-dependencies` groups a node's incoming/outgoing edges by architecture area, canonical package, and language with confidence counts and sample edge indexes; `component-contract` lists the exact directed dependency edges between two architecture areas with edge kinds, confidence counts, and related risk counts for boundary reviews. Areas are named exactly as `architecture` lists them: a name that fits several areas — `crates` where each crate is its own area — is answered with the candidates rather than a guess.

Hand an agent everything it needs to plan a refactor in one request:

```bash
cargo run -p codegraph-cli -- refactor-context load_config . --from "cargo bin:codegraph-server"
curl 'http://127.0.0.1:3765/api/refactor-context?path=.&target=load_config&from=main&depth=8'
```

The `codegraph.refactor_context.v1` bundle combines the blast-radius impact report, component dependency groups, an optional ranked entrypoint-to-target journey with fragile flags, all risks touching the target or its dependents, and a source preview around the target span — one JSON payload with node ids, edge indexes, and source spans an agent can act on without raw repository reads.

Rank cross-area boundaries by coupling friction before choosing where to cut:

```bash
cargo run -p codegraph-cli -- seams . --limit 25
curl 'http://127.0.0.1:3765/api/seams?path=.&limit=25'
```

The seam report aggregates directed dependency edges between architecture areas and ranks every boundary two ways: `safest` (ascending friction — thin, well-declared seams where extraction is safest) and `most_needed` (descending friction — tangled boundaries where splitting pays off most). `friction_score` = edges + 2×low-confidence edges + 3×edge risks + distinct edge kinds; each candidate carries edge-kind and confidence breakdowns plus sample edge indexes for exact evidence.

Report the blast radius of changing a node before a refactor:

```bash
cargo run -p codegraph-cli -- impact load_config . --depth 6 --limit 40
cargo run -p codegraph-cli -- impact load_config . --include-risks
curl 'http://127.0.0.1:3765/api/impact?path=.&target=load_config&depth=6'
```

The default impact report performs only the bounded reverse traversal and marks `risks_evaluated: false`; this keeps CLI/API/MCP agent calls responsive on large graphs. With `--include-risks`, it also attaches dependent risk counts and computes the risk-weighted `impact_score` (dependents + 5 per entrypoint + 1 per test + 5/2/1 per error/warning/info risk).

Trace incoming dependents for impact analysis:

```bash
cargo run -p codegraph-cli -- trace-dependents load_config . --depth 3
```

Explain why a graph edge exists:

```bash
cargo run -p codegraph-cli -- explain-edge . --source main --target load_config --kind calls
cargo run -p codegraph-cli -- explain-edge . --edge-index 12
```

Trace startup flows from entrypoint candidates:

```bash
cargo run -p codegraph-cli -- trace-entrypoints . --depth 3
cargo run -p codegraph-cli -- trace-entrypoints . --search server --depth 4
```

Trace config files and environment variables back to readers and entrypoints:

```bash
cargo run -p codegraph-cli -- trace-config DATABASE_URL . --depth 6
```

Each entry says which side it is on: `role` is `sets` for a workflow job or
Compose service that assigns the variable and `reads` for everything that
uses it.

Trace potential error and exception constructs back to sources and entrypoints:

```bash
cargo run -p codegraph-cli -- trace-errors 'failed to load data' . --depth 6
```

Include hidden paths:

```bash
cargo run -p codegraph-cli -- scan . --include-hidden
```

Include default ignored paths such as `target` and `node_modules`:

```bash
cargo run -p codegraph-cli -- scan . --include-ignored
```

CLI graph commands use the persistent graph cache by default. Disable it for a
single run or pin records to a specific directory:

```bash
cargo run -p codegraph-cli -- summary . --no-cache
cargo run -p codegraph-cli -- summary . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- cache-diff . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- cache-chunks . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-plan . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-scan . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-merge-preview . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-update . --cache-dir /tmp/codegraph-cache
cargo run -p codegraph-cli -- incremental-update . --full-graph   # embed the merged graph JSON
```

The output is JSON using the shared graph schema from `codegraph-core`.
`incremental-merge-preview` and `incremental-update` print a compact summary
by default — plan, merge stats, node/edge counts, and the cache result —
because the full merged graph runs to tens of megabytes on real
repositories; pass `--full-graph` to embed it (the API endpoints keep
returning the full graph for the web canvas).

Keep the graph fresh automatically while editing:

```bash
cargo run -p codegraph-cli -- watch . --interval-ms 2000
cargo run -p codegraph-cli -- watch . --max-refreshes 1   # one refresh, then exit
```

`watch` polls the project fingerprint (the same content hash the cache uses) and runs the safe incremental update whenever it changes, streaming NDJSON events to stdout: a `watching` line on start, then one `refreshed` line per change with the plan action, changed/rescanned/removed file counts and paths, graph size, and refresh duration. Updates that are not surface-stable (for example a new file adding top-level nodes) automatically fall back to a storing full rescan (`fallback_full_scan: true`), so the cache always converges to the current tree. A warm cache that already matches the tree syncs silently, scan errors become `error` events without stopping the watcher, and polling keeps the watcher editor-agnostic with no extra dependencies.

Refresh the graph automatically after commits and checkouts instead:

```bash
cargo run -p codegraph-cli -- install-hooks .
```

```toml
# .codegraph/config.toml — optional export regeneration on every hook run
[hooks]
exports = ["dot", "json"]          # json, dot, ndjson
export_dir = ".codegraph/exports"  # default
```

`install-hooks` writes marker-delimited blocks into `.git/hooks/post-commit` and `.git/hooks/post-checkout` (following `gitdir:` redirects for linked worktrees). Existing hook scripts are preserved — the codegraph block is appended or replaced in place, and reruns are idempotent. Each hook runs `codegraph hook-run <kind> .`, which performs the same safe incremental refresh as watch mode (with the full-rescan fallback) and regenerates any exports configured under `[hooks]`, appending one JSON result line per run to `.codegraph/hooks.log` so commits stay quiet. The hooks are no-ops when the `codegraph` binary is not on `PATH`.

Query several local repositories as one set through the global registry:

```bash
cargo run -p codegraph-cli -- registry-add ~/work/backend --name backend
cargo run -p codegraph-cli -- registry-add ~/work/frontend
cargo run -p codegraph-cli -- registry-list
cargo run -p codegraph-cli -- registry-query 'configs target:DATABASE_URL'
cargo run -p codegraph-cli -- registry-query 'path from:main to:load_config' --project backend
cargo run -p codegraph-cli -- registry-remove frontend
```

The registry is a machine-managed JSON file (default `<cache-dir>/registry.json`, override with `--registry-path`) mapping short project names to canonical repository roots. `registry-query` runs one query expression — any slice the query language supports, including `path from:.. to:..` — against every registered repository (or the `--project` subset) through the shared persistent cache, and returns per-project results with `succeeded`/`failed` counters. A repository that fails to scan reports its error inline instead of failing the whole run, unknown `--project` names fail loudly with the list of known projects, and re-adding the same root under the same name is a no-op.

Combine several graph artifacts into one typed graph:

```bash
cargo run -p codegraph-cli -- merge code.json docs.json --output merged.json
cargo run -p codegraph-cli -- merge incident.json --project backend --output merged.json
```

`merge` accepts exported graph JSON files and/or `--project` names from the registry (project, docs, incident, and external-system graphs alike). Nodes merge only when kind, label, and source path all match; every merged node and edge records its contributors in `merge_sources` metadata, non-conflicting metadata is unioned, and duplicate edges collapse to the highest-confidence contributor. The strategy is conflict-safe for committed artifacts: inputs are processed in sorted-by-name order and ids are reassigned deterministically, so the same inputs always produce byte-identical output — re-merging in CI or by a teammate never churns a committed file. Metadata disagreements keep the first source's value and are enumerated in the merge report (`codegraph.merge.v1`) instead of being dropped silently. With `--output` the merged graph goes to the file and the report to stdout; without it the merged graph itself prints to stdout.

Export a Markdown wiki that opens directly as an Obsidian vault:

```bash
cargo run -p codegraph-cli -- export-wiki . --output codegraph-wiki
```

`export-wiki` writes interlinked notes — `Home`, `Communities`, `Entrypoints`, `Hotspots`, `Config Flows`, and `Risks` — cross-referenced with `[[wikilinks]]` so the folder opens in Obsidian or renders in any Markdown wiki without conversion. Content derives deterministically from the existing graph reports (communities, entrypoints, hotspots, config/environment reads with their readers, and warning/error findings grouped by kind), each section states its truncation explicitly, and regeneration overwrites the same files byte-stably so the vault can live in version control.

Size up a change before merging it:

```bash
cargo run -p codegraph-cli -- pr-impact . --base origin/main --ci-state passing
cargo run -p codegraph-cli -- pr-impact . --file src/util.rs --file src/main.rs
```

`pr-impact` takes the changed-file list from `git diff --name-only <base>` (default `HEAD`, so working-tree changes; `--file` overrides skip git entirely) and maps it onto the graph as a `codegraph.pr_impact.v1` dashboard: which communities the change lands in, which shared hotspots it contains or feeds (`contains_changes` / `depends_on_changes`), the blast radius of reverse dependents with affected entrypoint/test/route counts and sample entrypoints, warning/error findings anchored in the changed code, and a deterministic risk score. `--ci-state` and `--review-state` strings are recorded verbatim so CI pipelines can stamp their context into the artifact. The same report is served by `GET /api/pr-impact` (`base`, comma-separated `files`, `ci_state`, `review_state` parameters, documented in `/api/schema`) and by the web PR Impact panel, which runs the dashboard against a base ref or explicit file list and downloads the JSON artifact.

Measure what the graph is worth on a real corpus:

```bash
cargo run -p codegraph-cli -- bench-context . --samples 20
```

`bench-context` produces a `codegraph.benchmark.v1` report with two families of numbers. Token/context savings compare the bytes (and estimated tokens at bytes/4) an agent would otherwise read — the whole corpus for a project overview, every file mentioning a name for grep-style symbol and config lookups — against the bytes of the bounded graph slice answering the same question, as per-task and total basis-point savings. Graph-query recall is measured against oracles built independently of the parser pipeline by plain text scanning (function definition headers, environment-variable read patterns), so extraction gaps surface as recall below 100% with sample misses listed for follow-up. The oracles skip test-convention paths, Rust `#[cfg(test)]` regions, and matches inside string literals, so embedded fixture code does not count as expected facts. Sampling is sorted and deterministic: the same corpus produces the same report.

Save investigation outcomes as repository memory and reuse them between sessions:

```bash
cargo run -p codegraph-cli -- memory-save 'configs target:DATABASE_URL' . --outcome useful --note 'reader is load_config' --node-id 42
cargo run -p codegraph-cli -- memory-list . --only-stale
```

Aggregate that memory into repository lessons:

```bash
cargo run -p codegraph-cli -- memory-reflect .
```

The `codegraph.reflection.v1` report groups records into per-node lessons (resolving node ids to current graph labels and flagging ids that no longer exist), separates dead-end queries to avoid repeating and corrections that override earlier conclusions, counts outcomes, and emits explicit stale-source warnings for every record whose fingerprint no longer matches the source tree.

Memory records live in repository-owned `.codegraph/memory.jsonl` with outcome (`useful`, `dead-end`, `corrected`), a free-text lesson, linked graph node ids, and the project fingerprint hash at save time. A record stores the reference as it was given, so the durable `cg-*` id still names the same definition when the memory is read back after an edit; records written earlier with numeric ids read as `n42`, and `memory-reflect` resolves either form to the node it names. Listings compare each record against the current fingerprint and mark records from a changed source tree as `stale`, so outdated conclusions are flagged instead of silently trusted.

Install agent guidance into a repository in one command:

```bash
cargo run -p codegraph-cli -- install-agent . --platform all
```

`install-agent` writes a `codegraph` server entry into `.mcp.json` (preserving other servers; conflicting entries need `--force`) and adds marker-delimited guidance blocks to `CLAUDE.md` and/or `AGENTS.md` (`--platform claude|codex|generic|all`) that nudge assistants to query the graph — `ask`, `query`, `journey`, `impact`, `refactor-context`, `report` — before broad file reads. Reruns are idempotent: marker blocks are replaced in place and user content around them is preserved; the JSON result lists created, updated, unchanged, and skipped files. With `--hooks`, it also writes assistant hook configuration snippets under `.codegraph/hooks/`: a ready-to-merge Claude Code `PreToolUse` block (a non-blocking nudge on Grep/Glob, with a documented strict `exit 2` variant), a portable `pre-search-nudge.sh` for any command-hook runner, and a README explaining where each snippet goes — CodeGraph generates the snippets but never runs hooks itself.

Serve the graph to coding assistants over MCP (stdio):

```bash
cargo run -p codegraph-cli -- mcp .
```

Register it in an assistant's `.mcp.json`:

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "codegraph",
      "args": ["mcp", "."]
    }
  }
}
```

The MCP server speaks newline-delimited JSON-RPC on stdin/stdout, scans the project once at startup (using the shared persistent cache), and exposes `query_graph`, `get_node_card`, `get_neighbors`, `shortest_path` (ranked journeys with fragile hops), `workflow`, `insights`, `impact`, `report`, `refactor_context` (one-shot refactor bundle), `ask` (natural-language questions), `source_search`, and `memory_save`/`memory_list`/`memory_reflect` (fingerprint-stamped investigation memory in `.codegraph/memory.jsonl`) tools with JSON Schema input contracts — so assistants can query the repository graph, request refactor context, and persist lessons between sessions instead of reading raw files. The same 14 tools are served over the authenticated HTTP transport at `POST /api/mcp`.

Serve the same MCP tools over HTTP for shared team graph access:

```bash
cargo run -p codegraph-server -- --root . --api-token team-secret
curl -s -X POST http://127.0.0.1:3765/api/mcp \
  -H 'authorization: Bearer team-secret' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"impact","arguments":{"target":"main"}}}'
```

`POST /api/mcp` handles one MCP JSON-RPC message per request (`initialize`, `ping`, `tools/list`, `tools/call`) through the same engine as the stdio transport, so both surfaces answer identically. It sits under the server's existing optional bearer-token protection — configure the token once and every MCP call is authenticated like the rest of the API. Notifications return `202` with no body; batch arrays are rejected with a JSON-RPC error. Register it in assistants that support HTTP MCP servers with the URL plus an `Authorization: Bearer <token>` header; HTTP calls are covered by the server's access logs.

Keep a local audit trail of how the graph is interrogated:

```toml
# .codegraph/config.toml
[query_log]
enabled = true          # audit logging is off until the repository opts in
log_responses = false   # response previews are a separate opt-in
redact = ["acme-internal"]  # extra literal terms to mask
```

```bash
cargo run -p codegraph-cli -- query 'nodes kind:function label:main' .
cargo run -p codegraph-cli -- ask "Where is DATABASE_URL read?" . --log-queries
cargo run -p codegraph-cli -- query-log . --action ask --limit 20
```

With `[query_log]` enabled (or `--log-queries` forced for one run), CLI `query`/`ask`/`journey` commands and MCP tool calls append `codegraph.query_log.v1` records to repository-owned `.codegraph/query-log.jsonl`: surface (`cli`/`mcp`), action, query text, outcome, and duration. Privacy defaults are conservative — logging is disabled until opted in, responses are never stored unless `log_responses = true` (and are then redacted plus truncated to a bounded preview), sensitive `key=value`/`key:value` assignments such as tokens and passwords are masked before writing, and configured `redact` terms are stripped everywhere. Nothing leaves the repository; `query-log` lists the most recent records with per-action counts for review.

Run the web application:

```bash
cargo run -p codegraph-server -- --root .
```

Open:

```text
http://127.0.0.1:3765
```

Run as a local desktop application:

```bash
cargo run -p codegraph-ui -- --root .
```

The desktop launcher starts `codegraph-server` on an automatically selected
local port, waits for `/api/health`, and opens CodeGraph in a native WebView
window. Pass `--port <port>` to choose a stable local backend port, or attach to
an already running backend:

```bash
cargo run -p codegraph-ui -- --server-url http://127.0.0.1:3765
```

If the launcher cannot find a sibling `codegraph-server` binary, it falls back
to `cargo run -p codegraph-server -- ...` from the workspace. Packaged builds
can pass `--server-bin <path>` or set `CODEGRAPH_SERVER_BIN`.

The server stores persistent graph cache records outside the project by default
(`CODEGRAPH_CACHE_DIR`, `XDG_CACHE_HOME/codegraph`, `~/Library/Caches/codegraph`
on macOS, or a temp fallback). Use `--cache-dir <path>` to choose a directory or
`--no-cache` to force every request to rescan.

Build and run the container image:

```bash
docker build -t codegraph:local .
docker run --rm -p 3765:3765 \
  -v "$PWD:/workspace:ro" \
  -v codegraph-cache:/cache \
  codegraph:local
```

The image starts `codegraph-server` as a non-root user with `/workspace` as the
default project root and `/cache` as the persistent graph cache directory.
Use `--max-file-size <bytes>` to cap per-file reads. Source/manifest files above
the limit remain visible as skipped file nodes and produce `skipped_large_file`
insights. A file the scan could not open at all — permissions, a broken
symlink — is reported as `unreadable_file` with the reason, so a file with no
facts is never silently empty. When a selected project has `.codegraph/config.toml`, the server uses
that repository-owned scan policy for API and web requests.
Completed scan and semantic enrichment jobs are retained in bounded in-memory
stores so large graph results do not accumulate without limit. Use
`--max-scan-jobs <count>` and `--max-semantic-jobs <count>` to tune the retained
history; active queued/running jobs are not pruned.
Scan and semantic enrichment jobs also have independent concurrency limits. Use
`--max-scan-concurrency <count>` and `--max-semantic-concurrency <count>` to tune
how many long-running scans and LSP enrichment runs may execute at once; excess
jobs stay queued until a slot is available. `/api/health` reports active and
available slots for both pools.
JSON API request bodies are capped by `--max-api-body-bytes <bytes>` and the
effective limit is published through `/api/capabilities` for clients and agents.

Expose multiple local repositories to the web project selector by repeating
`--project`. Requests remain constrained to the configured roots unless
`--allow-any-path` is set explicitly:

```bash
cargo run -p codegraph-server -- --root . --project ../service-a --project ../tooling
```

Protect API routes with a URL-safe token when binding beyond trusted local use:

```bash
CODEGRAPH_API_TOKEN=change-me cargo run -p codegraph-server -- --root . --host 127.0.0.1
curl -H 'authorization: Bearer change-me' 'http://127.0.0.1:3765/api/health'
```

When token protection is enabled, the embedded web UI prompts for the token and
stores it in browser local storage for later same-origin API and SSE requests.

Scan API:

```bash
curl 'http://127.0.0.1:3765/api/projects'
curl 'http://127.0.0.1:3765/api/capabilities'
curl 'http://127.0.0.1:3765/api/schema'
curl 'http://127.0.0.1:3765/api/health'
curl 'http://127.0.0.1:3765/api/metrics'
curl 'http://127.0.0.1:3765/api/scan-options?path=.'
curl 'http://127.0.0.1:3765/api/languages'
curl 'http://127.0.0.1:3765/api/lsp'
curl 'http://127.0.0.1:3765/api/semantic-readiness?path=.'
curl 'http://127.0.0.1:3765/api/semantic-plan?path=.'
curl 'http://127.0.0.1:3765/api/semantic-plan?path=.&work_item_limit=25'
curl 'http://127.0.0.1:3765/api/semantic-plan?path=.&work_status=ready&work_capability=definitions'
curl 'http://127.0.0.1:3765/api/semantic-batch?path=.&work_status=ready&work_capability=definitions'
curl 'http://127.0.0.1:3765/api/semantic-batch?path=.&work_status=ready&work_capability=workspace_symbols'
curl -X POST 'http://127.0.0.1:3765/api/semantic-patch' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_status":"ready","work_capability":"definitions","responses":[]}'
curl -X POST 'http://127.0.0.1:3765/api/semantic-apply' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_status":"ready","work_capability":"definitions","responses":[]}'
curl -X POST 'http://127.0.0.1:3765/api/semantic-enrich' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_item_limit":25,"work_status":"ready","work_capability":"definitions"}'
curl -X POST 'http://127.0.0.1:3765/api/semantic-jobs' \
  -H 'content-type: application/json' \
  --data '{"path":".","work_item_limit":25,"work_status":"ready","work_capability":"definitions"}'
curl 'http://127.0.0.1:3765/api/semantic-jobs?status=running&limit=20'
curl 'http://127.0.0.1:3765/api/semantic-jobs/semantic-1/events'
curl 'http://127.0.0.1:3765/api/semantic-jobs/semantic-1/result'
curl -X DELETE 'http://127.0.0.1:3765/api/semantic-jobs/semantic-1'
curl 'http://127.0.0.1:3765/api/report?path=.&fail_on=warning&insight_limit=100'
curl 'http://127.0.0.1:3765/api/report?path=.&format=markdown&fail_on=warning&insight_limit=100'
curl 'http://127.0.0.1:3765/api/coverage?path=.'
curl 'http://127.0.0.1:3765/api/scan?path=.'
curl 'http://127.0.0.1:3765/api/cache-diff?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/cache-chunks?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/incremental-plan?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/incremental-scan?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/incremental-merge-preview?path=.&limit=50'
curl -X POST 'http://127.0.0.1:3765/api/incremental-update?path=.&limit=50'
```

The scan response includes `cache.status` as `hit`, `miss`, or `disabled`. A miss also reports whether the graph reached the cache: `cache.stored` is `false` with a `cache.store_error` when the directory could not be written, so a cache that never fills is told apart from a project that keeps changing. Cache diff responses and the web Cache Diff panel explain the previous and current project fingerprints without performing a full graph scan, including reuse strategy, changed file counts, reusable file/byte counts, and reuse ratios for incremental-scan planning. Incremental scan responses include the plan plus a focused graph for changed current files, while full-scan actions still return a complete graph. Merge preview responses use persistent impact and chunk indexes to remove cached scopes for changed or removed files, then add changed-file rescans. Surface-stable partial previews, such as body-only edits with the same graph signatures and no incoming cross-file blockers, are marked complete and can be stored; structural changes remain incomplete until a full scan rebuilds cross-file incoming edges.
Incremental update responses use `POST` because they may persist the graph cache when the result is complete; incomplete partial previews report `stored: false` with the reason and leave the previous cache record untouched. Incomplete previews also include structured blocker counters for removed paths, incoming cross-file edges, and graph-surface additions/removals so agents and the web UI can explain why the cache was not updated.
Cache chunk responses list the persistent per-file node and edge scopes currently stored in the graph cache, including compact node/edge id previews for agent diagnostics.

Export API:

```bash
curl 'http://127.0.0.1:3765/api/export?path=.&format=dot'
curl 'http://127.0.0.1:3765/api/export?path=.&format=ndjson'
curl 'http://127.0.0.1:3765/api/export?path=.&format=graphml'
curl 'http://127.0.0.1:3765/api/export?path=.&format=mermaid_html'
curl 'http://127.0.0.1:3765/api/export?path=.&format=cypher'
```

The web Export panel can also download `Report JSON`, which uses `/api/report`
instead of the raw graph export route.

Async scan job API:

```bash
curl -X POST 'http://127.0.0.1:3765/api/scan-jobs' \
  -H 'content-type: application/json' \
  -d '{"path":"."}'
curl 'http://127.0.0.1:3765/api/scan-jobs?status=complete&limit=20'
curl 'http://127.0.0.1:3765/api/scan-jobs/scan-1'
curl -N 'http://127.0.0.1:3765/api/scan-jobs/scan-1/events'
curl 'http://127.0.0.1:3765/api/scan-jobs/scan-1/result'
curl -X DELETE 'http://127.0.0.1:3765/api/scan-jobs/scan-1'
```

Analysis APIs:

```bash
curl 'http://127.0.0.1:3765/api/graph?path=.&node_limit=250&kind=function'
curl 'http://127.0.0.1:3765/api/node-context?path=.&node_id=1&edge_limit=80'
curl 'http://127.0.0.1:3765/api/node-card?path=.&node_id=1&edge_limit=80&source_context=5&insight_limit=8'
curl 'http://127.0.0.1:3765/api/focus?path=.&node_ids=1,2&edge_indexes=0&edge_limit=200'
curl 'http://127.0.0.1:3765/api/summary?path=.'
curl 'http://127.0.0.1:3765/api/architecture?path=.&group_limit=50&edge_limit=200'
curl 'http://127.0.0.1:3765/api/language-dependencies?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/surprising-links?path=.&limit=50'
curl 'http://127.0.0.1:3765/api/hotspots?path=.&limit=25'
curl 'http://127.0.0.1:3765/api/communities?path=.&limit=25'
curl 'http://127.0.0.1:3765/api/entrypoints?path=.'
curl 'http://127.0.0.1:3765/api/entrypoint-traces?path=.&search=server&depth=4'
curl 'http://127.0.0.1:3765/api/entrypoint-workflows?path=.&search=server&depth=4&block_limit=200'
curl 'http://127.0.0.1:3765/api/check?path=.&fail_on=warning&kind=dependency'
curl 'http://127.0.0.1:3765/api/insights?path=.'
curl 'http://127.0.0.1:3765/api/insights?path=.&severity=warning&kind=dependency&limit=25'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=nodes kind:function label:main'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=neighbors label:main direction:out depth:2 edge_kind:calls' \
  --data-urlencode 'compact=true'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=dependents label:load_config depth:3'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=path from:main to:load_config depth:6'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=unreachable language:rust'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=unreachable scope:errors search:LegacyError'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=diagnostics severity:error language:rust'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=annotations key:domain value:payments direction:out edge_limit:300'
curl --get 'http://127.0.0.1:3765/api/query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=insights severity:error'
curl --get 'http://127.0.0.1:3765/api/ask' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=Where is DATABASE_URL read from the environment?'
curl --get 'http://127.0.0.1:3765/api/ask' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=Кто вызывает load_config?' \
  --data-urlencode 'compact=true'
curl --get 'http://127.0.0.1:3765/api/source-search' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=DATABASE_URL' \
  --data-urlencode 'path_filter=src'
curl --get 'http://127.0.0.1:3765/api/explain-edge' \
  --data-urlencode 'path=.' \
  --data-urlencode 'source=main' \
  --data-urlencode 'target=load_config' \
  --data-urlencode 'kind=calls'
curl 'http://127.0.0.1:3765/api/trace?path=.&label=main&depth=3'
curl 'http://127.0.0.1:3765/api/workflow?path=.&label=main&depth=10&block_limit=200&compact=true&max_fanout=8'
curl 'http://127.0.0.1:3765/api/workflow?path=.&label=main&edge_kind=calls&confidence=heuristic&block_kind=call'
curl 'http://127.0.0.1:3765/api/journey?path=.&from=main&to=load_config&depth=8&paths=3'
curl --get 'http://127.0.0.1:3765/api/workflow-query' \
  --data-urlencode 'path=.' \
  --data-urlencode 'q=nodes kind:function search:main' \
  --data-urlencode 'edge_kind=calls'
curl 'http://127.0.0.1:3765/api/entrypoint-workflows?path=.&search=server&depth=4&block_limit=200&limit=25'
curl 'http://127.0.0.1:3765/api/entrypoint-workflows?path=.&entrypoint_kind=route&depth=4&block_limit=200&limit=25'
curl 'http://127.0.0.1:3765/api/dependents?path=.&label=load_config&depth=3'
curl 'http://127.0.0.1:3765/api/trace-config?path=.&target=DATABASE_URL&depth=6'
curl --get 'http://127.0.0.1:3765/api/trace-errors' \
  --data-urlencode 'path=.' \
  --data-urlencode 'target=failed to load data' \
  --data-urlencode 'depth=6'
```

`/api/graph` supports `node_offset`, `node_limit`, `edge_offset`,
`edge_limit`, `path_prefix`, `kind`, `search`, `language`, `item_kind`,
`edge_kind`, `confidence`, `edge_relation`, and `edge_source`.
Returned edges connect nodes in the returned node page.

Source preview API:

```bash
curl 'http://127.0.0.1:3765/api/source?path=crates/codegraph-cli/src/main.rs&start_line=1&end_line=20'
```

At the current stage, supported source languages are detected by extension:

- Rust: `rs`
- Python: `py`, `pyw`
- JavaScript: `js`, `mjs`, `cjs`
- TypeScript/TSX: `ts`, `mts`, `cts`, `tsx`
- Go: `go`
- C: `c`, `h`
- C++: `cc`, `cpp`, `cxx`, `hpp`, `hh`, `hxx`
- Dart/Flutter: `dart`, plus `pubspec.yaml` package metadata
- PHP: `php`, `phtml`
- Bash/shell: `sh`, `bash`, `zsh`, `ksh`, `Makefile`
- Markdown/ADR/RFC repository docs: `md`, `markdown`, `mdown`, `mkdn`
- Plain-text knowledge files: `txt`, `text` (excluding manifest conventions)
- SQL schema/migration files: `sql`

Planned repository-knowledge features inspired by Graphify-style workflows:

- Deeper document ingestion beyond Markdown, deeper SQL query analysis, migration ordering, ORM/database-config linking, optional non-code document ingestion, and SVG export.
- Task-oriented investigation guides (investigate a bug, trace a config value, plan a refactor) with live-verified commands are in [`docs/GUIDES.md`](docs/GUIDES.md).
- The detailed Graphify parity map is tracked in [`docs/GRAPHIFY_PARITY.md`](docs/GRAPHIFY_PARITY.md), including already covered capabilities, gaps, priorities, and compatibility principles.
- The Phase 9 end-to-end feature audit (all CLI commands, API endpoints, web panels, and MCP tools exercised against this repository) is recorded in [`docs/FEATURE_AUDIT.md`](docs/FEATURE_AUDIT.md) with 13 filed findings.
- The Phase 9 roadmap sweep disposition of every remaining unchecked item (3 dropped, 2 re-scoped, 27 kept and scheduled) is recorded in [`docs/ROADMAP_TRIAGE.md`](docs/ROADMAP_TRIAGE.md).
- The CLI/API/web/MCP parity matrix — which analyses are reachable from which surface, and which gaps are intentional — is maintained in [`docs/SURFACE_PARITY.md`](docs/SURFACE_PARITY.md).

Supported package manifests:

- Rust/Cargo: `Cargo.toml`
- JavaScript/TypeScript/npm-compatible: `package.json`, `package-lock.json`, `pnpm-lock.yaml`
- Go modules: `go.mod`
- Python: `requirements.txt`, `pyproject.toml`, `setup.py`, `setup.cfg`, `Pipfile`
- PHP/Composer: `composer.json`, `composer.lock`
- C/C++ package managers: `vcpkg.json`, `conanfile.txt`, `CMakeLists.txt` `find_package(...)`

Manifest dependencies are normalized into canonical package nodes with a stable
`package_id` metadata value such as `cargo:serde` or `python:fastapi`. Source
imports link to the same hubs wherever package identity is stable — Rust `use`
roots, npm/Dart module specifiers, Python module roots, PHP vendor namespaces,
and Go module prefixes — through heuristic `depends_on` edges with
`relation: package_import`, and the matched import facts carry the hub's
`package_id`, so manifests, lockfiles, and code imports share one canonical
package node per ecosystem. Individual
manifest files connect to those package nodes with `depends_on` edges; the edge
metadata records whether the declaration is runtime, dev, optional, peer,
build, or test dependency data when the manifest format exposes that
distinction, plus the raw `dependency_version` constraint or locked package
version when the manifest declares one. Version-bearing dependency edges also
set `dependency_version_kind` to `constraint` or `locked` so lockfile versions
do not masquerade as conflicting manifest constraints. Cargo
`workspace = true` dependencies resolve to the root workspace constraint when
one exists; path-only workspace dependencies omit `dependency_version`.

Manifest and runtime entrypoints are represented as `entrypoint` nodes linked from the
repository root with exact `entrypoint` edges. Examples include Cargo binaries,
npm scripts, Python project and setup.py/setup.cfg console scripts, Composer
scripts, Composer binaries, Dockerfile instructions, Docker Compose services,
and Kubernetes workloads or Ingresses.
When a manifest target can be mapped back to code, the entrypoint node also
emits `references` edges with metadata such as `relation=entrypoint_file` or
`relation=entrypoint_function`; traces follow these edges before continuing into
regular call, import, config, environment, dependency, and error-flow edges.
A node that declares a block spans it: a workflow job runs from its name
through its last step, a Compose service through its ports and volumes, a
Kubernetes document through the resource it declares, and a `CREATE TABLE`
through its columns.
Compose services also emit `depends_on`, `reads_environment`, `reads_config`,
and `references` edges for service dependencies, `environment`, `env_file`,
published `ports`, and local bind `volumes` entries without storing literal
environment values in graph metadata.
Kubernetes Services emit `references` edges to matching workloads when their
selectors match workload pod-template labels, keeping runtime traffic surfaces
connected to the entrypoint graph.
Kubernetes Ingresses emit backend `references` edges to Service refs and then
to matching Service manifests, preserving host/path route context for traffic
entrypoint investigation.
Entrypoint trace reports run this traversal for all matching entrypoints so a
project's startup flows can be compared without manually copying labels.
An environment variable or config key is one node however many places name
it, so the workflow job that sets `GITHUB_TOKEN` and the function that reads
it meet there. What each site says about the variable — the value kind, the
line, the job or service that assigns it — travels on the edge, because
another job can assign something else and a read says nothing about a value
at all.
Config traces specialize that graph traversal by matching `config` and
`environment` nodes, listing direct readers, and returning shortest known paths
from manifest entrypoints to the reader and final read edge.
Error traces use the same upstream traversal for `may_error` edges, listing the
function or construct that may produce an error and returning the shortest known
entrypoint path to the final error edge.

## Workspace

```text
crates/
  codegraph-core/      graph schema and shared domain types
  codegraph-analysis/  summaries, entrypoints, and trace subgraphs
  codegraph-parser/    Tree-sitter syntax extraction
  codegraph-lsp/       LSP server discovery and semantic enrichment foundation
  codegraph-indexer/   project scanning and graph construction
  codegraph-storage/   persistent graph cache and project fingerprints
  codegraph-cli/       command-line interface
  codegraph-server/    HTTP API and embedded static web app
  codegraph-web/       browser UI assets
  codegraph-ui/        native desktop launcher with an embedded WebView
```

## Development

Contribution guidance and local verification commands live in
[`CONTRIBUTING.md`](CONTRIBUTING.md). The GitHub Actions workflow runs the same
core checks for pushes and pull requests.

## Design Principles

- The CLI and UI must use the same graph model.
- Agent-facing output must be stable, structured, and documented.
- Every extracted fact should carry a confidence level.
- Language support should degrade gracefully from semantic to syntactic to heuristic analysis.
- The project should prefer proven language tooling such as Tree-sitter and LSP servers instead of inventing parsers from scratch.
