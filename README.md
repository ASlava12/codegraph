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
- A layout is an entrypoint of its own. Next.js runs `layout`, `template`, `error`, `loading`,
  `not-found` and `default` around the routes beneath them, and SvelteKit its `+layout` and
  `+error`: none has a URL, and without them the components a layout renders -- eleven in taxonomy
  -- are reached by nothing.
- `baseUrl` in a tsconfig makes every directory under it importable by name: `import { User } from
  "types"` is the `types/` directory beside the tsconfig, which eleven of taxonomy's files write.
  Only the directories that are really there become aliases, so a package name still reads as a
  package, and an alias matches at a path boundary -- `types` says nothing about `typescript`.
- `<TailwindIndicator />` is how a JSX runtime calls a component -- it compiles to
  `jsx(TailwindIndicator, props)` -- and rendering one is now using it. A lower-case tag is the
  platform's and a dashed name a custom element, so neither is read as a component the project
  declares.
- A TypeScript project with React components is written in two languages, `.ts` and `.tsx`, and they
  share one set of symbols: every import from a module into a component crossed a line the resolver
  would not, so a Next.js app resolved 32 of its 494 calls. The same holds for `.js` and `.jsx`.
- Next.js, Nuxt and SvelteKit declare a route by where the file sits, and a project written that way
  had no entrypoints at all -- no routes, and nothing for a workflow, a journey or the coverage
  finding to start from. The layout is read as the framework reads it: `app/api/users/route.ts`
  exporting `GET` and `POST` is two routes, `app/blog/[slug]/page.tsx` is `GET /blog/:slug`, a
  `(marketing)` segment groups files without naming a URL, `[...rest]` catches what is left, a
  `pages/api` handler serves whatever method it is sent, and SvelteKit's `+page.svelte` and
  `+server.ts` work the same way. Which framework the project uses is read from its manifest --
  `app/` is a PHP directory as often as a Next.js one.
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
- How a project is laid out is answered by what the graph can answer: "how is this project
  organized" searched it for the word `project` and found five nodes, and "what are the main
  subsystems" answered with the entrypoints because it says `main`. Both now answer with the busiest
  parts of the graph, under a rule that says the communities and architecture commands are what
  really group a project into areas. An ordinal is not a name either -- "what should I read first"
  no longer filters the entrypoints by `first`.
- Asking what breaks settles a question before any topic word does: "what breaks if I change the
  SongResource endpoint" was answered with koel's HTTP routes and "what would break if I remove the
  Setting config" with its configuration reads, because the topic rules key on nouns a symbol's name
  may sit beside. Both now answer with what depends on the name.
- Haskell states its programs in the package's `.cabal` file, and they are read the same way:
  `executable shellcheck` with `main-is: shellcheck.hs` is the program, a `test-suite` states a test
  program under its own `hs-source-dirs`, and a `library` states none. shellcheck's coverage finding
  is gone -- its entrypoints were its shell scripts and CI jobs.
- A tool's own configuration is not what the project ships, wherever it sits: openzeppelin's
  `eslint.config.mjs` imports `@eslint/js` and mastodon's `vite.config.mts` imports `browserslist`,
  neither of which their package.json declares. Both read as notes now, the way a script under
  `scripts/` already did -- on the CLI and in the browser alike.
- A catalog the database itself provides is not a table the project forgot to declare: mastodon asks
  `pg_class` how large a table is before importing into it, and postgres, MySQL and SQLite all name
  their catalogs plainly (`pg_*`, `sqlite_*`, `information_schema.*`, `mysql.*`,
  `performance_schema.*`).
- A library has no program for a configuration read to be reachable from, so "not reachable from
  any entrypoint" describes the project's shape rather than the read: spdlog's `is_color_terminal`
  reads `TERM` and gin's `resolveAddress` reads `PORT`, and neither project ever starts itself.
  Those read as notes now, the way the coverage finding already does for a library.
- A version a test fixture pins is not the project disagreeing with itself: axios keeps typescript
  4.9.5 under `tests/module/cjs` to prove it still compiles there. The versions outside the tests
  are what have to disagree, and an example app still counts against the project -- an example that
  pins an older version of the library it shows off has gone stale -- while two examples disagreeing
  with each other stays the examples' business.
- A file contains what its own lines hold. A namespace many files reopen (C# `namespace`, PHP
  `namespace`, Ruby `module`) is one node, and every declaring file said it *contained* that node --
  putting one file's span inside another 1020 times in koel and 122 in mastodon. The file the
  declaration sits in contains it; the files that reopen it declare it.
- A program with no extension states its language in its first line, and four interpreters the
  corpus uses were not read: `ruby` (mastodon keeps thirteen programs in `bin/`), `lua`/`luajit`/
  `resty` (kong's `bin/kong` is the gateway's whole CLI), `ocaml` and `dash`, along with `elixir`,
  `julia` and `Rscript`. The entrypoint names the interpreter that runs it.
- OCaml states what it builds in `dune` files, one per directory, and they are read as the manifests
  they are: `(executable (name main))` in `bin/dune` is `bin/main.ml`, `(executables (names a b))`
  states two programs, and `(test ...)`/`(tests ...)` state the test programs. The dune repository
  goes from eighteen entrypoints to 355, and the share of its functions an entrypoint reaches from
  1% to 9%. A program a test fixture declares, whose module the test writes as it runs, reads as a
  note rather than a warning -- as anything a test declares does.
- A `.c` file is read as C. Only a header's extension is ambiguous -- `.h` is C's, C++'s and
  Objective-C's alike -- and sniffing a `.c` file for what it declares read redis's `class =
  getClientType(c)`, an assignment to a variable named `class`, as a C++ class: `networking.c` and
  `config.c` were parsed as C++, which put their `addReplyError` and `addReply` in a different
  language from the calls to them. redis resolves 1961 more calls, and the cycle its blocking code
  really has becomes visible.
- A Java static import says whose method a bare call means: `import static
  com.google.common.truth.Truth.assertThat` makes `assertThat` Truth's, which is the only thing that
  tells it from the `assertThat` retrofit declares in a test helper -- 663 calls read as the
  helper's. A static import names a member of a class, so the file it points at is the class
  (`retrofit2/TestingUtils.java`), and the project's own static imports still reach what they name.
  gson reclassifies 1749 calls as leaving the project, petclinic 97.
- A call the resolver refuses says why: an edge to an unresolved placeholder carries
  `unresolved_reason=not_imported` when the module cannot reach the name at all, beside the
  `local_value` a call through a bound value already carried. 1445 of koel's calls say it.
- A Python module calls what it declares or imports too, and a bare name never means a method --
  pytudes writes a `print` method on its grid class, and 58 notebook calls to the builtin were
  answered by it. `%run other.ipynb` is an import written in IPython's dialect: it runs the other
  notebook in this one's namespace, so the dependency is now an edge in the graph, and a file that
  runs one (or writes `from x import *`) keeps the old name matching, because its bindings cannot be
  listed. Across flask, requests, django-oscar and pytudes that is about 660 calls that no longer
  reach a module their file never names, and 51 that now reach the one it does.
- A JavaScript or TypeScript module calls what it declares or imports, and nothing else by a bare
  name: `const h = originalH` in vue's Teleport spec and `const { trigger } = useContextMenu()` in
  koel's context menus bind names the file never imports, and matching by name alone sent those
  calls into other modules -- 204 of koel's cross-file calls, 201 of vue's, 34 of axios's. A
  `require` binds a name the same way an import statement does (`var compileETag =
  require('./utils').compileETag`), and a file that states no import at all may be a classic script,
  where a bare name really can come from anywhere, so the rule asks only files that import
  something.
- A call's name cannot hold a parenthesis, a quote or a space. When one survives, the callee was an
  expression rather than a name -- terraform's `(*StackChangeProgress_Hook)(x)`, nlohmann's
  `(std::numeric_limits` and `j.template get`, redis's `"/sbin/$sysctl"`, vue's `(transformSrcset as
  Function)` -- and the label is a fragment of source. 812 of terraform's call nodes were of that
  kind, and no resolved edge is lost by refusing them.
- A call into OTP or Elixir's standard library is the platform's: cowboy calls `gen_tcp:recv` 283
  times and `lists:keyfind` 238, ecto `Enum.reverse` 79 and `Enum.reduce` 72. Those read as calls
  the resolver had failed on; a module the platform ships is not a dependency the repository failed
  to hold.
- A C++ cast is written like a call and is the language's own: nlohmann/json writes 437
  `static_cast` and 70 `reinterpret_cast`, which read as calls the scan had failed to resolve, along
  with `strcasecmp`, `qsort`, `strtol` and the rest of the C library it leans on.
- A type parameter is not a type: every generic declaration writes `T`, `A`, `K`, `V`, and no
  project means its own type by them. Reading them as references pointed 10756 of cats' 13896 at
  whatever happened to be called `A`, which is why Scala and Objective-C are left out of this
  reading altogether -- Scala writes type parameters everywhere, and Objective-C's references land
  on the platform's `NSString` and `NSURL` rather than on a project's own classes.
- Go and C# too: a Go parameter, field or variable names the struct it holds, and C# writes its
  types as plain identifiers, so what a declaration states sits in its `type` field and the classes
  it derives from in its base list. gin's `Context` -- the type its whole framework is written
  against -- goes from 27 references for 208 types to 660, and `impact` on it names 447 dependents,
  twelve routes and 393 tests; Polly goes from 395 to 2253 and Newtonsoft.Json to 5984.
- Java and Rust types are reached the same way -- a field's type, a parameter's, a return type, a
  generic argument, `extends`/`implements`, and the type an `impl` block is written for. gson goes
  from 236 references into its 763 classes to 4116, ripgrep from 63 to 1157, serde to 2146, and
  `impact Gson` names 824 dependents and 720 affected tests where it named none.
- TypeScript types are reached the same way: a parameter's annotation, a property's, a return type,
  a generic argument, and the interfaces a class extends or implements. vue's
  `ComponentInternalInstance` -- the interface its whole runtime is written against -- had nothing
  pointing at it and now has 150 references, and `impact` on it names 1339 dependents and 486
  affected tests. The name in `interface Foo {}` declares the type rather than referring to one.
- An Elixir module attribute is a declaration, not something the module does: the grammar reads what
  follows the `@` as a call, so ecto filed 356 calls to things named `doc`, `type` and `spec`. And
  `fun.(new, current)` invokes whatever the variable holds -- the label it produced, `fun.`, names
  nothing at all, and ecto writes 82 of them. Its unresolved calls fall from 2556 to 2213.
- The coverage report says what it passed over, by extension. "5757 files were not indexed" says
  nothing a reader can act on; mastodon's are 4276 `.svg` -- the assets, rightly left alone -- and
  310 `.haml`, a language this scan does not read, and only the breakdown tells one from the other.
  A repository with a great many distinct suffixes counts the rest together rather than growing
  without bound.
- A `.jsx` file is javascript and is read. The extension was not among those any adapter claimed,
  so the file was walked and never parsed: mastodon's hundred components held nothing at all, and
  414 functions in 3140 nodes were invisible. The javascript grammar reads JSX -- the same component
  saved as `.js` parses without a syntax error -- so the extension belongs to it rather than to a
  dialect of its own.
- A configuration trace shows the program's own readers first. The order was the file walk and
  `.github/` sorts early, so mastodon's `RAILS_ENV` -- read by six workflow jobs, one spec helper,
  `bin/dev` and `config/boot.rb` -- answered with the jobs and none of the program under the default
  limit. Its own code comes first now, then its tests, then what builds and runs it.
- Whose file it is decides how loudly a parse failure is said. redis failed its own quality gate --
  `check` exits 2 -- over `deps/lua/test/life.lua` and `deps/tre/tests/retest.c`: upstream's test
  data, in upstream's tree, not valid utf-8 on purpose. A manifest the scan could not parse was
  already quieter when the project did not own it; a source file is now too, and redis's gate
  passes.
- An edge explanation says why the edge is there, not only how: `references` covers a type
  reference, an import that resolved to a file, an entrypoint's handler and a function that writes
  another inside it, and only the relation tells them apart. It sat in the evidence list rather than
  in the sentence a reader reads first, so `create_app references index` now reads `create_app
  references (encloses) index`.
- A definition written inside another is reached through the one that holds it, and only the
  metadata said so: flask's `route` returns a `decorator` that calls `add_url_rule`, and asking for
  the way from `route` to `add_url_rule` found no path at all. Every decorator, factory and
  callback-returning function was a dead end. The nesting is an edge now -- the span picks which
  holder when a file writes several by one name -- and flask gains 605 of them.
- A boundary with the suite on one side is neither a seam to extract nor coupling to fix: a suite
  leans on the program by design, and mastodon's specs reach `app/models` 937 times and `app/lib`
  832, which topped the seams worth untangling. Both lists now put the boundaries inside the program
  first, so mastodon leads with `config -> app/controllers` and `app/controllers -> app/models`.
