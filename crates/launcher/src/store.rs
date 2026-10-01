//! A folder of games: the executable at `<dir>/<id>` and its info at
//! `<dir>/<id>.info`, written when the game is added. Listing reads only the
//! info files and never runs a game. The same layout serves installed games
//! and the local catalog.

use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use arcade_sdk::info::{Info, valid_id};

/// How long a game binary gets to answer `--info` before it is killed.
const INFO_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Store {
    dir: PathBuf,
}

pub struct Installed {
    pub path: PathBuf,
    /// Err with the reason when the info file is missing or bad, or the binary is missing.
    pub info: Result<Info, String>,
}

impl Store {
    pub fn open(dir: PathBuf) -> Result<Store> {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create {}", dir.display()))?;
        Ok(Store { dir })
    }

    /// Every game in the folder sorted by id, broken ones included. Reads only
    /// `.info` files, so a stray or hostile binary never runs here.
    pub fn list(&self) -> Result<Vec<Installed>> {
        let mut ids = BTreeSet::new();
        for e in std::fs::read_dir(&self.dir)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().to_string();
            if e.file_type()?.is_file() && !name.starts_with('.') {
                ids.insert(name.strip_suffix(".info").unwrap_or(&name).to_string());
            }
        }
        Ok(ids
            .into_iter()
            .map(|id| Installed {
                info: self.read(&id).map_err(|e| format!("{e:#}")),
                path: self.dir.join(id),
            })
            .collect())
    }

    fn read(&self, id: &str) -> Result<Info> {
        let text = std::fs::read_to_string(self.dir.join(format!("{id}.info")))
            .context("no info file, install it again")?;
        let info = Info::parse(&text)?;
        if info.id != id {
            bail!("info file is for {}", info.id);
        }
        if !self.dir.join(id).is_file() {
            bail!("game binary is missing");
        }
        Ok(info)
    }

    /// Runs `<src> --info` once to learn what the game is, then adds it.
    pub fn install(&self, src: &Path) -> Result<Info> {
        // Absolute, because Command looks a bare name like "snake" up on PATH.
        let src = &std::fs::canonicalize(src)
            .with_context(|| format!("cannot find {}", src.display()))?;
        let info = info(src).with_context(|| format!("{} is not an arcade game", src.display()))?;
        self.add(&info, src)?;
        Ok(info)
    }

    /// Copies `bin` in as `<id>` and writes `<id>.info`, replacing older copies.
    /// Each file goes to a hidden temp name and is renamed, so a failed copy
    /// never leaves a half-written game.
    pub fn add(&self, info: &Info, bin: &Path) -> Result<()> {
        let tmp = self.dir.join(format!(".{}.tmp", info.id));
        std::fs::copy(bin, &tmp).with_context(|| format!("cannot copy {}", bin.display()))?;
        std::fs::rename(&tmp, self.dir.join(&info.id))?;
        std::fs::write(&tmp, info.to_text())?;
        std::fs::rename(&tmp, self.dir.join(format!("{}.info", info.id)))?;
        Ok(())
    }

    /// Deletes the binary and its info file.
    pub fn remove(&self, id: &str) -> Result<()> {
        if !valid_id(id) {
            bail!("{id:?} is not a game id");
        }
        let mut found = false;
        for path in [self.dir.join(id), self.dir.join(format!("{id}.info"))] {
            if path.exists() {
                std::fs::remove_file(&path)?;
                found = true;
            }
        }
        if !found {
            bail!("{id} is not installed");
        }
        Ok(())
    }
}

/// Runs `<path> --info` and parses what it prints. Kills it after INFO_TIMEOUT.
fn info(path: &Path) -> Result<Info> {
    let mut child = Command::new(path)
        .arg("--info")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("cannot run {}", path.display()))?;
    let deadline = Instant::now() + INFO_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            bail!("--info did not finish within {INFO_TIMEOUT:?}");
        }
        sleep(Duration::from_millis(10));
    };
    if !status.success() {
        bail!("--info exited with {status}");
    }
    let mut text = String::new();
    child.stdout.take().unwrap().read_to_string(&mut text)?;
    Info::parse(&text)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        os::unix::fs::PermissionsExt,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("arcade-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A shell script that answers --info like a game.
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn game(dir: &Path, id: &str) -> PathBuf {
        script(
            dir,
            &format!("build-{id}"),
            &format!("printf 'protocol=1\\nid={id}\\ntitle={id}\\nabout=a {id} game\\n'"),
        )
    }

    #[test]
    fn install_list_and_remove() {
        let root = temp_dir("store");
        let store = Store::open(root.join("games")).unwrap();
        assert!(store.list().unwrap().is_empty());

        let info = store.install(&game(&root, "snake")).unwrap();
        assert_eq!(info.id, "snake");
        // A bare file name in the current directory, not looked up on PATH.
        std::env::set_current_dir(&root).unwrap();
        store.install(Path::new("build-snake")).unwrap(); // upgrade in place
        store.install(&game(&root, "brick-ball")).unwrap();

        let ids: Vec<String> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|g| g.info.unwrap().id)
            .collect();
        assert_eq!(ids, ["brick-ball", "snake"]);

        store.remove("snake").unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(!root.join("games/snake.info").exists(), "info goes too");
        assert!(store.remove("snake").is_err(), "already removed");
        assert!(store.remove("../games").is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn listing_never_runs_a_binary() {
        let root = temp_dir("norun");
        let games = root.join("games");
        let store = Store::open(games.clone()).unwrap();
        let marker = root.join("ran");
        // Would leave a marker if run. Its info file is copied in by add.
        let bin = script(&root, "bin", &format!("touch {}", marker.display()));
        let info = Info::parse("protocol=1\nid=quiet\ntitle=quiet\nabout=a\n").unwrap();
        store.add(&info, &bin).unwrap();
        let all = store.list().unwrap();
        assert_eq!(all[0].info.as_ref().unwrap().id, "quiet");
        assert!(!marker.exists(), "list ran the game");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_files_that_are_not_games() {
        let root = temp_dir("reject");
        let store = Store::open(root.join("games")).unwrap();
        for (name, body) in [
            ("fails", "exit 1"),
            ("silent", "true"),
            (
                "escape",
                "printf 'protocol=1\\nid=../x\\ntitle=x\\nabout=x\\n'",
            ),
            ("hangs", "sleep 30"),
        ] {
            let err = store.install(&script(&root, name, body)).unwrap_err();
            assert!(
                format!("{err:#}").contains("not an arcade game"),
                "{name}: {err:#}"
            );
        }
        assert!(store.list().unwrap().is_empty(), "nothing was copied");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn broken_entries_are_listed_with_the_reason() {
        let root = temp_dir("broken");
        let games = root.join("games");
        let store = Store::open(games.clone()).unwrap();
        script(&games, "stray", "exit 0");
        std::fs::write(
            games.join("lost.info"),
            "protocol=1\nid=lost\ntitle=lost\nabout=a\n",
        )
        .unwrap();
        let all = store.list().unwrap();
        let reasons: Vec<String> = all
            .iter()
            .map(|g| g.info.as_ref().unwrap_err().clone())
            .collect();
        assert!(reasons[0].contains("binary is missing"), "{reasons:?}");
        assert!(reasons[1].contains("no info file"), "{reasons:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
