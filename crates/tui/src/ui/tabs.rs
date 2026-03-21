use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Tabs};
use ratatui::Frame;

use crate::app::{App, Tab};

pub fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            Line::from(Span::styled(
                format!(" [{}] {} ", i + 1, tab.title()),
                Style::default().fg(Color::White),
            ))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" HomeRun "))
        .select(app.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}
