//! The game library: installed games and catalog games merged by id, in the
//! order the launcher shows them.

use std::{cmp::Reverse, collections::HashMap, path::PathBuf};

use arcade_sdk::info::Info;

use crate::{played::Played, store::Installed};

pub struct Card {
    /// Info of the installed copy, else of the catalog copy. Err for an
    /// installed file that does not answer --info, with the reason.
    pub info: Result<Info, String>,
    /// Where the installed copy is. None when it is only in the catalog.
    pub installed: Option<PathBuf>,
    /// Where the catalog copy is, for Get.
    pub source: Option<PathBuf>,
    /// Size of the catalog copy in bytes, shown on Get.
    pub size: Option<u64>,
    /// Last start from the launcher, unix seconds.
    pub played: Option<u64>,
    /// Sort and display fallback: the title, or the file name of a broken game.
    pub name: String,
}

impl Card {
    pub fn id(&self) -> Option<&str> {
        self.info.as_ref().ok().map(|i| i.id.as_str())
    }

    /// 0 installed and played, 1 installed, 2 only in the catalog.
    fn tier(&self) -> u8 {
        match (&self.installed, self.played) {
            (Some(_), Some(_)) => 0,
            (Some(_), None) => 1,
            (None, _) => 2,
        }
    }
}

/// Merges the two lists by id. Catalog files that are not games are dropped,
/// broken installed files are kept so the player can remove them.
/// Order: played games most recent first, then installed, then catalog only,
/// A to Z inside each tier.
pub fn build(installed: Vec<Installed>, catalog: Vec<Installed>, played: &Played) -> Vec<Card> {
    let mut cards: Vec<Card> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for g in installed {
        let name = match &g.info {
            Ok(i) => i.title.clone(),
            Err(_) => g
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
        };
        if let Ok(i) = &g.info {
            by_id.insert(i.id.clone(), cards.len());
        }
        cards.push(Card {
            played: g
                .info
                .as_ref()
                .ok()
                .and_then(|i| played.at.get(&i.id).copied()),
            info: g.info,
            installed: Some(g.path),
            source: None,
            size: None,
            name,
        });
    }
    for g in catalog {
        let Ok(info) = g.info else { continue };
        let size = std::fs::metadata(&g.path).ok().map(|m| m.len());
        match by_id.get(&info.id) {
            Some(&i) => {
                cards[i].source = Some(g.path);
                cards[i].size = size;
            }
            None => {
                by_id.insert(info.id.clone(), cards.len());
                cards.push(Card {
                    played: played.at.get(&info.id).copied(),
                    name: info.title.clone(),
                    info: Ok(info),
                    installed: None,
                    source: Some(g.path),
                    size,
                });
            }
        }
    }
    // Play time orders only the played tier. The others are A to Z.
    cards.sort_by_key(|c| {
        let tier = c.tier();
        let played = if tier == 0 { c.played } else { None };
        (tier, Reverse(played), c.name.to_lowercase())
    });
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: &str, dir: &str) -> Installed {
        Installed {
            path: PathBuf::from(format!("/{dir}/{id}")),
            info: Ok(Info {
                id: id.into(),
                title: id.replace('-', " "),
                about: String::new(),
                thumb: vec![],
            }),
        }
    }

    fn played(at: &[(&str, u64)]) -> Played {
        let mut p = Played::open(PathBuf::from("/nonexistent/last-played"));
        p.at = at.iter().map(|(id, s)| (id.to_string(), *s)).collect();
        p
    }

    fn order(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn played_then_installed_then_catalog_each_a_to_z() {
        let installed = vec![
            game("zork", "games"),
            game("snake", "games"),
            game("tetris", "games"),
            game("asteroids", "games"),
        ];
        let catalog = vec![
            game("pong", "cat"),
            game("snake", "cat"),
            game("brick-ball", "cat"),
        ];
        let cards = build(
            installed,
            catalog,
            &played(&[("tetris", 10), ("snake", 50), ("pong", 99)]),
        );
        assert_eq!(
            order(&cards),
            ["snake", "tetris", "asteroids", "zork", "brick ball", "pong"],
            "pong was played but is not installed, so it sorts with the catalog"
        );
        let snake = &cards[0];
        assert_eq!(snake.installed, Some(PathBuf::from("/games/snake")));
        assert_eq!(
            snake.source,
            Some(PathBuf::from("/cat/snake")),
            "merged by id"
        );
        assert!(cards[5].installed.is_none() && cards[5].source.is_some());
    }

    #[test]
    fn broken_installed_kept_and_broken_catalog_dropped() {
        let broken = |dir: &str| Installed {
            path: PathBuf::from(format!("/{dir}/junk")),
            info: Err("--info exited with 3".into()),
        };
        let cards = build(vec![broken("games")], vec![broken("cat")], &played(&[]));
        assert_eq!(order(&cards), ["junk"]);
        assert!(cards[0].id().is_none());
    }
}
