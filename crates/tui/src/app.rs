use crossterm::event::{KeyCode, KeyModifiers};

use crate::client::{AuthStatus, MetricsResponse, RepoInfo, RunnerInfo};

/// Actions that require async daemon calls — returned from handle_key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    StartRunner(String),
    StopRunner(String),
    RestartRunner(String),
    DeleteRunner(String),
    RefreshRunners,
    RefreshRepos,
    RefreshMetrics,
}

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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
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
            self.selected_repo_index = (self.selected_repo_index + 1).min(self.repos.len() - 1);
        }
    }

    pub fn select_prev_repo(&mut self) {
        self.selected_repo_index = self.selected_repo_index.saturating_sub(1);
    }

    /// Handle a key event. Returns an optional Action requiring a daemon call.
    pub fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> Option<Action> {
        // Help overlay captures all keys except ? and Esc
        if self.show_help {
            match code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                _ => {}
            }
            return None;
        }

        match code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                None
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                None
            }
            KeyCode::Char('1') => {
                self.active_tab = Tab::Runners;
                None
            }
            KeyCode::Char('2') => {
                self.active_tab = Tab::Repos;
                if self.repos.is_empty() {
                    Some(Action::RefreshRepos)
                } else {
                    None
                }
            }
            KeyCode::Char('3') => {
                self.active_tab = Tab::Monitoring;
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match self.active_tab {
                    Tab::Runners => self.select_next_runner(),
                    Tab::Repos => self.select_next_repo(),
                    _ => {}
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match self.active_tab {
                    Tab::Runners => self.select_prev_runner(),
                    Tab::Repos => self.select_prev_repo(),
                    _ => {}
                }
                None
            }
            KeyCode::Char('s') => {
                if let Some(runner) = self.selected_runner() {
                    let id = runner.config.id.clone();
                    let action = if runner.state == "online" || runner.state == "busy" {
                        Action::StopRunner(id)
                    } else {
                        Action::StartRunner(id)
                    };
                    return Some(action);
                }
                None
            }
            KeyCode::Char('r') => {
                if let Some(runner) = self.selected_runner() {
                    return Some(Action::RestartRunner(runner.config.id.clone()));
                }
                None
            }
            KeyCode::Char('d') => {
                if let Some(runner) = self.selected_runner() {
                    return Some(Action::DeleteRunner(runner.config.id.clone()));
                }
                None
            }
            _ => None,
        }
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

    fn make_test_runner(id: &str, state: &str) -> crate::client::RunnerInfo {
        crate::client::RunnerInfo {
            config: crate::client::RunnerConfig {
                id: id.to_string(),
                name: format!("runner-{id}"),
                repo_owner: "test".to_string(),
                repo_name: "repo".to_string(),
                labels: vec!["self-hosted".to_string()],
                mode: "app".to_string(),
                work_dir: std::path::PathBuf::from("/tmp"),
            },
            state: state.to_string(),
            pid: None,
            uptime_secs: None,
            jobs_completed: 0,
            jobs_failed: 0,
        }
    }

    #[test]
    fn test_handle_key_quit() {
        let mut app = App::new();
        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_key_tab_switch() {
        let mut app = App::new();
        app.handle_key(KeyCode::Char('2'), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Repos);
        app.handle_key(KeyCode::Char('3'), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Monitoring);
        app.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Runners);
    }

    #[test]
    fn test_handle_key_help_toggle() {
        let mut app = App::new();
        assert!(!app.show_help);
        app.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(app.show_help);
        app.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(!app.show_help);
    }

    #[test]
    fn test_handle_key_navigation() {
        let mut app = App::new();
        app.runners = vec![
            make_test_runner("r1", "online"),
            make_test_runner("r2", "busy"),
            make_test_runner("r3", "offline"),
        ];
        assert_eq!(app.selected_runner_index, 0);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 1);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 2);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 2); // stays at end
        app.handle_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 1);
    }

    #[test]
    fn test_handle_key_vim_navigation() {
        let mut app = App::new();
        app.runners = vec![
            make_test_runner("r1", "online"),
            make_test_runner("r2", "busy"),
        ];
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 1);
        app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(app.selected_runner_index, 0);
    }
}
