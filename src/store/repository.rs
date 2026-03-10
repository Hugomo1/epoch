use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use color_eyre::Result;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use ulid::Ulid;

use crate::store::schema;
use crate::store::types::{
    EventJumpTarget, RunAttachResult, RunEventRecord, RunMetadata, RunRecord, RunSourceKind,
    RunStatus, now_epoch_secs,
};

pub struct RunStore {
    conn: Connection,
}

pub fn global_store_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "epoch").map(|dirs| {
        let data_dir = dirs.data_dir().to_path_buf();
        let _ = std::fs::create_dir_all(&data_dir);
        data_dir.join("runs.db")
    })
}

pub fn source_fingerprint(
    source_kind: RunSourceKind,
    source_locator: Option<&str>,
    project_root: Option<&str>,
) -> String {
    let mut hasher = DefaultHasher::new();
    source_kind.as_str().hash(&mut hasher);
    source_locator.unwrap_or_default().hash(&mut hasher);
    project_root.unwrap_or_default().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn step_to_i64(step: u64) -> Result<i64> {
    i64::try_from(step).map_err(|_| color_eyre::eyre::eyre!("step value too large for sqlite i64"))
}

fn row_to_run_record(row: &rusqlite::Row) -> rusqlite::Result<RunRecord> {
    let source_kind = row.get::<_, String>(2)?;
    let status = row.get::<_, String>(6)?;
    let last_step_i64 = row.get::<_, Option<i64>>(13)?;
    Ok(RunRecord {
        run_id: row.get(0)?,
        source_fingerprint: row.get(1)?,
        source_kind: RunSourceKind::from_db_value(&source_kind).unwrap_or(RunSourceKind::LogFile),
        source_locator: row.get(3)?,
        project_root: row.get(4)?,
        display_name: row.get(5)?,
        status: RunStatus::from_db_value(&status).unwrap_or(RunStatus::Active),
        command: row.get(7)?,
        cwd: row.get(8)?,
        git_commit: row.get(9)?,
        git_dirty: row.get::<_, Option<i64>>(10)?.map(|v| v != 0),
        started_at_epoch_secs: row.get(11)?,
        ended_at_epoch_secs: row.get(12)?,
        last_step: last_step_i64.and_then(|value| u64::try_from(value).ok()),
        last_updated_epoch_secs: row.get(14)?,
    })
}

impl RunStore {
    pub fn open(path: &Path) -> Result<Self> {
        const MAX_ATTEMPTS: usize = 20;
        const RETRY_DELAY: Duration = Duration::from_millis(50);

        let mut last_error = None;

        for attempt in 0..MAX_ATTEMPTS {
            match Self::open_once(path) {
                Ok(store) => return Ok(store),
                Err(err) if Self::is_lock_error(&err) && attempt + 1 < MAX_ATTEMPTS => {
                    last_error = Some(err);
                    thread::sleep(RETRY_DELAY);
                }
                Err(err) => return Err(err),
            }
        }

        match last_error {
            Some(err) => Err(err),
            None => color_eyre::eyre::bail!("failed to open run store after retries"),
        }
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<i64> {
        let value = self.conn.query_row(
            "SELECT value FROM schema_meta WHERE key='schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )?;
        Ok(value.parse::<i64>()?)
    }

    pub fn attach_or_create_active_run(
        &self,
        source_fingerprint: &str,
        source_kind: RunSourceKind,
        metadata: RunMetadata,
    ) -> Result<RunAttachResult> {
        let existing = self
            .conn
            .query_row(
                "
                SELECT run_id
                FROM runs
                WHERE source_fingerprint = ?1
                  AND status = 'active'
                ORDER BY started_at_epoch_secs DESC
                LIMIT 1
                ",
                params![source_fingerprint],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(run_id) = existing {
            return Ok(RunAttachResult {
                run_id,
                reused_existing_active: true,
            });
        }

        let run_id = Ulid::new().to_string();
        let now = now_epoch_secs();

        self.conn.execute(
            "
            INSERT INTO runs(
                run_id,
                source_fingerprint,
                source_kind,
                source_locator,
                project_root,
                display_name,
                status,
                command,
                cwd,
                git_commit,
                git_dirty,
                started_at_epoch_secs,
                ended_at_epoch_secs,
                last_step,
                last_updated_epoch_secs
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, ?8, ?9, ?10, ?11, NULL, NULL, ?12
            )
            ",
            params![
                run_id,
                source_fingerprint,
                source_kind.as_str(),
                metadata.source_locator,
                metadata.project_root,
                metadata.display_name,
                metadata.command,
                metadata.cwd,
                metadata.git_commit,
                metadata.git_dirty.map(i64::from),
                now,
                now,
            ],
        )?;

        Ok(RunAttachResult {
            run_id,
            reused_existing_active: false,
        })
    }

    pub fn update_last_step(&self, run_id: &str, step: u64) -> Result<()> {
        let now = now_epoch_secs();
        let step = step_to_i64(step)?;
        self.conn.execute(
            "
            UPDATE runs
            SET last_step = ?2,
                last_updated_epoch_secs = ?3
            WHERE run_id = ?1
            ",
            params![run_id, step, now],
        )?;
        Ok(())
    }

    pub fn complete_run(&self, run_id: &str, status: RunStatus) -> Result<()> {
        let end = now_epoch_secs();
        self.conn.execute(
            "
            UPDATE runs
            SET status = ?2,
                ended_at_epoch_secs = ?3,
                last_updated_epoch_secs = ?3
            WHERE run_id = ?1
            ",
            params![run_id, status.as_str(), end],
        )?;
        Ok(())
    }

    pub fn get_run(&self, run_id: &str) -> Result<Option<RunRecord>> {
        self.conn
            .query_row(
                "
                SELECT
                    run_id,
                    source_fingerprint,
                    source_kind,
                    source_locator,
                    project_root,
                    display_name,
                    status,
                    command,
                    cwd,
                    git_commit,
                    git_dirty,
                    started_at_epoch_secs,
                    ended_at_epoch_secs,
                    last_step,
                    last_updated_epoch_secs
                FROM runs
                WHERE run_id = ?1
                ",
                params![run_id],
                row_to_run_record,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn add_event(
        &self,
        run_id: &str,
        kind: &str,
        note: Option<&str>,
        pinned: bool,
        event_epoch_secs: i64,
        step: Option<u64>,
    ) -> Result<i64> {
        let step_i64 = step.map(step_to_i64).transpose()?;
        self.conn.execute(
            "
            INSERT INTO run_events(
                run_id,
                kind,
                note,
                pinned,
                event_epoch_secs,
                step
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                run_id,
                kind,
                note,
                i64::from(pinned),
                event_epoch_secs,
                step_i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_events(&self, run_id: &str) -> Result<Vec<RunEventRecord>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, run_id, kind, note, pinned, event_epoch_secs, step
            FROM run_events
            WHERE run_id = ?1
            ORDER BY event_epoch_secs ASC, id ASC
            ",
        )?;

        let mapped = stmt.query_map(params![run_id], |row| {
            let step = row.get::<_, Option<i64>>(6)?;
            Ok(RunEventRecord {
                id: row.get(0)?,
                run_id: row.get(1)?,
                kind: row.get(2)?,
                note: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
                event_epoch_secs: row.get(5)?,
                step: step.and_then(|value| u64::try_from(value).ok()),
            })
        })?;

        let mut events = Vec::new();
        for row in mapped {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn jump_to_event(&self, event_id: i64) -> Result<Option<EventJumpTarget>> {
        self.conn
            .query_row(
                "
                SELECT run_id, event_epoch_secs, step
                FROM run_events
                WHERE id = ?1
                ",
                params![event_id],
                |row| {
                    let step = row.get::<_, Option<i64>>(2)?;
                    Ok(EventJumpTarget {
                        run_id: row.get(0)?,
                        event_epoch_secs: row.get(1)?,
                        step: step.and_then(|value| u64::try_from(value).ok()),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_runs(
        &self,
        status_filter: Option<&str>,
        search_query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RunRecord>> {
        let search_pattern = search_query.map(|q| format!("%{}%", q));
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);

        let mut stmt = self.conn.prepare(
            "
            SELECT
                run_id,
                source_fingerprint,
                source_kind,
                source_locator,
                project_root,
                display_name,
                status,
                command,
                cwd,
                git_commit,
                git_dirty,
                started_at_epoch_secs,
                ended_at_epoch_secs,
                last_step,
                last_updated_epoch_secs
            FROM runs
            WHERE (?1 IS NULL OR status = ?1)
              AND (?2 IS NULL OR (display_name LIKE ?2 OR source_locator LIKE ?2))
            ORDER BY started_at_epoch_secs DESC
            LIMIT ?3
            ",
        )?;

        let mapped = stmt.query_map(
            params![status_filter, search_pattern, limit_i64],
            row_to_run_record,
        )?;

        let mut runs = Vec::new();
        for row in mapped {
            runs.push(row?);
        }
        Ok(runs)
    }

    pub fn list_recent_runs(&self, limit: usize) -> Result<Vec<RunRecord>> {
        self.list_runs(None, None, limit)
    }

    fn open_once(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        conn.busy_timeout(Duration::from_secs(10))?;
        if !Self::is_wal_mode(&conn)? {
            conn.pragma_update(None, "journal_mode", "WAL")?;
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    fn is_wal_mode(conn: &Connection) -> Result<bool> {
        let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        Ok(mode.eq_ignore_ascii_case("wal"))
    }

    fn is_lock_error(err: &color_eyre::Report) -> bool {
        let message = err.to_string().to_lowercase();
        message.contains("database is locked") || message.contains("database schema is locked")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_runs_empty_store() -> Result<()> {
        let store = RunStore::open_in_memory()?;
        let runs = store.list_runs(None, None, 10)?;
        assert_eq!(runs.len(), 0);
        Ok(())
    }

    #[test]
    fn test_list_runs_with_limit() -> Result<()> {
        let store = RunStore::open_in_memory()?;

        // Create 5 runs
        for i in 0..5 {
            let fingerprint = format!("fp_{}", i);
            let metadata = RunMetadata {
                display_name: Some(format!("run_{}", i)),
                project_root: Some("/project".to_string()),
                command: Some("python train.py".to_string()),
                cwd: Some("/project".to_string()),
                git_commit: Some("abc123".to_string()),
                git_dirty: Some(false),
                source_locator: Some(format!("train_{}.log", i)),
            };
            store.attach_or_create_active_run(&fingerprint, RunSourceKind::LogFile, metadata)?;
        }

        // Test limit
        let runs = store.list_runs(None, None, 2)?;
        assert_eq!(runs.len(), 2);

        // Test no limit (large limit)
        let runs = store.list_runs(None, None, 100)?;
        assert_eq!(runs.len(), 5);

        Ok(())
    }

    #[test]
    fn test_list_runs_status_filter() -> Result<()> {
        let store = RunStore::open_in_memory()?;

        // Create 3 active runs
        for i in 0..3 {
            let fingerprint = format!("fp_active_{}", i);
            let metadata = RunMetadata {
                display_name: Some(format!("active_{}", i)),
                project_root: Some("/project".to_string()),
                command: Some("python train.py".to_string()),
                cwd: Some("/project".to_string()),
                git_commit: Some("abc123".to_string()),
                git_dirty: Some(false),
                source_locator: Some(format!("train_{}.log", i)),
            };
            store.attach_or_create_active_run(&fingerprint, RunSourceKind::LogFile, metadata)?;
        }

        // Create 2 completed runs
        for i in 0..2 {
            let fingerprint = format!("fp_completed_{}", i);
            let metadata = RunMetadata {
                display_name: Some(format!("completed_{}", i)),
                project_root: Some("/project".to_string()),
                command: Some("python train.py".to_string()),
                cwd: Some("/project".to_string()),
                git_commit: Some("abc123".to_string()),
                git_dirty: Some(false),
                source_locator: Some(format!("train_{}.log", i)),
            };
            let result = store.attach_or_create_active_run(
                &fingerprint,
                RunSourceKind::LogFile,
                metadata,
            )?;
            store.complete_run(&result.run_id, RunStatus::Completed)?;
        }

        // Filter by active status
        let active_runs = store.list_runs(Some("active"), None, 100)?;
        assert_eq!(active_runs.len(), 3);
        for run in &active_runs {
            assert_eq!(run.status, RunStatus::Active);
        }

        // Filter by completed status
        let completed_runs = store.list_runs(Some("completed"), None, 100)?;
        assert_eq!(completed_runs.len(), 2);
        for run in &completed_runs {
            assert_eq!(run.status, RunStatus::Completed);
        }

        // No filter returns all
        let all_runs = store.list_runs(None, None, 100)?;
        assert_eq!(all_runs.len(), 5);

        Ok(())
    }

    #[test]
    fn test_list_runs_search_filter() -> Result<()> {
        let store = RunStore::open_in_memory()?;

        // Create runs with different names and locators
        let metadata1 = RunMetadata {
            display_name: Some("training_experiment_v1".to_string()),
            project_root: Some("/project".to_string()),
            command: Some("python train.py".to_string()),
            cwd: Some("/project".to_string()),
            git_commit: Some("abc123".to_string()),
            git_dirty: Some(false),
            source_locator: Some("logs/train_v1.log".to_string()),
        };
        store.attach_or_create_active_run("fp1", RunSourceKind::LogFile, metadata1)?;

        let metadata2 = RunMetadata {
            display_name: Some("inference_test".to_string()),
            project_root: Some("/project".to_string()),
            command: Some("python infer.py".to_string()),
            cwd: Some("/project".to_string()),
            git_commit: Some("def456".to_string()),
            git_dirty: Some(false),
            source_locator: Some("logs/infer_test.log".to_string()),
        };
        store.attach_or_create_active_run("fp2", RunSourceKind::LogFile, metadata2)?;

        let metadata3 = RunMetadata {
            display_name: Some("eval_metrics".to_string()),
            project_root: Some("/project".to_string()),
            command: Some("python eval.py".to_string()),
            cwd: Some("/project".to_string()),
            git_commit: Some("ghi789".to_string()),
            git_dirty: Some(false),
            source_locator: Some("logs/eval_metrics.log".to_string()),
        };
        store.attach_or_create_active_run("fp3", RunSourceKind::LogFile, metadata3)?;

        // Search by display_name
        let results = store.list_runs(None, Some("training"), 100)?;
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].display_name,
            Some("training_experiment_v1".to_string())
        );

        // Search by source_locator
        let results = store.list_runs(None, Some("infer"), 100)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, Some("inference_test".to_string()));

        // Search with no matches
        let results = store.list_runs(None, Some("nonexistent"), 100)?;
        assert_eq!(results.len(), 0);

        // Search with partial match
        let results = store.list_runs(None, Some("eval"), 100)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, Some("eval_metrics".to_string()));

        Ok(())
    }

    #[test]
    fn test_list_recent_runs() -> Result<()> {
        let store = RunStore::open_in_memory()?;

        // Create 3 runs
        for i in 0..3 {
            let fingerprint = format!("fp_{}", i);
            let metadata = RunMetadata {
                display_name: Some(format!("run_{}", i)),
                project_root: Some("/project".to_string()),
                command: Some("python train.py".to_string()),
                cwd: Some("/project".to_string()),
                git_commit: Some("abc123".to_string()),
                git_dirty: Some(false),
                source_locator: Some(format!("train_{}.log", i)),
            };
            store.attach_or_create_active_run(&fingerprint, RunSourceKind::LogFile, metadata)?;
        }

        // list_recent_runs should return all with default limit
        let recent = store.list_recent_runs(10)?;
        assert_eq!(recent.len(), 3);

        // list_recent_runs with smaller limit
        let recent = store.list_recent_runs(2)?;
        assert_eq!(recent.len(), 2);

        Ok(())
    }

    #[test]
    fn test_list_runs_combined_filters() -> Result<()> {
        let store = RunStore::open_in_memory()?;

        // Create active runs with different names
        let metadata1 = RunMetadata {
            display_name: Some("training_v1".to_string()),
            project_root: Some("/project".to_string()),
            command: Some("python train.py".to_string()),
            cwd: Some("/project".to_string()),
            git_commit: Some("abc123".to_string()),
            git_dirty: Some(false),
            source_locator: Some("logs/train_v1.log".to_string()),
        };
        store.attach_or_create_active_run("fp1", RunSourceKind::LogFile, metadata1)?;

        let metadata2 = RunMetadata {
            display_name: Some("training_v2".to_string()),
            project_root: Some("/project".to_string()),
            command: Some("python train.py".to_string()),
            cwd: Some("/project".to_string()),
            git_commit: Some("def456".to_string()),
            git_dirty: Some(false),
            source_locator: Some("logs/train_v2.log".to_string()),
        };
        let result2 =
            store.attach_or_create_active_run("fp2", RunSourceKind::LogFile, metadata2)?;

        // Complete the second run
        store.complete_run(&result2.run_id, RunStatus::Completed)?;

        // Filter by active status AND search for "training"
        let results = store.list_runs(Some("active"), Some("training"), 100)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, Some("training_v1".to_string()));
        assert_eq!(results[0].status, RunStatus::Active);

        // Filter by completed status AND search for "training"
        let results = store.list_runs(Some("completed"), Some("training"), 100)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].display_name, Some("training_v2".to_string()));
        assert_eq!(results[0].status, RunStatus::Completed);

        Ok(())
    }
}
