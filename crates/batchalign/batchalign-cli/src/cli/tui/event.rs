//! Terminal event polling bridge between crossterm and the TUI event loop.
//!
//! The TUI render loop alternates between polling for user input and polling
//! the server for job status updates. This module wraps crossterm's raw event
//! stream into a simplified [`TuiEvent`] enum so the main loop does not need
//! to handle mouse events, focus events, or other crossterm-specific details
//! that the batchalign TUI does not use.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};

/// Simplified event type for TUI input handling.
///
/// Only the event types that the batchalign TUI actually responds to are
/// represented here. All other crossterm events (mouse, focus, paste) are
/// silently dropped by [`poll_event`].
pub enum TuiEvent {
    /// A key was pressed. Emitted on every key-down event. The TUI main
    /// loop matches on `KeyCode` for navigation (arrows, Tab) and actions
    /// (`e` to toggle errors, `c` to cancel, `q`/Esc to quit).
    /// `KeyModifiers` is included so Ctrl+C can be distinguished from
    /// a plain `c` press.
    Key(KeyCode, KeyModifiers),

    /// The terminal was resized. Emitted by the terminal emulator when the
    /// user drags the window border. The two values are the new width and
    /// height in columns and rows respectively. The TUI re-renders
    /// immediately after receiving this event to avoid layout artifacts.
    Resize(u16, u16),
}

/// Poll for a TUI event with the given timeout.
///
/// Returns `None` if no event is available within the timeout.
pub fn poll_event(timeout: Duration) -> Option<TuiEvent> {
    if event::poll(timeout).ok()? {
        match event::read().ok()? {
            Event::Key(key) => Some(TuiEvent::Key(key.code, key.modifiers)),
            Event::Resize(w, h) => Some(TuiEvent::Resize(w, h)),
            _ => None,
        }
    } else {
        None
    }
}
