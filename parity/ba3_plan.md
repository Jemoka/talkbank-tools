# implementation plan

## todo
- [x] Runs typed pre- and post-stage gates and treats pre-serialization failure as a hard error.
- [x] Fixes single-word replacement retraces, `@Media` hashing, parser recovery spans, and Japanese token merging.
- [x] Fixed BA3 output passes `chatter validate`, making the validator the final output-integrity gate.
- [x] Expands compound fillers and recovers their audio spans between recognized words.
- [x] Detects long-file drift and monotonicity violations, then re-anchors or repairs timing. | FA recovery and repair passes
- [x] Integrates Cantonese FA through Jyutping and wav2vec2 (no need for the common recovery layer).
- [x] Fails unsupported primary languages per file instead of silently skipping them.
- [x] Synthesizes non-analyzable special forms such as `@q` and `@n` from typed `form_type` data.
- [x] Rolls back invalid L2 splices, skips `NoAlign` words, and provides a fallback harness where secondary Stanza coverage is absent.
- [x] Detects bogus Stanza lemmas, missing fields, and invalid analyses, preserving the surface form and recording anomalies.
- [x] Applies numbered, retireable Stanza workarounds for Italian compound imperatives and related defects.
- [x] Corrects known English transcribe patterns, Chinese bogus lemmas, and Japanese token merging.
- [x] Preflights large Rev.AI batches up front instead of submitting every file independently. (Please do this without moving revai to rust; keep it in Python.)
- [x] Avoids Whisper MPS `bfloat16` crashes on Apple Silicon.
- [x] Coordinates transcribe memory, splits work safely, and prevents OOM-created zombie workers.
- [x] Supports Qwen3-ASR plus Qwen3-ForcedAligner and fixes HK_QWEN backend/type-stub integration.
- [x] Propagates utterance metadata to children and keeps `ReplacedWord` atomic across splits.
- [x] Uses a sliding dispatch window so huge ASR input lists do not become huge in-flight sets.

## additional differences
- [x] 1/10 Sanitizes CHAT-illegal ASR tokens without failing the transcript.
- [x] 2/10 Suppresses terminator-only translation tiers.
- [x] 3/10 Excludes paired CA segment repetitions from lexical text.
- [x] 4/10 Keeps retraces with their retry across utterance splits.
- [x] 5/10 Projects comparison candidates to one majority utterance.
- [x] 6/10 Attributes matched comparison POS from the gold transcript.
- [x] 7/10 Keeps experimental review tiers off by default.
- [x] 8/10 Skips AppleDouble CHAT sidecars during every discovery pass.
- [x] 9/10 Removes wrapped `%mor`/`%gra` tiers without orphan continuations.

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

### Apply numbered, retireable Italian Stanza workarounds
- **component**: Python Stanza morphology renderer and Italian compatibility registry
- **summary**: Expands the fork's closed Defects 8, 12, and 13 allowlist into synthetic verb-plus-clitic MWTs, reindexes following dependency heads, records the numbered repair, and leaves every unknown surface unchanged.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/input/raw-stanza.json
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/tbt-output/renderer-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/ba3-pre-output/renderer-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/ba3-post-output/renderer-seam.txt
- **depends on**: []
- **commit**: 21c30bb
- **new**: no

The regression reproduces the confirmed mid-sentence `dammela` shape (`ADJ`, lemma `dammelo`) and pins the full three-chunk output. Separate cases exercise missing-MWT `aprilo` and fabricated-lemma `leggila`; the registry test requires explicit defect numbers and retirement criteria. Targeted verification uses the Bazel-built test executable without the rule's fixed whole-suite argument: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_morphotag_render.py -q` (11 passed). Fork verification: `RUSTUP_TOOLCHAIN=1.95.0 cargo test -p batchalign test_italian_defect8_dammela` (2 passed).

