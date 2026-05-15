//! CHAT text normalization for diagnostic rendering.
//!
//! This module converts raw CHAT lines into display-friendly text while keeping
//! offset mappings so diagnostics can still point to the right source region.
//!
//! Normalization rules:
//! - Tabs expand to spaces at 8-column tab stops.
//! - Media bullet delimiters (`\u{0015}`) render as `•`.
//! - Underline control-marker pairs are removed from plain output.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Special_Markers>
///
/// Stateful processor that yields display events from CHAT control text.
///
/// This iterator handles:
/// - Tab expansion to 8-column boundaries
/// - Bullet delimiter rendering (\u0015 -> '•')
/// - Underline marker tracking (\u0002\u0001 begin, \u0002\u0002 end)
///
/// Consumers can use this to build styled output (TUI) or plain text (miette).
pub struct ChatTextProcessor<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    char_pos: usize,     // Byte offset in original text
    display_pos: usize,  // Byte offset in display output
    is_underlined: bool, // Current underline state
}

/// A display event produced by [`ChatTextProcessor`] when processing CHAT text.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayEvent {
    /// Regular character to display
    Char(char),
    /// Spaces from tab expansion (count)
    TabSpaces(usize),
    /// Bullet character
    Bullet,
    /// Start underlined region
    UnderlineBegin,
    /// End underlined region
    UnderlineEnd,
}

impl<'a> ChatTextProcessor<'a> {
    /// Create a new processor for the given CHAT text.
    pub fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars().peekable(),
            char_pos: 0,
            display_pos: 0,
            is_underlined: false,
        }
    }

    /// Current byte offset in original text.
    pub fn char_pos(&self) -> usize {
        self.char_pos
    }

    /// Current byte offset in display output.
    pub fn display_pos(&self) -> usize {
        self.display_pos
    }

    /// Whether we're currently in an underlined region
    pub fn is_underlined(&self) -> bool {
        self.is_underlined
    }

    /// Process the next CHAT character and return one normalized display event.
    ///
    /// Offsets exposed by [`Self::char_pos`] and [`Self::display_pos`] are
    /// advanced in UTF-8 bytes.
    pub fn next_event(&mut self) -> Option<DisplayEvent> {
        let ch = self.chars.next()?;
        let ch_len = ch.len_utf8();

        // Handle underline markers
        if ch == '\u{0002}'
            && let Some(&next_ch) = self.chars.peek()
        {
            if next_ch == '\u{0001}' {
                // UNDERLINE_BEGIN
                self.chars.next(); // consume \u{0001}
                self.char_pos += ch_len + next_ch.len_utf8();
                self.is_underlined = true;
                return Some(DisplayEvent::UnderlineBegin);
            } else if next_ch == '\u{0002}' {
                // UNDERLINE_END
                self.chars.next(); // consume second \u{0002}
                self.char_pos += ch_len + next_ch.len_utf8();
                self.is_underlined = false;
                return Some(DisplayEvent::UnderlineEnd);
            }
        }

        // Handle special characters
        let event = match ch {
            '\t' => {
                let spaces_to_add = 8 - (self.display_pos % 8);
                self.display_pos += spaces_to_add;
                DisplayEvent::TabSpaces(spaces_to_add)
            }
            '\u{0015}' => {
                self.display_pos += '•'.len_utf8();
                DisplayEvent::Bullet
            }
            _ => {
                self.display_pos += ch_len;
                DisplayEvent::Char(ch)
            }
        };

        self.char_pos += ch_len;
        Some(event)
    }
}

use tracing::warn;

/// Result of processing CHAT text for plain display.
///
/// Contains normalized display text and a mapping from original UTF-8 byte
/// offsets to display UTF-8 byte offsets.
pub struct PlainDisplayResult {
    /// Formatted text with tabs expanded, bullets rendered, markers removed
    pub text: String,
    /// Sorted list of `(original_byte_offset, display_byte_offset)` breakpoints.
    /// Use [`Self::map_offset`] to look up a display position.
    offset_map: Vec<(usize, usize)>,
}

