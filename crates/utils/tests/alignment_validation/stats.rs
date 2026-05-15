//! Test module for stats in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.\n
/// Type representing AlignmentStats.
#[derive(Default)]
pub struct AlignmentStats {
    pub total_files: usize,
    pub files_with_alignment_errors: usize,
    pub total_utterances: usize,
    pub utterances_with_alignment_errors: usize,
    pub total_alignment_errors: usize,
}

impl AlignmentStats {
    /// Runs note file.
    pub fn note_file(&mut self) {
        self.total_files += 1;
    }

    /// Runs note file with errors.
    pub fn note_file_with_errors(&mut self) {
        self.files_with_alignment_errors += 1;
    }

    /// Runs note utterance.
    pub fn note_utterance(&mut self) {
        self.total_utterances += 1;
    }

    /// Runs note utterance with errors.
    pub fn note_utterance_with_errors(&mut self) {
        self.utterances_with_alignment_errors += 1;
    }

    /// Runs note alignment errors.
    pub fn note_alignment_errors(&mut self, count: usize) {
        self.total_alignment_errors += count;
    }

    /// Prints summary.
    pub fn print_summary(&self) {
        eprintln!("\n=== Alignment Validation Summary ===");
        eprintln!("Total files tested: {}", self.total_files);
        eprintln!(
            "Files with alignment errors: {}",
            self.files_with_alignment_errors
        );
        eprintln!("Total utterances: {}", self.total_utterances);
        eprintln!(
            "Utterances with alignment errors: {}",
            self.utterances_with_alignment_errors
        );
        eprintln!("Total alignment errors: {}", self.total_alignment_errors);
    }
}
