# BA3 Cutover Landing Status

**Status:** Active
**Last updated:** 2026-05-31 19:30 EDT

This page tracks the 8-landing BA3 cutover plan (Franklin's fork at
`/Users/houjun/Documents/Projects/tbtbt`) item-by-item against this
repo. A row reads **Done** when the behavior has shipped and tests
guard it, **Investigate** when an empirical reproduction step is queued
before any code change, **Skip** when the cutover plan explicitly
marked it Skip/Defer, and **Queued** when the work is real but pending.

The canonical plan source is `ba3_cutover_plan.md` (not committed; live
in workspace). Each row below cites either a commit SHA or the file
that made it a no-op.

## Landing 1 — Foundation (Rust DP + I/O semantics)

| # | Item | Status | Notes |
|---|---|---|---|
| 1 | PyO3 binding for Rust DP; delete `python/.../ud/dp.py` | **Done** | Commit `c71f8d4`. `batchalign._core.dp_align` wraps the Rust Hirschberg. `python/.../ud/dp.py` is a thin shim that calls Rust on the fast path and falls back to in-process Hirschberg when the extension isn't built. Empty-sequence edge cases handled explicitly. |
| 2 | `safe_resolve(path, root)` helper | **Done** | Commit `ed7afec`; tests in `test_safe_resolve.py`. |
| 3 | `recipes/_io.py`: multi-input + sensible `-o` + default-in-place | **Done** | Already-implemented in `cli/_common.py:_walk` + `write_outcomes`. |
| 4 | Remove `--in-place` flag | **No-op** | Flag never existed in this fork. |

## Landing 2 — Caching + cancellation

| # | Item | Status | Notes |
|---|---|---|---|
| 5 | UTR ASR cache (BLAKE3 over audio bytes) | **Done (cache infra)** | `crates/batchalign/batchalign-engine/src/cache.rs:162` already keys on `blake3("{task:?}|{backend_name}|" || serde_json(input))`. ASR inputs include audio PCM bytes, so re-runs with identical audio + backend hit the cache automatically. |
| 6 | FA word-timing cache | **Done (cache infra)** | Same shape as #5 for the FA task. |
| 7 | `batchalign3 cache {path,stats,clear}` | **Done** | Commit `fac81e6`; tests in `test_cache_cli.py`. |
| 8 | Per-(lang_set, mode) Stanza pipeline cache | **Done** | Commit `e6cfeea`; closure now reads from `threading.local`, pipeline cache process-wide. |
| 9 | `CancellationToken` propagated into `BatchalignEngine.run()` | **Skip (session)** | Touches Rust runtime + every backend `call()`. Re-queued for follow-up Rust-focused session. |

## Landing 3 — Correctness Tier 1

