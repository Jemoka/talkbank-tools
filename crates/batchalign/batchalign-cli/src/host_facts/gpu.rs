//! GPU presence facts.
//!
//! `GpuPresence` is the typed representation of "what GPU does this host
//! have, and is it usable for batchalign3 inference?" The latter half is
//! load-bearing: a host may have an Apple Silicon MPS device that's
//! technically present but excluded for batchalign3 by policy (kernel
//! deadlock under our workload). The `functional_for_batchalign` field
//! captures the *policy decision*, not just the hardware fact, so the
//! recommendation function can read one struct and answer "should this
//! host use the GPU?" without looking up exclusion lists elsewhere.
//!
//! The design rule: every variant that says "GPU is here" must also say
//! "and we have decided whether to use it." `None` is the only variant
//! that can be answered without a policy decision.

use std::process::Command;

use serde::{Deserialize, Serialize};

use super::os::{CpuArch, OperatingSystem};
use super::warnings::DetectionWarning;

/// What GPU is detected on this host, and whether batchalign3 will use it.
///
/// `functional_for_batchalign` on `AppleMps` and `Other` captures the
/// policy decision separately from the hardware fact. The recommendation
/// function reads it directly to decide `force_cpu` and
/// `gpu_thread_pool_size`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GpuPresence {
    /// No GPU detected, or no GPU stack we can use.
    None,
    /// Apple Metal Performance Shaders — present on every Apple Silicon
    /// Mac and on some Intel Macs. Currently `functional_for_batchalign`
    /// is always false; the kernel deadlock under batchalign3's Whisper
    /// workload (documented in `developer/apple-mps-workarounds.md`)
    /// rules MPS out for now.
    AppleMps {
        /// Whether batchalign3 will dispatch GPU work here. Currently
        /// always false; carried as a field rather than implied so a
        /// future fix can flip it without touching the rest of the
        /// pipeline.
        functional_for_batchalign: bool,
        /// Why MPS is excluded. `None` means MPS is in active use.
        reason_excluded: Option<MpsExclusionReason>,
    },
    /// NVIDIA CUDA — the production GPU path on Linux fleet hosts.
    NvidiaCuda {
        /// Number of CUDA-visible devices. Often 1; >1 on multi-GPU
        /// servers.
        device_count: u32,
        /// Total VRAM in MB across all visible devices. Used by the
        /// recommendation function for memory-aware budgeting.
        total_vram_mb: u64,
        /// Driver version string from `nvidia-smi`. Logged for debugging
        /// driver-vs-CUDA-version mismatches; not used by `recommend()`.
        driver_version: String,
    },
    /// A device we have not characterized — e.g., AMD ROCm, Intel Arc,
    /// future Apple GPU stacks. `functional_for_batchalign` defaults to
    /// false until we explicitly add support.
    Other {
        /// Human-readable identifier for the device class. Renamed away
        /// from `kind` to avoid colliding with the `tag = "kind"`
        /// serde attribute on the enum itself.
        device_kind: String,
        /// Whether batchalign3 will dispatch GPU work here.
        functional_for_batchalign: bool,
    },
}

/// Why batchalign3 has chosen to exclude an otherwise-present GPU.
///
/// New variants are added when new exclusion reasons are discovered;
/// the recommendation function treats every variant identically (force
/// CPU, set `gpu_thread_pool_size` to 1) — the variant exists for
/// observability so operator-facing tooling can explain *why*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MpsExclusionReason {
    /// Apple Silicon AGXG14X kernel deadlock under batchalign3's
    /// Whisper FA workload. See
    /// `book/src/developer/apple-mps-workarounds.md` for the full
    /// incident write-up.
    AppleSiliconKernelDeadlock,
}

impl GpuPresence {
    /// True when batchalign3 will actually dispatch inference to a GPU
    /// on this host. Encapsulates the "present + functional" check so
    /// the recommendation function reads one method instead of pattern-
    /// matching every variant.
    pub fn is_functional_for_batchalign(&self) -> bool {
        match self {
            GpuPresence::None => false,
            GpuPresence::AppleMps {
                functional_for_batchalign,
                ..
            }
            | GpuPresence::Other {
                functional_for_batchalign,
                ..
            } => *functional_for_batchalign,
            GpuPresence::NvidiaCuda { device_count, .. } => *device_count > 0,
        }
    }
}

// ----------------------------------------------------------------------
// Detection
// ----------------------------------------------------------------------

