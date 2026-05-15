//! High-level library service for executing CLAN analyses.
//!
//! `clan-core` already exposes low-level command traits and runner plumbing in
//! [`crate::framework`], but editor integrations should not need to import every
//! individual command type just to execute a named analysis. This module keeps the
//! higher-level analysis execution boundary inside the library so CLI and LSP
//! wrappers can stay focused on adapting outer request shapes. That boundary
//! now also includes a typed analysis-command identifier so outer adapters do
//! not need to route analysis work through raw command-name strings.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::commands::chains::ChainsConfig;
use crate::commands::codes::CodesConfig;
use crate::commands::combo::ComboConfig;
use crate::commands::corelex::CorelexConfig;
use crate::commands::dss::DssConfig;
use crate::commands::eval::EvalConfig;
use crate::commands::flucalc::FlucalcConfig;
use crate::commands::freq::FreqConfig;
use crate::commands::ipsyn::IpsynConfig;
use crate::commands::keymap::KeymapConfig;
use crate::commands::kideval::KidevalConfig;
use crate::commands::kwal::KwalConfig;
use crate::commands::maxwd::MaxwdConfig;
use crate::commands::mlu::MluConfig;
use crate::commands::mortable::MortableConfig;
use crate::commands::rely::RelyConfig;
use crate::commands::script::ScriptConfig;
use crate::commands::sugar::SugarConfig;
use crate::commands::trnfix::TrnfixConfig;
use crate::commands::uniq::UniqConfig;
use crate::commands::wdsize::WdsizeConfig;
use crate::framework::{RunnerError, TransformError};

/// Typed identifier for one supported CLAN analysis command.
///
/// Outer adapters such as the CLI and LSP should parse raw command-name
/// strings into this enum at their boundary, then pass the typed identifier
/// through the shared builder and service layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisCommandName {
    /// `freq`
    Freq,
    /// `mlu`
    Mlu,
    /// `mlt`
    Mlt,
    /// `wdlen`
    Wdlen,
    /// `wdsize`
    Wdsize,
    /// `maxwd`
    Maxwd,
    /// `freqpos`
    Freqpos,
    /// `timedur`
    Timedur,
    /// `kwal`
    Kwal,
    /// `gemlist`
    Gemlist,
    /// `combo`
    Combo,
    /// `cooccur`
    Cooccur,
    /// `dist`
    Dist,
    /// `chip`
    Chip,
    /// `phonfreq`
    Phonfreq,
    /// `modrep`
    Modrep,
    /// `vocd`
    Vocd,
    /// `codes`
    Codes,
    /// `chains`
    Chains,
    /// `complexity`
    Complexity,
    /// `corelex`
    Corelex,
    /// `dss`
    Dss,
    /// `eval`
    Eval,
    /// `eval-d`
    #[serde(rename = "eval-d")]
    EvalDialect,
    /// `flucalc`
    Flucalc,
    /// `ipsyn`
    Ipsyn,
    /// `keymap`
    Keymap,
    /// `kideval`
    Kideval,
    /// `mortable`
    Mortable,
    /// `rely`
    Rely,
    /// `script`
    Script,
    /// `sugar`
    Sugar,
    /// `trnfix`
    Trnfix,
    /// `uniq`
    Uniq,
}

impl AnalysisCommandName {
    /// Return the stable wire-format name used by CLI and editor adapters.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Freq => "freq",
            Self::Mlu => "mlu",
            Self::Mlt => "mlt",
            Self::Wdlen => "wdlen",
            Self::Wdsize => "wdsize",
            Self::Maxwd => "maxwd",
            Self::Freqpos => "freqpos",
            Self::Timedur => "timedur",
            Self::Kwal => "kwal",
            Self::Gemlist => "gemlist",
            Self::Combo => "combo",
            Self::Cooccur => "cooccur",
            Self::Dist => "dist",
            Self::Chip => "chip",
            Self::Phonfreq => "phonfreq",
            Self::Modrep => "modrep",
            Self::Vocd => "vocd",
            Self::Codes => "codes",
            Self::Chains => "chains",
            Self::Complexity => "complexity",
            Self::Corelex => "corelex",
            Self::Dss => "dss",
            Self::Eval => "eval",
            Self::EvalDialect => "eval-d",
            Self::Flucalc => "flucalc",
            Self::Ipsyn => "ipsyn",
            Self::Keymap => "keymap",
            Self::Kideval => "kideval",
            Self::Mortable => "mortable",
            Self::Rely => "rely",
            Self::Script => "script",
            Self::Sugar => "sugar",
            Self::Trnfix => "trnfix",
            Self::Uniq => "uniq",
        }
    }
}