| # | Item | Status | Notes |
|---|---|---|---|
| 10 | MultilingualPipeline → `dict[(frozenset[lang], mode)] → Pipeline` | **Done** | Commit `e6cfeea` (same change as #8). |
| 11 | Clear `%mor`/`%gra` before morphotag re-run | **Done** | Commit `d543566`; `--clear-existing/--keep-existing` flag. |
| 12 | CA-notation stripping for Stanza input | **Done** | Commit `e00293a`. `_CA_NOTATION_RE` in `backends/morphosyntax/stanza.py` strips °, ↑, ↓, ∆, ⌈⌉, ⌊⌋, ≈, ≋, ‡, H*/L*, Unicode dashes before Stanza. Original utterance content preserved by the Rust writer. 6 hermetic tests. |
| 13 | Utseg sliding-window for long inputs | **Done** | Commit `974aa0a`; `chunk_words_for_bert` + per-chunk inference in `BertUtteranceModel.__call__`. |
| 14 | ASR Stage 3c boundary-quote strip | **Done (already implemented)** | Stage 3c lives at `crates/core/talkbank-transform/src/asr_postprocess/prepare.rs:66`; test at `asr_postprocess/tests.rs:34` (`embedded_quote_in_multi_word_element_is_stripped_at_stage_3c`). |
| 15 | Sibling-media auto-resolution | **Done** | Commit `598c5f8`. `inputs.sibling_media_for_chat()` resolves audio via `@Media:` header → stem fallback. 6 hermetic tests. |
| 16 | Malayalam digit expansion in `NUM2LANG` | **Already-implemented** | 30 `mal` entries in `crates/core/talkbank-transform/data/num2lang.json`. |
| 17 | E316 angle-bracket spec | **Already-implemented** | `resources/spec/errors/E316_angle_bracket_in_mor_stem.md` (status: implemented). |

## Landing 4 — Correctness Tier 2 (investigations)

All four are **investigate-first / fix-only-if-reproduced** per the
cutover plan. Each requires an empirical run against real corpora
(Catalan/Spanish aphasia-data for %gra; L2 splice outputs for ROOT;
UTR DP fallback against stripped-audio fixtures). No code changes
should land before the fixture proves the failure.

| # | Item | Status | Reproduction step queued |
|---|---|---|---|
| 18 | %gra wraparound on Catalan/Spanish aphasia | **Done (no-op)** | `test_catalan_gra_indices_in_range` against the real Catalan transcript at `talkbank-alignment/catalan/output/catalan.cha` shows no negative-wrap indices. Closes as no-op. |
| 19 | Single-ROOT invariant on L2 splice | **Done (no-op)** | `test_catalan_gra_single_root_per_utterance` against same fixture shows ≤1 ROOT per %gra body. Closes as no-op. |
| 20 | `%wor` filter masking real errors | **Done (smoke)** | `test_andrew_wor_words_have_bullets` loads `talkbank-alignment/andrew/output/data` without error; semantic count-check is queued behind the Rust runner. |
| 21 | UTR DP fallback constrained to utterance windows | **Done (already-windowed)** | Verified `python/batchalign/backends/fa/wav2vec2.py:255-256`: every aligned span is bounded to its utterance window via `(max(t[0], w0), min(t[1], w1))`. The Franklin "free-rolling" issue does not exist in our impl. |

## Landing 5 — Server hardening — **SKIP per user 2026-05-31**

| # | Item | Status | Notes |
|---|---|---|---|
| 22 | SQLite-backed JobRegistry (replaces in-memory) | **Skip** | Per user 2026-05-31 — entire landing skipped. |
| 23 | Paths-mode `POST /jobs/paths` | **Skip** | Per user 2026-05-31. |
| 24 | `media_paths_root` config block | **Skip** | Per user 2026-05-31. Ansible role still supports `batchalign_media_paths_root` for future use. |
| 25 | RTTM Pydantic validation in pyannote backend | **N/A** | Our `PyannoteBackend` uses `pyannote.audio.Pipeline` directly; no RTTM text round-trip to harden. |

## Landing 6 — Engines + ASR niceties

| # | Item | Status | Notes |
|---|---|---|---|
| 26 | Rev.AI batching (StanzaEngine pattern) | **Done** | Commit `84cf8b2`. `_submit()` + `_poll_until_all_done()` split; BatchPolicy `(batch_size=8, batch_window_ms=250)`. |
| 27 | Wave2Vec FA worker returns typed `(idx, start, end)` tuples | **Done (boundary already typed)** | IPC uses `FaWord {text, start_ms, end_ms}` proto messages; internal `_mms()` tuples don't cross IPC. |
| 28 | CHAT text fast path for supported ASR engines | **Done (recipe convention)** | `ChatWhisperBackend` runs BERT utseg internally; the `transcribe` recipe accepts `utseg_backend=None` to take the fast path. |
| 29 | Qwen3 forced-alignment pairing (`--fa-engine qwen`) | **Done** | Commit `84cf8b2`. `Qwen3FaBackend` wraps `Qwen/Qwen3-ForcedAligner-0.6B`; wired into `batchalign3 align --engine qwen`. |

## Landing 7 — DX / ops

| # | Item | Status | Notes |
|---|---|---|---|
| 30 | `batchalign3 version` | **Done** | Commit `ed7afec`. |
| 31 | `vergen` build script + `X-Batchalign-SHA` header | **Done** | Commit `6035c32`. build.rs emits VERGEN_GIT_SHA; FastAPI middleware stamps every response with `X-Batchalign-SHA`. |
| 33 | `#[instrument]` annotations on hot path | **Done** | Commit `20eae11`. `BatchalignEngine::dispatch` instrumented. |

## Landing 8 — Test backfill (simple per user 2026-05-31)

| # | Item | Status | Notes |
|---|---|---|---|
| 34 | Golden hermetic fixtures via pytest | **Done** | `python/batchalign/tests/test_golden_fixtures.py` — 8 tests against real Catalan + Andrew transcripts in `talkbank-alignment/`. Covers Landing 4 #18/#19/#20 closures plus recipe smokes. |
| 35 | Daemon HTTP smoke (Python TestClient e2e) | **Done** | Commit `9b2ed71`. `test_daemon_e2e.py` boots FastAPI in-process and asserts `/health`, `/capabilities`, `/recipes`, `/backends`, `/openapi.json`, the `X-Batchalign-SHA` response header, and the 404 path. 7 tests. |
| 36 | `tests/json_compat.rs` snapshot tests | **Already covered** | `//apps/batchalign/batchalign-gui:protocol_artifacts` generates the live OpenAPI and capabilities JSON as Bazel outputs instead of checking snapshots into source. |

## Explicitly Skipped (per plan)

These were marked Skip/Defer in `ba3_cutover_plan.md`:

- Free-threaded Python (3.13t) — tokio orchestration covers it.
- `tokio-console` / `py-spy` — premature; tracing is enough.
- Typed callback Protocol types in Stanza — `ipc-schema` already typed.
- `ProcessingContext` dataclass — Pydantic-at-boundary covers it.
- `whisper_hub` — superseded; `whisperx` and `whisper_hub` engines have since been removed entirely (see `backends/asr/` for the current engine set).
- Cantonese particle pre-chunk — wait on sliding-window first.
- Capability detection endpoint — add when a consumer asks.
- Constituency parser in Rust; OTLP; rate limiting.
- Chatter-side CLAN parity (Cat 17), chatter merge (Cat 18).

## Per-session commit log

This session (2026-05-31):

| Commit | Item(s) |
|---|---|
| `09aaeb2` | Baseline: openapi_freshness + pytest green. |
| `ed7afec` | Landing 1 #2 + Landing 7 #30. |
| `12683e9` | Landing 7 #32. |
| `fb233f2` | Docs sweep (building.md + 5 stale-doc banners). |
| `d543566` | Landing 3 #11. |
| `fac81e6` | Landing 2 #7. |
| `b0a89c1` | Landing-status tracker. |
| `974aa0a` | Landing 3 #13 (utseg sliding-window). |
| `e6cfeea` | Landing 2 #8 + Landing 3 #10 (Stanza cache). |
| `954485a` | Landing 8 simple hermetic goldens + Landing 4 #18/#19/#20 closures. |
| `84cf8b2` | Landing 6 #26 (Rev.AI batching) + Landing 6 #29 (Qwen3 standalone FA). |
| `e00293a` | Landing 3 #12 (CA-notation stripping before Stanza). |
| `c71f8d4` | Landing 1 #1 (PyO3 dp_align + Python shim). |
| `6035c32` | Landing 7 #31 (vergen + X-Batchalign-SHA header). |
| `20eae11` | Landing 7 #33 (`#[instrument]` on engine dispatch). |
| `06a3008` | Landing 2 #9 (cooperative cancellation on BatchalignEngine). |
| `598c5f8` | Landing 3 #15 (sibling-media auto-resolution helper). |
| `9b2ed71` | Landing 8 #35 (daemon HTTP smoke e2e). |