/// Outcome of running `nvidia-smi` (or whatever probe stands in for it
/// in tests). Three terminal states the detection function needs to
/// distinguish: not on PATH, ran-but-failed, ran-and-printed-output.
///
/// The output format we ask for is per-device CSV, one row per GPU,
/// columns `memory.total,driver_version` with `--format=csv,noheader,nounits`.
/// Picking CSV over `-q -x` (XML) avoids a heavy XML dep for two
/// fields; the parser is a few lines of `split`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NvidiaSmiOutcome {
    /// `nvidia-smi` is not on PATH at all. Expected on hosts without
    /// NVIDIA hardware (e.g., every Mac and most Linux dev boxes); the
    /// detector treats this as `GpuPresence::None`, no error.
    NotFound,
    /// The binary ran and exited 0; the captured stdout is `stdout`.
    /// Whether the content parses cleanly is the parser's call.
    Success {
        /// stdout payload as returned by the probe. The parser tolerates
        /// trailing whitespace.
        stdout: String,
    },
    /// The binary ran and exited non-zero. Most often this means the
    /// driver is in a bad state (kernel module mismatch, GPU hung).
    /// Treated as `GpuPresence::None` for safety; surfaced in
    /// `DetectionWarning` so an operator knows to investigate.
    Failed {
        /// Exit code from the `nvidia-smi` invocation.
        exit_code: i32,
        /// Captured stderr (truncated by the probe to a reasonable size).
        stderr: String,
    },
}

/// A source of `NvidiaSmiOutcome`. The trait exists so detection logic
/// can be unit-tested without spawning subprocesses; the production
/// impl is `RealNvidiaSmiProbe`, tests construct closures or static
/// `NvidiaSmiOutcome`-returning impls.
pub trait NvidiaSmiProbe: Send + Sync {
    /// Run the probe and return its outcome.
    fn probe(&self) -> NvidiaSmiOutcome;
}

/// Production probe — spawns `nvidia-smi` as a subprocess. Truncates
/// stderr to 4 KB on `Failed` so a runaway driver does not bloat the
/// `HostFacts` struct or log lines.
#[derive(Debug, Default)]
pub struct RealNvidiaSmiProbe;

impl NvidiaSmiProbe for RealNvidiaSmiProbe {
    fn probe(&self) -> NvidiaSmiOutcome {
        const STDERR_CAP: usize = 4096;
        let result = Command::new("nvidia-smi")
            .args([
                "--query-gpu=memory.total,driver_version",
                "--format=csv,noheader,nounits",
            ])
            .output();
        match result {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => NvidiaSmiOutcome::NotFound,
            Err(err) => {
                // Treat any non-NotFound spawn error as a failure with no
                // exit code (exec failed before the process started).
                NvidiaSmiOutcome::Failed {
                    exit_code: -1,
                    stderr: err.to_string(),
                }
            }
            Ok(output) if output.status.success() => NvidiaSmiOutcome::Success {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            },
            Ok(output) => NvidiaSmiOutcome::Failed {
                exit_code: output.status.code().unwrap_or(-1),
                stderr: truncate_at_char_boundary(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                    STDERR_CAP,
                ),
            },
        }
    }
}

/// Truncate `s` to at most `max_bytes` bytes without splitting a UTF-8
/// codepoint. `String::truncate` panics on a non-char-boundary offset;
/// nvidia-smi's stderr is well-formed UTF-8 so the issue is rare, but
/// the cap is a hard guarantee against runaway driver output and a
/// panic here would crash startup.
fn truncate_at_char_boundary(mut s: String, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s;
    }
    let mut cut = max_bytes;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    s
}

