# Python/Rust Interface Map

**Status:** Current
**Last updated:** 2026-04-29 08:30 EDT

This document is the unified reference for all Python/Rust interface boundaries in batchalign3.

## Interface Boundaries

### 1. Worker Protocol Dispatch (`pyo3-protocol`)
**Purpose:** IPC message routing and envelope validation

| Aspect | Location |
|--------|----------|
| Rust source of truth | `crates/batchalign-pyo3/src/worker_protocol.rs` |
| Python implementation | `python/batchalign/worker/_protocol.py` + `python/batchalign/worker/_handlers.py` |
| Shared schema | `schemas/ipc/worker_v2/` (JSON Schema) |
| IPC types | `crates/batchalign-types/src/worker_v2/mod.rs` |
| Python types | `python/batchalign/worker/_types_v2.py` (hand-written + validated) |
| Generated Python | `python/batchalign/generated/worker_v2/` |
| Tests | `crates/batchalign/tests/worker_protocol_v2_compat.rs`, `batchalign/tests/test_ipc_type_conformance.py` |

**Cross-references:**
- Rust: `crates/batchalign-pyo3/src/worker_protocol.rs` line 1 onward references `schemas/ipc/worker_v2/`
- Python: `python/batchalign/worker/_protocol.py` implements protocol handlers
- See architecture docs: `book/src/batchalign/developer/worker-protocol-v2.md`

**Responsibility:**
- **Rust controls:** Message validation, routing, envelope structure, error codes
- **Python implements:** Handler dispatch, message logging, resource cleanup

---

### 2. ASR Execution V2 (`pyo3-asr`)
**Purpose:** Audio-to-text transcription via Whisper or HK providers

| Aspect | Location |
|--------|----------|
| Rust FFI | `crates/batchalign-pyo3/src/worker_asr_exec.rs::execute_asr_request_v2()` |
| Rust request type | `crates/batchalign-types/src/worker_v2/requests.rs::AsrRequestV2` |
| Rust response type | `crates/batchalign-types/src/worker_v2/responses.rs::ExecuteResponseV2` |
| Python executor | `python/batchalign/worker/_asr_v2.py::execute_asr_request_v2()` |
| Python host | `python/batchalign/worker/_asr_v2.py::AsrExecutionHostV2` |
| Whisper bridge | `python/batchalign/inference/asr.py` (local model) |
| HK providers | `python/batchalign/inference/languages/cantonese/` (Tencent, FunASR, Aliyun) |
| Schema | `schemas/ipc/worker_v2/AsrRequestV2.json`, `schemas/ipc/worker_v2/ExecuteResponseV2.json` |

**Rust/Python contract:**
- Input: `AsrRequestV2` with `prepared_audio` reference + metadata
- Output: `ExecuteResponseV2` with ASR results or error
- Side-effects: None (stateless)

**Cross-references:**
- Python calls Rust-exposed `batchalign_core.execute_asr_request_v2()`
- Rust prepares audio bytes; Python loads from attachment refs
- See: `book/src/batchalign/architecture/asr-token-pipeline.md`

**Responsibility:**
- **Rust owns:** Audio preparation, artifact management, result type structure
- **Python owns:** Model loading, inference execution, error reporting to Rust

---

### 3. Forced Alignment V2 (`pyo3-fa`)
**Purpose:** Word-level timing alignment via Whisper or Wave2Vec

| Aspect | Location |
|--------|----------|
| Rust FFI | `crates/batchalign-pyo3/src/worker_fa_exec.rs::execute_forced_alignment_request_v2()` |
| Rust request type | `crates/batchalign-types/src/worker_v2/requests.rs::ForcedAlignmentRequestV2` |
| Python executor | `python/batchalign/worker/_fa_v2.py::execute_forced_alignment_request_v2()` |
| Schema | `schemas/ipc/worker_v2/ForcedAlignmentRequestV2.json` |

**Rust/Python contract:**
- Input: `ForcedAlignmentRequestV2` with prepared audio + normalized text
- Output: Word timing results or error
- Side-effects: None

**Cross-references:**
- See: `book/src/batchalign/architecture/alignment-structures.md`
- Rust server caches FA results; Python is stateless

**Responsibility:**
- **Rust owns:** Text normalization, cache eviction, timing post-processing
- **Python owns:** Alignment model execution

---

### 4. Media Analysis V2: OpenSMILE (`pyo3-media-opensmile`)
**Purpose:** Acoustic feature extraction

| Aspect | Location |
|--------|----------|
| Rust FFI | `crates/batchalign-pyo3/src/worker_media_exec.rs::execute_opensmile_request_v2()` |
| Python executor | `python/batchalign/worker/_opensmile_v2.py::execute_opensmile_request_v2()` |
| Schema | `schemas/ipc/worker_v2/OpenSmileRequestV2.json` |

