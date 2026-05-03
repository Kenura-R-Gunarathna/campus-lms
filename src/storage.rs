use rusqlite::{Connection, Result, params};

pub struct Storage {
    conn: Connection,
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
        ")?;
        Ok(Self { conn })
    }

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
}
