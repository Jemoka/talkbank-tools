//! Ratatui rendering for the live corpus test dashboard.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
};

use crate::test_dashboard::app::AppState;

/// Render one dashboard frame from the latest shared state snapshot.
pub fn render_dashboard(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(9),
            Constraint::Length(7),
            Constraint::Length(5),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(f.area());

    render_title(f, chunks[0]);
    render_overall_stats(f, chunks[1], state);
    render_current_corpus(f, chunks[2], state);
    render_cache_stats(f, chunks[3], state);
    render_recent_failures(f, chunks[4], state);
    render_status(f, chunks[5], state);
}

/// Draw the static title header.
pub fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new("TalkBank Corpus Testing Dashboard")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

/// Draw aggregate progress, pass rate, throughput, and ETA for all corpora.
pub fn render_overall_stats(f: &mut Frame, area: Rect, state: &AppState) {
    let progress_pct = state.overall_progress_pct();
    let pass_rate = if state.total_passed + state.total_failed > 0 {
        (state.total_passed as f64 / (state.total_passed + state.total_failed) as f64) * 100.0
    } else {
        0.0
    };

    let eta_str = if let Some(eta_secs) = state.eta_seconds() {
        let hours = eta_secs / 3600;
        let minutes = (eta_secs % 3600) / 60;
        let seconds = eta_secs % 60;
        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    } else {
        "Calculating...".to_string()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Progress: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{:.1}%", progress_pct),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  ({} / {} files)",
                    state.total_passed + state.total_failed,
                    state.total_files
                ),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("✓ Passed: ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("{} ", state.total_passed),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ✗ Failed: ", Style::default().fg(Color::Red)),
            Span::styled(
                format!("{} ", state.total_failed),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ⏳ Remaining: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", state.total_not_tested),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("Pass Rate: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{:.1}%", pass_rate),
                Style::default().fg(if pass_rate >= 90.0 {
                    Color::Green
                } else if pass_rate >= 75.0 {
                    Color::Yellow
                } else {
                    Color::Red
                }),
            ),
            Span::styled("  Speed: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{:.1} files/sec", state.files_per_second()),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled("  ETA: ", Style::default().fg(Color::White)),
            Span::styled(eta_str, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("Overall: {:.0}%", progress_pct),
            Style::default().fg(Color::Gray),
        )]),
    ];

    let gauge = Gauge::default()
        .ratio(progress_pct / 100.0)
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray));

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Overall Progress");
    let inner = block.inner(area);

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);

    if inner.height > 0 {
        let gauge_area = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        f.render_widget(gauge, gauge_area);
    }
}

/// Draw progress metrics for the corpus currently under test.
pub fn render_current_corpus(f: &mut Frame, area: Rect, state: &AppState) {
    let corpus_progress = state.current_corpus_progress_pct();

    let lines = vec![
        Line::from(vec![
            Span::styled("Corpus: ", Style::default().fg(Color::White)),
            Span::styled(
                format!(
                    "{} ({}/{})",
                    state.current_corpus_name,
                    state.current_corpus_idx + 1,
                    state.total_corpora
                ),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Files: ", Style::default().fg(Color::White)),
            Span::styled(
                format!(
                    "{} / {}",
                    state.current_corpus_files_tested, state.current_corpus_total_files
                ),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("  Passed: ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("{}", state.current_corpus_passed),
                Style::default().fg(Color::Green),
            ),
            Span::styled("  Failed: ", Style::default().fg(Color::Red)),
            Span::styled(
                format!("{}", state.current_corpus_failed),
                Style::default().fg(Color::Red),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("Progress: {:.0}%", corpus_progress),
            Style::default().fg(Color::Gray),
        )]),
    ];

    let gauge = Gauge::default()
        .ratio(corpus_progress / 100.0)
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Current Corpus");
    let inner = block.inner(area);

    let para = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);

    if inner.height > 0 {
        let gauge_area = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        f.render_widget(gauge, gauge_area);
    }
}

/// Draw cache hit and miss counts plus derived hit rate.
pub fn render_cache_stats(f: &mut Frame, area: Rect, state: &AppState) {
    let cache_hit_rate = state.cache_hit_rate();

    let lines = vec![
        Line::from(vec![
            Span::styled("Hits: ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("{} ", state.cache_hits),
                Style::default().fg(Color::Green),
            ),
            Span::styled("  Misses: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", state.cache_misses),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("Hit Rate: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{:.1}%", cache_hit_rate),
                Style::default().fg(if cache_hit_rate >= 75.0 {
                    Color::Green
                } else if cache_hit_rate >= 50.0 {
                    Color::Yellow
                } else {
                    Color::Red
                }),
            ),
        ]),
    ];

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Cache Statistics"),
    );

    f.render_widget(para, area);
}

/// Draw the rolling list of recently failed files.
pub fn render_recent_failures(f: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state
        .recent_failures
        .iter()
        .map(|entry| ListItem::new(entry.clone()).style(Style::default().fg(Color::Red)))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Recent Failures ({})", state.recent_failures.len())),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, area);
}

/// Draw the footer status line and control hints.
pub fn render_status(f: &mut Frame, area: Rect, state: &AppState) {
    let status_color = if state.is_paused {
        Color::Yellow
    } else if state.is_testing {
        Color::Green
    } else {
        Color::Cyan
    };

    let status_text = if state.is_paused {
        format!("⏸ PAUSED | {}", state.status_message)
    } else if state.is_testing {
        format!("▶ TESTING | {}", state.status_message)
    } else {
        format!("✓ {}", state.status_message)
    };

    let para = Paragraph::new(vec![Line::from(vec![Span::styled(
        status_text,
        Style::default()
            .fg(status_color)
            .add_modifier(Modifier::BOLD),
    )])])
    .block(Block::default().borders(Borders::ALL).title("Status"))
    .alignment(Alignment::Left);

    f.render_widget(para, area);
}