### Enforce typed gates at every pipeline and serialization boundary
- **component**: `batchalign-engine` pipeline boundary, `batchalign-core` CHAT writer, and typed validation API
- **summary**: Validates every CHAT carried directly, in pairs, or in artifact lists before and after each task; stage violations become per-file failures, and the exact serialized bytes must reparse and pass full validation before any disk write.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/input/stage-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/tbt-output/gate.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/ba3-pre-output/invalid-output.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/ba3-post-output/gate.txt
- **depends on**: []
- **commit**: 6d1aaae
- **new**: no

The seam deliberately mutates an already-validated typed AST after admission by removing its main-tier terminator. Pre-edit BA3 could serialize the invalid transcript; post-edit, the shared pre/post boundary rejects it and `Chat::write` independently reparses the exact output bytes as a final hard gate. The validation helpers are now generic over both CHAT typestates, avoiding a string round-trip inside stage checks. Targeted verification: `bazel test --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test //crates/batchalign/batchalign-core:batchalign_core_unit_test //crates/batchalign/batchalign-engine:batchalign_engine_unit_test --test_output=errors`; Bazel product build: `just batchalign build debug`.

### Lock Python-owned Rev.AI batch preflight ordering
- **component**: Python Rev.AI ASR/speaker backend
- **summary**: Pins the existing submit-then-poll path on a 32-file batch: every unique provider job is submitted before polling begins, and duplicate atomic ASR/speaker projections reuse one submission.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/input/manifest.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/tbt-output/order.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/ba3-pre-output/order.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/ba3-post-output/order.txt
- **depends on**: []
- **commit**: c39b3d5
- **new**: no

Audit note: the Python backend already had the intended batched runtime behavior before this item was audited; the independent commit supplies the missing large-batch regression and explicitly prevents a later return to per-file submit-and-wait. Rev.AI remains Python-owned. The hermetic fake makes no provider calls and asserts 32 submit events precede the single poll event. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_revai_preflight.py -q` (1 passed).

### Lock utterance metadata and atomic replacement splitting together
- **component**: typed `talkbank-transform` utterance segmentation
- **summary**: Pins the child-propagation policy for linkers, language code, postcodes, timing bullets, and source spans while a classifier boundary falls inside a multiword replacement; the complete `ReplacedWord` follows its first assignment.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/input/split.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/tbt-output/split-contract.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/ba3-pre-output/split-contract.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/ba3-post-output/split-contract.txt
- **depends on**: []
- **commit**: 5ebd4cc
- **new**: no

Audit note: the individual propagation and atomicity rules were already present; the new independent regression exercises their interaction in one typed split. Its five-word morphology domain assigns `I, want` to child 0 and `to, go, now` to child 1, forcing the boundary inside `wanna [: want to]`. The replacement remains wholly on child 0; utterance-scope language and spans reach both children, the prior-turn linker stays first, and end-scope postcode/bullet stay last. Targeted verification: `bazel test --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test --test_output=errors`.

### Repair long-file FA monotonicity before validation
- **component**: `batchalign-core` forced-alignment task runner
- **summary**: Runs a typed timing repair immediately after FA injection: backward utterance anchors lose both their unsafe main bullet and stale `%wor`, while forward anchors whose end crosses the next start are clamped to that start.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/input/timing-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/tbt-output/repair.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/ba3-pre-output/repair.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/ba3-post-output/repair.txt
- **depends on**: []
- **commit**: 5b412fa
- **new**: no

The synthetic 500-utterance regression includes one ordinary end overlap and one severe backward anchor at utterance 400. The post-pass reports one clamp and one strip, removes `%wor` together with the untrustworthy drifted main bullet, and reparses the complete 500-utterance serialization through the full CHAT validator. This follows the fork's conservative rule: preserve forward-moving conversational overlap starts, but never invent a location for a backward anchor. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors`.

### Romanize Cantonese only at the wav2vec2 alignment seam
- **component**: Python `Wav2Vec2FaBackend`
- **summary**: Converts resolved `yue` source words to tone-free, apostrophe-delimited Jyutping before MMS_FA while preserving the original Hanzi surfaces in returned CHAT; non-Cantonese and unresolved-language inputs retain the shared path unchanged.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/input/alignment-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/tbt-output/alignment-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/ba3-pre-output/alignment-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/ba3-post-output/alignment-seam.txt
- **depends on**: []
- **commit**: 2ad0e3a
- **new**: no

