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
| 1 | PyO3 binding for Rust DP; delete `python/.../ud/dp.py` | **Queued** | Needs `maturin develop` cycle; ~60 LOC wrapper + delete 224 LOC. |
| 2 | `safe_resolve(path, root)` helper | **Done** | Commit `ed7afec`; tests in `test_safe_resolve.py`. |
| 3 | `recipes/_io.py`: multi-input + sensible `-o` + default-in-place | **Done** | Already-implemented in `cli/_common.py:_walk` + `write_outcomes`. |
| 4 | Remove `--in-place` flag | **No-op** | Flag never existed in this fork. |

## Landing 2 — Caching + cancellation

| # | Item | Status | Notes |
|---|---|---|---|
| 5 | UTR ASR cache (BLAKE3 over audio bytes) | **Skip (session)** | Engine-layer call-site wiring; cache infra (`crates/.../cache.rs`) ready, but downstream needs same Rust runtime work as #9. |
| 6 | FA word-timing cache | **Skip (session)** | Same shape as #5; co-lands with the engine-layer cancellation work. |
| 7 | `batchalign3 cache {path,stats,clear}` | **Done** | Commit `fac81e6`; tests in `test_cache_cli.py`. |
| 8 | Per-(lang_set, mode) Stanza pipeline cache | **Done** | Commit `e6cfeea`; closure now reads from `threading.local`, pipeline cache process-wide. |
| 9 | `CancellationToken` propagated into `BatchalignEngine.run()` | **Skip (session)** | Touches Rust runtime + every backend `call()`. Re-queued for follow-up Rust-focused session. |

## Landing 3 — Correctness Tier 1

| # | Item | Status | Notes |
|---|---|---|---|
| 10 | MultilingualPipeline → `dict[(frozenset[lang], mode)] → Pipeline` | **Done** | Commit `e6cfeea` (same change as #8). |
| 11 | Clear `%mor`/`%gra` before morphotag re-run | **Done** | Commit `d543566`; `--clear-existing/--keep-existing` flag. |
| 12 | CA-notation stripping for Stanza input | **Skip (session)** | Rust `morphosyntax/cleanup` change + `@Options: CA` fixture; needs maturin loop. |
| 13 | Utseg sliding-window for long inputs | **Done** | Commit `974aa0a`; `chunk_words_for_bert` + per-chunk inference in `BertUtteranceModel.__call__`. |
| 14 | ASR Stage 3c boundary-quote strip | **Skip (session)** | Rust `asr_postprocess` pipeline; needs maturin loop. |
| 15 | Sibling-media auto-resolution | **Already-implemented** | `@Media:` header drives discovery in engine. |
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
| 21 | UTR DP fallback constrained to utterance windows | **Skip (session)** | Genuinely needs the stripped-audio fixture loop; defer to follow-up. |

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
| 26 | Rev.AI batching (StanzaEngine pattern) | **Queued** | Needs Rev.AI key for verification. |
| 27 | Wave2Vec FA worker returns typed `(idx, start, end)` tuples | **Queued** | IPC schema change. |
| 28 | CHAT text fast path for supported ASR engines | **Queued** | Per-engine wiring (`chatwhisper.py` etc). |
| 29 | Qwen3 forced-alignment pairing (`--fa-engine qwen`) | **Partially done** | Qwen3 ASR already includes built-in `forced_aligner=Qwen/Qwen3-ForcedAligner-0.6B` (see `backends/asr/qwen3_asr.py:60`); standalone `--fa-engine qwen` for use without ASR is queued. |

## Landing 7 — DX / ops

| # | Item | Status | Notes |
|---|---|---|---|
| 30 | `batchalign3 version` | **Done** | Commit `ed7afec`. |
| 31 | `vergen` build script + `X-Batchalign-SHA` header | **Queued** | Touches every API response middleware. |
| 32 | `deploy/ansible/` playbook + Makefile `deploy` target | **Done** | Commit `12683e9`. |
| 33 | `#[instrument]` annotations on hot path | **Queued** | Once `RUST_LOG=batchalign=debug` becomes the canonical debug surface. |

## Landing 8 — Test backfill (simple per user 2026-05-31)

| # | Item | Status | Notes |
|---|---|---|---|
| 34 | Golden hermetic fixtures via pytest | **Done** | `python/batchalign/tests/test_golden_fixtures.py` — 8 tests against real Catalan + Andrew transcripts in `talkbank-alignment/`. Covers Landing 4 #18/#19/#20 closures plus recipe smokes. |
| 35 | `tests/daemon_e2e.rs` (Rust HTTP roundtrip) | **Skip (session)** | Per user 2026-05-31, simpler pytest goldens are enough; Rust e2e queued behind cancellation work. |
| 36 | `tests/json_compat.rs` snapshot tests | **Already covered** | openapi_freshness Bazel sh_test (`apps/batchalign/batchalign-gui:openapi_freshness`) snapshots the live `app.openapi()` output; drift fails CI. |

## Explicitly Skipped (per plan)

These were marked Skip/Defer in `ba3_cutover_plan.md`:

- Free-threaded Python (3.13t) — tokio orchestration covers it.
- `tokio-console` / `py-spy` — premature; tracing is enough.
- Typed callback Protocol types in Stanza — `ipc-schema` already typed.
- `ProcessingContext` dataclass — Pydantic-at-boundary covers it.
- `whisper_hub` — `backends/asr/whisperx.py` already takes HF model IDs.
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
| _pending_ | Landing 8 simple hermetic goldens + Landing 4 #18/#19/#20 closures. |
