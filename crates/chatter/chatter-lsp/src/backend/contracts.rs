//! Public JSON contracts shared by editor integrations and the LSP backend.
//!
//! These types describe the stable JSON payloads that cross the extension/server
//! boundary. They are intentionally transport-shaped, derive `JsonSchema`, and
//! are reused directly by the backend decoder so schema generation and runtime
//! decoding stay in sync.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use clan_core::database::{DatabaseFilter, Gender};
use clan_core::service_types::AnalysisCommandName;

/// Demographic filter payload used by `talkbank/analyze`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct AnalysisDatabaseFilterPayload {
    /// Optional language code such as `eng`.
    pub language: Option<String>,
    /// Optional demographic group such as `TD`.
    pub group: Option<String>,
    /// Optional gender selector.
    pub gender: Option<Gender>,
    /// Optional lower age bound in months.
    pub age_from_months: Option<u32>,
    /// Optional upper age bound in months.
    pub age_to_months: Option<u32>,
    /// Optional speaker-code subset.
    pub speaker_codes: Vec<String>,
}

impl From<AnalysisDatabaseFilterPayload> for DatabaseFilter {
    fn from(value: AnalysisDatabaseFilterPayload) -> Self {
        Self {
            language: value.language,
            group: value.group,
            gender: value.gender,
            age_from_months: value.age_from_months,
            age_to_months: value.age_to_months,
            speaker_codes: value.speaker_codes,
        }
    }
}

/// Command-specific option payload for `talkbank/analyze`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(default)]
pub struct AnalysisOptionsPayload {
    /// Whether to run `freq` against `%mor`.
    pub mor: bool,
    /// Whether `mlu` should count words instead of morphemes.
    pub words: bool,
    /// Whether `wdsize` should read from the main tier.
    #[serde(rename = "mainTier")]
    pub main_tier: bool,
    /// Result limit for commands such as `maxwd`.
    pub limit: Option<usize>,
    /// Keyword list for `kwal` and `keymap`.
    pub keywords: Vec<String>,
    /// Search expressions for `combo`.
    pub search: Vec<String>,
    /// Maximum code depth for `codes`.
    #[serde(rename = "maxDepth")]
    pub max_depth: Option<usize>,
    /// Tier selector used by several analysis commands.
    pub tier: Option<String>,
    /// Frequency threshold for `corelex`.
    pub threshold: Option<u64>,
    /// Shared max-utterance limit used by several analyzers.
    #[serde(rename = "maxUtterances")]
    pub max_utterances: Option<usize>,
    /// Optional normative database path.
    #[serde(rename = "databasePath")]
    pub database_path: Option<PathBuf>,
    /// Optional normative database demographic filter.
    #[serde(rename = "databaseFilter")]
    pub database_filter: Option<AnalysisDatabaseFilterPayload>,
    /// Whether `flucalc` should use syllable mode.
    #[serde(rename = "syllableMode")]
    pub syllable_mode: bool,
    /// KidEval DSS utterance cap.
    #[serde(rename = "dssMaxUtterances")]
    pub dss_max_utterances: Option<usize>,
    /// KidEval IPSyn utterance cap.
    #[serde(rename = "ipsynMaxUtterances")]
    pub ipsyn_max_utterances: Option<usize>,
    /// Mortable script path.
    #[serde(rename = "scriptPath")]
    pub script_path: Option<PathBuf>,
    /// Secondary file URI for `rely`.
    #[serde(rename = "secondFile")]
    pub second_file: Option<String>,
    /// Template file path for `script`.
    #[serde(rename = "templatePath")]
    pub template_path: Option<PathBuf>,
    /// Minimum utterance count for `sugar`.
    #[serde(rename = "minUtterances")]
    pub min_utterances: Option<usize>,
    /// First tier for `trnfix`.
    pub tier1: Option<String>,
    /// Second tier for `trnfix`.
    pub tier2: Option<String>,
    /// Whether `uniq` should sort by frequency.
    #[serde(rename = "sortByFrequency")]
    pub sort_by_frequency: bool,
}

/// Canonical object payload for `talkbank/analyze`.
///
/// The LSP wire protocol still carries this object inside the
/// `workspace/executeCommand` argument vector, but the logical payload is a
/// single object rather than a positional tuple. This shape is the source of
/// truth for schema generation and editor/server type sharing.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct AnalyzeCommandPayload {
    /// CLAN analysis command identifier.
    #[serde(rename = "commandName")]
    pub command_name: AnalysisCommandName,
    /// File or directory URI to analyze.
    #[serde(rename = "targetUri")]
    pub target_uri: String,
    /// Command-specific options payload.
    #[serde(default)]
    pub options: AnalysisOptionsPayload,
}
