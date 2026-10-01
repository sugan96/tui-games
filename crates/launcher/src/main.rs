//! `arcade`: the game library. Shows installed games and games in the catalog,
//! plays, gets and removes them. Each game is its own executable built on arcade-sdk.

mod library;
mod played;
mod store;
mod ui;

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use played::Played;
use store::Store;
use ui::{Action, Library};

const USAGE: &str =
    "usage: arcade [install <game binary> | publish <game binary> | remove <id> | list]";

/// Installed games, the catalog and the last played file.
struct Dirs {
    games: Store,
    catalog: Store,
    catalog_dir: PathBuf,
    played: Played,
}

fn main() -> Result<()> {
    let mut dirs = open()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => play(&mut dirs),
        ["install", path] => {
            let info = dirs.games.install(Path::new(path))?;
            println!("installed {}", info.id);
            Ok(())
        }
        ["publish", path] => {
            let info = dirs.catalog.install(Path::new(path))?;
            println!("published {} to {}", info.id, dirs.catalog_dir.display());
            Ok(())
        }
        ["remove", id] => {
            dirs.games.remove(id)?;
            println!("removed {id}");
            Ok(())
        }
        ["list"] => {
            for c in cards(&dirs)? {
                let state = match (&c.info, &c.installed) {
                    (Err(e), _) => format!("broken: {e}"),
                    (Ok(_), Some(_)) => "installed".into(),
                    (Ok(_), None) => "available".into(),
                };
                println!("{}\t{state}", c.id().unwrap_or(&c.name));
            }
            Ok(())
        }
        _ => bail!(USAGE),
    }
}

/// Games in ~/.local/share/arcade/games, the catalog in
/// ~/.local/share/arcade/catalog unless ARCADE_CATALOG names another folder.
fn open() -> Result<Dirs> {
    let home = std::env::home_dir().context("home directory is unknown")?;
    let data = home.join(".local/share/arcade");
    let catalog_dir = std::env::var_os("ARCADE_CATALOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| data.join("catalog"));
    Ok(Dirs {
        games: Store::open(data.join("games"))?,
        catalog: Store::open(catalog_dir.clone())?,
        catalog_dir,
        played: Played::open(data.join("last-played")),
    })
}

fn cards(d: &Dirs) -> Result<Vec<library::Card>> {
    Ok(library::build(
        d.games.list()?,
        d.catalog.list()?,
        &d.played,
    ))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The library loop. The terminal is handed to a game while it runs.
fn play(d: &mut Dirs) -> Result<()> {
    let hint = format!(
        "add games with arcade publish <file> (catalog at {}) or arcade install <file>",
        d.catalog_dir.display()
    );
    let mut lib = Library::new(cards(d)?, hint);
    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|f| lib.draw(f, now()))?;
            let Event::Key(k) = event::read()? else {
                continue;
            };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match lib.key(k.code) {
                Action::None => continue,
                Action::Quit => return Ok(()),
                Action::Play { id, path } => {
                    if let Err(e) = d.played.touch(&id, now()) {
                        lib.say(format!("{e:#}"), true);
                    }
                    ratatui::restore();
                    match Command::new(&path).status() {
                        Ok(s) if s.success() => {}
                        Ok(s) => lib.say(format!("{id} exited with {s}"), true),
                        Err(e) => lib.say(format!("cannot run {id}: {e}"), true),
                    }
                    terminal = ratatui::init();
                }
                Action::Get { info, src } => match d.games.add(&info, &src) {
                    Ok(()) => lib.say(format!("got {}", info.title.to_uppercase()), false),
                    Err(e) => lib.say(format!("{e:#}"), true),
                },
                Action::Remove(id) => match d.games.remove(&id) {
                    Ok(()) => lib.say(format!("removed {id}"), false),
                    Err(e) => lib.say(format!("{e:#}"), true),
                },
            }
            lib.replace(cards(d)?);
        }
    })();
    ratatui::restore();
    result
}
