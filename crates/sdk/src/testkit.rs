//! Test helpers for game crates. Call `check(&ENTRY)` from a test to verify
//! a game against the contract in `Game` and `Entry`.

use ratatui::{Frame, Terminal, backend::TestBackend, crossterm::event::KeyCode};

use crate::{DrawCtx, Entry, Game, Input, Status};

/// Draw into a `cols` by `rows` test terminal and return its text, one line per row.
pub fn render(cols: u16, rows: u16, draw: impl FnOnce(&mut Frame)) -> String {
    let mut t = Terminal::new(TestBackend::new(cols, rows)).unwrap();
    t.draw(draw).unwrap();
    t.backend()
        .buffer()
        .content()
        .chunks(cols as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Panics unless the entry is well formed and every start key gives a game
/// that runs to Over under a stream of keys within 100 000 ticks, drawing at
/// its minimum size, live and dimmed, along the way.
pub fn check(e: &Entry) {
    assert!(
        crate::info::valid_id(e.id),
        "id {:?} must be lower case letters and dashes",
        e.id
    );
    assert!(
        e.title.chars().all(|c| c.is_ascii_alphabetic() || c == ' '),
        "{} title must be letters and spaces",
        e.id
    );
    assert!(!e.about.contains('\n'), "{} about must be one line", e.id);
    assert!(!e.starts.is_empty(), "{} has no start hints", e.id);
    let keys: Vec<char> = (' '..='~')
        .filter(|&c| c != 'q' && (e.start)(c).is_some())
        .collect();
    assert!(!keys.is_empty(), "{} starts on no key", e.id);

    const KEYS: [KeyCode; 6] = [
        KeyCode::Up,
        KeyCode::Left,
        KeyCode::Down,
        KeyCode::Right,
        KeyCode::Char(' '),
        KeyCode::Enter,
    ];
    let input = Input::new(true);
    let (w, h) = e.min_size;
    for c in keys {
        let mut g: Box<dyn Game> = (e.start)(c).unwrap();
        let mut over = false;
        for i in 0..100_000 {
            assert!(!g.tick_rate().is_zero(), "{} {c}: zero tick rate", e.id);
            if i % 7 == 0 {
                g.key(KEYS[i / 7 % KEYS.len()]);
            }
            if i % 97 == 0 {
                for dim in [false, true] {
                    render(w, h, |f| g.draw(f, &DrawCtx { best: None, dim }));
                }
            }
            if let Status::Over(o) = g.tick(&input) {
                assert!(!o.variant.is_empty(), "{} {c}: empty variant", e.id);
                over = true;
                break;
            }
        }
        assert!(over, "{} {c}: never ended", e.id);
    }
}
