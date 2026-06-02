# User Workflow Migration (batchalign2 -> batchalign3)

**Status:** Current
**Last updated:** 2026-06-01 21:36 PDT

This page describes durable differences between:

- the Jan 9 2026 `batchalign2-master` baseline
  `84ad500b09e52a82aca982c41a8ccd46b01f4f2c` for core / non-HK behavior,
- the Jan 9 2026 `BatchalignHK` baseline
  `84ad500b09e52a82aca982c41a8ccd46b01f4f2c` for HK / Cantonese behavior,
- the later released `batchalign2` master-branch point
  `e8f8bfada6170aa0558a638e5b73bf2c3675fe6d` (2026-02-09) where relevant, and
- current `batchalign3`.

It does not document transient unreleased migration-stage behavior.

## 1) Command surface: what changed for daily usage

### Binary/package naming

- batchalign2 CLI entrypoint: `batchalign`
- batchalign3 CLI entrypoint: `batchalign3` (plus Rust binary integrations)

### Historical HK command nuance

For HK / Cantonese work, the preserved legacy command history is slightly
different from stock BA2:

- stock legacy CLI: `batchalign`
- HK legacy CLI: `batchalignhk`
- preserved Jan 9 legacy runners use native directory-I/O invocation:
  `command inputfolder outputfolder`

Current `batchalign3` unifies the modern surface under `batchalign3`, but live
parity checks should still compare against the historically correct legacy
command.

### Command continuity and expansion

Core commands are preserved (align/transcribe/translate/morphotag/coref/utseg).
Relative to the Jan 9 BA2 baseline, batchalign3 also adds operational commands
that were not yet first-class there:

- `serve`, `jobs`, `logs`
- `cache`

### Command crosswalk (BA2 baseline -> BA3)

| BA2 command @ `84ad500` | BA3 equivalent | Notes |
|---|---|---|
| `align` | `align` | same top-level purpose; newer runtime contracts and deterministic remap behavior |
| `transcribe` | `transcribe` | same top-level purpose; expanded engine/runtime routing |
| `translate` | `translate` | same |
| `morphotag` | `morphotag` | same command name, stronger token/validation contracts, multilingual parallel dispatch, and default-on per-word L2 dispatch |
| `coref` | `coref` | same purpose; public in BA3, still English-only, and still local-oriented |
| `utseg` | `utseg` | same |
| `benchmark` | `benchmark` | same high-level goal |
| _(none)_ | `compare` | Added to BA2 master post-Feb 9; present in both current BA2 and BA3 |
| `setup` | `setup` | still initializes local config |
| `version` | `version` | still available |
| _(none)_ | `serve` | BA3-only server control surface |
| _(none)_ | `jobs` / `logs` | BA3-only job/log operational UX |
| _(none)_ | `openapi` | BA3-only contributor-facing API schema export tooling |
| _(none)_ | `cache` | BA3-only first-class cache management relative to the Jan 9 BA2 baseline |
| _(none)_ | `bench` | Not in Jan 9 baseline; present in Feb 9 BA2 and BA3 |
| `models` | `models` | still available; current implementation is behind the Rust CLI/runtime |

Important nuance:

- this table is anchored to the Jan 9 BA2 baseline on purpose;
- by the later released Feb 9 BA2 point, `cache` and `bench` were already
  public and runtime support around `--server` had expanded substantially;
- batchalign3 should therefore be read as a further rewrite/hardening of that
  direction rather than as the first moment operational tooling appeared.

### Comparison discipline for user-facing validation

When validating migration behavior or rebaselining expectations, use the
correct Jan 9 anchor for the material you are testing:

- core / non-HK: Jan 9 `batchalign2-master` pinned to `84ad500...`
- HK / Cantonese: Jan 9 `BatchalignHK` pinned to `84ad500...`

- Use Feb 9 BA2 only when the specific question is about the later released BA2
  master-branch surface.
- Do **not** use later Python operational packages as the migration baseline;
  those represent later deployment/package choices, not the Jan 9 migration
  anchor.

For practical local checks:

- use `scripts/stock_batchalign_harness.py` for curated `benchmark` cases
- use `scripts/compare_stock_batchalign.py` for raw transcript/tier diffs

