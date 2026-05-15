//! CLAN analysis-command adapters for the `chatter` CLI.
//!
//! Each match arm here now performs only CLI-facing adaptation: convert parsed
//! clap arguments into a typed
//! [`clan_core::service::AnalysisCommandName`] plus
//! [`clan_core::service::AnalysisOptions`], then delegate defaults,
//! validation, and execution to the library-owned builder and service
//! boundaries. Keep command construction and shared output policy inside
//! `clan-core`; keep CLI argument mapping and terminal UX here.

use crate::cli::ClanCommands;
use clan_core::commands::chains::ChainsConfig;
use clan_core::commands::codes::CodesConfig;
use clan_core::commands::corelex::CorelexConfig;
use clan_core::commands::dss::DssConfig;
use clan_core::commands::ipsyn::IpsynConfig;
use clan_core::commands::keymap::KeymapConfig;
use clan_core::commands::maxwd::MaxwdConfig;
use clan_core::commands::rely::RelyConfig;
use clan_core::commands::trnfix::TrnfixConfig;
use clan_core::service_types::{AnalysisCommandName, AnalysisOptions};

use super::helpers::{run_analysis_and_print, run_paired_analysis_and_print};

pub(super) fn dispatch(command: ClanCommands) -> Result<(), ClanCommands> {
    match command {
        ClanCommands::Freq {
            path, mor, common, ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Freq,
                AnalysisOptions {
                    mor,
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Mlu {
            path,
            words,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Mlu,
                AnalysisOptions {
                    words,
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Mlt { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Mlt,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Wdlen { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Wdlen,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Wdsize {
            path,
            main_tier,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Wdsize,
                AnalysisOptions {
                    main_tier,
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Maxwd {
            path,
            limit,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Maxwd,
                AnalysisOptions {
                    limit: option_if_not_default(limit, MaxwdConfig::default().limit),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Freqpos { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Freqpos,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Timedur { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Timedur,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Kwal {
            path,
            keyword,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Kwal,
                AnalysisOptions {
                    keywords: keyword
                        .into_iter()
                        .map(clan_core::framework::KeywordPattern::from)
                        .collect(),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Gemlist { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Gemlist,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Combo {
            path,
            search,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Combo,
                AnalysisOptions {
                    search,
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Cooccur { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Cooccur,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Dist { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Dist,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Chip { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Chip,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Phonfreq { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Phonfreq,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Modrep { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Modrep,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Vocd { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Vocd,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Uniq {
            path, sort, common, ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Uniq,
                AnalysisOptions {
                    sort_by_frequency: sort,
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Codes {
            path,
            max_depth,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Codes,
                AnalysisOptions {
                    max_depth: option_if_not_default(max_depth, CodesConfig::default().max_depth),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Trnfix {
            path,
            tier1,
            tier2,
            common,
            ..
        } => {
            let default = TrnfixConfig::default();
            run_analysis_and_print(
                AnalysisCommandName::Trnfix,
                AnalysisOptions {
                    tier1: option_if_not_default(tier1, default.tier1),
                    tier2: option_if_not_default(tier2, default.tier2),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Sugar { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Sugar,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Mortable {
            path,
            script,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Mortable,
                AnalysisOptions {
                    script_path: Some(script),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Chains {
            path, tier, common, ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Chains,
                AnalysisOptions {
                    tier: option_if_not_default(tier, ChainsConfig::default().tier),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Complexity { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Complexity,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Corelex {
            path,
            threshold,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Corelex,
                AnalysisOptions {
                    threshold: option_if_not_default(
                        threshold,
                        CorelexConfig::default().min_frequency,
                    ),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Keymap {
            path,
            keyword,
            tier,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Keymap,
                AnalysisOptions {
                    keywords: keyword
                        .into_iter()
                        .map(clan_core::framework::KeywordPattern::from)
                        .collect(),
                    tier: option_if_not_default(tier, KeymapConfig::default().tier),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Script {
            path,
            template,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Script,
                AnalysisOptions {
                    template_path: Some(template),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Rely {
            file1,
            file2,
            tier,
            format,
        } => {
            run_paired_analysis_and_print(
                AnalysisCommandName::Rely,
                AnalysisOptions {
                    second_file: Some(file2),
                    tier: option_if_not_default(tier, RelyConfig::default().tier),
                    ..AnalysisOptions::default()
                },
                &file1,
                format,
            );
        }
        ClanCommands::Flucalc { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Flucalc,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Dss {
            path,
            rules,
            max_utterances,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Dss,
                AnalysisOptions {
                    rules_path: rules,
                    max_utterances: option_if_not_default(
                        max_utterances,
                        DssConfig::default().max_utterances,
                    ),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Ipsyn {
            path,
            rules,
            max_utterances,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Ipsyn,
                AnalysisOptions {
                    rules_path: rules,
                    max_utterances: option_if_not_default(
                        max_utterances,
                        IpsynConfig::default().max_utterances,
                    ),
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::Eval { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::Eval,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        ClanCommands::Kideval {
            path,
            dss_rules,
            ipsyn_rules,
            common,
            ..
        } => {
            run_analysis_and_print(
                AnalysisCommandName::Kideval,
                AnalysisOptions {
                    dss_rules_path: dss_rules,
                    ipsyn_rules_path: ipsyn_rules,
                    ..AnalysisOptions::default()
                },
                &path,
                &common,
            );
        }
        ClanCommands::EvalD { path, common, .. } => run_analysis_and_print(
            AnalysisCommandName::EvalDialect,
            AnalysisOptions::default(),
            &path,
            &common,
        ),
        other => return Err(other),
    }
    Ok(())
}

fn option_if_not_default<T: PartialEq>(value: T, default: T) -> Option<T> {
    if value == default { None } else { Some(value) }
}
