use std::collections::{HashMap, HashSet};

use crossterm::event::{KeyCode, KeyModifiers};

use crate::client::{
    AuthStatus, DaemonLogEntry, JobHistoryEntry, MetricsResponse, RepoInfo, RunnerInfo,
    StepsResponse,
};

/// Actions that require async daemon calls — returned from handle_key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    StartRunner(String),
    StopRunner(String),
    RestartRunner(String),
    DeleteRunner(String),
    StartGroup(String),
    StopGroup(String),
    RestartGroup(String),
    DeleteGroup(String),
    ScaleUp(String),
    ScaleDown(String),
    RefreshRunners,
    RefreshRepos,
    RefreshMetrics,
    RefreshDaemonLogs,
    StartDaemon,
    StopDaemon,
    RestartDaemon,
    StartLogin,
}

#[derive(Debug, Clone)]
pub enum DisplayItem {
    GroupRow {
        group_id: String,
        name_prefix: String,
        runner_count: usize,
        status_summary: HashMap<String, usize>,
    },
    RunnerRow {
        runner_index: usize,
        group_id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Runners,
    Repos,
    Monitoring,
    Daemon,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Runners, Tab::Repos, Tab::Monitoring, Tab::Daemon]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Runners => "Runners",
            Tab::Repos => "Repos",
            Tab::Monitoring => "Monitoring",
            Tab::Daemon => "Daemon",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Tab::Runners => 0,
            Tab::Repos => 1,
            Tab::Monitoring => 2,
            Tab::Daemon => 3,
        }
    }

    pub fn from_index(i: usize) -> Option<Tab> {
        match i {
            0 => Some(Tab::Runners),
            1 => Some(Tab::Repos),
            2 => Some(Tab::Monitoring),
            3 => Some(Tab::Daemon),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginState {
    Polling {
        device_code: String,
        user_code: String,
        verification_uri: String,
        interval: u64,
    },
    Success {
        username: String,
    },
    Error {
        message: String,
    },
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
    pub expanded_groups: HashSet<String>,
    pub display_items: Vec<DisplayItem>,
    pub selected_display_index: usize,
    pub daemon_logs: Vec<DaemonLogEntry>,
    pub daemon_log_scroll: usize,
    pub daemon_follow: bool,
    pub daemon_log_level: String,
    pub daemon_search: String,
    pub daemon_searching: bool,
    pub selected_runner_steps: Option<StepsResponse>,
    pub selected_runner_history: Vec<JobHistoryEntry>,
    pub login_state: Option<LoginState>,
    pub repo_search: String,
    pub repo_searching: bool,
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
            expanded_groups: HashSet::new(),
            display_items: Vec::new(),
            selected_display_index: 0,
            daemon_logs: Vec::new(),
            daemon_log_scroll: 0,
            daemon_follow: true,
            daemon_log_level: "INFO".to_string(),
            daemon_search: String::new(),
            daemon_searching: false,
            selected_runner_steps: None,
            selected_runner_history: Vec::new(),
            login_state: None,
            repo_search: String::new(),
            repo_searching: false,
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

    pub fn select_next(&mut self) {
        if !self.display_items.is_empty() {
            self.selected_display_index =
                (self.selected_display_index + 1).min(self.display_items.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_display_index = self.selected_display_index.saturating_sub(1);
    }

    pub fn selected_display_item(&self) -> Option<&DisplayItem> {
        self.display_items.get(self.selected_display_index)
    }

    /// Returns the RunnerInfo for the currently selected item, if it's a RunnerRow
    pub fn selected_runner(&self) -> Option<&RunnerInfo> {
        match self.selected_display_item() {
            Some(DisplayItem::RunnerRow { runner_index, .. }) => self.runners.get(*runner_index),
            _ => None,
        }
    }

    pub fn select_next_repo(&mut self) {
        if !self.repos.is_empty() {
            self.selected_repo_index = (self.selected_repo_index + 1).min(self.repos.len() - 1);
        }
    }

    pub fn select_prev_repo(&mut self) {
        self.selected_repo_index = self.selected_repo_index.saturating_sub(1);
    }

    pub fn toggle_group(&mut self, group_id: &str) {
        if self.expanded_groups.contains(group_id) {
            self.expanded_groups.remove(group_id);
        } else {
            self.expanded_groups.insert(group_id.to_string());
        }
    }

    pub fn rebuild_display_items(&mut self) {
        let mut items = Vec::new();
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        let mut solo_indices: Vec<usize> = Vec::new();

        for (i, runner) in self.runners.iter().enumerate() {
            if let Some(ref gid) = runner.config.group_id {
                groups.entry(gid.clone()).or_default().push(i);
            } else {
                solo_indices.push(i);
            }
        }

        // Sort groups by first runner's name for stable ordering
        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by(|a, b| {
            let name_a = &self.runners[a.1[0]].config.name;
            let name_b = &self.runners[b.1[0]].config.name;
            name_a.cmp(name_b)
        });

        for (group_id, indices) in &sorted_groups {
            let first_runner = &self.runners[indices[0]];
            let name_prefix = first_runner
                .config
                .name
                .rsplit_once('-')
                .map(|(prefix, _)| prefix.to_string())
                .unwrap_or_else(|| first_runner.config.name.clone());

            let mut status_summary = HashMap::new();
            for &idx in indices {
                *status_summary
                    .entry(self.runners[idx].state.clone())
                    .or_insert(0) += 1;
            }

            items.push(DisplayItem::GroupRow {
                group_id: group_id.clone(),
                name_prefix,
                runner_count: indices.len(),
                status_summary,
            });

            if self.expanded_groups.contains(group_id) {
                for &idx in indices {
                    items.push(DisplayItem::RunnerRow {
                        runner_index: idx,
                        group_id: Some(group_id.clone()),
                    });
                }
            }
        }

        for idx in solo_indices {
            items.push(DisplayItem::RunnerRow {
                runner_index: idx,
                group_id: None,
            });
        }

        self.display_items = items;
        // Clamp selection
        if self.selected_display_index >= self.display_items.len() && !self.display_items.is_empty()
        {
            self.selected_display_index = self.display_items.len() - 1;
        }
    }

    /// Returns context-sensitive key hints for the active tab.
    /// Each row is a Vec of (key, description) pairs laid out as columns.
    pub fn key_hints(&self) -> Vec<Vec<(&'static str, &'static str)>> {
        let is_authenticated = self.auth_status.as_ref().is_some_and(|a| a.authenticated);

        let tab_col = [
            ("F1", "Runners"),
            ("F2", "Repos"),
            ("F3", "Monitoring"),
            ("F4", "Daemon"),
        ];

        let (action_col, extra_col, util_col) = match self.active_tab {
            Tab::Runners => (
                vec![
                    ("s", "Start/Stop"),
                    ("r", "Restart"),
                    ("d", "Delete"),
                    ("+", "Scale Up"),
                ],
                vec![
                    ("a", "Add Runner"),
                    ("l", "Logs"),
                    ("e", "Edit Labels"),
                    ("-", "Scale Down"),
                ],
                if is_authenticated {
                    vec![("?", "Help"), ("q", "Quit")]
                } else {
                    vec![("L", "Login"), ("?", "Help"), ("q", "Quit")]
                },
            ),
            Tab::Repos => (
                vec![("a", "Add Runner"), ("/", "Search")],
                vec![],
                if is_authenticated {
                    vec![("?", "Help"), ("q", "Quit")]
                } else {
                    vec![("L", "Login"), ("?", "Help"), ("q", "Quit")]
                },
            ),
            Tab::Monitoring => (
                vec![],
                vec![],
                if is_authenticated {
                    vec![("?", "Help"), ("q", "Quit")]
                } else {
                    vec![("L", "Login"), ("?", "Help"), ("q", "Quit")]
                },
            ),
            Tab::Daemon => (
                vec![
                    ("s", "Start Daemon"),
                    ("x", "Stop Daemon"),
                    ("r", "Restart"),
                ],
                vec![("1..5", "Log Level"), ("/", "Search"), ("f", "Follow")],
                if is_authenticated {
                    vec![("?", "Help"), ("q", "Quit")]
                } else {
                    vec![("L", "Login"), ("?", "Help"), ("q", "Quit")]
                },
            ),
        };

        let mut rows: Vec<Vec<(&'static str, &'static str)>> = Vec::with_capacity(tab_col.len());

        for (i, &tab_hint) in tab_col.iter().enumerate() {
            let mut row = Vec::new();
            row.push(tab_hint);
            if let Some(hint) = action_col.get(i) {
                row.push(*hint);
            }
            if let Some(hint) = extra_col.get(i) {
                row.push(*hint);
            }
            if let Some(hint) = util_col.get(i) {
                row.push(*hint);
            }
            rows.push(row);
        }

        rows
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

        // Login popup captures all keys except Esc
        if self.login_state.is_some() {
            if code == KeyCode::Esc {
                self.login_state = None;
            }
            return None;
        }

        // Repos tab search mode captures all input
        if self.active_tab == Tab::Repos && self.repo_searching {
            match code {
                KeyCode::Esc => {
                    self.repo_searching = false;
                    self.repo_search.clear();
                }
                KeyCode::Enter => {
                    self.repo_searching = false;
                }
                KeyCode::Backspace => {
                    self.repo_search.pop();
                }
                KeyCode::Char(c) => {
                    self.repo_search.push(c);
                }
                _ => {}
            }
            return None;
        }

        // Daemon tab search mode captures all input
        if self.active_tab == Tab::Daemon && self.daemon_searching {
            match code {
                KeyCode::Esc => {
                    self.daemon_searching = false;
                    self.daemon_search.clear();
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Enter => {
                    self.daemon_searching = false;
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Backspace => {
                    self.daemon_search.pop();
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Char(c) => {
                    self.daemon_search.push(c);
                    return Some(Action::RefreshDaemonLogs);
                }
                _ => {}
            }
            return None;
        }

        // F1-F4 for tab switching — always works regardless of active tab
        match code {
            KeyCode::F(1) => {
                self.active_tab = Tab::Runners;
                return None;
            }
            KeyCode::F(2) => {
                self.active_tab = Tab::Repos;
                if self.repos.is_empty() {
                    return Some(Action::RefreshRepos);
                }
                return None;
            }
            KeyCode::F(3) => {
                self.active_tab = Tab::Monitoring;
                return None;
            }
            KeyCode::F(4) => {
                self.active_tab = Tab::Daemon;
                if self.daemon_logs.is_empty() {
                    return Some(Action::RefreshDaemonLogs);
                }
                return None;
            }
            _ => {}
        }

        // Daemon tab key handling (before global keys to intercept 1-5 for log levels)
        if self.active_tab == Tab::Daemon {
            match code {
                KeyCode::Char('s') => return Some(Action::StartDaemon),
                KeyCode::Char('x') => return Some(Action::StopDaemon),
                KeyCode::Char('r') => return Some(Action::RestartDaemon),
                KeyCode::Char('1') => {
                    self.daemon_log_level = "TRACE".to_string();
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Char('2') => {
                    self.daemon_log_level = "DEBUG".to_string();
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Char('3') => {
                    self.daemon_log_level = "INFO".to_string();
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Char('4') => {
                    self.daemon_log_level = "WARN".to_string();
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Char('5') => {
                    self.daemon_log_level = "ERROR".to_string();
                    return Some(Action::RefreshDaemonLogs);
                }
                KeyCode::Char('/') => {
                    self.daemon_searching = true;
                    return None;
                }
                KeyCode::Char('f') => {
                    self.daemon_follow = !self.daemon_follow;
                    if self.daemon_follow && !self.daemon_logs.is_empty() {
                        self.daemon_log_scroll = self.daemon_logs.len().saturating_sub(1);
                    }
                    return None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.daemon_logs.is_empty() {
                        self.daemon_log_scroll = (self.daemon_log_scroll + 1)
                            .min(self.daemon_logs.len().saturating_sub(1));
                    }
                    return None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.daemon_log_scroll = self.daemon_log_scroll.saturating_sub(1);
                    self.daemon_follow = false;
                    return None;
                }
                _ => {} // Fall through to global keys
            }
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
            KeyCode::Down | KeyCode::Char('j') => {
                match self.active_tab {
                    Tab::Runners => self.select_next(),
                    Tab::Repos => self.select_next_repo(),
                    _ => {}
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match self.active_tab {
                    Tab::Runners => self.select_prev(),
                    Tab::Repos => self.select_prev_repo(),
                    _ => {}
                }
                None
            }
            KeyCode::Enter | KeyCode::Right => {
                if self.active_tab == Tab::Runners {
                    if let Some(DisplayItem::GroupRow { group_id, .. }) =
                        self.selected_display_item().cloned()
                    {
                        if !self.expanded_groups.contains(&group_id) {
                            self.toggle_group(&group_id);
                            self.rebuild_display_items();
                        }
                    }
                }
                None
            }
            KeyCode::Left => {
                if self.active_tab == Tab::Runners {
                    if let Some(DisplayItem::GroupRow { group_id, .. }) =
                        self.selected_display_item().cloned()
                    {
                        if self.expanded_groups.contains(&group_id) {
                            self.toggle_group(&group_id);
                            self.rebuild_display_items();
                        }
                    }
                }
                None
            }
            KeyCode::Char('s') => {
                if self.active_tab == Tab::Runners {
                    if let Some(DisplayItem::RunnerRow { runner_index, .. }) =
                        self.selected_display_item().cloned()
                    {
                        if let Some(runner) = self.runners.get(runner_index) {
                            let id = runner.config.id.clone();
                            let action = if runner.state == "online" || runner.state == "busy" {
                                Action::StopRunner(id)
                            } else {
                                Action::StartRunner(id)
                            };
                            return Some(action);
                        }
                    }
                }
                None
            }
            KeyCode::Char('S') => {
                if self.active_tab == Tab::Runners {
                    if let Some(DisplayItem::GroupRow { group_id, .. }) =
                        self.selected_display_item().cloned()
                    {
                        return Some(Action::StartGroup(group_id));
                    }
                }
                None
            }
            KeyCode::Char('X') => {
                if self.active_tab == Tab::Runners {
                    if let Some(DisplayItem::GroupRow { group_id, .. }) =
                        self.selected_display_item().cloned()
                    {
                        return Some(Action::StopGroup(group_id));
                    }
                }
                None
            }
            KeyCode::Char('r') => {
                if self.active_tab == Tab::Runners {
                    match self.selected_display_item().cloned() {
                        Some(DisplayItem::GroupRow { group_id, .. }) => {
                            return Some(Action::RestartGroup(group_id));
                        }
                        Some(DisplayItem::RunnerRow { runner_index, .. }) => {
                            if let Some(runner) = self.runners.get(runner_index) {
                                return Some(Action::RestartRunner(runner.config.id.clone()));
                            }
                        }
                        None => {}
                    }
                }
                None
            }
            KeyCode::Char('d') => {
                if self.active_tab == Tab::Runners {
                    match self.selected_display_item().cloned() {
                        Some(DisplayItem::GroupRow { group_id, .. }) => {
                            return Some(Action::DeleteGroup(group_id));
                        }
                        Some(DisplayItem::RunnerRow { runner_index, .. }) => {
                            if let Some(runner) = self.runners.get(runner_index) {
                                return Some(Action::DeleteRunner(runner.config.id.clone()));
                            }
                        }
                        None => {}
                    }
                }
                None
            }
            KeyCode::Char('+') => {
                if self.active_tab == Tab::Runners {
                    if let Some(DisplayItem::GroupRow { group_id, .. }) =
                        self.selected_display_item().cloned()
                    {
                        return Some(Action::ScaleUp(group_id));
                    }
                }
                None
            }
            KeyCode::Char('-') => {
                if self.active_tab == Tab::Runners {
                    if let Some(DisplayItem::GroupRow { group_id, .. }) =
                        self.selected_display_item().cloned()
                    {
                        return Some(Action::ScaleDown(group_id));
                    }
                }
                None
            }
            KeyCode::Char('/') => {
                if self.active_tab == Tab::Repos {
                    self.repo_searching = true;
                }
                None
            }
            KeyCode::Char('L') => {
                let is_authenticated = self.auth_status.as_ref().is_some_and(|a| a.authenticated);
                if !is_authenticated && self.login_state.is_none() {
                    Some(Action::StartLogin)
                } else {
                    None
                }
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
        }
    }

    fn make_test_runner_with_group(
        id: &str,
        state: &str,
        group_id: Option<&str>,
    ) -> crate::client::RunnerInfo {
        let mut r = make_test_runner(id, state);
        r.config.group_id = group_id.map(String::from);
        r
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
        app.handle_key(KeyCode::F(2), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Repos);
        app.handle_key(KeyCode::F(3), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Monitoring);
        app.handle_key(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Runners);
        app.handle_key(KeyCode::F(4), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Daemon);
    }

    #[test]
    fn test_f_key_tab_switch_from_daemon_tab() {
        let mut app = App::new();
        app.active_tab = Tab::Daemon;
        app.handle_key(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(app.active_tab, Tab::Runners);
    }

    #[test]
    fn test_daemon_tab_number_keys_set_log_level() {
        let mut app = App::new();
        app.active_tab = Tab::Daemon;
        let action = app.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
        assert_eq!(app.daemon_log_level, "TRACE");
        assert_eq!(action, Some(Action::RefreshDaemonLogs));

        let action = app.handle_key(KeyCode::Char('5'), KeyModifiers::NONE);
        assert_eq!(app.daemon_log_level, "ERROR");
        assert_eq!(action, Some(Action::RefreshDaemonLogs));
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
        app.rebuild_display_items();
        assert_eq!(app.selected_display_index, 0);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_display_index, 1);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_display_index, 2);
        app.handle_key(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(app.selected_display_index, 2); // stays at end
        app.handle_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(app.selected_display_index, 1);
    }

    #[test]
    fn test_handle_key_vim_navigation() {
        let mut app = App::new();
        app.runners = vec![
            make_test_runner("r1", "online"),
            make_test_runner("r2", "busy"),
        ];
        app.rebuild_display_items();
        app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(app.selected_display_index, 1);
        app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(app.selected_display_index, 0);
    }

    #[test]
    fn test_group_expand_collapse() {
        let mut app = App::new();
        app.runners = vec![
            make_test_runner_with_group("r1", "online", Some("g1")),
            make_test_runner_with_group("r2", "online", Some("g1")),
            make_test_runner_with_group("r3", "offline", None),
        ];
        app.rebuild_display_items();
        assert_eq!(app.display_items.len(), 2); // 1 group row + 1 solo

        app.toggle_group("g1");
        app.rebuild_display_items();
        assert_eq!(app.display_items.len(), 4); // 1 group + 2 expanded + 1 solo
    }

    #[test]
    fn test_solo_runners_no_group() {
        let mut app = App::new();
        app.runners = vec![
            make_test_runner("r1", "online"),
            make_test_runner("r2", "offline"),
        ];
        app.rebuild_display_items();
        assert_eq!(app.display_items.len(), 2);
        assert!(matches!(
            app.display_items[0],
            DisplayItem::RunnerRow {
                runner_index: 0,
                group_id: None
            }
        ));
    }

    #[test]
    fn test_key_hints_runners_tab() {
        let app = App::new(); // default tab is Runners
        let hints = app.key_hints();
        // Should have 4 rows (one per tab nav entry)
        assert_eq!(hints.len(), 4);
        // First row should have tab nav + action keys
        assert!(hints[0].iter().any(|(k, _)| k == &"F1"));
        assert!(hints[0].iter().any(|(k, _)| k == &"s"));
    }

    #[test]
    fn test_key_hints_daemon_tab() {
        let mut app = App::new();
        app.active_tab = Tab::Daemon;
        let hints = app.key_hints();
        assert_eq!(hints.len(), 4);
        // Should have daemon-specific keys
        assert!(hints[0].iter().any(|(k, _)| k == &"1..5"));
        assert!(hints[1].iter().any(|(k, _)| k == &"x"));
    }

    #[test]
    fn test_key_hints_monitoring_tab() {
        let mut app = App::new();
        app.active_tab = Tab::Monitoring;
        let hints = app.key_hints();
        assert_eq!(hints.len(), 4);
        // Monitoring has fewer keys — only tab nav + util column
        assert_eq!(hints[0].len(), 2); // F1 + L (no action/extra columns, unauthenticated)
    }

    #[test]
    fn test_login_key_starts_login_when_unauthenticated() {
        let mut app = App::new();
        let action = app.handle_key(KeyCode::Char('L'), KeyModifiers::NONE);
        assert_eq!(action, Some(Action::StartLogin));
    }

    #[test]
    fn test_login_key_noop_when_authenticated() {
        let mut app = App::new();
        app.auth_status = Some(AuthStatus {
            authenticated: true,
            user: Some(crate::client::GitHubUser {
                login: "octocat".to_string(),
                avatar_url: String::new(),
            }),
        });
        let action = app.handle_key(KeyCode::Char('L'), KeyModifiers::NONE);
        assert_eq!(action, None);
    }

    #[test]
    fn test_login_key_noop_when_already_logging_in() {
        let mut app = App::new();
        app.login_state = Some(LoginState::Polling {
            device_code: "test".to_string(),
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            interval: 5,
        });
        let action = app.handle_key(KeyCode::Char('L'), KeyModifiers::NONE);
        assert_eq!(action, None);
    }

    #[test]
    fn test_esc_cancels_login() {
        let mut app = App::new();
        app.login_state = Some(LoginState::Polling {
            device_code: "test".to_string(),
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            interval: 5,
        });
        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.login_state.is_none());
    }

    #[test]
    fn test_login_popup_swallows_keys() {
        let mut app = App::new();
        app.login_state = Some(LoginState::Polling {
            device_code: "test".to_string(),
            user_code: "ABCD-1234".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            interval: 5,
        });
        app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_key_hints_show_login_when_unauthenticated() {
        let app = App::new();
        let hints = app.key_hints();
        let has_login = hints.iter().any(|row| row.iter().any(|(k, _)| *k == "L"));
        assert!(has_login);
    }

    #[test]
    fn test_key_hints_hide_login_when_authenticated() {
        let mut app = App::new();
        app.auth_status = Some(AuthStatus {
            authenticated: true,
            user: Some(crate::client::GitHubUser {
                login: "octocat".to_string(),
                avatar_url: String::new(),
            }),
        });
        let hints = app.key_hints();
        let has_login = hints.iter().any(|row| row.iter().any(|(k, _)| *k == "L"));
        assert!(!has_login);
    }

    #[test]
    fn test_repo_search_mode() {
        let mut app = App::new();
        app.active_tab = Tab::Repos;
        app.handle_key(KeyCode::Char('/'), KeyModifiers::NONE);
        assert!(app.repo_searching);

        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(app.repo_search, "te");

        app.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(app.repo_search, "t");

        app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
        assert!(!app.repo_searching);
        assert!(app.repo_search.is_empty());
    }

    #[test]
    fn test_repo_search_enter_confirms() {
        let mut app = App::new();
        app.active_tab = Tab::Repos;
        app.handle_key(KeyCode::Char('/'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Char('t'), KeyModifiers::NONE);
        app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!app.repo_searching);
        assert_eq!(app.repo_search, "t"); // search term preserved after Enter
    }

    #[test]
    fn test_repo_search_slash_only_on_repos_tab() {
        let mut app = App::new();
        app.active_tab = Tab::Runners;
        app.handle_key(KeyCode::Char('/'), KeyModifiers::NONE);
        assert!(!app.repo_searching);
    }
}
