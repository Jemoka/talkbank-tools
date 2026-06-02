# Batchalign Command I/O Parity: Local CLI vs Server

**Status:** Current
**Last updated:** 2026-06-01 00:51 PDT

This document describes the input/output flow for every batchalign command,
comparing direct local CLI execution with the server-based (`--server`)
dispatch.
For implementation details, treat the command-owned entrypoints under
`crates/batchalign/src/commands/` plus the owning orchestrator modules
(`compare.rs`, `benchmark.rs`, `transcribe/`, `fa/`, and `morphosyntax/`) as
the source of truth for command semantics. The CLI and runner layers should
stay thin.

For each command: what goes in, where it comes from, what gets written, and
whether files are mutated in place.

---

## Global Path Semantics

Most processing commands use shared `CommonOpts`:

```bash
batchalign3 <command> PATH [PATH ...] [-o OUTPUT_DIR] [--file-list FILE] [--in-place]
```

- Inputs can be files and/or directories.
- `-o/--output` omitted means direct-write behavior for mutating commands.
- `--file-list` is its own input mode: the file's contents become the input path set.
- `--in-place` is available on commands that use `CommonOpts`.

Exceptions:

- `batchalign3 opensmile INPUT_DIR OUTPUT_DIR`
- `batchalign3 avqi INPUT_DIR OUTPUT_DIR`

For legacy readability, the tables below still use `IN_DIR`/`OUT_DIR` shorthand.
Interpret `IN_DIR` as "input path set" in current CLI usage.

When you are adding a new command or changing an existing one, remember the
current architecture split:

- CLI args live in `crates/batchalign`
- released-command identity and top-level orchestration live in
  `crates/batchalign/src/commands/`
- shared command-shape metadata lives in
  `crates/batchalign/src/command_family.rs`
- reusable text-batch helper types live in
  `crates/batchalign/src/text_batch.rs`
- job lifecycle / queueing live in `crates/batchalign/src/runner/`
- output materialization belongs with the owning command or orchestrator module

When output resolves to the same path as input, mutating commands overwrite the
original `.cha` file (no automatic backup).

For generation commands such as `transcribe` and `benchmark`, omitting `-o` or
passing `--in-place` still creates new output files next to the source media; it
does not rewrite the media input.

### Submission retry semantics