impl fmt::Display for AnalysisCommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when a raw outer-layer command name is not recognized.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("Unknown analysis command: {command_name}")]
pub struct ParseAnalysisCommandNameError {
    command_name: String,
}

impl FromStr for AnalysisCommandName {
    type Err = ParseAnalysisCommandNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "freq" => Ok(Self::Freq),
            "mlu" => Ok(Self::Mlu),
            "mlt" => Ok(Self::Mlt),
            "wdlen" => Ok(Self::Wdlen),
            "wdsize" => Ok(Self::Wdsize),
            "maxwd" => Ok(Self::Maxwd),
            "freqpos" => Ok(Self::Freqpos),
            "timedur" => Ok(Self::Timedur),
            "kwal" => Ok(Self::Kwal),
            "gemlist" => Ok(Self::Gemlist),
            "combo" => Ok(Self::Combo),
            "cooccur" => Ok(Self::Cooccur),
            "dist" => Ok(Self::Dist),
            "chip" => Ok(Self::Chip),
            "phonfreq" => Ok(Self::Phonfreq),
            "modrep" => Ok(Self::Modrep),
            "vocd" => Ok(Self::Vocd),
            "codes" => Ok(Self::Codes),
            "chains" => Ok(Self::Chains),
            "complexity" => Ok(Self::Complexity),
            "corelex" => Ok(Self::Corelex),
            "dss" => Ok(Self::Dss),
            "eval" => Ok(Self::Eval),
            "eval-d" => Ok(Self::EvalDialect),
            "flucalc" => Ok(Self::Flucalc),
            "ipsyn" => Ok(Self::Ipsyn),
            "keymap" => Ok(Self::Keymap),
            "kideval" => Ok(Self::Kideval),
            "mortable" => Ok(Self::Mortable),
            "rely" => Ok(Self::Rely),
            "script" => Ok(Self::Script),
            "sugar" => Ok(Self::Sugar),
            "trnfix" => Ok(Self::Trnfix),
            "uniq" => Ok(Self::Uniq),
            _ => Err(ParseAnalysisCommandNameError {
                command_name: value.to_owned(),
            }),
        }
    }
}

impl<'de> Deserialize<'de> for AnalysisCommandName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// Typed, library-owned request for a CLAN analysis command.
///
/// This enum is the stable integration boundary for higher-level consumers such
/// as the CLI and LSP. It keeps command-specific configuration typed without
/// forcing those outer layers to import and execute each command type directly.
#[derive(Debug)]
pub enum AnalysisRequest {
    /// `freq`
    Freq(FreqConfig),
    /// `mlu`
    Mlu(MluConfig),
    /// `mlt`
    Mlt,
    /// `wdlen`
    Wdlen,
    /// `wdsize`
    Wdsize(WdsizeConfig),
    /// `maxwd`
    Maxwd(MaxwdConfig),
    /// `freqpos`
    Freqpos,
    /// `timedur`
    Timedur,
    /// `kwal`
    Kwal(KwalConfig),
    /// `gemlist`
    Gemlist,
    /// `combo`
    Combo(ComboConfig),
    /// `cooccur`
    Cooccur,
    /// `dist`
    Dist,
    /// `chip`
    Chip,
    /// `phonfreq`
    Phonfreq,
    /// `modrep`
    Modrep,
    /// `vocd`
    Vocd,
    /// `codes`
    Codes(CodesConfig),
    /// `chains`
    Chains(ChainsConfig),
    /// `complexity`
    Complexity,
    /// `corelex`
    Corelex(CorelexConfig),
    /// `dss`
    Dss(DssConfig),
    /// `eval`
    Eval(EvalConfig),
    /// `flucalc`
    Flucalc(FlucalcConfig),
    /// `ipsyn`
    Ipsyn(IpsynConfig),
    /// `keymap`
    Keymap(KeymapConfig),
    /// `kideval`
    Kideval(KidevalConfig),
    /// `mortable`
    Mortable(MortableConfig),
    /// `script`
    Script(ScriptConfig),
    /// `sugar`
    Sugar(SugarConfig),
    /// `trnfix`
    Trnfix(TrnfixConfig),
    /// `uniq`
    Uniq(UniqConfig),
}

