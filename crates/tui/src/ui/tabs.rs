use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Tab};

pub fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::raw(" "));

    for (i, tab) in Tab::all().iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  │  ", Style::default().fg(Color::DarkGray)));
        }

        let style = if *tab == app.active_tab {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        spans.push(Span::styled(format!(" {} ", tab.title()), style));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
