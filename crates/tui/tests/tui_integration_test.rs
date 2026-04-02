use crossterm::event::{KeyCode, KeyModifiers};
use homerun::app::{Action, App, DisplayItem, LoginState, Tab};
use homerun::client::{
    AuthStatus, DeviceFlowResponse, GitHubUser, RepoInfo, RunnerConfig, RunnerInfo,
};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_runner(id: &str, state: &str) -> RunnerInfo {
    RunnerInfo {
        config: RunnerConfig {
            id: id.to_string(),
            name: format!("runner-{id}"),
            repo_owner: "test-org".to_string(),
            repo_name: "test-repo".to_string(),
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

fn make_runner_with_group(id: &str, state: &str, group: &str) -> RunnerInfo {
    let mut r = make_runner(id, state);
    r.config.group_id = Some(group.to_string());
    r
}

fn make_repo(name: &str) -> RepoInfo {
    RepoInfo {
        id: 1,
        full_name: format!("org/{name}"),
        name: name.to_string(),
        owner: "org".to_string(),
        private: false,
        html_url: format!("https://github.com/org/{name}"),
        is_org: true,
    }
}

fn authenticated_app() -> App {
    let mut app = App::new();
    app.auth_status = Some(AuthStatus {
        authenticated: true,
        user: Some(GitHubUser {
            login: "octocat".to_string(),
            avatar_url: String::new(),
        }),
    });
    app
}

// ---------------------------------------------------------------------------
// Tab switching (2 tests)
// ---------------------------------------------------------------------------

#[test]
fn f1_f4_switches_to_correct_tabs() {
    let mut app = App::new();
    assert_eq!(app.active_tab, Tab::Runners);

    app.handle_key(KeyCode::F(2), KeyModifiers::NONE);
    assert_eq!(app.active_tab, Tab::Repos);

    app.handle_key(KeyCode::F(3), KeyModifiers::NONE);
    assert_eq!(app.active_tab, Tab::Monitoring);

    app.handle_key(KeyCode::F(4), KeyModifiers::NONE);
    assert_eq!(app.active_tab, Tab::Daemon);

    app.handle_key(KeyCode::F(1), KeyModifiers::NONE);
    assert_eq!(app.active_tab, Tab::Runners);
}

#[test]
fn f_keys_work_from_daemon_tab_and_number_keys_set_log_level() {
    let mut app = App::new();
    app.active_tab = Tab::Daemon;

    // Number keys set log level on daemon tab
    let action = app.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
    assert_eq!(app.daemon_log_level, "TRACE");
    assert_eq!(action, Some(Action::RefreshDaemonLogs));

    let action = app.handle_key(KeyCode::Char('3'), KeyModifiers::NONE);
    assert_eq!(app.daemon_log_level, "INFO");
    assert_eq!(action, Some(Action::RefreshDaemonLogs));

    // F-keys still switch tabs from daemon
    app.handle_key(KeyCode::F(1), KeyModifiers::NONE);
    assert_eq!(app.active_tab, Tab::Runners);
}

// ---------------------------------------------------------------------------
// Runner navigation (2 tests)
// ---------------------------------------------------------------------------

#[test]
fn jk_navigates_runner_list() {
    let mut app = App::new();
    app.runners = vec![
        make_runner("r1", "online"),
        make_runner("r2", "busy"),
        make_runner("r3", "offline"),
    ];
    app.rebuild_display_items();
    assert_eq!(app.selected_display_index, 0);

    app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(app.selected_display_index, 1);

    app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(app.selected_display_index, 2);

    // Clamps at end
    app.handle_key(KeyCode::Char('j'), KeyModifiers::NONE);
    assert_eq!(app.selected_display_index, 2);

    app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(app.selected_display_index, 1);

    app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(app.selected_display_index, 0);

    // Clamps at start
    app.handle_key(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(app.selected_display_index, 0);
}

#[test]
fn enter_expands_groups_left_collapses() {
    let mut app = App::new();
    app.runners = vec![
        make_runner_with_group("r1", "online", "g1"),
        make_runner_with_group("r2", "busy", "g1"),
        make_runner("r3", "offline"),
    ];
    app.rebuild_display_items();

    // Initially: group row (collapsed) + solo runner = 2 items
    assert_eq!(app.display_items.len(), 2);
    assert!(matches!(app.display_items[0], DisplayItem::GroupRow { .. }));

    // Enter on group row expands it
    app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(app.display_items.len(), 4); // group + 2 runners + solo
    assert!(app.expanded_groups.contains("g1"));

    // Left on group row collapses it (navigate back to group row first)
    app.selected_display_index = 0;
    app.handle_key(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(app.display_items.len(), 2);
    assert!(!app.expanded_groups.contains("g1"));
}

// ---------------------------------------------------------------------------
// Runner actions (3 tests)
// ---------------------------------------------------------------------------

#[test]
fn s_toggles_start_stop() {
    let mut app = App::new();
    app.runners = vec![make_runner("r1", "online"), make_runner("r2", "offline")];
    app.rebuild_display_items();

    // On online runner -> StopRunner
    app.selected_display_index = 0;
    let action = app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::StopRunner("r1".to_string())));

    // On offline runner -> StartRunner
    app.selected_display_index = 1;
    let action = app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::StartRunner("r2".to_string())));
}

#[test]
fn r_restarts_runner() {
    let mut app = App::new();
    app.runners = vec![make_runner("r1", "online")];
    app.rebuild_display_items();

    let action = app.handle_key(KeyCode::Char('r'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::RestartRunner("r1".to_string())));
}

#[test]
fn d_deletes_runner() {
    let mut app = App::new();
    app.runners = vec![make_runner("r1", "online")];
    app.rebuild_display_items();

    let action = app.handle_key(KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::DeleteRunner("r1".to_string())));
}