- A subsystem is what refers to itself, so that is what ranks the communities. They were ordered by
  node count, and mastodon's `public/` -- 3953 nodes with four edges among them -- led every
  subsystem of the program. A group whose parts refer to nothing, not even each other, follows even
  the suite: flask leads with `src` now, then docs and examples, with its tests and its one-node
  `<computed name>` last.
- An architecture area matters by how much of the program it holds, and the order decides what
  survives `--group-limit` as well as what a reader sees first. It was ranked by file count, so
  mastodon's `public/` -- 3949 files, four symbols -- led the map. Areas holding none of the program
  now come last and a suite follows the program whatever it weighs: flask's tests hold 1395 symbols
  against its `src`'s 443, and the map leads with `src`.
- Which directory is a container and which is an architecture area is a question about the program,
  so it is asked of the files that hold some of it. mastodon keeps 3949 static assets under
  `public/` and four symbols among them, and counting those hid the fact that `app/` -- 3219 files
  and 11795 symbols -- is where the program lives: its models, controllers, services and javascript
  were one box. It now divides into 26 areas, while terraform's 71, koel's 13 and flask's 6 stay as
  they were.
- A Python property and a JavaScript accessor are read the same way: `@property def description` is
  reached by writing `obj.description` and `get inSFCRoot()` by writing `parser.inSFCRoot`.
  django-oscar's functions with no caller fall from 1270 to 1032, vue's from 351 to 338 and
  requests' from 121 to 109.
- A Dart getter is read rather than called: `bool get isEmpty => length == 0` is written as a method
  and reached by writing `x.isEmpty`, so no call edge can ever point at one and "nothing calls it"
  says nothing about it -- the same reasoning that already leaves a value a factory built alone. The
  http package's functions with no caller fall from 3086 to 2185.
- A C file hands a function over by writing its name: `iter->_next_fp = all_values_iter_next`
  stores one and `aeCreateFileEvent(.., redisAeReadEvent, ..)` passes one, and both make it run when
  the time comes. Only a function the same file declares counts, so nothing is guessed at. redis's
  functions with no caller fall from 4620 to 3280, and 2900 more calls resolve across redis,
  nlohmann and spdlog.
- Ruby calls a method by writing its name, with no parentheses and no receiver to tell it from a
  variable: `filtered_statuses` calls `default_statuses` and `hashtag_scope`, and nothing recorded
  any of it. Only a name the same class declares counts, so nothing is guessed at, and a name the
  body binds is a variable whatever the class also declares. mastodon's private methods with no
  caller fall from 1495 to 382 and 2018 more of its calls resolve.
- Only so many dependency cycles are worth listing, and which ones survive the cap matters: the
  fiftieth found was the last kept, so a cycle across files -- the kind the finding exists to
  surface -- was dropped while ones inside a single file stayed. The most severe survive now, which
  brings back seven warnings the corpus always had: three in terraform's `jsonformat/differ`, two in
  gqlgen, one in dune and one in mastodon. The edges that close a cycle are collected once, for the
  cycles that are kept, rather than once per component.
- A haskell data constructor is named after itself rather than after a keyword -- `data
  VariableState = Dead Token String | Alive` -- and building a value of a type something reaches
  uses them, the same way building a class runs its constructor. shellcheck's orphan functions fall
  from 3101 to 2968.
- Zig binds `const server = @import("Server.zig")` and writes `server.` at every call, so the name
  a reader asks with differs from the file's only in case; an unambiguous stem now answers either
  way, and `server` reaches `src/Server.zig` with 243 dependents. The same import names the file by
  the name its label already carries, which is not a module name and is no longer recorded as one.
- A lua file is a module and so is a python one, and neither states its own name: `require
  "kong.tools.table"` and `import oscar.core.loading` do. A file now carries the name its importers
  call it by -- the one most of them write, when they disagree -- so `kong.tools.table` reaches the
  file with 1756 dependents and `requests.models` the one with 253, where before the first found
  nothing and the second found the import statement that mentions it.
- `module ShellCheck.Analytics where` states the name every import and every qualified call writes,
  and nothing recorded it -- nor did any of shellcheck's 5985 functions know which module it was in.
  Both are read now: its ambiguous calls fall from 308 to 11, and asking about the module answers
  with 714 dependents instead of nothing.
- A module that *is* the file holds everything the file declares, which a module written inside a
  file cannot claim. A Haskell type carries no owner, and shellcheck's `Analytics` is reached
  through its types as much as through its functions.
- An OCaml file is a module, and nothing in the graph said so: only `module X = struct` was read,
  and that produced no label either, so dune had no OCaml module node at all and `Path` answered as
  something in dune-rpc. A file now declares the module it is -- `path.ml` is `Path` -- and a module
  written inside a file gets its name from the binding. dune gains 5737 module nodes.
- A go package is written by the last segment of its directory, so `addrs` is how a reader asks
  about `internal/addrs` -- and a fixture of the same name is not what the question means, whether
  it is terraform's `tools/defect-detector/testdata/.../tfdiags.go` or the directory beside it.
- A go package is a directory, and nothing points at one: asking what depends on `internal/addrs`
  answered nothing while the `NewDefaultProvider` it holds is called 1139 times. The repository
  holds every file directly, so a directory stands for the files whose path it prefixes, and a file
  for what it declares. `internal/addrs` reports 4244 dependents, `internal/tfdiags` 5021,
  `kong/tools` 2125, and oscar's `loading.py` rises from 1476 to 1751.
- Changing a module means changing what it declares. A call reaches the function, not the module
  around it, so `impact Path` had nothing to report while 2725 things used what path.ml holds; a
  module's own definitions now seed the walk without counting as dependents of themselves. The same
  answers which of twenty-two modules named `Path` the question means.
- `SearcherBuilder::new()` names the type outright, so the three `new` that ripgrep's JSON printer
  declares for types of its own are not it, however near they sit. The escape that keeps a
  definition in the caller's own file reachable is for a module the graph could not name -- OCaml's
  and julia's -- not for a language where every definition carries the type it belongs to. ripgrep's
  ambiguous calls fall from 1982 to 1596 and serde's from 1071 to 951, at the cost of two edges.
- A C or C++ file reaches a declaration through the headers it includes, and nothing else.
  nlohmann keeps its sources under `include/` and an amalgamated copy under `single_include/`, so
  every macro and method is declared several times; a caller under `include/` includes only the
  first. Its own includes are written in angle brackets -- the build puts `include/` on the
  compiler's path -- and none of them reached another header, so a bracketed header written as a
  path is now looked for in the repository as well. `<fmt/format.h>` is written the same way and
  belongs to a library, so a miss stays quiet. nlohmann's ambiguous calls fall from 5830 to 5264,
  redis's from 2294 to 1796 and spdlog's from 1691 to 1598.
- A header-only library writes one header in two halves -- `x.h` and `x-inl.h`, each including the
  other by design -- which is one unit rather than a cycle, as a Dart `part` and its library are.
- A module the project declares answers only with what belongs to it. dune's `List.map` is the
  standard library's -- stdune's list.ml declares plenty but not `map` -- and matching on the name
  alone offered fifty-nine other modules' `map` instead. A nested path is read from its head, so
  `Path.Build.append_source` still finds path.ml, a method the named class inherits still answers,
  and a definition the caller's own file writes is reachable whatever module path the call spells.
  dune's ambiguous calls fall from 11905 to 4353 with 2090 more resolved.
- A method on a type from a package the file never imports cannot be the one meant. terraform
  declares `Diagnostics.HasErrors` in `internal/policy` and in `internal/tfdiags`, and every file
  that calls it imports exactly one of the two. Its ambiguous calls fall from 18005 to 13019 with
  4413 more resolved -- and a real dependency cycle between four files of
  `internal/command/jsonformat/differ` surfaces, which is why the corpus sweep rises by one.
- A julia file is not a module: DataFrames writes `module DataFrames` once and `include`s the rest,
  so only 98 of its 1387 functions sat inside the block that names them all. The include list is the
  only thing that says which module an included file's functions belong to, and following it settles
  them honestly -- its ambiguous calls fall from 2995 to 1697 with 1259 more resolved.
- A note left in a build script is about the build. terraform's `scripts/goimportscheck.sh` says a
  Bash 4 feature is missing on macOS and swift-argument-parser's `Tools/generate-manual/` carries
  three of its own; reading them as loudly as a note in the program buries the ones somebody can
  act on, the same reasoning that already quiets vendored code and tests. Eight findings across the
  corpus become notes.
- PHP keeps the receiver in a field of its own too, and states its types where the class is
  written: `public function handle(LogRecord $record)`, `private FormatterInterface $formatter`,
  `$handler = new StreamHandler(..)`. monolog declares nine `handle` and 170 calls chose between
  them; its ambiguous calls fall from 542 to 318 and koel's from 2663 to 2551.
- A ruby call keeps only the method in its label and states the constant it was written through
  beside it, so the class a call names has to be asked for rather than read off the label. When the
  project declares that class and none of the candidates is one of its methods, the call means the
  class itself: `Account.new` is not the `new` action twenty-three of mastodon's controllers
  declare. Its ambiguous calls fall from 13988 to 12107, with 722 more resolved and none lost.
- Swift loses its receiver the same way, and states its types the same way: a parameter always
  names one and `let manager = Manager()` names what it builds. Alamofire's ambiguous calls fall
  from 2143 to 2032 with 123 more resolved.
- Kotlin writes the callee as one navigation expression, so `sink.writeUtf8(..)` reaches the graph
  as `writeUtf8` and what it was written through is lost. The receiver is now recorded and read
  against what the file states -- a parameter always names its type, and `val buffer: Buffer =
  Buffer()` names one twice. okio's ambiguous calls fall from 5403 to 4863 with 603 more resolved.
- Java states the type of everything it binds -- `Gson gson = new Gson();`, `void check(JsonReader
  reader)` -- and none of it had ever been read, so a call through a receiver had only its method
  name to match on. gson declares fourteen `fromJson` and 563 calls chose between all fourteen; its
  ambiguous calls fall from 4465 to 2146 with 2365 more resolved, and retrofit's from 3016 to 2560.
- `using Assert = Newtonsoft.Json.Tests.XUnitAssert;` renames a type, and every call written
  through the alias means the type it stands for. Newtonsoft's tests write 2199 `Assert.AreEqual`,
  each a choice between the three `AreEqual` the project declares; its ambiguous calls fall from
  6579 to 4295 and 2284 more resolve.
- `expect class Buffer` in commonMain and `actual class Buffer` in jvmMain are one class written
  twice, and a source set is a directory of its own -- so what tells the two halves apart is exactly
  what the overload test asked them to share. okio's ambiguous calls fall from 5749 to 5403 and 346
  more resolve.
- `F.map(fa)(f)` goes through a value whose type is a type parameter, so nothing the project
  declares can be named by it. cats writes 178 of those and each was reported as a choice between
  every `map` in the repository; 744 of its ambiguous calls stop claiming a choice that was never
  there.
- A header defines a macro once per side of an `#ifdef`, and a caller means the macro rather than
  one of the two arms. nlohmann keeps three copies of its header, so `JSON_THROW` was a choice
  between six definitions of one name; 487 of its calls, 594 of redis's and 95 of spdlog's now
  resolve.
- A zig function belongs to the container that holds it -- `const Server = struct { pub fn init }`
  -- and a zig file is a container too, which is what `const analysis = @import("analysis.zig")`
  binds. All 1215 of zls's functions now say whose they are.
- A function knows the module its file declares. Erlang states it once at the top --
  `-module(cowboy_req).` -- and OCaml names one after the file itself, so neither encloses anything
  a walk up the tree can find: cowboy's 3924 functions and dune's 14636 belonged to nobody, and
  every name two files shared was a choice the graph could not make. cowboy's ambiguous calls fall
  from 2744 to 809 and dune's from 17791 to 12119. A julia module is written out, so only what one
  encloses belongs to it -- a file that states none leaves the question open rather than inventing
  a module from the file name.
- A C++ method written inside its class body has only the class around it to say whose it is: a
  method defined outside names the owner in the declarator, but nlohmann and spdlog write nearly
  every one inline and 96% of their functions knew no owner. nlohmann's ambiguous calls fall from
  6895 to 6422 and its resolved calls rise by 470.
- An Elixir function belongs to the module that declares it, and a module is a `defmodule` call
  rather than a block the grammar names -- so the walk that finds a class or an impl block never saw
  one, and ecto's 3029 functions knew no module at all. Two modules writing the same name were one
  name with two answers: ecto's ambiguous calls fall from 5000 to 1692 and its resolved calls rise
  from 2068 to 5294.
- OCaml has no global namespace: a bare name is the standard library's, the file's own, or one an
  `open` brought into scope. Nobody in dune opens `Predicate_lang`, yet the `not` it declares
  answered 436 calls to the language's; `open Dune_sexp.Decoder` is read now, so the names that
  really do come from a module still find it. 2772 of dune's ambiguous calls resolve or stop
  claiming a choice that was not there.
- A nix file's `let` bindings are its own: the language has no global namespace to reach another
  file's through, and `lib`, `pkgs` and `builtins` are what the evaluator hands a module rather than
  anything the project declares. home-manager's `modules/lib/dag.nix` had answered 132 calls to the
  `map` primop and its termite module 27 to nixpkgs' `optionalString`; 290 such edges go.
