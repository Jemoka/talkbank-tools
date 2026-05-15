//! Progress bar throttling utilities.
//!
//! Centralizes a small but performance-sensitive behavior: progress bars should
//! provide feedback without dominating runtime on large cached runs. Uses a
//! fixed refresh rate and a coarse stride to keep updates O(1) per file while
//! still surfacing failures promptly.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// Throttled progress bar wrapper with optional disable-on-error behavior.
#[derive(Debug)]
pub struct ProgressThrottle {
    bar: ProgressBar,
    draw_stride: u64,
    last_message_at: u64,
    disabled: bool,
}

impl ProgressThrottle {
    /// Create a throttled progress bar.
    #[must_use]
    pub fn new(total: u64, template: &str, chars: &str, draw_hz: u8, draw_stride: u64) -> Self {
        let bar = ProgressBar::new(total);
        let style = ProgressStyle::default_bar()
            .template(template)
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars(chars);
        bar.set_style(style);
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(draw_hz.max(1)));

        Self {
            bar,
            draw_stride: draw_stride.max(1),
            last_message_at: 0,
            disabled: false,
        }
    }

    /// Return a reference to the underlying progress bar.
    pub fn bar(&self) -> &ProgressBar {
        &self.bar
    }

    /// Update the bar length.
    pub fn set_length(&self, len: u64) {
        if !self.disabled {
            self.bar.set_length(len);
        }
    }

    /// Increment the bar by `delta`.
    pub fn inc(&self, delta: u64) {
        if !self.disabled {
            self.bar.inc(delta);
        }
    }

    /// Set the message, throttled by stride unless `force` is true.
    pub fn set_message_throttled(&mut self, message: impl Into<String>, force: bool) {
        if self.disabled {
            return;
        }
        let pos = self.bar.position();
        if force || pos.saturating_sub(self.last_message_at) >= self.draw_stride {
            let msg: String = message.into();
            self.bar.set_message(msg);
            self.last_message_at = pos;
        }
    }

    /// Disable the progress bar and clear it from the terminal.
    pub fn disable_and_clear(&mut self) {
        if !self.disabled {
            self.bar.finish_and_clear();
            self.disabled = true;
        }
    }

    /// Finish the progress bar and clear it from the terminal.
    pub fn finish_and_clear(&mut self) {
        self.disable_and_clear();
    }
}