**Rust/Python contract:**
- Input: `OpenSmileRequestV2` with prepared audio
- Output: Numeric feature vectors
- Side-effects: None

---

### 5. Media Analysis V2: AVQI (`pyo3-media-avqi`)
**Purpose:** Voice quality analysis (paired audio comparison)

| Aspect | Location |
|--------|----------|
| Rust FFI | `crates/batchalign-pyo3/src/worker_media_exec.rs::execute_avqi_request_v2()` |
| Python executor | `python/batchalign/worker/_avqi_v2.py::execute_avqi_request_v2()` |
| Schema | `schemas/ipc/worker_v2/AvqiRequestV2.json` |

**Rust/Python contract:**
- Input: `AvqiRequestV2` with paired audio references
- Output: Quality score + metrics
- Side-effects: None

---

### 6. Media Analysis V2: Speaker Diarization (`pyo3-media-speaker`)
**Purpose:** Speaker boundary and identity detection

| Aspect | Location |
|--------|----------|
| Rust FFI | `crates/batchalign-pyo3/src/worker_media_exec.rs::execute_speaker_request_v2()` |
| Python executor | `python/batchalign/worker/_speaker_v2.py::execute_speaker_request_v2()` |
| Schema | `schemas/ipc/worker_v2/SpeakerRequestV2.json` |

**Rust/Python contract:**
- Input: `SpeakerRequestV2` with prepared audio
- Output: Speaker segments with confidence scores
- Side-effects: None

---

### 7. Text Task Result Normalization (`pyo3-text`)
**Purpose:** Reshape BatchInferResponse (legacy V1) into V2 shapes for morphotag, utseg, translate, coref

| Aspect | Location |
|--------|----------|
| Rust FFI | `crates/batchalign-pyo3/src/worker_text_results.rs::normalize_text_task_result()` |
| Rust FFI | `crates/batchalign-pyo3/src/worker_text_results.rs::align_tokens()` |
| Python caller | `python/batchalign/worker/_execute_v2.py::execute_text_request_v2()` (legacy path) |
| Schema | Responds with V2 ExecuteResponseV2 |

**Purpose:** During cutover from V1 to V2, this normalizer bridges the old text-pipeline response types into the new V2 response envelope.

**Future:** Once all text tasks migrate to V2, this can be removed.

---

### 8. Worker Artifact Loading (`pyo3-artifacts`)
**Purpose:** Resolve and load Rust-prepared artifacts (audio bytes, JSON metadata)

| Aspect | Location |
|--------|----------|
| Rust FFI functions | `crates/batchalign-pyo3/src/worker_artifacts.rs` (6 functions) |
| Python caller | `python/batchalign/worker/_execute_v2.py` + `python/batchalign/worker/_asr_v2.py` etc. |

**Functions:**
- `find_worker_attachment_by_id()` — locate artifact by ID
- `load_worker_json_attachment()` — deserialize JSON artifact
- `load_worker_prepared_text_json()` — load prepared text payload
- `load_worker_prepared_audio_f32le_bytes()` — load audio bytes (F32LE codec)

**Design:**
- Artifacts are **file-backed** in a per-worker temp directory
- IPC message contains artifact references (`ArtifactRefV2`)
- Python loads refs → Rust returns bytes or JSON
- Future: Can migrate to shared memory without changing Python logic

---

### 9. HK/Cantonese ASR Bridges (`pyo3-hk`)
**Purpose:** Project provider-specific ASR output (FunASR, Tencent, Aliyun) into common shapes

| Aspect | Location |
|--------|----------|
| Rust FFI functions | `crates/batchalign-pyo3/src/cantonese_asr_bridge.rs` (6 functions) |
| Python callers | `python/batchalign/inference/languages/cantonese/`, `python/batchalign/worker/_asr_v2.py` |

**Functions:**
- `funaudio_segments_to_asr()` — project FunASR segment output
- `tencent_result_detail_to_asr()` — project Tencent API response
- `aliyun_sentences_to_asr()` — project Aliyun API response
- `normalize_cantonese()` — simplified ↔ traditional + domain replacements
- `cantonese_char_tokens()` — per-character tokenization for FA
- `clean_funaudio_segment_text()` — text cleanup before normalization

**Design:** These bridge the gap between provider-native formats and Batchalign `MonologueAsrResultV2`.

**Responsibility:**
- **Rust owns:** Output shape (`MonologueAsrResultV2`), char tokenization
- **Python owns:** Provider SDK invocation, raw response parsing

---

### 10. Worker V2 IPC Schema (`worker-v2-schema`)
**Purpose:** Single source of truth for all IPC message types

