//! Generate tree-sitter test corpus files from specifications
//!
//! Reads construct spec files and generates test/corpus/*.txt files
//! in tree-sitter native format directly to tree-sitter-talkbank.

use clap::Parser;
use generators::output::tree_sitter;
use generators::spec::{ConstructSpec, ErrorSpec};
use std::path::PathBuf;

/// CLI arguments: construct/error spec directories, output corpus directory, and template directory.
#[derive(Parser)]
#[command(name = "gen_tree_sitter_tests")]
#[command(about = "Generate tree-sitter test corpus files")]
struct Args {
    /// Root directory containing construct specs
    #[arg(short, long, default_value = "spec/constructs")]
    spec_dir: PathBuf,

    /// Root directory containing error specs
    #[arg(short = 'e', long, default_value = "spec/errors")]
    error_dir: PathBuf,

    /// Output directory for generated test files (e.g., path/to/tree-sitter-talkbank/test/corpus)
    /// WARNING: Generated test files in this directory will be removed before regenerating
    /// to ensure no stale tests remain when specs are deleted
    #[arg(short, long)]
    output_dir: PathBuf,

    /// Template directory for wrapping fragments
    #[arg(short, long, default_value = "generators/templates")]
    template_dir: PathBuf,
}

/// Generates tree-sitter corpus test files from construct and error specs.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!(
        "Loading construct specifications from: {}",
        args.spec_dir.display()
    );

    let specs = ConstructSpec::load_all(&args.spec_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load specs: {}", e))?;

    println!("Loaded {} construct specifications", specs.len());

    println!(
        "Loading error specifications from: {}",
        args.error_dir.display()
    );

    let error_specs = ErrorSpec::load_all(&args.error_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {}", e))?;

    // Filter to parser-layer errors only (validation errors parse fine — no ERROR nodes)
    let parser_errors: Vec<&ErrorSpec> = error_specs
        .iter()
        .filter(|s| s.metadata.error_type == "parser")
        .collect();

    println!(
        "Loaded {} error specs ({} parser-layer)",
        error_specs.len(),
        parser_errors.len()
    );
    println!("Using templates from: {}", args.template_dir.display());
    println!("Output directory: {}", args.output_dir.display());

    // Generate construct corpus files with template support
    let construct_files =
        tree_sitter::generate_corpus_files_with_templates(&specs, Some(&args.template_dir))
            .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Generate error corpus files
    let error_files = tree_sitter::generate_error_corpus_files(&parser_errors)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let construct_count = construct_files.len();
    let error_count = error_files.len();

    // ALWAYS clean before regenerating to prevent stale test files
    // (when specs are deleted, their corresponding test files should also be deleted)
    if args.output_dir.exists() {
        println!("Cleaning old generated test files...");
        std::fs::remove_dir_all(&args.output_dir)?;
    }

    // Ensure output directory exists
    std::fs::create_dir_all(&args.output_dir)?;

    // Write all files (creating parent directories as needed)
    let all_files = construct_files.into_iter().chain(error_files);
    for (filename, content) in all_files {
        let path = args.output_dir.join(&filename);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        println!("✓ Generated: {}", path.display());
    }

    println!(
        "\n✓ Generated {} construct + {} error test corpus files to {}",
        construct_count,
        error_count,
        args.output_dir.display()
    );
    println!("\nTo test, run 'tree-sitter test' in the tree-sitter-talkbank directory");

    Ok(())
}
