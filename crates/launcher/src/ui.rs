//! The library screen: a grid of game cards with thumbnails. Keys map to an
//! Action, and main does the work (play, get, remove) and hands back new cards.

use std::{cell::Cell, path::PathBuf};

use arcade_sdk::{
    info::Info,
    ui::{chips, theme, thumb, too_small},
};
use ratatui::{
    Frame,
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, BorderType},
};

use crate::library::Card;

const CARD_W: u16 = thumb::THUMB_W as u16 + 2;
/// Border, thumbnail, separator, title, status, border.
const CARD_H: u16 = thumb::THUMB_ROWS + 5;
const GAP_X: u16 = 2;
const GAP_Y: u16 = 1;
/// Panel border, one card, the about and message lines.
const MIN_W: u16 = CARD_W + 4;
const MIN_H: u16 = CARD_H + 6;

pub enum Action {
    None,
    Quit,
    Play { id: String, path: PathBuf },
    Get { info: Info, src: PathBuf },
    Remove(String),
}

pub struct Library {
    pub cards: Vec<Card>,
    pub sel: usize,
    /// One line under the grid, and whether it reports a failure.
    pub message: Option<(String, bool)>,
    /// True after the first x on a card: the second x removes it.
    confirm: bool,
    /// Shown when there are no cards: where games come from.
    empty_hint: String,
    /// Columns in the last drawn grid, so Up and Down move by a row.
    cols: Cell<usize>,
}

impl Library {
    pub fn new(cards: Vec<Card>, empty_hint: String) -> Library {
        Library {
            cards,
            sel: 0,
            message: None,
            confirm: false,
            empty_hint,
            cols: Cell::new(1),
        }
    }

    /// New cards after a play, get or remove. The selection follows the same game.
    pub fn replace(&mut self, cards: Vec<Card>) {
        let name = self.cards.get(self.sel).map(|c| c.name.clone());
        self.cards = cards;
        self.sel = name
            .and_then(|n| self.cards.iter().position(|c| c.name == n))
            .unwrap_or(self.sel)
            .min(self.cards.len().saturating_sub(1));
    }

    pub fn say(&mut self, text: impl Into<String>, failed: bool) {
        self.message = Some((text.into(), failed));
    }

    pub fn key(&mut self, key: KeyCode) -> Action {
        let confirming = std::mem::take(&mut self.confirm);
        self.message = None;
        let n = self.cards.len();
        let cols = self.cols.get().max(1);
        match key {
            KeyCode::Char('q') | KeyCode::Esc => return Action::Quit,
            KeyCode::Left | KeyCode::Char('h') => self.sel = self.sel.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') if self.sel + 1 < n => self.sel += 1,
            KeyCode::Up | KeyCode::Char('k') if self.sel >= cols => self.sel -= cols,
            KeyCode::Down | KeyCode::Char('j') if self.sel / cols < (n.max(1) - 1) / cols => {
                self.sel = (self.sel + cols).min(n - 1)
            }
            KeyCode::Enter => {
                let Some(c) = self.cards.get(self.sel) else {
                    return Action::None;
                };
                match (&c.info, &c.installed, &c.source) {
                    (Err(e), _, _) => self.say(format!("broken: {e}"), true),
                    (Ok(info), Some(path), _) => {
                        return Action::Play {
                            id: info.id.clone(),
                            path: path.clone(),
                        };
                    }
                    (Ok(info), None, Some(src)) => {
                        return Action::Get {
                            info: info.clone(),
                            src: src.clone(),
                        };
                    }
                    (Ok(_), None, None) => {}
                }
            }
            KeyCode::Char('x') => {
                let Some(c) = self.cards.get(self.sel) else {
                    return Action::None;
                };
                let Some(path) = &c.installed else {
                    self.say(format!("{} is not installed", c.name.to_uppercase()), true);
                    return Action::None;
                };
                // A broken file has no id, its file name is the install name.
                let id = c.id().map(String::from).unwrap_or_else(|| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into()
                });
                if confirming {
                    return Action::Remove(id);
                }
                self.confirm = true;
                self.say(
                    format!("remove {}? press x again", c.name.to_uppercase()),
                    true,
                );
            }
            _ => {}
        }
        Action::None
    }

    pub fn draw(&self, frame: &mut Frame, now: u64) {
        if too_small(frame, MIN_W, MIN_H) {
            return;
        }
        let area = frame.area();
        let block = Block::bordered()
            .border_type(BorderType::Double)
            .border_style(theme::CHROME)
            .title_top(Line::styled(" ▀▄ ARCADE ▄▀ ", theme::ACCENT_HI).centered())
            .title_bottom(
                chips(&[
                    ("←↑↓→", "move"),
                    ("Enter", "play / get"),
                    ("x", "remove"),
                    ("q", "quit"),
                ])
                .centered(),
            );
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let buf = frame.buffer_mut();

        let footer = inner.y + inner.height - 2;
        if let Some(c) = self.cards.get(self.sel) {
            let about = match &c.info {
                Ok(i) => i.about.clone(),
                Err(e) => format!("broken: {e}"),
            };
            put(
                buf,
                inner.x + 2,
                footer,
                &about,
                inner.width - 4,
                theme::MUTED,
            );
        }
        if let Some((m, failed)) = &self.message {
            let style = if *failed { theme::DANGER } else { theme::GOLD };
            put(buf, inner.x + 2, footer + 1, m, inner.width - 4, style);
        }
        if self.cards.is_empty() {
            put(
                buf,
                inner.x + 2,
                inner.y + 1,
                "no games yet",
                inner.width - 4,
                theme::TEXT_BOLD,
            );
            put(
                buf,
                inner.x + 2,
                inner.y + 3,
                &self.empty_hint,
                inner.width - 4,
                theme::MUTED,
            );
            return;
        }

        let cols = ((inner.width - 2 + GAP_X) / (CARD_W + GAP_X)).max(1) as usize;
        let rows = ((footer - inner.y - 1 + GAP_Y) / (CARD_H + GAP_Y)).max(1) as usize;
        self.cols.set(cols);
        let grid_w = cols as u16 * (CARD_W + GAP_X) - GAP_X;
        let gx = inner.x + (inner.width - grid_w) / 2;
        // Scroll by rows so the selected card stays in view.
        let first = (self.sel / cols).saturating_sub(rows - 1) * cols;
        for (i, c) in self.cards.iter().enumerate().skip(first).take(rows * cols) {
            let (col, row) = ((i - first) % cols, (i - first) / cols);
            let x = gx + col as u16 * (CARD_W + GAP_X);
            let y = inner.y + 1 + row as u16 * (CARD_H + GAP_Y);
            card(buf, Rect::new(x, y, CARD_W, CARD_H), c, i == self.sel, now);
        }
    }
}

