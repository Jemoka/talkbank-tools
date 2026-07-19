//! Per-backend batcher loop (spec2.md §11.1).
//!
//! One of these runs per registered backend. It accumulates `BatchItem`s
//! from the shared mpsc channel, drains the cache for hits, then ships the
//! cache-miss inputs to the backend via `spawn_blocking` (so Python's GIL
//! doesn't park a tokio worker).

use std::sync::Arc;

use batchalign_core::{
    BAError, BAResult, Backend, BackendProgress, BatchPolicy, NullBackendProgress, TaskInput,
    TaskOutput,
};
// (NullBackendProgress is the batched-batch progress; see fan-in comment below.)
use tokio::sync::{mpsc, oneshot};

use crate::backend_impl::BackendImpl;
use crate::cache::Cache;

/// One in-flight request as it travels backend-ward.
///
/// `progress` is the per-item progress handle: for an atomic-call backend
/// (max_size=1) the batcher hands it straight to the backend; for genuinely
/// batched backends (max_size>1) the batcher cannot meaningfully fan a
/// single per-batch tick stream back to multiple per-item channels, so it
/// passes [`NullBackendProgress`] for the whole batch. The trait object is
/// pinned via an `Arc` so the batcher (running on a `spawn_blocking`
/// thread) keeps it alive across the call.
pub struct BatchItem {
    pub input: TaskInput,
    pub reply: oneshot::Sender<BAResult<TaskOutput>>,
    pub progress: Arc<dyn BackendProgress>,
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
    mut rx: mpsc::Receiver<BatchItem>,
) {
    let window = std::time::Duration::from_millis(policy.window_ms);
    let max_size = policy.max_size.max(1);

    let backend_name = backend.name().to_string();
    loop {
        // 1. Block until first item or channel closes.
        let first = match rx.recv().await {
            Some(item) => item,
            None => return,
        };

        // 2. Cache-check the first item up front. If it hits we never start
        //    the batching window — serial cache-hit dispatch (the common
        //    "re-run with full cache" path) gets to skip `window_ms` per call
        //    instead of paying it once for every dispatch.
        let mut fresh_inputs: Vec<TaskInput> = Vec::with_capacity(max_size);
        let mut fresh_replies: Vec<oneshot::Sender<BAResult<TaskOutput>>> =
            Vec::with_capacity(max_size);
        // Per-input progress handles travel alongside the inputs so the
        // batcher can pick the right one for the actual `backend.call`.
        let mut fresh_progress: Vec<Arc<dyn BackendProgress>> = Vec::with_capacity(max_size);
        let mut had_miss = false;
        {
            let task = first.input.task();
            if let Some(cached) = cache.get(task, &backend_name, &first.input) {
                let _ = first.reply.send(Ok(cached));
            } else {
                fresh_inputs.push(first.input);
                fresh_replies.push(first.reply);
                fresh_progress.push(first.progress);
                had_miss = true;
            }
        }

        // 3. Drain up to max_size or until window elapses. Start the window
        //    only after we see the first miss — a queue full of hits should
        //    drain at channel-recv latency.
        if had_miss {
            let deadline = tokio::time::sleep(window);
            tokio::pin!(deadline);
            while fresh_inputs.len() < max_size {
                tokio::select! {
                    biased;
                    _ = &mut deadline => break,
                    next = rx.recv() => match next {
                        Some(item) => {
                            let task = item.input.task();
                            if let Some(cached) = cache.get(task, &backend_name, &item.input) {
                                let _ = item.reply.send(Ok(cached));
                            } else {
                                fresh_inputs.push(item.input);
                                fresh_replies.push(item.reply);
                                fresh_progress.push(item.progress);
                            }
                        }
                        None => break,
                    }
                }
            }
        } else {
            // No miss yet — opportunistically drain any items that are already
            // queued so we still batch when callers submit faster than we
            // process. `try_recv` returns immediately when the channel is
            // empty, so we never block here.
            while fresh_inputs.len() < max_size {
                match rx.try_recv() {
                    Ok(item) => {
                        let task = item.input.task();
                        if let Some(cached) = cache.get(task, &backend_name, &item.input) {
                            let _ = item.reply.send(Ok(cached));
                        } else {
                            fresh_inputs.push(item.input);
                            fresh_replies.push(item.reply);
                            fresh_progress.push(item.progress);
                        }
                    }
                    Err(_) => break,
                }
            }
        }

        if fresh_inputs.is_empty() {
            continue;
        }

        // 4. Call backend off the runtime thread.
        //
        // Progress fan-in: a single `backend.call(batch)` produces one
        // tick stream, but the batch may have come from multiple
        // dispatchers each with their own progress handle. For atomic-
        // call backends (`max_size=1`) the mapping is unambiguous — hand
        // the lone item's handle straight to the backend. For genuinely
        // batched backends (`max_size>1`) there is no sensible per-item
        // attribution of "I'm done with audio group 3 of 17" across N
        // callers, so we pass `NullBackendProgress` and rely on the
        // runner's outer `start_step` ticks alone. If a future backend
        // really needs per-item ticks while batching, the right fix is
        // a tick-with-item-id protocol, not heuristic fan-out here.
        let n = fresh_inputs.len();
        let call_progress: Arc<dyn BackendProgress> = if n == 1 {
            fresh_progress[0].clone()
        } else {
            Arc::new(NullBackendProgress)
        };
        let backend_for_call = backend.clone();
        let inputs_for_cache = fresh_inputs.clone();
        let backend_name_clone = backend_name.clone();
        let result = tokio::task::spawn_blocking(move || {
            backend_for_call.call_with_progress(fresh_inputs, call_progress)
        })
        .await;

        // 5. Distribute outputs (or errors) back through the oneshot replies.
        match result {
            Ok(Ok(outputs)) if outputs.len() == n => {
                // Cache writes hit LMDB's process-wide writer lock, which
                // can briefly block (if another batcher or another process
                // is also writing). Offload to `spawn_blocking` so we
                // don't park a tokio worker on the lock. Reply to the
                // dispatcher BEFORE awaiting the cache put — the cache
                // is best-effort and shouldn't gate the response. Errors
                // from put are already swallowed (logged) inside `Cache`.
                let cache_for_put = cache.clone();
                let backend_name_for_put = backend_name_clone.clone();
                let mut to_cache: Vec<(TaskInput, TaskOutput)> = Vec::with_capacity(n);
                for ((reply, input), output) in fresh_replies
                    .into_iter()
                    .zip(inputs_for_cache.into_iter())
                    .zip(outputs.into_iter())
                {
                    to_cache.push((input, output.clone()));
                    let _ = reply.send(Ok(output));
                }
                let put_handle = tokio::task::spawn_blocking(move || {
                    for (input, output) in to_cache {
                        cache_for_put.put(input.task(), &backend_name_for_put, &input, &output);
                    }
                });
                // Await so the put commits before runtime shutdown can
                // cancel the blocking task. Replies have already been
                // sent above (line 186), so dispatchers are unblocked;
                // this only delays the *next* batcher iteration, not
                // user-visible latency.
                let _ = put_handle.await;
            }
            Ok(Ok(outputs)) => {
                let msg = format!(
                    "backend {backend_name_clone:?} returned wrong-length batch: {} != {}",
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
