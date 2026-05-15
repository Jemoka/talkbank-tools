//! Shared release-facing manifest for the published `chatter` command surface.

#![allow(dead_code)]

/// Help scope for one command-surface group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceScope {
    /// Commands listed by `chatter --help`.
    TopLevel,
    /// Commands listed by `chatter clan --help`.
    Clan,
}

/// Functional family for one command-surface group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceFamily {
    /// Validation, watch, and lint flows.
    Validation,
    /// Normalize and inspect commands over one CHAT file.
    Formatting,
    /// JSON or alignment conversion/inspection commands.
    Conversion,
    /// Cache maintenance flows.
    Cache,
    /// Schema-printing surface.
    Schema,
    /// Top-level gateway into the CLAN subcommand surface.
    ClanHub,
    /// CLAN analysis/statistics commands.
    ClanAnalysis,
    /// CLAN transform commands that rewrite CHAT files.
    ClanTransform,
    /// CLAN converter commands for external formats.
    ClanConverter,
    /// Preserved compatibility shims with real behavior.
    ClanCompatibilityShim,
    /// Preserved compatibility placeholders that stay intentionally
    /// unimplemented.
    ClanCompatibilityPlaceholder,
}

/// Release-readiness coverage expectations for one surface family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageExpectation {
    /// The command must appear in CLI help.
    HelpContract,
    /// The command family wants systematic option/argument matrix coverage.
    OptionMatrix,
    /// The command family has human-readable or structured output contracts.
    OutputContract,
    /// The command family depends on cache, watch, path, or other runtime state.
    StatefulPath,
    /// The command exists specifically because we preserve a legacy CLI surface.
    LegacyCompatibility,
}

/// One reviewed command-surface family entry.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceGroup {
    /// Whether the commands live at top level or under `clan`.
    pub scope: SurfaceScope,
    /// Functional family and compatibility posture.
    pub family: SurfaceFamily,
    /// Concrete published command names as shown in clap help.
    pub commands: &'static [&'static str],
    /// Coverage work that must exist for this group.
    pub coverage: &'static [CoverageExpectation],
    /// Brief rationale for why this grouping exists.
    pub note: &'static str,
}

const VALIDATION_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::StatefulPath,
];

const FORMATTING_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::OutputContract,
];

const CONVERSION_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::OutputContract,
];

const CACHE_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::StatefulPath,
];

const SCHEMA_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OutputContract,
];

const CLAN_HUB_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::LegacyCompatibility,
];

const CLAN_ANALYSIS_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::OutputContract,
];

const CLAN_TRANSFORM_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::OutputContract,
];

const CLAN_CONVERTER_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::OptionMatrix,
    CoverageExpectation::OutputContract,
];

const CLAN_LEGACY_COVERAGE: &[CoverageExpectation] = &[
    CoverageExpectation::HelpContract,
    CoverageExpectation::LegacyCompatibility,
];

/// Reviewed release-facing command-surface groups.
pub const SURFACE_GROUPS: &[SurfaceGroup] = &[
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Validation,
        commands: &["validate", "watch", "lint"],
        coverage: VALIDATION_COVERAGE,
        note: "validation lifecycle and continuous feedback commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Formatting,
        commands: &["normalize", "clean", "new-file"],
        coverage: FORMATTING_COVERAGE,
        note: "single-file normalization, inspection, and scaffold commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Conversion,
        commands: &["to-json", "from-json", "show-alignment"],
        coverage: CONVERSION_COVERAGE,
        note: "JSON conversion and alignment inspection commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Cache,
        commands: &["cache"],
        coverage: CACHE_COVERAGE,
        note: "stateful validation-cache maintenance surface",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::Schema,
        commands: &["schema"],
        coverage: SCHEMA_COVERAGE,
        note: "JSON schema printing surface",
    },
    SurfaceGroup {
        scope: SurfaceScope::TopLevel,
        family: SurfaceFamily::ClanHub,
        commands: &["clan"],
        coverage: CLAN_HUB_COVERAGE,
        note: "top-level gateway to the preserved CLAN-style command family",
    },
    SurfaceGroup {
        scope: SurfaceScope::Clan,
        family: SurfaceFamily::ClanAnalysis,
        commands: &[
            "freq",
            "mlu",
            "mlt",
            "wdlen",
            "wdsize",
            "maxwd",
            "freqpos",
            "timedur",
            "kwal",
            "gemlist",
            "combo",
            "cooccur",
            "dist",
            "chip",
            "phonfreq",
            "modrep",
            "vocd",
            "uniq",
            "codes",
            "trnfix",
            "sugar",
            "mortable",
            "chains",
            "complexity",
            "corelex",
            "keymap",
            "script",
            "rely",
            "flucalc",
            "dss",
            "ipsyn",
            "eval",
            "kideval",
            "eval-d",
        ],
        coverage: CLAN_ANALYSIS_COVERAGE,
        note: "statistics and scoring commands over CHAT corpora",
    },
    SurfaceGroup {
        scope: SurfaceScope::Clan,
        family: SurfaceFamily::ClanTransform,
        commands: &[
            "flo",
            "lowcase",
            "chstring",
            "dates",
            "delim",
            "fixbullets",
            "retrace",
            "repeat",
            "combtier",
            "compound",
            "tierorder",
            "lines",
            "dataclean",
            "quotes",
            "ort",
            "postmortem",
            "makemod",
            "trim",
            "roles",
            "indent",
        ],
        coverage: CLAN_TRANSFORM_COVERAGE,
        note: "commands that rewrite or normalize CHAT files",
    },
    SurfaceGroup {
        scope: SurfaceScope::Clan,
        family: SurfaceFamily::ClanConverter,
        commands: &[
            "chat2text",
            "srt2chat",
            "chat2srt",
            "chat2vtt",
            "text2chat",
            "lipp2chat",
            "elan2chat",
            "praat2chat",
            "chat2praat",
            "lena2chat",
            "play2chat",
            "lab2chat",
            "rtf2chat",
            "salt2chat",
            "gem",
            "chat2elan",
        ],
        coverage: CLAN_CONVERTER_COVERAGE,
        note: "format conversion surface around CHAT and adjacent tools",
    },
    SurfaceGroup {
        scope: SurfaceScope::Clan,
        family: SurfaceFamily::ClanCompatibilityShim,
        commands: &["check", "fixit", "longtier", "gemfreq"],
        coverage: CLAN_LEGACY_COVERAGE,
        note: "preserved CLAN compatibility entries implemented by routing to current commands",
    },
    SurfaceGroup {
        scope: SurfaceScope::Clan,
        family: SurfaceFamily::ClanCompatibilityPlaceholder,
        commands: &[
            "mor",
            "post",
            "megrasp",
            "postlist",
            "postmodrules",
            "posttrain",
        ],
        coverage: CLAN_LEGACY_COVERAGE,
        note: "preserved legacy names that remain intentionally present but unimplemented",
    },
];

/// Look up the reviewed surface-group metadata for one published family.
pub fn surface_group(family: SurfaceFamily) -> &'static SurfaceGroup {
    SURFACE_GROUPS
        .iter()
        .find(|group| group.family == family)
        .expect("surface family should exist in the shared command manifest")
}
