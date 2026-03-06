use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use epoch::store::repository::RunStore;
use epoch::store::repository::source_fingerprint;
use epoch::store::schema::SCHEMA_VERSION;
use epoch::store::types::{RunMetadata, RunSourceKind, RunStatus, now_epoch_secs};
use rusqlite::Connection;

fn temp_db_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("epoch-{label}-{unique}"));
    fs::create_dir_all(&root).expect("temp directory should be created");
    root.join("runs.db")
}

#[test]
fn run_store_schema_migrates_from_empty() {
    let db = temp_db_path("store-schema");
    let store = RunStore::open(&db).expect("store should open and migrate");
    let version = store
        .schema_version()
        .expect("schema version should be available");
    assert_eq!(version, SCHEMA_VERSION);
}

#[test]
fn run_identity_dedupes_active_source_fingerprint() {
    let db = temp_db_path("store-dedupe");
    let store = RunStore::open(&db).expect("store should open");

    let first = store
        .attach_or_create_active_run(
            "fp-log-a",
            RunSourceKind::LogFile,
            RunMetadata {
                source_locator: Some("/tmp/train.log".to_string()),
                ..RunMetadata::default()
            },
        )
        .expect("first attach should succeed");
    assert!(!first.reused_existing_active);

    let second = store
        .attach_or_create_active_run(
            "fp-log-a",
            RunSourceKind::LogFile,
            RunMetadata {
                source_locator: Some("/tmp/train.log".to_string()),
                ..RunMetadata::default()
            },
        )
        .expect("second attach should succeed");
    assert!(second.reused_existing_active);
    assert_eq!(first.run_id, second.run_id);

    store
        .complete_run(&first.run_id, RunStatus::Completed)
        .expect("completion should work");

    let third = store
        .attach_or_create_active_run(
            "fp-log-a",
            RunSourceKind::LogFile,
            RunMetadata {
                source_locator: Some("/tmp/train.log".to_string()),
                ..RunMetadata::default()
            },
        )
        .expect("third attach should succeed");
    assert!(!third.reused_existing_active);
    assert_ne!(first.run_id, third.run_id);
}

#[test]
fn run_store_roundtrip_persists_after_restart() {
    let db = temp_db_path("store-roundtrip");
    let first_run_id = {
        let store = RunStore::open(&db).expect("store should open");
        let attach = store
            .attach_or_create_active_run(
                "fp-log-roundtrip",
                RunSourceKind::LogFile,
                RunMetadata {
                    source_locator: Some("/tmp/train.log".to_string()),
                    display_name: Some("run roundtrip".to_string()),
                    project_root: Some("/tmp/project".to_string()),
                    ..RunMetadata::default()
                },
            )
            .expect("attach should succeed");
        store
            .update_last_step(&attach.run_id, 42)
            .expect("step update should succeed");
        attach.run_id
    };

    let store = RunStore::open(&db).expect("store should reopen");
    let run = store
        .get_run(&first_run_id)
        .expect("query should succeed")
        .expect("run should exist after reopen");
    assert_eq!(run.last_step, Some(42));
    assert_eq!(run.display_name.as_deref(), Some("run roundtrip"));
    assert_eq!(run.project_root.as_deref(), Some("/tmp/project"));
}

#[test]
fn run_store_uses_wall_clock_timestamps() {
    let db = temp_db_path("store-clock");
    let now = now_epoch_secs();
    let store = RunStore::open(&db).expect("store should open");

    let attach = store
        .attach_or_create_active_run("fp-clock", RunSourceKind::Stdin, RunMetadata::default())
        .expect("attach should succeed");
    let run = store
        .get_run(&attach.run_id)
        .expect("query should succeed")
        .expect("run should exist");

    assert!(
        run.started_at_epoch_secs > 0,
        "started timestamp must be wall-clock epoch seconds"
    );
    assert!(
        (run.started_at_epoch_secs - now).abs() <= 10,
        "started timestamp should be near current wall-clock time"
    );
}

#[test]
fn run_store_handles_schema_version_mismatch() {
    let db = temp_db_path("store-version-mismatch");
    {
        let conn = Connection::open(&db).expect("sqlite connection should open");
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR REPLACE INTO schema_meta(key, value) VALUES('schema_version', '999');
            ",
        )
        .expect("schema meta should initialize");
    }

    let err = match RunStore::open(&db) {
        Ok(_) => panic!("future schema must be rejected"),
        Err(err) => err,
    };
    let text = format!("{err}");
    assert!(
        text.contains("unsupported schema version"),
        "mismatch error should explain unsupported schema"
    );
}