/// Raw analysis options supplied by outer adapters before defaults are applied.
#[derive(Debug, Clone, Default)]
pub struct AnalysisOptions {
    /// Whether to run `freq` against `%mor`.
    pub mor: bool,
    /// Whether `mlu` should count words instead of morphemes.
    pub words: bool,
    /// Whether `wdsize` should read from the main tier.
    pub main_tier: bool,
    /// Result limit for commands such as `maxwd`.
    pub limit: Option<crate::framework::WordLimit>,
    /// Keyword list for `kwal` and `keymap`.
    pub keywords: Vec<crate::framework::KeywordPattern>,
    /// Search expressions for `combo`.
    pub search: Vec<String>,
    /// Maximum code depth for `codes`.
    pub max_depth: Option<crate::framework::CodeDepth>,
    /// Tier selector used by several analysis commands.
    pub tier: Option<crate::framework::TierKind>,
    /// Frequency threshold for `corelex`.
    pub threshold: Option<crate::framework::FrequencyThreshold>,
    /// Shared max-utterance limit used by several analyzers.
    pub max_utterances: Option<crate::framework::UtteranceLimit>,
    /// Optional normative database path.
    pub database_path: Option<PathBuf>,
    /// Optional normative database demographic filter.
    pub database_filter: Option<crate::database::DatabaseFilter>,
    /// Whether `flucalc` should use syllable mode.
    pub syllable_mode: bool,
    /// KidEval DSS utterance cap.
    pub dss_max_utterances: Option<crate::framework::UtteranceLimit>,
    /// KidEval IPSyn utterance cap.
    pub ipsyn_max_utterances: Option<crate::framework::UtteranceLimit>,
    /// Shared rules path for analyzers such as `dss` and `ipsyn`.
    pub rules_path: Option<PathBuf>,
    /// Kideval DSS rules path.
    pub dss_rules_path: Option<PathBuf>,
    /// Kideval IPSyn rules path.
    pub ipsyn_rules_path: Option<PathBuf>,
    /// Mortable script path.
    pub script_path: Option<PathBuf>,
    /// Secondary file for `rely`.
    pub second_file: Option<PathBuf>,
    /// Template file path for `script`.
    pub template_path: Option<PathBuf>,
    /// Minimum utterance count for `sugar`.
    pub min_utterances: Option<crate::framework::UtteranceLimit>,
    /// First tier for `trnfix`.
    pub tier1: Option<crate::framework::TierKind>,
    /// Second tier for `trnfix`.
    pub tier2: Option<crate::framework::TierKind>,
    /// Whether `uniq` should sort by frequency.
    pub sort_by_frequency: bool,
}

/// Built analysis plan after library-owned defaults and validation are applied.
#[derive(Debug)]
pub enum AnalysisPlan {
    /// Standard request executed through [`AnalysisService`].
    Service(AnalysisRequest),
    /// `rely` still uses an explicit two-file execution path.
    Rely(RelyRequest),
}

/// Typed request for `rely`.
#[derive(Debug)]
pub struct RelyRequest {
    /// Parsed secondary file path.
    pub secondary_file: PathBuf,
    /// Validated RELY configuration.
    pub config: RelyConfig,
}

/// Builder that turns raw outer-layer options into typed library requests.
pub struct AnalysisRequestBuilder {
    command_name: AnalysisCommandName,
    options: AnalysisOptions,
}

impl AnalysisRequestBuilder {
    /// Create a builder for one named CLAN analysis command.
    pub fn new(command_name: AnalysisCommandName, options: AnalysisOptions) -> Self {
        Self {
            command_name,
            options,
        }
    }