- A callee that navigates through an expression -- `args.into_iter().map(..)` -- reaches the graph
  as the name alone, because the receiver is not part of what is called. A call now says when its
  receiver was dropped that way, so a method every value has is still read as the language's:
  ripgrep's `Match::map` loses the 101 iterator `map`s it had collected, along with `is_empty`,
  `unwrap` and `parse`.
- Java writes a call's receiver in a field of its own, so `Arrays.asList(..)` reaches the graph as
  `asList` and the class it was written through is lost. The receiver is now recorded and read
  against the file's imports the way Go's and Python's qualifiers already are: 201 of gson's calls
  and 160 of retrofit's stop landing on a project method that shares only a name.
- A bare Scala call means a method the caller already has -- its own, one it inherits, or one its
  file declares. cats writes `def f` on a case class in `FreeT.scala`, and 833 bare `f(...)` calls
  across the repository, each a function its own body was handed, read as that one method.
- A Lua file says what a qualified call means where it binds the module: `local pl_path = require
  "pl.path"` is the only thing that tells `pl_path.exists` from any project function named `exists`.
  The name the require binds is now recorded and read, so a call through it reaches the module the
  file named or nothing at all — 1219 of kong's calls are outside the repository and said so, and its
  LDAP plugin's `decode` loses the 291 callers it had from `cjson.decode`.
- `table.concat` is Lua's, whatever a project names its own helpers. A call written through the
  runtime's own namespace is now answered by the runtime rather than by a definition that shares
  only the tail: kong's `kong/tools/table.lua` had 123 callers it never had, and 571 of its call
  edges were resolved or weighed against the wrong definition.
- A serializer's `attributes :actor, :object` names the methods it renders with, so mastodon's 35
  `def actor` are called by the class that lists them rather than by nobody. Its orphan functions
  fall from 5109 to 4672.
- `before_action :set_account` is Rails invoking a method of the controller that wrote it, and the
  same holds for a model's `after_commit :notify` and a `validate :check`. mastodon names 342 methods
  that way and every one read as a method nobody calls; eleven of its controllers declare
  `set_account`, and the class the registration sits in is what tells them apart. Its orphan
  functions fall from 5617 to 5109.
- Building a class runs its constructor, and a framework builds most of them: koel's container
  instantiates 208 classes whose `__construct` no `new` in the repository names. A class a route, a
  type hint or a `new` reaches is built; one nothing points at is still worth reporting, and its
  constructor with it. koel's orphan functions fall from 2085 to 1877.
- A test is run by its runner, and no edge records that: 3160 of vue's 3937 orphan functions, 4381
  of terraform's 12120 and 684 of this repository's own 1018 were tests, burying the code somebody
  could actually delete. A function in a test-like file is one, and so is a Rust `#[test]` -- or a
  helper inside the `#[cfg(test)] mod tests` a crate keeps beside its code, where the path says
  nothing. The corpus's orphan functions fall from 106357 to 62758, and this repository's from 1018
  to 225, which are the functions it passes as values.
- Go resolves an unqualified name inside its own package, and a package is a directory: gqlgen
  declares `isBinInPath` in several and every call to it was ambiguous. That is the language's rule
  rather than a guess about where a name lives.
- busted hands a Lua spec its cases and its assertions -- `describe`, `it`, `lazy_setup`,
  `assert.same` -- and munit and ScalaCheck do the same for a Scala suite with `test`, `checkAll` and
  `forAll`. kong's unresolved calls fall from 14083 to 9942 and cats' from 2189 to 1654, which is
  what the two projects really do reach for.
- A Zig file is a struct and a Java file is its class, so a name can belong to a file rather than to
  anything the file declares: zls writes `Server` in `src/Server.zig`, and `impact Server` found
  nothing at all. A name that matches exactly one file's stem names that file -- 65 dependents for
  `Server`, 81 for `DocumentStore`.
- A C# program is every statement its file writes outside a declaration, not the first of them: with
  only the first in its span, the calls the rest make belonged to the file, and eShopOnWeb's three
  programs reached nothing at all. Following one now walks 16 blocks deep into the services it
  registers, and its entrypoints reach 115 of 471 functions rather than 103.
- Every test framework writes its cases as callbacks -- `describe('x', () => { .. })` in JavaScript,
  `test("x", function() .. end)` in Lua, `test("x") { .. }` in Scala -- and a callback is otherwise
  a body that runs when something invokes it. In a test file the callback is the test: koel's 498
  spec files made 1456 calls between them and now make 6504, vue's 248 make 7579, zod's 194 make
  6347, kong's 1011 make 16725 and cats' 255 make 4246.
- A spec is the blocks it is written in. `describe .. do it .. do expect(subject.call).to be end
  end` is what the file runs, and a Ruby block is otherwise a callback that runs when something
  invokes it -- so mastodon's 1312 spec files had 3163 calls between them and "which tests cover
  this" had almost nothing to answer with. Its spec files now make 25410 calls, and `impact Account`
  names 83 affected tests rather than 56.
- Every word before a make rule's colon is one of its targets, so a word that is not a target means
  the line is not a rule: requests writes `$(error The '$(SPHINXBUILD)' command was not found. ..
  https://www.sphinx-doc.org/)` in its docs Makefile, and the URL's colon turned the sentence in
  front of it into make targets called `The`, `command` and `was`.
- `SPDLOG_NAMESPACE_BEGIN` opens a namespace through a macro the grammar has never seen, and
  everything after it is read as something else: 169 files across spdlog and nlohmann/json are
  written that way, and spdlog's central `logger` class had no node at all. Blanking a line that is
  a bare uppercase macro ending in `_BEGIN` or `_END` keeps every other line where it was, and
  `impact logger` names 175 dependents and 75 affected tests.
- `class SPDLOG_API logger { .. }` is how a C++ library exports a class, and the grammar reads the
  whole declaration as a function returning `class SPDLOG_API` called `logger`: the class had no node
  at all and every member inside it read as a free function. A class or struct with no body of its
  own, followed by a plain name rather than a parameter list, is a class an export macro stands in
  front of.
- A Python class is reached by what it inherits and what annotates it. django-oscar declares 1697
  classes and 14% of them had anything pointing at them, because a Django project states its
  structure through inheritance -- `class Basket(AbstractBasket)` -- and nothing read it. Its
  reached classes go from 248 to 415, requests' from 48 to 63 of 96, and `impact Session` on
  requests names 81 dependents and 72 affected tests. `str`, `Optional` and `Exception` are the
  language's, not a class the project declares.
- A Haskell signature names the types a function works with -- `runChecker :: Parameters -> Checker
  -> [TokenComment]` -- and a Julia annotation the type a value has: `df::AbstractDataFrame`. Neither
  was read, so shellcheck's `Parameters` and DataFrames.jl's `AbstractDataFrame` had nothing pointing
  at them; `impact Token` now names 179 dependents, `Parameters` 37 and `AbstractDataFrame` 394.
- Swift and Erlang reach what they name too. `extension Session { .. }` adds to a type declared
  elsewhere and declares none, so Alamofire's `Session` -- the type its whole API is written around
  -- had four declarations and nothing pointing at any of them; `impact Session` now names 497
  dependents and 476 affected tests, and Alamofire's type nodes fall from 595 to 399 while its
  references rise to 3413. An Erlang module is named by the remote call that reaches it --
  `cowboy_req:reply(..)` -- and by `-behaviour(cowboy_handler)`: cowboy's references go from 134 to
  1023 and `impact cowboy_req` names 294 dependents.
- `struct client { .. }` declares a type and `struct client *c` names one: reading both as
  declarations gave redis 183 nodes for `redisCommand` and 3635 types for its 1492 names, so no
  reference could choose a target. `typedef struct client { .. } client;` is one declaration written
  twice, and the name a program uses is the typedef's. C and C++ now read a type the way every other
  language does -- redis's references go from 39 to 11241, nlohmann/json's from 87 to 798, spdlog's
  from 75 to 672 -- and `impact robj` names 1583 dependents where it named none. Where two files
  declare the same name, "what breaks if I change it" means the declaration something depends on.
- An Elixir module is reached by the alias that names it: `alias Ecto.Changeset`, `use Ecto.Schema`,
  and the module on the left of a qualified call. ecto declares 390 modules and nothing pointed at
  any of them; `impact Ecto.Changeset` now names 129 dependents and 99 affected tests, `Ecto.Query`
  244 and 133. A `defstruct` states the shape of the module it sits in rather than a type of its
  own, and naming it after that module declared `Ecto.Changeset` twice -- which is why every
  reference to it was ambiguous.
- Kotlin types are reached the same way: a parameter's type, a property's, what a function returns
  and what a class extends. okio declares 358 types and four references pointed into them, so "what
  breaks if I change `Buffer`" answered with nothing -- it now names 22 dependents, `Source` 274 and
  140 affected tests. okio's references go from 4 to 875 and retrofit's to 1912. A Kotlin source set
  is a directory too, and okio declares `Buffer` once per platform it builds for.
- A Go package is a directory, and a type written inside one is its own: terraform declares
  `Backend` in seventeen packages, one per remote state backend, so every reference was ambiguous
  and `impact Backend` answered with nothing. It now names 142 dependents and 134 affected tests,
  and terraform's Go type references go from 10446 to 14488.
- Ruby classes are reached by the constants that name them. mastodon declares 2083 classes and
  modules and nothing pointed at any of them, so "what breaks if I change `Account`" answered with
  nothing at all: `impact Account` now names 536 dependents and 56 affected tests, `Status` 213 and
  32, `User` 161 and 51. A Ruby program names a class by its constant -- `Account.where(..)`, `class
  X < ApplicationRecord`, `include Payloadable` -- and a class states the constant path it answers
  to, so `module Admin; class AccountsController` is `Admin::AccountsController` and not the
  `AccountsController` beside it. A name written on its own means the class that answers to exactly
  that name: mastodon's `Account` is the model, not one of the fifteen stubs its migrations declare
  or the one its maintenance task does, all of which end with the same word. References into its
  Ruby classes and modules go from none to 5628.
- A PHP test case gets its assertions from the class it extends: `$this->assertSame(..)` is
  PHPUnit's and `$mock->shouldReceive(..)` is Mockery's, and guzzle writes 1800 such calls, koel a
  thousand more and monolog nine hundred. The runner is asked last, so a project that writes an
  assertion helper of its own keeps its callers -- guzzle declares 27 and koel 32. guzzle's
  unresolved calls fall from 9391 to 5906, koel's from 9665 to 7124.
- Solidity states its own primitives: `require` and `revert` state a condition the call has to meet,
  `keccak256` and `ecrecover` are the chain's, and `abi.encode` is how a contract encodes what it
  sends. A Foundry test inherits its assertions and cheatcodes from the base contract -- `assertEq`,
  `bound`, `vm.assume`. 887 of openzeppelin's 3012 unresolved Solidity calls were one of those.
- An entrypoint reaching the code that serves it is the architecture, not a surprise: a route sits
  in `routes/` and its controller in `app/`, so the link crosses an area by construction, and eight
  of koel's top ten "surprising links" were a route reaching its own `__invoke`. What ranks now is a
  real crossing -- koel's `config/koel.php` calling `find_ffmpeg_path`.
- A table is a SQL entity wherever it is declared. mastodon writes its schema in Ruby migrations and
  some of its indexes in raw SQL, and each table took the language of the file that declared it, so
  an index and the table it belongs to looked like a link across languages. Code reaching a table is
  a link from a program to its data rather than a name matched across languages, and mastodon's
  cross-language findings fall from 436 to 427 with the nine false ones gone.
- A module the language ships answers only where the project declares none. 1144 of dune's
  unresolved calls named an OCaml module that comes with the compiler -- `Printf.sprintf`,
  `Unix.getenv`, `Filename.concat` -- while dune's own `stdune` declares `String`, `List` and
  `Array` of its own, and 19,897 of its qualified calls resolve into the project that way. Asking
  the language only after resolution has failed keeps both answers right, and does the same for
  Julia's `Base` and `Core`.
- Zig reaches its standard library through the constant a file binds with `@import("std")`: 775 of
  zls's 2955 unresolved calls were `std.` -- `std.debug.assert`, `std.ArrayList` -- and 174 more
  resolved into zls itself, so `std.debug.print` claimed the project's own `print` as its target and
  `std.testing.expectEqual` claimed its `expectEqual`. The standard library answers for its own now,
  and zls's unresolved calls fall to 2176.
- `params.require(:source)` is how a Rails controller reads a parameter, not how a file requires a
  library: `require` is Kernel's, and a bare call is the only way to reach it. mastodon filed fifteen
  imports of things called `params.require(:post)`.
- A flake states the flakes it is built from, flat -- `inputs.nixpkgs.url = "github:NixOS/nixpkgs"`
  -- or inside an `inputs = { .. }` block, and home-manager writes both across five files. It was the
  last project in the corpus whose dependencies came from nowhere; every one of the 44 now declares
  what it needs.
