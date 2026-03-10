use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use epoch::app::{App, HomeFocusTarget, MonitoringRoute, PanelFocus, PrimaryView};
use epoch::collectors::process::{ProbeStatus, ProcessCandidate};
use epoch::config::Config;
use epoch::home::service::default_actions;
use epoch::store::types::{
    RunRecord, RunSourceKind, RunStatus, filter_runs_by_project_status_date, fuzzy_search_runs,
    run_explorer_columns, system_processes_columns,
};
use epoch::types::TrainingMetrics;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn monitoring_routes_include_home_and_run_detail() {
    use epoch::ui::phase1_primary_views;
    let routes = phase1_primary_views();
    assert_eq!(routes, [MonitoringRoute::Home, MonitoringRoute::RunDetail]);
}

#[test]
fn app_state_routes_between_home_and_run_detail() {
    let mut app = App::new(Config::default());
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::RunDetail);
    assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::Home);
    assert_eq!(app.ui_state.primary_view, PrimaryView::Home);

    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::RunDetail);
    assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
}

#[test]
fn home_tab_cycles_panels_not_routes() {
    let mut app = App::new(Config::default());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::Home);
    assert_eq!(
        app.ui_state.monitoring.home_focus,
        HomeFocusTarget::Overview
    );

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.home_focus, HomeFocusTarget::Runs);
    assert_eq!(
        app.ui_state.monitoring.focused_panel,
        Some(PanelFocus::Runs)
    );
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::Home);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.ui_state.monitoring.home_focus,
        HomeFocusTarget::Processes
    );

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.home_focus, HomeFocusTarget::Files);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.home_focus, HomeFocusTarget::Alerts);
    assert_eq!(app.ui_state.monitoring.focused_panel, None);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.ui_state.monitoring.home_focus,
        HomeFocusTarget::Overview
    );

    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
    assert_eq!(app.ui_state.monitoring.home_focus, HomeFocusTarget::Alerts);
}

#[test]
fn home_view_renders_required_sections() {
    use epoch::home::service::home_sections;
    let sections = home_sections();
    for required in [
        "Active Runs",
        "Recent Runs",
        "Recent Projects",
        "Alerts Needing Attention",
        "Available Checkpoints",
        "Discovered Processes",
    ] {
        assert!(sections.contains(&required), "missing section: {required}");
    }
}

#[test]
fn home_empty_state_offers_required_actions() {
    let actions = default_actions();
    let ids = actions.iter().map(|a| a.id.as_str()).collect::<Vec<_>>();
    for required in [
        "attach_active_run",
        "open_recent_project",
        "scan_current_directory",
        "search_all_runs",
        "browse_checkpoints",
    ] {
        assert!(ids.contains(&required), "missing action: {required}");
    }
}

#[test]
fn run_explorer_renders_required_columns() {
    let columns = run_explorer_columns();
    for required in [
        "Name",
        "Project",
        "Status",
        "Duration",
        "Best Metric",
        "Current/Final Step",
        "Start Date",
        "Git State",
        "Device Info",
    ] {
        assert!(columns.contains(&required), "missing column: {required}");
    }
}

#[test]
fn run_explorer_filters_by_project_status_date() {
    let rows = vec![
        (
            "proj-a".to_string(),
            "active".to_string(),
            "2026-03-06".to_string(),
        ),
        (
            "proj-a".to_string(),
            "completed".to_string(),
            "2026-03-06".to_string(),
        ),
        (
            "proj-b".to_string(),
            "active".to_string(),
            "2026-03-06".to_string(),
        ),
    ];
    let filtered = filter_runs_by_project_status_date(&rows, "proj-a", "active", "2026-03-06");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].0, "proj-a");
    assert_eq!(filtered[0].1, "active");
}

#[test]
fn run_explorer_fuzzy_search_returns_expected_matches() {
    let rows = vec![
        "run-alpha".to_string(),
        "run-beta".to_string(),
        "evaluation".to_string(),
    ];
    let found = fuzzy_search_runs(&rows, "run");
    assert_eq!(found.len(), 2);
    assert!(found.contains(&"run-alpha".to_string()));
    assert!(found.contains(&"run-beta".to_string()));
}

#[test]
fn system_processes_view_renders_pid_command_cwd_usage() {
    let columns = system_processes_columns();
    assert_eq!(columns, ["PID", "Command", "CWD", "CPU", "Memory"]);
}

