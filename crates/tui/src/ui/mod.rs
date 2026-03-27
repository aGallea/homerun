pub mod daemon;
pub mod header;
pub mod monitoring;
pub mod repos;
pub mod runners;
pub mod status_bar;
pub mod tabs;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let outer_area = f.area();

    // Reserve 1 line at bottom for transient status bar (outside the box)
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(outer_area);

    let main_area = outer_chunks[0];
    let status_area = outer_chunks[1];

    // Draw the outer bordered frame
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .title(" HomeRun ")
        .title_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .border_style(Style::default().fg(Color::DarkGray));
    let inner_area = outer_block.inner(main_area);
    f.render_widget(outer_block, main_area);

    // Split inner area: header + divider + tab bar + divider + content
    // Header = 2 info lines + 1 blank + 4 key grid lines = 7
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // header (info + keys)
            Constraint::Length(1), // top divider
            Constraint::Length(1), // tab bar
            Constraint::Length(1), // bottom divider
            Constraint::Min(0),    // content
        ])
        .split(inner_area);

    let header_area = inner_chunks[0];
    let top_div_area = inner_chunks[1];
    let tab_area = inner_chunks[2];
    let bot_div_area = inner_chunks[3];
    let content_area = inner_chunks[4];

    // Draw header (info bar + key grid)
    header::draw_header(f, app, header_area);

    // Draw horizontal dividers around tab bar
    draw_horizontal_divider(f, top_div_area);
    tabs::draw_tabs(f, app, tab_area);
    draw_horizontal_divider(f, bot_div_area);

    // Draw tab content
    match app.active_tab {
        crate::app::Tab::Runners => runners::draw_runners(f, app, content_area),
        crate::app::Tab::Repos => repos::draw_repos(f, app, content_area),
        crate::app::Tab::Monitoring => monitoring::draw_monitoring(f, app, content_area),
        crate::app::Tab::Daemon => daemon::draw_daemon_tab(f, app, content_area),
    }

    // Draw transient status bar outside the box
    status_bar::draw_status_bar(f, app, status_area);

    // Help popup on top of everything
    if app.show_help {
        draw_help_popup(f);
    }

    if let Some(ref login_state) = app.login_state {
        draw_login_popup(f, login_state);
    }
}

fn draw_horizontal_divider(f: &mut Frame, area: Rect) {
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let divider = "─".repeat(area.width as usize);
    let line = Line::from(Span::styled(divider, Style::default().fg(Color::DarkGray)));
    f.render_widget(Paragraph::new(line), area);
}

fn draw_help_popup(f: &mut Frame) {
    use ratatui::widgets::{Clear, Paragraph};

    let area = f.area();
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 18.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let help_text = "\
  Keybindings

  F1-F4      Switch tabs
  Up/Down    Navigate list
  s          Start/Stop runner
  r          Restart runner
  d          Delete runner
  l          View logs
  e          Edit labels
  a          Add runner
  +/-        Scale group up/down
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

fn draw_login_popup(f: &mut Frame, login_state: &crate::app::LoginState) {
    use ratatui::widgets::{Clear, Paragraph};

    let area = f.area();
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 10.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let text = match login_state {
        crate::app::LoginState::Polling {
            user_code,
            verification_uri,
            ..
        } => {
            format!(
                "\n  Open: {verification_uri}\n  Code: {user_code}\n\n  Waiting for authorization...\n\n  [Esc] Cancel"
            )
        }
        crate::app::LoginState::Success { username } => {
            format!("\n  Logged in as {username}!")
        }
        crate::app::LoginState::Error { message } => {
            format!("\n  Login failed: {message}\n\n  [Esc] Dismiss")
        }
    };

    f.render_widget(Clear, popup_area);
    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" Login "))
            .style(Style::default().fg(Color::White)),
        popup_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::LoginState;
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
    fn test_full_draw_renders_homerun_title() {
        let app = App::new();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw(f, &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("HomeRun"),
            "should render the HomeRun title in the outer border"
        );
    }

    #[test]
    fn test_draw_renders_tab_names() {
        let app = App::new();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw(f, &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Runners"), "should render Runners tab");
        assert!(content.contains("Repos"), "should render Repos tab");
        assert!(
            content.contains("Monitoring"),
            "should render Monitoring tab"
        );
        assert!(content.contains("Daemon"), "should render Daemon tab");
    }

    #[test]
    fn test_draw_renders_header_info() {
        let app = App::new();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw(f, &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("User:"),
            "should render User label in header"
        );
        assert!(
            content.contains("Daemon:"),
            "should render Daemon label in header"
        );
    }

    #[test]
    fn test_help_popup_renders_when_show_help() {
        let mut app = App::new();
        app.show_help = true;

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw(f, &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Help"), "should render Help popup title");
        assert!(
            content.contains("Keybindings"),
            "should render keybindings content in help popup"
        );
    }

    #[test]
    fn test_login_popup_renders_when_login_state_polling() {
        let mut app = App::new();
        app.login_state = Some(LoginState::Polling {
            device_code: "DEVCODE".to_string(),
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            interval: 5,
        });

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw(f, &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(content.contains("Login"), "should render Login popup title");
        assert!(
            content.contains("ABCD-1234"),
            "should render user code in login popup"
        );
    }
}
