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
    fn test_renders_all_tab_names() {
        let app = App::new();
        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_tabs(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Runners"), "should contain Runners tab");
        assert!(content.contains("Repos"), "should contain Repos tab");
        assert!(
            content.contains("Monitoring"),
            "should contain Monitoring tab"
        );
        assert!(content.contains("Daemon"), "should contain Daemon tab");
    }

    #[test]
    fn test_active_tab_runners() {
        let mut app = App::new();
        app.active_tab = Tab::Runners;

        let backend = TestBackend::new(100, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_tabs(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Runners"));
    }

    #[test]
    fn test_different_active_tabs_highlight_differently() {
        // Render with Runners active
        let mut app1 = App::new();
        app1.active_tab = Tab::Runners;
        let backend1 = TestBackend::new(100, 1);
        let mut terminal1 = Terminal::new(backend1).unwrap();
        terminal1
            .draw(|f| {
                draw_tabs(f, &app1, f.area());
            })
            .unwrap();
        let buf1 = terminal1.backend().buffer().clone();

        // Render with Daemon active
        let mut app2 = App::new();
        app2.active_tab = Tab::Daemon;
        let backend2 = TestBackend::new(100, 1);
        let mut terminal2 = Terminal::new(backend2).unwrap();
        terminal2
            .draw(|f| {
                draw_tabs(f, &app2, f.area());
            })
            .unwrap();
        let buf2 = terminal2.backend().buffer().clone();

        // The "Runners" cell should have different styles between the two buffers
        // Find "Runners" position — it starts after the initial " " and " " padding
        // In buf1, Runners is active (Yellow+Bold); in buf2, Runners is inactive (White)
        let runners_x = 2u16; // " " prefix + " " padding in the span
        let cell1 = buf1.cell((runners_x, 0)).unwrap();
        let cell2 = buf2.cell((runners_x, 0)).unwrap();
        assert_ne!(
            cell1.fg, cell2.fg,
            "Active vs inactive tab should have different foreground color"
        );
    }
}
