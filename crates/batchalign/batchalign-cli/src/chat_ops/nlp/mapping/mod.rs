//! UD-to-CHAT morphosyntax mapping. Re-exports
//! `talkbank_transform::morphosyntax`. New consumers should import from
//! `talkbank_transform` directly.

#[cfg(test)]
mod tests;

pub use talkbank_transform::morphosyntax::*;