    /// Validate, apply library defaults, and build the analysis plan.
    pub fn build(self) -> Result<AnalysisPlan, AnalysisServiceError> {
        let command_name = self.command_name;
        let options = self.options;

        match command_name {
            AnalysisCommandName::Freq => {
                Ok(AnalysisPlan::Service(AnalysisRequest::Freq(FreqConfig {
                    use_mor: options.mor,
                })))
            }
            AnalysisCommandName::Mlu => {
                Ok(AnalysisPlan::Service(AnalysisRequest::Mlu(MluConfig {
                    words_only: options.words,
                })))
            }
            AnalysisCommandName::Mlt => Ok(AnalysisPlan::Service(AnalysisRequest::Mlt)),
            AnalysisCommandName::Wdlen => Ok(AnalysisPlan::Service(AnalysisRequest::Wdlen)),
            AnalysisCommandName::Wdsize => Ok(AnalysisPlan::Service(AnalysisRequest::Wdsize(
                WdsizeConfig {
                    use_main_tier: options.main_tier,
                },
            ))),
            AnalysisCommandName::Maxwd => {
                let default = MaxwdConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Maxwd(MaxwdConfig {
                    limit: options.limit.unwrap_or(default.limit),
                })))
            }
            AnalysisCommandName::Freqpos => Ok(AnalysisPlan::Service(AnalysisRequest::Freqpos)),
            AnalysisCommandName::Timedur => Ok(AnalysisPlan::Service(AnalysisRequest::Timedur)),
            AnalysisCommandName::Kwal => Ok(AnalysisPlan::Service(AnalysisRequest::kwal(
                options.keywords,
            )?)),
            AnalysisCommandName::Gemlist => Ok(AnalysisPlan::Service(AnalysisRequest::Gemlist)),
            AnalysisCommandName::Combo => {
                let search: Vec<crate::commands::combo::SearchExpr> = options
                    .search
                    .iter()
                    .map(|expr| crate::commands::combo::SearchExpr::parse(expr))
                    .collect();
                if search.is_empty() {
                    return Err(AnalysisServiceError::InvalidRequest(
                        "combo requires at least one search expression".to_owned(),
                    ));
                }
                Ok(AnalysisPlan::Service(AnalysisRequest::Combo(ComboConfig {
                    search,
                })))
            }
            AnalysisCommandName::Cooccur => Ok(AnalysisPlan::Service(AnalysisRequest::Cooccur)),
            AnalysisCommandName::Dist => Ok(AnalysisPlan::Service(AnalysisRequest::Dist)),
            AnalysisCommandName::Chip => Ok(AnalysisPlan::Service(AnalysisRequest::Chip)),
            AnalysisCommandName::Phonfreq => Ok(AnalysisPlan::Service(AnalysisRequest::Phonfreq)),
            AnalysisCommandName::Modrep => Ok(AnalysisPlan::Service(AnalysisRequest::Modrep)),
            AnalysisCommandName::Vocd => Ok(AnalysisPlan::Service(AnalysisRequest::Vocd)),
            AnalysisCommandName::Codes => {
                let default = CodesConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Codes(CodesConfig {
                    max_depth: options.max_depth.unwrap_or(default.max_depth),
                })))
            }
            AnalysisCommandName::Chains => {
                let default = ChainsConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Chains(
                    ChainsConfig {
                        tier: options.tier.unwrap_or(default.tier),
                    },
                )))
            }
            AnalysisCommandName::Complexity => {
                Ok(AnalysisPlan::Service(AnalysisRequest::Complexity))
            }
            AnalysisCommandName::Corelex => {
                let default = CorelexConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Corelex(
                    CorelexConfig {
                        min_frequency: options.threshold.unwrap_or(default.min_frequency),
                    },
                )))
            }
            AnalysisCommandName::Dss => {
                let default = DssConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Dss(DssConfig {
                    rules_path: options.rules_path,
                    max_utterances: options.max_utterances.unwrap_or(default.max_utterances),
                })))
            }
            AnalysisCommandName::Eval => {
                let default = EvalConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Eval(EvalConfig {
                    database_path: options.database_path,
                    database_filter: options.database_filter,
                    ..default
                })))
            }
            AnalysisCommandName::EvalDialect => {
                Ok(AnalysisPlan::Service(AnalysisRequest::Eval(EvalConfig {
                    database_path: options.database_path,
                    database_filter: options.database_filter,
                    variant: crate::commands::eval::EvalVariant::Dialect,
                })))
            }
            AnalysisCommandName::Flucalc => Ok(AnalysisPlan::Service(AnalysisRequest::Flucalc(
                FlucalcConfig {
                    syllable_mode: options.syllable_mode,
                },
            ))),
            AnalysisCommandName::Ipsyn => {
                let default = IpsynConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Ipsyn(IpsynConfig {
                    rules_path: options.rules_path,
                    max_utterances: options.max_utterances.unwrap_or(default.max_utterances),
                })))
            }
            AnalysisCommandName::Keymap => {
                let tier = options.tier.unwrap_or_else(|| KeymapConfig::default().tier);
                Ok(AnalysisPlan::Service(AnalysisRequest::keymap(
                    options.keywords,
                    tier,
                )?))
            }
            AnalysisCommandName::Kideval => {
                let default = KidevalConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Kideval(
                    KidevalConfig {
                        dss_rules_path: options.dss_rules_path,
                        ipsyn_rules_path: options.ipsyn_rules_path,
                        dss_max_utterances: options
                            .dss_max_utterances
                            .unwrap_or(default.dss_max_utterances),
                        ipsyn_max_utterances: options
                            .ipsyn_max_utterances
                            .unwrap_or(default.ipsyn_max_utterances),
                        database_path: options.database_path,
                        database_filter: options.database_filter,
                    },
                )))
            }
            AnalysisCommandName::Mortable => {
                let script_path = options.script_path.ok_or_else(|| {
                    AnalysisServiceError::InvalidRequest(
                        "mortable requires a scriptPath option".to_owned(),
                    )
                })?;
                Ok(AnalysisPlan::Service(AnalysisRequest::Mortable(
                    MortableConfig { script_path },
                )))
            }
            AnalysisCommandName::Rely => {
                let secondary_file = options.second_file.ok_or_else(|| {
                    AnalysisServiceError::InvalidRequest(
                        "rely requires a secondFile option".to_owned(),
                    )
                })?;
                let tier = options.tier.unwrap_or_else(|| RelyConfig::default().tier);
                Ok(AnalysisPlan::Rely(RelyRequest {
                    secondary_file,
                    config: RelyConfig { tier },
                }))
            }
            AnalysisCommandName::Script => {
                let template_path = options.template_path.ok_or_else(|| {
                    AnalysisServiceError::InvalidRequest(
                        "script requires a templatePath option".to_owned(),
                    )
                })?;
                Ok(AnalysisPlan::Service(AnalysisRequest::Script(
                    ScriptConfig { template_path },
                )))
            }
            AnalysisCommandName::Sugar => {
                let default = SugarConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Sugar(SugarConfig {
                    min_utterances: options.min_utterances.unwrap_or(default.min_utterances),
                })))
            }
            AnalysisCommandName::Trnfix => {
                let default = TrnfixConfig::default();
                Ok(AnalysisPlan::Service(AnalysisRequest::Trnfix(
                    TrnfixConfig {
                        tier1: options.tier1.unwrap_or(default.tier1),
                        tier2: options.tier2.unwrap_or(default.tier2),
                    },
                )))
            }
            AnalysisCommandName::Uniq => {
                Ok(AnalysisPlan::Service(AnalysisRequest::Uniq(UniqConfig {
                    sort_by_frequency: options.sort_by_frequency,
                })))
            }
        }
    }
}

