pub mod monitoring;
pub mod repos;
pub mod runners;
pub mod status_bar;
pub mod tabs;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.area());

    tabs::draw_tabs(f, app, chunks[0]);

    match app.active_tab {
        crate::app::Tab::Runners => runners::draw_runners(f, app, chunks[1]),
        crate::app::Tab::Repos => repos::draw_repos(f, app, chunks[1]),
        crate::app::Tab::Monitoring => monitoring::draw_monitoring(f, app, chunks[1]),
    }

    status_bar::draw_status_bar(f, app, chunks[2]);

    if app.show_help {
        draw_help_popup(f);
    }
}

fn draw_help_popup(f: &mut Frame) {
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    let area = f.area();
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let help_text = "\
  Keybindings

  Up/Down    Navigate list
  1-3        Switch tabs
  s          Start/Stop runner
  r          Restart runner
  d          Delete runner (confirm)
  l          View logs
  e          Edit labels
  a          Add runner
  ?          Toggle this help
  q          Quit";

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title(" Help "))
            .style(Style::default().fg(Color::White)),
        popup_area,
    );
}
