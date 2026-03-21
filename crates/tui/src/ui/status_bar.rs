use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let online = app.runners.iter().filter(|r| r.state == "online").count();
    let busy = app.runners.iter().filter(|r| r.state == "busy").count();
    let total = app.runners.len();

    let status = if let Some(ref msg) = app.status_message {
        Span::styled(msg.as_str(), Style::default().fg(Color::Yellow))
    } else {
        Span::raw("")
    };

    let connection = if app.daemon_connected {
        Span::styled("Connected", Style::default().fg(Color::Green))
    } else {
        Span::styled("Disconnected", Style::default().fg(Color::Red))
    };

    let line = Line::from(vec![
        Span::raw(" "),
        connection,
        Span::raw("  |  "),
        Span::styled(
            format!("{total} runners ({online} online, {busy} busy)"),
            Style::default().fg(Color::White),
        ),
        Span::raw("  |  "),
        status,
        Span::raw("  "),
        Span::styled("q:quit  ?:help", Style::default().fg(Color::DarkGray)),
    ]);

    f.render_widget(Paragraph::new(line), area);
}
