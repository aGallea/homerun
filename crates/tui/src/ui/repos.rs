use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn draw_repos(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_repo_list(f, app, chunks[0]);
    draw_repo_detail(f, app, chunks[1]);
}

fn draw_repo_list(f: &mut Frame, app: &App, area: Rect) {
    if !app.auth_status.as_ref().is_some_and(|a| a.authenticated) {
        let msg = Paragraph::new(" Not authenticated.\n\n Run: homerun login --token <PAT>")
            .block(Block::default().borders(Borders::ALL).title(" Repos "));
        f.render_widget(msg, area);
        return;
    }

    let show_search = app.repo_searching || !app.repo_search.is_empty();

    let (search_area, list_area) = if show_search {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let Some(search_area) = search_area {
        let search_text = if app.repo_searching {
            format!(" Search: {}▏", app.repo_search)
        } else {
            format!(" Search: {}", app.repo_search)
        };
        let search_bar = Paragraph::new(search_text).style(Style::default().fg(Color::Yellow));
        f.render_widget(search_bar, search_area);
    }

    let filtered_repos: Vec<(usize, _)> = app
        .repos
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            if app.repo_search.is_empty() {
                true
            } else {
                r.full_name
                    .to_lowercase()
                    .contains(&app.repo_search.to_lowercase())
            }
        })
        .collect();

    let items: Vec<ListItem> = filtered_repos
        .iter()
        .map(|(_, r)| {
            let visibility = if r.private { "private" } else { "public" };
            let org_marker = if r.is_org { " [org]" } else { "" };
            let line = Line::from(vec![
                Span::raw(r.full_name.as_str()),
                Span::styled(
                    format!(" ({visibility}{org_marker})"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = if app.repo_search.is_empty() {
        format!(" Repos ({}) ", app.repos.len())
    } else {
        format!(" Repos ({}/{}) ", filtered_repos.len(), app.repos.len())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    // Map the app's selected_repo_index to the filtered list position
    let selected_in_filtered = filtered_repos
        .iter()
        .position(|(i, _)| *i == app.selected_repo_index);

    let mut list_state = ListState::default();
    if !filtered_repos.is_empty() {
        list_state.select(Some(selected_in_filtered.unwrap_or(0)));
    }

    f.render_stateful_widget(list, list_area, &mut list_state);
}

fn draw_repo_detail(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.repos.get(app.selected_repo_index) {
        Some(repo) => {
            let runner_count = app
                .runners
                .iter()
                .filter(|r| r.config.repo_owner == repo.owner && r.config.repo_name == repo.name)
                .count();
            format!(
                " Repository: {}\n\
                 \n\
                  Owner:      {}\n\
                  Visibility: {}\n\
                  URL:        {}\n\
                  Runners:    {}\n\
                 \n\
                  Press 'a' to add a runner for this repo.",
                repo.full_name,
                repo.owner,
                if repo.private { "Private" } else { "Public" },
                repo.html_url,
                runner_count,
            )
        }
        None => " No repos loaded.\n\n Authenticate first, then repos will appear.".to_string(),
    };

    let paragraph =
        Paragraph::new(content).block(Block::default().borders(Borders::ALL).title(" Detail "));

    f.render_widget(paragraph, area);
}