#[test]
fn startup_with_log_file_creates_active_run_record() {
    let db = temp_db_path("store-startup-log");
    let store = RunStore::open(&db).expect("store should open");

    let locator = "/tmp/startup-train.log";
    let fp = source_fingerprint(RunSourceKind::LogFile, Some(locator), Some("/tmp/project"));
    let created = store
        .attach_or_create_active_run(
            &fp,
            RunSourceKind::LogFile,
            RunMetadata {
                source_locator: Some(locator.to_string()),
                project_root: Some("/tmp/project".to_string()),
                display_name: Some("startup-train.log".to_string()),
                ..RunMetadata::default()
            },
        )
        .expect("attach should succeed");

    let run = store
        .get_run(&created.run_id)
        .expect("query should succeed")
        .expect("run should exist");

    assert_eq!(run.source_kind, RunSourceKind::LogFile);
    assert_eq!(run.status, RunStatus::Active);
    assert_eq!(run.source_locator.as_deref(), Some(locator));
}

#[test]
fn stdin_source_creates_run_with_stdin_log_source() {
    let db = temp_db_path("store-startup-stdin");
    let store = RunStore::open(&db).expect("store should open");

    let fp = source_fingerprint(RunSourceKind::Stdin, Some("stdin"), Some("/tmp/project"));
    let created = store
        .attach_or_create_active_run(
            &fp,
            RunSourceKind::Stdin,
            RunMetadata {
                source_locator: Some("stdin".to_string()),
                display_name: Some("stdin session".to_string()),
                ..RunMetadata::default()
            },
        )
        .expect("stdin attach should succeed");

    let run = store
        .get_run(&created.run_id)
        .expect("query should succeed")
        .expect("run should exist");
    assert_eq!(run.source_kind, RunSourceKind::Stdin);
    assert_eq!(run.source_locator.as_deref(), Some("stdin"));
}

#[test]
fn metrics_persist_updates_without_blocking_event_loop() {
    let db = temp_db_path("store-step-update");
    let store = RunStore::open(&db).expect("store should open");

    let created = store
        .attach_or_create_active_run(
            &source_fingerprint(RunSourceKind::LogFile, Some("/tmp/metrics.log"), None),
            RunSourceKind::LogFile,
            RunMetadata {
                source_locator: Some("/tmp/metrics.log".to_string()),
                ..RunMetadata::default()
            },
        )
        .expect("attach should succeed");

    let before = std::time::Instant::now();
    for step in 1..=1_000_u64 {
        store
            .update_last_step(&created.run_id, step)
            .expect("step update should succeed");
    }
    let elapsed = before.elapsed();

    let run = store
        .get_run(&created.run_id)
        .expect("query should succeed")
        .expect("run should exist");
    assert_eq!(run.last_step, Some(1_000));
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "step updates should remain bounded to avoid event-loop stalls"
    );
}

#[test]
fn run_store_handles_concurrent_writers_without_corruption() {
    use std::thread;

    let db = temp_db_path("store-concurrent");
    let mut handles = Vec::new();

    for worker in 0..4_u64 {
        let db_path = db.clone();
        handles.push(thread::spawn(move || {
            let store = RunStore::open(&db_path).expect("store should open in worker");
            let source = format!("/tmp/concurrent-{worker}.log");
            let attach = store
                .attach_or_create_active_run(
                    &source_fingerprint(RunSourceKind::LogFile, Some(&source), None),
                    RunSourceKind::LogFile,
                    RunMetadata {
                        source_locator: Some(source.clone()),
                        ..RunMetadata::default()
                    },
                )
                .expect("attach should succeed");

            for step in 1..=250_u64 {
                store
                    .update_last_step(&attach.run_id, step)
                    .expect("step update should succeed");
            }

            store
                .complete_run(&attach.run_id, RunStatus::Completed)
                .expect("completion should succeed");
            attach.run_id
        }));
    }

    let mut run_ids = Vec::new();
    for handle in handles {
        run_ids.push(handle.join().expect("worker thread should join"));
    }

    let verify = RunStore::open(&db).expect("store should reopen for verification");
    for run_id in run_ids {
        let run = verify
            .get_run(&run_id)
            .expect("query should succeed")
            .expect("run should persist");
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.last_step, Some(250));
    }
}
