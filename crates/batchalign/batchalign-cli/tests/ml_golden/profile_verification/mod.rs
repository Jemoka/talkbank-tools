//! Worker profile verification tests — real-model resource usage assertions.
//!
//! These tests intentionally stay on the server path because they inspect
//! `/health`, live worker keys, and other server-side profile state. They verify
//! that the worker profile system correctly groups InferTasks into shared
//! workers, reducing memory consumption compared to per-task worker spawning.
//!
//! Requirements:
//! - Python 3 with batchalign installed
//! - FA models (Wave2Vec) for GPU profile tests
//! - Stanza models for Stanza profile tests
//!
//! Tests skip gracefully if models are unavailable.
//!
//! Run: `cargo nextest run -p batchalign-cli --test ml_golden --profile ml`

mod gpu;
mod helpers;
mod labels;
mod stanza;
