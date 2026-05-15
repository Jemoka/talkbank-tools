//! JSON conversion commands (to-json, from-json).
//!
//! `chat_to_json` optionally runs validation/alignment and schema checking before
//! serializing to JSON. `json_to_chat` parses the JSON back into a `ChatFile`
//! and writes canonical CHAT text. Keeping both conversions in one module keeps
//! command-level concerns centralized.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use talkbank_model::model::{ChatFile, WriteChat};
use tracing::{Level, debug, info, span, warn};
use walkdir::WalkDir;

use crate::output::print_errors;

/// Convert the CHAT file into a JSON representation, optionally running validation/alignment.
///
/// This function reads the file, configures the pipeline options (validation + `%wor` alignment),
/// and routes through `talkbank_transform::chat_to_json` so that the resulting JSON matches the
/// structure described in the CHAT manual's File Format and Main Tier sections. Schema checks
/// mirror the CHAT manual’s requirements when not explicitly skipped, and any validation failures
/// emit the same diagnostic codes the manual discusses before exiting with a failure status.
pub fn chat_to_json(
    input: &PathBuf,
    output: Option<&PathBuf>,
    pretty: bool,
    validate: bool,
    alignment: bool,
    skip_schema_validation: bool,
) {
    let _span = span!(Level::INFO, "chat_to_json", input = %input.display()).entered();
    info!("Converting CHAT to JSON");

    // Read CHAT file
    let content = {
        let _span = span!(Level::DEBUG, "read_file").entered();
        match fs::read_to_string(input) {
            Ok(c) => {
                debug!("Read {} bytes from file", c.len());
                c
            }
            Err(e) => {
                warn!("Failed to read file: {}", e);
                eprintln!("Error reading file {:?}: {}", input, e);
                std::process::exit(1);
            }
        }
    };

    // Build pipeline options
    let mut options = talkbank_model::ParseValidateOptions::default();
    if validate {
        options = options.with_validation();
    }
    if alignment {
        options = options.with_alignment();
    }

    // Use pipeline function to parse, validate, and serialize to JSON
    // Schema validation is now integrated into the pipeline (unless skipped)
    let json = {
        let _span = span!(Level::DEBUG, "pipeline").entered();
        let result = if skip_schema_validation {
            debug!("Skipping JSON Schema validation (--skip-schema-validation)");
            talkbank_transform::chat_to_json_unvalidated(&content, options, pretty)
        } else {
            talkbank_transform::chat_to_json(&content, options, pretty)
        };
        match result {
            Ok(json_str) => {
                debug!("Pipeline successful, {} bytes", json_str.len());
                if validate || alignment {
                    info!("✓ Validation passed");
                    eprintln!("✓ Validation passed");
                }
                if !skip_schema_validation {
                    info!("✓ JSON schema validation passed");
                }
                json_str
            }
            Err(e) => {
                match e {
                    talkbank_transform::PipelineError::Validation(errors) => {
                        warn!("Validation found {} errors", errors.len());
                        eprintln!("✗ Validation errors found:");
                        print_errors(input, &content, &errors);
                    }
                    talkbank_transform::PipelineError::JsonSerialization(msg) => {
                        warn!("JSON serialization/validation error: {}", msg);
                        eprintln!("✗ JSON error: {}", msg);
                    }
                    _ => {
                        warn!("Pipeline error: {}", e);
                        eprintln!("Error: {}", e);
                    }
                }
                std::process::exit(1);
            }
        }
    };

    // Write or print JSON
    if let Some(output_path) = output {
        let _span = span!(Level::DEBUG, "write_output").entered();
        if let Err(e) = fs::write(output_path, &json) {
            warn!("Failed to write output: {}", e);
            eprintln!("Error writing JSON to {:?}: {}", output_path, e);
            std::process::exit(1);
        }
        info!("Converted {} to {}", input.display(), output_path.display());
        eprintln!(
            "✓ Converted {} to {}",
            input.display(),
            output_path.display()
        );
    } else {
        println!("{}", json);
    }
}

