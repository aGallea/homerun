use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::client::RunnerInfo;

pub fn draw_runners(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_runner_list(f, app, chunks[0]);
    draw_runner_detail(f, app, chunks[1]);
}

fn state_color(state: &str) -> Color {
    match state {
        "online" => Color::Green,
        "busy" => Color::Yellow,
        "offline" => Color::Gray,
        "error" => Color::Red,
        "creating" | "registering" => Color::Cyan,
        "stopping" | "deleting" => Color::Magenta,
        _ => Color::White,
    }
}

fn draw_runner_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .runners
        .iter()
        .map(|r| {
            let status_color = state_color(&r.state);
            let line = Line::from(vec![
                Span::styled("● ", Style::default().fg(status_color)),
                Span::raw(r.config.name.as_str()),
                Span::styled(
                    format!(" ({})", r.state),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Runners ({}) ", app.runners.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !app.runners.is_empty() {
        list_state.select(Some(app.selected_runner_index));
    }

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_runner_detail(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.selected_runner() {
        Some(runner) => format_runner_detail(runner, app),
        None => " No runner selected.\n\n Press 'a' to add a new runner.".to_string(),
    };

    let paragraph =
        Paragraph::new(content).block(Block::default().borders(Borders::ALL).title(" Detail "));

    f.render_widget(paragraph, area);
}

fn format_runner_detail(runner: &RunnerInfo, app: &App) -> String {
    let repo = format!("{}/{}", runner.config.repo_owner, runner.config.repo_name);
    let labels = runner.config.labels.join(", ");
    let uptime = runner
        .uptime_secs
        .map(format_duration)
        .unwrap_or_else(|| "—".to_string());

    let mut lines = format!(
        " Name:    {}\n\
         \n\
          State:   {}\n\
          Repo:    {}\n\
          Mode:    {}\n\
          Labels:  {}\n\
          Uptime:  {}\n\
          Jobs:    {} completed, {} failed\n",
        runner.config.name,
        runner.state,
        repo,
        runner.config.mode,
        labels,
        uptime,
        runner.jobs_completed,
        runner.jobs_failed,
    );

    // Show per-runner metrics if available
    if let Some(ref metrics) = app.metrics {
        if let Some(rm) = metrics
            .runners
            .iter()
            .find(|m| m.runner_id == runner.config.id)
        {
            lines.push_str(&format!(
                "\n CPU:     {:.1}%\n Memory:  {}\n",
                rm.cpu_percent,
                format_bytes(rm.memory_bytes),
            ));
        }
    }

    lines.push_str("\n [s] start/stop  [r] restart  [d] delete  [e] edit  [l] logs");
    lines
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
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

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0m");
        assert_eq!(format_duration(90), "1m");
        assert_eq!(format_duration(3661), "1h 1m");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn test_state_color_mapping() {
        assert_eq!(state_color("online"), Color::Green);
        assert_eq!(state_color("error"), Color::Red);
        assert_eq!(state_color("busy"), Color::Yellow);
    }
}