- The rest of the ecosystems state it their own way, and each was read by nobody: cowboy declared
  nothing at all, and ecto, kong, shellcheck, DataFrames.jl, dplyr, cats and zls declared only the
  GitHub Actions their workflows use. A `mix.exs` writes `{:telemetry, "~> 1.0"}` with `only: :test`
  saying what a dependency is for; a `rebar.config` names each one by the atom that opens its tuple,
  and the tuples inside it -- `{git, ..}`, `{tag, ..}` -- name nothing; a `.rockspec` writes the
  rock and its version in one string; a `.cabal` file lists what each stanza needs, so a package can
  be a library's and a test suite's at once; a Julia `Project.toml` names its `[deps]` and its
  `[extras]`; an R `DESCRIPTION` separates `Imports:` from `Suggests:`; `build.sbt` names Maven
  coordinates with `% Test` marking the tests'; and `build.zig.zon` names a dependency by the field
  that holds it. Between them: ecto 7, cowboy 2, kong 33, shellcheck 17, DataFrames.jl 41, dplyr 29,
  cats 10, zls 4.
- A .NET project was read by nobody either: eShopOnWeb and Newtonsoft.Json declared no dependency at
  all, and now declare 49 and 13. A `<PackageReference>` names a package and, where the repository
  does not manage versions centrally, the version it wants; `PrivateAssets="All"` marks a package
  that builds the project and ships with nothing, and a test project's packages are what its tests
  need.
- Nor was a JVM manifest read: gson declares 19 dependencies across four `pom.xml` files, petclinic
  30 in a Gradle build, okio 22 and retrofit 50 in a Gradle version catalog. A `<dependency>` names
  a group and an artifact and `<scope>test</scope>` says what it is for, while a
  `<dependencyManagement>` block pins a version for whoever declares the dependency and declares
  none of its own. A Gradle line states a coordinate as a string, or names an entry of
  `gradle/libs.versions.toml`, whose `[libraries]` table states the coordinate and whose
  `version.ref` names an entry of `[versions]`.
- Nothing read a Ruby manifest, so a Ruby project's dependencies were known from nowhere: mastodon
  declares 154 gems in its `Gemfile` and sinatra 43 across a `Gemfile` and its gemspec, and "which
  packages does it depend on?" answered both with the GitHub Actions their workflows use. A `gem`
  line states a name and, second, the version it wants; a `group :development, :test do` block says
  the gems inside it are not what the program runs on, and so does `group: :development` on the line
  itself. A workflow's action is now ranked last in that answer -- it is how the project is built,
  not what the program is built on.
- A Go program spells the variables it reads as constants: `os.Getenv(envLogFile)` reads
  `TF_LOG_PATH` wherever terraform declares that name. The read filed a hole -- `<computed name>` --
  for 62 of terraform's 299 environment reads, and 45 of them named a constant the project binds to
  a literal. Those reads now say which variable they read: terraform's computed reads fall to 18 and
  the variables it is known to read rise from 102 to 124. A key a loop builds still names nothing to
  look up, and stays a hole, which is the honest answer.
- What a program reads is its configuration; how it is linted is not, and neither is what a
  demonstration of it configures. "What configuration does it read?" answered koel with twelve
  GitHub Actions run steps before naming a single Laravel config key, and flask with its Celery
  example -- flask reads 35 values in `src/` and 22 in `examples/`, and the walk reaches the
  examples first. The program's own configuration comes first now, then the examples and tests,
  then the repository's tooling.
- A document is not code. A markdown file's headings look like the symbols a file holds, so every
  project in the corpus was told that its `README.md` "contains markdown code but is not reachable
  from any entrypoint", and "which code is unused?" answered koel with
  `.github/copilot-instructions.md` and four of its headings before naming a single PHP file. The
  answer now opens with the code, and where the question is asked of every node rather than of
  files, code is ranked ahead of the documents and configuration around it.
- `AlbumController::class` is how PHP writes down a class it does not build, and koel writes 111 of
  them in its routes alone -- every Laravel route names its controller that way, as does a container
  binding and a config file's provider list. None was read, so "what breaks if I change
  AlbumController" answered with the class and nothing else; it now names the route file and the 171
  routes it holds. koel's type references go from 3798 to 4892.
- A .NET program starts in a file of statements. C# lets one file per project write statements
  outside any declaration and the compiler wraps them in `Program.Main`, which is how eShopOnWeb
  starts all three of its programs -- and with no `Main` to find, nothing said where any of them
  begins. And "where does the program start" was answered with everything except the programs: a
  program the parser recognises is a Function node an `Entrypoint` edge points at, so the
  `entrypoints` query, which `ask` runs, filtered every `main` out while the `entrypoints` command
  and the report listed them. Both read it the same way now.
