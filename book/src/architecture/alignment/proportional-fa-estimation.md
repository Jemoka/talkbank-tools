# Proportional FA Estimation

**Status:** Current
**Last updated:** 2026-05-01 17:07 EDT

When forced alignment runs against a CHAT file with utterances that
have no timing bullets, the FA grouping algorithm falls back to
**proportional estimation**: untimed utterances get an estimated
audio window based on their word-count fraction of the file. This
gives the FA model a reasonable search window so it can produce
real timing, instead of skipping untimed utterances entirely.

## Estimation rule

For each untimed utterance:

```text
estimated_start = (words_before / total_words) * total_audio_ms
estimated_end   = (words_before + this_utt_words) / total_words * total_audio_ms
```

A 2-second buffer is added on each side, clamped to
`[0, total_audio_ms]`:

```rust,ignore
let buffer_ms = 2000;
let start = estimated_start.saturating_sub(buffer_ms);
let end = (estimated_end + buffer_ms).min(total_audio_ms);
```

The FA model (Whisper or Wave2Vec) conditions on transcript text
and finds where it occurs in the audio, so the window only needs
to be approximately correct. If the estimate is off, FA produces
slightly less accurate timing but won't crash or skip the
utterance.

## Mixed files

Files with some timed and some untimed utterances are handled
naturally. Timed utterances use their real bullets. Untimed
utterances use proportional estimates. Both are grouped normally.

## When proportional estimation runs

UTR (Utterance Timing Recovery) is the default first pass —
`inject_utr_timing()` sets utterance bullets from ASR tokens before
FA grouping, so ~100% of utterances get timing from ASR.
Proportional estimation is the fallback:

1. **UTR enabled (default).** UTR populates utterance bullets from
   the ASR pre-pass. Proportional estimation usually doesn't fire.
2. **UTR disabled (`--no-utr`).** Proportional estimation kicks in
   during `group_utterances()` when `total_audio_ms` is available.
   ~96% coverage on test corpora.
3. **UTR partial success.** Individual utterances that UTR could
   not match (the `unmatched` count in `UtrResult`) fall through
   to proportional estimation.
4. **Neither, no `total_audio_ms`.** Untimed utterances are
   skipped from FA grouping (legacy behavior).

The Rust server always passes `total_audio_ms` when available
(`crates/batchalign/src/runner/dispatch/fa_pipeline.rs`). It's a
no-op for pre-timed files (Rust never hits the estimation path).

## Implementation

`group_utterances()` in
`crates/batchalign/src/fa/grouping.rs` takes a
`total_audio_ms: Option<u64>` parameter and uses a two-pass
approach:

1. **First pass** — count total alignable words across all
   utterances (timed and untimed). If `total_audio_ms` is `None`,
   skip untimed utterances as before.
2. **Second pass** — for untimed utterances when `total_audio_ms`
   is `Some`, compute the proportional estimate and use it as the
   bullet.

The PyO3 bridge accepts `total_audio_ms: Option<u64>` in
`ParsedChat.add_forced_alignment(...)`
(`crates/batchalign-pyo3/src/parsed_chat/fa.rs`). The post-processing
loop handles utterances that were untimed on input but received
timing from FA — they need `postprocess_utterance_timings` and
`add_wor_tier` too.

The Python worker (`python/batchalign/inference/fa.py`) computes audio
duration from the loaded `ASRAudioFile`:

```python
duration_ms = int(round(f.tensor.shape[0] / f.rate * 1000))
```

For audio files passed by path (without preloading), `torchaudio.info()`
gives duration without loading the full file.

## Why proportional, not learned

- **No new dependencies** — pure arithmetic, runs in microseconds.
- **Deterministic** — same input always produces same windows.
- **Good enough for FA** — FA only needs an approximate window; it
  does precise alignment within the window using the actual audio
  signal.
- **Graceful degradation** — if the estimate is off, FA may
  produce slightly less accurate timing but won't crash or skip
  utterances.

A learned window predictor would add a model dependency and
training pipeline for marginal gain — proportional estimation is
already accurate enough that the FA window finds the utterance.

## Tests

| Layer | Coverage |
|---|---|
| Rust unit (`fa/grouping.rs`) | Untimed grouped with proportional estimates when `total_audio_ms` is provided |
| Rust unit | Untimed still skipped when `total_audio_ms` is `None` (backward compat) |
| Rust unit | Mixed timed/untimed grouped correctly |
| Rust unit | Buffer clamped to `[0, total_audio_ms]` |
| Python integration | `add_forced_alignment` with untimed CHAT + `total_audio_ms` produces timing |
| Python integration | `add_forced_alignment` with untimed CHAT + no `total_audio_ms` produces no timing (backward compat) |