Both tools should be pointed at the correct `84ad500...` baseline executable,
and preserved legacy runners should keep their native
`command inputfolder outputfolder` syntax.

For HK material in particular, that means comparing against `batchalignhk`, not
stock `batchalign`.

### `transcribe` for daily English work: nothing changed

If you transcribe English audio, your commands work exactly as before:

```bash
# BA2 — --lang defaults to "eng", no need to type it
batchalign transcribe recordings/ output/

# BA3 — same default, same behavior
batchalign3 transcribe recordings/ -o output/
```

The only required change is the binary name (`batchalign3`) and the preferred
output flag (`-o` instead of positional). `--lang` still defaults to `"eng"`.
You do not need to type `--lang eng` unless you want to be explicit.

**New in BA3: `--lang auto`.** This is an *optional* feature for bilingual or
code-switched recordings where you don't want to pick a single language.
Whisper's multilingual model auto-detects the spoken language from the audio.
You never need `--lang auto` for monolingual English work.

**`=` sign syntax.** Both `--lang eng` and `--lang=eng` are identical and have
always been identical (this was true in BA2 as well). Use whichever you prefer.

### Flag behavior and defaults

The Feb-9-BA2-era global flags listed below were **not** carried
into BA3. A Feb-9 BA2 script that passes any of them gets a clap
parse error, not a silent no-op. They were never in the Jan 9 BA2
baseline either; they were added in Feb 9 master and removed in BA3.

- `--memlog`
- `--mem-guard`
- `--adaptive-workers` / `--no-adaptive-workers`
- `--pool` / `--no-pool`
- `--adaptive-safety-factor`
- `--adaptive-warmup`
- `--shared-models` / `--no-shared-models`
- `--lazy-audio`

There are hidden per-command BA2 aliases that still parse and map onto the
current typed options:

- `align`: `--whisper`, `--rev`, `--whisper-fa`, `--wav2vec`
- `transcribe`: `--whisper`, `--whisper-oai`, `--rev`, `--diarize`, `--nodiarize`
- `benchmark`: `--whisper`, `--whisper-oai`, `--rev`

**Removed in BA3:** `--whisperx` (BA2 only). The WhisperX engine no longer
ships with BA3; the flag is not recognized. Use `--engine whisper` (HF) or
`--engine openai` instead. The BA3 engine set is `rev, whisper, chatwhisper,
openai, funaudio, tencent, qwen3`.

**`transcribe` diarization semantics are now identical in BA2 and BA3.** Both
systems keep `--diarize` as an opt-in path with a `False` / `auto` default.
If you use Rev.AI (the default engine), speaker labels are already part of the
ASR response and are always applied — you get multi-speaker output in both BA2
and BA3 without `--diarize`, and BA-side utterance segmentation still runs
separately. When `--diarize` is explicitly requested, both Jan 9 BA2 and
current BA3 run the separate Pyannote speaker stage as post-processing on top
of the ASR output, including Rev-labeled transcripts. The flag remains most
important for Whisper-based workflows where the ASR engine does not produce
speaker labels at all. BA2's old help text claiming Rev ignored `--diarize`
was stale; the implementation did not.

### Utility command migration

The operational command surface also changed in stages:

| Command family | Jan 9 BA2 | Feb 9 BA2 | Current BA3 |
|---|---|---|---|
| `setup`, `version`, `models` | present | still present | still present, but behind Rust CLI |
| `cache` | absent as public CLI | public command | public command with Rust CLI/server integration |
| `bench` | absent | public command | public command |
| `serve`, `jobs`, `logs` | absent | not public CLI commands in released BA2 master | public BA3 utility/ops surfaces |
| `openapi` | absent | not public CLI command in released BA2 master | contributor-facing BA3 utility surface |

