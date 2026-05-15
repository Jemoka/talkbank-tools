//! Command-line interface handlers for talkbank-tools.
//!
//! This module contains CLI definitions and command dispatch.
//! Individual command implementations are in the `commands` module.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod args;
mod logging;
mod run;

pub use args::{
    AlignmentTier, CacheCommands, ClanCommands, ClanOutputFormat, Cli, Commands,
    CommonAnalysisArgs, DebugCommands, LogFormat, OutputFormat, ParserBackend,
    apply_clan_help_grouping,
};
pub use logging::init_tracing;
pub use run::run;
