//! Re-exports `talkbank_transform::morphosyntax`. New consumers should
//! import from `talkbank_transform` directly.

pub mod l2;

#[cfg(test)]
mod tests;

pub use talkbank_transform::morphosyntax::*;
