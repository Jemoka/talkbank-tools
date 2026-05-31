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
| 5 | UTR ASR cache (BLAKE3 over audio bytes) | **Queued** | Engine-layer; cache infra (`crates/.../cache.rs`) is in place. |
| 6 | FA word-timing cache | **Queued** | Same shape as #5. |
| 7 | `batchalign3 cache {path,stats,clear}` | **Done** | Commit `fac81e6`; tests in `test_cache_cli.py`. |
| 8 | Per-(lang_set, mode) Stanza pipeline cache | **Queued** | Needs closure-state refactor in `backends/morphosyntax/stanza.py` (`_current_sentence` capture). |
| 9 | `CancellationToken` propagated into `BatchalignEngine.run()` | **Queued** | Touches Rust runtime. |

## Landing 3 — Correctness Tier 1

| # | Item | Status | Notes |
|---|---|---|---|
| 10 | MultilingualPipeline → `dict[(frozenset[lang], mode)] → Pipeline` | **Queued** | Blocked by #8 closure refactor. |
| 11 | Clear `%mor`/`%gra` before morphotag re-run | **Done** | Commit `d543566`; `--clear-existing/--keep-existing` flag. |
| 12 | CA-notation stripping for Stanza input | **Queued** | Rust morphosyntax/cleanup path. |
| 13 | Utseg sliding-window for long inputs | **Queued** | `backends/utseg/chatutterance.py`. |
| 14 | ASR Stage 3c boundary-quote strip | **Queued** | Rust `asr_postprocess`. |
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
| 18 | %gra wraparound on Catalan/Spanish aphasia | **Investigate** | Construct fixture from aphasia-data 426 corpus; assert no negative-wrap in `morphosyntax/injection.rs`. |
| 19 | Single-ROOT invariant on L2 splice | **Investigate** | Add E723 regression test against splice output; `assert_joint_root_invariant` already present. |
| 20 | `%wor` filter masking real errors | **Investigate** | Grep `.filter(` in `morphosyntax.rs`; remove only if masking. |
| 21 | UTR DP fallback constrained to utterance windows | **Investigate** | Run windowed vs free-rolling on stripped-audio fixtures; ship only if no-worse. |

## Landing 5 — Server hardening

| # | Item | Status | Notes |
|---|---|---|---|
| 22 | SQLite-backed JobRegistry (replaces in-memory) | **Queued** | Rust runtime + migrations; ~80 LOC. |
| 23 | Paths-mode `POST /jobs/paths` | **Queued** | Blocked by #22 (single source of truth for job state). |
| 24 | `media_paths_root` config block | **Queued** | Ansible role already supports `batchalign_media_paths_root`. |
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

## Landing 8 — Test backfill

| # | Item | Status | Notes |
|---|---|---|---|
| 34 | `crates/batchalign/batchalign-engine/tests/golden/` skeleton + 1 fixture per recipe | **Queued** | Each fixture needs ≤30s trimmed audio + expected `.cha`. |
| 35 | `tests/daemon_e2e.rs` | **Queued** | Spins up FastAPI in-process, submits per-recipe jobs via HTTP. |
| 36 | `tests/json_compat.rs` snapshot tests | **Queued** | Pydantic Job/JobFile/ProgressEvent shapes. |

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

Net: 4 plan items shipped + 1 partially shipped (Qwen3 FA built-in
already); 4 verified already-implemented (#3, #15, #16, #17); 1 N/A
(#25); 1 no-op (#4); 26 items queued for follow-up sessions.
