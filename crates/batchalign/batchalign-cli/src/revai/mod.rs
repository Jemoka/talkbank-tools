//! Rust-owned Rev.AI integration for the server control plane.
//!
//! This module is where batchalign-app keeps Rev.AI work that does not belong
//! in Python:
//! - loading credentials for server-owned operations
//! - pre-submitting batches of media files in parallel
//! - direct server-owned Rev.AI ASR inference for transcribe/benchmark
//!
//! In server mode, Rev.AI is no longer a Python worker concern. The worker
//! boundary remains reserved for engines that genuinely require Python runtime
//! or model libraries.

mod asr;
mod client;
mod credentials;
mod preflight;
mod types;
mod utr;

pub(crate) use asr::infer_revai_asr;
pub(crate) use client::{Result, RevAiClient, TranscriptResult, extract_timed_words};
pub(crate) use credentials::{RevAiApiKey, RevAiCredentialError, load_revai_api_key};
pub(crate) use preflight::{
    RevAiLanguageHint, RevAiPreflightPlan, preflight_submit_audio_paths, revai_known_broken,
    try_revai_language_hint,
};
pub(crate) use types::{SubmitOptions, Transcript};
pub(crate) use utr::infer_revai_utr;
