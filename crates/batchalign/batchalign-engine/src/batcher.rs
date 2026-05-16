//! Per-backend batcher loop (spec2.md §11.1).
//!
//! One of these runs per registered backend. It accumulates `BatchItem`s
//! from the shared mpsc channel, drains the cache for hits, then ships the
//! cache-miss inputs to the backend via `spawn_blocking` (so Python's GIL
//! doesn't park a tokio worker).

use std::sync::Arc;

use batchalign_core::{BAError, BAResult, Backend, BatchPolicy, TaskInput, TaskOutput};
use tokio::sync::{mpsc, oneshot};

use crate::backend_impl::BackendImpl;
use crate::cache::Cache;

/// One in-flight request as it travels backend-ward.
pub struct BatchItem {
    pub input: TaskInput,
    pub reply: oneshot::Sender<BAResult<TaskOutput>>,
}

/// The batcher loop. Returns when its receiver is closed (engine shutdown).
///
/// Behavior (in lockstep with the spec, comments mirror the spec rationale):
///  * Block on the first item or channel-close.
///  * Drain up to `policy.max_size` items, or until `policy.window_ms`
///    elapses, whichever first. Uses `biased` select so the deadline wins
///    ties — the predictable choice for batching.
///  * Serve cache hits inline. Accumulate misses, ship them once.
///  * Call `backend.call(misses)` via `tokio::task::spawn_blocking` —
///    required so Python backends acquiring the GIL inside `call` don't
///    block the tokio runtime workers.
///  * Output length is required to match input length; mismatch is a
///    contract violation that errors all replies.
///  * Per-input cache writes happen on the happy path before reply.
pub async fn batcher_loop(
    backend: Arc<BackendImpl>,
    policy: BatchPolicy,
    cache: Arc<Cache>,
    mut rx: mpsc::UnboundedReceiver<BatchItem>,
) {
    let window = std::time::Duration::from_millis(policy.window_ms);
    let max_size = policy.max_size.max(1);

    loop {
        // 1. Block until first item or channel closes.
        let first = match rx.recv().await {
            Some(item) => item,
            None => return,
        };

        let mut buf: Vec<BatchItem> = Vec::with_capacity(max_size);
        buf.push(first);

        // 2. Drain up to max_size or until window elapses.
        let deadline = tokio::time::sleep(window);
        tokio::pin!(deadline);
        while buf.len() < max_size {
            tokio::select! {
                biased;
                _ = &mut deadline => break,
                next = rx.recv() => match next {
                    Some(item) => buf.push(item),
                    None => break,
                }
            }
        }

        // 3. Cache lookup: reply hits inline, accumulate misses.
        let backend_name = backend.name().to_string();
        let mut fresh_inputs: Vec<TaskInput> = Vec::with_capacity(buf.len());
        let mut fresh_replies: Vec<oneshot::Sender<BAResult<TaskOutput>>> =
            Vec::with_capacity(buf.len());
        for item in buf {
            let task = item.input.task();
            if let Some(cached) = cache.get(task, &backend_name, &item.input) {
                let _ = item.reply.send(Ok(cached));
            } else {
                fresh_inputs.push(item.input);
                fresh_replies.push(item.reply);
            }
        }
        if fresh_inputs.is_empty() {
            continue;
        }

        // 4. Call backend off the runtime thread.
        let n = fresh_inputs.len();
        let backend_for_call = backend.clone();
        let inputs_for_cache = fresh_inputs.clone();
        let result = tokio::task::spawn_blocking(move || backend_for_call.call(fresh_inputs)).await;

        // 5. Distribute outputs (or errors) back through the oneshot replies.
        match result {
            Ok(Ok(outputs)) if outputs.len() == n => {
                for ((reply, input), output) in fresh_replies
                    .into_iter()
                    .zip(inputs_for_cache.into_iter())
                    .zip(outputs.into_iter())
                {
                    cache.put(input.task(), &backend_name, &input, &output);
                    let _ = reply.send(Ok(output));
                }
            }
            Ok(Ok(outputs)) => {
                let msg = format!(
                    "backend {backend_name:?} returned wrong-length batch: {} != {}",
                    outputs.len(),
                    n
                );
                for reply in fresh_replies {
                    let _ = reply.send(Err(BAError::Worker(msg.clone())));
                }
            }
            Ok(Err(err)) => {
                let msg = format!("backend error: {err:#}");
                for reply in fresh_replies {
                    let _ = reply.send(Err(BAError::Worker(msg.clone())));
                }
            }
            Err(join_err) => {
                let msg = format!("backend task panicked: {join_err}");
                for reply in fresh_replies {
                    let _ = reply.send(Err(BAError::Worker(msg.clone())));
                }
            }
        }
    }
}
