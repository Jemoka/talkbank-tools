//! Comprehensive alignment corpus tests
//!
//! This test suite validates the alignment facility against a corpus of
//! deliberately crafted CHAT files with both correct alignments (happy path)
//! and various alignment errors (sad path).

#[path = "alignment_corpus_tests/happy_path.rs"]
mod happy_path;
#[path = "alignment_corpus_tests/helpers.rs"]
mod helpers;
#[path = "alignment_corpus_tests/sad_path.rs"]
mod sad_path;
#[path = "alignment_corpus_tests/summary.rs"]
mod summary;
#[path = "test_utils/mod.rs"]
mod test_utils;
