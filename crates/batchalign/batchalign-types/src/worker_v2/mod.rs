//! Worker protocol V2 schema types shared across batchalign crates.
//!
//! **Status:** Active production protocol for audio/media and text tasks.
//! **Frozen parallel protocol:** [`crate::worker`] (V1 JSON-lines, marked as frozen).
//!
//! The `worker_v2` suffix is intentional. The older JSON-lines `worker` surface remains
//! in-tree as a frozen compatibility contract, so the Rust module (`crates/batchalign-types/src/worker_v2/`),
//! schema directory (`schemas/ipc/worker_v2/`), generated Python package
//! (`python/batchalign/generated/worker_v2/`), and hand-written Pydantic overlays
//! (`python/batchalign/worker/_types_v2.py`) stay versioned together until V1 is removed as a whole.
//!
//! ## Protocol Ownership & Responsibility Boundaries
//!
//! | Layer | Location | Responsibility | Owner |
//! |-------|----------|-----------------|-------|
//! | **Schema definition (source of truth)** | [`crate::worker_v2`] (this module) | Request/response envelopes, error codes, task discriminators, serialization format | Rust (batchalign-types crate) |
//! | **IPC schema codegen** | `schemas/ipc/worker_v2/` | Generates Python types from Rust schema via `schemars` | Rust (build-time) |
//! | **Python validation & serialization** | `python/batchalign/worker/_types_v2.py` | Pydantic V2 models with field validation (e.g., non-finite float rejection, range checks) | Python |
//! | **Artifact preparation & caching** | `batchalign` task dispatch | Prepares typed audio/text artifacts before sending to Python | Rust (batchalign crate) |
//! | **Artifact storage & references** | Worker temp directories | Prepared artifacts (PCM, text, JSON) accessible via filesystem paths | Shared (Rust allocates, Python reads) |
//! | **Task orchestration & result postprocessing** | `batchalign` dispatch loop | Task retry logic, CHAT mutation, output assembly | Rust (batchalign crate) |
//! | **Model inference & preprocessing** | `python/batchalign/worker/` | Receives model-ready inputs, runs models, emits structured output | Python |
//!
//! ## Key Design Principles
//!
//! 1. **Rust owns the contract:** The schema in this module is the single source of truth.
//!    Python types are generated or hand-written to match, never the reverse.
//!
//! 2. **Python input validation is enforceable:** Pydantic validators in `_types_v2.py`
//!    (e.g., `@model_validator` on timing ranges) are part of the contract and must be
//!    maintained in sync when fields change. Rust does not re-validate because it trusts
//!    Python's wire output.
//!
//! 3. **Serialization format is stable:** The format and order of fields in request/response
//!    enums are part of the wire contract. Adding or reordering fields requires migration
//!    steps and version bumps.
//!
//! 4. **Backward compatibility is optional:** The V2 protocol is not required to remain
//!    backward-compatible with V1. If the entire fleet must migrate atomically (e.g., during
//!    a single deployment cycle), incompatible wire-format changes are acceptable.
//!
//! 5. **Artifacts live in the data plane, not in request bodies:** Large binary inputs
//!    (prepared audio, prepared text) are stored as files with explicit references, not
//!    embedded in JSON. This allows Rust to manage lifecycle and caching independently.
//!
//! ## Current Status of Task Migration
//!
//! - **Fully migrated to V2:** FA (forced-alignment), ASR (automatic speech recognition),
//!   Speaker ID, OpenSmile, AVQI, Morphosyntax, Utseg, Translation, Coref.
//!
//! - **Still using V1 (frozen):** Direct Python model inference via `dispatch_batch_infer`
//!   (JSON-lines interface in [`crate::worker`]). Used for backward compatibility and
//!   direct Python execution paths not routed through the task orchestration layer.
//!
//! - **Active dispatch methods:** `dispatch_execute_v2` (V2, primary) and `dispatch_batch_infer` (V1, legacy).
//!
//! ## Memory Field Design & Ownership
//!
//! Prepared artifacts (audio, text) carry explicit ownership semantics in their references:
//!
//! - **PreparedAudioRefV2**: Filesystem path + metadata (sample rate, channels, frame count,
//!   byte range). Rust allocates the artifact; Python reads via `mmap` or file I/O.
//!
//! - **PreparedTextRefV2**: Filesystem path + encoding hint. Rust prepares and owns the file;
//!   Python reads and deserializes.
//!
//! - **Lifecycle:** Rust creates artifacts in the worker's temp directory before sending the request.
//!   Python processes them during `execute_v2`. Rust may clean up after a response (configurable
//!   per artifact type for debugging/logging purposes).
//!
//! Split into submodules:
//! - [`requests`] — request envelopes, task payloads, shared enums, newtypes
//! - [`responses`] — result types, execute response, progress events

pub mod requests;
pub mod responses;

pub use requests::*;
pub use responses::*;
