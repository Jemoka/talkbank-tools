//! Dispatch CLAN analysis, transform, converter, and compatibility commands.

mod analysis;
mod compatibility;
mod converters;
mod helpers;
mod transforms;

use crate::cli::ClanCommands;

/// Dispatch a `chatter clan` subcommand.
pub fn run_clan(command: ClanCommands) {
    let command = match analysis::dispatch(command) {
        Ok(()) => return,
        Err(command) => command,
    };
    let command = match transforms::dispatch(command) {
        Ok(()) => return,
        Err(command) => command,
    };
    let command = match converters::dispatch(command) {
        Ok(()) => return,
        Err(command) => command,
    };
    if let Err(command) = compatibility::dispatch(command) {
        match command {
            ClanCommands::Mor {} => {
                eprintln!(
                    "Error: {}",
                    clan_core::commands::mor::run_mor().unwrap_err()
                );
            }
            ClanCommands::Post {} => {
                eprintln!(
                    "Error: {}",
                    clan_core::commands::post::run_post().unwrap_err()
                );
            }
            ClanCommands::Megrasp {} => {
                eprintln!(
                    "Error: {}",
                    clan_core::commands::megrasp::run_megrasp().unwrap_err()
                );
            }
            ClanCommands::Postlist {} => {
                eprintln!(
                    "Error: {}",
                    clan_core::commands::postlist::run_postlist().unwrap_err()
                );
            }
            ClanCommands::Postmodrules {} => {
                eprintln!(
                    "Error: {}",
                    clan_core::commands::postmodrules::run_postmodrules().unwrap_err()
                );
            }
            ClanCommands::Posttrain {} => {
                eprintln!(
                    "Error: {}",
                    clan_core::commands::posttrain::run_posttrain().unwrap_err()
                );
            }
            // Routing invariant: this branch only runs for clan
            // commands routed by the dispatcher; non-clan variants
            // are filtered upstream.
            #[allow(clippy::unreachable)]
            _ => unreachable!("unhandled clan command family"),
        }
        std::process::exit(1);
    }
}
