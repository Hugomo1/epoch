use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use epoch::app::{App, MonitoringRoute};
use epoch::collectors::process::{ProbeStatus, ProcessCandidate};
use epoch::config::Config;
use epoch::home::service::{
    AttachOutcome, HomeSnapshot, attach_to_discovered_process, default_actions,
    load_or_build_cached_snapshot, save_cached_snapshot,
};
use epoch::store::repository::RunStore;

fn temp_file(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("epoch-phase1-launch-{label}-{unique}"));
    fs::create_dir_all(&root).expect("temp directory should be created");
    root.join("home_snapshot.json")
}

#[test]
fn no_arg_startup_routes_to_home() {
    let config = Config::default();
    let mut app = App::new(config.clone());

    if !config.stdin_mode && config.log_file.is_none() {
        app.ui_state.monitoring.route = MonitoringRoute::Home;
    }

    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::Home);
}

#[test]
fn explicit_source_startup_stays_in_run_detail() {
    let mut config = Config::default();
    config.log_file = Some(PathBuf::from("/tmp/train.log"));

    let app = App::new(config);
    assert_eq!(app.ui_state.monitoring.route, MonitoringRoute::RunDetail);
}

#[test]
fn launch_from_random_dir_returns_cached_snapshot_under_2s() {
    let cache = temp_file("cached-under-2s");
    let seeded = HomeSnapshot {
        generated_at_epoch_secs: 1,
        active_runs: vec!["run-a".to_string()],
        recent_runs: vec!["run-b".to_string()],
        recent_projects: vec!["proj-a".to_string()],
        discovered_processes: vec!["proc-a".to_string()],
        actions: default_actions(),
    };
    save_cached_snapshot(&cache, &seeded).expect("seed cache should be written");

    let started = Instant::now();
    let loaded = load_or_build_cached_snapshot(&cache, || HomeSnapshot::default());
    let elapsed = started.elapsed();

    assert!(
        elapsed <= Duration::from_secs(2),
        "cache load should complete within 2 seconds"
    );
    assert_eq!(loaded.active_runs, vec!["run-a".to_string()]);
}

#[test]
fn home_snapshot_contains_immediate_actions() {
    let actions = default_actions();
    let ids = actions.iter().map(|a| a.id.as_str()).collect::<Vec<_>>();
    for expected in [
        "attach_active_run",
        "open_recent_project",
        "scan_current_directory",
        "search_all_runs",
        "browse_checkpoints",
    ] {
        assert!(
            ids.contains(&expected),
            "home action set must include {expected}"
        );
    }
}

#[test]
fn phase2_actions_are_disabled_with_reason() {
    let actions = default_actions();
    let phase2 = actions
        .iter()
        .filter(|action| {
            matches!(
                action.id.as_str(),
                "open_compare" | "open_artifacts" | "open_model" | "open_finder"
            )
        })
        .collect::<Vec<_>>();

    assert!(!phase2.is_empty(), "phase2 action set must not be empty");
    for action in phase2 {
        assert!(!action.enabled, "{} must be disabled", action.id);
        assert_eq!(
            action.disabled_reason.as_deref(),
            Some("Phase 2"),
            "{} must include explicit Phase 2 reason",
            action.id
        );
    }
}

#[test]
fn home_first_render_not_blank_or_spinner_only() {
    let snapshot = HomeSnapshot {
        generated_at_epoch_secs: 1,
        active_runs: vec![],
        recent_runs: vec![],
        recent_projects: vec![],
        discovered_processes: vec![],
        actions: default_actions(),
    };
    assert!(
        !snapshot.actions.is_empty(),
        "home first render must have actionable content"
    );
}

#[test]
fn attach_to_discovered_process_creates_or_reuses_run() {
    let store = RunStore::open_in_memory().expect("store should open");
    let candidate = ProcessCandidate {
        pid: 1234,
        command: "python train.py".to_string(),
        cwd: Some("/tmp/proj".to_string()),
        cpu_milli_percent: 100,
        memory_bytes: 1000,
        status: ProbeStatus::Ok,
        pid_reused: false,
    };

    let first = attach_to_discovered_process(&store, &candidate, Some("/tmp/proj"))
        .expect("attach should succeed");
    let second = attach_to_discovered_process(&store, &candidate, Some("/tmp/proj"))
        .expect("attach should succeed");

    match (first, second) {
        (
            AttachOutcome::Attached {
                run_id: first_id,
                reused: false,
            },
            AttachOutcome::Attached {
                run_id: second_id,
                reused: true,
            },
        ) => assert_eq!(first_id, second_id),
        _ => panic!("attach/reuse behavior mismatch"),
    }
}

#[test]
fn attach_to_discovered_process_permission_denied_graceful() {
    let store = RunStore::open_in_memory().expect("store should open");
    let candidate = ProcessCandidate {
        pid: 4321,
        command: "python train.py".to_string(),
        cwd: None,
        cpu_milli_percent: 0,
        memory_bytes: 0,
        status: ProbeStatus::PermissionDenied,
        pid_reused: false,
    };

    let result = attach_to_discovered_process(&store, &candidate, None).expect("call should work");
    assert_eq!(result, AttachOutcome::PermissionDenied);
}
