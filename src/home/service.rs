use std::fs;
use std::path::Path;

use color_eyre::Result;
use serde::{Deserialize, Serialize};

use crate::collectors::process::{ProbeStatus, ProcessCandidate};
use crate::store::repository::{RunStore, source_fingerprint};
use crate::store::types::{RunMetadata, RunSourceKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HomeAction {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct HomeSnapshot {
    pub generated_at_epoch_secs: i64,
    pub active_runs: Vec<String>,
    pub recent_runs: Vec<String>,
    pub recent_projects: Vec<String>,
    pub discovered_processes: Vec<String>,
    pub actions: Vec<HomeAction>,
}

pub fn default_actions() -> Vec<HomeAction> {
    vec![
        HomeAction {
            id: "attach_active_run".to_string(),
            label: "Attach to active run".to_string(),
            enabled: true,
            disabled_reason: None,
        },
        HomeAction {
            id: "open_recent_project".to_string(),
            label: "Open recent project".to_string(),
            enabled: true,
            disabled_reason: None,
        },
        HomeAction {
            id: "scan_current_directory".to_string(),
            label: "Scan current directory".to_string(),
            enabled: true,
            disabled_reason: None,
        },
        HomeAction {
            id: "search_all_runs".to_string(),
            label: "Search all runs".to_string(),
            enabled: true,
            disabled_reason: None,
        },
        HomeAction {
            id: "browse_checkpoints".to_string(),
            label: "Browse checkpoints".to_string(),
            enabled: true,
            disabled_reason: None,
        },
        HomeAction {
            id: "open_compare".to_string(),
            label: "Open compare".to_string(),
            enabled: false,
            disabled_reason: Some("Phase 2".to_string()),
        },
        HomeAction {
            id: "open_artifacts".to_string(),
            label: "Open artifacts".to_string(),
            enabled: false,
            disabled_reason: Some("Phase 2".to_string()),
        },
        HomeAction {
            id: "open_model".to_string(),
            label: "Open model".to_string(),
            enabled: false,
            disabled_reason: Some("Phase 2".to_string()),
        },
        HomeAction {
            id: "open_finder".to_string(),
            label: "Open finder".to_string(),
            enabled: false,
            disabled_reason: Some("Phase 2".to_string()),
        },
    ]
}

/// Home view sections - data that defines the home view structure.
/// This belongs in core/home rather than UI to allow tests to verify
/// the expected sections without coupling to UI implementation.
pub fn home_sections() -> Vec<&'static str> {
    vec![
        "Active Runs",
        "Recent Runs",
        "Recent Projects",
        "Alerts Needing Attention",
        "Available Checkpoints",
        "Discovered Processes",
    ]
}

pub fn load_cached_snapshot(path: &Path) -> Option<HomeSnapshot> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<HomeSnapshot>(&text).ok()
}

pub fn save_cached_snapshot(path: &Path, snapshot: &HomeSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(snapshot)?;
    fs::write(path, serialized)?;
    Ok(())
}

pub fn snapshot_cache_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", "epoch")
        .map(|dirs| dirs.cache_dir().join("home_snapshot.json"))
}

pub fn load_or_build_cached_snapshot<F>(path: &Path, build: F) -> HomeSnapshot
where
    F: FnOnce() -> HomeSnapshot,
{
    if let Some(cached) = load_cached_snapshot(path) {
        return cached;
    }

    let snapshot = build();
    let _ = save_cached_snapshot(path, &snapshot);
    snapshot
}

pub fn empty_snapshot(now_epoch_secs: i64) -> HomeSnapshot {
    HomeSnapshot {
        generated_at_epoch_secs: now_epoch_secs,
        actions: default_actions(),
        ..HomeSnapshot::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachOutcome {
    Attached { run_id: String, reused: bool },
    PermissionDenied,
}

pub fn attach_to_discovered_process(
    store: &RunStore,
    candidate: &ProcessCandidate,
    project_root: Option<&str>,
) -> Result<AttachOutcome> {
    if candidate.status == ProbeStatus::PermissionDenied {
        return Ok(AttachOutcome::PermissionDenied);
    }

    let source_locator = format!("pid:{}", candidate.pid);
    let fingerprint =
        source_fingerprint(RunSourceKind::Process, Some(&source_locator), project_root);
    let attach = store.attach_or_create_active_run(
        &fingerprint,
        RunSourceKind::Process,
        RunMetadata {
            display_name: Some(candidate.command.clone()),
            project_root: project_root.map(ToOwned::to_owned),
            command: Some(candidate.command.clone()),
            cwd: candidate.cwd.clone(),
            git_commit: None,
            git_dirty: None,
            source_locator: Some(source_locator),
        },
    )?;

    Ok(AttachOutcome::Attached {
        run_id: attach.run_id,
        reused: attach.reused_existing_active,
    })
}