/// Pure detection function: given an OS/arch context and a probe,
/// produce the `GpuPresence` plus any non-fatal warnings. No I/O.
///
/// Algorithm:
/// - **Apple Silicon (macOS + arm64)** short-circuits to
///   `AppleMps { functional_for_batchalign: false, reason: AppleSiliconKernelDeadlock }`.
///   The `nvidia-smi` probe is **never called** on this path —
///   guaranteed by the function shape (the `probe` argument is unused
///   in this branch). This matters because Apple Silicon hosts have
///   no nvidia-smi binary; calling it would return `NotFound` and
///   pollute warnings with a non-issue.
/// - **Other macOS/arm64 combinations** (Intel mac, Apple Silicon
///   Linux): fall through to the probe.
/// - **Probe outcomes**:
///   - `NotFound` ⇒ `None` + `DetectionWarning::NvidiaSmiNotFound`
///   - `Failed` ⇒ `None` + `DetectionWarning::NvidiaSmiFailed { ... }`
///   - `Success { stdout }` ⇒ parse; on parse failure, `None` +
///     `DetectionWarning::NvidiaSmiUnparseable { detail }`; on
///     success, `NvidiaCuda { … }`.
pub fn detect_gpu_presence(
    os: &OperatingSystem,
    arch: &CpuArch,
    probe: &dyn NvidiaSmiProbe,
) -> (GpuPresence, Vec<DetectionWarning>) {
    if os.is_macos() && arch.is_arm64() {
        return (
            GpuPresence::AppleMps {
                functional_for_batchalign: false,
                reason_excluded: Some(MpsExclusionReason::AppleSiliconKernelDeadlock),
            },
            Vec::new(),
        );
    }

    match probe.probe() {
        NvidiaSmiOutcome::NotFound => {
            (GpuPresence::None, vec![DetectionWarning::NvidiaSmiNotFound])
        }
        NvidiaSmiOutcome::Failed { exit_code, stderr } => (
            GpuPresence::None,
            vec![DetectionWarning::NvidiaSmiFailed { exit_code, stderr }],
        ),
        NvidiaSmiOutcome::Success { stdout } => match parse_nvidia_smi_csv(&stdout) {
            Ok(row) => (
                GpuPresence::NvidiaCuda {
                    device_count: row.device_count,
                    total_vram_mb: row.total_vram_mb,
                    driver_version: row.driver_version,
                },
                Vec::new(),
            ),
            Err(err) => (
                GpuPresence::None,
                vec![DetectionWarning::NvidiaSmiUnparseable {
                    detail: err.to_string(),
                }],
            ),
        },
    }
}

/// Aggregated facts parsed from one `nvidia-smi` CSV invocation.
#[derive(Debug, PartialEq, Eq)]
struct CudaCsvRow {
    /// Number of visible CUDA devices (rows in the output).
    device_count: u32,
    /// Sum of `memory.total` across all visible devices, in MB.
    total_vram_mb: u64,
    /// Driver version from the first row. Identical across rows on a
    /// single host.
    driver_version: String,
}

/// Typed error from CSV parsing. Operator-readable via `Display`.
#[derive(Debug, thiserror::Error)]
enum CudaCsvParseError {
    #[error("nvidia-smi output had no GPU rows")]
    NoRows,
    #[error("line {line}: missing memory.total column")]
    MissingMemoryColumn { line: usize },
    #[error("line {line}: missing driver_version column")]
    MissingDriverColumn { line: usize },
    #[error("line {line}: driver_version column was empty")]
    EmptyDriver { line: usize },
    #[error("line {line}: memory.total {raw:?} is not an integer ({source})")]
    MemoryNotInteger {
        line: usize,
        raw: String,
        source: std::num::ParseIntError,
    },
}

