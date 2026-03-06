use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use color_eyre::Result;
use rusqlite::{Connection, OptionalExtension, params};
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

impl RunStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
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
                |row| {
                    let source_kind = row.get::<_, String>(2)?;
                    let status = row.get::<_, String>(6)?;
                    let last_step_i64 = row.get::<_, Option<i64>>(13)?;
                    Ok(RunRecord {
                        run_id: row.get(0)?,
                        source_fingerprint: row.get(1)?,
                        source_kind: RunSourceKind::from_db_value(&source_kind)
                            .unwrap_or(RunSourceKind::LogFile),
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
                },
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
}
