//! Shared concurrent GPU worker wrappers.
//!
//! ## Module layout
//!
//! | File | Responsibility |
//! |------|----------------|
//! | `mod.rs` | Shared types, envelope deserialization helpers, re-exports |
//! | `stdio.rs` | `SharedGpuWorker` — concurrent V2 dispatch over stdio |
//! | `tcp.rs` | `SharedGpuTcpWorker` — concurrent V2 dispatch over TCP |
//! | `reader.rs` | Generic JSON-lines reader loop shared by both transports |

mod reader;
mod stdio;
mod tcp;

pub(crate) use stdio::SharedGpuWorker;
pub(crate) use tcp::SharedGpuTcpWorker;

use tokio::sync::Semaphore;

use crate::types::worker_v2::ExecuteResponseV2;
pub(crate) use crate::worker::EnsureTaskResponse;

/// Convert a `gpu_thread_pool_size` value into a permit count for the
/// dispatch semaphore. Floor at 1 (zero permits would deadlock every
/// caller) and clamp to `Semaphore::MAX_PERMITS`. Shared between the
/// stdio and TCP shared-GPU-worker constructors so both transports
/// derive the same in-flight ceiling from the same input.
pub(super) fn dispatch_permits_from(gpu_thread_pool_size: u32) -> usize {
    gpu_thread_pool_size
        .max(1)
        .min(u32::try_from(Semaphore::MAX_PERMITS).unwrap_or(u32::MAX)) as usize
}

/// Non-V2 responses routed via the control channel.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum WorkerControlResponse {
    Health(crate::worker::WorkerHealthResponse),
    Capabilities(crate::worker::WorkerCapabilities),
    EnsureTask(EnsureTaskResponse),
    Shutdown,
    Error(String),
}

/// Deserialization envelope types used by the reader loop to parse
/// JSON-lines responses from the worker process.
pub(super) mod envelopes {
    /// Helper envelope for deserializing `{"op": "execute_v2", "response": {...}}`.
    #[derive(serde::Deserialize)]
    pub(crate) struct ExecuteResponseV2Envelope {
        pub(crate) response: super::ExecuteResponseV2,
    }

    /// Helper envelope for deserializing `{"op": "health", "response": {...}}`.
    #[derive(serde::Deserialize)]
    pub(crate) struct HealthResponseEnvelope {
        pub(crate) response: crate::worker::WorkerHealthResponse,
    }

    /// Helper envelope for deserializing `{"op": "capabilities", "response": {...}}`.
    #[derive(serde::Deserialize)]
    pub(crate) struct CapabilitiesResponseEnvelope {
        pub(crate) response: crate::worker::WorkerCapabilities,
    }

    /// Helper envelope for deserializing `{"op": "ensure_task", "response": {...}}`.
    #[derive(serde::Deserialize)]
    pub(crate) struct EnsureTaskResponseEnvelope {
        pub(crate) response: super::EnsureTaskResponse,
    }
}