impl PlainDisplayResult {
    /// Map one original byte offset to the corresponding display byte offset.
    ///
    /// Uses binary search on the breakpoint table built during processing.
    pub fn map_offset(&self, original: usize) -> usize {
        match self
            .offset_map
            .binary_search_by_key(&original, |&(orig, _)| orig)
        {
            Ok(i) => self.offset_map[i].1,
            Err(0) => 0,
            Err(i) => {
                // Interpolate linearly between breakpoints.
                let (prev_orig, prev_disp) = self.offset_map[i - 1];
                prev_disp + (original - prev_orig)
            }
        }
    }

    /// Map an original (start, end) span to display coordinates, ensuring minimum span width of 1.
    pub fn map_span(&self, start: usize, end: usize) -> (usize, usize) {
        let ds = self.map_offset(start);
        let de = self.map_offset(end);
        (ds, de.max(ds + 1))
    }
}

/// Process CHAT text into normalized display text and a reusable byte-offset map.
///
/// Single pass: builds output text and records offset breakpoints.
pub fn process_for_plain_display_mapped(text: &str) -> PlainDisplayResult {
    let mut processor = ChatTextProcessor::new(text);
    let mut display = String::with_capacity(text.len() * 2);
    let mut offset_map: Vec<(usize, usize)> = Vec::new();

    // Record initial position
    offset_map.push((0, 0));

    while let Some(event) = processor.next_event() {
        let char_pos = processor.char_pos();
        let display_pos = processor.display_pos();

        match event {
            DisplayEvent::Char(ch) => display.push(ch),
            DisplayEvent::TabSpaces(n) => {
                for _ in 0..n {
                    display.push(' ');
                }
            }
            DisplayEvent::Bullet => display.push('•'),
            DisplayEvent::UnderlineBegin | DisplayEvent::UnderlineEnd => {
                // Don't add anything to plain text — but record the position
                // shift (original bytes consumed, display position unchanged)
            }
        }

        // Record breakpoint whenever char_pos and display_pos diverge from
        // a simple 1:1 mapping (tabs, markers, bullets change the ratio)
        offset_map.push((char_pos, display_pos));
    }

    // Deduplicate consecutive entries with same original offset (keep last)
    offset_map.dedup_by_key(|entry| entry.0);

    PlainDisplayResult {
        text: display,
        offset_map,
    }
}