- A class says what it inherits from, and a route reaches the action its parent declares. Eleven of
  mastodon's settings pages declare no action of their own -- `class BrandingController <
  Admin::SettingsController` inherits `show` and `update` -- so the route reached nothing and the
  flow stopped there. Every class node now carries `extends` (Ruby, PHP, JS/TS, Python, C#, Java:
  923 of koel's classes, 1469 of django-oscar's), and route resolution follows the chain. And
  `with_options only: [:index] do` hands its options to every resource inside it, which mastodon
  writes six times: reading the resources without them claimed 42 routes it does not serve.
  mastodon's routes that reach code go from 620 to 648, and 781 routes remain of the 856 first
  counted -- the difference is what it never served.
- What a Rails router states about its resources, read five ways it was not. `only: []` declares
  none of the seven, and reading it as "no restriction" had mastodon serving 33 routes it does not
  -- `/admin/users`, `/backups/:id`, a whole `/api/v1_alpha/accounts` set. A singular `resource
  :setup` is served by `SetupsController`, named for the set. `module: :terms_of_service` puts the
  controller one module deeper without moving the path. `get :export` inside a resource block is
  that resource's `export` action, which the line never names. And a route written inside a
  `concern` block is a template served wherever `concerns:` names it, not where it is written. A
  member route's id is `:id` -- `:backup_id` is what a resource nested inside one is given -- which
  corrected 75 of mastodon's paths. Its routes that reach the code serving them went from 457 to
  620.
- ASP.NET declares a route three ways, and two of them were invisible. A minimal API writes the verb
  into the method name -- `app.MapGet("api/catalog-items", ..)` -- so the `.get(` every other route
  call ends in never appeared. A Razor Page states its URL by where it sits: `Pages/Basket/Index`
  serves `/Basket`, `Areas/Identity/Pages/Account/Login` serves `/Identity/Account/Login`, and the
  `.cshtml.cs` beside it says which methods it serves, `OnGet` and `OnPost` at a time. A Blazor
  component writes the path out: `@page "/admin"`. eShopOnWeb went from 25 routes to 52, and its
  entrypoints now reach 100 of its 468 functions rather than 37. Nothing needs a manifest to confirm
  a Razor Page, because `@page` at the top of the file states it.
- A step of a flow says where it happens. One node stands for every call to a name the resolver
  cannot place, and it carries the span of whichever call site minted it -- 14620 of koel's 17732
  calls into such a node named a file other than the one the call is written in, so following a
  route in Flow opened somebody else's file. The edge that reached the step answers it: the caller's
  file, at the line the call is written on. Reading `route POST /api/posts` in taxonomy now points
  `db.post.count` at `app/api/posts/route.ts:55`, where it is written, rather than at a sibling
  route.
- `import('./Home.vue')` loads a file; it is not a call to a function named `import`. That is how a
  router reaches a page it loads on demand, and koel filed 168 of them as calls and reached none of
  the pages -- 186 of its dynamic imports now reach the file they load, and the three that do not
  name npm packages. A specifier the program builds -- ``import(`./pages/${name}.vue`)`` -- names
  nothing to resolve and is left alone. Reading them found five true packaging faults nothing else
  had: koel loads `pusher-js` and `laravel-echo` from its app while declaring both as dev
  dependencies, vue's compiler does the same with `sass` and `@babel/types`, and mastodon imports
  `tesseract.js-core`, which its `package.json` never mentions.
- A name the file's environment hands it is provided, not missing: `defineProps` is expanded by the
  compiler that reads a `<script setup>` block, and `describe` is handed to a test file by its
  runner. Neither is imported, and reading them as resolver failures buried the ones somebody can
  act on -- koel's unresolved calls drop from 1392 to 1027 and vue's from 1775 to 1410. The macro
  wins even where the repository exports a function by that name, which vue does.
- A value a factory builds is a declaration other files call: vue writes most of its public API as
  `export const onMounted = createHook(MOUNTED)`, a component library its variants as
  `const buttonVariants = cva(..)`. Neither was in the graph, so 523 of vue's calls resolved to
  nothing and `impact onMounted` had nothing to answer with -- it now names 109 dependents and 94
  affected tests. vue's functions went 5039 -> 5326 and its resolved calls 45% -> 48%, taxonomy's
  170 -> 297 and 26% -> 36%. Only what a module declares counts: `const rows = getRows()` inside a
  function body is a local variable, and the call that builds a value belongs to the value.
- Building a class runs its constructor: `new SongService($repository)` reached the class and
  stopped there, so koel's 378 `__construct` methods had no caller between them -- and a
  constructor is where a framework hands a class what it needs. 237 of koel's construction sites now
  reach one, and the same holds for `constructor`, `__init__` and `new` wherever a language names it
  that way. An object key written as a string is a name with quotes around it, and the quotes are
  syntax: `{ 'onUpdate:folderId': () => {} }` declares `onUpdate:folderId`.
- PHP classes are reached by the types that name them: `new SongService(..)` builds one, a
  constructor's type hint states the one Laravel injects, a return type names what a method hands
  back, and `extends`/`implements` name the class and interfaces a class states. None of those were
  read -- koel had two references pointing into its 1319 classes, so `impact`, `refactor-context`
  and "what breaks if I change this class" all answered with nothing. koel now holds 2796, guzzle
  1901, monolog 697, and `refactor-context SongService` names twelve dependents, two routes and
  three tests.
- PHP resolves through the class a static call is written through. `Uuid::generate()` kept only
  `generate` in its label, so a class the project declares and a package's facade looked like one
  call: `File::hash($path)` was answered by koel's own authenticator and `Cache::put` by a
  controller's `put`. The class settles it -- `Song::query()` reaches the model that declares it,
  and a class the project never declares leaves the project. Across koel that is 852 fewer ambiguous
  calls, 505 more that reach the method they name, and 1405 recorded as leaving.
- Python names its receiver too, and `self` (or `cls`) is the one whose methods are the class's own:
  `key.split(',')` is a string's and `kwargs.setdefault` a dict's, while django-oscar declares a
  `split` template filter and flask a `setdefault`. The mapping protocol (`keys`, `values`, `items`)
  stays out of the list, because a project that mimics a dict declares all of it -- requests'
  `RequestsCookieJar` does, and its eleven callers are real.
- The same holds in JavaScript and TypeScript, where the label keeps the receiver and `this` is the
  one receiver whose methods are the class's own: `str.trim()` is a string's, `Buffer.concat` node's,
  `args.map` an array's, `promise.then` a promise's -- yet axios declares a `trim`, vue a `map` and
  zod a `startsWith`, and matching on the tail gave each of them callers it never had. Names a
  project defines as readily as the platform does (`get`, `set`, `has`, `add`, `on`, `emit`, `find`)
  stay out of the list.
- A ruby call written through a value is not a project method every value already has: `params.each`
  is a hash's, `formats.include?` an array's, `{ .. }.to_json` a hash's. The label keeps only the
  method name, so mastodon's `Trends::History#each` collected 268 callers, its connection pool's
  `empty?` 134 and its IP map's `include?` 126 -- and the pool's own `@queue.size` was answered by
  the `size` it declares two lines above. A bare call means `self` and is left alone; 601 call sites
  stop reaching a method they never name.
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
- A blast radius counts a project's suite the way the project spells it. Asking whether "test" appears anywhere in a path or a name is wrong twice over: it called `latest_admin_action_log` and terraform's own `internal/moduletest` package suite code, and it never once counted mastodon's 2741 `spec/` nodes or koel's 4636 `.spec.ts` ones, because RSpec and Vitest spell their suites with a word that substring cannot see. The shared classifier splits a path into words and answers for `spec` as readily as for `test`, and a symbol is still asked about by name, because Rust keeps `mod tests` inside the module it exercises. Changing mastodon's `app/models/status.rb` used to report 1 affected test; it reports 81.
- Risk is what a change reaches of the program, not how well it is covered. Both scores counted the suite in the blast radius twice -- once among the dependents, once again as `affected_tests` -- so a project earned merge-gate risk for having tests. Changing flask's `src/flask/app.py` reaches 752 dependents of which only 161 are program, and it scored 2318; it scores 1133. `pr-impact` reports `program_dependents` beside `dependents` so the score can be read off the report, and `impact` already reports both halves. What the suite reaches is still counted and still reported -- it answers what to run, which is a different question.
- A type written inside another one records its owner, and a bare name in a different file no longer means it. `object Ior { final case class Right }` was the only `Right` cats declares, so every `Right(...)` built for scala's own `Either` reached it: 134 of its 151 references came from files about `Either`, and `Ior.Right` and `Ior.Left` were the two largest architectural hubs the project had. Asking `hotspots` what is central in cats now answers `Eval.now`, `Eq.by` and `Eval.defer`. Reaching such a type by its bare name takes an import that names it; the same file still reaches it, and no same-file reference was lost. Removing the wrong candidate also settles calls that had two: okio gained 65 `Pipe(...)` references, Polly 14, gson 9, retrofit 7.
- A name each owner declares once is not a duplicate function label. Grouping by the bare label alone, 71% of Polly's 833 groups and 69% of mastodon's 1028 were functions with an owner apiece: `visit_string` on three of serde's visitors is what implementing a trait looks like, `Setup` on three of its benchmarks is what the harness asks for, and `AccountsController.filtered_statuses` is not `Admin::Trends::StatusesController.filtered_statuses`. What is left is the name nothing tells apart -- one owner declaring it in two files, or no owner at all -- and never a constructor, whose name is its class's and which gson writes 338 times. gson 293 findings -> 28, serde 250 -> 103, cats 869 -> 0, and the message now names the owner and how many files.
- A dialect is not another language, and prose is not a program. `cross_language_heuristic_edge` warns about a name matched across two languages, and across the 44-project corpus all 1380 of its findings were one of two things it should never have counted: 1298 were documentation naming a symbol -- `CHANGELOG.md#Features` referencing gin's `With` is the docs link the ingest was asked to make -- and 82 were `.tsx` calling `.ts`, which is typescript calling typescript. Both are excluded, the rule still reports a real crossing, and the room it gave back is the point: mastodon spent 77 of its 500 insight slots on this and now shows 50 dependency cycles where 9 fit before.
- A bounded insight list hears from every kind before any kind repeats. The list is sorted by severity and then by the kind's *name*, so cutting it at 500 kept whatever sorted early in the alphabet: mastodon has 12,215 findings and its report showed all 388 `ambiguous_call_resolution` and not one of its 4,985 `unresolved_call`, 3,434 `orphan_function` or 2,114 `unreachable_source_file` -- three of its four largest categories, so a reader would conclude the project has no orphans at all. Severity still decides first (all 35 of mastodon's warnings and all 77 of terraform's are heard before any note), the chosen findings are still returned in severity-then-kind order, and the totals in `by_kind`/`by_severity` were always complete and still are. mastodon now shows 23 kinds of 23, dune 20 of 20, terraform 16 of 16.
- One error flow, one finding. `unreachable_error_flow` says what `potential_error_flow` says and adds that nothing runs it, and both were emitted for the same edge, so every unreachable flow spent two places in a bounded list to state one fact. dune wrote 1834 potential findings of which 1606 were the same edges its unreachable rule had already described; mastodon 425 of which 270 were. The potential rule now stands aside where the other one speaks -- dune's total findings 15,517 -> 13,911, mastodon's 12,215 -> 11,945, and 27,429 duplicates across the corpus. When the reachability walk finds no entrypoint to start from it says so with an empty set, and then the potential finding is all there is and still reported. The browser recomputes both rules and is changed with it.
- The scan does not claim what a program never reaches when it cannot see how the program starts. mastodon's entrypoints reach 18% of its functions -- Rails loads `app/` by convention and no edge records that -- so "not reachable from any entrypoint" was true of 2,114 of its 3,127 source files. 35 of the corpus's 44 projects are in that state and carried 36,719 `unreachable_*` findings between them, a quarter of everything they reported, while the 9 whose entrypoints do reach their code carried 625. `low_entrypoint_coverage` already says this once, in a warning, and in its own words tells the reader to treat `unreachable_*` findings as gaps in call resolution; now the per-file and per-error-path restatements stay quiet instead. An unreachable configuration read is still reported, because a key is looked up by name and the whole corpus writes 587 of them. mastodon 12,215 findings -> 9,831; flask and gin, whose coverage is fine, keep every one.
- A function an object holds is a value the program indexes, not a name anybody calls. mastodon writes its modals as `{ 'ACCOUNT_NOTE': () => import(...) }` and picks one by a key it computes, and lint-staged writes `{ '**/*.ts?(x)': () => 'yarn tsc' }` -- so `orphan_function` reported that a glob has no incoming call edge. The graph already distinguishes a value a factory built from a function a file spells out; a function bound as an object's property or assigned to one is bound the same way and now says so. 214 of mastodon's functions were newly named as values, and the orphan lists lose their false entries: zod 859 -> 631, core 338 -> 244, express 24 -> 16, koel 1789 -> 1693.
- A slash in a JavaScript call is a regular expression, not a name. `/^\\s*$/.test(value)` left `/^\\s*$/.test` as a call target, so mastodon listed `+\\.json$/.exec` and axios `/axios\\.min\\.js$/.test` among the calls nothing could resolve. All 60 such labels in the corpus are regex fragments and not one is a name. The rule asks the language first, because the other 44 slashed call labels are shell scripts naming the program they run -- `./configure`, `/usr/bin/env`, `$BIN_PATH/redis-cli` -- and those are calls worth keeping.
- A line on an edge says which file it is in. `component-contract` answered `bot= -> actor_type` with `line: 224` and no file, and so did 82,320 of mastodon's 82,816 edges that carry a line -- a line without a file is not a place, and a bare one invites a wrong guess. Two edges already carried the file, with the reason written beside them: an edge a reader can place without looking its source node up first. The five call and type-reference sites do now too, so mastodon goes from 0.6% placeable to 99%, dune and koel likewise; the answer above reads `app/models/account.rb:224`. It costs 8.5% of the graph's size (mastodon 60.1 MB -> 65.2 MB). The tail is documentation links, which already name their path.
- A command handed back names a node by the handle that survives. A numeric id is a node's position in one scan: inserting a function above a file's others moves `target` from `n9` to `n10`, so a `codegraph impact n9 .` saved from an earlier answer reports on a different function. `journey`, `impact` and `node-card` offered exactly that, including when they had just been called with the durable id. They offer `cg-...` now, and every query kind accepts a `stable_id:` term -- it existed for `nodes` alone, so `symbols stable_id:cg-...` errored and `symbols node_id:cg-...` quietly answered with nothing. Each of `node-card`'s six actions and each suggested command was run against a real project to confirm it answers.
- A trace starts from the durable handle too. Completing `stable_id:` across the query kinds left one family behind: `dependents` and `trace` read only `label:` and a number, so `dependents stable_id:cg-...` was refused and `dependents id:cg-...` answered `invalid node id` -- to the very handle the card before it had offered. They take it now, an unknown one is an error naming what it could not find rather than an empty answer, and `trace stable_id:cg-...` and `trace id:233` return the same 2 nodes and 1 edge. Verified through the MCP server as well, which is how an agent reaches this.
- A route a test declares is not an entrypoint the program offers. A library writes its routes in its tests: flask declares 297 of its 307 entrypoints there, express 205 of 218, sinatra 236 of 240. Both risk scores weighted every affected entrypoint at five, so the suite walked back in through that term after being taken out of the dependents -- 192 of express's 194 affected entrypoints were its own test routes, and 97% of its score. `entrypoint_rank` already ranks a test last whatever declares it, and the scores ask it now: express 988 -> 28, flask 1133 -> 163, sinatra 1282 -> 117, while mastodon (595) and koel (2311) do not move at all, because every entrypoint they offer is their own. `pr-impact` reports `program_entrypoints` beside `affected_entrypoints`, as it does for dependents.
- An area is a directory all the way down. `is_test_like_source_path` looks past a path's last segment on purpose -- there it is a file name, and `example.rs` is code -- but an area id has no file name in it, so flask's `examples` was never recognised as a suite and sorted above `.github` among the project's areas. Asked as the directory it is, it sorts with `tests`, where it belongs. The same question now ranks the architecture map's edges, which were ordered by weight alone: a suite touches everything, so mastodon's two heaviest were `spec -> app/models` (937) and `spec -> app/lib` (832), and they answered "how is this organised" ahead of `app/controllers -> app/models`. They are still reported, after the program's own.
- A component is what it holds. `component-dependencies app/models` answered that mastodon's models depend on nothing and nothing depends on them: it counted the edges touching the directory node, and nothing calls a directory. `impact` had already learned this -- "what a container holds is what changing it changes" -- so the rule is shared now, and the boundary answer agrees with the architecture map edge for edge: 692 from `app/controllers`, 937 from `spec`, 364 from `app/lib`. An edge between two members is inside the component, not a dependency of it. Files and modules were equally silent: `app/models/status.rb` went from 0 incoming to 254, dune's `Path` from 0 to 1870. Sharing the rule also gave `impact` the importers a directory's files have, which it walked past: terraform's `internal/addrs` has 7686 dependents rather than 4244, the difference being every `import "github.com/hashicorp/terraform/internal/addrs"` in the repository.
- A contract states every base it names. Solidity composes rather than descends -- `abstract contract PaymasterSigner is AbstractSigner, EIP712, Paymaster` reaches `EIP712._hashTypedDataV4` through its second base -- and nothing recorded any of them, so all 354 of openzeppelin's contracts stated no parent. They state all of them now, comma-joined the way a CI job's `extends` list already is, and the ancestor walk follows every one rather than the first: of openzeppelin's 911 inherited calls, the first base alone reaches 312 and all of them reach 409.
- A definition's own type is not a scope that hides it. A definition nested inside another is visible only there -- a Haskell `where` binding, a local closure -- and solidity records the contract a method is written in the same way, because that is how a reference inside it knows whose it is. The visibility rule read that as nesting, so openzeppelin's 3477 methods could be reached by nobody: 2150 of its calls were unresolved, and `_hashTypedDataV4`, declared once and called twenty times, had not one edge. A definition whose enclosing declaration is its own `owner_type` is not hidden by it -- a contract's method is inherited, and a C++ method written in its class body is still `obj.method()`. openzeppelin resolves 66% of its calls rather than 39%, unresolved 2150 -> 119, and the inheritance recorded above now answers 271 of them by owner. No other language moves by as much as half a percent.
- What a module includes it re-exports. `include M` makes M's definitions the includer's own, and dune builds whole modules that way: `src/fiber/src/fiber.ml` is 38 lines of `include Core` and module aliases, and `src/memo/memo.ml` includes Fiber in turn -- so `Fiber.return` and `Memo.return` both name something `core.ml` declares, and 781 calls between them reached nothing. 207 of dune's files include another module. What a file includes is recorded as its `extends`, which is what a class already records, so the same walk that answers an inherited method answers a re-exported one, transitively. dune resolves 61% of its OCaml calls rather than 59%: 1216 more, of which 67 become bounded ambiguity rather than a confident answer. No other language moves by a third of a percent.
- A call is test code when its own file is. "The program does not call its own tests" asked the *caller node* for its file, and a call written at a Lua module's top level has no caller with a span -- so it read as program code wherever it was written, and the rule then refused it every helper its own suite declares. kong writes 287 `helpers.get_db_utils` in `spec/` and not one reached `spec/internal/db.lua`. The call's own path is always known and is the answer. Ten languages resolve more of their calls and none fewer: nix 13.2% -> 19.9%, lua 22.1% -> 24.1%, javascript 15.1% -> 16.8%, typescript 27.7% -> 28.8%, and six more by half a point. The guard itself still holds -- a call written in `src/` still cannot mean a helper in `spec/`.
- `codegraph doctor` reports what the graph says about itself. `insights` reports on the code; a graph can be wrong in ways no finding about the code would show. It counts edges naming a node the graph does not hold, facts recorded twice (identical endpoints, kind *and* metadata -- two reads of `PATH` on different lines are two facts, not one twice), and definitions that appear to call themselves, split into the recursion that is real and the calls through another object that are not: axios's `request` "calls itself" through node's `session.request`, and its `Buffer.from` through node's Buffer. `super.x` written inside `x` is always wrong -- it means the parent's -- and is counted on its own. Across the 44-project corpus: 0 dangling edges, 0 duplicates, 6266 recursive calls, 4546 self-calls through a receiver of which 178 are `super`, and 41 projects clean.
- A `super` call means the parent, never the caller. `super.x` written inside `x` is the parent's implementation by definition, and the resolver answered with the caller's own -- the same-file preference finds it first, since it sits right there. openzeppelin wrote 174 call edges from a definition to itself that way, which `doctor` is what surfaced. Settled before that preference now, against whatever the caller's type inherits: 70 of openzeppelin's 189 reach a real parent, 119 are honestly ambiguous where several parents declare the method, and none is a self-loop. When nothing inherited declares it the parent is outside the project -- an interface, a library base -- and the call is left unresolved rather than pointed at itself.
- A community says how much of its coupling stays inside it. The counts were already there -- internal edges, incoming and outgoing external ones -- and a share is what makes them comparable across subsystems of different sizes. mastodon's `app/javascript` is 99%, which is a frontend and a backend sharing a repository; `app/helpers` is 33%, which is what a subsystem that exists to be used by others looks like; `db` is 84% and `app/models` 47%, pulled on by everything. The measure counts coupling whichever way it points, so a subsystem nothing reaches into and one that reaches into nothing both read as self-contained.
- `codegraph questions` suggests what to ask about a project, and the command that answers each one. A question the tool cannot answer sends the reader back to grep, which is what the graph exists to avoid, so the pairing is the point. Three questions, each from a measured fact: the definition most of the project depends on, the program or route it starts at, and the subsystem least able to stand on its own. Calibrating them against real projects is where the work was -- mastodon's most-depended-on module is `Rails`, which it declares in `lib/rails/engine_extensions.rb` and which absorbs all 431 references to the framework, so a name the project also declares as a dependency is skipped; its best-ranked entrypoint is `script:bin/brakeman`, a linter, so only a program or a route is offered; and its least cohesive subsystem was `app/views` at 0%, which is 359 templates referencing nothing of each other rather than a subsystem being pulled apart. It now answers `ActivityPub::TagManager`, `main`, and `app/helpers`.
- A URL is a repository too. `codegraph summary https://github.com/owner/repo` clones it once under the cache directory and reads it from there afterwards; the same for every command, since they all take a path and the URL is resolved before any of them sees one. `https://` and `git@host:owner/repo.git` name the same clone, placed by host and owner so two repositories of a name do not collide, and `file://` works the same way -- git clones from a path as readily as from a host, which is what lets this be exercised without reaching anything. The clone is not refreshed on a later run: a scan that quietly fetched would answer about code the caller never asked for, and `git -C <dir> pull` is the command for when they want the new one. Only git does the talking, with the credentials the user has already set up for it.
- A server that says nothing is not a server that answered. Running the LSP pass for the first time -- it degrades to the syntactic graph when no language server is installed, and that degradation is all the corpus has ever exercised -- `semantic-run` completed with exit 0 and 297 responses, every one of them `[]`. `semantic-patch` was honest about it (`semantic_edges: []`), but the run itself printed an array of 297 answers and said nothing about their being empty. It now counts them beside the array, on stderr so the array stays the contract `semantic-patch` reads: `297 responses: 0 answered, 297 empty, 0 failed`. The requests themselves are well formed -- correct `file:///` URI, plausible position -- and why rust-analyzer answers nothing is not yet established; the pass no longer hides the question.
- The semantic pass waits for the server to finish loading, and now it works. rust-analyzer answers `textDocument/definition` with `[]` and `textDocument/references` with `-32801 content modified` until it has built the crate graph -- both of which read as answers rather than as "ask me later" -- and the client asked immediately after `initialized`. It says when it is done, with a `$/progress` whose value is `{"kind": "end"}`; loading has phases, so the wait keeps draining until the server goes quiet, and a server that reports no progress at all costs one idle gap. On a fresh workspace with the rust-analyzer cache cleared: 0 answered of 6 before, 6 of 6 after. On this repository: 0 of 297 before, 57 of 59 after, and `semantic-apply` replaces 5 heuristic edges with semantic ones -- the first semantic edges CodeGraph has ever produced.
- A definition that lands in a dependency is not a gap in the graph. `semantic-patch` had a single reason, `no_graph_node_at_lsp_location`, for every location it could not place, and it reads as a parser miss. On this repository all 19 of them were outside the project -- the Rust standard library and crates under `~/.rustup` -- and not one was a file CodeGraph had failed to parse. Those now read `location_is_outside_the_project`, which leaves the older reason to mean what it says: the server sees a definition in a file this project owns, and the graph has no node for it. The count that remains is a measurement of parser coverage against a language server; on this repository it is 0.
- The view read the same graph worse than the command line. Fixing the severity rule at the command line left the browser computing its own, and it recomputes these kinds from the graph it is given: on an enriched scan of this repository it called 5314 unresolved calls and 39 ambiguous ones warnings where the CLI called none. Nothing caught it because every corpus gate passes `--no-semantic` and the parity check had only ever been run on a syntax-only graph -- the path a user actually takes was the one nothing looked at. Both now ask whether the pass covered the plan, parity passes on an enriched graph as well as a syntactic one, and the severity fixture pins both halves of the rule.
- A warning says the server looked and did not find it. `codegraph insights .` reported 5351 warnings on this repository where `--no-semantic` reported 3, and the cause was a fix earlier this week: an unresolved call becomes a warning once semantic enrichment has run, and until the automatic pass was repaired it had never actually run. It asks about 100 work items -- 100 of this repository's 48000 -- so 5309 calls nobody had asked a server about were being reported as calls a server could not answer. The severity now rises only when the pass covered the whole plan, which the root already records as `asked/total`, and `semantic-apply` records the same so a full run still warns. The corpus sweep is unchanged at 329, because it passes `--no-semantic` and never saw this.
- An orphan and an export are two different findings. The largest kind left was `orphan_function` at 47042, and two thirds of it is the API: 77573 of the corpus's 115277 uncalled functions are exported, and the message said so while the kind did not, so a reader asking what can be deleted was handed both. They are named apart now -- terraform's `orphan_function` falls from 3044 to 336 with 2708 moving to `export_with_no_local_caller`, and shellcheck, which exports almost nothing, keeps 2756 of its 2968. Nothing is removed and the corpus total is unchanged; what changes is that filtering by kind now answers the question that was being asked. The browser, the kind list, the severity fixture and the interface's own name for it all moved with it.
- An ambiguity only a suite reaches is not the program's. The last of the four largest kinds gets the reading the other three now have: 1301 of terraform's 3658 ambiguous calls and 1320 of nlohmann/json's 1821 are reached only from a suite, an example or a generated file. 8348 findings across 42 projects go, no project gains one, warnings stay at 329. The estimate was short by half for json and cats because the hand-written check in the probe did not know a directory simply called `tests` -- the classifier does, which is the whole reason it exists rather than being written out at each site.
- Half of a journey's hops were called fragile for having crossed a file. `journey` marks a hop `low_confidence_edge` whenever the edge's confidence is heuristic, and a call resolved by matching a name is heuristic, so on a path from `main` to `is_test_like_source_path` 16 of 25 hops were fragile and 9 of them for no other reason. The confidence is honest about how the fact was learned, but a risk summary is asked a different question: on this repository, ripgrep, Polly and okio 95-99% of name matches had exactly one definition bearing the name, and of 23515 resolved call sites across the four not one had two targets. A real guess is already recorded as `ambiguous` and a miss as `unresolved`, and a journey reports both under their own names, so a call the resolver settled on no longer adds a reason of its own -- the sample keeps its 7 genuine flags and loses the 9 that only said the call left its file. Confidence, path ranking and every other edge kind are untouched, and the corpus sweep is unchanged at 329 because a journey is not an insight.
- A test's directory takes itself away. One `cargo test --workspace` left 31 directories behind in the system temp directory, where 24135 of them had collected -- 21437 from the indexer suite alone -- because 25 indexer tests, 8 server tests and 3 pr-impact tests built a root and never removed it, and because a test that panics never reaches its own cleanup line either. The three helpers now hand back a value that removes the directory when the test lets go of it, which needed no change at 333 call sites: it derefs to `Path` and the scan entry points take `impl AsRef<Path>`. A test pins both halves, the ordinary exit and the panic. One run now leaks nothing, and the server's root carries the process id the way every other suite's already did.
- A rust `use` says whose name a call is written through. `BTreeMap::new` was matched against the 8 functions this repository calls `new` and kept as one bounded ambiguity: 395 of its 464 ambiguous calls named a standard library or dependency type, and `PathBuf::from` had picked up a project `from` outright, 12 edges of it. The builtin list already held `String::new` and not `BTreeMap::new`, which is the shape of a hand-written enumeration; the import list is the rule underneath it, and no list could name `WalkDir` or `HyperlinkSpec` anyway. Rust's own scoping makes the answer safe -- a file cannot both import `BTreeMap` and declare one -- so no guard against a project's own name is needed here, unlike php. Ambiguous calls fall 464 to 69 here, 1585 to 1413 on ripgrep and 951 to 713 on serde; `external` goes from 0 to 160 on ripgrep and 950 on serde, where the conclusion that a call leaves the project did not previously exist for rust at all. Every resolved edge lost is a false one: 92 on ripgrep, all of them std or a dependency -- `File::open`, `OsStr::from_bytes`, `Regex::config` from regex-automata, `Database::open` from redb. The first version took 7 real calls with them, because ripgrep writes `use {grep_matcher::LineTerminator, ..};` with the brace first and every part carrying its own crate, and reading one root for the statement made a sibling crate of the workspace look foreign; the paths are expanded now, and a sibling crate is dropped with every other package the project owns. Corpus warnings are unchanged at 329 and only the two rust projects' finding totals move.
- A file that says a generator wrote it is not the program's own. The path rules cover `*.pb.go`, a `generated/` directory and vendored trees, and they see nothing where it matters most: gqlgen writes `generated.go` and `models_gen.go` beside the resolvers a person wrote, terraform writes `checkablekind_string.go` beside `checkablekind.go`. 219 of gqlgen's 865 go files carry a banner and hold 14363 of its 18653 functions; 168 of them sit where no path rule looks. The banner is what travels between languages -- `DO NOT EDIT`, `@generated`, `<auto-generated` -- and the generator writes it itself, so the scan reads the first six lines the way it already reads a file to tell whether it is minified, and records `written_by: generator` on the file node. 354 files across the corpus say it and not one of them is hand-written: stringer output, EF Core migrations, `.Designer.cs`, jnigen and mockito output, roxygen's `NAMESPACE`, rlang's standalone files. 2752 findings go -- gqlgen 6613 to 4907, http 4600 to 3609, eshop 27, dplyr 20, terraform 8 -- no project gains one and warnings are unchanged at 329 and per project. On this repository the rule finds exactly one such file, `Cargo.lock`, and changes nothing: the same source scanned by both binaries reports the same 6818 findings. The browser reads the same mark off the same file node.
- The rule that asks the same question in its own words is the one that keeps the bug. Every insight kind but one asks `is_the_programs_own`; the orphan rule spelled half of it out itself -- a test-like path, a `#[test]` attribute -- and so kept reporting generated functions after every other kind had stopped. Both sides had the same copy, so the browser agreed with the CLI about the wrong answer and the parity check saw nothing. Pointing both at the shared question removes 2461 more findings: http 3609 to 2504, gqlgen 4907 to 3925, terraform 341, dplyr 27, eshop 6. Warnings are unchanged at 329 and per project, no project gains one, and the fixture on each side now pins the orphan kinds beside the error flows it already pinned.
- The parity check had only ever been run on this repository. Two more rules spelled the shared question out themselves -- the duplicate-label rule and the unreachable-config-read rule -- and the browser's copy of the first already asked `isTheProgramsOwn`, so the two sides disagreed by 1151 findings on gqlgen and nothing noticed: `insight-parity` compares a scan of this repository, which has exactly one file a generator wrote and it is `Cargo.lock`. Running it against gqlgen said `duplicate_function_label: browser found 727, CLI 1878` on the first try. Both rules ask the shared question now, gqlgen falls to 2774, http 23, terraform 16, dplyr 1, and the fixture on each side pins a name two generated files share against one a person also wrote -- which is the protection that does not need a corpus to work. Warnings are unchanged at 329 and per project.
- The bundle's copy of the path classifier is now compared, not trusted. `check-defs` already fails when one of three shared lists drifts, but the largest duplicate is not a list: `isTestLikeSourcePath` is a hand-maintained copy of the whole `is_test_like_source_path` rule, and it has been brought back into step by hand every time the CLI's grew -- vendored directories, protobuf naming, storybook stories, a test runner's configuration. Nothing compared them, because `insight-parity` compares what the two sides compute and stays silent while they agree, which they do until a graph reaches the difference. The two rules spell out 44 path tokens each and today they match exactly, in both directions; the check now fails if either grows one the other lacks, and was confirmed to fail by adding one.
- A call the resolver could not settle is not the absence of a call. An ambiguity is recorded as one placeholder, so none of the definitions it might mean gets an incoming edge and every one of them then reads as a function nobody calls: cats declares `eqv` 72 times, 46 of them with no caller, while 39 unsettled calls reach for that name. 32479 of the corpus's 116130 uncalled functions are of that kind. The first version matched the name again in the insight and was four times too wide -- 333455 same-name declarations against terraform's 77707 real candidates -- because the narrowing that produced them (the file, the package, the receiver's type) is gone by then; the resolver marks the candidates it actually found instead, which costs terraform 250 KB and no node or edge, and the browser reads the same mark. 19458 findings go, the largest of these cleanups: cats 7956 to 5253, terraform 10508 to 8716, mastodon 8775 to 6995, okio 2794 to 1558. No project gains one and warnings are unchanged at 329 and per project. Operator methods were measured beside this and left alone: infix application is not recorded as a call, but that is 173 functions in the whole corpus.
- An unresolved entrypoint says which manifest declared it. Auditing the corpus's 329 warnings by kind -- 164 are the project's own FIXME, BUG and XXX comments, 66 are dependency cycles, 27 are dev dependencies imported from runtime code -- found no false one: vue's `packages/compiler-sfc` really declares `lru-cache` and four others under `devDependencies` with `dependencies` empty, guzzle's `composer.json` really lacks `psr/http-message`, and `use Openssl\Session;` really matches no package it declares. What it did find is that this one warning named no place while every neighbour does: zod writes `npm script:lint` twice, biome in `package.json:71` which runs and tslint in `packages/tsc/package.json:27` which is stale, and the message named neither of the eight manifests. It names the declaring file and line now. The corpus sweep is byte-identical, warnings included, because only the wording changed.
- A call written only outside the program is not the program's unresolved call, and a file carries its path in its label. `unresolved_call` was the corpus's second-largest kind at 46702 and 765 of terraform's 2485 labels are reached only from its suite, its examples and its generated servers. Reading the caller then turned up the larger half of the same mistake: a file node has no span, its path is its label, and ruby and lua write plenty of calls at the top of a file where the file itself is the caller -- so a spec's own top-level calls had been counting as the program's in every filter added this week. kong's error flows alone fall from 7135 to 1039. 21557 findings across 42 projects go, no project gains one, warnings stay at 329, and the browser recomputes this kind too.
- A suite throws on purpose. Reading the corpus's insight kinds rather than its warnings put `potential_error_flow` at 32109, and a third of it is not the program: a test raises to fail and a generated file raises whatever its generator wrote. 3758 of gqlgen's 5846 are its `_examples` and generated servers, 1258 of kong's are its specs, and 10005 findings across 40 projects go, with no project gaining one and warnings unmoved at 329. Both halves of the pair read the same way now -- `unreachable_error_flow` says the same thing about the same edge -- and so does the browser, which recomputes both.
- A name repeated only outside the program is not its duplicate. Nobody renames a generated function, and a suite's repeated helper is the harness's business: 766 of terraform's 1500 duplicate-label groups are its generated protobuf declaring `Reset` and `String` once per message, 518 of gqlgen's are the same, and 2542 findings across 34 projects said nothing a reader could act on. The orphan insight has always read them this way and this one did not. No project gains a finding, warnings stay at 329, and the browser's own count moved in step -- it recomputes this kind, so the two disagreed until it did.
- A Ruby class that descends from outside answers with its base. Ruby's remaining pile is not missed project methods but Rails: mastodon's biggest unresolved names are `where`, `present?`, `redirect_to` and `permit`, and none of them is among what the project declares. A class whose ancestry leaves the project -- `Account < ApplicationRecord < ActiveRecord::Base` -- answers with its base's methods, which is what the constant receiver already says for a call written through one. mastodon's unresolved ruby falls 17397 -> 16994 and its dependency calls rise 4055 -> 4458; sinatra 532 -> 516. Marking them `builtin` would have been the easier change and the wrong one: a gem the manifest declares is not the language. The estimate said 3684 and the answer was 403, because counting an owner that is not a declared type at all as "inheriting from outside" flattered it -- the rule reaches only classes whose ancestry the graph can actually walk.
- A module names the file that holds it, with nothing to narrow. OCaml's own rule was being used only to choose between candidates, and a call with no candidate at all never reached it, so dune's `stdune` answered none of its 683 `List.map`. dune resolves 39094 ocaml calls where it resolved 37800, and 14089 stay unresolved where 15307 did; 112 of those had read as the standard library's `List` and are dune's own. The lesson from the commit before was written into the harness first: `sweep.sh` now records every insight a project produces beside its warning count, and that second number is what showed this change -- dune 11451 -> 10776, kong 13382 -> 13324, warnings unmoved at 329 and nothing else touched. An existing test caught the one thing the measurement could not: looking a name up a second time must not walk around what the first look-up refused, and 23 of dune's new edges were `src/` reaching into `spec/`.
- What a project ships is still something it calls. Re-running the sharp probe after fifteen commits of this work found the most-called definition in every project unchanged, name for name and count for count -- and the resolution bases beside it showed dune down 1274 resolved calls and redis 127, which nothing had reported because neither is a project any of those commits touched. The cause was two commits back: a program never calls its own suite, so a call outside a test cannot resolve to one, and reading vendored and generated code as test-like put dune's `vendor/lwd` out of reach of the dune source that uses it. Both are outside the program for counting -- hotspots, coverage, orphans -- and only one of them is unreachable from it, so they are now two questions. dune resolves 40168 calls where it resolved 38962 before any of this and 37688 with the fault, redis 40688 from 40172, and the corpus sweep stays at 329 project for project.
- A test is run by its harness wherever it is written. The change above left entrypoint coverage counting vendored functions in its denominator, and fixing that turned out to need one thing fixed first. Rust writes its tests inside the file they test, so a path cannot say what the node already does: `fresh_install_creates_all_artifacts` reads `.mcp.json` from `install.rs`, and five of this repository's six `unreachable_config_read` findings were its own tests. The orphan insight has always asked both the path and `invoked_by: test_runner`; the reachability findings asked only the path. With that in place the denominator can count the program alone, which is what the entrypoints are expected to reach. This repository's `low_entrypoint_coverage` warning goes away -- its own tests were dragging the ratio down -- and redis keeps the genuine `tlsConfigure` finding it had lost. The corpus stays at 329, project for project, and the browser now agrees with the CLI on `potential_error_flow` too: that ratio decides whether a finding is reported at all, so both copies of it have to say the same thing.
- Code a project ships but did not write is not its program. Nearly half of what redis was calling its program is jemalloc, hiredis and lua under `deps` -- 6138 of 13770 definitions -- dune vendors a quarter of its own under `vendor` and nlohmann/json a fifth under `thirdparty`. Those directories now read the way `testdata` and `fixtures` already did. The corpus baseline is 329, from 333, and every one of redis's four is explained: it loses five `unreachable_config_read` findings, four of them about `deps/jemalloc/scripts/freebsd/before_script.sh`, and gains one `low_entrypoint_coverage`. That last one is the honest consequence of a half-measure: entrypoint coverage still counts vendored functions in its denominator, so excluding them from the entrypoints makes the ratio read worse. Counting only the program there was tried and measured -- it removes redis's warning and promotes five of this repository's own findings about test functions from info to warning, because the same ratio gates whether reachability is worth reporting at all -- so it wants its own change. The browser's copy of the classifier moved in step, including the stories and runner-config rules it had drifted behind on.
- Generated code is not the program either. protobuf names its output the same way in every language and nobody writes it: 25 `*.pb.go` files carry 4694 of terraform's 18277 definitions, a quarter of what the graph was calling the program, and Dart's `.g.dart` was already read this way. The question could not even be put for Go, whose own rule -- a test is a file named `_test.go` -- answers for every `.go` file and returned first, which also meant a directory called `generated` had never applied to one. Both are asked before it now. The corpus baseline is 333, from 335: gqlgen loses two `XXX` comments that live inside `apollo_trace.pb.go`, which nobody will act on because the next generation overwrites them, and nothing else moves.
- A php `use` says whose class a bare name means. Python's `from a.b import c` bound `c` and PHP's `use A\B\C;` bound nothing, so `new Request(..)` -- 612 of them in guzzle, all psr7's -- could only be matched by name against everything the project declares. Binding it takes guzzle's unresolved php from 6249 to 4579 and its dependency calls from 765 to 2426, koel's from 7342 to 5368 and monolog's from 1685 to 1350, with resolved, ambiguous and constructor unchanged to the call in all three. Two things it must not do, and the first attempt did both: a name the project declares is the project's whatever the import list says -- `use GuzzleHttp\Client;` names guzzle's own `src/Client.php`, and 425 `new Client(..)` calls stopped reaching its constructor -- and `use function Tests\create_user;` binds a name PSR-4 cannot place, because it maps namespaces onto directories for classes and not functions, which took 825 of koel's own test helpers away from `tests/Helpers.php`.
- A factory and a mock builder belong to the framework. PHP's list had PHPUnit's assertions and Mockery's expectations but not the builder around them, and nothing of Laravel: koel writes `Song::factory` 829 times, `createOne` 526 and `getJson` 158, monolog writes `getMock`, `onlyMethods` and `method`. 2030 calls, koel's unresolved php 7342 -> 5607 and monolog's 1685 -> 1399, with every other count in all three projects unchanged to the call. A php label can name the class it goes through, so the method is read off the end of it. guzzle moves by 9 and is the reason to stop here: its pile is `Request` 612 and `Response` 539, objects being constructed rather than a framework's methods, which is a different rule and a different measurement.
- A matcher is part of the runner that hands it over. `expect`, `describe` and `it` were on the javascript list and the matchers that read them were not, which is most of what a suite actually writes: `toBe` 226 in vue core, `toHaveBeenCalledWith` 166 in koel, `toEqual` and `toThrow` in zod, and chai's `to.be.revertedWithCustomError` and `withArgs` in openzeppelin. 3271 calls across those four, and each project's `resolved`, `ambiguous`, `external` and `constructor` counts come out to the call unchanged: core 5625 -> 4702 unresolved, koel 4880 -> 4022, zod 4523 -> 3919, openzeppelin 5246 -> 4360. The matchers are matched by prefix rather than listed -- `toBe`, `toHave`, `toEqual`, `toMatch`, `toThrow`, `toContain` -- because a framework adds them faster than a list can be kept, and a plain `to` would take `toString` and `toDTO` with it.
- Ruby provides its own methods. Ruby still carried the corpus's largest pile of unresolved calls, and 4434 of mastodon's 22033 are Ruby's own: `new` 672, `to_s` 350, `map` 347, `each` 298, `first` 281. mastodon's unresolved ruby falls to 17397 and sinatra's from 1131 to 532. Where this is asked matters and cost a measurement to get right: a ruby call written through a constant the project never declares is a gem's and is filed as one, which is why `Addressable::URI.parse(href).normalize` is not answered by the project's own `HashtagNormalizer#normalize`. Putting these names in the general builtin list let 107 of them past that rule and back onto same-named definitions, so the list is asked only where the choice is between `builtin` and `unresolved`: `resolved`, `ambiguous` and `external` are now unchanged to the call. ActiveSupport's `present?` and `blank?` are left out, being a gem's rather than the language's.
- Shouldly and xUnit hand a C# suite its assertions. Re-reading the corpus's unresolved calls with everything else in place put C# at the top of what was left: 5759 of Polly's 8552 unresolved csharp calls -- 67% -- are `Should.Throw`, `ShouldBe`, `Should.ThrowAsync` and the Moq setup around them, and C# was the last of the big languages with no test-runner rule. Polly's unresolved falls to 2793 and eshop's from 1156 to 1082, with `resolved` and `ambiguous` untouched in both. Newtonsoft is the case that shows why: it declares its own `XUnitAssert` shim, and its 2197 `Assert.AreEqual` calls still reach it, because a call that resolves never consults this list. Two other piles were measured and left: dune's `List.map` is dune's own `stdune`, not OCaml's, so naming those modules standard would be wrong, and reaching them by the language's module-is-a-file rule settles 752 of 15157; redis's `RedisModule_*` are a header's function pointers.
- Telling inside from outside takes an absolute root. A scan enriches through a language server on its own when one is installed, and the root node has been saying `semantic_enrichment: applied` all along -- while producing no semantic edges at all. A server answers with an absolute path, and `codegraph scan .` passed `.` as the root, so nothing could be stripped from the answer and every definition looked like it came from somewhere else. The `semantic-*` commands canonicalise for exactly this reason and the automatic pass did not, which is why the pass measured well when driven by hand and did nothing when a scan drove it. `codegraph scan .` on this repository now yields 92 semantic edges where it yielded 0, the same as naming the directory in full, and the cached and uncached scans stay byte-identical.
- A call edge says where the call is written. A semantic edge recorded the answer's location instead, under `path` where every other call edge writes `file`, so `line` meant the definition on one edge and the call site on the next. For a definition in the standard library that location is an absolute path on whichever machine ran the pass: all 919 of ripgrep's spans were `/Users/…/.rustup/toolchains/…`, which no other span in the graph is and which means nothing anywhere else. The call site now goes where it goes on every other call edge, taken from the edge the semantic one replaced, and the definition's own place is kept as `definition_file`/`definition_line` only when the project contains it -- outside it the evidence and the target already name the dependency or the library. All 919 spans name the call site now and none is absolute; what the pass settles is unchanged, unresolved 3525 -> 2836 and ambiguous 1585 -> 1395.
- A Rust macro declares a name calls reach. Validating the semantic pass on a second project -- serde, 500 questions, 476 answered, 441 edges, a higher share than ripgrep's -- left one kind of answer unmatched, and reading it named the cause: `seq_impl`, `tuple_impls`, `map_impl`, `impl_deserialize_num`. serde is built out of `macro_rules!`, 63 of them, and not one was in the graph; they existed only as the placeholders standing for calls nothing could answer. Julia's macros already counted as definitions and Rust's did not. 99 definitions across serde and ripgrep are now visible where the source declares them, and 16 calls find them. The corpus sweep is unchanged at 335, and the same reading turned up two things worth recording: `codegraph scan` enriches through a language server on its own when one is installed and the project is small enough, and a semantic edge records the definition's position under `path`/`line` while every other call edge records the call site under `file`/`line`.
- A language ships its own library somewhere it says. 595 of ripgrep's 1000 answers put the definition in rust's own library and were thrown away, leaving those calls reported as unresolved -- a resolver failure where the compiler had given a plain answer. `unwrap`, `clone` and `as_ref` were deliberately kept out of the hand-written builtin lists because a project may declare them; here the language server has said it did not, which is a thing no list can know. Only layouts a path states outright are read: rust ships its sources under `rustlib/src/rust/library`, python keeps its own modules in `lib/python3.x` with everything installed beside them in `site-packages`. The pass now explains 932 of those 1000 answers instead of 337, and applying them reads unresolved 3527 -> 2842, builtin 1641 -> 2194, ambiguous 1589 -> 1395, resolved 3751 -> 4077. What is left unmatched is 48 answers, down from 643.
- A semantic edge is a resolved call, and now says so. Running the pass on ripgrep at scale -- 1000 of its 14285 work items, 126 seconds, 967 answered -- produced 337 semantic edges, and applying them settled 119 calls the graph had called ambiguous and 207 it had called unresolved. That is the frontier the syntactic rules could not reach, reached. But the edges carried no `resolution`, no `call_label` and no `language`, so those 337 calls left every category at once: ripgrep's resolved count went *down*, from 3751 to 3740, when the pass had in fact resolved 326 more. A semantic edge now carries the provenance every other call edge carries, with `resolution_basis: semantic` naming what settled it, and the same run reads resolved 3751 -> 4077, unresolved 3527 -> 3320, ambiguous 1589 -> 1470.
- A question about a call is about its method. The span of a dotted call starts at the receiver, so the semantic pass asked the server about `builder` and got back the local variable -- 29 of ripgrep's first 100 answers landed on the caller that way, which the pass then discarded as self-referential. `builder.add` is ripgrep's own `GlobSetBuilder::add`. No reading of the source is needed to find the method: the parser rejects a label holding a space or a newline, so a label that survives is written contiguously and the method begins exactly as far along as everything before it is long. On the same 100 questions the self-referential answers go from 29 to none, and the edges from 37 to 40; the rest now say plainly that the method is the standard library's. Counted in characters rather than bytes, because that is what an LSP position counts and `Ünicöde.read` is a legal path.
- A definition in a dependency still says which one. The semantic pass was run on a project other than this one for the first time: ripgrep, 14285 work items, 100 asked. 98 answered and 25 became semantic edges -- `GlobSetBuilder::new` reaching its type, `RegexMatcher::new` settling a call the graph had called ambiguous -- which is a yield of 25% against the 8% this repository gives, because a project full of standard-library calls is a poor place to measure it. Of the 44 answers thrown away as outside the project, 32 were the Rust standard library and 12 named a crate ripgrep declares: `log`, `anyhow`, `regex`, `memchr`. Every ecosystem keeps its packages in a directory that names them -- cargo's `registry/src/<index>/regex-1.10.2`, npm's `node_modules/@scope/name`, python's `site-packages`, composer's `vendor/<org>/<name>`, rubygems' `gems/<name>-<version>` -- so those now reach the dependency node the manifest already declared. ripgrep's 100 answers yield 37 edges instead of 25. The standard library is in none of those directories and stays unmatched, which is the honest answer.
- A Python test case gets its assertions from unittest. django-oscar writes `self.assertEqual` 841 times and `self.assertTrue` 314 -- 1717 calls in all, 35% of everything unresolved in its python -- and every one comes from the `TestCase` it extends, the way a PHPUnit case gets `$this->assertSame`. oscar's unresolved python falls from 4866 to 3149. flask and requests do not move at all, because they write their checks with the `assert` statement, which is not a call. This was found while looking somewhere else: ambiguity, not unresolvedness, is now the larger pile in several languages -- go 17144, ruby 13046, cpp 8025 -- and three syntactic rules for narrowing it were measured and dropped. "A call in the program does not mean a definition in the suite" would settle 41 of terraform's 12812. A bare Go call already prefers its own package. Reading the receiver's type where it is written literally reaches 685. What is left is `diags.HasErrors` and `err.Error` -- methods on locals whose types need inference or a language server, which is what the semantic pass is for. The sweep is unchanged at 335.
- kotlin.test hands a Kotlin suite its assertions. Kotlin was next on the map at 31.4%, and 1791 of okio's 3985 unresolved kotlin calls -- 45% -- are the test framework's: `assertEquals` 876, `assertTrue` 216, `assertThat` 171, `fail` 124, plus the AssertJ chain that reads them. The one `assertEquals` okio declares is a private helper inside a sample, and a call that reaches a definition never consults this list, which is why `resolved` has not moved for any of these six languages. okio's unresolved kotlin falls to 2196 and retrofit's from 486 to 431. Scala was read at the same time and left alone: its biggest unresolved names in cats are `f` 829, `g` 111, `p` 47 -- function parameters being invoked, which is a call through a value rather than a definition anything could find. The sweep is unchanged at 335.
- XCTest hands a Swift suite its assertions. Swift sat next on the language map at 23.9% of in-project calls resolved, and 2559 of Alamofire's 4317 unresolved swift calls -- 59% of them -- are XCTest's own: `XCTAssertEqual` 516, `expectation` 390, `fulfill` 387, `waitForExpectations` 333. Alamofire's unresolved swift calls fall to 1758 and swift-argument-parser's from 1537 to 1252, with nothing resolving differently. What is left in swift-argument-parser has a different cause worth naming: tree-sitter-swift stops on `#_sourceLocation` in a default argument, so `Sources/ArgumentParserTestHelpers/TestHelpers.swift` gives up fifteen of its public helpers and 108 calls to its own `AssertParse` find nothing. The file already carries a `syntax_error` insight saying the grammar did not reach it, which is what that insight is for. Bash was measured alongside and left alone: shellcheck has 58 unresolved bash calls and they are `grep`, `find` and `docker`, which is the shell naming programs rather than a resolver failing. The sweep is unchanged at 335.
- A default import is a name calls are written through. Following the language map to javascript (21.4% resolved) found the biggest unresolved names to be `path.join`, `assert.strictEqual`, `axios.post`, `utils.isArray` -- calls through a module the file imports, which the graph could not see because only `import * as path` was recorded as a qualifier and `import path from 'path'`, the form nearly every file uses, was recorded as a bare name. Now both are. axios's javascript calls that read as a dependency rise from 490 to 770 and its unresolved fall from 2035 to 1821. It also corrects six edges: `zlib.gzip` in a test had been answered by a `const gzip` the same file declares. A project's own tests import it by the name it publishes -- axios's smoke tests write `import axios from 'axios'` -- and that name is not an outside dependency, so the qualifier is dropped once the walk has read every manifest, which the ordering of a single sorted walk cannot guarantee earlier. The sweep is unchanged at 335.
- A language names its own vocabulary. The per-language map that found Ruby's pile also ranked dart lowest at 14.9% of in-project calls resolved and nix next at 19.9%, and reading their biggest unresolved names again found no method of the project's own. Nix says outright what belongs to the evaluator -- everything under `builtins.`, plus `toString`, `import`, `map` and a handful more in the global scope -- and home-manager writes 798 of them. package:test hands a Dart suite its cases the way busted, munit and rspec do, 470 of them in the `http` package. home-manager's unresolved nix calls fall from 3649 to 2851 and `http`'s dart calls from 7628 to 7025. What is left in both is a dependency rather than a gap: 2019 of home-manager's remaining calls are nixpkgs' `lib.*` and `pkgs.*`, and 1626 of `http`'s are generated Objective-C bindings. The sweep is unchanged at 335.
- RSpec hands a Ruby spec its cases and its matchers. Ruby carries the corpus's largest pile of unresolved calls -- 31267 of 57683, a worse share than any language but dart, nix and javascript -- and reading the biggest names in it showed not one missed method of the project's own: `expect` 1133, `it` 1099, `let` 936, `Fabricate` 917, `before` 726. They come from rspec and the fabrication gem, and calling them unresolved says a resolver failed where a dependency simply provides them. Ruby now answers the way busted, munit, PHPUnit, Foundry and vitest already do: mastodon's unresolved ruby calls fall from 29668 to 22033 and its builtin ones rise from 2521 to 10156, sinatra's from 1494 to 1131. Nothing resolves differently -- these calls reached nothing before and reach nothing now, they just say why -- and the corpus sweep is unchanged at 335.
- A story is not the program. Checking the corpus's 337 warnings one kind at a time found `non_runtime_dependency_import` reporting 17 of mastodon's `*.stories.tsx` files as production code that needs `storybook` -- a package the manifest rightly declares for development only -- and zod's `vitest.root.mjs` the same way for `vitest`. Storybook names its files the way a test framework does, and a test runner's own configuration is not the program either, so both now read as test-like and the corpus baseline is 335. Nothing else moved. The kinds checked alongside them hold up: all 13 `undeclared_external_import` are real, guzzle importing `psr/http-message` without declaring it and pytudes' `from accum import *` naming a module the repository does not have; and the 41 `dependency_cycle` warnings in the six projects carrying most of them are closed cycles across files, mastodon's `interactions.js` and `interactions_typed.ts` importing each other among them.
- The table a Lua module returns can be written three ways, and all three name its exports. The rule above read only `return { … }`; kong writes the table under a name as often -- `kong/api/endpoints.lua` closes with `local Endpoints = { handle_error = handle_error, … }` and `return Endpoints` -- and `spec/fixtures/balancer_utils.lua` opens an empty table and fills it a line at a time, `balancer_utils.begin_testcase_setup = begin_testcase_setup`. Both say what the module hands out as plainly as writing it between the braces. The calls that named a definition in exactly the file the calling file requires went from 456 to 95 with the first form and to 2 with all three; kong's Lua calls are now 8541 resolved and 6287 unresolved, from 6934 and 8083 before this pair of changes. A table the module builds but does not return still says nothing -- kong's schema keeps its validators in one, and reading those as exports would hand out every name the file happens to write.
- A Lua module exports what it returns. `local` is how a Lua module writes every one of its functions, and the resolver read that as "private to this file", so a definition the module hands out in its `return` table was never a candidate for the files that require it: `bin/kong-health` calls `kill.is_running`, `kong/cmd/utils/kill.lua` ends `return { kill = kill, is_running = is_running }`, and the call was unresolved. 456 of kong's qualified calls named a definition in exactly the file the calling file requires; 95 remain, every one in a module that returns a named table rather than a literal one. kong's Lua calls went from 6934 resolved to 8438 and from 8083 unresolved to 6395. A name the file writes twice is left alone -- `local function each_strategy() end` and a later `each_strategy = function(..)` are one function declared and then defined, and promoting both turned 202 resolved calls into ambiguous ones. The corpus sweep is unchanged project for project.
- A call is written where its name is written. Every call recorded the span of the whole expression, so `graph.nodes.iter().any(..)` written over three lines reported `any` on the line that reads `graph` and at that column: the edge's label and its position named different tokens, and a reader who followed the edge landed on the receiver. 9779 of this repository's 37344 call edges pointed somewhere other than the name they carry, and 332 do now. The rule needs no per-language mirror of `call_label` -- the span goes to the node whose text *is* the label, so a dotted label such as `plan.term.as_deref` keeps the span it had, and a label the parser composed rather than read (a scope PHP prefixed, `collect::<Vec<_>>`) falls back to the call. 5596 call sites moved to their own line; the corpus sweep is unchanged project for project, and the only counts that moved are the calls inside the function this change added.
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

## Install

```bash
git clone https://github.com/ASlava12/codegraph
cd codegraph
./install.sh
```

That builds the CLI, puts `codegraph` on your PATH through `~/.cargo/bin`, and
installs the agent skill for every project. Rust is the only prerequisite
(<https://rustup.rs>); the first build takes a few minutes.

| Command | What it does |
| --- | --- |
| `./install.sh` | the CLI and the skill for every project |
| `./install.sh --cli` | the CLI alone |
| `./install.sh --skill` | the skill alone, in `~/.claude/skills/codegraph` |
| `./install.sh --project <path>` | the skill for one project, in `<path>/.claude/skills/codegraph` |
| `./install.sh --uninstall` | remove the CLI and the skill installed for every project |

Check the CLI with `codegraph summary . --no-semantic`.

The skill ([`skills/codegraph`](skills/codegraph)) teaches an agent to reach for
bounded graph slices before grepping or reading whole files, and it is Markdown
only — installing it for one project writes nothing outside that project's
`.claude/skills/codegraph/`. Installing it for every project makes it available
wherever the agent runs; installing it for one keeps it with that repository, so
it travels in the repository's own history if you commit it.

To wire CodeGraph into a repository as an MCP server instead — `.mcp.json` plus
`AGENTS.md` and `CLAUDE.md` guidance — run `codegraph install-agent <repo>
--platform all`. That writes into the repository, so read the diff afterwards.

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
