//! Themeable color palette for the validation TUI.
//!
//! Provides dark and light presets plus user-customizable themes loaded from
//! `~/.config/chatter/theme.toml`. Missing fields fall back to the preset
//! defaults via `#[serde(default)]`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use std::path::PathBuf;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// Color theme for the validation TUI.
///
/// Each field maps to a semantic role in the UI. Themes are serialized via
/// ratatui's built-in `serde` support for [`Color`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Error codes and file list items.
    #[serde(default = "defaults::error")]
    pub error: Color,
    /// Line:column location labels.
    #[serde(default = "defaults::location")]
    pub location: Color,
    /// Source line numbers in context display.
    #[serde(default = "defaults::line_number")]
    pub line_number: Color,
    /// Error underline carets (`^^^`).
    #[serde(default = "defaults::caret")]
    pub caret: Color,
    /// Suggestion lightbulb hints.
    #[serde(default = "defaults::suggestion")]
    pub suggestion: Color,
    /// Border of the focused pane.
    #[serde(default = "defaults::focus_border")]
    pub focus_border: Color,
    /// Background of the selected item.
    #[serde(default = "defaults::selected_bg")]
    pub selected_bg: Color,
    /// Header when validation succeeded.
    #[serde(default = "defaults::header_ok")]
    pub header_ok: Color,
    /// Header summary when errors exist.
    #[serde(default = "defaults::header_err")]
    pub header_err: Color,
    /// Header during in-progress validation.
    #[serde(default = "defaults::header_progress")]
    pub header_progress: Color,
}

/// Default is the dark preset.
impl Default for Theme {
    /// Use the dark preset when no theme is specified.
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Dark terminal preset — the original palette.
    pub fn dark() -> Self {
        Self {
            error: Color::LightRed,
            location: Color::LightCyan,
            line_number: Color::LightBlue,
            caret: Color::LightRed,
            suggestion: Color::LightYellow,
            focus_border: Color::LightYellow,
            selected_bg: Color::DarkGray,
            header_ok: Color::LightGreen,
            header_err: Color::LightYellow,
            header_progress: Color::LightCyan,
        }
    }

    /// Light terminal preset — darker colors for readability on white/light backgrounds.
    pub fn light() -> Self {
        Self {
            error: Color::Red,
            location: Color::DarkGray,
            line_number: Color::Blue,
            caret: Color::Red,
            suggestion: Color::Rgb(160, 120, 0),
            focus_border: Color::Blue,
            selected_bg: Color::Rgb(200, 200, 230),
            header_ok: Color::Green,
            header_err: Color::Rgb(180, 100, 0),
            header_progress: Color::Blue,
        }
    }

    /// Load a theme based on the CLI choice.
    ///
    /// Resolution order:
    /// 1. If a preset is specified via `--theme`, use that preset as the base.
    /// 2. If `~/.config/chatter/theme.toml` exists, deserialize it on top of
    ///    the base (missing fields keep preset defaults via `#[serde(default)]`).
    /// 3. Otherwise, use the preset (or dark if no preset specified).
    pub fn load(preset: Option<ThemePreset>) -> Self {
        let base = match preset {
            Some(ThemePreset::Light) => Self::light(),
            Some(ThemePreset::Dark) | None => Self::dark(),
        };

        let config_path = Self::config_path();
        if config_path.is_file() {
            match std::fs::read_to_string(&config_path) {
                Ok(contents) => match toml::from_str::<Theme>(&contents) {
                    Ok(user_theme) => return user_theme,
                    Err(e) => {
                        eprintln!("Warning: Failed to parse {}: {}", config_path.display(), e);
                    }
                },
                Err(e) => {
                    eprintln!("Warning: Failed to read {}: {}", config_path.display(), e);
                }
            }
        }

        base
    }

    /// Path to the user theme configuration file.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chatter")
            .join("theme.toml")
    }
}

/// CLI-selectable theme presets.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ThemePreset {
    /// Dark terminal palette (default)
    Dark,
    /// Light terminal palette
    Light,
}

/// serde default helpers — these return the dark preset values so that
/// partially-specified TOML files fall back field-by-field.
mod defaults {
    use ratatui::style::Color;

    /// Default color for error code labels.
    pub fn error() -> Color {
        Color::LightRed
    }
    /// Default color for line:column location labels.
    pub fn location() -> Color {
        Color::LightCyan
    }
    /// Default color for source line numbers.
    pub fn line_number() -> Color {
        Color::LightBlue
    }
    /// Default color for caret underlines.
    pub fn caret() -> Color {
        Color::LightRed
    }
    /// Default color for suggestion hints.
    pub fn suggestion() -> Color {
        Color::LightYellow
    }
    /// Default color for the focused pane border.
    pub fn focus_border() -> Color {
        Color::LightYellow
    }
    /// Default background color for selected rows.
    pub fn selected_bg() -> Color {
        Color::DarkGray
    }
    /// Default color for success headers.
    pub fn header_ok() -> Color {
        Color::LightGreen
    }
    /// Default color for error-summary headers.
    pub fn header_err() -> Color {
        Color::LightYellow
    }
    /// Default color for in-progress validation headers.
    pub fn header_progress() -> Color {
        Color::LightCyan
    }
}