// ---------------------------------------------------------------------------
// Daemon tab (3 tests)
// ---------------------------------------------------------------------------

#[test]
fn daemon_tab_number_keys_set_log_levels() {
    let mut app = App::new();
    app.active_tab = Tab::Daemon;

    let action = app.handle_key(KeyCode::Char('1'), KeyModifiers::NONE);
    assert_eq!(app.daemon_log_level, "TRACE");
    assert_eq!(action, Some(Action::RefreshDaemonLogs));

    let action = app.handle_key(KeyCode::Char('3'), KeyModifiers::NONE);
    assert_eq!(app.daemon_log_level, "INFO");
    assert_eq!(action, Some(Action::RefreshDaemonLogs));

    let action = app.handle_key(KeyCode::Char('5'), KeyModifiers::NONE);
    assert_eq!(app.daemon_log_level, "ERROR");
    assert_eq!(action, Some(Action::RefreshDaemonLogs));
}

#[test]
fn daemon_search_mode_type_and_esc_cancels() {
    let mut app = App::new();
    app.active_tab = Tab::Daemon;

    // / enters search mode
    let action = app.handle_key(KeyCode::Char('/'), KeyModifiers::NONE);
    assert!(app.daemon_searching);
    assert_eq!(action, None);

    // Typing appends to search
    app.handle_key(KeyCode::Char('e'), KeyModifiers::NONE);
    app.handle_key(KeyCode::Char('r'), KeyModifiers::NONE);
    assert_eq!(app.daemon_search, "er");

    // Esc cancels and clears search
    let action = app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.daemon_searching);
    assert!(app.daemon_search.is_empty());
    assert_eq!(action, Some(Action::RefreshDaemonLogs));
}

#[test]
fn daemon_f_toggles_follow() {
    let mut app = App::new();
    app.active_tab = Tab::Daemon;
    assert!(app.daemon_follow); // default is true

    app.handle_key(KeyCode::Char('f'), KeyModifiers::NONE);
    assert!(!app.daemon_follow);

    app.handle_key(KeyCode::Char('f'), KeyModifiers::NONE);
    assert!(app.daemon_follow);
}

// ---------------------------------------------------------------------------
// Repo search (1 test)
// ---------------------------------------------------------------------------

#[test]
fn repo_search_enter_confirms_preserving_term() {
    let mut app = App::new();
    app.active_tab = Tab::Repos;
    app.repos = vec![make_repo("alpha"), make_repo("beta")];

    // / enters search
    app.handle_key(KeyCode::Char('/'), KeyModifiers::NONE);
    assert!(app.repo_searching);

    // Type to filter
    app.handle_key(KeyCode::Char('b'), KeyModifiers::NONE);
    app.handle_key(KeyCode::Char('e'), KeyModifiers::NONE);
    assert_eq!(app.repo_search, "be");

    // Enter confirms and preserves term
    app.handle_key(KeyCode::Enter, KeyModifiers::NONE);
    assert!(!app.repo_searching);
    assert_eq!(app.repo_search, "be");
}

// ---------------------------------------------------------------------------
// Help popup (1 test)
// ---------------------------------------------------------------------------

