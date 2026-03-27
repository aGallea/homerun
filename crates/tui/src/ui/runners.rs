use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, DisplayItem};
use crate::client::{RunnerInfo, StepInfo};

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
    // Determine how many runners are in each group for tree markers
    // We need to know if a runner is the last child in its group
    let mut group_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for item in &app.display_items {
        if let DisplayItem::RunnerRow {
            group_id: Some(gid),
            ..
        } = item
        {
            *group_counts.entry(gid.clone()).or_insert(0) += 1;
        }
    }

    // Track position of each runner within its group
    let mut group_positions: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    let items: Vec<ListItem> = app
        .display_items
        .iter()
        .map(|item| match item {
            DisplayItem::GroupRow {
                group_id,
                name_prefix,
                runner_count,
                status_summary,
            } => {
                let expanded = app.expanded_groups.contains(group_id);
                let arrow = if expanded { "▼" } else { "▶" };

                // Build colored status dots
                let mut spans = vec![
                    Span::styled(format!("{arrow} "), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        format!("{name_prefix} "),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("({runner_count}) "),
                        Style::default().fg(Color::DarkGray),
                    ),
                ];

                // Add status dots sorted by state name for stability
                let mut states: Vec<_> = status_summary.iter().collect();
                states.sort_by_key(|(s, _)| s.as_str());
                for (state, count) in states {
                    let color = state_color(state);
                    spans.push(Span::styled(
                        format!("●×{count} "),
                        Style::default().fg(color),
                    ));
                }

                ListItem::new(Line::from(spans))
            }
            DisplayItem::RunnerRow {
                runner_index,
                group_id,
            } => {
                let runner = &app.runners[*runner_index];
                let status_color = state_color(&runner.state);

                if let Some(gid) = group_id {
                    let pos = group_positions.entry(gid.clone()).or_insert(0);
                    *pos += 1;
                    let current_pos = *pos;
                    let total = group_counts.get(gid).copied().unwrap_or(1);
                    let tree_marker = if current_pos == total {
                        "└─"
                    } else {
                        "├─"
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("  {tree_marker} "),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled("● ", Style::default().fg(status_color)),
                        Span::raw(runner.config.name.as_str()),
                        Span::styled(
                            format!(" ({})", runner.state),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::styled("● ", Style::default().fg(status_color)),
                        Span::raw(runner.config.name.as_str()),
                        Span::styled(
                            format!(" ({})", runner.state),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]))
                }
            }
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
    if !app.display_items.is_empty() {
        list_state.select(Some(app.selected_display_index));
    }

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_runner_detail(f: &mut Frame, app: &App, area: Rect) {
    match app.selected_display_item() {
        Some(DisplayItem::RunnerRow { runner_index, .. }) => {
            if let Some(runner) = app.runners.get(*runner_index) {
                draw_runner_panels(f, app, runner, area);
            } else {
                draw_empty_detail(f, area);
            }
        }
        Some(DisplayItem::GroupRow {
            group_id,
            name_prefix,
            runner_count,
            status_summary,
        }) => {
            let s = format_group_detail(group_id, name_prefix, *runner_count, status_summary);
            let lines: Vec<Line> = s.lines().map(|l| Line::from(l.to_string())).collect();
            let paragraph = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(" Group "));
            f.render_widget(paragraph, area);
        }
        None => {
            draw_empty_detail(f, area);
        }
    }
}

fn draw_empty_detail(f: &mut Frame, area: Rect) {
    let content = vec![
        Line::from(" No runner selected."),
        Line::from(""),
        Line::from(" Press 'a' to add a new runner."),
    ];
    let paragraph =
        Paragraph::new(content).block(Block::default().borders(Borders::ALL).title(" Detail "));
    f.render_widget(paragraph, area);
}

fn draw_runner_panels(f: &mut Frame, app: &App, runner: &RunnerInfo, area: Rect) {
    let has_progress = runner.state == "busy"
        && (app.selected_runner_steps.is_some() || runner.estimated_job_duration_secs.is_some());
    let has_history = !app.selected_runner_history.is_empty();

    // Dynamic layout based on what's available
    let has_second_panel = has_progress || has_history;
    let chunks = if has_progress && has_history {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12), // detail
                Constraint::Min(5),     // progress
                Constraint::Length(14), // history
            ])
            .split(area)
    } else if has_second_panel {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(12), Constraint::Min(5)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(area)
    };

    // Panel 1: Runner details
    let detail_text = format_runner_detail(runner, app);
    let detail_lines: Vec<Line> = detail_text
        .lines()
        .map(|l| Line::from(l.to_string()))
        .collect();
    let detail = Paragraph::new(detail_lines)
        .block(Block::default().borders(Borders::ALL).title(" Detail "));
    f.render_widget(detail, chunks[0]);

    // Panel 2 & 3: Progress and/or History
    if has_progress && has_history {
        draw_progress_panel(f, app, runner, chunks[1]);
        draw_history_panel(f, app, chunks[2]);
    } else if has_progress {
        draw_progress_panel(f, app, runner, chunks[1]);
    } else if has_second_panel {
        draw_history_panel(f, app, chunks[1]);
    }
}

