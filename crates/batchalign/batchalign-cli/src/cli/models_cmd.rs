//! `batchalign3 models` — model training utilities.
//!
//! - `models prep` — Rust-native CHAT→text extraction for training data.
//! - `models train` — forwards to Python for the PyTorch training loop.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::chat_ops::TierDomain;
use talkbank_transform::extract::extract_words;
use talkbank_transform::parse::{TreeSitterParser, parse_lenient};
use walkdir::WalkDir;

use crate::cli::args::{ModelsPrepArgs, ModelsTrainArgs};
use crate::cli::error::CliError;
use crate::cli::python::resolve_python_executable;

/// Run the `models prep` subcommand: parse CHAT files and extract utterance text.
pub fn run_prep(args: &ModelsPrepArgs) -> Result<(), CliError> {
    let input = Path::new(&args.input_dir);
    if !input.exists() {
        return Err(CliError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("input directory does not exist: {}", input.display()),
        )));
    }

    let cha_files = collect_chat_files(input)?;

    if cha_files.is_empty() {
        return Err(CliError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("no .cha files found in {}", input.display()),
        )));
    }

    let utterances = extract_utterances_from_files(&cha_files, args.min_length)?;

    let (train_lines, val_lines) = if let Some(val_dir) = &args.val_dir {
        let val_path = Path::new(val_dir);
        let val_files = collect_chat_files(val_path)?;
        let val_utts = extract_utterances_from_files(&val_files, args.min_length)?;
        (utterances, val_utts)
    } else {
        split_train_val(&utterances, args.val_fraction, &args.run_name)
    };

    let out = Path::new(&args.output_dir);
    std::fs::create_dir_all(out)?;

    write_lines(
        &out.join(format!("{}.train.txt", args.run_name)),
        &train_lines,
    )?;
    write_lines(&out.join(format!("{}.val.txt", args.run_name)), &val_lines)?;

    eprintln!(
        "Prepared {} train, {} val utterances → {}",
        train_lines.len(),
        val_lines.len(),
        out.display()
    );
    Ok(())
}

/// Run the `models train` subcommand: forward to Python training runtime.
pub fn run_train(args: &ModelsTrainArgs) -> Result<(), CliError> {
    let python = resolve_python_executable();
    let mut cmd = std::process::Command::new(&python);
    cmd.arg("-m")
        .arg("batchalign.models.training.run")
        .args(&args.args);

    let status = cmd.status().map_err(|e| {
        std::io::Error::other(format!(
            "failed to start Python training runtime '{python}': {e}"
        ))
    })?;

    if status.success() {
        return Ok(());
    }

    std::process::exit(status.code().unwrap_or(1));
}

fn collect_chat_files(root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| {
            let detail = if let Some(path) = error.path() {
                format!("walk model-prep input {}: {error}", path.display())
            } else {
                format!("walk model-prep input: {error}")
            };
            CliError::Io(std::io::Error::other(detail))
        })?;
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("cha"))
        {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

/// Parse CHAT files and extract utterance text using the Rust AST.
fn extract_utterances_from_files(
    files: &[PathBuf],
    min_word_count: usize,
) -> Result<Vec<String>, CliError> {
    let parser = TreeSitterParser::new()
        .map_err(|e| CliError::InvalidArgument(format!("parser init: {e}")))?;
    let mut utterances = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(path)?;
        let (chat_file, _warnings) = parse_lenient(&parser, &text);
        let extracted = extract_words(&chat_file, TierDomain::Wor);
        for utt in &extracted {
            let words: Vec<&str> = utt.words.iter().map(|w| w.text.as_str()).collect();
            if words.len() >= min_word_count {
                utterances.push(words.join(" "));
            }
        }
    }
    Ok(utterances)
}

/// Deterministic train/val split seeded by run name.
fn split_train_val(
    utterances: &[String],
    val_fraction: f64,
    seed_str: &str,
) -> (Vec<String>, Vec<String>) {
    use rand::SeedableRng;
    use rand::seq::SliceRandom;

    // Deterministic seed from run name.
    let seed = {
        let mut hash: u64 = 0;
        for b in seed_str.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(b));
        }
        hash
    };
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);

    let mut indices: Vec<usize> = (0..utterances.len()).collect();
    indices.shuffle(&mut rng);

    let val_count = (utterances.len() as f64 * val_fraction).round() as usize;
    let val_indices: std::collections::HashSet<usize> =
        indices[..val_count].iter().copied().collect();

    let mut train = Vec::new();
    let mut val = Vec::new();
    for (i, utt) in utterances.iter().enumerate() {
        if val_indices.contains(&i) {
            val.push(utt.clone());
        } else {
            train.push(utt.clone());
        }
    }
    (train, val)
}

/// Write lines to a text file.
fn write_lines(path: &Path, lines: &[String]) -> Result<(), CliError> {
    let mut f = std::fs::File::create(path)?;
    for line in lines {
        writeln!(f, "{line}")?;
    }
    Ok(())
}
