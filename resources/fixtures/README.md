# Test Fixtures

**Status:** Current
**Last updated:** 2026-04-17 17:52 EDT

This directory holds CHAT and CHAT+audio fixtures used by the test
suite. Two layouts coexist:

## Top-level CHAT files (existing convention)

Single-file CHAT fixtures used by unit tests across the workspace.
Names encode the feature under test (e.g. `fa_compound_filler.cha`,
`retok_gonna_eat.cha`, `eng_hello_world.cha`). These are used by
Rust unit tests and golden tests for individual parser,
retokenizer, and FA behaviors that do not need real audio.

Do not move or rename these without grepping the test suite first —
many of them are referenced by absolute path.

## Per-command regression directories (new convention)

```
resources/fixtures/
├── align/regressions/        — gitignored fixture subdirs
├── transcribe/regressions/
├── morphotag/regressions/
├── utseg/regressions/
├── translate/regressions/
└── coref/regressions/
```

These hold real-world bug reproductions for the command-local regression tests
under `crates/batchalign/tests/ml_golden/<command>/regressions.rs`.

**Privacy boundary.** Real-user bug reports usually involve real
corpus audio, CHAT transcripts, and reporter identity — material
that cannot be committed to this public repository. The runner in
this repo is generic scaffolding; the fixture content lives in a
separate private repository,
[`TalkBank/<private-fixtures>`](https://github.com/TalkBank/<private-fixtures>),
that each maintainer clones locally and exposes to the runner via
the `BATCHALIGN3_PRIVATE_FIXTURES_DIR` environment variable:

```bash
# ~/.zshrc or ~/.bashrc
export BATCHALIGN3_PRIVATE_FIXTURES_DIR="$HOME/talkbank/<private-fixtures>"
```

`resources/fixtures/<command>/regressions/` is gitignored below the
README level specifically so private material cannot land here by
accident. The test runner looks up fixtures under
`$BATCHALIGN3_PRIVATE_FIXTURES_DIR/<command>/regressions/<bug>/`
first, falling back to the in-tree path if no match.

Contributors without access to
[`TalkBank/<private-fixtures>`](https://github.com/TalkBank/<private-fixtures>)
will see the regression tests skip gracefully (`SKIP`) rather than fail.

### Bug-directory naming

```
<command>-regression-<NNN>/
```

The public test names and the private fixture directories use matching
opaque slot IDs such as `align-regression-004` or
`transcribe-regression-001`. Keep the human-readable date, reporter,
and corpus context inside the private fixture's `README.md` and
`source.json`, not in the directory name.

### Required files in every regression directory

| File | Purpose |
|------|---------|
| `source.json` | Typed manifest. See schema below. |
| `input.cha` | Optional for CHAT-first commands such as `align`, `morphotag`, `utseg`, `translate`, and `coref`. |
| `input.<ext>` | Optional for audio-first commands such as `transcribe`; also used as sidecar media for commands like `align`. |
| `expected.cha` | Optional. The correct output, if the bug is expressible byte-for-byte. |
| `actual.cha` | Optional. A snapshot of the current output, for documentation. |
| `README.md` | One paragraph: what's wrong, link to the originating bug report, what fixed it. |

### `source.json` schema

```json
{
  "command": "transcribe",
  "language": "eng",
  "audio": "input.mp3",
  "transcribe": {
    "asr_engine": "rev_ai",
    "wor": "omit"
  },
  "source": {
    "report": "<opaque ref to private email thread or ticket>",
    "original_chat": "<opaque ref to private source CHAT>",
    "audio_source": "<opaque ref to private source audio>",
    "trimmed_utterance_range": [149, 152],
    "trimmed_audio_offset_ms": 374000
  },
  "bug": {
    "summary": "<short plain-English description of the failure mode>",
    "class": "transcribe_regression_harness",
    "affected_main_tier_index": 0
  },
  "assertions": [
    {
      "kind": "no_zero_duration_wor_words",
      "main_tier_index": 0
    }
  ]
}
```

`input_chat` is required for CHAT-first commands and optional for audio-first
commands such as `transcribe`.

`transcribe` is optional and currently supports transcribe-local overrides such
as `"asr_engine": "whisper"` / `"rev_ai"`, `"wor": "include"` / `"wor": "omit"`,
and `"diarize": true`.

`assertions` is a list of typed checks. Add variants only when a real
fixture needs them. Utterance-scoped assertions carry `main_tier_index`;
whole-output assertions do not.

Currently supported `kind` values:

- `no_zero_duration_wor_words`
- `min_wor_word_duration_ms`
- `min_last_wor_word_duration_ms`
- `max_wor_word_duration_proportion`
- `max_main_tier_lead_before_first_wor_ms`
- `max_last_wor_overrun_past_main_end_ms`
- `min_main_tier_utterance_count`
- `max_first_main_tier_word_count`
- `no_wor_tiers_present`
- `min_distinct_main_tier_speaker_count`
- `media_header_matches_input_basename`

## Creating a new regression fixture

1. Stage a minimal command input in the shape the command actually consumes.
   For CHAT-first commands, trim a minimal CHAT + audio pair using a structured
   trim helper that preserves timing bullets, rewrites the `@Media` header,
   and rebases word timings to the trimmed audio. For audio-first commands such
   as `transcribe`, stage the minimal audio clip the bug needs. Do not
   hand-roll with `ffmpeg`, `head`, `tail`, or an improvised pipeline —
   reinventing any of that produces fixtures that look correct on inspection
   but silently mis-time alignment.

2. Stage the output into a new directory under the **private
   fixture repository** (not this public repo):
   ```
   $BATCHALIGN3_PRIVATE_FIXTURES_DIR/<command>/regressions/<bug-name>/
   ```

3. Write `source.json`, `actual.cha`, and `README.md` in the same
   private directory. Redact any reporter identity or private
   corpus path; use opaque references.

4. In the public batchalign3 repo, add (or reuse) one command-local test
   function slot in
   `crates/batchalign/tests/ml_golden/<command>/regressions.rs`
   pointing at the new `<bug-name>` directory. Use an opaque
   function name like `align_regression_<n>` or
   `transcribe_regression_<n>` — the runner resolves the real
   directory at test time via `load_fixture`.

5. Run:
   ```bash
   cargo nextest run -p batchalign-cli --profile ml \
        -E 'test(<command>::regressions::<command>_regression_<NNN>)'
   ```

6. Confirm the fixture is RED (captures the bug) or GREEN
   (regression-prevention snapshot), commit the runner change to
   the public repo, and commit the fixture content to the private
   repo.
