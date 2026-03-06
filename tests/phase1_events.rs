use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use epoch::store::repository::{RunStore, source_fingerprint};
use epoch::store::types::{RunMetadata, RunSourceKind};

fn temp_db_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("epoch-phase1-events-{label}-{unique}"));
    fs::create_dir_all(&root).expect("temp directory should be created");
    root.join("runs.db")
}

fn create_run(store: &RunStore, source: &str) -> String {
    store
        .attach_or_create_active_run(
            &source_fingerprint(RunSourceKind::LogFile, Some(source), None),
            RunSourceKind::LogFile,
            RunMetadata {
                source_locator: Some(source.to_string()),
                ..RunMetadata::default()
            },
        )
        .expect("run attach should succeed")
        .run_id
}

#[test]
fn notes_and_bookmarks_persist_across_restart() {
    let db = temp_db_path("persist");
    let run_id = {
        let store = RunStore::open(&db).expect("store should open");
        let run_id = create_run(&store, "/tmp/events.log");
        store
            .add_event(
                &run_id,
                "manual_note",
                Some("note A"),
                false,
                1000,
                Some(10),
            )
            .expect("note event should save");
        store
            .add_event(
                &run_id,
                "bookmark",
                Some("bookmark B"),
                true,
                1001,
                Some(11),
            )
            .expect("bookmark event should save");
        run_id
    };

    let reopened = RunStore::open(&db).expect("store should reopen");
    let events = reopened.list_events(&run_id).expect("events should load");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, "manual_note");
    assert_eq!(events[1].kind, "bookmark");
    assert!(events[1].pinned);
}

#[test]
fn timeline_order_is_timestamp_then_id() {
    let db = temp_db_path("ordering");
    let store = RunStore::open(&db).expect("store should open");
    let run_id = create_run(&store, "/tmp/order.log");

    let first = store
        .add_event(&run_id, "manual_note", Some("first"), false, 2000, Some(1))
        .expect("first event should save");
    let second = store
        .add_event(&run_id, "manual_note", Some("second"), false, 2000, Some(2))
        .expect("second event should save");
    assert!(second > first);

    let events = store.list_events(&run_id).expect("events should load");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].id, first);
    assert_eq!(events[1].id, second);
}

#[test]
fn jump_to_event_returns_expected_run_position() {
    let db = temp_db_path("jump");
    let store = RunStore::open(&db).expect("store should open");
    let run_id = create_run(&store, "/tmp/jump.log");

    let event_id = store
        .add_event(
            &run_id,
            "bookmark",
            Some("jump-here"),
            true,
            3000,
            Some(321),
        )
        .expect("event should save");

    let target = store
        .jump_to_event(event_id)
        .expect("jump query should succeed")
        .expect("jump target should exist");
    assert_eq!(target.run_id, run_id);
    assert_eq!(target.event_epoch_secs, 3000);
    assert_eq!(target.step, Some(321));
}
