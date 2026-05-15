//! `batchalign3 setup` — initialize ~/.batchalign.ini.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::cli::args::{SetupArgs, SetupEngine};
use crate::cli::error::CliError;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetupConfig {
    engine: SetupEngine,
    rev_key: Option<String>,
}

fn invalid_input(msg: &str) -> CliError {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, msg).into()
}

/// Path to the user-level configuration file (`~/.batchalign.ini`).
///
/// This file stores the default ASR engine and Rev.AI API key. Its existence
/// is used as the gate for first-run setup: if missing, the CLI triggers
/// interactive setup before processing commands (matching batchalign2 behavior).
pub fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".batchalign.ini")
}

/// True if `~/.batchalign.ini` exists on disk.
pub fn config_exists() -> bool {
    default_config_path().is_file()
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool, CliError> {
    loop {
        let suffix = if default { "[Y/n]" } else { "[y/N]" };
        eprint!("{prompt} {suffix} ");
        std::io::stderr().flush()?;

        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        let answer = line.trim().to_ascii_lowercase();
        if answer.is_empty() {
            return Ok(default);
        }
        if answer == "y" || answer == "yes" {
            return Ok(true);
        }
        if answer == "n" || answer == "no" {
            return Ok(false);
        }
        eprintln!("Please answer 'y' or 'n'.");
    }
}

fn prompt_non_empty(label: &str) -> Result<String, CliError> {
    loop {
        eprint!("{label}: ");
        std::io::stderr().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        eprintln!("Value cannot be empty.");
    }
}

fn normalize_rev_key(rev_key: Option<String>) -> Result<Option<String>, CliError> {
    match rev_key {
        Some(key) => {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                Err(invalid_input("Rev.ai key cannot be empty"))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        None => Ok(None),
    }
}

fn resolve_non_interactive(args: &SetupArgs) -> Result<SetupConfig, CliError> {
    let explicit_key = normalize_rev_key(args.rev_key.clone())?;
    let engine = match (args.engine, explicit_key.as_ref()) {
        (Some(engine), _) => engine,
        (None, Some(_)) => SetupEngine::Rev,
        (None, None) => SetupEngine::Whisper,
    };

    match engine {
        SetupEngine::Rev => {
            let rev_key = explicit_key.ok_or_else(|| {
                invalid_input("--engine rev requires --rev-key in non-interactive mode")
            })?;
            Ok(SetupConfig {
                engine,
                rev_key: Some(rev_key),
            })
        }
        SetupEngine::Whisper => {
            if explicit_key.is_some() {
                return Err(invalid_input(
                    "--rev-key is only valid when the selected engine is 'rev'",
                ));
            }
            Ok(SetupConfig {
                engine,
                rev_key: None,
            })
        }
    }
}

fn resolve_interactive(args: &SetupArgs) -> Result<SetupConfig, CliError> {
    let engine = match args.engine {
        Some(engine) => engine,
        None => {
            let use_rev =
                prompt_yes_no("Would you like to set Rev.ai as default ASR engine?", false)?;
            if use_rev {
                SetupEngine::Rev
            } else {
                SetupEngine::Whisper
            }
        }
    };

    match engine {
        SetupEngine::Whisper => {
            if args.rev_key.is_some() {
                return Err(invalid_input(
                    "--rev-key is only valid when the selected engine is 'rev'",
                ));
            }
            Ok(SetupConfig {
                engine,
                rev_key: None,
            })
        }
        SetupEngine::Rev => {
            if let Some(key) = normalize_rev_key(args.rev_key.clone())? {
                return Ok(SetupConfig {
                    engine,
                    rev_key: Some(key),
                });
            }

            loop {
                let first = prompt_non_empty("Rev.ai API key")?;
                let second = prompt_non_empty("Confirm Rev.ai API key")?;
                if first == second {
                    return Ok(SetupConfig {
                        engine,
                        rev_key: Some(first),
                    });
                }
                eprintln!("Keys did not match. Try again.");
            }
        }
    }
}

fn resolve_config(args: &SetupArgs) -> Result<SetupConfig, CliError> {
    if args.non_interactive {
        resolve_non_interactive(args)
    } else {
        resolve_interactive(args)
    }
}

fn render_ini(config: &SetupConfig) -> String {
    let mut out = String::from("[asr]\n");
    out.push_str(&format!(
        "engine = {}\n",
        match config.engine {
            SetupEngine::Rev => "rev",
            SetupEngine::Whisper => "whisper",
        }
    ));
    if let Some(key) = &config.rev_key {
        out.push_str(&format!("engine.rev.key = {key}\n"));
    }
    out
}

fn write_config(path: &Path, config: &SetupConfig) -> Result<(), CliError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, render_ini(config))?;
    Ok(())
}

/// Resolve configuration and write `~/.batchalign.ini`.
pub fn run(args: &SetupArgs) -> Result<(), CliError> {
    let config = resolve_config(args)?;
    let path = default_config_path();
    write_config(&path, &config)?;

    eprintln!("Saved configuration to {}", path.display());
    eprintln!(
        "Default ASR engine: {}",
        match config.engine {
            SetupEngine::Rev => "rev",
            SetupEngine::Whisper => "whisper",
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(
        engine: Option<SetupEngine>,
        rev_key: Option<&str>,
        non_interactive: bool,
    ) -> SetupArgs {
        SetupArgs {
            engine,
            rev_key: rev_key.map(ToString::to_string),
            non_interactive,
        }
    }

    #[test]
    fn resolve_non_interactive_defaults_to_whisper() {
        let cfg = resolve_config(&args(None, None, true)).unwrap();
        assert_eq!(cfg.engine, SetupEngine::Whisper);
        assert!(cfg.rev_key.is_none());
    }

    #[test]
    fn resolve_non_interactive_rev_requires_key() {
        let err = resolve_config(&args(Some(SetupEngine::Rev), None, true)).unwrap_err();
        assert!(err.to_string().contains("requires --rev-key"));
    }

    #[test]
    fn resolve_non_interactive_rejects_key_for_whisper() {
        let err = resolve_config(&args(Some(SetupEngine::Whisper), Some("abc"), true)).unwrap_err();
        assert!(err.to_string().contains("only valid"));
    }

    #[test]
    fn resolve_non_interactive_infers_rev_from_key() {
        let cfg = resolve_config(&args(None, Some("abc"), true)).unwrap();
        assert_eq!(cfg.engine, SetupEngine::Rev);
        assert_eq!(cfg.rev_key.as_deref(), Some("abc"));
    }

    #[test]
    fn write_config_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg_path = tmp.path().join(".batchalign.ini");
        let cfg = SetupConfig {
            engine: SetupEngine::Rev,
            rev_key: Some("secret".to_string()),
        };

        write_config(&cfg_path, &cfg).unwrap();
        let content = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(content.contains("[asr]"));
        assert!(content.contains("engine = rev"));
        assert!(content.contains("engine.rev.key = secret"));
    }
}