The conversion is local to the direct MMS input seam, so this language-specific route does not depend on the common recovery layer and does not alter typed output words. If the optional Cantonese dependency is absent, the backend reports an actionable extra requirement instead of silently sending Han characters to MMS. Targeted verification uses the Bazel-built test executable: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_cantonese_fa.py -q` (3 passed).

### Prove serialized Batchalign output through the Chatter validator
- **component**: `batchalign-core` final CHAT serialization gate
- **summary**: Locks the final output contract with a representative annotated document containing media, utterance and word bullets, `%mor`, and `%gra`; the exact serialized bytes re-enter the same full validation pipeline used by `chatter validate` before disk I/O.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/input/output-contract.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/tbt-output/annotated.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/ba3-pre-output/annotated.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/ba3-post-output/annotated.cha
- **depends on**: [6d1aaae]
- **commit**: 21fb255
- **new**: no

The implementation dependency made full reparsing a hard pre-write gate; this independent regression pins a realistic successful output instead of testing only rejection. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_filter=serialized_batchalign_output_passes_chatter_validation_pipeline --test_output=errors` (1 passed). The fixture also passed the actual Bazel-built CLI with `bazel-bin/crates/chatter/chatter-cli/chatter validate --force --tui-mode disable --quiet /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/ba3-post-output/annotated.cha`.

### Use the typed Qwen class for standalone forced alignment
- **component**: Python Qwen3 ASR/FA backends and public backend surface
- **summary**: Keeps Qwen3-ASR paired with its companion forced aligner for word timestamps, but constructs standalone FA directly as one `Qwen3ForcedAligner`; both backend types remain reachable through the lazy typed package surface.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/input/backend-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/tbt-output/backend-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/ba3-pre-output/backend-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/ba3-post-output/backend-seam.txt
- **depends on**: []
- **commit**: a0b8601
- **new**: no

Pre-edit standalone FA constructed `Qwen3ASRModel` from the alignment checkpoint and also passed that checkpoint as its nested `forced_aligner`, loading the same large model twice through the wrong outer class. The locked `qwen-asr` API exposes `Qwen3ForcedAligner.from_pretrained` directly; post-edit uses that supported surface with its correct `dtype` parameter. The regression also pins the ASR companion-aligner argument and the two public re-exports, covering the fork's HK_QWEN/type-stub intent in BA3's backend architecture. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_qwen3_contract.py -q` (3 passed).

### Honor NoAlign before FA work and retain safe L2 fallbacks
- **component**: `batchalign-core` FA task runner and typed L2 morphosyntax splice
- **summary**: Treats `@Options: NoAlign` as strict byte-stable pass-through before media lookup or backend dispatch; audited L2 splice transactions restore snapshots on invalid dependency output, while missing secondary Stanza coverage yields empty responses and `L2|xxx` fallback instead of aborting the batch.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/input/no-align.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/tbt-output/no-align.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/ba3-pre-output/no-align.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/ba3-post-output/no-align.cha
- **depends on**: []
- **commit**: b390560
- **new**: no

The missing runtime gap was the FA entry point: pre-edit a NoAlign transcript without media failed sibling lookup, while post-edit it reaches neither media nor a panic-on-call dispatcher and serializes identically. The existing typed L2 transaction was separately verified against an invalid secondary head into the host terminator (rollback/fallback), and the Python harness was verified across a missing secondary Stanza pipeline (five cache/fallback cases). Targeted verification used Bazel-built executables: `bazel-bin/crates/batchalign/batchalign-core/batchalign_core_unit_test --exact taskrunners::fa::tests::no_align_is_strict_pass_through_without_media_or_dispatch` (1 passed); `bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact morphosyntax::l2::splice::cardinality_tests::family_c_secondary_head_into_host_terminator_falls_back` (1 passed); `bazel-bin/python/batchalign/pytest python/batchalign/tests/test_stanza_pipeline_cache.py -q` (5 passed).

### Bound decoded-audio admission at each backend route
- **component**: `batchalign-engine` backend dispatcher and transcribe chunking contracts
- **summary**: Replaces the unbounded per-backend channel with an admission-budget-sized queue whose async send applies backpressure; one backend loop continues to own model calls, long BERT inputs use bounded overlapping splits, and route shutdown wakes pending producers rather than leaving model-worker processes behind.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/input/dispatch-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/tbt-output/dispatch-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/ba3-pre-output/dispatch-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/ba3-post-output/dispatch-seam.txt
- **depends on**: [d7c5d49]
- **commit**: 04ea516
- **new**: no

A dispatch item may own an entire decoded PCM file, so an unbounded route could defeat the earlier sliding future window when callers dispatch directly or future admission widths grow. Post-edit queue capacity comes from `EngineConfig.max_concurrent_values` (with a safe minimum of one), and closing the route causes blocked `send().await` producers to fail cleanly. BA3 runs Python model objects in-process behind one Rust batcher loop rather than spawning one process per input, so there is no OOM-created child-worker population to orphan. Targeted verification: `bazel build --config=dev //crates/batchalign/batchalign-engine:batchalign_engine_unit_test && bazel-bin/crates/batchalign/batchalign-engine/batchalign_engine_unit_test --exact engine::tests::backend_route_capacity_matches_memory_admission_budget` (1 passed); `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_utseg_sliding_window.py -q` (6 passed).

