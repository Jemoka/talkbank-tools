//! Build corpus manifest for entire data directory.
//!
//! Usage: cargo test --release --test build_corpus_manifest -- --nocapture
//!
//! This scans the internal workspace data checkout (sibling `../data` from the
//! `talkbank-tools` repo root, then legacy `~/data`) for all corpora
//! (directories with `0metadata.cdc`) and creates a manifest file for batch
//! processing.

use std::path::PathBuf;
use talkbank_transform::{ManifestError, build_manifest, format_manifest};
use thiserror::Error;

/// Entry point for this binary target.
fn main() {
    let data_root = default_data_root();

    if !data_root.exists() {
        eprintln!("Data directory not found: {}", data_root.display());
        std::process::exit(1);
    }

    println!("Building corpus manifest for: {}", data_root.display());
    println!();

    match build_manifest(&data_root) {
        Ok(manifest) => {
            println!();
            print!("{}", format_manifest(&manifest));

            // Save manifest
            let manifest_path =
                home_dir_or_exit().join(".cache/talkbank-tools/corpus-manifest.json");

            match manifest.save(&manifest_path) {
                Ok(()) => {
                    println!();
                    println!("✓ Manifest saved to: {}", manifest_path.display());
                }
                Err(e) => {
                    eprintln!("Failed to save manifest: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to build manifest: {}", e);
            std::process::exit(1);
        }
    }
}

/// Enum variants for TestError.
#[derive(Debug, Error)]
enum TestError {
    #[error("Manifest error")]
    Manifest(#[from] ManifestError),
}

/// Tests build manifest.
#[test]
fn test_build_manifest() -> Result<(), TestError> {
    let data_root = default_data_root();

    if !data_root.exists() {
        println!("Data directory not found, skipping test");
        return Ok(());
    }

    let manifest = build_manifest(&data_root)?;

    if manifest.total_corpora == 0 || manifest.total_files == 0 {
        println!(
            "No corpora discovered under {}, skipping environment-dependent assertion",
            data_root.display()
        );
        return Ok(());
    }

    println!();
    println!("Manifest built successfully:");
    println!("  Corpora: {}", manifest.total_corpora);
    println!("  Files: {}", manifest.total_files);
    Ok(())
}

/// Runs home dir or exit.
fn home_dir_or_exit() -> PathBuf {
    match dirs::home_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("Failed to get home directory");
            std::process::exit(1);
        }
    }
}

fn default_data_root() -> PathBuf {
    let workspace_data = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|dir| dir.join("data"))
        .unwrap_or_else(|| home_dir_or_exit().join("talkbank").join("data"));
    if workspace_data.exists() {
        return workspace_data;
    }

    let legacy_data = home_dir_or_exit().join("data");
    if legacy_data.exists() {
        return legacy_data;
    }

    workspace_data
}
