//! TUI state management and navigation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::widgets::ListState;
use talkbank_model::{ClanHiddenLineError, ParseError, resolve_clan_location};
use thiserror::Error;

use super::models::FileErrors;
use crate::ui::Theme;

/// Scroll state for the error details pane.
///
/// Populated by [`super::rendering::render_error_details`] each frame and read
/// by scroll navigation methods. Not valid before the first render pass.
#[derive(Debug, Default)]
pub struct ScrollState {
    /// Vertical scroll offset (in lines).
    pub offset: u16,
    /// Height of the viewport (set during rendering).
    pub viewport_height: u16,
    /// Total number of rendered lines (set during rendering).
    pub total_lines: u16,
    /// Line index where each error starts in the flattened line list.
    pub error_line_starts: Vec<u16>,
}

/// Streaming validation progress counters.
#[derive(Debug, Default)]
pub struct ProgressState {
    /// Total files to validate (from Started event).
    pub total_files: usize,
    /// Files processed so far (incremented on each FileComplete).
    pub files_processed: usize,
    /// Throttled display count to reduce redraw noise.
    pub files_processed_display: usize,
    /// True until Started event arrives (file discovery phase).
    pub discovering: bool,
    /// Final snapshot counts from Finished event.
    pub final_valid_files: Option<usize>,
    pub final_invalid_files: Option<usize>,
    pub final_cache_hits: Option<usize>,
    pub final_cache_misses: Option<usize>,
}

/// Metrics returned by [`super::rendering::render_error_details`] each frame.
///
/// The caller writes these into [`TuiState::scroll`] so navigation methods
/// have up-to-date viewport geometry.
pub struct DetailMetrics {
    pub viewport_height: u16,
    pub total_lines: u16,
    pub error_line_starts: Vec<u16>,
}

/// TUI state.
pub struct TuiState {
    pub theme: Theme,
    pub files: Vec<FileErrors>,
    pub selected_file_idx: usize,
    pub selected_error_idx: usize,
    pub focus: Focus,
    pub file_list_state: ListState,
    pub error_list_state: ListState,
    pub progress: ProgressState,
    pub scroll: ScrollState,
    /// Transient status message shown in the footer (e.g., send2clan errors).
    /// Cleared on the next keypress.
    pub status_message: Option<String>,
}

/// Which pane has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    FileList,
    ErrorList,
}

impl TuiState {
    const PROGRESS_DRAW_STRIDE: usize = 50;

    /// Initialize TUI state from the first validation snapshot.
    ///
    /// If the incoming list is non-empty, both panes start with their first
    /// item selected so keyboard navigation is immediately active.
    pub fn new(files: Vec<FileErrors>, theme: Theme) -> Self {
        let mut file_list_state = ListState::default();
        if !files.is_empty() {
            file_list_state.select(Some(0));
        }

        let mut error_list_state = ListState::default();
        if !files.is_empty() && !files[0].errors.is_empty() {
            error_list_state.select(Some(0));
        }

        Self {
            theme,
            files,
            selected_file_idx: 0,
            selected_error_idx: 0,
            focus: Focus::FileList,
            file_list_state,
            error_list_state,
            progress: ProgressState {
                discovering: true,
                ..Default::default()
            },
            scroll: ScrollState::default(),
            status_message: None,
        }
    }

    /// Apply metrics from the latest render pass to the scroll state.
    pub fn apply_detail_metrics(&mut self, metrics: DetailMetrics) {
        self.scroll.viewport_height = metrics.viewport_height;
        self.scroll.total_lines = metrics.total_lines;
        self.scroll.error_line_starts = metrics.error_line_starts;

        // Clamp scroll offset to valid range
        let max_scroll = self
            .scroll
            .total_lines
            .saturating_sub(self.scroll.viewport_height);
        if self.scroll.offset > max_scroll {
            self.scroll.offset = max_scroll;
        }
    }

    /// Updates progress display.
    pub fn update_progress_display(&mut self, force: bool) {
        if force
            || (self.progress.files_processed - self.progress.files_processed_display)
                >= Self::PROGRESS_DRAW_STRIDE
        {
            self.progress.files_processed_display = self.progress.files_processed;
        }
    }

    /// Return the currently selected file entry, if any.
    pub fn current_file(&self) -> Option<&FileErrors> {
        self.files.get(self.selected_file_idx)
    }

    /// Return the currently selected error inside the selected file.
    pub fn current_error(&self) -> Option<&ParseError> {
        self.current_file()?.errors.get(self.selected_error_idx)
    }

    /// Count all errors across currently tracked files.
    pub fn total_errors(&self) -> usize {
        self.files.iter().map(|f| f.errors.len()).sum()
    }

    /// Return the number of files currently in the error list.
    pub fn total_files_with_errors(&self) -> usize {
        self.files.len()
    }