### Exercise replacement retraces inside the Bazel parser target
- **component**: `talkbank-parser` replacement/retrace lowering
- **summary**: Locks the existing parser correction into the actual Bazel unit-test graph: a word carrying both `[: replacement]` and `[//]` lowers to a full `Retrace` containing a `ReplacedWord`, never a bare replacement that pollutes morphology alignment.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/input/retrace.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/tbt-output/shape.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/ba3-pre-output/shape.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/ba3-post-output/shape.txt
- **depends on**: [c90b9bff]
- **commit**: 0b010c0
- **new**: no

Audit note: runtime parity was already supplied by the historical focused fix, and a six-case regression existed under `crates/utils/tests`; however, that integration-test directory is explicitly deferred in `crates/utils/BUILD.bazel` and therefore was invisible to the required Bazel workflow. The new parser-local contract makes the critical AST shape part of `//crates/core/talkbank-parser:talkbank_parser_unit_test`. Targeted verification: `bazel build --config=dev //crates/core/talkbank-parser:talkbank_parser_unit_test && bazel-bin/crates/core/talkbank-parser/talkbank_parser_unit_test --exact parser::tree_parsing::main_tier::content::word::tests::replacement_with_retrace_marker_stays_a_retrace` (1 passed).

### Retain source locations on parser recovery tiers
- **component**: `talkbank-parser` malformed dependent-tier recovery
- **summary**: Carries the CST byte range into recovered empty `%mor` and `%gra` placeholders, including the synthetic morphology terminator, so later validation and regeneration diagnostics point at the malformed source tier instead of byte zero.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/input/recovery-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/tbt-output/spans.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/ba3-pre-output/spans.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/ba3-post-output/spans.txt
- **depends on**: []
- **commit**: 4578546
- **new**: no

Pre-edit recovery intentionally retained the dependent-tier slot but constructed its tier and morphology terminator with `Span::DUMMY`. Post-edit uses one exact `tree_sitter::Node` range for the recovered typed objects; the fallback remains empty and ordering-preserving, but it no longer fabricates a source location. Targeted verification: `bazel build --config=dev //crates/core/talkbank-parser:talkbank_parser_unit_test && bazel-bin/crates/core/talkbank-parser/talkbank_parser_unit_test --exact parser::chat_file_parser::dependent_tier_dispatch::parsed::tests::recovered_tier_placeholders_keep_source_spans` (1 passed).

### Lock Japanese Stanza splits back to one CHAT word
- **component**: `talkbank-transform` tokenizer realignment
- **summary**: Pins the character-alignment merge for Japanese: Stanza stem/auxiliary splits are recombined to the original CHAT surface and emitted with `expand_mwt=false`, preserving one morphology slot instead of allowing a second expansion.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/input/token-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/tbt-output/tokens.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/ba3-pre-output/tokens.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/ba3-post-output/tokens.txt
- **depends on**: []
- **commit**: a97cdb4
- **new**: no

