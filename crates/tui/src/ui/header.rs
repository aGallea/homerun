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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Tab;
    use crate::client::{AuthStatus, GitHubUser, RunnerConfig, RunnerInfo};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

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

    fn make_runner(name: &str, state: &str) -> RunnerInfo {
        RunnerInfo {
            config: RunnerConfig {
                id: format!("id-{name}"),
                name: name.to_string(),
                repo_owner: "owner".to_string(),
                repo_name: "repo".to_string(),
                labels: vec!["self-hosted".to_string()],
                mode: "app".to_string(),
                work_dir: PathBuf::from("/tmp"),
                group_id: None,
            },
            state: state.to_string(),
            pid: None,
            uptime_secs: None,
            jobs_completed: 0,
            jobs_failed: 0,
            current_job: None,
            job_context: None,
            job_started_at: None,
            estimated_job_duration_secs: None,
            last_completed_job: None,
        }
    }

    #[test]
    fn test_renders_not_logged_in_when_auth_none() {
        let app = App::new();
        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_header(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("Not logged in"),
            "should show 'Not logged in' when auth_status is None"
        );
    }

    #[test]
    fn test_renders_username_when_authenticated() {
        let mut app = App::new();
        app.auth_status = Some(AuthStatus {
            authenticated: true,
            user: Some(GitHubUser {
                login: "testuser".to_string(),
                avatar_url: String::new(),
            }),
        });

        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_header(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("testuser"),
            "should show the username when authenticated"
        );
    }

    #[test]
    fn test_renders_connected_when_daemon_connected() {
        let mut app = App::new();
        app.daemon_connected = true;

        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_header(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("Connected"),
            "should show 'Connected' when daemon is connected"
        );
    }

    #[test]
    fn test_renders_disconnected_when_daemon_not_connected() {
        let mut app = App::new();
        app.daemon_connected = false;

        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_header(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("Disconnected"),
            "should show 'Disconnected' when daemon is not connected"
        );
    }

    #[test]
    fn test_renders_runner_summary_counts() {
        let mut app = App::new();
        app.runners = vec![
            make_runner("r1", "online"),
            make_runner("r2", "online"),
            make_runner("r3", "busy"),
            make_runner("r4", "offline"),
        ];

        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_header(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("4 total"),
            "should show total runner count"
        );
        assert!(
            content.contains("2 online"),
            "should show online runner count"
        );
        assert!(content.contains("1 busy"), "should show busy runner count");
        assert!(
            content.contains("1 offline"),
            "should show offline runner count"
        );
    }

    #[test]
    fn test_renders_key_hints_runners_tab() {
        let mut app = App::new();
        app.active_tab = Tab::Runners;

        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_header(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("Start/Stop"),
            "Runners tab should show Start/Stop hint"
        );
        assert!(
            content.contains("Add Runner"),
            "Runners tab should show Add Runner hint"
        );
    }

    #[test]
    fn test_renders_key_hints_daemon_tab() {
        let mut app = App::new();
        app.active_tab = Tab::Daemon;

        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_header(f, &app, f.area());
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content = buffer_to_string(&buffer);
        assert!(
            content.contains("Start Daemon"),
            "Daemon tab should show Start Daemon hint"
        );
        assert!(
            content.contains("Log Level"),
            "Daemon tab should show Log Level hint"
        );
    }
}
