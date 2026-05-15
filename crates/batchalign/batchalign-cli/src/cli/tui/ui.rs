//! Ratatui rendering — layout, widgets, colors.

use std::time::SystemTime;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};

use crate::api::{FileProgressStage, FileStatusKind};

use super::app::AppState;

/// Braille spinner characters.
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// 5-phase pipeline labels mirroring the React `PipelineStageBar`.
/// Single-character labels for compact display: R=Read, T=Transcribe, A=Align, M=Morphosyntax/Analyze, F=Finalize.
const PHASE_LABELS: &[&str] = &["R", "T", "A", "M", "F"];

/// Map a `FileProgressStage` to a 0-4 phase index, matching the React
/// `PipelineStageBar` phase grouping. Returns `None` for `Processing`
/// (generic fallback) and `RetryScheduled`.
fn phase_index(stage: FileProgressStage) -> Option<usize> {
    match stage {
        // Phase 0: Read
        FileProgressStage::Reading
        | FileProgressStage::ResolvingAudio
        | FileProgressStage::CheckingCache
        | FileProgressStage::Parsing => Some(0),
        // Phase 1: Transcribe
        FileProgressStage::Transcribing
        | FileProgressStage::RecoveringUtteranceTiming
        | FileProgressStage::RecoveringTimingFallback => Some(1),
        // Phase 2: Align
        FileProgressStage::Aligning | FileProgressStage::ApplyingResults => Some(2),
        // Phase 3: Analyze
        FileProgressStage::AnalyzingMorphosyntax
        | FileProgressStage::SegmentingUtterances
        | FileProgressStage::Translating
        | FileProgressStage::ResolvingCoreference
        | FileProgressStage::Segmenting
        | FileProgressStage::Analyzing
        | FileProgressStage::Comparing
        | FileProgressStage::Benchmarking => Some(3),
        // Phase 4: Finalize
        FileProgressStage::PostProcessing
        | FileProgressStage::BuildingChat
        | FileProgressStage::Finalizing
        | FileProgressStage::Writing => Some(4),
        // No phase mapping for generic/retry
        FileProgressStage::Processing | FileProgressStage::RetryScheduled => None,
    }
}

/// Render a compact 5-dot phase indicator: `●●◐○○` style.
///
/// Completed phases are filled (`●`), active phase is highlighted,
/// future phases are hollow (`○`).
fn render_phase_dots(stage: Option<FileProgressStage>) -> Vec<Span<'static>> {
    let active = stage.and_then(phase_index);
    let mut spans = Vec::with_capacity(PHASE_LABELS.len());
    for i in 0..PHASE_LABELS.len() {
        let (ch, color) = match active {
            Some(idx) if i < idx => ('●', Color::Green),
            Some(idx) if i == idx => ('●', Color::Cyan),
            _ => ('○', Color::DarkGray),
        };
        spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
    }
    spans
}

/// Draw the full TUI frame.
pub fn draw(f: &mut Frame, state: &AppState) {
    let area = f.area();

    // Determine error panel height
    let error_height = if state.errors.entries.is_empty() {
        0
    } else if state.errors.expanded {
        (state.errors.entries.len() as u16 + 2).min(8)
    } else {
        1
    };

    // Metrics rows: 2 lines (workers + memory) when visible and health data available
    let metrics_height = if state.show_metrics && state.health.is_some() {
        2
    } else {
        0
    };

    let chunks = Layout::vertical([
        Constraint::Length(3),              // header + gauge
        Constraint::Length(metrics_height), // worker + memory lines
        Constraint::Min(4),                 // directory groups
        Constraint::Length(error_height),   // error summary
        Constraint::Length(1),              // keybind bar
    ])
    .split(area);

    draw_header(f, state, chunks[0]);
    if metrics_height > 0 {
        draw_metrics(f, state, chunks[1]);
    }
    draw_groups(f, state, chunks[2]);
    if error_height > 0 {
        draw_errors(f, state, chunks[3]);
    }
    draw_keybinds(f, state, chunks[4]);
}

