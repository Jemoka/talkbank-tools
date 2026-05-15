//! Internal pipeline helpers for command-local orchestration.
//!
//! This module intentionally stays private to `batchalign-server`. It is not a
//! general executor; it is a small sequential stage runner used to make
//! per-command orchestration explicit.

use crate::api::EngineVersion;
use crate::cache::UtteranceCache;
use crate::worker::pool::WorkerPool;

pub(crate) mod morphosyntax;
pub(crate) mod plan;
pub(crate) mod text_infer;
pub(crate) mod transcribe;

/// Shared services used by pipeline helpers.
///
/// `TreeSitterParser` is `!Send + !Sync` (uses `RefCell` internally), so it
/// cannot be stored here — `PipelineServices` is carried across async task
/// boundaries. Callers that need a parser create one locally via
/// `TreeSitterParser::new()`.
#[derive(Clone, Copy)]
pub(crate) struct PipelineServices<'a> {
    /// Worker pool for inference.
    pub pool: &'a WorkerPool,
    /// Shared utterance cache.
    pub cache: &'a UtteranceCache,
    /// Current engine version for cache keying.
    pub engine_version: &'a EngineVersion,
}

impl<'a> PipelineServices<'a> {
    /// Create services.
    pub fn new(
        pool: &'a WorkerPool,
        cache: &'a UtteranceCache,
        engine_version: &'a EngineVersion,
    ) -> Self {
        Self {
            pool,
            cache,
            engine_version,
        }
    }
}
