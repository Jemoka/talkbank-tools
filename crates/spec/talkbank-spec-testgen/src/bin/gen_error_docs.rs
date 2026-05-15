//! Generate error documentation in Markdown format
//!
//! Reads error specs and generates publishable documentation.

use clap::Parser;
use generators::output::markdown;
use generators::spec::ErrorSpec;
use std::path::PathBuf;

/// CLI arguments: input error spec directory and output directory for generated Markdown docs.
#[derive(Parser)]
#[command(name = "gen_error_docs")]
#[command(about = "Generate error documentation")]
struct Args {
    /// Root directory containing error specs
    #[arg(short, long, default_value = "spec/errors")]
    error_dir: PathBuf,

    /// Output directory for generated documentation
    #[arg(short, long, default_value = "docs/errors")]
    output_dir: PathBuf,
}

/// Generates publishable Markdown documentation (index + per-error pages) from error specs.
fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!(
        "Loading error specifications from: {}",
        args.error_dir.display()
    );

    let specs = ErrorSpec::load_all(&args.error_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load error specs: {}", e))?;

    println!("Loaded {} error specifications", specs.len());

    // ALWAYS clean generated markdown files before regenerating to prevent stale docs
    if args.output_dir.exists() {
        println!("Cleaning old generated documentation files...");
        if let Ok(entries) = std::fs::read_dir(&args.output_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    // Ensure output directory exists
    std::fs::create_dir_all(&args.output_dir)?;

    // Generate index page
    let index = markdown::generate_error_index(&specs);
    let index_path = args.output_dir.join("index.md");
    std::fs::write(&index_path, index)?;
    println!("✓ Generated: {}", index_path.display());

    // Generate individual error pages
    let mut page_count = 0;
    for spec in &specs {
        for error in &spec.errors {
            // Category-level status applies to every error in the spec;
            // see ErrorMetadata::status and generate_error_page docs.
            let page = markdown::generate_error_page(error, &spec.metadata.status);
            let page_path = args.output_dir.join(format!("{}.md", error.code));
            std::fs::write(&page_path, page)?;
            println!("✓ Generated: {}", page_path.display());
            page_count += 1;
        }
    }

    println!("\n✓ Generated {} error documentation pages", page_count + 1);

    Ok(())
}