fn card(buf: &mut Buffer, r: Rect, c: &Card, selected: bool, now: u64) {
    let border = if selected {
        theme::ACCENT_HI
    } else {
        theme::CHROME
    };
    let block = Block::bordered()
        .border_type(if selected {
            BorderType::Thick
        } else {
            BorderType::Rounded
        })
        .border_style(border);
    ratatui::widgets::Widget::render(block, r, buf);
    let (rows, initial): (&[String], char) = match &c.info {
        Ok(i) => (&i.thumb, i.title.chars().next().unwrap_or('?')),
        Err(_) => (&[], '?'),
    };
    thumb::draw(buf, r.x + 1, r.y + 1, rows, theme::FIELD[0], initial);
    let sep = r.y + 1 + thumb::THUMB_ROWS;
    let mid = "─".repeat(thumb::THUMB_W);
    let (l, rr) = if selected {
        ("┣", "┫")
    } else {
        ("├", "┤")
    };
    buf.set_string(r.x, sep, format!("{l}{mid}{rr}"), border);
    let title_style = if selected {
        theme::ACCENT_HI
    } else {
        theme::TEXT_BOLD
    };
    put(
        buf,
        r.x + 2,
        sep + 1,
        &c.name.to_uppercase(),
        CARD_W - 4,
        title_style,
    );
    let (text, style) = status(c, now);
    put(buf, r.x + 2, sep + 2, &text, CARD_W - 4, style);
}

/// The card's second line: broken, played when, installed, or get with size.
fn status(c: &Card, now: u64) -> (String, Style) {
    match (&c.info, &c.installed, c.played) {
        (Err(_), _, _) => ("✖ broken".into(), theme::DANGER),
        (Ok(_), Some(_), Some(at)) => (format!("▶ played {}", ago(at, now)), theme::METER_ON),
        (Ok(_), Some(_), None) => ("● installed".into(), theme::MUTED),
        (Ok(_), None, _) => match c.size {
            Some(bytes) => (format!("⇩ GET   {}", size(bytes)), theme::AMBER),
            None => ("⇩ GET".into(), theme::AMBER),
        },
    }
}

/// "just now", "5m ago", "3h ago", "2d ago".
fn ago(then: u64, now: u64) -> String {
    match now.saturating_sub(then) {
        s if s < 60 => "just now".into(),
        s if s < 3600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3600),
        s => format!("{}d ago", s / 86_400),
    }
}

/// "2.9 MB" or "640 KB".
fn size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else {
        format!("{} KB", bytes.div_ceil(1000))
    }
}

/// Write `text` cut to `width` characters.
fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, width: u16, style: Style) {
    let text: String = text.chars().take(width as usize).collect();
    buf.set_string(x, y, text, style);
}

#[cfg(test)]
mod tests {
    use arcade_sdk::testkit::render;

    use super::*;

    const NOW: u64 = 1_000_000;

    fn card(id: &str, installed: bool, played: Option<u64>) -> Card {
        Card {
            info: Ok(Info {
                id: id.into(),
                title: id.replace('-', " "),
                about: format!("about {id}"),
                thumb: vec!["rrrr".into(); 12],
            }),
            installed: installed.then(|| PathBuf::from(format!("/games/{id}"))),
            source: Some(PathBuf::from(format!("/cat/{id}"))),
            size: Some(2_900_000),
            played,
            name: id.replace('-', " "),
        }
    }

