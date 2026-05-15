//! Feature-oriented CLI command routing.
//!
//! This keeps `cli/run.rs` as a small composition root and lets each command family own
//! its own dispatch rules and shared runtime context.

use crate::cli::Commands;
use crate::ui::Theme;

use super::cache::run_cache_command;
use super::validate::{
    ValidateCommandExecution, ValidateCommandOptions, ValidateCommandPresentation,
    ValidateCommandRules, run_validate_command,
};
use super::{
    AlignmentValidationMode, CacheRefreshMode, RoundtripValidationMode, ValidationInterface,
    chat_to_json, chat_to_xml, clean_file, create_new_file, json_to_chat, lint_files,
    normalize_chat, run_clan, run_schema, show_alignment, watch_files,
};

/// Runtime context shared across top-level CLI command families.
#[derive(Clone)]
pub struct CommandContext {
    /// Whether the current invocation should prefer the interactive TUI surface.
    pub should_use_tui: bool,
    /// Loaded TUI color theme.
    pub theme: Theme,
}

/// One feature family that owns a subset of top-level CLI commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandFamily {
    Validation,
    Utility,
    Cache,
    Clan,
    Debug,
}

trait CommandFamilyService {
    fn dispatch(&self, command: Commands, context: &CommandContext);
}

struct CommandServices {
    validation: ValidationCommandService,
    utility: UtilityCommandService,
    cache: CacheCommandService,
    clan: ClanCommandService,
    debug: DebugCommandService,
}

impl CommandServices {
    const fn new() -> Self {
        Self {
            validation: ValidationCommandService,
            utility: UtilityCommandService,
            cache: CacheCommandService,
            clan: ClanCommandService,
            debug: DebugCommandService,
        }
    }

    fn dispatch(&self, command: Commands, context: &CommandContext) {
        match command.family() {
            CommandFamily::Validation => self.validation.dispatch(command, context),
            CommandFamily::Utility => self.utility.dispatch(command, context),
            CommandFamily::Cache => self.cache.dispatch(command, context),
            CommandFamily::Clan => self.clan.dispatch(command, context),
            CommandFamily::Debug => self.debug.dispatch(command, context),
        }
    }
}

impl Commands {
    const fn family(&self) -> CommandFamily {
        match self {
            Self::Validate { .. }
            | Self::ValidateUtseg { .. }
            | Self::ShowAlignment { .. }
            | Self::Watch { .. }
            | Self::Lint { .. } => CommandFamily::Validation,
            Self::Normalize { .. }
            | Self::ToJson { .. }
            | Self::ToXml { .. }
            | Self::FromJson { .. }
            | Self::Clean { .. }
            | Self::NewFile { .. }
            | Self::Schema { .. } => CommandFamily::Utility,
            Self::Cache { .. } => CommandFamily::Cache,
            Self::Clan { .. } => CommandFamily::Clan,
            Self::Debug { .. } => CommandFamily::Debug,
        }
    }
}

/// Dispatch one parsed top-level CLI command to its owning feature family.
pub fn dispatch_command(command: Commands, context: &CommandContext) {
    CommandServices::new().dispatch(command, context);
}

struct ValidationCommandService;

