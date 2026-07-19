# implementation plan

## todo
- [ ] Runs typed pre- and post-stage gates and treats pre-serialization failure as a hard error.
- [ ] Fixes single-word replacement retraces, `@Media` hashing, parser recovery spans, and Japanese token merging.
- [ ] Fixed BA3 output passes `chatter validate`, making the validator the final output-integrity gate.
- [x] Expands compound fillers and recovers their audio spans between recognized words.
- [ ] Detects long-file drift and monotonicity violations, then re-anchors or repairs timing. | FA recovery and repair passes
- [ ] Integrates Cantonese FA through Jyutping and wav2vec2 (no need for the common recovery layer).
- [x] Fails unsupported primary languages per file instead of silently skipping them.
- [x] Synthesizes non-analyzable special forms such as `@q` and `@n` from typed `form_type` data.
- [ ] Rolls back invalid L2 splices, skips `NoAlign` words, and provides a fallback harness where secondary Stanza coverage is absent.
- [x] Detects bogus Stanza lemmas, missing fields, and invalid analyses, preserving the surface form and recording anomalies.
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

### Lock typed special-form synthesis when Stanza returns no sentence
- **component**: `talkbank-transform` morphosyntax injection
- **summary**: Pins the existing typed synthesis path end to end: an empty Stanza response still maps `@q` to `meta` and `@n` to `neo`, preserves the surface lemma, and emits validator-safe ROOT/PUNCT relations.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/input/special-forms.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/tbt-output/special-forms.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/ba3-pre-output/special-forms.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/ba3-post-output/special-forms.cha
- **depends on**: []
- **commit**: 086894b
- **new**: no

Audit note: runtime parity was already present before this item, so pre- and post-edit semantic output is intentionally identical. The independent commit adds the missing injection-level regression by collecting typed `FormType` payloads, supplying an empty `UdResponse`, and validating the resulting aligned CHAT. Targeted verification: `bazel test --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test --test_output=errors`.

### Repair invalid Stanza fields without losing lexical words
- **component**: Python Stanza morphology renderer and backend diagnostics
- **summary**: Normalizes missing or invalid lemma, UPOS, ID, head, and dependency-relation fields before rendering; lexical surfaces fall back to `x|surface`, dependency repairs satisfy the head/ROOT invariant, and every repair is logged with file and utterance identity.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/input/raw-stanza.json
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/tbt-output/renderer-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/ba3-pre-output/renderer-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/ba3-post-output/renderer-seam.txt
- **depends on**: []
- **commit**: f2f60d0
- **new**: no

The seam deliberately combines a lexical `hello` with lemma `.`, null UPOS, out-of-range head `99`, and `<pad>` relation. Pre-edit BA3 silently classified it as punctuation and emitted no analysis. Post-edit BA3 preserves it as `x|hello`, repairs a validator-safe ROOT tree, and records four structured warnings. The fork's focused raw-Stanza regression proves its surface-lemma fallback; BA3 additionally repairs the invalid dependency fields instead of carrying them toward a later mapping failure. Targeted verification: `bazel test //python/batchalign:pytest --test_arg=-q --test_arg=python/batchalign/tests/test_morphotag_render.py --test_output=errors`; fork verification: `RUSTUP_TOOLCHAIN=1.95.0 cargo test -p batchalign-transform parse_raw_stanza_bogus_lemma`.
