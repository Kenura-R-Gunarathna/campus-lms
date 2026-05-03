use rusqlite::{Connection, Result, params};
use std::collections::HashMap;

pub struct Storage {
    conn: Connection,
}

#[derive(Clone)]
pub struct ContentChange {
    pub course_id: u64,
    pub module_id: u64,
    pub module_name: String,
    pub section_name: String,
    pub change_type: String, // "added" | "removed" | "renamed" | "file_updated"
    pub old_val: String,
    pub new_val: String,
    pub detected_at: i64,
}

#[derive(Clone)]
pub struct StoredFingerprint {
    pub module_id: u64,
    pub name: String,
    pub filesize: i64,
    pub fileurl: String,
    pub description: String,
}

#[derive(Clone)]
pub struct ActivityEntry {
    pub course_id: u64,
    pub course_name: String,
    pub module_id: u64,
    pub module_name: String,
    pub section_name: String,
    pub action: String, // "downloaded" | "streamed" | "opened"
    pub timestamp: i64,
}

impl Storage {
    pub fn open() -> Result<Self> {
        let path = dirs_next::data_dir()
            .unwrap_or_default()
            .join("campus-lms")
            .join("data.db");
        std::fs::create_dir_all(path.parent().unwrap()).ok();
        let conn = Connection::open(path)?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS session (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS course_metrics (
                course_id   INTEGER PRIMARY KEY,
                open_count  INTEGER NOT NULL DEFAULT 0,
                total_secs  INTEGER NOT NULL DEFAULT 0,
                last_opened TEXT
            );
            CREATE TABLE IF NOT EXISTS cache (
                key        TEXT PRIMARY KEY,
                value      TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS content_fingerprints (
                module_id    INTEGER PRIMARY KEY,
                course_id    INTEGER NOT NULL,
                name         TEXT NOT NULL,
                filesize     INTEGER NOT NULL DEFAULT 0,
                fileurl      TEXT NOT NULL DEFAULT '',
                description  TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS content_changes (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                course_id    INTEGER NOT NULL,
                module_id    INTEGER NOT NULL,
                module_name  TEXT NOT NULL,
                section_name TEXT NOT NULL DEFAULT '',
                change_type  TEXT NOT NULL,
                old_val      TEXT NOT NULL DEFAULT '',
                new_val      TEXT NOT NULL DEFAULT '',
                is_seen      INTEGER NOT NULL DEFAULT 0,
                detected_at  INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS user_activity (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                course_id    INTEGER NOT NULL,
                course_name  TEXT NOT NULL DEFAULT '',
                module_id    INTEGER NOT NULL,
                module_name  TEXT NOT NULL,
                section_name TEXT NOT NULL DEFAULT '',
                action       TEXT NOT NULL,
                timestamp    INTEGER NOT NULL
            );
        ")?;
        Ok(Self { conn })
    }

    // ── Session ──────────────────────────────────────────────────────────────

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO session (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM session WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        Ok(rows.next()?.map(|r| r.get(0).unwrap()))
    }

    pub fn clear_session(&self) -> Result<()> {
        self.conn.execute("DELETE FROM session", [])?;
        Ok(())
    }

    // ── Course metrics ────────────────────────────────────────────────────────

    pub fn record_course_open(&self, course_id: u64) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO course_metrics (course_id, open_count, total_secs, last_opened)
             VALUES (?1, 1, 0, ?2)
             ON CONFLICT(course_id) DO UPDATE SET
               open_count  = open_count + 1,
               last_opened = excluded.last_opened",
            params![course_id as i64, now],
        )?;
        Ok(())
    }

    pub fn add_course_time(&self, course_id: u64, seconds: u64) -> Result<()> {
        if seconds == 0 { return Ok(()); }
        self.conn.execute(
            "INSERT INTO course_metrics (course_id, open_count, total_secs, last_opened)
             VALUES (?1, 0, ?2, NULL)
             ON CONFLICT(course_id) DO UPDATE SET
               total_secs = total_secs + excluded.total_secs",
            params![course_id as i64, seconds as i64],
        )?;
        Ok(())
    }

    pub fn get_course_metrics(&self, course_id: u64) -> Result<(u64, u64)> {
        let mut stmt = self.conn.prepare(
            "SELECT open_count, total_secs FROM course_metrics WHERE course_id = ?1"
        )?;
        let mut rows = stmt.query(params![course_id as i64])?;
        Ok(rows.next()?.map(|r| {
            let opens: i64 = r.get(0).unwrap_or(0);
            let secs: i64 = r.get(1).unwrap_or(0);
            (opens as u64, secs as u64)
        }).unwrap_or((0, 0)))
    }

    // ── Content cache ─────────────────────────────────────────────────────────

    pub fn save_cache(&self, key: &str, value: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT OR REPLACE INTO cache (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, now],
        )?;
        Ok(())
    }

    pub fn load_cache(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM cache WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        Ok(rows.next()?.map(|r| r.get(0).unwrap()))
    }

    pub fn clear_cache(&self) -> Result<()> {
        self.conn.execute("DELETE FROM cache", [])?;
        Ok(())
    }

    // ── Content fingerprints ──────────────────────────────────────────────────

    pub fn load_fingerprints(&self, course_id: u64) -> Result<Vec<StoredFingerprint>> {
        let mut stmt = self.conn.prepare(
            "SELECT module_id, name, filesize, fileurl, description
             FROM content_fingerprints WHERE course_id = ?1"
        )?;
        let rows = stmt.query_map(params![course_id as i64], |r| {
            Ok(StoredFingerprint {
                module_id: r.get::<_, i64>(0)? as u64,
                name: r.get(1)?,
                filesize: r.get(2)?,
                fileurl: r.get(3)?,
                description: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_fingerprints(&self, course_id: u64, fps: &[StoredFingerprint]) -> Result<()> {
        for fp in fps {
            self.conn.execute(
                "INSERT OR REPLACE INTO content_fingerprints
                 (module_id, course_id, name, filesize, fileurl, description)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![fp.module_id as i64, course_id as i64,
                        fp.name, fp.filesize, fp.fileurl, fp.description],
            )?;
        }
        Ok(())
    }

    pub fn delete_fingerprints(&self, module_ids: &[u64]) -> Result<()> {
        for &id in module_ids {
            self.conn.execute(
                "DELETE FROM content_fingerprints WHERE module_id = ?1",
                params![id as i64],
            )?;
        }
        Ok(())
    }

    // ── Content changes ───────────────────────────────────────────────────────

    pub fn save_changes(&self, changes: &[ContentChange]) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        for c in changes {
            self.conn.execute(
                "INSERT INTO content_changes
                 (course_id, module_id, module_name, section_name, change_type, old_val, new_val, detected_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    c.course_id as i64, c.module_id as i64,
                    c.module_name, c.section_name, c.change_type,
                    c.old_val, c.new_val, now
                ],
            )?;
        }
        Ok(())
    }

    pub fn unseen_change_counts(&self) -> Result<HashMap<u64, u32>> {
        let mut stmt = self.conn.prepare(
            "SELECT course_id, COUNT(*) FROM content_changes
             WHERE is_seen = 0 GROUP BY course_id"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i32>(1)? as u32))
        })?;
        let mut map = HashMap::new();
        for row in rows { let (k, v) = row?; map.insert(k, v); }
        Ok(map)
    }

    pub fn mark_changes_seen(&self, course_id: u64) -> Result<()> {
        self.conn.execute(
            "UPDATE content_changes SET is_seen = 1 WHERE course_id = ?1",
            params![course_id as i64],
        )?;
        Ok(())
    }

    pub fn recent_changes(&self, course_id: u64, limit: usize) -> Result<Vec<ContentChange>> {
        let mut stmt = self.conn.prepare(
            "SELECT course_id, module_id, module_name, section_name,
                    change_type, old_val, new_val, detected_at
             FROM content_changes WHERE course_id = ?1
             ORDER BY detected_at DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![course_id as i64, limit as i64], |r| {
            Ok(ContentChange {
                course_id: r.get::<_, i64>(0)? as u64,
                module_id: r.get::<_, i64>(1)? as u64,
                module_name: r.get(2)?,
                section_name: r.get(3)?,
                change_type: r.get(4)?,
                old_val: r.get(5)?,
                new_val: r.get(6)?,
                detected_at: r.get(7)?,
            })
        })?;
        rows.collect()
    }

    // ── User activity ─────────────────────────────────────────────────────────

    pub fn record_activity(&self, entry: &ActivityEntry) -> Result<()> {
        self.conn.execute(
            "INSERT INTO user_activity
             (course_id, course_name, module_id, module_name, section_name, action, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.course_id as i64, entry.course_name,
                entry.module_id as i64, entry.module_name,
                entry.section_name, entry.action, entry.timestamp,
            ],
        )?;
        Ok(())
    }

    pub fn recent_activity(&self, limit: usize) -> Result<Vec<ActivityEntry>> {
        // Deduplicate by module_id — keep only the most recent action per module
        let mut stmt = self.conn.prepare(
            "SELECT course_id, course_name, module_id, module_name, section_name, action, MAX(timestamp)
             FROM user_activity
             GROUP BY module_id
             ORDER BY MAX(timestamp) DESC
             LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok(ActivityEntry {
                course_id: r.get::<_, i64>(0)? as u64,
                course_name: r.get(1)?,
                module_id: r.get::<_, i64>(2)? as u64,
                module_name: r.get(3)?,
                section_name: r.get(4)?,
                action: r.get(5)?,
                timestamp: r.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn clear_telemetry(&self) -> Result<()> {
        self.conn.execute_batch("
            DELETE FROM content_fingerprints;
            DELETE FROM content_changes;
            DELETE FROM user_activity;
        ")?;
        Ok(())
    }
}