impl CommandFamilyService for ValidationCommandService {
    fn dispatch(&self, command: Commands, context: &CommandContext) {
        match command {
            Commands::Validate {
                path,
                list_checks,
                format,
                skip_alignment,
                force,
                jobs,
                quiet,
                max_errors,
                roundtrip,
                parser,
                strict_linkers,
                audit,
                suppress,
                check_xphon,
            } => {
                // Short-circuit: --list-checks prints the check list and
                // exits successfully without touching any files.
                if list_checks {
                    super::list_checks::print_check_list();
                    return;
                }
                run_validate_command(
                    path,
                    ValidateCommandOptions {
                        rules: ValidateCommandRules {
                            alignment: AlignmentValidationMode::from_enabled(!skip_alignment),
                            roundtrip: RoundtripValidationMode::from_enabled(roundtrip),
                            parser_kind: match parser {
                                crate::cli::ParserBackend::TreeSitter => {
                                    talkbank_transform::ParserKind::TreeSitter
                                }
                                crate::cli::ParserBackend::Re2c => {
                                    talkbank_transform::ParserKind::Re2c
                                }
                            },
                            strict_linkers,
                        },
                        execution: ValidateCommandExecution {
                            cache_refresh: CacheRefreshMode::from_force(force),
                            jobs,
                            max_errors,
                        },
                        presentation: ValidateCommandPresentation {
                            format,
                            quiet,
                            audit_output: audit,
                            interface: ValidationInterface::from_tui(context.should_use_tui),
                            theme: context.theme.clone(),
                        },
                        suppress,
                        check_xphon,
                    },
                );
            }
            Commands::ValidateUtseg {
                input,
                output,
                quiet,
            } => {
                let rc = super::validate_utseg::run_validate_utseg(&input, &output, quiet);
                if rc != 0 {
                    std::process::exit(rc);
                }
            }
            Commands::ShowAlignment {
                input,
                tier,
                compact,
            } => show_alignment(&input, tier, compact),
            Commands::Watch {
                path,
                skip_alignment,
                clear,
            } => {
                if let Err(err) = watch_files(&path, !skip_alignment, true, clear) {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            }
            Commands::Lint {
                path,
                fix,
                dry_run,
                skip_alignment,
            } => lint_files(&path, fix, dry_run, true, !skip_alignment),
            // Routing invariant: `CommandRoutingService::dispatch` (in
            // this file) partitions `Commands` variants by family
            // before forwarding to each `CommandFamilyService`, so
            // only the validation-family variants reach this match.
            // Same pattern as the LSP `ExecuteCommandRoutingService`
            // — see `book/src/operations/panic-audit/chatter-lsp.md` for the
            // typed-sub-enum follow-up sketch.
            #[allow(clippy::unreachable)]
            _ => unreachable!("validation service received unsupported command"),
        }
    }
}

struct UtilityCommandService;

impl CommandFamilyService for UtilityCommandService {
    fn dispatch(&self, command: Commands, _context: &CommandContext) {
        match command {
            Commands::Normalize {
                input,
                output,
                validate,
                skip_alignment,
            } => normalize_chat(&input, output.as_ref(), validate, skip_alignment),
            Commands::ToJson {
                input,
                output,
                output_dir,
                compact,
                force,
                prune,
                jobs,
                validate: _,
                alignment: _,
                skip_alignment,
                skip_validation,
                skip_schema_validation,
            } => {
                let pretty = !compact;
                if input.is_dir() {
                    let out_dir = output_dir.unwrap_or_else(|| {
                        eprintln!("Error: directory input requires --output-dir");
                        std::process::exit(1);
                    });
                    super::json::chat_to_json_directory(
                        &input,
                        &out_dir,
                        pretty,
                        !skip_validation,
                        !skip_alignment && !skip_validation,
                        skip_schema_validation,
                        force,
                        prune,
                        jobs,
                    );
                } else {
                    let do_validate = !skip_validation;
                    let run_alignment = !skip_alignment && !skip_validation;
                    chat_to_json(
                        &input,
                        output.as_ref(),
                        pretty,
                        do_validate,
                        run_alignment,
                        skip_schema_validation,
                    );
                }
            }
            Commands::FromJson { input, output } => json_to_chat(&input, output.as_ref()),
            Commands::ToXml {
                input,
                output,
                skip_alignment,
            } => chat_to_xml(&input, output.as_ref(), skip_alignment),
            Commands::Clean {
                path,
                diff_only,
                format,
            } => clean_file(&path, diff_only, format),
            Commands::NewFile {
                output,
                speaker,
                language,
                role,
                corpus,
                utterance,
            } => create_new_file(
                output.as_deref(),
                &speaker,
                &language,
                &role,
                &corpus,
                utterance.as_deref(),
            ),
            Commands::Schema { url } => run_schema(url),
            // Same routing invariant as the validation service above.
            #[allow(clippy::unreachable)]
            _ => unreachable!("utility service received unsupported command"),
        }
    }
}

struct CacheCommandService;

impl CommandFamilyService for CacheCommandService {
    fn dispatch(&self, command: Commands, _context: &CommandContext) {
        match command {
            Commands::Cache { command } => run_cache_command(command),
            // Same routing invariant as the validation service above.
            #[allow(clippy::unreachable)]
            _ => unreachable!("cache service received unsupported command"),
        }
    }
}

struct ClanCommandService;

impl CommandFamilyService for ClanCommandService {
    fn dispatch(&self, command: Commands, _context: &CommandContext) {
        match command {
            Commands::Clan { command } => run_clan(command),
            // Same routing invariant as the validation service above.
            #[allow(clippy::unreachable)]
            _ => unreachable!("clan service received unsupported command"),
        }
    }
}

struct DebugCommandService;

impl CommandFamilyService for DebugCommandService {
    fn dispatch(&self, command: Commands, _context: &CommandContext) {
        match command {
            Commands::Debug { command } => run_debug(command),
            // Same routing invariant as the validation service above.
            #[allow(clippy::unreachable)]
            _ => unreachable!("debug service received unsupported command"),
        }
    }
}

fn run_debug(command: crate::cli::DebugCommands) {
    use crate::cli::DebugCommands;
    match command {
        DebugCommands::FixS { path } => {
            super::debug::run_fix_s(&path);
        }
        DebugCommands::OverlapAudit {
            path,
            format: _,
            database,
        } => {
            super::debug::run_overlap_audit(&path, database.as_deref());
        }
        DebugCommands::LinkerAudit { path, anomalies } => {
            super::debug::run_linker_audit(&path, anomalies.as_deref());
        }
        DebugCommands::Find(args) => {
            super::find::run_find(args);
        }
        DebugCommands::Sanitize { input, output } => {
            super::debug::run_sanitize(&input, output.as_deref());
        }
    }
}
