use color_eyre::Result;
use rusqlite::{Connection, OptionalExtension, params};

pub const SCHEMA_VERSION: i64 = 1;

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;

    let existing_version = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if let Some(version) = existing_version {
        let parsed = version.parse::<i64>().unwrap_or(-1);
        if parsed > SCHEMA_VERSION {
            color_eyre::eyre::bail!(
                "unsupported schema version {parsed}; max supported is {}",
                SCHEMA_VERSION
            );
        }
    }

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS runs (
            run_id TEXT PRIMARY KEY,
            source_fingerprint TEXT NOT NULL,
            source_kind TEXT NOT NULL,
            source_locator TEXT,
            project_root TEXT,
            display_name TEXT,
            status TEXT NOT NULL,
            command TEXT,
            cwd TEXT,
            git_commit TEXT,
            git_dirty INTEGER,
            started_at_epoch_secs INTEGER NOT NULL,
            ended_at_epoch_secs INTEGER,
            last_step INTEGER,
            last_updated_epoch_secs INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_runs_source_fingerprint ON runs(source_fingerprint);
        CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
        CREATE INDEX IF NOT EXISTS idx_runs_project_root ON runs(project_root);
        CREATE INDEX IF NOT EXISTS idx_runs_started_at ON runs(started_at_epoch_secs DESC);

        CREATE TABLE IF NOT EXISTS run_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            note TEXT,
            pinned INTEGER NOT NULL DEFAULT 0,
            event_epoch_secs INTEGER NOT NULL,
            step INTEGER,
            FOREIGN KEY(run_id) REFERENCES runs(run_id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_run_events_run_id ON run_events(run_id);
        CREATE INDEX IF NOT EXISTS idx_run_events_time ON run_events(event_epoch_secs, id);
        ",
    )?;

    conn.execute(
        "INSERT OR REPLACE INTO schema_meta(key, value) VALUES('schema_version', ?1)",
        params![SCHEMA_VERSION.to_string()],
    )?;

    Ok(())
}
