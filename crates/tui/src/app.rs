use crate::client::{AuthStatus, MetricsResponse, RunnerInfo, RepoInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Runners,
    Repos,
    Monitoring,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Runners, Tab::Repos, Tab::Monitoring]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Runners => "Runners",
            Tab::Repos => "Repos",
            Tab::Monitoring => "Monitoring",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Tab::Runners => 0,
            Tab::Repos => 1,
            Tab::Monitoring => 2,
        }
    }

    pub fn from_index(i: usize) -> Option<Tab> {
        match i {
            0 => Some(Tab::Runners),
            1 => Some(Tab::Repos),
            2 => Some(Tab::Monitoring),
            _ => None,
        }
    }
}

pub struct App {
    pub active_tab: Tab,
    pub should_quit: bool,
    pub runners: Vec<RunnerInfo>,
    pub selected_runner_index: usize,
    pub repos: Vec<RepoInfo>,
    pub selected_repo_index: usize,
    pub auth_status: Option<AuthStatus>,
    pub metrics: Option<MetricsResponse>,
    pub show_help: bool,
    pub status_message: Option<String>,
    pub daemon_connected: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Runners,
            should_quit: false,
            runners: Vec::new(),
            selected_runner_index: 0,
            repos: Vec::new(),
            selected_repo_index: 0,
            auth_status: None,
            metrics: None,
            show_help: false,
            status_message: None,
            daemon_connected: false,
        }
    }

    pub fn select_next_runner(&mut self) {
        if !self.runners.is_empty() {
            self.selected_runner_index =
                (self.selected_runner_index + 1).min(self.runners.len() - 1);
        }
    }

    pub fn select_prev_runner(&mut self) {
        self.selected_runner_index = self.selected_runner_index.saturating_sub(1);
    }

    pub fn selected_runner(&self) -> Option<&RunnerInfo> {
        self.runners.get(self.selected_runner_index)
    }

    pub fn select_next_repo(&mut self) {
        if !self.repos.is_empty() {
            self.selected_repo_index =
                (self.selected_repo_index + 1).min(self.repos.len() - 1);
        }
    }

    pub fn select_prev_repo(&mut self) {
        self.selected_repo_index = self.selected_repo_index.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_default_state() {
        let app = App::new();
        assert_eq!(app.active_tab, Tab::Runners);
        assert_eq!(app.selected_runner_index, 0);
        assert!(!app.should_quit);
        assert!(app.runners.is_empty());
    }

    #[test]
    fn test_tab_cycling() {
        let mut app = App::new();
        assert_eq!(app.active_tab, Tab::Runners);
        app.active_tab = Tab::Repos;
        assert_eq!(app.active_tab, Tab::Repos);
        app.active_tab = Tab::Monitoring;
        assert_eq!(app.active_tab, Tab::Monitoring);
    }

    #[test]
    fn test_runner_selection_bounds() {
        let mut app = App::new();
        // With no runners, selection stays at 0
        app.select_next_runner();
        assert_eq!(app.selected_runner_index, 0);
        app.select_prev_runner();
        assert_eq!(app.selected_runner_index, 0);
    }
}
