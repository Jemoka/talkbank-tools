//! TUI rendering for the two-pane validation error browser.
//!
//! Draws four regions using `ratatui`: a header with progress gauge, a file list
//! (left pane), an error details panel (right pane), and a footer with keybinding
//! hints. The error details panel renders miette-style source snippets with caret
//! underlines, line numbers, and suggestion annotations.
//!
//! Two variants exist: streaming (used during directory validation with a progress
//! gauge and cancel support) and static (used after single-file validation with
//! rerun support).

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use super::state::{DetailMetrics, Focus, TuiState};
use super::text_processing::{process_source_line_for_display, process_text_for_display};
use talkbank_model::SourceLocation;

/// Render header for streaming validation.
pub fn render_header_streaming(f: &mut Frame, area: Rect, state: &TuiState, complete: bool) {
    let title = if complete {
        let total = state.progress.total_files;
        let invalid = state
            .progress
            .final_invalid_files
            .unwrap_or_else(|| state.total_files_with_errors());
        format!("Done | {} files with errors / {} files", invalid, total)
    } else if state.progress.discovering {
        "Discovering files...".to_string()
    } else {
        "Validating...".to_string()
    };

    let color = if complete {
        if state.files.is_empty() {
            state.theme.header_ok
        } else {
            state.theme.header_err
        }
    } else {
        state.theme.header_progress
    };

    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let header = Paragraph::new(title)
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    f.render_widget(header, rows[0]);

    let ratio = if state.progress.total_files > 0 {
        (state.progress.files_processed_display as f64 / state.progress.total_files as f64)
            .clamp(0.0, 1.0)
    } else if complete {
        1.0
    } else {
        0.0
    };
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label("");
    f.render_widget(gauge, rows[1]);
}

