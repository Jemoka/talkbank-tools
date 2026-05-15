//! Shared `HostFacts` fixtures for tests across the host-facts pipeline.
//!
//! Test code in `recommendations`, `effective`, `validation`,
//! `mod.rs`, and the cross-crate doctor CLI all need synthesized
//! "Apple Silicon 64 GB" / "Linux CUDA 24 GB" host shapes to drive
//! their assertions. Centralizing the fixtures keeps every consumer
//! pointed at the same canonical fact set; if a future field is
//! added to `HostFacts`, one edit here updates every test.
//!
//! Always public so the cross-crate doctor tests in `batchalign-cli`
//! can `use crate::host_facts::test_helpers::*` without
//! wrestling with `#[cfg(test)]` visibility across the workspace.
//! The fixtures themselves are tiny pure-data builders — no state,
//! no I/O — so the public surface cost is essentially zero.

use crate::api::UnixTimestamp;

use super::{CpuArch, GpuPresence, HostFacts, MpsExclusionReason, OperatingSystem};

/// Canonical "Apple Silicon dev/fleet machine" fact set.
///
/// 64 GB physical RAM, 32 GB available, 12 logical / 8 physical
/// cores. GPU surfaces as `AppleMps { functional_for_batchalign:
/// false, reason: AppleSiliconKernelDeadlock }` — the policy
/// decision that the recommendation, validator, and doctor all
/// branch on.
pub fn apple_silicon_64gb() -> HostFacts {
    HostFacts {
        os: OperatingSystem::MacOs,
        arch: CpuArch::Arm64,
        cpu_logical_count: 12,
        cpu_physical_count: 8,
        ram_total_mb: 64 * 1024,
        ram_available_mb: 32 * 1024,
        gpu: GpuPresence::AppleMps {
            functional_for_batchalign: false,
            reason_excluded: Some(MpsExclusionReason::AppleSiliconKernelDeadlock),
        },
        disk_free_mb_for_cache: Some(500_000),
        hostname: "test-host".to_owned(),
        detection_timestamp: UnixTimestamp::from(1_700_000_000.0),
        detection_warnings: Vec::new(),
    }
}

/// Canonical "16 GB consumer laptop / GHA runner" fact set.
///
/// Sized to match the Small memory tier (< 24 GB) and shaped after
/// the GitHub Actions ubuntu-latest runner the Dashboard E2E test
/// runs on (16 GB physical, ~14 GB available after kernel/agent
/// overhead). Used by host-facts validator tests that pin the
/// "must work on Houjun's laptop" UX contract — a default
/// `uv tool install batchalign3` install on this host must not
/// be refused startup just because the capability surface
/// hypothetically includes GPU-class workloads the user has no
/// intention of running.
pub fn laptop_16gb() -> HostFacts {
    HostFacts {
        os: OperatingSystem::Linux,
        arch: CpuArch::X86_64,
        cpu_logical_count: 4,
        cpu_physical_count: 2,
        // GHA ubuntu-latest reports `MemTotal: 16370072 kB` ≈ 15_988 MB.
        // We use the round 15_989 to mirror the value Dashboard E2E
        // surfaced in CI.
        ram_total_mb: 15_989,
        ram_available_mb: 14_000,
        gpu: GpuPresence::None,
        disk_free_mb_for_cache: Some(50_000),
        hostname: "test-laptop".to_owned(),
        detection_timestamp: UnixTimestamp::from(1_700_000_000.0),
        detection_warnings: Vec::new(),
    }
}

/// Canonical "Linux + 1× NVIDIA CUDA 24 GB" fact set, sized to
/// resemble a single-A10G/L4 fleet host. Exists alongside the
/// Apple Silicon fixture so tests can exercise both branches of
/// the GPU-functional-vs-not split.
pub fn linux_cuda_24gb() -> HostFacts {
    HostFacts {
        os: OperatingSystem::Linux,
        arch: CpuArch::X86_64,
        cpu_logical_count: 16,
        cpu_physical_count: 8,
        ram_total_mb: 128 * 1024,
        ram_available_mb: 64 * 1024,
        gpu: GpuPresence::NvidiaCuda {
            device_count: 1,
            total_vram_mb: 24_000,
            driver_version: "555.42".into(),
        },
        disk_free_mb_for_cache: Some(1_000_000),
        hostname: "test-cuda".to_owned(),
        detection_timestamp: UnixTimestamp::from(1_700_000_000.0),
        detection_warnings: Vec::new(),
    }
}