| Aspect | Location |
|--------|----------|
| Rust types | `crates/batchalign-types/src/worker_v2/` |
| Re-exported by | `crates/batchalign/src/types/worker_v2.rs` |
| JSON Schema (generated) | `schemas/ipc/worker_v2/` |
| Python generated types | `python/batchalign/generated/worker_v2/` |
| Python hand-written | `python/batchalign/worker/_types_v2.py` (with validators) |
| Conformance tests | `crates/batchalign/tests/worker_protocol_v2_compat.rs`, `batchalign/tests/test_ipc_type_conformance.py` |

**Sync process:**
1. Modify Rust struct + add `#[derive(schemars::JsonSchema)]`
2. Register in `crates/batchalign/src/ipc_schema.rs`
3. Run `cargo run -p batchalign-cli -- ipc-schema --output schemas/ipc/`
4. Run `bash scripts/generate_ipc_types.sh` to update Python
5. Conformance tests verify hand-written Python types match schema
6. CI gate: `bash scripts/check_ipc_type_drift.sh`

**See:** `book/src/batchalign/developer/ipc-type-sync.md`

---

### 11. Rust/Python Type Sync System (`types-sync`)
**Purpose:** Ensure Rust and Python types stay synchronized

| Aspect | Location |
|--------|----------|
| Rust types | `crates/batchalign-types/src/worker_v2/` |
| Schema generator | `crates/batchalign/src/ipc_schema.rs` |
| Generated Python | `python/batchalign/generated/worker_v2/` + `python/batchalign/generated/batch_items/` |
| Hand-written overlays | `python/batchalign/worker/_types_v2.py` (validators, aliases) |
| Drift tests | `crates/batchalign/tests/worker_protocol_v2_compat.rs` |
| Conformance tests | `batchalign/tests/test_ipc_type_conformance.py` |
| Scripts | `scripts/generate_ipc_types.sh`, `scripts/check_ipc_type_drift.sh` |

**Invariant:** All Rust struct → Python model mappings are validated at test time.

---

## Documentation Map

### Architecture (Decision Record)
- `book/src/batchalign/architecture/python-rust-interface.md` — Overview of PyO3 boundary, worker architecture, GIL strategy
- `book/src/batchalign/architecture/server-architecture.md` — Server-side orchestration, job lifecycle
- `book/src/batchalign/architecture/worker-architecture-assessment.md` — Worker pool, memory model

### Developer Reference (Implementation Spec)
- `book/src/batchalign/developer/worker-protocol-v2.md` — IPC protocol, envelope types, V1/V2 migration status
- `book/src/batchalign/developer/ipc-type-sync.md` — Type generation pipeline, conformance testing, adding new types
- `book/src/batchalign/developer/maturin-pyo3-surface.md` — PyO3 wheel packaging, editable installs

### Crate Documentation (Source Code)
- `crates/batchalign-pyo3/CLAUDE.md` — Pyo3 crate architecture, standards, rules
- `crates/batchalign/CLAUDE.md` — Server module map, job lifecycle, concurrency model
- `crates/batchalign-types/CLAUDE.md` — Domain types, newtype conventions (if exists)

### In-Code Documentation
- `crates/batchalign-pyo3/src/lib.rs` — Module registration, what each function does
- `crates/batchalign-pyo3/src/worker_protocol.rs` — IPC dispatch logic
- `crates/batchalign-pyo3/src/worker_*_exec.rs` — Per-task executors
- `python/batchalign/worker/_execute_v2.py` — Python dispatch logic
- `python/batchalign/worker/_*_v2.py` — Per-task executors

---

## When Adding a New Interface

1. **Define Rust types** with `#[derive(schemars::JsonSchema)]`
2. **Register in schema generator** (`crates/batchalign/src/ipc_schema.rs`)
3. **Generate schemas** (`cargo run -p batchalign-cli -- ipc-schema --output schemas/ipc/`)
4. **Generate Python** (`bash scripts/generate_ipc_types.sh`)
5. **Add conformance test** in `batchalign/tests/test_ipc_type_conformance.py`
6. **Document in this file** (boundary name, locations, responsibility split)
7. **Add doc comment to Rust FFI** pointing to this file
8. **Add doc comment to Python** pointing to Rust source and this file

---

## Checklist: Are Docs Synced?

For each boundary, verify:

- [ ] Rust source has doc comments explaining contract (input, output, side-effects)
- [ ] Python source has doc comments explaining contract
- [ ] Both reference each other (cross-links in comments)
- [ ] Shared schema/types are generated and conformance-tested
- [ ] This Interface Map lists the boundary with both source locations
- [ ] mdbook builds without warnings about missing links
- [ ] Running `mdbook build batchalign-book` passes