/// Convert a JSON representation back into canonical CHAT text.
///
/// The deserialization/serialization cycle mirrors the chat format described in the manual's
/// File Format and Dependent Tier sections, and errors bubble up so callers receive clear
/// CHAT-aligned diagnostics when the JSON is malformed or cannot be emitted.
pub fn json_to_chat(input: &PathBuf, output: Option<&PathBuf>) {
    let _span = span!(Level::INFO, "json_to_chat", input = %input.display()).entered();
    info!("Converting JSON to CHAT");

    // Read JSON file
    let content = {
        let _span = span!(Level::DEBUG, "read_file").entered();
        match fs::read_to_string(input) {
            Ok(c) => {
                debug!("Read {} bytes from file", c.len());
                c
            }
            Err(e) => {
                warn!("Failed to read file: {}", e);
                eprintln!("Error reading file {:?}: {}", input, e);
                std::process::exit(1);
            }
        }
    };

    // Deserialize JSON to ChatFile
    let chat_file: ChatFile = {
        let _span = span!(Level::DEBUG, "deserialize_json").entered();
        match serde_json::from_str(&content) {
            Ok(cf) => {
                info!("Deserialized ChatFile successfully");
                cf
            }
            Err(e) => {
                warn!("JSON parse error: {}", e);
                eprintln!("Error parsing JSON: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Serialize to CHAT format
    let chat_text = {
        let _span = span!(Level::DEBUG, "serialize_to_chat").entered();
        let result = chat_file.to_chat_string();
        debug!("Serialized to {} bytes", result.len());
        result
    };

    // Write or print CHAT
    if let Some(output_path) = output {
        let _span = span!(Level::DEBUG, "write_output").entered();
        if let Err(e) = fs::write(output_path, &chat_text) {
            warn!("Failed to write output: {}", e);
            eprintln!("Error writing CHAT to {:?}: {}", output_path, e);
            std::process::exit(1);
        }
        info!("Converted {} to {}", input.display(), output_path.display());
        eprintln!(
            "✓ Converted {} to {}",
            input.display(),
            output_path.display()
        );
    } else {
        print!("{}", chat_text);
    }
}

/// Convert all CHAT files in a directory to JSON, preserving directory structure.
///
/// Walks `input_dir` recursively, converting each `.cha` file to a `.json`
/// file under `output_dir` with the same relative path. Incremental by default
/// (skips files whose JSON is newer than the CHAT source). Use `force` to
/// rebuild all. Use `prune` to remove orphaned `.json` files.
#[allow(clippy::too_many_arguments)]
pub fn chat_to_json_directory(
    input_dir: &Path,
    output_dir: &Path,
    pretty: bool,
    validate: bool,
    alignment: bool,
    skip_schema_validation: bool,
    force: bool,
    prune: bool,
    jobs: Option<usize>,
) {
    let _span =
        span!(Level::INFO, "chat_to_json_directory", input = %input_dir.display()).entered();

    // Collect all .cha files
    let cha_files: Vec<PathBuf> = WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "cha"))
        .map(|e| e.into_path())
        .collect();

    let total = cha_files.len();
    eprintln!("Found {total} .cha files in {}", input_dir.display());

    let converted = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    let worker_count = jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()))
        .max(1);

    if worker_count == 1 {
        // Serial mode
        for (i, cha_path) in cha_files.iter().enumerate() {
            if i > 0 && i % 1000 == 0 {
                eprintln!(
                    "  ...{i}/{total} ({} converted, {} skipped, {} failed)",
                    converted.load(Ordering::Relaxed),
                    skipped.load(Ordering::Relaxed),
                    failed.load(Ordering::Relaxed),
                );
            }
            convert_one_file(
                cha_path,
                input_dir,
                output_dir,
                pretty,
                validate,
                alignment,
                skip_schema_validation,
                force,
                &converted,
                &skipped,
                &failed,
            );
        }
    } else {
        // Parallel mode
        let (tx, rx) = crossbeam_channel::bounded::<PathBuf>(worker_count * 2);

        let workers: Vec<_> = (0..worker_count)
            .map(|_| {
                let rx = rx.clone();
                let input_dir = input_dir.to_path_buf();
                let output_dir = output_dir.to_path_buf();
                let converted = &converted as *const AtomicUsize as usize;
                let skipped = &skipped as *const AtomicUsize as usize;
                let failed = &failed as *const AtomicUsize as usize;
                std::thread::spawn(move || {
                    // SAFETY: atomics outlive the thread (we join before returning)
                    let converted = unsafe { &*(converted as *const AtomicUsize) };
                    let skipped = unsafe { &*(skipped as *const AtomicUsize) };
                    let failed = unsafe { &*(failed as *const AtomicUsize) };
                    while let Ok(cha_path) = rx.recv() {
                        convert_one_file(
                            &cha_path,
                            &input_dir,
                            &output_dir,
                            pretty,
                            validate,
                            alignment,
                            skip_schema_validation,
                            force,
                            converted,
                            skipped,
                            failed,
                        );
                    }
                })
            })
            .collect();

        // Feed files
        for (i, cha_path) in cha_files.into_iter().enumerate() {
            if i > 0 && i % 1000 == 0 {
                eprintln!(
                    "  ...{i}/{total} ({} converted, {} skipped, {} failed)",
                    converted.load(Ordering::Relaxed),
                    skipped.load(Ordering::Relaxed),
                    failed.load(Ordering::Relaxed),
                );
            }
            // Worker-lifetime invariant: workers are spawned in the
            // for-loop above and joined in the for-loop below;
            // `tx.send` only fails when every Receiver has dropped,
            // which can only happen after all workers exit. Workers
            // exit when the channel closes (drop(tx) below), so this
            // send is during the active phase.
            #[allow(clippy::expect_used)]
            tx.send(cha_path).expect("worker alive");
        }
        drop(tx);

        for w in workers {
            // Thread::join only returns Err on a worker panic, which
            // is itself a panic-level bug — propagating it is correct.
            #[allow(clippy::expect_used)]
            w.join().expect("worker panicked");
        }
    }

    let conv = converted.load(Ordering::Relaxed);
    let skip = skipped.load(Ordering::Relaxed);
    let fail = failed.load(Ordering::Relaxed);

    // Prune orphaned .json files
    let mut pruned: usize = 0;
    if prune {
        pruned = prune_orphaned_json(input_dir, output_dir);
    }

    eprintln!();
    eprintln!(
        "Done: {conv} converted, {skip} up-to-date, {fail} failed, {pruned} pruned (of {total} total)"
    );
}

