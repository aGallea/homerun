use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // --- Info bar line 1: User, Daemon, Uptime, Version ---
    let user_spans = if let Some(ref auth) = app.auth_status {
        if auth.authenticated {
            let username = auth
                .user
                .as_ref()
                .map(|u| u.login.as_str())
                .unwrap_or("unknown");
            vec![
                Span::styled("User: ", Style::default().fg(Color::Blue)),
                Span::styled(username, Style::default().fg(Color::Green)),
            ]
        } else {
            vec![
                Span::styled("User: ", Style::default().fg(Color::Blue)),
                Span::styled("Not logged in", Style::default().fg(Color::Red)),
            ]
        }
    } else {
        vec![
            Span::styled("User: ", Style::default().fg(Color::Blue)),
            Span::styled("Not logged in", Style::default().fg(Color::Red)),
        ]
    };

    let daemon_spans = if app.daemon_connected {
        let pid_str = app
            .metrics
            .as_ref()
            .and_then(|m| m.daemon.as_ref())
            .map(|d| format!(" (PID {})", d.pid))
            .unwrap_or_default();
        vec![
            Span::styled("Daemon: ", Style::default().fg(Color::Blue)),
            Span::styled(
                format!("Connected{pid_str}"),
                Style::default().fg(Color::Green),
            ),
        ]
    } else {
        vec![
            Span::styled("Daemon: ", Style::default().fg(Color::Blue)),
            Span::styled("Disconnected", Style::default().fg(Color::Red)),
        ]
    };

    let uptime_str = app
        .metrics
        .as_ref()
        .and_then(|m| m.daemon.as_ref())
        .map(|d| {
            let hours = d.uptime_seconds / 3600;
            let mins = (d.uptime_seconds % 3600) / 60;
            format!("{hours}h {mins}m")
        })
        .unwrap_or_else(|| "n/a".to_string());

    let version = env!("CARGO_PKG_VERSION");

    let mut line1 = Vec::new();
    line1.push(Span::raw(" "));
    line1.extend(user_spans);
    line1.push(Span::raw("    "));
    line1.extend(daemon_spans);
    line1.push(Span::raw("    "));
    line1.push(Span::styled("Uptime: ", Style::default().fg(Color::Blue)));
    line1.push(Span::raw(&uptime_str));
    line1.push(Span::raw("    "));
    line1.push(Span::styled("Version: ", Style::default().fg(Color::Blue)));
    line1.push(Span::raw(format!("v{version}")));

    lines.push(Line::from(line1));

    // --- Info bar line 2: Runner summary ---
    let online = app.runners.iter().filter(|r| r.state == "online").count();
    let busy = app.runners.iter().filter(|r| r.state == "busy").count();
    let offline = app.runners.iter().filter(|r| r.state == "offline").count();
    let total = app.runners.len();

    let line2 = Line::from(vec![
        Span::raw(" "),
        Span::styled("Runners: ", Style::default().fg(Color::Blue)),
        Span::raw(format!("{total} total (")),
        Span::styled(
            format!("{online} online"),
            Style::default().fg(Color::Green),
        ),
        Span::raw(", "),
        Span::styled(format!("{busy} busy"), Style::default().fg(Color::Magenta)),
        Span::raw(", "),
        Span::styled(
            format!("{offline} offline"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(")"),
    ]);
    lines.push(line2);

    // --- Blank separator line ---
    lines.push(Line::raw(""));

    // --- Key hints grid ---
    let hints = app.key_hints();
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);

    // Column layout: each hint is "<key> description" padded to a fixed width
    let col_width = 18u16; // enough for "<C-3> Monitoring" with padding

    for row in &hints {
        let mut spans = vec![Span::raw(" ")];
        for (key, desc) in row {
            let hint_text = format!("<{key}>");
            let entry = format!("{hint_text} {desc}");
            let padding = col_width as usize - entry.len().min(col_width as usize);
            spans.push(Span::styled(hint_text, key_style));
            spans.push(Span::styled(format!(" {desc}"), desc_style));
            spans.push(Span::raw(" ".repeat(padding)));
        }
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), area);
}