Per-command details (see [comparison states](index.md#comparison-states-and-policy)
for the three-state framing):

- `setup`:
  - Jan 9 / Feb 9 BA2: Python-side config wizard for `~/.batchalign.ini` and
    Rev.AI defaults
  - current BA3: same public purpose, but implemented in Rust with explicit
    interactive/non-interactive validation
- `models`:
  - Jan 9 / Feb 9 BA2: public training entrypoint mounted directly from the
    Python training runtime
  - current BA3: still fundamentally a Python training surface; the Rust CLI
    forwards to the Python runtime rather than re-implementing training logic
- `version`:
  - Jan 9 / Feb 9 BA2: version surfaced through Click root-command metadata
  - current BA3: explicit `version` subcommand with package version plus build
    hash
- `cache`:
  - Jan 9 BA2: no public cache-management command
  - Feb 9 BA2: Python `cache` command for stats/clear/warm against Python-side
    cache-manager state
  - current BA3: Rust `cache` command for analysis/media cache inspection and
    clearing aligned to the current SQLite/media-cache runtime
  - practical delta:
    `cache clear --all` also removes permanent UTR cache entries, while BA2's
    `cache warm` prewarm flow is not carried forward
- `bench`:
  - Jan 9 BA2: no public benchmarking command
  - Feb 9 BA2: Python repeated-dispatch timing helper with runtime toggles
  - current BA3: Rust repeated-dispatch benchmarking with typed options and
    structured output for regression work
- `serve`, `jobs`, `logs`:
  - absent from Jan 9 and released Feb 9 BA2 as public CLI commands
  - current BA3: real server/job operations surface, reflecting the shift from
    one-shot local execution toward explicit daemon/server/job control
- `openapi`:
  - absent from Jan 9 and released Feb 9 BA2
  - current BA3: contributor-facing API/schema export tooling rather than a
    normal end-user workflow
- daemon/server routing:
  - released Feb 9 BA2 already had richer local runtime controls
  - current BA3: dispatch distinguishes commands that prefer the local daemon
    from commands that can target a remote server directly. The set that
    prefers local-daemon execution is `transcribe`, `transcribe_s`,
    ignored in favor of the local daemon. (See `command_prefers_local_daemon`
    in `crates/batchalign/src/cli/dispatch/mod.rs`.) A separate sidecar
    daemon profile exists in `cli/daemon.rs` for transcribe workloads that
    need a different Python environment, but the current dispatch code does
    not auto-route commands to it on capability mismatch — it is started
    and stopped through `serve` lifecycle commands.

## 1.1) Biggest durable user-visible changes

If you are coming from BA2, the changes most likely to affect real corpus
results are:

- `morphotag` correctness is stronger: `%mor`/`%gra` generation now runs against
  a structured CHAT representation and preserves token provenance more
  consistently. Multilingual files process all languages in parallel
  (semaphore-bounded cross-language dispatch, see `morphosyntax/batch.rs`
  in `crates/batchalign/`), and large single-language batches are split
  across multiple workers (up to `DEFAULT_MAX_WORKERS_PER_KEY`) — the
  observed practical effect is substantially faster wall-clock time on
  multilingual corpora, though no published benchmark anchors a specific
  multiplier.
- retokenization is more predictable: Batchalign3 no longer relies on runtime
  global DP remapping to reconcile Stanza output back to CHAT.
- alignment and timing writeback now preserve stable identity and explicit order
  more often instead of reconstructing results from flattened strings later.
- repeated runs are materially faster: utterance-level caching and daemon/server
  execution remove much of the Jan 9 BA2 per-file process startup cost.
- long runs are easier to operate: job/log/status surfaces replace much of the
  Jan 9 BA2 "watch one terminal and inspect files later" model.

Some of these improvements were already present in the Feb 9 BA2 release
(see [comparison states](index.md#comparison-states-and-policy)); BA3
adds the Rust-first control plane and stronger CHAT-ownership boundaries.

## 2) Runtime mode: local CLI vs daemon/server discovery

In batchalign2, most workflows were "run command locally, wait, inspect files."
Batchalign3 supports that, but also supports:

- local daemon-backed execution,
- server-managed job queues and status APIs,
- explicit operational commands such as `serve`, `jobs`, and `logs`.

UI consequence: in addition to terminal progress, the modern stack supports
dashboard-style and API-style operational visibility (`jobs`, `logs`, health and
OpenAPI surfaces), which substantially changes how teams monitor long runs.

## 2.1) UI migration notes

- **CLI UX**: still primary for batch workflows, but now with explicit operational
  subcommands rather than implicit one-shot process assumptions.
- **Server/API UX**: job/status endpoints support automation and remote control
  workflows that BA2 users previously handled with custom shell glue; `openapi`
  is the contributor-facing schema export surface for that API.
- **Dashboard UX**: the server-hosted web dashboard is real when dashboard
  assets are installed. What is deferred from the first public `batchalign3`
  release is the separate desktop/Tauri launcher path, not the web dashboard
  itself.
- **Editor UX (ecosystem)**: LSP/VS Code integrations now prefer structured
  alignment sidecars where available, reducing regex-only timing extraction drift.

This changes how users should think about failures/retries:

- prefer `jobs`/`logs` inspection over searching ad-hoc terminal output,
- use explicit cache controls for reproducibility and reruns,
- treat processing as resumable jobs instead of monolithic one-shot runs.

## 3) Alignment behavior users will notice

This section is about user-visible consequences. The mechanism-level story
(ID-first timing transfer, retokenization mapping, `%gra` validation, and the
reduced role of broad runtime DP remap) lives in
[Algorithms, Language, and Alignment Migration](algorithms-and-language.md).

### Realign-after-edit behavior

Old BA2 workflows often resolved transcript edits by broad remap over flattened
text. With repeated words, retraces, or overlap, that could produce unstable
timing reassignment.

Current BA3 prefers deterministic transfer and explicit untimed outcomes:

- fewer surprise timing jumps across utterances,
- clearer unresolved cases instead of silent "best fit" remaps,
- more stable `%wor` and bullet writeback ordering.

The released Feb 9 BA2 point had already improved `align` materially relative to
Jan 9, but current BA3 is where the transfer/writeback policy becomes much more
consistently identity-aware and validation-driven.

### Retokenization and `%mor` / `%gra` differences

If you compare corpus outputs, expect some `%mor` / `%gra` differences to be
corrections, not regressions.

The user-visible changes that matter most are:

- `%gra` root attachment now follows `head=0`,
- invalid root/head structures are rejected instead of written out,
- MWT and contraction handling are more stable,
- special forms such as `@c`, `@s`, and `xbxxx` are handled more explicitly,
- reflexive pronouns emit `reflx`,
- `retokenize=false` preserves original tokenization instead of silently
  rewriting it.

Important comparison nuance:

- some special-form and pronoun behavior already existed in BA2,
- the durable BA3 change is that these behaviors now sit inside structured
  mapping and validation rather than positional repair.

### Alignment and morphotag migration in two steps

| Area | Jan 9 BA2 -> Feb 9 BA2 | Feb 9 BA2 -> current BA3 |
|---|---|---|
| `align` | released BA2 already improved cache use, failure handling, and runtime robustness | FA grouping, timing injection, `%wor`, monotonicity handling, and much of the parse/cache/infer/inject flow move into Rust orchestration |
| `morphotag` | released BA2 already improved caching, DP/robustness edges, and internal cleanup | `%mor`/`%gra` mapping and injection gain explicit root/head/chunk validation, a clearer Rust-owned CHAT boundary, and semaphore-bounded concurrent file dispatch |

For users, the practical current-state rule is:

- current alignment prefers deterministic transfer and explicit untimed
  outcomes over silent global remap choices;
- current morphotag output is more strongly validated, so some BA2-to-BA3 corpus diffs
  should be treated as bug fixes.

### Other commands

| Command | Jan 9 BA2 -> Feb 9 BA2 | Feb 9 BA2 -> current BA3 |
|---|---|---|
| `transcribe` | Python pipeline becomes faster and more robust, especially in dispatch/startup/long-audio handling | Python stops owning transcript construction; Rust owns postprocess, CHAT assembly, and optional downstream stages |
| `translate` | mostly lazy-load/runtime cleanup, not a major algorithm shift | Rust takes over CHAT extraction, cache, validation, and `%xtra` injection; Python becomes pure text inference |
| `utseg` | same Python constituency + DP alignment algorithm, with cache/lazy-load cleanup | Python returns raw trees; Rust computes assignments and mutates CHAT directly |
| `coref` | essentially same document-level Python+DP remap path | Python returns structured chains; Rust injects sparse `%xcoref` and enforces output policy |
| `benchmark` | runtime/dispatch and benchmarking UX improve, but still Python-owned command flow | Rust now owns benchmark orchestration end to end; Python only contributes raw ASR inference when needed, and current BA3 still honors `--wor` / `--nowor` |

## 4) Multilingual and language-specific changes users will notice

### Code-switching and `@s` policy

Relative to BA2, current BA3 changes both the analysis path and the transcript
contract for `@s`:

- per-word `@s` / `@s:LANG` routing is now default-on for morphotag; use
  `--no-l2-morphotag` if you need the older `L2|xxx` placeholder behavior for
  reproducibility
- explicit `@s:LANG` still routes to `LANG` even when `LANG` is missing from
  `@Languages`, but validation emits warn-only E254 so the header mismatch is
  visible
- whole-utterance same-language all-`@s` runs are no longer accepted as an
  utterance-language shorthand; BA3 validates them as E255 and expects `[- lang]`
  instead
- `chatter debug fix-s` is the companion repair tool for migrated corpora: it
  rewrites qualifying whole-utterance `@s` runs, appends missing explicit
  languages to `@Languages`, and skips already-correct files

### Stanza multi-word token (MWT) outputs

Batchalign3 handles tokenizer expansions (one orthographic token -> multiple UD
units) more predictably than BA2-era remap-heavy paths.

This matters directly for migration because BA2-era outputs could drift when
tokenizer-created words were later forced back onto CHAT via heuristic remap.
Batchalign3 keeps the original CHAT structure as the primary truth and maps UD
analysis back onto it deterministically. The mechanism details live in
[Algorithms, Language, and Alignment Migration](algorithms-and-language.md).

### Japanese and other language preprocessing/postprocessing

Key upgrades include stronger language-aware normalization and postprocessing
guards (including Japanese-specific morphology handling and punctuation/token
cleanup pipelines) to reduce downstream alignment and `%mor` drift.

## 4.1) Structural theme across text commands

The command table above already covers the command-by-command migration story.
The shared implementation consequence is:

- Python is now a pure model server: it returns raw inference outputs and
  contains zero CHAT parsing, zero text normalization, and zero domain logic.
- Rust owns all CHAT parsing, payload extraction, validation, reinjection,
  and output policy — a complete inversion of BA2's Python-owns-everything
  architecture, enforced by allowlist tests.

That pattern is why `transcribe`, `translate`, `utseg`, and `coref` now behave
more predictably than the older Python-monolithic paths. The same separation
also explains why `benchmark` is easier to reason about in BA3: Rust owns the
composed workflow boundary instead of hiding it inside Python dispatch glue.


These are lighter migration targets than the CHAT-processing commands above.
For most users the command lines did not change:

- feature-set and language options are preserved

The important durable differences are operational rather than algorithmic:

- BA3 runs them behind typed prepared-audio V2 worker contracts
- failures surface through job/log tooling instead of ad hoc `.error.txt`
  hunting during server/daemon runs

One user-visible migration note does matter for scripting:

  than the transposed BA2 dataframe export shape (feature names as rows)

## 5) Migration checklist for existing users

1. Update the binary name in your scripts: `batchalign` → `batchalign3`.
   Output is now an `-o` flag instead of a positional argument.
2. Remove any Feb-9-BA2-era global flags from your scripts. The
   following were never carried into BA3 and will produce a parse
   error: `--memlog`, `--mem-guard`, `--adaptive-workers`,
   `--no-adaptive-workers`, `--pool`, `--no-pool`,
   `--adaptive-safety-factor`, `--adaptive-warmup`, `--shared-models`,
   `--no-shared-models`, `--lazy-audio`. (Per-command hidden BA2
   aliases like `--whisper`, `--rev`, `--diarize` still work — they
   translate to current typed flags.)
3. Validate your expected outputs against current golden behavior,
   especially if your corpus has overlap / retraces / repetitions.
4. Rebaseline any `%mor` / `%gra` expectations that depended on BA2
   bugs or unstable remap behavior. Compare against current intended
   outputs, not Jan 9 BA2 accidental ones.
5. If you previously relied on out-of-tree Python integration code,
   port it to subprocess calls into `batchalign3`. There is no
   public Python API in BA3 — see
   [No Python API](../user-guide/python-api.md).
   check whether it assumed the old BA2 transposed CSV layout and
   update it for BA3's row-oriented output.
7. Adopt `jobs` / `logs` / `cache` operational commands for
   repeatability.
8. For editor media workflows, use sidecar / timing-index aware
   tooling where available (instead of bullet-regex-only extraction).
