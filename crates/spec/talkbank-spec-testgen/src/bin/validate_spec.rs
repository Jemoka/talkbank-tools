//! Validate Markdown specification files
//!
//! Checks that all spec files have valid format and required fields.

use clap::Parser;
use generators::spec::ConstructSpec;
use std::path::PathBuf;

/// CLI arguments: root directory containing spec files to validate.
#[derive(Parser)]
#[command(name = "validate_spec")]
#[command(about = "Validate Markdown specification files")]
struct Args {
    /// Root directory containing spec files
    #[arg(short, long, default_value = "spec")]
    spec_dir: PathBuf,
}

/// Validates that all Markdown spec files have correct format and required metadata fields.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Validating specifications in: {}", args.spec_dir.display());

    let mut construct_count = 0;
    let mut errors = Vec::new();

    // Validate constructs (Markdown)
    let constructs_dir = args.spec_dir.join("constructs");
    if constructs_dir.exists() {
        match ConstructSpec::load_all(&constructs_dir) {
            Ok(specs) => {
                construct_count = specs.len();
                println!(
                    "✓ Loaded {} construct specifications from Markdown",
                    construct_count
                );
            }
            Err(e) => {
                errors.push(format!("✗ Construct loading failed: {}", e));
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Construct specs: {}", construct_count);

    if !errors.is_empty() {
        println!("\n=== Errors ===");
        for error in &errors {
            println!("{}", error);
        }
        anyhow::bail!("Validation failed with {} errors", errors.len());
    }

    println!("\n✓ All specifications are valid");
    Ok(())
}
