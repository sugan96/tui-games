//! When each game was last started from the launcher: one `<id> <unix secs>`
//! line per game in a text file only the launcher writes.

use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};

pub struct Played {
    path: PathBuf,
    pub at: HashMap<String, u64>,
}

impl Played {
    /// Reads the file. A missing file is empty and unreadable lines are skipped,
    /// so a damaged file only loses the order, never blocks the launcher.
    pub fn open(path: PathBuf) -> Played {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let at = text
            .lines()
            .filter_map(|l| {
                let (id, secs) = l.split_once(' ')?;
                Some((id.to_string(), secs.parse().ok()?))
            })
            .collect();
        Played { path, at }
    }

    /// Records a start and rewrites the file through a temp file and a rename,
    /// so a crash mid-write keeps the old file.
    pub fn touch(&mut self, id: &str, secs: u64) -> Result<()> {
        self.at.insert(id.to_string(), secs);
        let mut lines: Vec<String> = self
            .at
            .iter()
            .map(|(id, s)| format!("{id} {s}\n"))
            .collect();
        lines.sort();
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, lines.concat())
            .with_context(|| format!("cannot write {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn touch_persists_and_bad_lines_are_skipped() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("arcade-played-{}-{nanos}", std::process::id()));
        assert!(
            Played::open(path.clone()).at.is_empty(),
            "missing file is empty"
        );
        std::fs::write(&path, "snake 100\ngarbage\nbrick-ball notanumber\n").unwrap();
        let mut p = Played::open(path.clone());
        assert_eq!(p.at, HashMap::from([("snake".into(), 100)]));
        p.touch("brick-ball", 200).unwrap();
        p.touch("snake", 300).unwrap();
        let p = Played::open(path.clone());
        assert_eq!(
            p.at,
            HashMap::from([("snake".into(), 300), ("brick-ball".into(), 200)])
        );
        std::fs::remove_file(&path).unwrap();
    }
}
