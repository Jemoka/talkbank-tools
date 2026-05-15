//! Banner and per-utterance header rendering for alignment output.
//!
//! Prints the file-level banner and per-utterance separator lines that frame
//! the tier-alignment rows. The utterance header shows the 1-based index,
//! speaker code, and main-tier content string.

use std::path::Path;

/// Print the banner for an alignment visualization run.
pub(super) fn render_intro(input: &Path) {
    println!("Alignment visualization for: {}", input.display());
    println!("{}", "=".repeat(80));
}

/// Print one utterance header line before per-tier alignment rows.
pub(super) fn render_utterance_header(utterance_index: usize, speaker: &str, main_content: &str) {
    println!();
    println!(
        "Utterance #{} - {}:\t{}",
        utterance_index + 1,
        speaker,
        main_content
    );
    println!("{:-<80}", "");
}