Audit note: BA3 and the fork already share the same realignment implementation, but neither the generic compound case nor the English contraction case proved Japanese behavior. The focused contract uses `食べちゃう` split as `食べ + ちゃう`, checks exact surface restoration, and prevents accidental English-style MWT expansion. This one seam satisfies the Japanese-merging clause shared by both remaining original checklist lines. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact tokenizer_realign::tests::test_japanese_split_tokens_merge_back_to_chat_word` (1 passed).

### Audit known English transcribe corrections end to end
- **component**: `talkbank-transform` ASR postprocessing and CHAT construction
- **summary**: Verifies the three ordered English hooks in one output contract: pronoun-I and I-contraction capitalization, allowlisted title-period stripping before retokenization, and first lexical-word capitalization after utterance boundaries are known.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/input/raw-asr.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/tbt-output/transcript.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/ba3-pre-output/transcript.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/ba3-post-output/transcript.txt
- **depends on**: []
- **commit**: e8235c13
- **new**: no

Audit note: the fork-origin implementation and its end-to-end test were already present before this parity pass. The test starts from timed raw ASR elements, crosses `process_raw_asr`, retokenization, transcript description, typed CHAT construction, and serialization, and checks all three corrected utterances plus negative stale-form assertions. The separate evidence commit records the verified Bazel contract without perturbing working runtime code. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact build_chat::tests::english_transcribe_rules_fire_end_to_end` (1 passed).

### Lock bogus Chinese lemmas to their Han surface
- **component**: Python structured Stanza morphology renderer
- **summary**: Proves a Chinese lexical word whose Stanza lemma is punctuation retains its Han surface as the lemma, remains a noun with a valid ROOT relation, and records a structured lemma anomaly rather than disappearing as punctuation.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/input/raw-stanza.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/tbt-output/analysis.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/ba3-pre-output/analysis.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/ba3-post-output/analysis.txt
- **depends on**: [f2f60d0]
- **commit**: bf3035a
- **new**: no

Audit note: the repair implementation was introduced by the earlier general invalid-Stanza fix, but only Latin lexical surfaces were pinned. This independent Chinese regression supplies `苹果` with the observed bogus lemma `。`, asserts exact `%mor` surface preservation and ROOT repair, and checks the anomaly retains the Han source text. Together with the English end-to-end evidence and Japanese realignment contract above, this closes the final three-language original TODO. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_morphotag_render.py -q -k chinese_bogus_punctuation_lemma` (1 passed, 11 deselected).

### Hash the complete typed `@Media` identity
- **component**: `talkbank-model` typed media header
- **summary**: Adds `Eq` and `Hash` to `MediaHeader` so typed headers can safely key deduplication and memoization; the derived identity includes filename, capture modality, and optional linkage status.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/input/media-identities.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/tbt-output/hash-set.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/ba3-pre-output/hash-set.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/ba3-post-output/hash-set.txt
- **depends on**: []
- **commit**: dd92e0b
- **new**: no

Pre-edit every constituent media field was already hashable, but the composite header omitted the contract and could not be used directly in a `HashSet`/`HashMap`. The regression inserts an exact duplicate plus three single-field variations and requires four unique identities, proving no field is accidentally ignored. This closes the last clause of the first bundled original TODO; its retrace, recovery-span, and Japanese clauses are independently evidenced above. Targeted verification: `bazel build --config=dev //crates/core/talkbank-model:talkbank_model_unit_test && bazel-bin/crates/core/talkbank-model/talkbank_model_unit_test --exact model::header::media::tests::media_hash_includes_filename_type_and_status` (1 passed).

### Sanitize CHAT-illegal ASR tokens at the final word seam
- **component**: `talkbank-transform` ASR postprocessing
- **summary**: Uses the typed CHAT word parser as an oracle after number expansion, stripping illegal characters while retaining valid residue and timing; entirely structural tokens are dropped, and empty utterances created by sanitization are removed.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/input/raw-asr.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/tbt-output/transcript.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/ba3-pre-output/transcript.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/ba3-post-output/transcript.txt
- **depends on**: []
- **commit**: 3ccc897
- **new**: yes