    pub fn move_up(&mut self) {
        match self.focus {
            Focus::FileList => {
                if self.selected_file_idx > 0 {
                    self.selected_file_idx -= 1;
                    self.selected_error_idx = 0;
                    self.file_list_state.select(Some(self.selected_file_idx));
                    self.error_list_state.select(Some(0));
                    self.scroll.offset = 0;
                }
            }
            Focus::ErrorList => {
                if self.selected_error_idx > 0 {
                    self.selected_error_idx -= 1;
                    self.error_list_state.select(Some(self.selected_error_idx));
                    self.scroll_to_selected_error();
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        match self.focus {
            Focus::FileList => {
                if self.selected_file_idx < self.files.len().saturating_sub(1) {
                    self.selected_file_idx += 1;
                    self.selected_error_idx = 0;
                    self.file_list_state.select(Some(self.selected_file_idx));
                    self.error_list_state.select(Some(0));
                    self.scroll.offset = 0;
                }
            }
            Focus::ErrorList => {
                if let Some(file) = self.current_file()
                    && self.selected_error_idx < file.errors.len().saturating_sub(1)
                {
                    self.selected_error_idx += 1;
                    self.error_list_state.select(Some(self.selected_error_idx));
                    self.scroll_to_selected_error();
                }
            }
        }
    }

    pub fn scroll_error_up(&mut self) {
        self.scroll.offset = self.scroll.offset.saturating_sub(1);
    }

    pub fn scroll_error_down(&mut self) {
        let max_scroll = self
            .scroll
            .total_lines
            .saturating_sub(self.scroll.viewport_height);
        if self.scroll.offset < max_scroll {
            self.scroll.offset += 1;
        }
    }

    pub fn scroll_error_page_up(&mut self) {
        let page = self.scroll.viewport_height.max(1);
        self.scroll.offset = self.scroll.offset.saturating_sub(page);
    }

    pub fn scroll_error_page_down(&mut self) {
        let page = self.scroll.viewport_height.max(1);
        let max_scroll = self
            .scroll
            .total_lines
            .saturating_sub(self.scroll.viewport_height);
        self.scroll.offset = (self.scroll.offset + page).min(max_scroll);
    }

    /// Auto-scroll so the selected error is visible.
    fn scroll_to_selected_error(&mut self) {
        if let Some(&start_line) = self.scroll.error_line_starts.get(self.selected_error_idx) {
            let viewport_end = self.scroll.offset + self.scroll.viewport_height;
            if start_line < self.scroll.offset {
                self.scroll.offset = start_line;
            } else if start_line >= viewport_end {
                self.scroll.offset = start_line;
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::FileList => Focus::ErrorList,
            Focus::ErrorList => Focus::FileList,
        };
    }

    /// Handle keybindings shared between the streaming and static event loops.
    ///
    /// Returns `true` if the key was consumed, `false` if the caller should
    /// handle it (e.g., quit, cancel, rerun).
    pub fn handle_common_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match (code, modifiers) {
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => self.move_up(),
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => self.move_down(),
            (KeyCode::Char('K'), KeyModifiers::SHIFT) if self.focus == Focus::ErrorList => {
                self.scroll_error_up();
            }
            (KeyCode::Char('J'), KeyModifiers::SHIFT) if self.focus == Focus::ErrorList => {
                self.scroll_error_down();
            }
            (KeyCode::PageUp, _) if self.focus == Focus::ErrorList => self.scroll_error_page_up(),
            (KeyCode::PageDown, _) if self.focus == Focus::ErrorList => {
                self.scroll_error_page_down();
            }
            (KeyCode::Tab, _) => self.toggle_focus(),
            (KeyCode::Enter, _) => {
                if let Err(e) = self.open_in_clan() {
                    self.status_message = Some(format!("Failed to open in CLAN: {}", e));
                }
            }
            _ => return false,
        }
        true
    }

    /// Open the currently selected error in CLAN.
    pub fn open_in_clan(&self) -> Result<(), OpenInClanError> {
        let error = self.current_error().ok_or(OpenInClanError::NoError)?;
        let file = self.current_file().ok_or(OpenInClanError::NoFile)?;

        let clan_loc = resolve_clan_location(&error.location, &file.source)?;

        send2clan::send_to_clan(
            30,
            file.path.to_str().ok_or(OpenInClanError::InvalidPath)?,
            clan_loc.line as i32,
            clan_loc.column as i32,
            Some(&error.message),
        )?;

        Ok(())
    }
}

/// Errors raised when forwarding the selected error location to CLAN.
#[derive(Debug, Error)]
pub enum OpenInClanError {
    #[error("No error selected")]
    NoError,
    #[error("No file selected")]
    NoFile,
    #[error("Invalid file path")]
    InvalidPath,
    #[error("{0}")]
    HiddenLine(#[from] ClanHiddenLineError),
    #[error("Failed to send to CLAN")]
    Send(#[from] send2clan::Error),
}
