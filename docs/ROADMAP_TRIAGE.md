# Roadmap Triage (Phase 9 Sweep)

Disposition of every unchecked `../ROADMAP.md` item as of 2026-07-10, per the
Phase 9 requirement that each one is finished, re-scoped, or explicitly
dropped with a recorded reason. Findings from the end-to-end audit are
referenced by their `FEATURE_AUDIT.md` ids (F1–F13).

Dispositions used:

- **keep** — stays as written; scheduled into a `ROADMAP_NEXT.md` milestone.
- **re-scope** — reworded in place to a smaller or clearer deliverable; the
  original scope cut is recorded here.
- **drop** — removed from the active checkbox list; the item text and reason
  move to the "Dropped" note in its phase.

## Phase 1

| Item | Disposition |
| --- | --- |
| Deduplicate unresolved-call placeholder nodes, classify builtins (F2) | keep — scheduled in Milestone 7; top graph-quality lever (27k placeholder nodes on this repository). |

## Phase 3

| Item | Disposition |
| --- | --- |
| Calibrate `unresolved_call` severity, dedupe by label (F3) | keep — Milestone 7; unblocks useful risk grades on syntactic-only scans. |
| Dedicated node kind / `item_kind` facets for control-flow facts (F4) | keep — Milestone 7. |
| Route env/config questions in `ask` to the config rule (F13) | keep — Milestone 7. |

## Phase 4

| Item | Disposition |
| --- | --- |
| Bound `quality_gate` payload in report snapshots (F1) | keep — Milestone 7; makes the report usable as an agent artifact again. |

## Phase 7

| Item | Disposition |
| --- | --- |
| Document ingestion for Markdown, plain text, PDFs, Office files, sidecars | **re-scope** — narrowed to plain-text files and generated Markdown sidecars with size limits and provenance. PDF and Office binary parsing is dropped: it needs heavyweight external parsers, conflicts with the deterministic local-first scanner, and the sidecar convention already covers "put extracted text next to the binary". Scheduled in Milestone 9. |
| Optional local/configured-model semantic extraction for non-code documents | **drop** — model-backed extraction is nondeterministic and contradicts the project's deterministic, provenance-first contract; deterministic Markdown/plain-text ingestion plus agent-side summarization over MCP covers the same need without embedding model calls in the scanner. |
| Media ingestion hooks for audio/video transcript sidecars | **drop** — no user demand, and the re-scoped document ingestion already indexes transcript *sidecar files* (plain text/Markdown) if a user generates them; a dedicated media pipeline adds surface without new graph facts. |
| Canonicalize package manifests into shared package hub nodes | keep — Milestone 9; complements the F2 placeholder dedup work. |
| Optional local hooks nudging agents toward CodeGraph before grep-heavy workflows | **re-scope** — narrowed to extending the existing `install-agent` command with optional assistant hook configuration snippets (for assistants that support command hooks). A standalone hook runtime is out of scope; guidance files plus hook snippets are the deliverable. Scheduled in Milestone 9. |
| MCP tools for `refactor_context`, `ask`, `source_search`, memory (F7) | keep — Milestone 7; highest agent-parity leverage. |
| SVG export target | keep — Milestone 9; small, self-contained export addition. |
| Expose PR impact dashboard via API and web (F6) | keep — Milestone 7. |
| Explicit security model for external ingestion (URL validation, redirect blocking, …) | **drop** — CodeGraph has no external URL/network ingestion surface: all ingestion reads local files, and code-only scans are offline by contract. The item guarded features (remote docs/media fetching) that are themselves dropped or re-scoped to local files. Revisit only if a network ingestion feature is ever added; its design must then include this model. |
| Exclude string-literal/fixture patterns from benchmark recall oracles (F12) | keep — Milestone 7. |

## Phase 8

| Item | Disposition |
| --- | --- |
| Web views/actions for impact, seams, contracts, refactor-context (F5) | keep — Milestone 7. |
| Reuse cached results and bound journey search in refactor-context (F11) | keep — Milestone 7. |

## Phase 9

All Phase 9 items are the active milestone (Milestone 7) and stay as
written: parity verification, dogfooding, CLI ergonomics, `n42` id
acceptance (F8), API param/error contract unification (F9), compact
incremental outputs (F10), web discoverability, task guides, and
self-describing agent outputs. The audit item and this sweep are complete.

## Phase 10

All six items stay as written and remain scheduled as Milestone 8
(module splits, clippy clean with `-D warnings`, shared helpers, grouped
request structs, module docs, regression fixtures before refactors); the
two duplication-focused items map to one milestone entry.

## Outcome

Of the 32 unchecked items besides this sweep itself:

- 3 items dropped with reasons (model-based extraction, media hooks,
  external-ingestion security model).
- 2 items re-scoped in place (document ingestion narrowed to plain text +
  sidecars; agent hooks narrowed to `install-agent` hook snippets).
- 27 items kept as written.

All 29 remaining items are scheduled in a `ROADMAP_NEXT.md` milestone
(Milestone 7: audit follow-ups and usability; Milestone 8: internal
quality; Milestone 9: remaining parity features).