fn draw_progress_panel(f: &mut Frame, app: &App, runner: &RunnerInfo, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // Progress bar first (always visible at top)
    if let (Some(ref started_str), Some(estimate)) =
        (&runner.job_started_at, runner.estimated_job_duration_secs)
    {
        if estimate > 0 {
            if let Ok(started) = chrono::DateTime::parse_from_rfc3339(started_str) {
                let elapsed = (chrono::Utc::now() - started.with_timezone(&chrono::Utc))
                    .num_seconds()
                    .max(0) as u64;
                let pct = ((elapsed as f64 / estimate as f64) * 100.0).min(100.0) as u16;
                let bar_width = 20u16;
                let filled = ((pct as f64 / 100.0) * bar_width as f64) as usize;
                let empty = bar_width as usize - filled;
                let bar = format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty));
                let elapsed_m = elapsed / 60;
                let estimate_m = estimate / 60;
                let color = if pct >= 100 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(bar, Style::default().fg(color)),
                    Span::styled(
                        format!(" {pct}% ({elapsed_m}m / ~{estimate_m}m)"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                lines.push(Line::from(""));
            }
        }
    }

    // Steps — show from bottom so the running/latest step is always visible
    if let Some(ref steps_resp) = app.selected_runner_steps {
        // Calculate how many lines we have left in the panel
        // Panel inner height = area.height - 2 (borders) - lines already used
        let available = (area.height as usize).saturating_sub(2 + lines.len());
        let steps = &steps_resp.steps;
        let skip = steps.len().saturating_sub(available);

        if skip > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ... {skip} more steps above"),
                Style::default().fg(Color::DarkGray),
            )));
        }

        for step in steps.iter().skip(skip) {
            let (icon, color) = match step.status.as_str() {
                "succeeded" => ("\u{2713}", Color::Green),
                "failed" => ("\u{2715}", Color::Red),
                "running" => ("\u{27F3}", Color::Yellow),
                "skipped" => ("\u{2298}", Color::DarkGray),
                _ => ("\u{25CB}", Color::DarkGray),
            };
            let duration_str = format_step_duration(step);
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(format!("{icon} "), Style::default().fg(color)),
                Span::styled(step.name.clone(), Style::default().fg(color)),
                Span::styled(duration_str, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    let title = app
        .selected_runner_steps
        .as_ref()
        .map(|s| format!(" Progress: {} ", s.job_name))
        .unwrap_or_else(|| " Progress ".to_string());
    let paragraph =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(paragraph, area);
}

fn draw_history_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for entry in app.selected_runner_history.iter().rev().take(10) {
        let icon = if entry.succeeded {
            "\u{2713}"
        } else {
            "\u{2717}"
        };
        let color = if entry.succeeded {
            Color::Green
        } else {
            Color::Red
        };
        let branch_str = entry
            .branch
            .as_deref()
            .map(|b| {
                if let Some(pr) = entry.pr_number {
                    format!(" ({b} PR #{pr})")
                } else {
                    format!(" ({b})")
                }
            })
            .unwrap_or_default();
        let time_str = entry
            .started_at
            .parse::<chrono::DateTime<chrono::Utc>>()
            .map(|dt| dt.format("%H:%M").to_string())
            .unwrap_or_default();
        let duration_str = if entry.duration_secs > 0 {
            let mins = entry.duration_secs / 60;
            let secs = entry.duration_secs % 60;
            if mins > 0 {
                format!(" {mins}m{secs}s")
            } else {
                format!(" {secs}s")
            }
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::styled(&entry.job_name, Style::default().fg(color)),
            Span::styled(branch_str, Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("  {time_str}{duration_str}"),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    let paragraph =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" History "));
    f.render_widget(paragraph, area);
}

fn format_group_detail(
    group_id: &str,
    name_prefix: &str,
    runner_count: usize,
    status_summary: &std::collections::HashMap<String, usize>,
) -> String {
    let mut states: Vec<_> = status_summary.iter().collect();
    states.sort_by_key(|(s, _)| s.as_str());
    let status_str: Vec<String> = states
        .iter()
        .map(|(state, count)| format!("{count} {state}"))
        .collect();

    format!(
        " Group:   {name_prefix}\n\
         \n\
          ID:      {group_id}\n\
          Runners: {runner_count}\n\
          Status:  {}\n\
         \n\
          [S] start all  [X] stop all  [r] restart all  [d] delete all\n\
          [+] scale up   [-] scale down\n\
          [Enter/→] expand  [←] collapse",
        status_str.join(", "),
    )
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

    // Show current job and branch/PR info if busy
    if let Some(job) = &runner.current_job {
        lines.push_str(&format!("\n Current: {}\n", job));
        if let Some(ctx) = &runner.job_context {
            let branch_line = if let Some(pr_num) = ctx.pr_number {
                format!(" Branch:  {} (PR #{})\n", ctx.branch, pr_num)
            } else {
                format!(" Branch:  {}\n", ctx.branch)
            };
            lines.push_str(&branch_line);
        }
    }

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

fn format_step_duration(step: &StepInfo) -> String {
    // Try to compute duration from started_at / completed_at timestamps
    if let Some(ref started) = step.started_at {
        if let Some(ref completed) = step.completed_at {
            // Both are ISO 8601 timestamps; parse and diff
            if let (Ok(s), Ok(e)) = (
                chrono::DateTime::parse_from_rfc3339(started),
                chrono::DateTime::parse_from_rfc3339(completed),
            ) {
                let secs = (e - s).num_seconds();
                return format!("  {secs}s");
            }
        } else if step.status == "running" {
            // Running step: show elapsed with trailing ellipsis
            if let Ok(s) = chrono::DateTime::parse_from_rfc3339(started) {
                let now = chrono::Utc::now();
                let secs = (now - s.with_timezone(&chrono::Utc)).num_seconds();
                return format!("  {secs}s\u{2026}");
            }
        }
    }
    String::new()
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
