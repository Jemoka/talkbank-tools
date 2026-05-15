//! Compatibility shims for CLAN-adjacent commands in the CLI.
//!
//! Not every command in this file belongs to the new shared analysis-service
//! boundary. CHECK and transform aliases still have their own execution paths,
//! while compatibility-style analysis entrypoints such as `gemfreq` adapt into
//! the library-owned typed analysis-command surface before delegating
//! execution.

use crate::cli::ClanCommands;
use clan_core::commands::check::{CheckConfig, list_all_errors, run_check};
use clan_core::framework::{CommandOutput, DiscoveredChatFiles};
use clan_core::service_types::{AnalysisCommandName, AnalysisOptions};

use super::helpers::{
    exit_with_error, read_file_or_exit, run_analysis_and_print, run_normalize_alias,
};

pub(super) fn dispatch(command: ClanCommands) -> Result<(), ClanCommands> {
    match command {
        ClanCommands::Check {
            paths,
            bullets,
            include_errors,
            exclude_errors,
            list_errors,
            check_target,
            check_id,
            check_unused,
            check_ud,
        } => {
            if list_errors {
                print!("{}", list_all_errors());
                return Ok(());
            }

            if paths.is_empty() {
                exit_with_error("Error: path is required unless --list-errors is used".to_owned());
            }

            let discovered = DiscoveredChatFiles::from_paths(&paths);
            for skipped in discovered.skipped_paths() {
                eprintln!(
                    "Warning: {:?} is not a file or directory, skipping",
                    skipped
                );
            }
            let files = discovered.into_files();
            if files.is_empty() {
                exit_with_error("Error: no .cha files found".to_owned());
            }

            let config = CheckConfig {
                bullets,
                include_errors: include_errors.into_iter().collect(),
                exclude_errors: exclude_errors.into_iter().collect(),
                list_errors: false,
                check_target_child: check_target,
                check_missing_id: check_id.unwrap_or(true),
                check_unused_speakers: check_unused,
                check_ud_features: check_ud,
            };

            let mut any_errors = false;
            let mut all_output = String::new();
            for file_path in &files {
                let content = read_file_or_exit(file_path);
                let result = run_check(file_path, &content, &config);
                if result.has_errors || !result.errors.is_empty() {
                    any_errors = true;
                }
                let text = result.render_text();
                if !text.is_empty() {
                    all_output.push_str(&text);
                }
            }

            if !all_output.is_empty() {
                print!("{all_output}");
            }
            if any_errors {
                eprintln!("Warning: Please repeat CHECK until no error messages are reported!");
                std::process::exit(1);
            } else {
                eprintln!("ALL FILES CHECKED OUT OK!");
            }
        }
        ClanCommands::Fixit { path, output } => run_normalize_alias(&path, output.as_deref()),
        ClanCommands::Indent { path, output } => {
            clan_core::transforms::indent::run_indent(&path, output.as_deref())
                .unwrap_or_else(|e| exit_with_error(format!("Error: {e}")));
        }
        ClanCommands::Longtier { path, output } => {
            clan_core::transforms::longtier::run_longtier(&path, output.as_deref())
                .unwrap_or_else(|e| exit_with_error(format!("Error: {e}")));
        }
        ClanCommands::Gemfreq { path, mor, common } => {
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
        other => return Err(other),
    }
    Ok(())
}