/// Parse the per-device CSV output of
/// `nvidia-smi --query-gpu=memory.total,driver_version --format=csv,noheader,nounits`.
///
/// Expected: one non-empty line per visible device, two comma-
/// separated fields, both non-empty. The aggregate `CudaCsvRow` sums
/// VRAM across rows and uses the first row's driver version.
fn parse_nvidia_smi_csv(stdout: &str) -> Result<CudaCsvRow, CudaCsvParseError> {
    let mut device_count: u32 = 0;
    let mut total_vram_mb: u64 = 0;
    let mut driver_version: Option<String> = None;
    for (line_no, raw_line) in stdout.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let line_num = line_no + 1;
        let mut parts = line.splitn(2, ',').map(str::trim);
        let memory_str = parts
            .next()
            .ok_or(CudaCsvParseError::MissingMemoryColumn { line: line_num })?;
        let driver_str = parts
            .next()
            .ok_or(CudaCsvParseError::MissingDriverColumn { line: line_num })?;
        let memory_mb =
            memory_str
                .parse::<u64>()
                .map_err(|source| CudaCsvParseError::MemoryNotInteger {
                    line: line_num,
                    raw: memory_str.to_owned(),
                    source,
                })?;
        if driver_str.is_empty() {
            return Err(CudaCsvParseError::EmptyDriver { line: line_num });
        }
        device_count = device_count.saturating_add(1);
        total_vram_mb = total_vram_mb.saturating_add(memory_mb);
        if driver_version.is_none() {
            driver_version = Some(driver_str.to_owned());
        }
    }
    let driver_version = driver_version.ok_or(CudaCsvParseError::NoRows)?;
    Ok(CudaCsvRow {
        device_count,
        total_vram_mb,
        driver_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A probe that panics if invoked. Used in the Apple Silicon test
    /// to assert that detection short-circuits before touching the
    /// subprocess layer — important because `nvidia-smi` does not
    /// exist on Apple Silicon hosts and we must not pollute warnings
    /// with a `NotFound` for a probe we never should have run.
    struct NeverProbe;
    impl NvidiaSmiProbe for NeverProbe {
        fn probe(&self) -> NvidiaSmiOutcome {
            panic!("Apple Silicon detection must short-circuit before probing nvidia-smi");
        }
    }

    /// A probe that returns a fixed outcome. Used in tests where we
    /// want to assert what detection does *given* a particular probe
    /// outcome, decoupled from the live binary.
    struct FixedProbe(NvidiaSmiOutcome);
    impl NvidiaSmiProbe for FixedProbe {
        fn probe(&self) -> NvidiaSmiOutcome {
            self.0.clone()
        }
    }

    #[test]
    fn apple_silicon_short_circuits_to_mps_excluded() {
        let (presence, warnings) =
            detect_gpu_presence(&OperatingSystem::MacOs, &CpuArch::Arm64, &NeverProbe);
        assert_eq!(
            presence,
            GpuPresence::AppleMps {
                functional_for_batchalign: false,
                reason_excluded: Some(MpsExclusionReason::AppleSiliconKernelDeadlock),
            }
        );
        assert!(
            warnings.is_empty(),
            "Apple Silicon detection should not produce nvidia-smi warnings"
        );
    }

    #[test]
    fn linux_without_nvidia_smi_falls_to_none_with_warning() {
        let (presence, warnings) = detect_gpu_presence(
            &OperatingSystem::Linux,
            &CpuArch::X86_64,
            &FixedProbe(NvidiaSmiOutcome::NotFound),
        );
        assert_eq!(presence, GpuPresence::None);
        assert_eq!(warnings, vec![DetectionWarning::NvidiaSmiNotFound]);
    }

    #[test]
    fn linux_with_failing_nvidia_smi_falls_to_none_with_warning() {
        let (presence, warnings) = detect_gpu_presence(
            &OperatingSystem::Linux,
            &CpuArch::X86_64,
            &FixedProbe(NvidiaSmiOutcome::Failed {
                exit_code: 9,
                stderr: "Failed to initialize NVML: Driver/library version mismatch".to_owned(),
            }),
        );
        assert_eq!(presence, GpuPresence::None);
        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            DetectionWarning::NvidiaSmiFailed { exit_code, stderr } => {
                assert_eq!(*exit_code, 9);
                assert!(stderr.contains("Driver/library version mismatch"));
            }
            other => panic!("expected NvidiaSmiFailed, got {other:?}"),
        }
    }

    #[test]
    fn linux_with_parseable_nvidia_smi_returns_cuda() {
        let stdout = "24576, 535.183.01\n24576, 535.183.01\n".to_owned();
        let (presence, warnings) = detect_gpu_presence(
            &OperatingSystem::Linux,
            &CpuArch::X86_64,
            &FixedProbe(NvidiaSmiOutcome::Success { stdout }),
        );
        assert_eq!(
            presence,
            GpuPresence::NvidiaCuda {
                device_count: 2,
                total_vram_mb: 49_152,
                driver_version: "535.183.01".to_owned(),
            }
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn linux_with_unparseable_nvidia_smi_falls_to_none_with_warning() {
        let stdout = "this is not csv\n".to_owned();
        let (presence, warnings) = detect_gpu_presence(
            &OperatingSystem::Linux,
            &CpuArch::X86_64,
            &FixedProbe(NvidiaSmiOutcome::Success { stdout }),
        );
        assert_eq!(presence, GpuPresence::None);
        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            DetectionWarning::NvidiaSmiUnparseable { .. }
        ));
    }

    #[test]
    fn parse_nvidia_smi_csv_handles_empty_output() {
        let result = parse_nvidia_smi_csv("\n\n");
        assert!(result.is_err());
    }

    #[test]
    fn parse_nvidia_smi_csv_handles_trailing_newline() {
        let row = parse_nvidia_smi_csv("8192, 555.42.06\n").expect("valid CSV");
        assert_eq!(row.device_count, 1);
        assert_eq!(row.total_vram_mb, 8192);
        assert_eq!(row.driver_version, "555.42.06");
    }

    #[test]
    fn parse_nvidia_smi_csv_rejects_missing_driver_column() {
        let result = parse_nvidia_smi_csv("8192\n");
        assert!(result.is_err());
    }

    #[test]
    fn parse_nvidia_smi_csv_rejects_non_integer_memory() {
        let result = parse_nvidia_smi_csv("not_a_number, 555.42\n");
        assert!(result.is_err());
    }
}
