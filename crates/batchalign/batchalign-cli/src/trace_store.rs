//! Ephemeral in-memory trace store for algorithm visualization.
//!
//! Uses a moka async cache with TTL eviction. Traces are diagnostic-only
//! and not persisted to disk or database. When the server restarts, all
//! traces are lost.

use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;

use crate::api::JobId;
use crate::types::traces::{FileTraces, JobTraces};

/// Default maximum number of jobs with traces in the cache.
const DEFAULT_MAX_CAPACITY: u64 = 50;

/// Default time-to-live before eviction (1 hour).
const DEFAULT_TTL: Duration = Duration::from_secs(60 * 60);

/// Ephemeral store for algorithm traces, keyed by job ID.
///
/// Uses moka's async cache for automatic TTL eviction and bounded memory.
/// Thread-safe for concurrent reads and writes from multiple job runners.
///
/// Per-key updates use moka's `and_upsert_with`, which serializes concurrent
/// calls on the same key without a global lock — so concurrent FA file
/// completions for the same job are safe without blocking other jobs.
pub struct TraceStore {
    cache: Cache<JobId, Arc<JobTraces>>,
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceStore {
    /// Create a new trace store with default capacity (50 jobs) and TTL (1 hour).
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(DEFAULT_MAX_CAPACITY)
                .time_to_live(DEFAULT_TTL)
                .build(),
        }
    }

    /// Store traces for a completed job, replacing any existing entry.
    pub async fn insert(&self, job_id: JobId, traces: JobTraces) {
        self.cache.insert(job_id, Arc::new(traces)).await;
    }

    /// Atomically insert or update a single file's traces within a job.
    ///
    /// Creates the `JobTraces` entry if it doesn't exist yet.  Uses moka's
    /// per-key `and_upsert_with` which serializes concurrent calls on the
    /// same key — safe for multiple `process_one_fa_file` tasks finishing
    /// concurrently without blocking unrelated jobs.
    pub async fn upsert_file(&self, job_id: &JobId, file_index: usize, file_traces: FileTraces) {
        self.cache
            .entry_by_ref(job_id)
            .and_upsert_with(|maybe_entry| {
                let mut job_traces = maybe_entry
                    .map(|e| (*e.into_value()).clone())
                    .unwrap_or_default();
                job_traces.files.insert(file_index, file_traces);
                std::future::ready(Arc::new(job_traces))
            })
            .await;
    }

    /// Retrieve traces for a job, if they exist and haven't been evicted.
    pub async fn get(&self, job_id: &JobId) -> Option<Arc<JobTraces>> {
        self.cache.get(job_id).await
    }
}
