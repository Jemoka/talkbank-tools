//! UI module exports shared by validation and watch commands.
//!
//! These helpers centralize TUI state (`Theme`, file error models, actions, stream helpers)
//! so terminal tooling doesn’t duplicate layout logic. `validation_tui` exposes the same
//! data structures for interactive error browsing and streaming progress.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod theme;
pub mod validation_tui;

pub use theme::{Theme, ThemePreset};
pub use validation_tui::{FileErrors, TuiAction, run_validation_tui, run_validation_tui_streaming};
