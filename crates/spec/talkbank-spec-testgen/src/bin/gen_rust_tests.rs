//! Generate Rust test files from specifications
//!
//! Reads construct and error specs and generates Rust test files
//! directly to talkbank-tools.

use clap::Parser;
use talkbank_spec_testgen::output::rust_test;
use talkbank_spec_testgen::spec::{ConstructSpec, ErrorSpec};
use std::path::PathBuf;

/// CLI arguments: input spec directories, output directory for generated `.rs` files, and test error type path.
#[derive(Parser)]
#[command(name = "gen_rust_tests")]
#[command(about = "Generate Rust test files")]
struct Args {
    /// Root directory containing construct specs
    #[arg(long, default_value = "spec/constructs")]
    construct_dir: PathBuf,

    /// Root directory containing error specs
    #[arg(long, default_value = "spec/errors")]
    error_dir: PathBuf,

    /// Output directory for generated test files (e.g., path/to/talkbank-tools/crates/talkbank-parser-tests/tests/generated)
    /// WARNING: Generated test files in this directory will be removed before regenerating
    /// to ensure no stale tests remain when specs are deleted
    #[arg(short, long)]
    output_dir: PathBuf,

    /// Fully-qualified path to the TestError type used in generated tests
    #[arg(long, default_value = "talkbank_utils::test_error::TestError")]
    test_error_path: String,
}

/// Generates Rust test files from construct and error specs for the parser test suite.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Loading specifications...");

    let construct_specs = ConstructSpec::load_all(&args.construct_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load construct specs: {}", e))?;

    let error_specs = ErrorSpec::load_all(&args.error_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {}", e))?;

    println!(
        "Loaded {} construct specs, {} error specs",
        construct_specs.len(),
        error_specs.len()
    );
    println!("Output directory: {}", args.output_dir.display());

    // ALWAYS clean generated_*.rs files before regenerating to prevent stale tests
    // (when specs are deleted, their corresponding test files should also be deleted)
    if args.output_dir.exists() {
        println!("Cleaning old generated test files...");
        for entry in std::fs::read_dir(&args.output_dir)? {
            let entry = entry?;
            let path = entry.path();
            // Remove known generated files only
            if let Some(filename) = path.file_name() {
                if let Some(name_str) = filename.to_str() {
                    if is_generated_output(name_str) && path.extension().is_some_and(|e| e == "rs")
                    {
                        std::fs::remove_file(&path)?;
                    }
                }
            }
        }
    }

    // Ensure output directory exists
    std::fs::create_dir_all(&args.output_dir)?;

    // Generate construct tests (both versions)
    let construct_tests =
        rust_test::generate_construct_test_file(&construct_specs, &args.test_error_path);
    let construct_path = args.output_dir.join("generated_construct_tests.rs");
    std::fs::write(&construct_path, construct_tests)?;
    println!("✓ Generated: {}", construct_path.display());

    let construct_body =
        rust_test::generate_construct_test_body(&construct_specs, &args.test_error_path);
    let construct_body_path = args.output_dir.join("generated_construct_tests_body.rs");
    std::fs::write(&construct_body_path, construct_body)?;
    println!("✓ Generated: {}", construct_body_path.display());

    // Generate error tests (both versions)
    let error_tests = rust_test::generate_error_test_file(&error_specs, &args.test_error_path);
    let error_path = args.output_dir.join("generated_error_tests.rs");
    std::fs::write(&error_path, error_tests)?;
    println!("✓ Generated: {}", error_path.display());

    let error_body = rust_test::generate_error_test_body(&error_specs, &args.test_error_path);
    let error_body_path = args.output_dir.join("generated_error_tests_body.rs");
    std::fs::write(&error_body_path, error_body)?;
    println!("✓ Generated: {}", error_body_path.display());

    println!(
        "\n✓ Generated 4 test files to {}",
        args.output_dir.display()
    );

    Ok(())
}

fn is_generated_output(filename: &str) -> bool {
    matches!(
        filename,
        "generated_construct_tests.rs"
            | "generated_construct_tests_body.rs"
            | "generated_error_tests.rs"
            | "generated_error_tests_body.rs"
    )
}
