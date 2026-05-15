//! Shared retry policy for worker inference calls.
//!
//! This helper keeps batch-oriented server orchestrators from immediately
//! degrading transient worker failures into terminal per-file errors. The retry
//! boundary is the actual worker request, not the higher-level per-file
//! orchestration that consumes its results.

use std::time::Duration;

use crate::api::LanguageCode3;
use crate::scheduling::RetryPolicy;
use crate::types::worker_v2::{ExecuteRequestV2, ExecuteResponseV2};
use crate::worker::pool::WorkerPool;
use tracing::warn;

use crate::error::ServerError;
use crate::runner::util::{classify_worker_error, is_retryable_worker_failure};

/// Dispatch one `execute_v2` request with automatic retries for transient worker
/// failures.
pub(crate) async fn dispatch_execute_v2_with_retry(
    pool: &WorkerPool,
    lang: &LanguageCode3,
    request: &ExecuteRequestV2,
) -> Result<ExecuteResponseV2, ServerError> {
    dispatch_execute_v2_with_retry_and_progress(pool, lang, request, None).await
}

/// Dispatch one `execute_v2` request with retries and progress forwarding.
pub(crate) async fn dispatch_execute_v2_with_retry_and_progress(
    pool: &WorkerPool,
    lang: &LanguageCode3,
    request: &ExecuteRequestV2,
    progress_tx: Option<&tokio::sync::mpsc::Sender<crate::types::worker_v2::ProgressEventV2>>,
) -> Result<ExecuteResponseV2, ServerError> {
    let retry_policy = RetryPolicy::default();

    for attempt_number in 1..=retry_policy.max_attempts {
        match pool
            .dispatch_execute_v2_with_progress(lang, request, progress_tx)
            .await
        {
            Ok(response) => return Ok(response),
            Err(error) => {
                let category = classify_worker_error(&error);
                let has_retry_budget = attempt_number < retry_policy.max_attempts;

                if is_retryable_worker_failure(category) && has_retry_budget {
                    let backoff_ms = retry_policy.backoff_for_retry(attempt_number);
                    warn!(
                        task = ?request.task,
                        lang = %lang,
                        attempt_number,
                        max_attempts = retry_policy.max_attempts,
                        error = %error,
                        category = %category,
                        %backoff_ms,
                        "Retrying execute_v2 after transient worker failure"
                    );
                    tokio::time::sleep(Duration::from_millis(backoff_ms.0)).await;
                    continue;
                }

                return Err(ServerError::Worker(error));
            }
        }
    }

    // Loop invariant: every iteration either returns `Ok(...)` on
    // success, returns `Err(...)` on a non-retryable category, or
    // `continue`s after a backoff. The terminal `continue` is gated
    // by `attempt_number < retry_policy.max_attempts`, so the for-loop
    // bound is exclusive. This `unreachable!` therefore covers the
    // case where the loop exits without taking either return path —
    // which the bound guarantees cannot happen.
    #[allow(clippy::unreachable)]
    {
        unreachable!("retry loop should return on success or terminal failure")
    }
}
