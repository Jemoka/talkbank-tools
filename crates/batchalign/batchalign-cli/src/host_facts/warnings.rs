//! Non-fatal probe warnings.
//!
//! `DetectionWarning` carries information about probes that did not
//! succeed but did not block detection — e.g., `nvidia-smi` returned a
//! non-zero exit, the Stanza resources file was unreadable, the Python
//! interpreter could not be located. The point is to surface these in
//! `batchalign3 doctor` output so an operator can see *why* a host
//! fact is missing, without crashing the server at startup.
//!
//! Errors that genuinely block detection (e.g., the OS itself is
//! unidentifiable) belong elsewhere — `HostFacts::detect()` returns a
//! struct, not a `Result`. Any failure deep enough that we cannot fill
//! out a sensible `HostFacts` would indicate a much larger problem.

use serde::{Deserialize, Serialize};

/// A non-fatal issue surfaced during host fact detection.
///
/// Variants are added as new probe failure modes are characterized. The
/// `Display` impl produces the operator-facing message; the `Debug` impl
/// is used in tracing logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DetectionWarning {
    /// `nvidia-smi` was not found on PATH. Expected on hosts with no
    /// NVIDIA hardware; the detector treats this as `GpuPresence::None`
    /// rather than an error.
    #[error("nvidia-smi not found on PATH; treating as no CUDA GPU")]
    NvidiaSmiNotFound,
    /// `nvidia-smi` was found but exited non-zero. The host *might* have
    /// a CUDA GPU in a bad state. The detector falls back to
    /// `GpuPresence::None`; the operator should investigate.
    #[error("nvidia-smi failed (exit {exit_code}): {stderr}")]
    NvidiaSmiFailed {
        /// Exit code from the `nvidia-smi` invocation.
        exit_code: i32,
        /// Captured stderr (truncated to a reasonable size by the probe).
        stderr: String,
    },
    /// `nvidia-smi` ran and exited 0 but emitted unparseable output.
    /// Treated as `GpuPresence::None` for safety; the operator should
    /// upgrade the driver or report the parse failure.
    #[error("nvidia-smi output could not be parsed: {detail}")]
    NvidiaSmiUnparseable {
        /// Short description of what failed to parse (e.g., "missing
        /// `attached_gpus` field").
        detail: String,
    },
}