/// Convert CHAT text to normalized display text and one mapped span.
///
/// Returns `(display_text, display_start, display_end)` where:
/// - display_text: formatted text with tabs expanded, bullets rendered, markers removed
/// - display_start/end: UTF-8 byte offsets in `display_text`
pub fn process_for_plain_display(
    text: &str,
    span_start: usize,
    span_end: usize,
) -> (String, usize, usize) {
    let mut processor = ChatTextProcessor::new(text);
    let mut display = String::with_capacity(text.len() * 2);
    let mut display_start: Option<usize> = None;
    let mut display_end: Option<usize> = None;

    // Check for span at position 0 before processing any events
    if span_start == 0 && display_start.is_none() {
        display_start = Some(processor.display_pos());
    }
    if span_end == 0 && display_end.is_none() {
        display_end = Some(processor.display_pos());
    }

    while let Some(event) = processor.next_event() {
        // Track span boundaries after processing event
        let char_pos = processor.char_pos();
        let current_display_pos = processor.display_pos();

        // Check if we just passed span boundaries
        if char_pos == span_start && display_start.is_none() {
            display_start = Some(current_display_pos);
        }
        if char_pos == span_end && display_end.is_none() {
            display_end = Some(current_display_pos);
        }

        // Append to output (ignore underline markers for plain text)
        match event {
            DisplayEvent::Char(ch) => display.push(ch),
            DisplayEvent::TabSpaces(n) => {
                for _ in 0..n {
                    display.push(' ');
                }
            }
            DisplayEvent::Bullet => display.push('•'),
            DisplayEvent::UnderlineBegin | DisplayEvent::UnderlineEnd => {
                // Don't add anything to plain text
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

    let offset = match display_start {
        Some(offset) => offset,
        None => {
            warn!(span_start, "Missing display start; defaulting to 0");
            0
        }
    };
    let end = match display_end {
        Some(end) => end,
        None => {
            warn!(
                span_end,
                final_display_pos, "Missing display end; defaulting to end of display"
            );
            final_display_pos
        }
    };

    (display, offset, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests processor tabs.
    #[test]
    fn test_processor_tabs() {
        let mut proc = ChatTextProcessor::new("a\tb");

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('a')));
        assert_eq!(proc.display_pos(), 1);

        assert_eq!(proc.next_event(), Some(DisplayEvent::TabSpaces(7))); // 8 - 1 = 7
        assert_eq!(proc.display_pos(), 8);

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('b')));
        assert_eq!(proc.display_pos(), 9);
    }

    /// Tests processor bullet.
    #[test]
    fn test_processor_bullet() {
        let mut proc = ChatTextProcessor::new("a\u{0015}b");

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('a')));
        assert_eq!(proc.next_event(), Some(DisplayEvent::Bullet));
        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('b')));
    }

    /// Tests processor underline.
    #[test]
    fn test_processor_underline() {
        let mut proc = ChatTextProcessor::new("a\u{0002}\u{0001}b\u{0002}\u{0002}c");

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('a')));
        assert!(!proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::UnderlineBegin));
        assert!(proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('b')));
        assert!(proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::UnderlineEnd));
        assert!(!proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('c')));
        assert!(!proc.is_underlined());
    }

    /// Tests plain display with underline.
    #[test]
    fn test_plain_display_with_underline() {
        let text = "hello \u{0002}\u{0001}world\u{0002}\u{0002} bad";
        let span_start = 16; // "bad"
        let span_end = 19;

        let (display, offset, end) = process_for_plain_display(text, span_start, span_end);

        assert_eq!(display, "hello world bad");
        assert_eq!(offset, 12); // "hello world " = 12 chars
        assert_eq!(end, 15); // "bad" ends at 15
    }

    /// Tests plain display with tab.
    #[test]
    fn test_plain_display_with_tab() {
        let text = "word\tbad";
        let span_start = 5; // "bad"
        let span_end = 8;

        let (display, offset, end) = process_for_plain_display(text, span_start, span_end);

        assert_eq!(display, "word    bad"); // tab expands to 4 spaces (8 - 4 = 4)
        assert_eq!(offset, 8);
        assert_eq!(end, 11);
    }

    /// Span mapping uses UTF-8 byte offsets for non-ASCII input.
    #[test]
    fn test_plain_display_utf8_span_mapping() {
        let text = "é\t日";
        let span_start = 3; // "日" starts after "é"(2 bytes) + tab(1 byte)
        let span_end = 6; // "日" is 3 bytes

        let (display, offset, end) = process_for_plain_display(text, span_start, span_end);

        assert_eq!(display, "é      日");
        assert_eq!(offset, 8); // "é"(2 bytes) + six spaces
        assert_eq!(end, 11); // plus "日"(3 bytes)
    }

    /// Bullet replacement expands one-byte CHAT delimiter to a three-byte glyph.
    #[test]
    fn test_plain_display_bullet_byte_mapping() {
        let text = "a\u{0015}b";
        let span_start = 2; // "b"
        let span_end = 3;

        let (display, offset, end) = process_for_plain_display(text, span_start, span_end);

        assert_eq!(display, "a•b");
        assert_eq!(offset, 4); // "a"(1) + "•"(3)
        assert_eq!(end, 5);
    }
}
