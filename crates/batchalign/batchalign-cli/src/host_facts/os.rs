//! Operating-system and CPU-architecture facts.
//!
//! Variants enumerate everything `recommend()` needs to branch on,
//! plus an `Other` escape hatch so the binary keeps starting on
//! platforms we have not characterized (the recommendation function
//! treats `Other` conservatively). The `Other` payload is
//! `Cow<'static, str>` so the common case (mapping
//! `std::env::consts::OS` / `ARCH`, both `&'static str`) costs no
//! allocation.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// The host operating system family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatingSystem {
    /// Apple macOS — both Intel and Apple Silicon.
    MacOs,
    /// Linux — server fleet, CUDA hosts.
    Linux,
    /// Windows — recently joining the fleet; treat conservatively until
    /// we have direct experience.
    Windows,
    /// A platform we have not characterized. The payload is the raw
    /// source string (`std::env::consts::OS` in production) so debug
    /// logs can identify it without allocating in the common case.
    Other(Cow<'static, str>),
}

/// The host CPU architecture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuArch {
    /// 64-bit ARM (Apple Silicon, AWS Graviton, …).
    Arm64,
    /// 64-bit x86 (Intel/AMD).
    X86_64,
    /// A CPU architecture we have not characterized.
    Other(Cow<'static, str>),
}

impl OperatingSystem {
    /// True for macOS — Apple Silicon detection requires the OS check too.
    pub fn is_macos(&self) -> bool {
        matches!(self, OperatingSystem::MacOs)
    }

    /// Map a `std::env::consts::OS`-shaped string to a typed variant.
    /// Unrecognized values fall through to `Other` carrying the
    /// `&'static str` directly (no allocation).
    pub fn from_consts(os: &'static str) -> Self {
        match os {
            "macos" => Self::MacOs,
            "linux" => Self::Linux,
            "windows" => Self::Windows,
            other => Self::Other(Cow::Borrowed(other)),
        }
    }
}

impl CpuArch {
    /// True for arm64.
    pub fn is_arm64(&self) -> bool {
        matches!(self, CpuArch::Arm64)
    }

    /// Map a `std::env::consts::ARCH`-shaped string to a typed variant.
    pub fn from_consts(arch: &'static str) -> Self {
        match arch {
            "aarch64" => Self::Arm64,
            "x86_64" => Self::X86_64,
            other => Self::Other(Cow::Borrowed(other)),
        }
    }
}