/// Render header for static validation.
pub fn render_header(f: &mut Frame, area: Rect, state: &TuiState) {
    let title = format!(
        " Validation Errors - {} errors in {} files ",
        state.total_errors(),
        state.total_files_with_errors()
    );

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(state.theme.header_err)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

/// Render file list pane.
pub fn render_file_list(f: &mut Frame, area: Rect, state: &mut TuiState) {
    let error_color = state.theme.error;
    let focus_border = state.theme.focus_border;
    let selected_bg = state.theme.selected_bg;

    let items: Vec<ListItem> = state
        .files
        .iter()
        .map(|file| {
            let path_str = file.path.display().to_string();
            let error_count = file.errors.len();
            let line = format!("{} ({}) ✗", path_str, error_count);

            ListItem::new(line).style(Style::default().fg(error_color))
        })
        .collect();

    let title = if state.focus == Focus::FileList {
        " Files — Tab to view errors → "
    } else {
        " Files — Tab → "
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(if state.focus == Focus::FileList {
                    Style::default().fg(focus_border)
                } else {
                    Style::default()
                }),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(selected_bg),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut state.file_list_state);
}

/// Render error details pane with line-level scrolling.
///
/// Uses `Paragraph` with `.scroll()` instead of `List` so that tall error
/// items (e.g., mor 1:1 alignment tables) can be scrolled line-by-line
/// rather than being clipped at the viewport boundary.
pub fn render_error_details(
    f: &mut Frame,
    area: Rect,
    state: &mut TuiState,
) -> Option<DetailMetrics> {
    let filename = match state.current_file() {
        Some(file) => file.path.display().to_string(),
        None => "No file".to_string(),
    };
    let title = if state.focus == Focus::ErrorList {
        format!(" Errors in {} — ↵ Enter to open in CLAN ", filename)
    } else {
        format!(" Errors in {} — Tab to focus & scroll ", filename)
    };

    let has_file = state.current_file().is_some();
    let is_focused = state.focus == Focus::ErrorList;

    let color_error = state.theme.error;
    let color_location = state.theme.location;
    let color_line_num = state.theme.line_number;
    let color_caret = state.theme.caret;
    let color_suggestion = state.theme.suggestion;
    let color_focus_border = state.theme.focus_border;
    let color_selected_bg = state.theme.selected_bg;

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if is_focused {
            Style::default().fg(color_focus_border)
        } else {
            Style::default()
        });

    if !has_file {
        let no_file = Paragraph::new("No file selected")
            .block(block)
            .wrap(Wrap { trim: false });
        f.render_widget(no_file, area);
        return None;
    }

    let file_idx = state.selected_file_idx;
    let selected_error = state.selected_error_idx;

    // Build a flat list of Line objects for all errors, tracking where each
    // error starts so we can highlight the selected one and auto-scroll.
    let mut all_lines: Vec<Line> = Vec::new();
    let mut error_line_starts: Vec<u16> = Vec::new();

    if let Some(file) = state.files.get(file_idx) {
        for (error_idx, error) in file.errors.iter().enumerate() {
            let is_selected = error_idx == selected_error;
            // Style applied to all lines belonging to this error for selection highlight
            let bg_style = if is_selected {
                Style::default().bg(color_selected_bg)
            } else {
                Style::default()
            };

            error_line_starts.push((all_lines.len()).min(u16::MAX as usize) as u16);

            let code = error.code.as_str();
            let message = &error.message;
            let (line, column) = error_line_column(error, &file.source);

            let message_lines: Vec<&str> = message.split('\n').collect();

            // Selection indicator + error code prefix
            let prefix = if is_selected && is_focused {
                "↵ "
            } else {
                "  "
            };
            let mut first_line_spans = vec![
                Span::raw(prefix),
                Span::styled(
                    format!("[{}]", code),
                    Style::default()
                        .fg(color_error)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ];
            if let Some(first) = message_lines.first() {
                first_line_spans.extend(process_text_for_display(first));
            }
            all_lines.push(Line::from(first_line_spans).style(bg_style));

            // Subsequent message lines
            for msg_line in message_lines.iter().skip(1) {
                let mut line_spans = vec![Span::raw("    ")];
                line_spans.extend(process_text_for_display(msg_line));
                all_lines.push(Line::from(line_spans).style(bg_style));
            }

            // Location line
            all_lines.push(
                Line::from(vec![Span::styled(
                    format!("    Line {}:{}", line, column),
                    Style::default().fg(color_location),
                )])
                .style(bg_style),
            );

            // Source context (miette-style)
            if let Some(ctx) = &error.context
                && !ctx.source_text.is_empty()
            {
                let context_line = match ctx.line_offset {
                    Some(offset) => offset,
                    None => line,
                };

                let span_start = ctx.span.start as usize;
                let span_end = ctx.span.end as usize;
                let source_lines: Vec<&str> = ctx.source_text.split('\n').collect();

                all_lines.push(Line::from(""));

                let mut byte_offset = 0usize;
                for (i, src_line) in source_lines.iter().enumerate() {
                    let line_start = byte_offset;
                    let line_end = byte_offset + src_line.len();
                    let line_num = context_line + i;

                    let line_span_start = span_start.max(line_start).saturating_sub(line_start);
                    let line_span_end = span_end.min(line_end).saturating_sub(line_start);

                    let (display_spans, display_offset, display_length) =
                        process_source_line_for_display(src_line, line_span_start, line_span_end);

                    let line_num_prefix = format!("    {} ", line_num);
                    let mut source_spans = vec![Span::styled(
                        format!("{}│ ", line_num_prefix),
                        Style::default().fg(color_line_num),
                    )];
                    source_spans.extend(display_spans);
                    all_lines.push(Line::from(source_spans));

                    if span_start < line_end && span_end > line_start {
                        let line_num_display_width = line_num_prefix.width();
                        let spaces = " ".repeat(display_offset);
                        let carets = "^".repeat(display_length.max(1));

                        all_lines.push(Line::from(vec![
                            Span::raw(" ".repeat(line_num_display_width)),
                            Span::raw("│ "),
                            Span::styled(
                                format!("{}{}", spaces, carets),
                                Style::default()
                                    .fg(color_caret)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }

                    byte_offset = line_end + 1;
                }
            }

            // Suggestion
            if let Some(ref suggestion) = error.suggestion {
                all_lines.push(Line::from(vec![
                    Span::styled("    💡 ", Style::default().fg(color_suggestion)),
                    Span::raw(suggestion),
                ]));
            }

            // Blank separator between errors
            all_lines.push(Line::from(""));
        }
    }

    let inner = block.inner(area);
    let viewport_height = inner.height;
    let total_lines = (all_lines.len()).min(u16::MAX as usize) as u16;

    // Clamp scroll offset before rendering
    let max_scroll = total_lines.saturating_sub(viewport_height);
    let scroll_offset = state.scroll.offset.min(max_scroll);

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .scroll((scroll_offset, 0));

    f.render_widget(paragraph, area);

    // Return metrics so the caller can update scroll state
    Some(DetailMetrics {
        viewport_height,
        total_lines,
        error_line_starts,
    })
}

/// Resolve line and column for an error, computing from byte span when needed.
fn error_line_column(error: &talkbank_model::ParseError, source: &str) -> (usize, usize) {
    match (error.location.line, error.location.column) {
        (Some(line), Some(column)) => (line, column),
        _ => SourceLocation::calculate_line_column(error.location.span.start as usize, source),
    }
}

/// Build the prominent action row that tells users about Enter → CLAN,
/// or shows a transient status message (e.g., send2clan error).
///
/// Row 1 of the two-row footer. Bright colors, bold text so it can't be missed.
fn render_footer_action_row(f: &mut Frame, area: Rect, state: &TuiState) {
    // Show status message (e.g., send2clan failure) instead of the action hint
    if let Some(ref msg) = state.status_message {
        let status_line = Line::from(vec![Span::styled(
            msg.as_str(),
            Style::default()
                .fg(state.theme.error)
                .add_modifier(Modifier::BOLD),
        )]);
        let status_bar = Paragraph::new(status_line).alignment(Alignment::Center);
        f.render_widget(status_bar, area);
        return;
    }

    let action_line = Line::from(vec![
        Span::styled(
            " ↵ Enter ",
            Style::default()
                .fg(state.theme.focus_border)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        ),
        Span::raw("  "),
        Span::styled(
            "Jump to selected error in CLAN",
            Style::default()
                .fg(state.theme.header_ok)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let action_bar = Paragraph::new(action_line).alignment(Alignment::Center);
    f.render_widget(action_bar, area);
}

/// Build the secondary navigation hints row.
///
/// Row 2 of the two-row footer. The Tab hint is context-sensitive: it describes
/// the specific destination so users know exactly what will happen.
fn render_footer_nav_row(
    f: &mut Frame,
    area: Rect,
    state: &TuiState,
    extra_hints: &[(&str, &str)],
) {
    let tab_label = match state.focus {
        Focus::FileList => ": Move to error list →  ",
        Focus::ErrorList => ": ← Back to file list  ",
    };

    let mut spans = vec![
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(tab_label),
        Span::styled("j/k", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Navigate  "),
        Span::styled("J/K", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Scroll  "),
    ];

    for (key, label) in extra_hints {
        spans.push(Span::styled(
            key.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(label.to_string()));
    }

    let nav_bar = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    f.render_widget(nav_bar, area);
}

/// Render footer for streaming validation.
pub fn render_footer_streaming(
    f: &mut Frame,
    area: Rect,
    state: &TuiState,
    validation_complete: bool,
) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    render_footer_action_row(f, rows[0], state);

    if validation_complete {
        render_footer_nav_row(
            f,
            rows[1],
            state,
            &[("r", ": Rerun  "), ("q/Esc", ": Quit")],
        );
    } else {
        render_footer_nav_row(
            f,
            rows[1],
            state,
            &[("c/Ctrl+C", ": Cancel  "), ("q/Esc", ": Quit")],
        );
    }
}

/// Render footer for static validation.
pub fn render_footer(f: &mut Frame, area: Rect, state: &TuiState) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    render_footer_action_row(f, rows[0], state);
    render_footer_nav_row(
        f,
        rows[1],
        state,
        &[("r", ": Rerun  "), ("q/Esc", ": Quit")],
    );
}
