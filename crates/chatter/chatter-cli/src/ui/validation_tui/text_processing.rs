//! CHAT text formatting for TUI source snippets.
//!
//! Bridges [`ChatTextProcessor`] from `talkbank-model` into `ratatui` styled spans.
//! Handles tab expansion to spaces, bullet character substitution (`\x15` → `•`),
//! and underline marker regions, while tracking the mapping from byte offsets to
//! display column positions so caret underlines align correctly even when tabs or
//! multi-byte characters shift the column layout.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use talkbank_model::chat_formatting::{ChatTextProcessor, DisplayEvent};

/// Process source line for display: strip markers, expand tabs, track positions, apply styling.
///
/// Returns (styled_spans, display_offset, display_length) where:
/// - styled_spans: Text with underline styling applied, tabs expanded, bullets rendered
/// - display_offset: Where the error span starts in display coordinates
/// - display_length: Width of error span in display coordinates
pub fn process_source_line_for_display(
    text: &str,
    span_start: usize,
    span_end: usize,
) -> (Vec<Span<'static>>, usize, usize) {
    let mut processor = ChatTextProcessor::new(text);
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut display_start: Option<usize> = None;
    let mut display_end: Option<usize> = None;

    // Helper to flush current text as a span
    let mut flush_span = |text: &mut String, processor: &ChatTextProcessor| {
        if !text.is_empty() {
            let span = if processor.is_underlined() {
                Span::styled(
                    text.clone(),
                    Style::default().add_modifier(Modifier::UNDERLINED),
                )
            } else {
                Span::raw(text.clone())
            };
            spans.push(span);
            text.clear();
        }
    };

    while let Some(event) = processor.next_event() {
        // Track span boundaries
        let char_pos = processor.char_pos();
        let display_pos = processor.display_pos();

        if char_pos == span_start && display_start.is_none() {
            display_start = Some(display_pos);
        }
        if char_pos == span_end && display_end.is_none() {
            display_end = Some(display_pos);
        }

        // Handle display events
        match event {
            DisplayEvent::Char(ch) => {
                current_text.push(ch);
            }
            DisplayEvent::TabSpaces(n) => {
                for _ in 0..n {
                    current_text.push(' ');
                }
            }
            DisplayEvent::Bullet => {
                current_text.push('•');
            }
            DisplayEvent::UnderlineBegin => {
                flush_span(&mut current_text, &processor);
            }
            DisplayEvent::UnderlineEnd => {
                flush_span(&mut current_text, &processor);
            }
        }
    }

    // Check for span boundaries at end
    let final_char_pos = processor.char_pos();
    let final_display_pos = processor.display_pos();
    if final_char_pos == span_start && display_start.is_none() {
        display_start = Some(final_display_pos);
    }
    if final_char_pos == span_end && display_end.is_none() {
        display_end = Some(final_display_pos);
    }

    // Flush any remaining text
    flush_span(&mut current_text, &processor);

    // Ensure we have at least one span
    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }

    let offset = display_start.unwrap_or_default();
    // DEFAULT: When no display start is found, begin at the first column.
    let end = match display_end {
        Some(value) => value,
        None => final_display_pos,
    };
    let length = end.saturating_sub(offset).max(1);

    (spans, offset, length)
}

/// Process CHAT formatted text for display (error messages, etc.).
///
/// Handles tabs, bullets, and underline markers, returning styled spans.
pub fn process_text_for_display(text: &str) -> Vec<Span<'static>> {
    let mut processor = ChatTextProcessor::new(text);
    let mut spans = Vec::new();
    let mut current_text = String::new();

    // Helper to flush current text as a span
    let mut flush_span = |text: &mut String, processor: &ChatTextProcessor| {
        if !text.is_empty() {
            let span = if processor.is_underlined() {
                Span::styled(
                    text.clone(),
                    Style::default().add_modifier(Modifier::UNDERLINED),
                )
            } else {
                Span::raw(text.clone())
            };
            spans.push(span);
            text.clear();
        }
    };

    while let Some(event) = processor.next_event() {
        match event {
            DisplayEvent::Char(ch) => {
                current_text.push(ch);
            }
            DisplayEvent::TabSpaces(n) => {
                for _ in 0..n {
                    current_text.push(' ');
                }
            }
            DisplayEvent::Bullet => {
                current_text.push('•');
            }
            DisplayEvent::UnderlineBegin | DisplayEvent::UnderlineEnd => {
                flush_span(&mut current_text, &processor);
            }
        }
    }

    // Flush any remaining text
    flush_span(&mut current_text, &processor);

    // Return at least one span
    if spans.is_empty() {
        vec![Span::raw(String::new())]
    } else {
        spans
    }
}