/// Header: command badge + progress gauge + status breakdown + elapsed + ETA.
fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let elapsed = state.progress.start_time.elapsed();
    let mins = elapsed.as_secs() / 60;
    let secs = elapsed.as_secs() % 60;

    let completed = state.progress.completed;
    let total = state.progress.total_files;

    let ratio = if total > 0 {
        (completed as f64) / (total as f64)
    } else {
        0.0
    };

    // Status breakdown across all groups
    let (done, active, errors, queued) = state.directories.groups.iter().fold(
        (0usize, 0usize, 0usize, 0usize),
        |(d, a, e, q), g| {
            (
                d + g.done_count,
                a + g.active_count,
                e + g.error_count,
                q + g.queued_count,
            )
        },
    );

    let breakdown = if done + active + errors + queued > 0 {
        let mut parts = Vec::new();
        if done > 0 {
            parts.push(format!("{done}✓"));
        }
        if active > 0 {
            parts.push(format!("{active}⠋"));
        }
        if errors > 0 {
            parts.push(format!("{errors}✗"));
        }
        if queued > 0 {
            parts.push(format!("{queued}·"));
        }
        format!("  {}", parts.join(" "))
    } else {
        String::new()
    };

    // ETA or completion message. A cancelled job overrides the
    // generic "Done — N failed" with the explicit "CANCELLED"
    // marker plus the cancel source/host so the user can tell the
    // difference between natural completion and a cancel they
    // initiated.
    let suffix = if let Some(receipt) = &state.progress.cancelled_receipt {
        let host = receipt
            .host
            .as_deref()
            .filter(|h| !h.is_empty())
            .unwrap_or("(unknown host)");
        format!("  CANCELLED via {} from {}", receipt.source, host)
    } else if state.progress.finished {
        let error_count = state.errors.entries.len();
        if error_count > 0 {
            format!("  Done — {error_count} failed")
        } else {
            "  Done!".to_string()
        }
    } else if completed > 0 && completed < total {
        let elapsed_s = elapsed.as_secs_f64();
        let rate = completed as f64 / elapsed_s;
        let remaining = (total - completed) as f64 / rate;
        let rem_mins = remaining as u64 / 60;
        let rem_secs = remaining as u64 % 60;
        format!("  ~{rem_mins:02}:{rem_secs:02}")
    } else {
        String::new()
    };

    let label = format!(
        " {} — {completed}/{total} files{breakdown}  [{mins:02}:{secs:02}]{suffix}",
        state.progress.command,
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio.clamp(0.0, 1.0))
        .label(label);

    f.render_widget(gauge, area);
}

/// Directory groups — bordered sections with file rows.
fn draw_groups(f: &mut Frame, state: &AppState, area: Rect) {
    if state.directories.groups.is_empty() {
        let msg =
            Paragraph::new("  Waiting for server…").style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, area);
        return;
    }

    // Split area evenly across groups (up to what fits)
    let group_count = state
        .directories
        .groups
        .len()
        .min(area.height as usize / 3)
        .max(1);
    let constraints: Vec<Constraint> = (0..group_count).map(|_| Constraint::Min(3)).collect();
    let group_areas = Layout::vertical(constraints).split(area);

    for (i, group_area) in group_areas.iter().enumerate() {
        let group_idx = if state.directories.focused_group < group_count {
            i
        } else {
            // Scroll groups so focused is visible
            let start = state
                .directories
                .focused_group
                .saturating_sub(group_count - 1);
            start + i
        };

        if group_idx >= state.directories.groups.len() {
            break;
        }

        let group = &state.directories.groups[group_idx];
        let is_focused = group_idx == state.directories.focused_group;
        let all_terminal = group.active_count == 0 && group.queued_count == 0;

        let title = if all_terminal && !is_focused {
            // Collapsed summary for completed groups
            let check = if group.error_count > 0 {
                format!("{}✓ {}✗", group.done_count, group.error_count)
            } else {
                format!("{}✓", group.done_count)
            };
            format!(" {} ({check}) ", group.dir)
        } else {
            format!(
                " {} ({}/{}) ",
                group.dir,
                group.done_count + group.error_count,
                group.files.len()
            )
        };

        let border_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let inner = block.inner(*group_area);
        f.render_widget(block, *group_area);

        // File rows with scrolling and scroll indicators
        let visible_rows = inner.height as usize;
        let scroll = if is_focused {
            state.directories.scroll_offset
        } else {
            0
        };

        let has_above = scroll > 0;
        let has_below = group.files.len() > scroll + visible_rows;

        // Reserve rows for scroll indicators if needed
        let indicator_above = has_above && visible_rows > 2;
        let indicator_below = has_below && visible_rows > 2;
        let file_rows = visible_rows
            - if indicator_above { 1 } else { 0 }
            - if indicator_below { 1 } else { 0 };

        let mut items: Vec<ListItem> = Vec::new();

        if indicator_above {
            let hidden = scroll;
            items.push(ListItem::new(Line::from(Span::styled(
                format!("  ▲ {hidden} more above"),
                Style::default().fg(Color::DarkGray),
            ))));
        }

        for file in group.files.iter().skip(scroll).take(file_rows) {
            let line = render_file_line(file, state.directories.spinner_tick, inner.width);
            items.push(ListItem::new(line));
        }

        if indicator_below {
            let hidden = group.files.len() - scroll - file_rows;
            items.push(ListItem::new(Line::from(Span::styled(
                format!("  ▼ {hidden} more below"),
                Style::default().fg(Color::DarkGray),
            ))));
        }

        let list = List::new(items);
        f.render_widget(list, inner);
    }
}

