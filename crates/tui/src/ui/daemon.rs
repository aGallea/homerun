use crate::app::App;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw_daemon_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Info bar
            Constraint::Length(1), // Filter bar
            Constraint::Min(5),    // Logs
        ])
        .split(area);

    draw_info_bar(f, app, chunks[0]);
    draw_filter_bar(f, app, chunks[1]);
    draw_log_viewer(f, app, chunks[2]);
}

fn draw_info_bar(f: &mut Frame, app: &App, area: Rect) {
    let line = match app.metrics.as_ref().and_then(|m| m.daemon.as_ref()) {
        Some(dm) => {
            let uptime = format_uptime(dm.uptime_seconds);
            let memory = format_bytes(dm.memory_bytes);
            Line::from(vec![
                Span::styled(" PID: ", Style::default().fg(Color::DarkGray)),
                Span::styled(dm.pid.to_string(), Style::default().fg(Color::White)),
                Span::styled("  Uptime: ", Style::default().fg(Color::DarkGray)),
                Span::styled(uptime, Style::default().fg(Color::White)),
                Span::styled("  CPU: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:.1}%", dm.cpu_percent),
                    Style::default().fg(Color::White),
                ),
                Span::styled("  Memory: ", Style::default().fg(Color::DarkGray)),
                Span::styled(memory, Style::default().fg(Color::White)),
                Span::styled("  Children: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} processes", dm.child_processes.len()),
                    Style::default().fg(Color::White),
                ),
            ])
        }
        None => Line::from(Span::styled(
            " Loading...",
            Style::default().fg(Color::DarkGray),
        )),
    };

    f.render_widget(Paragraph::new(line), area);
}

fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let levels = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];
    let mut spans: Vec<Span> = vec![Span::raw(" ")];

    for level in &levels {
        let style = if *level == app.daemon_log_level {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!("[{level}]"), style));
        spans.push(Span::raw(" "));
    }

    spans.push(Span::styled(
        "  Search: ",
        Style::default().fg(Color::DarkGray),
    ));
    if app.daemon_searching {
        spans.push(Span::styled(
            format!("{}_", app.daemon_search),
            Style::default().fg(Color::Yellow),
        ));
    } else if app.daemon_search.is_empty() {
        spans.push(Span::styled(
            "(press / to search)",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(
            &app.daemon_search,
            Style::default().fg(Color::White),
        ));
    }

    spans.push(Span::raw("  "));
    let follow_style = if app.daemon_follow {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    spans.push(Span::styled("Follow", follow_style));

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_log_viewer(f: &mut Frame, app: &App, area: Rect) {
    if app.daemon_logs.is_empty() {
        let msg = Paragraph::new(" No daemon logs yet")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Daemon Logs "),
            )
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, area);
        return;
    }

    let inner_height = area.height.saturating_sub(2) as usize; // account for borders
    let total = app.daemon_logs.len();

    // Determine the starting index for visible logs
    let start = if app.daemon_follow {
        total.saturating_sub(inner_height)
    } else {
        app.daemon_log_scroll
            .min(total.saturating_sub(inner_height))
    };
    let end = (start + inner_height).min(total);

    let lines: Vec<Line> = app.daemon_logs[start..end]
        .iter()
        .map(|entry| {
            let time = if entry.timestamp.len() >= 19 {
                &entry.timestamp[11..19]
            } else {
                &entry.timestamp
            };

            let level_color = match entry.level.as_str() {
                "ERROR" => Color::Red,
                "WARN" => Color::Yellow,
                "INFO" => Color::Green,
                "DEBUG" => Color::Blue,
                "TRACE" => Color::DarkGray,
                _ => Color::White,
            };

            let short_target = entry.target.rsplit("::").next().unwrap_or(&entry.target);

            Line::from(vec![
                Span::styled(format!("{time} "), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<5} ", entry.level),
                    Style::default().fg(level_color),
                ),
                Span::styled(
                    format!("{:<16} ", short_target),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(&entry.message, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let title = format!(
        " Daemon Logs ({}/{}) ",
        if app.daemon_follow { total } else { end },
        total
    );

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(0), "0m");
        assert_eq!(format_uptime(60), "1m");
        assert_eq!(format_uptime(3661), "1h 1m");
        assert_eq!(format_uptime(90061), "1d 1h 1m");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1048576), "1 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
        assert_eq!(format_bytes(45 * 1024 * 1024), "45 MB");
    }
}