Whisper and Tencent can emit bare CHAT separators such as `:` and `~`, or glue invalid Unicode to otherwise useful text. Pre-edit the downstream typed transcript gate rejected the whole utterance. Post-edit sanitization runs only after currency/percent expansion, preserves every already-valid word byte-for-byte, keeps timestamps on repaired residue, and prevents an all-dropped utterance from becoming an invalid empty main tier. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact build_chat::tests::chat_illegal_asr_separator_does_not_fail_transcription` (1 passed).

### Suppress terminator-only translation noise
- **component**: `talkbank-transform` translation injection
- **summary**: Treats whitespace-only and bare `.`, `!`, or `?` backend responses as no translation, leaving the utterance unchanged instead of emitting a meaningless `%xtra` tier.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/input/translation.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/ba3-post-output/tiers.txt
- **depends on**: []
- **commit**: be2b98b
- **new**: yes

Translator APIs can answer with only the source terminator when a turn has no lexical material. Pre-edit BA3 accepted nonempty `.` and serialized `%xtra:\t.`; BA2 and the fork omit it. The trim-first guard handles all three CHAT terminators and whitespace variants without changing genuine translations or replacement of an existing tier. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact translate::tests::test_inject_bare_terminator_translation_is_noop` (1 passed).

### Exclude paired CA segment repetitions from lexical text
- **component**: `talkbank-model` cleaned words and `talkbank-parser-re2c` conversion
- **summary**: Treats text bracketed by paired `↫` markers as a repeated non-lexical fragment while preserving text inside every other CA delimiter, keeping both parser implementations consistent.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/input/stutter.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/tbt-output/lexical.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/ba3-pre-output/lexical.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/ba3-post-output/lexical.txt
- **depends on**: []
- **commit**: dec565c
- **new**: yes

In CHAT, `↫sch↫schaap` records a repeated onset followed by the lexical Dutch word `schaap`. Pre-edit cleaned text concatenated every text node and sent `schschaap` to morphology, alignment, comparison, and frequency consumers. Post-edit both CST conversion paths toggle a segment-repetition state and omit only bracketed material; the regression also proves ordinary CA content such as `∆snel∆` remains lexical. Targeted verification: `bazel build --config=dev //crates/core/talkbank-model:talkbank_model_unit_test //crates/core/talkbank-parser-re2c:talkbank_parser_re2c_unit_test && bazel-bin/crates/core/talkbank-model/talkbank_model_unit_test --exact model::content::word::word_type::cleaned_text_tests::segment_repetition_is_excluded_but_other_ca_content_is_lexical` (1 passed).

### Keep retraces with their following retry during UtSeg
- **component**: `talkbank-transform` utterance splitting
- **summary**: Assigns uncounted `Retrace` content forward to the next word-bearing child before generic marker back-fill, preventing `[/]`, `[//]`, or `[///]` from being stranded at the end of the preceding child.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/input/retrace.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/tbt-output/split.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/ba3-pre-output/split.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/ba3-post-output/split.txt
- **depends on**: [5ebd4cc]
- **commit**: a309006
- **new**: yes

Retraced words are deliberately absent from the morphology word domain, so their top-level `Retrace` nodes receive no classifier assignment. Pre-edit generic back-fill attached those nodes to the preceding assigned word, creating a dangling retrace when the classifier boundary fell before the kept retry. Post-edit retraces first inherit the next assigned content group; genuinely utterance-final retraces still use the existing fallback. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact utseg::tests::utseg_split_keeps_retrace_with_following_retry` (1 passed).

### Project comparison candidates to one utterance
- **component**: `talkbank-transform` transcript comparison
- **summary**: Projects every candidate main-token window to its majority utterance before overlap and alignment scoring, preventing a single gold utterance from collecting matches across two main utterances.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/tbt-output/metrics.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/ba3-pre-output/metrics.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/ba3-post-output/metrics.txt
- **depends on**: []
- **commit**: 6ac567d
- **new**: yes

Pre-edit bag-of-words scoring selected `the sky this dog ran` and counted `the` from one main utterance together with `dog ran` from the next, inflating the match count from two to three. Post-edit preserves Python `Counter.most_common(1)` first-seen tie behavior, trims candidate edges to the majority utterance, and scores only the projected range. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact compare::tests::find_best_segment_does_not_score_across_utterance_boundaries && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact compare::tests::compare_does_not_steal_match_across_utterance_boundary` (2 passed).

