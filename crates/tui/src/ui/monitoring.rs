use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn draw_monitoring(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let gauge_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    match &app.metrics {
        Some(metrics) => {
            draw_cpu_gauge(f, &metrics.system, gauge_row[0]);
            draw_memory_gauge(f, &metrics.system, gauge_row[1]);
            draw_runner_metrics(f, app, chunks[1]);
        }
        None => {
            let msg = Paragraph::new(" Loading metrics...")
                .block(Block::default().borders(Borders::ALL).title(" Monitoring "));
            f.render_widget(msg, area);
        }
    }
}

fn draw_cpu_gauge(f: &mut Frame, sys: &crate::client::SystemMetrics, area: Rect) {
    let ratio = (sys.cpu_percent / 100.0).clamp(0.0, 1.0);
    let color = gauge_color(ratio);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" CPU "))
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label(format!("{:.1}%", sys.cpu_percent));
    f.render_widget(gauge, area);
}

fn draw_memory_gauge(f: &mut Frame, sys: &crate::client::SystemMetrics, area: Rect) {
    let ratio = if sys.memory_total_bytes > 0 {
        (sys.memory_used_bytes as f64 / sys.memory_total_bytes as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let color = gauge_color(ratio);
    let used = format_bytes(sys.memory_used_bytes);
    let total = format_bytes(sys.memory_total_bytes);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Memory "))
        .gauge_style(Style::default().fg(color))
        .ratio(ratio)
        .label(format!("{used} / {total}"));
    f.render_widget(gauge, area);
}

fn draw_runner_metrics(f: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = match &app.metrics {
        Some(m) if !m.runners.is_empty() => {
            let mut lines = vec![Line::from(Span::styled(
                " Runner                       CPU      Memory",
                Style::default().fg(Color::DarkGray),
            ))];
            for rm in &m.runners {
                let name = app
                    .runners
                    .iter()
                    .find(|r| r.config.id == rm.runner_id)
                    .map(|r| r.config.name.as_str())
                    .unwrap_or(rm.runner_id.as_str());
                lines.push(Line::from(format!(
                    " {:<30}{:>5.1}%   {}",
                    name,
                    rm.cpu_percent,
                    format_bytes(rm.memory_bytes),
                )));
            }
            lines
        }
        _ => vec![Line::from(" No active runner processes.")],
    };

    let paragraph =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Per-Runner "));
    f.render_widget(paragraph, area);
}

fn gauge_color(ratio: f64) -> Color {
    if ratio > 0.9 {
        Color::Red
    } else if ratio > 0.7 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{MetricsResponse, SystemMetrics};
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
    fn test_gauge_color_thresholds() {
        assert_eq!(gauge_color(0.5), Color::Green);
        assert_eq!(gauge_color(0.8), Color::Yellow);
        assert_eq!(gauge_color(0.95), Color::Red);
    }

    #[test]
    fn test_renders_loading_when_no_metrics() {
        let app = App::new();
        assert!(app.metrics.is_none());

        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_monitoring(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("Loading metrics"),
            "should show loading message when metrics is None"
        );
    }

    #[test]
    fn test_renders_cpu_and_memory_gauges_when_metrics_exist() {
        let mut app = App::new();
        app.metrics = Some(MetricsResponse {
            system: SystemMetrics {
                cpu_percent: 42.5,
                memory_used_bytes: 8 * 1024 * 1024 * 1024,
                memory_total_bytes: 16 * 1024 * 1024 * 1024,
                disk_used_bytes: 100_000_000_000,
                disk_total_bytes: 500_000_000_000,
            },
            runners: vec![],
            daemon: None,
        });

        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_monitoring(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("CPU"),
            "should show CPU gauge when metrics exist"
        );
        assert!(
            content.contains("Memory"),
            "should show Memory gauge when metrics exist"
        );
    }
}
