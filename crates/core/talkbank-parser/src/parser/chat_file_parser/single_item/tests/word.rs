//! Test module for word in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

#[path = "word/basic.rs"]
mod basic;
#[path = "word/categories.rs"]
mod categories;
#[path = "word/cleaned_text_markers.rs"]
mod cleaned_text_markers;
#[path = "word/cleaned_text_overlap.rs"]
mod cleaned_text_overlap;
#[path = "word/helpers.rs"]
mod helpers;
#[path = "word/shortening.rs"]
mod shortening;
#[path = "word/untranscribed.rs"]
mod untranscribed;
