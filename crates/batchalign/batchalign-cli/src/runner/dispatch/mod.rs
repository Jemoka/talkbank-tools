//! Dispatch functions for released infer processing.
//!
//! This module is the "traffic director" between the job runner and
//! command-specific orchestrators.
//!
//! # Three dispatch shapes
//!
//! - [`dispatch_batched_infer`] handles text-only infer tasks that can pool
//!   multiple files into one worker `batch_infer` request path
//!   (`morphotag`, `utseg`, `translate`, `coref`).
//! - [`dispatch_fa_infer`] handles forced alignment (`align`), which is audio-
//!   bound and therefore processed per file (with per-group batching inside each
//!   file).
//! - [`dispatch_transcribe_infer`] handles transcription (`transcribe`), which
//!   takes audio input and produces CHAT output through a multi-step pipeline
//!   (ASR -> post-processing -> CHAT assembly -> optional utseg/morphosyntax).
//! - [`dispatch_benchmark_infer`] handles benchmarking (`benchmark`), which
//!   composes the Rust transcribe and compare pipelines for audio-plus-gold
//!   evaluation.
//!
//! # Why this split exists
//!
//! Text tasks operate on CHAT payloads only, so batching across files improves
//! throughput and model reuse. FA and transcribe require per-file audio, so
//! the top-level loop stays per-file to keep file/audio provenance and failure
//! handling deterministic.
//!
//! # Related modules
//!
//! - `crate::runner::mod` decides whether infer mode is enabled.
//! - `plan` translates store-owned job snapshots into typed command-family
//!   plans before orchestration begins.
//! - `crate::morphosyntax`, `crate::utseg`, `crate::translate`, `crate::coref`
//!   implement text-task orchestrators.
//! - `crate::fa` implements forced-alignment orchestration.
//! - `crate::transcribe` implements the multi-step transcribe orchestrator.

mod asr_media;
mod audio_output;
mod audio_task;
mod benchmark_pipeline;
mod fa_pipeline;
mod infer_batched;
mod media_analysis_v2;
mod options;
mod plan;
mod transcribe_pipeline;
mod utr;

pub(crate) use benchmark_pipeline::{BenchmarkDispatchRuntime, dispatch_benchmark_infer};
pub(crate) use fa_pipeline::*;
pub(crate) use infer_batched::dispatch_batched_infer;
pub(crate) use media_analysis_v2::{MediaAnalysisDispatchRuntime, dispatch_media_analysis_v2};
pub(crate) use plan::*;
pub(crate) use transcribe_pipeline::{TranscribeDispatchRuntime, dispatch_transcribe_infer};
