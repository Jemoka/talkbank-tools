// Build scripts run at build time, not runtime. Panics here fail
// `cargo build`, which is the intended behaviour for missing files
// or invalid embedded data. Per-crate `deny` panic lints would
// otherwise fire on standard `env::var().unwrap()` and
// `fs::File::create().unwrap()` patterns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Build script for talkbank-model.
//!
//! Generates a compile-time perfect hash set of ISO 639-3 language codes from
//! the vendored registry at `data/iso639-3.txt` (committed inside this crate).
//!
//! The ISO 639-3 data file was extracted from `clan-info/lib/fixes/ISO 639-3.txt`
//! and vendored into this crate so CI and fresh clones always have it without
//! needing to clone the private `clan-info` submodule.
//!
//! ## Syncing the vendored list
//!
//! The ISO 639-3 standard is updated infrequently (new codes are occasionally
//! added for newly-documented languages; retired codes are deprecated but kept).
//! When the master list in `clan-info/lib/fixes/ISO 639-3.txt` is updated,
//! sync `data/iso639-3.txt` manually:
//!
//! ```bash
//! cp clan-info/lib/fixes/ISO\ 639-3.txt talkbank-tools/crates/talkbank-model/data/iso639-3.txt
//! ```
//!
//! There is no automated check for this — syncing is a periodic maintenance
//! task, not a CI gate.
//!
//! The generated file is written to `$OUT_DIR/iso639_3_set.rs` and included
//! by `src/model/header/codes/iso639.rs` at compile time.

use std::env;
use std::fs;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;

fn main() {
    generate_iso639_3_set();
}

/// Parse the ISO 639-3 registry and generate a `phf::Set<&str>`.
fn generate_iso639_3_set() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let crate_root = Path::new(&manifest_dir);

    // Primary: vendored copy committed inside this crate (data/iso639-3.txt).
    // Always present in CI and fresh clones — no external submodule needed.
    let vendored = crate_root.join("data/iso639-3.txt");

    // Fallback: clan-info sibling repo in the talkbank-dev workspace.
    // Used when a developer clones clan-info alongside talkbank-tools.
    let clan_info_path = crate_root
        .parent() // crates/
        .and_then(|p| p.parent()) // talkbank-tools/
        .and_then(|p| p.parent()) // talkbank-dev/
        .map(|workspace| workspace.join("clan-info/lib/fixes/ISO 639-3.txt"));

    let iso_path = if vendored.exists() {
        vendored
    } else if let Some(ref p) = clan_info_path {
        if p.exists() {
            p.clone()
        } else {
            // Neither source found — emit an empty set (graceful degradation).
            eprintln!(
                "cargo:warning=ISO 639-3 file not found at data/iso639-3.txt or \
                 clan-info/lib/fixes/. Language code membership validation will be disabled."
            );
            generate_empty_set();
            return;
        }
    } else {
        eprintln!(
            "cargo:warning=ISO 639-3 file not found, generating empty set. \
             Language code membership validation will be disabled."
        );
        generate_empty_set();
        return;
    };

    println!("cargo:rerun-if-changed={}", iso_path.display());

    let content = fs::read_to_string(&iso_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read ISO 639-3 file at {}: {}",
            iso_path.display(),
            e
        )
    });

    let mut codes: Vec<&str> = Vec::with_capacity(8500);

    for line in content.lines() {
        // Format: `aaa\t|...|...|Language Name
        // The backtick prefix + 3-letter code is positions 0..4.
        if line.starts_with('`') && line.len() >= 4 {
            let code = &line[1..4];
            if code.len() == 3 && code.chars().all(|c| c.is_ascii_lowercase()) {
                codes.push(code);
            }
        }
    }

    if codes.is_empty() {
        eprintln!("cargo:warning=No codes parsed from ISO 639-3 file, generating empty set.");
        generate_empty_set();
        return;
    }

    // Generate the phf set.
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("iso639_3_set.rs");
    let file = fs::File::create(&dest_path).unwrap();
    let mut writer = BufWriter::new(file);

    writeln!(
        writer,
        "/// ISO 639-3 language code set ({} codes).",
        codes.len()
    )
    .unwrap();
    writeln!(
        writer,
        "/// Generated from clan-info/lib/fixes/ISO 639-3.txt by build.rs."
    )
    .unwrap();

    let mut set = phf_codegen::Set::new();
    for code in &codes {
        set.entry(*code);
    }

    writeln!(
        writer,
        "static ISO_639_3_CODES: phf::Set<&'static str> = {};",
        set.build()
    )
    .unwrap();

    eprintln!(
        "cargo:warning=Generated ISO 639-3 set with {} codes",
        codes.len()
    );
}

/// Generate an empty set for environments without the ISO file.
fn generate_empty_set() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("iso639_3_set.rs");
    let mut file = fs::File::create(&dest_path).unwrap();
    writeln!(
        file,
        "/// Empty ISO 639-3 set (file not available at build time).\n\
         static ISO_639_3_CODES: phf::Set<&'static str> = {};",
        phf_codegen::Set::<&str>::new().build()
    )
    .unwrap();
}