Every command eventually reaches the server via `POST /jobs`. The CLI's
`BatchalignClient::submit_job` retries transient connect/timeout failures
with exponential backoff (3 attempts, starting at 2.0 s with jitter) and
**never retries HTTP 4xx/5xx responses** — those are deterministic server
rejections (validation error, conflict, payload too large, panic). This
retry contract is shared by every command in the tables below; it is not
command-specific. See
[Submit-path retries](../architecture/observability.md#submit-path-retries)
for the sequence diagram. The retry is load-bearing because the
local daemon has a brief accept-gap window during job finalization
when a fresh submission can transiently get `Connection refused`.

---

## Command Reference

### 1. align

**Purpose:** Add word-level and utterance-level time alignment to existing
CHAT transcripts by running forced alignment against the corresponding audio.

| Aspect | Local CLI | Explicit remote `--server` |
|--------|-----------|---------------------|
| **Input files** | `.cha` files in `IN_DIR` | `.cha` content sent over HTTP |
| **Input media** | Audio referenced by `@Media:` header, found adjacent to `.cha` or via `--media-dir` | Server resolves audio from `@Media:` against its own visible filesystem (`media_roots`, `media_mappings`, or `--media-dir`) |
| **Extensions filter** | `["cha"]` | Same |
| **Output** | `.cha` with `%wor` timing line, word `time` fields populated | Same `.cha` returned to the client, which writes it to the requested output path |
| **Mutation** | If `OUT_DIR = IN_DIR`: overwrites original `.cha` in place. Media files untouched. | Same |
| **Key options** | `--utr-engine`, `--utr-engine-custom`, `--utr-strategy`, `--fa-engine`, `--fa-engine-custom`, `--pauses`, `--wor/--nowor`, `--override-media-cache` | All passed through typed command options |

**What changes in the `.cha`:** `%wor` tier added/updated with word-level
timestamps. Utterance-level bullet times (`\x15start_end\x15`) updated.
Existing `%mor`, `%gra` tiers preserved. Media file is read but never modified.

**Non-matching files:** For directory inputs, the current Rust CLI copies
non-`.cha` files and dummy CHAT files from `IN_DIR` to `OUT_DIR` before
submitting matching files, in both explicit-server content mode and local
paths-mode preparation.

---

### 2. transcribe

**Purpose:** Create a new CHAT transcript from audio files via ASR.

| Aspect | Local CLI / direct host | Explicit remote `--server` |
|--------|--------------------------|-----------------------------|
| **Input files** | `.mp3`, `.mp4`, `.wav` files in `IN_DIR` | Media filenames only; the server must resolve the audio on its own filesystem |
| **Extensions filter** | `["mp3", "mp4", "wav"]` | Same |
| **Output** | New `.cha` files (audio extension replaced: `foo.wav` → `foo.cha`) | Same `.cha` files returned to the client, which writes them locally |
| **Mutation** | **Never mutates input.** Creates new `.cha` files in `OUT_DIR`. Original audio untouched. If `OUT_DIR = IN_DIR`, the new `.cha` appears alongside the audio. | Same |
| **Key options** | `--asr-engine`, `--asr-engine-custom`, `--diarization`, `--wor/--nowor`, `--lang`, `-n`, `--batch-size` | Same |

**Current routing note (Rust CLI):** when `auto_daemon` is enabled (the
default), `transcribe`-family commands try the local daemon first. Explicit
remote `--server` remains the fallback when that daemon path is disabled or
unavailable.

**What gets created:** A new `.cha` file per audio file. Contains `@Comment`
line with Batchalign version and ASR engine name, `@Languages`, `@Participants`,
`@ID`, and utterance lines with timing. No `%mor`/`%gra` tiers.

**Segmentation note:** speaker attribution and utterance segmentation are
separate. With Rev.AI, BA3 uses the provider's speaker labels even without
`--diarize`, but it still re-segments the transcript into utterances. For
English, Mandarin, and Cantonese, BA3 uses dedicated utterance-boundary models
before CHAT assembly; for other languages, BA3 uses the later `utseg` stage.

**Rev.AI `--lang auto` note:** `--lang auto` is not always equivalent to
explicit `--lang eng`, even when the final transcript is treated as English.
There are two internal paths:

1. **Language ID succeeds before transcript submission** — BA3 resolves the
   request to English up front, and the Rev request path matches explicit
   `--lang eng`.
2. **Language ID fails or returns an unmapped code** — BA3 submits a true Rev
   auto request. Later stages may still resolve the resulting transcript to
   English for segmentation and CHAT headers, but provider-side request options
   differ from explicit English.

This distinction matters because provider punctuation, diarization, and turn
boundaries can differ across those two request paths.

**Note on hidden BA2 aliases:** Hidden compatibility flags such as `--diarize`,
`--whisper`, and `--rev` still parse, but they are migration shims. Public docs
should prefer `--diarization` and `--asr-engine`.

### 3. transcribe_s (transcribe --diarize)

Identical to `transcribe` above, except the pipeline may run a dedicated
Pyannote speaker diarization stage when separate diarization is needed. Output
`.cha` files have multiple `@Participants` and speaker-attributed utterances.
Not a separate CLI command — triggered by `batchalign3 transcribe --diarize`.

**When to use:** This path is primarily for Whisper-based transcription
(`--engine whisper`, `chatwhisper`, `openai`), where the ASR engine does
not return speaker labels. For Rev.AI (the default engine), speaker labels are
already present in the ASR response and are always applied without
`--diarize`, so the normal Rev.AI path already produces speaker-attributed
output. When `--diarize` is explicitly requested, BA3 runs the dedicated
Pyannote stage as a post-processing speaker relabeling step even on top of
Rev-labeled output, matching `batchalign2-jan9`. Utterance segmentation remains
a separate step from diarization in both paths, and still runs on the default
Rev.AI path.

**BA2 parity note:** BA2's CLI/pipeline wiring for `transcribe_s` is
`asr,speaker`, and its speaker processor relabels already built utterances from
Pyannote segments. BA3 now follows that audited BA2 behavior for explicit
`--diarize`.

---

### 4. morphotag

**Purpose:** Add morphosyntactic analysis (`%mor` and `%gra` tiers) to
existing CHAT transcripts.

| Aspect | Local CLI | Server (`--server`) |
|--------|-----------|---------------------|
| **Input files** | `.cha` files in `IN_DIR` | `.cha` content sent as text |
| **Extensions filter** | `["cha"]` | Same |
| **Output** | `.cha` with `%mor` and `%gra` tiers added/replaced | Same `.cha` returned as text |
| **Mutation** | If `OUT_DIR = IN_DIR`: **overwrites original `.cha` in place**. | Same |
| **Key options** | `--retokenize`, `--skipmultilang`, `--lexicon <CSV>`, `--override-media-cache`, `--merge-abbrev` | All passed. Lexicon CSV is read on the client and injected into typed command options before submission. |

**What changes in the `.cha`:** `%mor` tier added/replaced with POS tags and
lemmas. `%gra` tier added/replaced with dependency relations. Main tier text
may be retokenized if `--retokenize` is set. Special `%mor` notation
(`@Options: dummy`) is auto-detected and preserved.

**No media involved.** This is a text-only operation.

---

### 5. utseg

**Purpose:** Segment a transcript into utterances using Stanza.

| Aspect | Local CLI | Server (`--server`) |
|--------|-----------|---------------------|
| **Input files** | `.cha` files in `IN_DIR` | `.cha` content sent as text |
| **Extensions filter** | `["cha"]` | Same |
| **Output** | `.cha` with utterance boundaries recomputed | Same |
| **Mutation** | If `OUT_DIR = IN_DIR`: **overwrites original `.cha` in place**. | Same |
| **Key options** | `--lang`, `-n`, `--merge-abbrev` | All passed |

**What changes in the `.cha`:** Utterance boundaries (`*SPK:` lines) are
recomputed. Existing `%mor`/`%gra` tiers may be invalidated (would need
re-running morphotag afterwards).

**No media involved.**

---

### 6. translate

**Purpose:** Add English translations to non-English transcripts.

| Aspect | Local CLI | Server (`--server`) |
|--------|-----------|---------------------|
| **Input files** | `.cha` files in `IN_DIR` | `.cha` content sent as text |
| **Extensions filter** | `["cha"]` | Same |
| **Output** | `.cha` with translation tiers | Same |
| **Mutation** | If `OUT_DIR = IN_DIR`: **overwrites original `.cha` in place**. | Same |
| **Key options** | `--merge-abbrev` | Passed |

**What changes in the `.cha`:** Translation tier added to each utterance.

**No media involved.**

---

### 7. coref

**Purpose:** Add coreference annotations to transcripts.

| Aspect | Local CLI | Server (`--server`) |
|--------|-----------|---------------------|
| **Input files** | `.cha` files in `IN_DIR` | `.cha` content sent as text |
| **Extensions filter** | `["cha"]` | Same |
| **Output** | `.cha` with coreference annotations | Same |
| **Mutation** | If `OUT_DIR = IN_DIR`: **overwrites original `.cha` in place**. | Same |
| **Key options** | `--merge-abbrev` | Passed |

**No media involved.**

---

### 8. compare

**Purpose:** Compare CHAT transcripts against gold-standard references to compute
word error rate (WER) and inject per-utterance comparison annotations.

| Aspect | Local CLI | Server (`--server`) |
|--------|-----------|---------------------|
| **Input files** | `.cha` files in `IN_DIR` | `.cha` content sent as text |
| **Gold files** | `FILE.gold.cha` in same directory as `FILE.cha` | Gold files sent alongside main files, or read from server filesystem in paths mode |
| **Extensions filter** | `["cha"]` | Same |
| **Output** | `.cha` with `%xsrep` / `%xsmor` tiers + `.compare.csv` metrics | Same — client writes both files to `OUT_DIR` |
| **Mutation** | If `OUT_DIR = IN_DIR`: **overwrites original `.cha` in place**. Gold files are never modified. | Same |
| **Key options** | `--lang`, `--merge-abbrev`, `--override-media-cache` | All passed through typed command options |

**What changes in the `.cha`:** The released output is the projected
gold/reference transcript written at the main file's output path. BA3
morphotags the main transcript, keeps the gold transcript raw during artifact
construction, projects structurally safe `%mor` / `%gra` / `%wor` information
onto the gold AST, and injects `%xsrep` / `%xsmor` on that projected reference
output. `%xsrep` uses `word`, `+word`, and `-word`; `%xsmor` mirrors the same
alignment with POS tags such as `NOUN`, `+ADJ`, and `-?`. Those tiers are now
materialized from typed compare-tier models and lowered once at the final CHAT
serialization boundary.

**Additional output:** A companion `.compare.csv` file is written alongside each
`.cha` output with aggregate metrics (WER, accuracy, match/insertion/deletion
counts, total word counts) plus per-POS rows. The CSV is emitted from a typed
metrics table model via the Rust `csv` crate, not by assembling row strings by
hand.

**Gold file convention:** For each `FILE.cha`, the gold companion is
`FILE.gold.cha` in the same directory. Files ending in `.gold.cha` are
automatically skipped as inputs (they are companions). If no gold file is
found, the file is marked as failed with an error message.

**Pipeline:** pair main + gold → morphosyntax on main only → parse raw gold →
BA2-style per-gold-utterance local-window alignment → `ComparisonBundle`
(main view, gold view, structural word matches, metrics) → materialization. The
command-owned compare layer now models compare as a reference-projection
command rather than "just another per-file mutator." The semantic unit is the
comparison bundle, not a flat text rewrite.

**Output shapes:** compare can materialize more than one view of the same
comparison bundle. The released command now emits the projected reference view.
Benchmark-style flows can still materialize a main-annotated view internally.
The projection path works over the CHAT AST: exact structural matches can copy
`%mor` / `%gra` / `%wor`, while partial matches stay conservative instead of
reconstructing tiers from strings. Compare parity is semantic — the workflow
matches BA2 behavior without copying BA2's string/document shell.

**No media involved.** This is a text-only operation.

---

### 9. benchmark

**Purpose:** Run ASR and evaluate word accuracy against ground truth.

| Aspect | Local CLI | Explicit remote `--server` |
|--------|-----------|---------------------|
| **Input files** | `.mp3`, `.mp4`, `.wav` files in `IN_DIR` | Media filenames only; the server must resolve the audio on its own filesystem |
| **Extensions filter** | `["mp3", "mp4", "wav"]` | Same |
| **Output** | New `.cha` files with ASR output + eval metrics | Same files returned to the client, which writes them locally |
| **Mutation** | **Never mutates input.** Creates new `.cha` files. | Same |
| **Key options** | `--asr-engine`, `--asr-engine-custom`, `--lang`, `-n`, `--wor/--nowor` | All passed |

**Same I/O pattern as transcribe** — creates new `.cha` files with audio
extension renamed. Additionally includes evaluation metrics from comparing
ASR output against reference transcripts.

`benchmark` is a composite command: it runs transcribe first and then calls a
main-annotated compare path internally. It deliberately shares compare-side
internals, but it does **not** share compare's released projected-reference
contract. If you are changing benchmark behavior, look at the command-owned
Rust layer first rather than adding logic in CLI dispatch.

---

### 10. opensmile

**Purpose:** Extract acoustic features from audio files.

| Aspect | Local CLI | Explicit remote `--server` |
|--------|-----------|---------------------|
| **Input files** | `.mp3`, `.mp4`, `.wav` files in `INPUT_DIR` | Media filenames only; the server must resolve the audio on its own filesystem |
| **Extensions filter** | `["mp3", "mp4", "wav"]` | Same |
| **Output** | `.opensmile.csv` files (NOT `.cha`) | Same `.opensmile.csv` files returned to the client, which writes them locally |
| **Mutation** | **Never mutates input.** Creates new `.opensmile.csv` files in `OUT_DIR`. | Same |
| **Key options** | `--feature-set` (eGeMAPSv02, etc.), `--lang` | All passed |

**Special output:** This is the only command that produces non-CHAT output.

---

### 11. avqi

**Purpose:** Calculate Acoustic Voice Quality Index from paired `.cs`/`.sv`
audio files.

| Aspect | Local CLI | Explicit remote `--server` |
|--------|-----------|---------------------|
| **Input files** | Paired `.cs.*` and `.sv.*` audio files in input paths | Media filenames only; the server must resolve the partner files on its own filesystem |
| **Output** | `.avqi.txt` with metrics per file pair | Same `.avqi.txt` files returned to the client, which writes them locally |
| **Mutation** | **Never mutates input.** Creates new `.avqi.txt` files. | Same |

**Current routing note:** when `auto_daemon` is enabled (the default), `avqi`
prefers the local daemon and ignores explicit `--server`. Explicit remote
`--server` is only used when that daemon path is disabled or unavailable.

**Current syntax note:** `opensmile` and `avqi` do not use the shared `PATHS` /
`-o` command form. Their CLI syntax is positional:

```bash
batchalign3 opensmile INPUT_DIR OUTPUT_DIR
batchalign3 avqi INPUT_DIR OUTPUT_DIR
```

---

## Summary: Input Sources and Mutation Patterns

### Commands that mutate `.cha` files in place (when `OUT_DIR = IN_DIR`)

| Command | Input | What changes |
|---------|-------|--------------|
| **align** | Existing `.cha` + audio | Adds `%wor` tier, updates bullet times |
| **morphotag** | Existing `.cha` | Adds/replaces `%mor` + `%gra` tiers |
| **utseg** | Existing `.cha` | Recomputes utterance boundaries |
| **translate** | Existing `.cha` | Adds translation tier |
| **coref** | Existing `.cha` | Adds coreference annotations |
| **compare** | Existing `.cha` + gold `.cha` | Writes projected reference `.cha` with `%xsrep` / `%xsmor`, plus `.compare.csv` |

These commands read `.cha`, process the `Document`, and write the result
back. When `OUT_DIR = IN_DIR`, the original file is **overwritten**. The
audio files referenced by `align` are read but never modified.

### Commands that create new files (never mutate input)

| Command | Input | Output created |
|---------|-------|----------------|
| **transcribe** | Audio files (`.mp3`/`.mp4`/`.wav`) | New `.cha` files |
| **benchmark** | Audio files | New `.cha` files with eval metrics |
| **opensmile** | Audio files | New `.opensmile.csv` files |
| **avqi** | Paired `.cs`/`.sv` audio | New `.avqi.txt` files |

These commands never touch the input files. The output always has a
different extension or name than the input.

---

## Server Dispatch: What Crosses the Network

The table below describes **explicit remote `--server`** (content mode). On
the local-daemon path the CLI uses paths mode and no file contents cross
the process boundary for any command listed here; see
[Submission Modes](#submission-modes-paths_modetrue-vs-paths_modefalse).

| Direction | Text/CHAT commands (morphotag, compare, ...) | Explicit remote audio commands (`align`, `transcribe`, `opensmile`, ...) |
|-----------|----------------------------------------------|----------------------------------------------------|
| **Client → Server** | Full `.cha` text (~2KB each) | `.cha` text for `align`, or media filenames for media-input commands |
| **Server → Client** | Processed `.cha` text | Processed outputs returned over HTTP and written locally by the client |
| **Media** | No media transfer | Execution host still resolves media from its own visible filesystem |

Audio/video payload bytes **do not** cross the network in the current explicit
server path. The execution host must already have a way to resolve the media.

---

## Submission Modes: `paths_mode=true` vs `paths_mode=false`

Every `POST /jobs` from the CLI carries a `paths_mode` flag. The two modes
differ in what crosses the HTTP boundary and what the server reads from disk.

### Selection rule

```text
paths_mode = allow_paths_mode
          && released_command_supports_paths_mode(command)
          && is_local_server(server_url)
```

- `allow_paths_mode` is set by the CLI dispatch layer. It is `true` for the
  local-daemon path and for an auto-detected loopback server; it is `false`
  for an explicit `--server URL`, even if that URL happens to resolve to
  localhost.
- `released_command_supports_paths_mode(command)` is the authoritative
  predicate, defined at
  `crates/batchalign/src/commands/mod.rs` and re-exported from
  `batchalign::lib.rs`. It reads each command's `io_profile`
  (`CommandIoProfile` on `CommandWorkflowDescriptor` in
  `crates/batchalign/src/commands/spec.rs`) and returns `true` for
  the `PathsModeText` and `PathsModeAudio` variants.
- `is_local_server(url)` at
  `crates/batchalign/src/cli/dispatch/single.rs` returns `true` only for
  `localhost`, `127.0.0.1`, and `::1` (loopback). Any non-loopback host is
  treated as remote, so a `--server http://net:8001` submission stays on
  content mode even when the CLI is running on `net` itself.

Paths mode is therefore strictly a **local, same-filesystem** routing mode.
Remote submissions always use content mode.

### Per-command paths_mode support

| Command | `io_profile` | Why |
|---------|--------------|-----|
| `align` | `PathsModeAudio` | Forced alignment needs audio paths the server can open |
| `transcribe` / `transcribe_s` | `PathsModeAudio` | ASR runs on server-visible media files |
| `benchmark` | `PathsModeAudio` | Composite of transcribe + compare over server-visible media |
| `avqi` | `PathsModeAudio` | Reads paired `.cs`/`.sv` audio directly from the filesystem |
| `morphotag` | `PathsModeText` | Server-side runner reads CHAT input from `source_paths` |
| `utseg` | `PathsModeText` | Same runner as morphotag |
| `translate` | `PathsModeText` | Same runner as morphotag |
| `coref` | `PathsModeText` | Same runner as morphotag |
| `compare` | `PathsModeText` | Reads main `.cha` + gold `.cha` pair by path |
| `opensmile` | `ContentOnly` | Intentional: kept on content mode this round |

Earlier in the project, only the five audio-first commands opted
in. Text-command local submissions were forced onto content mode, which
shipped full CHAT text in the request body. A single 500-file chunk of a
large corpus routinely exceeded `max_body_bytes_mb` (default 100 MB
previously, now 512 MB) and failed the whole chunk with
HTTP 413 Payload Too Large.
Extending paths-mode eligibility to the text commands (via `PathsModeText`)
is the **structural
fix**: on the local path, the request body no longer contains file
contents, so a 413 from a local submission is now unreachable. The 512 MB
default remains as a guard for remote submissions; see the troubleshooting
entry [`server returned 413: length limit exceeded`](../user-guide/troubleshooting.md#server-returned-413-length-limit-exceeded).

### Mode-by-mode detail

| Direction | `paths_mode=true` (local daemon) | `paths_mode=false` (remote `--server`) |
|-----------|----------------------------------|----------------------------------------|
| **Request body** | `JobSubmission { paths_mode: true, source_paths, output_paths, before_paths, display_names, ... }` — path lists only (~KB) | `JobSubmission { paths_mode: false, files: [FilePayload { filename, content }], ... }` — full file bytes inline (can be 100+ MB) |
| **Server read** | Runner opens each `source_paths[i]` directly via `tokio::fs::read_to_string` (see `crates/batchalign/src/runner/dispatch/infer_batched.rs:112-141`) | Runner reads the staged copy from `staging_dir/input/<filename>` that the POST handler wrote before returning 202 |
| **Server write** | Runner writes outputs directly to `output_paths[i]` on the shared filesystem | Runner writes to `staging_dir/output/`; the CLI polls `/jobs/{id}/results/<filename>` and saves each file locally |
| **Body limit (`max_body_bytes_mb`)** | Not a factor — body is a path list | Structural ceiling; operators raise `max_body_bytes_mb` for large remote payloads |
| **Where `DirectHost` fits in** | `DirectHost` is a further optimization used when the CLI falls back to inline in-process execution (no HTTP). It uses the same `source_paths` / `output_paths` convention the local daemon uses, just without the HTTP hop | n/a |

### Request/response sequence

```mermaid
sequenceDiagram
    autonumber
    participant CLI as "CLI dispatch<br/>(crates/batchalign/src/cli/dispatch/single.rs)"
    participant Gate as "Gate: supports_paths_mode(cmd)<br/>&& is_local_server(url)<br/>(single.rs:105-107)"
    participant Paths as "CLI paths builder<br/>(crates/batchalign/src/cli/dispatch/paths.rs)"
    participant Content as "CLI content builder<br/>(crates/batchalign/src/cli/dispatch/single.rs:140-201)"
    participant Srv as "POST /jobs handler<br/>(crates/batchalign/src/routes/jobs/mod.rs:161)"
    participant Runner as "Runner file read<br/>(crates/batchalign/src/runner/dispatch/infer_batched.rs:112)"

    CLI->>Gate: choose mode for (command, url)
    alt Local daemon + supports_paths_mode
        Gate-->>CLI: paths_mode = true
        CLI->>Paths: prepare_paths_submission(...)
        Paths-->>CLI: JobSubmission { paths_mode=true,<br/>source_paths=[...], output_paths=[...] }
        CLI->>Srv: POST /jobs (path lists only, ~KB)
        Srv-->>CLI: 202 Accepted { job_id }
        Srv->>Runner: dispatch job
        Runner->>Runner: read_to_string(source_paths[i])
        Runner->>Runner: write outputs to output_paths[i]
        Note over CLI,Runner: CLI does not download results;<br/>outputs land directly on shared FS.
    else Remote --server (or opensmile)
        Gate-->>CLI: paths_mode = false
        CLI->>Content: classify_files + FilePayload {filename, content}
        Content-->>CLI: JobSubmission { paths_mode=false,<br/>files=[FilePayload{..}], media_files=[..] }
        CLI->>Srv: POST /jobs (full file bytes inline)
        Srv->>Srv: stage files into staging_dir/input/
        Srv-->>CLI: 202 Accepted { job_id }
        Srv->>Runner: dispatch job
        Runner->>Runner: read staging_dir/input/<filename>
        Runner->>Runner: write staging_dir/output/<filename>
        CLI->>Srv: GET /jobs/{id}/results/<filename>
        Srv-->>CLI: result bytes (per file)
        Note over CLI,Srv: CLI writes each downloaded file<br/>into the caller's -o OUT_DIR.
    end
```

Diagram verified against:
`crates/batchalign/src/cli/dispatch/single.rs`,
`crates/batchalign/src/cli/dispatch/paths.rs`,
`crates/batchalign/src/cli/dispatch/mod.rs`,
`crates/batchalign/src/commands/spec.rs`,
`crates/batchalign/src/commands/mod.rs`,
`crates/batchalign/src/routes/jobs/mod.rs`,
`crates/batchalign/src/runner/dispatch/infer_batched.rs`.

### Inline (no-HTTP) fallback: `DirectHost`

When the CLI cannot reach or start any daemon, it falls back to inline
in-process execution via `DirectHost`. `DirectHost` reuses the same
`source_paths` / `output_paths` convention that `paths_mode=true` uses on
the wire — canonical filesystem paths are prepared once and handed to the
direct host. No HTTP hop, no staging directory, no body limit.

**Media resolution:** Same as local CLI — the direct host resolves
`@Media:` headers against its filesystem (same machine, same paths).

---

## Non-Matching File Handling

**Current Rust CLI** (`discover.rs`, `dispatch/single.rs`, `dispatch/paths.rs`):
- Files that don't match the command's extensions are copied from `IN_DIR` to
  `OUT_DIR` for directory inputs.
- Dummy CHAT files (`@Options: dummy`) are copied unchanged and are not
  submitted for processing.
- Matching files are sorted by size descending before submission to reduce
  straggler effects on long runs.

This means current single-server content mode and direct local paths mode are
closer than the older Python split: both preserve non-matching files and both
filter dummy CHAT locally.

---

## Parity Status

| Command | I/O parity | Options parity | Direct local path | Notes |
|---------|------------|----------------|-------------------|-------|
| align | Full | Full | Full | Media resolution differs (local path vs server lookup) but equivalent |
| transcribe | Full | Full | Full | With `auto_daemon: true`, the CLI tries the local daemon first and only warns when that reroute succeeds; otherwise explicit `--server` uses remote content mode with server-side media lookup |
| transcribe_s | Full | Full | Full | Triggered by `--diarize`; follows the same local-daemon-vs-explicit-server rules as `transcribe` |
| morphotag | Full | Full | Full | Lexicon CSV read on client, sent as parsed dict |
| utseg | Full | Full | Full | |
| translate | Full | Full | Full | |
| coref | Full | Full | Full | |
| benchmark | Full | Full | Full | Prefers the local daemon when `auto_daemon` is enabled; explicit `--server` stays the fallback if the daemon path is unavailable |
| opensmile | Full | Full | Full | Special CSV output handling on both sides |
| compare | Full | Full | Full | Gold file resolved locally or server-side |
| avqi | Full (local) | Full (local) | Full | Prefers the local daemon when `auto_daemon` is enabled; explicit `--server` stays the fallback if the daemon path is unavailable |
