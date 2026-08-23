# Contributing

CodeGraph is early, but contributions should already keep the CLI, API, UI, and graph schema moving together.

## Local Checks

Run the same core checks used by CI before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node --check crates/codegraph-web/static/label-policy.js
for module in crates/codegraph-web/static/js/*.js; do node --check "$module"; done
node --test crates/codegraph-web/static/label-policy.test.js
node crates/codegraph-web/static/check-defs.mjs
node crates/codegraph-web/static/flow-smoke.mjs
node crates/codegraph-web/static/views-smoke.mjs
cargo run -p codegraph-cli -- scan . --format ndjson > /tmp/codegraph.ndjson
```

For server changes, also smoke-test the API:

```bash
cache_dir="$(mktemp -d)"
cargo run -p codegraph-server -- --root . --port 3765 --cache-dir "$cache_dir"
curl 'http://127.0.0.1:3765/api/health'
curl 'http://127.0.0.1:3765/api/scan?path=.'
```

## Engineering Expectations

- Keep the shared graph schema stable and documented.
- Prefer typed graph facts with explicit confidence over opaque strings.
- Add tests when behavior changes in parser, indexer, analysis, storage, server, or CLI code.
- Land refactors only behind green workspace tests: when a planned refactor touches code without regression coverage, add the fixture or guard test first (in the same or an earlier commit), then refactor. Structural changes must not change behavior silently — run a live CLI smoke against this repository and compare the insight-warning baseline before committing.
- Keep UI behavior backed by the same API and graph model used by agents.
- Avoid checking in generated build artifacts or local cache records.
- Both caches are keyed by the build that filled them (`build_identity`), so changed extraction
  rules never serve a previous binary's facts. Bump `CACHE_SCHEMA_VERSION` or
  `PARSE_CACHE_SCHEMA_VERSION` as well when a record's *shape* changes, so an old record is
  rejected rather than decoded into the new struct.
