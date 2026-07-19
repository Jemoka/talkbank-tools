# implementation plan

## todo
- [ ] Runs typed pre- and post-stage gates and treats pre-serialization failure as a hard error.
- [ ] Fixes single-word replacement retraces, `@Media` hashing, parser recovery spans, and Japanese token merging.
- [ ] Fixed BA3 output passes `chatter validate`, making the validator the final output-integrity gate.
- [x] Expands compound fillers and recovers their audio spans between recognized words.
- [ ] Detects long-file drift and monotonicity violations, then re-anchors or repairs timing. | FA recovery and repair passes
- [ ] Integrates Cantonese FA through Jyutping and wav2vec2 (no need for the common recovery layer).
- [x] Fails unsupported primary languages per file instead of silently skipping them.
- [ ] Synthesizes non-analyzable special forms such as `@q` and `@n` from typed `form_type` data.
- [ ] Rolls back invalid L2 splices, skips `NoAlign` words, and provides a fallback harness where secondary Stanza coverage is absent.
- [ ] Detects bogus Stanza lemmas, missing fields, and invalid analyses, preserving the surface form and recording anomalies. 
- [ ] Applies numbered, retireable Stanza workarounds for Italian compound imperatives and related defects.
- [ ] Corrects known English transcribe patterns, Chinese bogus lemmas, and Japanese token merging.
- [ ] Preflights large Rev.AI batches up front instead of submitting every file independently. (Please do this without moving revai to rust; keep it in Python.)
- [x] Avoids Whisper MPS `bfloat16` crashes on Apple Silicon.
- [ ] Coordinates transcribe memory, splits work safely, and prevents OOM-created zombie workers.
- [ ] Supports Qwen3-ASR plus Qwen3-ForcedAligner and fixes HK_QWEN backend/type-stub integration.
- [ ] Propagates utterance metadata to children and keeps `ReplacedWord` atomic across splits.
- [x] Uses a sliding dispatch window so huge ASR input lists do not become huge in-flight sets.

## done

### example change title
- **component**: which crate, component, etc.
- **summary**: one-line summary
- **input example**: /file/here
- **tbt output example**: /file/output/example
- **ba3 output, pre-edit**: /file/output/example
- **ba3 output, post-edit**: /file/output/example
- **depends on**: [any commit hash]
- **commit**: [commit hash for the change]
- **new**: yes/no (new, not from the original TODO, or old from the above)

Discussion here, notes, things for me to review. Please keep this example block around.

### Fail unsupported primary morphotag languages per file
- **component**: `batchalign-core` morphosyntax task runner
- **summary**: Rejects unsupported primary `@Languages` values before backend dispatch with an actionable per-file validation error; `@Options: CA` remains a legitimate unchanged pass-through.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/input/unsupported-srp.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/tbt-output/cli-output.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/ba3-pre-output/unsupported-srp.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/ba3-post-output/cli-output.txt
- **depends on**: []
- **commit**: 871c8d4
- **new**: no

The fixture is deliberately `srp`: the local Stanza 1.12 installation can tag it, so pre-edit BA3 reported success and generated `%mor`/`%gra`; the fork intentionally admits only its known-complete static language set and fails the file. Post-edit BA3 now matches that deterministic gate. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors` and the Bazel-backed `just batchalign cli` fixture run.

### Use float32 for CHATWhisper on Apple MPS
- **component**: Python CHATWhisper ASR backend
- **summary**: Selects `torch.float32` before constructing a CHATWhisper pipeline on MPS, avoiding a late `bfloat16` attention crash while preserving the existing `bfloat16` to `float16` fallback on other devices.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/input/device.json
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/tbt-output/dtype.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/ba3-pre-output/dtype.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/ba3-post-output/dtype.txt
- **depends on**: []
- **commit**: c425fe8
- **new**: no

The example isolates the loader policy so it is deterministic on machines without Apple GPU access and does not require downloading a model. Targeted verification: `bazel test //python/batchalign:pytest --test_arg=-q --test_arg=python/batchalign/tests/test_chatwhisper_device.py --test_output=errors`.

### Bound the pipeline dispatch-future window
- **component**: `batchalign-engine` pipeline scheduler
- **summary**: Feeds inputs through an eight-item ordered/unordered stream window instead of eagerly polling or retaining a future for every file; the existing semaphore remains the execution limit and result ordering remains stable.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/input/manifest.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/tbt-output/dispatch.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/ba3-pre-output/dispatch.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/ba3-post-output/dispatch.txt
- **depends on**: []
- **commit**: d7c5d49
- **new**: no

The regression blocks 100 synthetic per-input futures immediately after their first poll. Pre-edit scheduling admitted all 100 even though only eight could execute; post-edit scheduling admits exactly eight, then slides forward as slots complete, and still returns `0..99` in input order. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-engine:batchalign_engine_unit_test --test_output=errors`.

### Expand compound fillers for FA and restore one source span
- **component**: `batchalign-core` forced-alignment task runner
- **summary**: Sends underscore-separated compound filler parts to FA as independently recognizable words, then consumes and merges their returned timings into one `%wor` word spanning the first recognized onset through the last recognized offset.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/input/compound-filler.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/tbt-output/fa-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/ba3-pre-output/fa-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/ba3-post-output/fa-seam.txt
- **depends on**: []
- **commit**: 503525d
- **new**: no

The fixture exercises `&-you_know`: FA now receives `you`, `know`, `today`, while typed injection restores the source word domain as `you_know` at `100_350` and `today` at `400_600`. The same cursor policy applies to original words inside `ReplacedWord`. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors`.
