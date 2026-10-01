//! How a game binary describes itself to the launcher: `<game> --info` prints
//! `key=value` lines and exits. The launcher reads them on install and at start.

use anyhow::{Context, Result, bail};

use crate::{
    Entry,
    ui::thumb::{THUMB_H, THUMB_W},
};

/// Bump when the launcher can no longer run games built against the old SDK.
pub const PROTOCOL: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub id: String,
    pub title: String,
    pub about: String,
    /// Thumbnail rows, at most THUMB_H of at most THUMB_W letters. Empty when the game has none.
    pub thumb: Vec<String>,
}

impl Info {
    pub fn of(e: &Entry) -> Info {
        Info {
            id: e.id.into(),
            title: e.title.into(),
            about: e.about.into(),
            thumb: e.thumb.iter().map(|r| r.to_string()).collect(),
        }
    }

    /// One `thumb=` line per thumbnail row, in order.
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "protocol={PROTOCOL}\nid={}\ntitle={}\nabout={}\n",
            self.id, self.title, self.about
        );
        for row in &self.thumb {
            text.push_str(&format!("thumb={row}\n"));
        }
        text
    }

    /// Inverse of to_text. Fails on another protocol version, a missing key, or an
    /// id that is not lower case letters and dashes. The launcher uses the id as a
    /// file name, so this check keeps a game from writing outside its folder.
    /// Unknown keys are ignored so newer games can add fields.
    pub fn parse(text: &str) -> Result<Info> {
        let get = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
                .with_context(|| format!("game info has no {key}"))
        };
        let protocol = get("protocol")?;
        if protocol != PROTOCOL.to_string() {
            bail!("game uses protocol {protocol}, this launcher speaks {PROTOCOL}");
        }
        let id = get("id")?;
        if !valid_id(id) {
            bail!("game id {id:?} must be lower case letters and dashes");
        }
        Ok(Info {
            id: id.into(),
            title: get("title")?.into(),
            about: get("about")?.into(),
            thumb: text
                .lines()
                .filter_map(|l| l.strip_prefix("thumb="))
                .take(THUMB_H)
                .map(|r| r.chars().take(THUMB_W).collect())
                .collect(),
        })
    }
}

/// Lower case ASCII letters and dashes, not empty, not starting with a dash.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && !id.starts_with('-') && id.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> Info {
        Info {
            id: "snake".into(),
            title: "snake".into(),
            about: "an apple = level points".into(),
            thumb: vec!["r.g".into(), "..y".into()],
        }
    }

    #[test]
    fn round_trips_and_ignores_unknown_keys() {
        let text = info().to_text();
        assert_eq!(Info::parse(&text).unwrap(), info());
        assert_eq!(Info::parse(&format!("extra=1\n{text}")).unwrap(), info());
    }

    #[test]
    fn thumb_is_optional_and_clipped_to_size() {
        let text = info().to_text();
        let bare: String = text
            .lines()
            .filter(|l| !l.starts_with("thumb="))
            .map(|l| format!("{l}\n"))
            .collect();
        assert!(Info::parse(&bare).unwrap().thumb.is_empty());
        let big = format!("{bare}{}", format!("thumb={}\n", "g".repeat(99)).repeat(99));
        let thumb = Info::parse(&big).unwrap().thumb;
        assert_eq!(thumb.len(), THUMB_H);
        assert!(thumb.iter().all(|r| r.chars().count() == THUMB_W));
    }

    #[test]
    fn rejects_other_protocols_and_missing_keys() {
        let text = info().to_text().replace("protocol=1", "protocol=2");
        assert!(
            Info::parse(&text)
                .unwrap_err()
                .to_string()
                .contains("protocol 2")
        );
        assert!(Info::parse("protocol=1\nid=x\n").is_err());
        for bad in ["../x", "a/b", "", "-x", "Snake"] {
            let text = info().to_text().replace("id=snake", &format!("id={bad}"));
            assert!(Info::parse(&text).is_err(), "{bad:?} accepted");
        }
    }
}