#[test]
fn home_runs_support_search_and_filter_interactions() {
    let mut app = App::new(Config::default());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.ui_state.monitoring.focused_panel = Some(PanelFocus::Runs);
    app.ui_state.monitoring.home_focus = HomeFocusTarget::Runs;
    app.ui_state.explorer.records = vec![sample_run("run-alpha", RunStatus::Active)];

    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
    assert!(app.ui_state.explorer.search_active);

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
    assert_eq!(app.ui_state.explorer.search_query, "al");

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(!app.ui_state.explorer.search_active);

    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert_eq!(app.ui_state.explorer.status_filter, Some(RunStatus::Active));
}

#[test]
fn home_workspace_attach_process_opens_run_detail() {
    let mut app = App::new(Config::default());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.ui_state.monitoring.focused_panel = Some(PanelFocus::Processes);
    app.ui_state.monitoring.home_focus = HomeFocusTarget::Processes;
    app.set_discovered_processes(vec![ProcessCandidate {
        pid: 4242,
        command: "python train.py".to_string(),
        cwd: Some("/tmp/proj".to_string()),
        cpu_milli_percent: 100,
        memory_bytes: 1024,
        status: ProbeStatus::Ok,
        pid_reused: false,
    }]);

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::RunDetail);
    assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
    assert_eq!(app.ui_state.monitoring.selected_pid, Some(4242));
}

#[test]
fn home_runs_enter_drills_to_run_detail_and_esc_goes_back_home() {
    let mut app = App::new(Config::default());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.ui_state.monitoring.focused_panel = Some(PanelFocus::Runs);
    app.ui_state.monitoring.home_focus = HomeFocusTarget::Runs;
    app.ui_state.explorer.records = vec![sample_run("run-42", RunStatus::Active)];
    app.ui_state.explorer.selected_idx = 0;

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::RunDetail);
    assert_eq!(
        app.ui_state
            .monitoring
            .run_detail
            .selected_run_id
            .as_deref(),
        Some("run-42")
    );

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::Home);
}

#[test]
fn render_buffer_home_workspace_shows_header_and_shell_hints() {
    let backend = TestBackend::new(140, 45);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    let mut app = App::new(Config::default());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    terminal
        .draw(|frame| epoch::ui::render(frame, &app))
        .expect("home render should succeed");
    let content = buffer_to_string(terminal.backend().buffer());

    assert!(content.contains("Home"));
    assert!(content.contains("No Active Run"));
    assert!(content.contains("Runs"));
    assert!(content.contains("Alerts"));
    assert!(content.contains("Tab:focus panel"));
    assert!(content.contains("o:open  a:attach  e:detail"));
    assert!(content.contains("?:help"));
}

#[test]
fn render_buffer_run_detail_shows_breadcrumb_live_content_and_hints() {
    let backend = TestBackend::new(140, 45);
    let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
    let mut app = App::new(Config::default());
    app.ui_state.monitoring.run_detail.selected_run_id = Some("run-42".to_string());
    app.push_metrics(TrainingMetrics {
        loss: Some(0.42),
        learning_rate: Some(1e-4),
        step: Some(120),
        ..TrainingMetrics::default()
    });

    terminal
        .draw(|frame| epoch::ui::render(frame, &app))
        .expect("run detail render should succeed");
    let content = buffer_to_string(terminal.backend().buffer());

    assert!(content.contains("Home > Run Detail"));
    assert!(content.contains("Esc:back"));
    assert!(content.contains("Run Detail: run-42"));
    assert!(content.contains("Loss:"));
    assert!(content.contains("1-4:box"));
}

fn sample_run(run_id: &str, status: RunStatus) -> RunRecord {
    RunRecord {
        run_id: run_id.to_string(),
        source_fingerprint: format!("fp-{run_id}"),
        source_kind: RunSourceKind::LogFile,
        source_locator: Some(format!("/tmp/{run_id}.log")),
        project_root: Some("/tmp/project".to_string()),
        display_name: Some(run_id.to_string()),
        status,
        command: None,
        cwd: Some("/tmp/project".to_string()),
        git_commit: None,
        git_dirty: None,
        started_at_epoch_secs: 1,
        ended_at_epoch_secs: None,
        last_step: Some(10),
        last_updated_epoch_secs: 1,
    }
}

fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .expect("cell should exist")
                        .symbol()
                        .to_string()
                })
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}
