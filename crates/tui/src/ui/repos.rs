use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

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

    let items: Vec<ListItem> = app
        .repos
        .iter()
        .map(|r| {
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

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Repos ({}) ", app.repos.len())),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !app.repos.is_empty() {
        list_state.select(Some(app.selected_repo_index));
    }

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_repo_detail(f: &mut Frame, app: &App, area: Rect) {
    let content = match app.repos.get(app.selected_repo_index) {
        Some(repo) => {
            let runner_count = app
                .runners
                .iter()
                .filter(|r| {
                    r.config.repo_owner == repo.owner && r.config.repo_name == repo.name
                })
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

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(" Detail "));

    f.render_widget(paragraph, area);
}
