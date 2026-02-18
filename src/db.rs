use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct PrimeRecord {
    pub id: i64,
    pub form: String,
    pub expression: String,
    pub digits: i64,
    pub found_at: String,
}

#[derive(Serialize)]
pub struct FormCount {
    pub form: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct Stats {
    pub total: i64,
    pub by_form: Vec<FormCount>,
    pub largest_digits: i64,
    pub largest_expression: Option<String>,
}

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

    pub fn get_stats(&self) -> Result<Stats> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM primes", [], |row| row.get(0))?;

        let mut stmt = self
            .conn
            .prepare("SELECT form, COUNT(*) as cnt FROM primes GROUP BY form ORDER BY cnt DESC")?;
        let by_form = stmt
            .query_map([], |row| {
                Ok(FormCount {
                    form: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let (largest_digits, largest_expression) = if total > 0 {
            self.conn.query_row(
                "SELECT digits, expression FROM primes ORDER BY digits DESC LIMIT 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, Some(row.get::<_, String>(1)?))),
            )?
        } else {
            (0, None)
        };

        Ok(Stats {
            total,
            by_form,
            largest_digits,
            largest_expression,
        })
    }

    pub fn get_primes(&self, limit: i64, offset: i64) -> Result<Vec<PrimeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, form, expression, digits, found_at FROM primes ORDER BY id DESC LIMIT ?1 OFFSET ?2",
        )?;
        let records = stmt
            .query_map(params![limit, offset], |row| {
                Ok(PrimeRecord {
                    id: row.get(0)?,
                    form: row.get(1)?,
                    expression: row.get(2)?,
                    digits: row.get(3)?,
                    found_at: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(records)
    }

    pub fn get_total_count(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM primes", [], |row| row.get(0))?;
        Ok(count)
    }
}
