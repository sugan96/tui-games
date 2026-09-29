use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::{Connection, ErrorCode, params};

use crate::game::Mode;

pub const TOP_N: usize = 10;

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS games (
  id INTEGER PRIMARY KEY,
  name TEXT,
  score INTEGER NOT NULL,
  apples INTEGER NOT NULL,
  mode INTEGER NOT NULL,
  played_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);";

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreRow {
    pub name: String,
    pub score: u32,
    pub mode: Mode,
}

pub struct GameRecord {
    pub score: u32,
    pub apples: u32,
    pub mode: Mode,
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

    fn init(conn: Connection) -> Result<Db> {
        conn.execute_batch(SCHEMA)?;
        Ok(Db { conn })
    }

    /// Inserts a row with name NULL. Returns the row id.
    pub fn record_game(&self, rec: &GameRecord) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO games (score, apples, mode) VALUES (?1, ?2, ?3)",
            params![rec.score, rec.apples, rec.mode.as_db()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Sets the name on a row.
    pub fn set_name(&self, id: i64, name: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE games SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        anyhow::ensure!(n == 1, "no game with id {id}");
        Ok(())
    }

    /// Named rows only, ordered by score desc then id asc, limited to n.
    pub fn top_scores(&self, n: usize) -> Result<Vec<ScoreRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, score, mode FROM games WHERE name IS NOT NULL
             ORDER BY score DESC, id ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map([n as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (name, score, mode) = row?;
            let mode = Mode::from_db(mode).with_context(|| format!("bad mode {mode}"))?;
            Ok(ScoreRow { name, score, mode })
        })
        .collect()
    }

    /// score > 0 and (fewer than TOP_N named rows or score > the TOP_N-th named score).
    pub fn qualifies(&self, score: u32) -> Result<bool> {
        let top = self.top_scores(TOP_N)?;
        Ok(score > 0 && (top.len() < TOP_N || score > top[TOP_N - 1].score))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(db: &Db, name: &str, score: u32) -> i64 {
        let id = db
            .record_game(&GameRecord {
                score,
                apples: 1,
                mode: Mode::Fixed(5),
            })
            .unwrap();
        db.set_name(id, name).unwrap();
        id
    }

    #[test]
    fn unnamed_rows_are_hidden_until_named() {
        let db = Db::open_in_memory().unwrap();
        let id = db
            .record_game(&GameRecord {
                score: 7,
                apples: 2,
                mode: Mode::Variable,
            })
            .unwrap();
        assert!(db.top_scores(TOP_N).unwrap().is_empty());
        db.set_name(id, "ABC").unwrap();
        assert_eq!(
            db.top_scores(TOP_N).unwrap(),
            vec![ScoreRow {
                name: "ABC".into(),
                score: 7,
                mode: Mode::Variable
            }]
        );
    }

    #[test]
    fn orders_by_score_desc_then_id() {
        let db = Db::open_in_memory().unwrap();
        add(&db, "LOW", 1);
        add(&db, "FST", 5);
        add(&db, "SND", 5);
        add(&db, "TOP", 9);
        let names: Vec<String> = db
            .top_scores(3)
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, ["TOP", "FST", "SND"]);
    }

    #[test]
    fn qualifies_rules() {
        let db = Db::open_in_memory().unwrap();
        assert!(!db.qualifies(0).unwrap());
        assert!(db.qualifies(1).unwrap());
        for i in 0..TOP_N as u32 {
            add(&db, "AAA", 10 + i);
        }
        // Tenth best score is 10.
        assert!(!db.qualifies(0).unwrap());
        assert!(!db.qualifies(10).unwrap());
        assert!(db.qualifies(11).unwrap());
    }

    #[test]
    fn qualifies_at_nine_and_eleven_named_rows() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..TOP_N as u32 - 1 {
            add(&db, "AAA", 10 + i);
        }
        assert!(db.qualifies(1).unwrap());

        let db = Db::open_in_memory().unwrap();
        for i in 0..TOP_N as u32 + 1 {
            add(&db, "AAA", 10 + i);
        }
        // Scores 10 to 20. Tenth best is 11, lowest is 10.
        assert!(!db.qualifies(11).unwrap());
        assert!(db.qualifies(12).unwrap());
        assert!(!db.qualifies(10).unwrap());
    }

    #[test]
    fn mode_round_trips() {
        let db = Db::open_in_memory().unwrap();
        for m in [Mode::Variable, Mode::Fixed(1), Mode::Fixed(9)] {
            let id = db
                .record_game(&GameRecord {
                    score: 3,
                    apples: 0,
                    mode: m,
                })
                .unwrap();
            db.set_name(id, "MOD").unwrap();
        }
        let modes: Vec<Mode> = db
            .top_scores(TOP_N)
            .unwrap()
            .into_iter()
            .map(|r| r.mode)
            .collect();
        assert_eq!(modes, [Mode::Variable, Mode::Fixed(1), Mode::Fixed(9)]);
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
        assert_eq!(db.top_scores(TOP_N).unwrap().len(), 1);
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
