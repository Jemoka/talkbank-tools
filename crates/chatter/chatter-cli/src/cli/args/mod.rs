//! CLI argument definitions for `talkbank` commands and global flags.
//!
//! This module is split by concern:
//! - `core` for top-level CLI and non-CLAN commands
//! - `clan_common` for shared CLAN argument groups and formats
//! - `clan_commands` for CLAN subcommands

mod clan_commands;
mod clan_common;
mod core;

pub use clan_commands::{ClanCommands, apply_clan_help_grouping};
pub use clan_common::{ClanOutputFormat, CommonAnalysisArgs};
pub use core::{
    AlignmentTier, CacheCommands, Cli, Commands, DebugCommands, LogFormat, OutputFormat,
    ParserBackend,
};