impl AnalysisRequest {
    /// Validate and construct a `kwal` request.
    pub fn kwal(
        keywords: Vec<crate::framework::KeywordPattern>,
    ) -> Result<Self, AnalysisServiceError> {
        if keywords.is_empty() {
            return Err(AnalysisServiceError::InvalidRequest(
                "kwal requires at least one keyword".to_owned(),
            ));
        }

        Ok(Self::Kwal(KwalConfig { keywords }))
    }

    /// Validate and construct a `keymap` request.
    pub fn keymap(
        keywords: Vec<crate::framework::KeywordPattern>,
        tier: crate::framework::TierKind,
    ) -> Result<Self, AnalysisServiceError> {
        if keywords.is_empty() {
            return Err(AnalysisServiceError::InvalidRequest(
                "keymap requires at least one keyword".to_owned(),
            ));
        }

        Ok(Self::Keymap(KeymapConfig { keywords, tier }))
    }
}

/// Error from the high-level analysis service boundary.
#[derive(Debug, Error)]
pub enum AnalysisServiceError {
    /// Invalid request shape or unsupported option combination.
    #[error("{0}")]
    InvalidRequest(String),
    /// Underlying transform failure used by non-runner commands such as `rely`.
    #[error(transparent)]
    Transform(#[from] TransformError),
    /// Underlying runner failure.
    #[error(transparent)]
    Runner(#[from] RunnerError),
}