### Attribute matched POS from the gold transcript
- **component**: `talkbank-transform` comparison annotation and metrics
- **summary**: Uses the gold word's morphology tag for every matched token, so `%xsmor` and per-POS match counts describe the reference annotation rather than silently copying a conflicting main-transcript tag.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/tbt-output/xsmor.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/ba3-pre-output/xsmor.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/ba3-post-output/xsmor.txt
- **depends on**: []
- **commit**: 0064d0d
- **new**: yes

Pre-edit identical surfaces with disagreeing morphology produced `NOUN ADJ`, masking the gold annotation `INTJ NOUN` and charging matches to the wrong POS buckets. Post-edit matched and gold-only tokens both derive POS from gold, while main-only insertions continue to retain main POS; gold files without `%mor` correctly produce `?` instead of borrowing a tag. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test compare::tests::` (32 passed).

### Keep experimental review tiers off by default
- **component**: `talkbank-transform` decision reporting and `batchalign-core` UTR
- **summary**: Defaults decision-tier emission to `None`, preserving structured UTR decisions internally without adding `%xalign` or `%xrev` cleanup noise to ordinary CHAT output.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/input/decision.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/tbt-output/decision.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/ba3-pre-output/decision.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/ba3-post-output/decision.cha
- **depends on**: []
- **commit**: f0aa660
- **new**: yes

Pre-edit UTR hardcoded `ReviewLevel::All`, so any unmatched or zero-duration decision leaked experimental dependent tiers into finished research files. Post-edit `ReviewLevel::default()` is `None` and UTR uses that default; callers that explicitly pass `LowConfidence` or `All` to the retained injection API still receive review tiers. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test //crates/batchalign/batchalign-core:batchalign_core_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact decisions::tests::default_review_level_produces_no_output_tiers` (1 passed).

### Skip AppleDouble CHAT sidecars during discovery
- **component**: Python CLI CHAT discovery and align language preflight
- **summary**: Routes align's language inference through the shared hidden-file filter and rejects explicitly supplied dotfiles, so macOS `._*.cha` metadata cannot be parsed as transcript text.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/input/corpus.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/tbt-output/discovered.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/ba3-pre-output/discovered.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/ba3-post-output/discovered.txt
- **depends on**: []
- **commit**: 565c0bc
- **new**: yes

The normal input collector already ignored directory dotfiles, but align independently scanned `*.cha` to choose its UTR language. Since `._session.cha` sorts before `session.cha`, binary metadata raised `UnicodeDecodeError` before any real input ran. Post-edit both preflight and collection share `_walk`, and a sidecar passed as the explicit path yields a clear no-language error rather than entering the pipeline. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_chat_discovery.py -q` (2 passed).

### Remove wrapped analysis tiers completely before morphotagging
- **component**: Python morphotag `--clear-existing` staging
- **summary**: Removes every tab-led continuation belonging to an existing `%mor` or `%gra` tier while preserving following main tiers, headers, and unrelated dependent tiers.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/input/wrapped.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/tbt-output/cleared.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/ba3-pre-output/cleared.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/ba3-post-output/cleared.cha
- **depends on**: []
- **commit**: 48655f8
- **new**: yes

The default refresh path stages a copy without old analysis so the engine cannot skip already-tagged utterances. Pre-edit it removed only `%mor:` and `%gra:` header lines, leaving wrapped payload lines as orphan CHAT continuations; the staged file could then fail parsing before regeneration. Post-edit a small state machine consumes the complete logical analysis tiers and stops at the next non-continuation line. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_morphotag_clear.py -q` (3 passed).