/// Convert a single .cha file to .json under the output directory.
fn convert_one_file(
    cha_path: &Path,
    input_dir: &Path,
    output_dir: &Path,
    pretty: bool,
    validate: bool,
    alignment: bool,
    skip_schema_validation: bool,
    force: bool,
    converted: &AtomicUsize,
    skipped: &AtomicUsize,
    failed: &AtomicUsize,
) {
    // Compute relative path and output path
    let rel = cha_path.strip_prefix(input_dir).unwrap_or(cha_path);
    let json_path = output_dir.join(rel).with_extension("json");

    // Incremental: skip if json is newer than cha
    if !force {
        if let (Ok(cha_meta), Ok(json_meta)) = (fs::metadata(cha_path), fs::metadata(&json_path)) {
            if let (Ok(cha_mtime), Ok(json_mtime)) = (cha_meta.modified(), json_meta.modified()) {
                if json_mtime >= cha_mtime {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            }
        }
    }

    // Read source
    let content = match fs::read_to_string(cha_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR: cannot read {}: {e}", cha_path.display());
            failed.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    // Build pipeline options
    let mut options = talkbank_model::ParseValidateOptions::default();
    if validate {
        options = options.with_validation();
    }
    if alignment {
        options = options.with_alignment();
    }

    // Convert
    let json = if skip_schema_validation {
        talkbank_transform::chat_to_json_unvalidated(&content, options, pretty)
    } else {
        talkbank_transform::chat_to_json(&content, options, pretty)
    };

    match json {
        Ok(json_str) => {
            // Ensure parent directory exists
            if let Some(parent) = json_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!("ERROR: cannot create directory {}: {e}", parent.display());
                    failed.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            }
            if let Err(e) = fs::write(&json_path, &json_str) {
                eprintln!("ERROR: cannot write {}: {e}", json_path.display());
                failed.fetch_add(1, Ordering::Relaxed);
                return;
            }
            converted.fetch_add(1, Ordering::Relaxed);
        }
        Err(e) => {
            eprintln!("ERROR: {}: {e}", cha_path.display());
            // Remove stale json if it exists
            let _ = fs::remove_file(&json_path);
            failed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Remove `.json` files in `output_dir` that have no matching `.cha` in `input_dir`.
fn prune_orphaned_json(input_dir: &Path, output_dir: &Path) -> usize {
    let mut pruned: usize = 0;
    for entry in WalkDir::new(output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "json")
        })
    {
        let json_path = entry.path();
        let rel = match json_path.strip_prefix(output_dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let cha_path = input_dir.join(rel).with_extension("cha");
        if !cha_path.exists() {
            if let Err(e) = fs::remove_file(json_path) {
                eprintln!("WARN: cannot prune {}: {e}", json_path.display());
            } else {
                pruned += 1;
                // Clean empty parent dirs
                let mut dir = json_path.parent();
                while let Some(d) = dir {
                    if d == output_dir {
                        break;
                    }
                    if fs::read_dir(d).is_ok_and(|mut entries| entries.next().is_none()) {
                        let _ = fs::remove_dir(d);
                        dir = d.parent();
                    } else {
                        break;
                    }
                }
            }
        }
    }
    pruned
}
