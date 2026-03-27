use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;

pub fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    if let Some(ref msg) = app.status_message {
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(msg.as_str(), Style::default().fg(Color::Yellow)),
        ]);
        f.render_widget(Paragraph::new(line), area);
    }
}
