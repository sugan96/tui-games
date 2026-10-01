use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::{Connection, ErrorCode, params};

pub const TOP_N: usize = 10;

/// Schema version 1. Version 0 is either a new file or the snake-only `games` table.
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS scores (
  id INTEGER PRIMARY KEY,
  game TEXT NOT NULL,
  variant TEXT NOT NULL,
  name TEXT,
  score INTEGER NOT NULL,
  played_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);";

/// Copies rows from the snake-only table, where mode 0 was variable and 1 to 9 fixed.
const MIGRATE_SNAKE: &str = "INSERT INTO scores (id, game, variant, name, score, played_at)
  SELECT id, 'snake', CASE mode WHEN 0 THEN 'variable' ELSE 'fixed ' || mode END,
         name, score, played_at FROM games;
DROP TABLE games;";

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreRow {
    pub name: String,
    pub score: u32,
    /// Game-defined label, for example "fixed 5".
    pub variant: String,
}

/// False when the file is not a SQLite database or fails the integrity check.
/// Other errors (permissions, IO) are returned so a readable file is never renamed by mistake.
fn is_valid(conn: &Connection) -> rusqlite::Result<bool> {
    match conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)) {
        Ok(s) => Ok(s == "ok"),
        Err(e)
            if matches!(
                e.sqlite_error_code(),
                Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
            ) =>
        {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

impl Db {
    /// Opens ~/.config/snake/snake.db, creating the directory.
    pub fn open_default() -> Result<Db> {
        let dir = std::env::home_dir()
            .context("home directory is unknown")?
            .join(".config/snake");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;
        Db::open(&dir.join("snake.db"))
    }

    /// Opens the file at path. A file that is not a valid SQLite database is renamed
    /// to "<path>.broken-<unix seconds>" and a fresh database is created. Never deletes it.
    pub fn open(path: &Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        if is_valid(&conn)? {
            return Db::init(conn);
        }
        drop(conn);
        let secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let mut broken = path.as_os_str().to_owned();
        broken.push(format!(".broken-{secs}"));
        std::fs::rename(path, &broken)
            .with_context(|| format!("cannot move aside corrupt {}", path.display()))?;
        Db::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Db> {
        Db::init(Connection::open_in_memory()?)
    }

    /// Creates the schema, migrating the snake-only table in one transaction.
    fn init(mut conn: Connection) -> Result<Db> {
        let tx = conn.transaction()?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            tx.execute_batch(SCHEMA)?;
            let old: bool = tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'games')",
                [],
                |r| r.get(0),
            )?;
            if old {
                tx.execute_batch(MIGRATE_SNAKE)?;
            }
            tx.execute_batch("PRAGMA user_version = 1")?;
        }
        tx.commit()?;
        Ok(Db { conn })
    }

    /// Inserts a row with name NULL. Returns the row id.
    pub fn record(&self, game: &str, variant: &str, score: u32) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO scores (game, variant, score) VALUES (?1, ?2, ?3)",
            params![game, variant, score],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Sets the name on a row.
    pub fn set_name(&self, id: i64, name: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE scores SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        anyhow::ensure!(n == 1, "no game with id {id}");
        Ok(())
    }

    /// Named rows of one game, ordered by score desc then id asc, limited to n.
    pub fn top(&self, game: &str, n: usize) -> Result<Vec<ScoreRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, score, variant FROM scores WHERE game = ?1 AND name IS NOT NULL
             ORDER BY score DESC, id ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![game, n as i64], |r| {
            Ok(ScoreRow {
                name: r.get(0)?,
                score: r.get(1)?,
                variant: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// score > 0 and (fewer than TOP_N named rows of the game or score > its TOP_N-th named score).
    pub fn qualifies(&self, game: &str, score: u32) -> Result<bool> {
        let top = self.top(game, TOP_N)?;
        Ok(score > 0 && (top.len() < TOP_N || score > top[TOP_N - 1].score))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: &str = "snake";

    fn add(db: &Db, name: &str, score: u32) -> i64 {
        let id = db.record(G, "fixed 5", score).unwrap();
        db.set_name(id, name).unwrap();
        id
    }

    #[test]
    fn unnamed_rows_are_hidden_until_named() {
        let db = Db::open_in_memory().unwrap();
        let id = db.record(G, "variable", 7).unwrap();
        assert!(db.top(G, TOP_N).unwrap().is_empty());
        db.set_name(id, "ABC").unwrap();
        assert_eq!(
            db.top(G, TOP_N).unwrap(),
            vec![ScoreRow {
                name: "ABC".into(),
                score: 7,
                variant: "variable".into()
            }]
        );
    }

    #[test]
    fn games_have_separate_tables() {
        let db = Db::open_in_memory().unwrap();
        add(&db, "SNK", 5);
        let id = db.record("other", "normal", 9).unwrap();
        db.set_name(id, "OTH").unwrap();
        assert_eq!(db.top(G, TOP_N).unwrap()[0].name, "SNK");
        assert_eq!(db.top("other", TOP_N).unwrap()[0].name, "OTH");
    }

    #[test]
    fn orders_by_score_desc_then_id() {
        let db = Db::open_in_memory().unwrap();
        add(&db, "LOW", 1);
        add(&db, "FST", 5);
        add(&db, "SND", 5);
        add(&db, "TOP", 9);
        let names: Vec<String> = db.top(G, 3).unwrap().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["TOP", "FST", "SND"]);
    }

    #[test]
    fn qualifies_rules() {
        let db = Db::open_in_memory().unwrap();
        assert!(!db.qualifies(G, 0).unwrap());
        assert!(db.qualifies(G, 1).unwrap());
        for i in 0..TOP_N as u32 {
            add(&db, "AAA", 10 + i);
        }
        // Tenth best score is 10.
        assert!(!db.qualifies(G, 0).unwrap());
        assert!(!db.qualifies(G, 10).unwrap());
        assert!(db.qualifies(G, 11).unwrap());
        assert!(
            db.qualifies("other", 1).unwrap(),
            "another game's rows do not count"
        );
    }

    #[test]
    fn qualifies_at_nine_and_eleven_named_rows() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..TOP_N as u32 - 1 {
            add(&db, "AAA", 10 + i);
        }
        assert!(db.qualifies(G, 1).unwrap());

        let db = Db::open_in_memory().unwrap();
        for i in 0..TOP_N as u32 + 1 {
            add(&db, "AAA", 10 + i);
        }
        // Scores 10 to 20. Tenth best is 11, lowest is 10.
        assert!(!db.qualifies(G, 11).unwrap());
        assert!(db.qualifies(G, 12).unwrap());
        assert!(!db.qualifies(G, 10).unwrap());
    }

    #[test]
    fn snake_table_migrates_to_scores() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE games (id INTEGER PRIMARY KEY, name TEXT, score INTEGER NOT NULL,
               apples INTEGER NOT NULL, mode INTEGER NOT NULL,
               played_at TEXT NOT NULL DEFAULT '2026-09-24T00:00:00Z');
             INSERT INTO games (name, score, apples, mode) VALUES
               ('ABC', 42, 9, 5), ('XYZ', 7, 3, 0), (NULL, 99, 20, 9);",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        assert_eq!(
            db.top(G, TOP_N).unwrap(),
            vec![
                ScoreRow {
                    name: "ABC".into(),
                    score: 42,
                    variant: "fixed 5".into()
                },
                ScoreRow {
                    name: "XYZ".into(),
                    score: 7,
                    variant: "variable".into()
                },
            ]
        );
        // The unnamed row came across too: its id still accepts a name.
        db.set_name(3, "LAT").unwrap();
        assert_eq!(db.top(G, 1).unwrap()[0].variant, "fixed 9");
        // A second open does not migrate again.
        let id = db.record(G, "fixed 1", 1).unwrap();
        let db = Db::init(db.conn).unwrap();
        db.set_name(id, "AGN").unwrap();
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snake-test-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snake.db");
        std::fs::write(&path, vec![0xAB; 4096]).unwrap();

        let db = Db::open(&path).unwrap();
        add(&db, "NEW", 4);
        assert_eq!(db.top(G, TOP_N).unwrap().len(), 1);
        drop(db);

        let broken = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("snake.db.broken-")
            })
            .count();
        assert_eq!(broken, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
