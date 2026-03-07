use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use epoch::app::{App, PrimaryView};
use epoch::config::Config;
use epoch::ui::events_notes;
use epoch::ui::home;
use epoch::ui::phase1_primary_views;
use epoch::ui::run_explorer;
use epoch::ui::system_processes;

#[test]
fn navigation_routes_include_phase1_views() {
    let views = phase1_primary_views();
    assert!(views.contains(&PrimaryView::Home));
    assert!(views.contains(&PrimaryView::LiveRun));
    assert!(views.contains(&PrimaryView::RunExplorer));
    assert!(views.contains(&PrimaryView::EventsNotes));
    assert!(views.contains(&PrimaryView::SystemProcesses));
}

#[test]
fn primary_view_count_matches_phase1_views() {
    let views = phase1_primary_views();
    assert_eq!(views.len(), 5);
    assert!(views.contains(&PrimaryView::Home));
    assert!(views.contains(&PrimaryView::LiveRun));
    assert!(views.contains(&PrimaryView::RunExplorer));
    assert!(views.contains(&PrimaryView::EventsNotes));
    assert!(views.contains(&PrimaryView::SystemProcesses));
}

#[test]
fn explicit_source_skips_home_and_starts_live_run() {
    let app = App::new(Config::default());
    assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
}

#[test]
fn home_view_renders_required_sections() {
    let sections = home::home_sections();
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
    let actions = epoch::home::service::default_actions();
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
    let columns = run_explorer::explorer_columns();
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
    let filtered =
        run_explorer::filter_runs_by_project_status_date(&rows, "proj-a", "active", "2026-03-06");
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
    let found = run_explorer::fuzzy_search_runs(&rows, "run");
    assert_eq!(found.len(), 2);
    assert!(found.contains(&"run-alpha".to_string()));
    assert!(found.contains(&"run-beta".to_string()));
}

#[test]
fn events_notes_view_supports_add_filter_pin_jump() {
    assert!(events_notes::supports_required_actions());
}

#[test]
fn system_processes_view_renders_pid_command_cwd_usage() {
    let columns = system_processes::required_columns();
    assert_eq!(columns, ["PID", "Command", "CWD", "CPU", "Memory"]);
}

#[test]
fn key_driven_view_switching_routes_primary_views() {
    let mut app = App::new(Config::default());
    assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.ui_state.primary_view, PrimaryView::RunExplorer);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.ui_state.primary_view, PrimaryView::EventsNotes);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.ui_state.primary_view, PrimaryView::SystemProcesses);

    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.ui_state.primary_view, PrimaryView::Home);
}
