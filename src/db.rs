use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS primes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                form TEXT NOT NULL,
                expression TEXT NOT NULL,
                digits INTEGER NOT NULL,
                found_at TEXT NOT NULL,
                search_params TEXT NOT NULL
            );",
        )?;
        Ok(Database { conn })
    }

    pub fn insert_prime(
        &self,
        form: &str,
        expression: &str,
        digits: u64,
        search_params: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO primes (form, expression, digits, found_at, search_params)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![form, expression, digits as i64, now, search_params],
        )?;
        Ok(())
    }
}