    fn broken() -> Card {
        Card {
            info: Err("--info exited with 3".into()),
            installed: Some(PathBuf::from("/games/junk")),
            source: None,
            size: None,
            played: None,
            name: "junk".into(),
        }
    }

    fn lib() -> Library {
        Library::new(
            vec![
                card("snake", true, Some(NOW - 7200)),
                card("tetris", true, None),
                card("brick-ball", false, None),
                broken(),
            ],
            "hint".into(),
        )
    }

    fn draw(l: &Library, w: u16, h: u16) -> String {
        render(w, h, |f| l.draw(f, NOW))
    }

    #[test]
    fn cards_show_title_status_thumb_and_about() {
        let s = draw(&lib(), 120, 40);
        for want in [
            "ARCADE",
            "SNAKE",
            "▶ played 2h ago",
            "● installed",
            "BRICK BALL",
            "⇩ GET   2.9 MB",
            "✖ broken",
            "about snake",
            "Enter  play / get",
        ] {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
        assert!(s.contains("▀▀▀▀"), "thumbnail drawn");
        assert!(
            s.matches("╭").count() == 3,
            "three unselected rounded cards\n{s}"
        );
    }

    #[test]
    fn grid_wraps_to_the_width_and_scrolls_to_the_selection() {
        let mut l = lib();
        let s = draw(&l, 60, 20); // 2 columns, 1 row
        assert_eq!(l.cols.get(), 2);
        assert!(s.contains("SNAKE") && s.contains("TETRIS") && !s.contains("BRICK"));
        l.key(KeyCode::Down);
        assert_eq!(l.sel, 2);
        let s = draw(&l, 60, 20);
        assert!(s.contains("BRICK") && !s.contains("SNAKE"), "{s}");
    }

    #[test]
    fn small_and_empty_states() {
        assert!(draw(&lib(), 20, 10).contains("terminal too small"));
        let s = draw(&Library::new(vec![], "put games in /cat".into()), 60, 20);
        assert!(
            s.contains("no games yet") && s.contains("put games in /cat"),
            "{s}"
        );
    }

    #[test]
    fn enter_plays_installed_gets_catalog_and_reports_broken() {
        let mut l = lib();
        let Action::Play { id, path } = l.key(KeyCode::Enter) else {
            panic!()
        };
        assert_eq!(
            (id.as_str(), path),
            ("snake", PathBuf::from("/games/snake"))
        );
        l.sel = 2;
        let Action::Get { src, .. } = l.key(KeyCode::Enter) else {
            panic!()
        };
        assert_eq!(src, PathBuf::from("/cat/brick-ball"));
        l.sel = 3;
        assert!(matches!(l.key(KeyCode::Enter), Action::None));
        assert!(l.message.as_ref().unwrap().0.contains("broken"));
    }

    #[test]
    fn remove_needs_a_second_x() {
        let mut l = lib();
        assert!(matches!(l.key(KeyCode::Char('x')), Action::None));
        assert!(l.message.as_ref().unwrap().0.contains("press x again"));
        assert!(
            matches!(l.key(KeyCode::Right), Action::None),
            "another key cancels"
        );
        l.sel = 0;
        l.key(KeyCode::Char('x'));
        let Action::Remove(id) = l.key(KeyCode::Char('x')) else {
            panic!()
        };
        assert_eq!(id, "snake");
        l.sel = 3;
        l.key(KeyCode::Char('x'));
        let Action::Remove(id) = l.key(KeyCode::Char('x')) else {
            panic!()
        };
        assert_eq!(id, "junk", "broken files remove by file name");
        l.sel = 2;
        assert!(matches!(l.key(KeyCode::Char('x')), Action::None));
        assert!(l.message.as_ref().unwrap().0.contains("not installed"));
    }

    #[test]
    fn moves_stay_in_the_grid_and_replace_keeps_the_game() {
        let mut l = lib();
        l.cols.set(3);
        l.key(KeyCode::Left);
        assert_eq!(l.sel, 0);
        l.key(KeyCode::Up);
        assert_eq!(l.sel, 0);
        l.key(KeyCode::Down);
        assert_eq!(l.sel, 3);
        l.key(KeyCode::Down);
        assert_eq!(l.sel, 3, "no row below");
        l.key(KeyCode::Right);
        assert_eq!(l.sel, 3, "last card");
        l.sel = 1;
        l.replace(vec![
            card("brick-ball", true, None),
            card("tetris", true, Some(1)),
        ]);
        assert_eq!(l.sel, 1, "tetris moved but stays selected");
        assert!(matches!(l.key(KeyCode::Char('q')), Action::Quit));
    }

    #[test]
    fn ago_and_size_read_naturally() {
        assert_eq!(ago(NOW - 5, NOW), "just now");
        assert_eq!(ago(NOW - 300, NOW), "5m ago");
        assert_eq!(ago(NOW - 2 * 86_400, NOW), "2d ago");
        assert_eq!(size(2_900_000), "2.9 MB");
        assert_eq!(size(640_000), "640 KB");
    }
}
