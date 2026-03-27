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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf.cell((x, y)).unwrap().symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn test_no_status_message_renders_empty() {
        let app = App::new();
        assert!(app.status_message.is_none());

        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_status_bar(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        // Should be all spaces (blank) when no status message
        assert!(content.trim().is_empty());
    }

    #[test]
    fn test_status_message_renders_text() {
        let mut app = App::new();
        app.status_message = Some("Runner started successfully".to_string());

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_status_bar(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Runner started successfully"));
    }
}