#[test]
fn help_popup_blocks_q_esc_closes() {
    let mut app = App::new();

    // ? opens help
    app.handle_key(KeyCode::Char('?'), KeyModifiers::NONE);
    assert!(app.show_help);

    // q does NOT quit while help is open
    app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
    assert!(!app.should_quit);
    assert!(app.show_help); // still open

    // Esc closes help
    app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.show_help);
}

// ---------------------------------------------------------------------------
// Login popup (3 tests)
// ---------------------------------------------------------------------------

#[test]
fn l_triggers_start_login_when_unauthenticated() {
    let mut app = App::new();
    // auth_status is None (unauthenticated)
    let action = app.handle_key(KeyCode::Char('L'), KeyModifiers::NONE);
    assert_eq!(action, Some(Action::StartLogin));
}

#[test]
fn l_is_noop_when_authenticated() {
    let mut app = authenticated_app();
    let action = app.handle_key(KeyCode::Char('L'), KeyModifiers::NONE);
    assert_eq!(action, None);
}

#[test]
fn login_popup_blocks_all_keys_esc_cancels() {
    let mut app = App::new();
    app.login_state = Some(LoginState::Polling {
        device_code: "DC_001".to_string(),
        user_code: "ABCD-1234".to_string(),
        verification_uri: "https://github.com/login/device".to_string(),
        interval: 5,
    });

    // q should be swallowed (not quit)
    let action = app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(action, None);
    assert!(!app.should_quit);

    // s should be swallowed
    let action = app.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(action, None);

    // Esc cancels login
    app.handle_key(KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.login_state.is_none());
}

// ---------------------------------------------------------------------------
// Login flow with mock daemon (1 test)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn login_flow_with_mock_daemon() {
    use homerun::client::DaemonClient;
    use test_utils::MockDaemon;

    let mock = MockDaemon::builder()
        .with_device_flow(DeviceFlowResponse {
            device_code: "DC_TEST".to_string(),
            user_code: "TEST-CODE".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            expires_in: 900,
            interval: 5,
        })
        .build()
        .await;

    let client = DaemonClient::new(mock.socket_path().clone());

    // Step 1: Start device flow
    let flow = client.start_device_flow().await.unwrap();
    assert_eq!(flow.user_code, "TEST-CODE");
    assert_eq!(flow.device_code, "DC_TEST");

    // Step 2: Poll — not yet authorized, returns None
    let result = client
        .poll_device_flow(&flow.device_code, flow.interval)
        .await
        .unwrap();
    assert!(result.is_none());

    // Step 3: Check auth status — still unauthenticated
    let status = client.auth_status().await.unwrap();
    assert!(!status.authenticated);
}

// ---------------------------------------------------------------------------
// Key hints (2 tests)
// ---------------------------------------------------------------------------

#[test]
fn key_hints_runners_tab_has_srd_daemon_tab_has_levels_and_follow() {
    // Runners tab
    let app = App::new();
    let hints = app.key_hints();
    let all_keys: Vec<&str> = hints
        .iter()
        .flat_map(|row| row.iter().map(|(k, _)| *k))
        .collect();
    assert!(all_keys.contains(&"s"), "runners tab should show s");
    assert!(all_keys.contains(&"r"), "runners tab should show r");
    assert!(all_keys.contains(&"d"), "runners tab should show d");

    // Daemon tab
    let mut app = App::new();
    app.active_tab = Tab::Daemon;
    let hints = app.key_hints();
    let all_keys: Vec<&str> = hints
        .iter()
        .flat_map(|row| row.iter().map(|(k, _)| *k))
        .collect();
    assert!(all_keys.contains(&"1..5"), "daemon tab should show 1..5");
    assert!(all_keys.contains(&"f"), "daemon tab should show f");
}

#[test]
fn key_hints_l_shown_unauthenticated_hidden_authenticated() {
    // Unauthenticated: L is shown
    let app = App::new();
    let hints = app.key_hints();
    let has_l = hints.iter().any(|row| row.iter().any(|(k, _)| *k == "L"));
    assert!(has_l, "L should be shown when unauthenticated");

    // Authenticated: L is hidden
    let app = authenticated_app();
    let hints = app.key_hints();
    let has_l = hints.iter().any(|row| row.iter().any(|(k, _)| *k == "L"));
    assert!(!has_l, "L should be hidden when authenticated");
}

// ---------------------------------------------------------------------------
// Quit (1 test)
// ---------------------------------------------------------------------------

#[test]
fn q_sets_should_quit() {
    let mut app = App::new();
    assert!(!app.should_quit);
    app.handle_key(KeyCode::Char('q'), KeyModifiers::NONE);
    assert!(app.should_quit);
}