/// Render a single file line with status glyph, name, progress info.
fn render_file_line(
    file: &super::app::FileState,
    spinner_tick: usize,
    width: u16,
) -> Line<'static> {
    let w = width as usize;

    match file.status {
        FileStatusKind::Queued | FileStatusKind::Interrupted => {
            let text = format!("  · {}", file.name);
            Line::from(Span::styled(
                pad_or_truncate(&text, w),
                Style::default().fg(Color::DarkGray),
            ))
        }
        FileStatusKind::Processing => {
            let spinner = SPINNER[spinner_tick % SPINNER.len()];
            let label = file.progress_label.as_deref().unwrap_or("");
            let pct = match (file.progress_current, file.progress_total) {
                (Some(c), Some(t)) if t > 0 => format!("  {c}/{t}"),
                _ => String::new(),
            };

            // Build line with pipeline phase dots when a typed stage exists
            let mut spans: Vec<Span<'static>> = Vec::new();
            spans.push(Span::styled(
                format!("  {spinner} "),
                Style::default().fg(Color::Cyan),
            ));

            let name_col = format!("{:<28}", file.name);
            spans.push(Span::styled(name_col, Style::default().fg(Color::Cyan)));

            if file.progress_stage.is_some() {
                spans.push(Span::raw(" "));
                spans.extend(render_phase_dots(file.progress_stage));
                spans.push(Span::raw("  "));
            } else {
                spans.push(Span::raw("        "));
            }

            spans.push(Span::styled(
                format!("{label}{pct}"),
                Style::default().fg(Color::Cyan),
            ));

            // Per-file elapsed timer from started_at
            if let Some(started) = file.started_at {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                let elapsed = (now - started).max(0.0);
                let e_mins = elapsed as u64 / 60;
                let e_secs = elapsed as u64 % 60;
                spans.push(Span::styled(
                    format!("  {e_mins}:{e_secs:02}"),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        }
        FileStatusKind::Done => {
            let dur = file
                .duration_s
                .map(|d| format!("{d:.1}s"))
                .unwrap_or_default();
            let name_part = format!("  ✓ {}", file.name);
            let padding = w.saturating_sub(name_part.len() + dur.len());
            let text = format!("{name_part}{:>pad$}{dur}", "", pad = padding);
            Line::from(Span::styled(
                pad_or_truncate(&text, w),
                Style::default().fg(Color::Green),
            ))
        }
        FileStatusKind::Error => {
            let code = file
                .error_codes
                .first()
                .map(|c| format!("  {c}"))
                .unwrap_or_default();
            let msg = file
                .error_msg
                .as_deref()
                .and_then(|m| m.split('\n').next())
                .unwrap_or("");
            let msg_short = if msg.len() > 30 {
                format!("{}…", &msg[..29])
            } else {
                msg.to_string()
            };
            let text = format!("  ✗ {}{code}   {msg_short}", file.name);
            Line::from(Span::styled(
                pad_or_truncate(&text, w),
                Style::default().fg(Color::Red),
            ))
        }
    }
}

/// Worker status + memory gauge (2 rows).
fn draw_metrics(f: &mut Frame, state: &AppState, area: Rect) {
    let Some(health) = &state.health else {
        return;
    };

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    // ── Row 1: Workers ──
    let mut worker_spans: Vec<Span<'static>> = vec![Span::styled(
        "  Workers: ",
        Style::default().fg(Color::DarkGray),
    )];

    if health.live_worker_keys.is_empty() {
        worker_spans.push(Span::styled("none", Style::default().fg(Color::DarkGray)));
    } else {
        for (i, key) in health.live_worker_keys.iter().enumerate() {
            if i > 0 {
                worker_spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
            }
            worker_spans.push(Span::styled(key.clone(), Style::default().fg(Color::Cyan)));
        }
    }

    worker_spans.push(Span::styled(
        format!("    Warmup: {}", health.warmup_status),
        Style::default().fg(Color::DarkGray),
    ));

    // Show operational counters when nonzero
    if health.worker_crashes > 0 {
        worker_spans.push(Span::styled(
            format!("  {}crash", health.worker_crashes),
            Style::default().fg(Color::Red),
        ));
    }
    if health.attempts_started > 0 {
        worker_spans.push(Span::styled(
            format!("  {}att", health.attempts_started),
            Style::default().fg(Color::DarkGray),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(worker_spans)), rows[0]);

    // ── Row 2: Memory ──
    let total = health.system_memory_total_mb.0;
    let used = health.system_memory_used_mb.0;
    let avail = health.system_memory_available_mb.0;
    let gate = health.memory_gate_threshold_mb.0;

    let (total_gb, used_gb) = (total as f64 / 1024.0, used as f64 / 1024.0);

    // Bar rendering: 20-char gauge
    let bar_width = 20usize;
    let ratio = if total > 0 {
        (used as f64 / total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (ratio * bar_width as f64).round() as usize;
    let bar_filled: String = "█".repeat(filled);
    let bar_empty: String = "░".repeat(bar_width.saturating_sub(filled));

    // Gate proximity color (same thresholds as React MemoryPanel)
    let bar_color = if gate == 0 {
        Color::Green
    } else {
        let headroom = avail as f64 / gate as f64;
        if headroom > 4.0 {
            Color::Green
        } else if headroom > 2.0 {
            Color::Yellow
        } else {
            Color::Red
        }
    };

    let gate_label = if gate > 0 {
        let gate_gb = gate as f64 / 1024.0;
        if gate_gb >= 1.0 {
            format!("Gate: {gate_gb:.0} GB")
        } else {
            format!("Gate: {gate} MB")
        }
    } else {
        "Gate: off".into()
    };

    let safe_label = if gate == 0 {
        ""
    } else {
        let headroom = avail as f64 / gate as f64;
        if headroom > 4.0 {
            " ● safe"
        } else if headroom > 2.0 {
            " ● warn"
        } else if headroom > 1.0 {
            " ● danger — gate may block new workers"
        } else {
            " ● BLOCKED — below gate threshold"
        }
    };

    // Host-memory pressure indicator (from coordinator, more precise than
    // the simple gate proximity heuristic above).
    let pressure = &health.host_memory_pressure;
    let (pressure_label, pressure_color) = match pressure.as_str() {
        "healthy" => ("healthy", Color::Green),
        "guarded" => ("guarded", Color::Yellow),
        "constrained" => ("constrained", Color::Red),
        "critical" => ("CRITICAL", Color::Red),
        _ => (pressure.as_str(), Color::DarkGray),
    };

    let mem_spans = vec![
        Span::styled("  Memory: [", Style::default().fg(Color::DarkGray)),
        Span::styled(bar_filled, Style::default().fg(bar_color)),
        Span::styled(bar_empty, Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("] {used_gb:.0}/{total_gb:.0} GB   {gate_label}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(safe_label.to_string(), Style::default().fg(bar_color)),
        Span::styled(
            format!("  [{pressure_label}]"),
            Style::default().fg(pressure_color),
        ),
    ];

    f.render_widget(Paragraph::new(Line::from(mem_spans)), rows[1]);
}

/// Error summary panel.
fn draw_errors(f: &mut Frame, state: &AppState, area: Rect) {
    if state.errors.entries.is_empty() {
        return;
    }

    if !state.errors.expanded {
        let summary = format!(
            "  {} error(s) — press 'e' to expand",
            state.errors.entries.len()
        );
        let p = Paragraph::new(summary).style(Style::default().fg(Color::Red));
        f.render_widget(p, area);
        return;
    }

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(format!(" Errors ({}) ", state.errors.entries.len()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = state
        .errors
        .entries
        .iter()
        .take(inner.height as usize)
        .map(|err| {
            let first_line = err.message.split('\n').next().unwrap_or("unknown");
            let code_str = err
                .code
                .as_deref()
                .map(|c| format!("[{c}] "))
                .unwrap_or_default();
            let text = format!("  ✗ {}: {code_str}{first_line}", err.filename);
            ListItem::new(Line::from(Span::styled(
                text,
                Style::default().fg(Color::Red),
            )))
        })
        .collect();

    f.render_widget(List::new(items), inner);
}

/// Bottom keybind bar.
fn draw_keybinds(f: &mut Frame, state: &AppState, area: Rect) {
    let line = if state.interaction.cancel_confirm {
        Line::from(vec![
            Span::styled(
                "  Cancel job? ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "n",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("  q", Style::default().fg(Color::Cyan)),
            Span::styled(" quit  ", Style::default().fg(Color::DarkGray)),
            Span::styled("c", Style::default().fg(Color::Cyan)),
            Span::styled(" cancel  ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑↓", Style::default().fg(Color::Cyan)),
            Span::styled(" scroll  ", Style::default().fg(Color::DarkGray)),
            Span::styled("tab", Style::default().fg(Color::Cyan)),
            Span::styled(" group  ", Style::default().fg(Color::DarkGray)),
            Span::styled("e", Style::default().fg(Color::Cyan)),
            Span::styled(" errors  ", Style::default().fg(Color::DarkGray)),
            Span::styled("m", Style::default().fg(Color::Cyan)),
            Span::styled(" metrics", Style::default().fg(Color::DarkGray)),
        ])
    };

    f.render_widget(Paragraph::new(line), area);
}

/// Pad or truncate a string to exactly `width` characters.
fn pad_or_truncate(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{s:<width$}")
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::api::{MemoryMb, UnixTimestamp};

    use super::*;
    use crate::cli::tui::app::AppState;

    fn make_entry(filename: &str, status: FileStatusKind) -> crate::api::FileStatusEntry {
        crate::api::FileStatusEntry {
            filename: filename.into(),
            status,
            error: if status == FileStatusKind::Error {
                Some("morph lookup failed".into())
            } else {
                None
            },
            error_category: None,
            error_codes: if status == FileStatusKind::Error {
                Some(vec!["E4012".into()])
            } else {
                None
            },
            error_line: None,
            bug_report_id: None,
            started_at: if status == FileStatusKind::Done {
                Some(UnixTimestamp(0.0))
            } else {
                None
            },
            finished_at: if status == FileStatusKind::Done {
                Some(UnixTimestamp(1.2))
            } else {
                None
            },
            progress_current: if status == FileStatusKind::Processing {
                Some(12)
            } else {
                None
            },
            progress_total: if status == FileStatusKind::Processing {
                Some(45)
            } else {
                None
            },
            next_eligible_at: None,
            progress_stage: None,
            progress_label: if status == FileStatusKind::Processing {
                Some("stanza".into())
            } else {
                None
            },
        }
    }

    #[test]
    fn render_empty_state() {
        let state = AppState::new(10, "morphotag");
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw should not panic");
    }

    #[test]
    fn render_mixed_statuses() {
        let mut state = AppState::new(4, "morphotag");
        let entries = vec![
            make_entry("eng/a.cha", FileStatusKind::Done),
            make_entry("eng/b.cha", FileStatusKind::Processing),
            make_entry("eng/c.cha", FileStatusKind::Queued),
            make_entry("eng/d.cha", FileStatusKind::Error),
        ];
        state.update_from_poll(1, &entries);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw should not panic");
    }

    #[test]
    fn render_error_expanded() {
        let mut state = AppState::new(2, "morphotag");
        state.add_error("test.cha", "something broke", None);
        state.errors.expanded = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw should not panic");
    }

    #[test]
    fn render_cancel_confirm() {
        let mut state = AppState::new(2, "morphotag");
        state.interaction.cancel_confirm = true;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw should not panic");
    }

    #[test]
    fn render_with_health_metrics() {
        let mut state = AppState::new(2, "morphotag");
        state.health = Some(crate::cli::tui::app::ServerHealth {
            live_workers: 2,
            live_worker_keys: vec!["infer:morphosyntax:eng".into(), "infer:utseg:eng".into()],
            system_memory_total_mb: MemoryMb(262144),
            system_memory_available_mb: MemoryMb(100000),
            system_memory_used_mb: MemoryMb(162144),
            memory_gate_threshold_mb: MemoryMb(2048),
            warmup_status: "complete".into(),
            host_memory_pressure: "healthy".into(),
            worker_crashes: 0,
            attempts_started: 0,
        });
        state.show_metrics = true;

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw with metrics should not panic");
    }

    #[test]
    fn render_metrics_hidden_when_toggled_off() {
        let mut state = AppState::new(2, "morphotag");
        state.health = Some(crate::cli::tui::app::ServerHealth {
            live_workers: 1,
            live_worker_keys: vec!["infer:asr:eng".into()],
            system_memory_total_mb: MemoryMb(65536),
            system_memory_available_mb: MemoryMb(30000),
            system_memory_used_mb: MemoryMb(35536),
            memory_gate_threshold_mb: MemoryMb(2048),
            warmup_status: "in_progress".into(),
            host_memory_pressure: "guarded".into(),
            worker_crashes: 0,
            attempts_started: 0,
        });
        state.show_metrics = false;

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw with hidden metrics should not panic");
    }

    #[test]
    fn render_processing_with_pipeline_stage() {
        let mut state = AppState::new(1, "align");
        let mut entry = make_entry("eng/test.cha", FileStatusKind::Processing);
        entry.progress_stage = Some(FileProgressStage::Aligning);
        state.update_from_poll(0, &[entry]);

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw with pipeline stage should not panic");
    }

    #[test]
    fn phase_index_mapping_covers_all_phases() {
        // Phase 0: Read
        assert_eq!(phase_index(FileProgressStage::Reading), Some(0));
        assert_eq!(phase_index(FileProgressStage::CheckingCache), Some(0));
        // Phase 1: Transcribe
        assert_eq!(phase_index(FileProgressStage::Transcribing), Some(1));
        // Phase 2: Align
        assert_eq!(phase_index(FileProgressStage::Aligning), Some(2));
        // Phase 3: Analyze
        assert_eq!(
            phase_index(FileProgressStage::AnalyzingMorphosyntax),
            Some(3)
        );
        assert_eq!(phase_index(FileProgressStage::Analyzing), Some(3));
        // Phase 4: Finalize
        assert_eq!(phase_index(FileProgressStage::Finalizing), Some(4));
        assert_eq!(phase_index(FileProgressStage::Writing), Some(4));
        // No phase
        assert_eq!(phase_index(FileProgressStage::Processing), None);
        assert_eq!(phase_index(FileProgressStage::RetryScheduled), None);
    }

    #[test]
    fn render_finished_state_shows_summary() {
        let mut state = AppState::new(3, "morphotag");
        let entries = vec![
            make_entry("eng/a.cha", FileStatusKind::Done),
            make_entry("eng/b.cha", FileStatusKind::Done),
            make_entry("eng/c.cha", FileStatusKind::Error),
        ];
        state.update_from_poll(2, &entries);
        state.apply_update(crate::cli::tui::app::TuiUpdate::Finished);

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw finished state should not panic");
    }

    #[test]
    fn render_with_scroll_indicators() {
        // 20 files in one group on a small terminal — should show ▲/▼
        let mut state = AppState::new(20, "morphotag");
        let entries: Vec<_> = (0..20)
            .map(|i| make_entry(&format!("eng/{i:02}.cha"), FileStatusKind::Queued))
            .collect();
        state.update_from_poll(0, &entries);
        state.directories.scroll_offset = 5;

        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw with scroll should not panic");
    }

    #[test]
    fn render_with_elapsed_timer() {
        let mut state = AppState::new(1, "align");
        let mut entry = make_entry("eng/test.cha", FileStatusKind::Processing);
        // Set started_at to 60 seconds ago
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        entry.started_at = Some(UnixTimestamp(now - 60.0));
        state.update_from_poll(0, &[entry]);

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, &state))
            .expect("draw with elapsed timer should not panic");
    }
}
