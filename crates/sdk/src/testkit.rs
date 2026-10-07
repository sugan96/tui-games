//! Test helpers for game crates. Call `check(&ENTRY)` from a test to verify
//! a game against the contract in `Game` and `Entry`.

use ratatui::{Frame, Terminal, backend::TestBackend, crossterm::event::KeyCode};

use crate::{DrawCtx, Entry, Input, Status};

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

/// Stage a game with stages is also started at, to cover its later stages.
const CHECK_STAGE: u32 = 20;

/// Panics unless the entry is well formed and every start key gives a game
/// that runs to Over under a stream of keys within 100 000 ticks, drawing at
/// its minimum size, live and dimmed, along the way. A game with stages runs
/// from stage 1 and from `CHECK_STAGE`, and must report at least its start
/// stage as reached. An outcome from `give_up` on a running game must have
/// a variant and a summary.
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
    {
        use crate::ui::thumb::{THUMB_H, THUMB_W, is_palette};
        assert_eq!(
            e.thumb.len(),
            THUMB_H,
            "{} thumb needs {THUMB_H} rows",
            e.id
        );
        for (i, row) in e.thumb.iter().enumerate() {
            assert_eq!(
                row.chars().count(),
                THUMB_W,
                "{} thumb row {i} needs {THUMB_W} letters",
                e.id
            );
            if let Some(c) = row.chars().find(|&c| !is_palette(c)) {
                panic!("{} thumb row {i} has {c:?}, not a palette letter", e.id);
            }
        }
    }
    let keys: Vec<char> = (' '..='~')
        .chain(['\n'])
        .filter(|&c| c != 'q' && (e.start)(c, 1).is_some())
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
    let input = Input::new(true, false);
    let (w, h) = e.min_size;
    let stages: &[u32] = if e.stages { &[1, CHECK_STAGE] } else { &[1] };
    for c in keys {
        for &stage in stages {
            let Some(mut g) = (e.start)(c, stage) else {
                panic!("{} {c:?}: no game at stage {stage}", e.id);
            };
            if let Some(o) = g.give_up() {
                assert!(
                    !o.variant.is_empty(),
                    "{} {c:?}: give_up without variant",
                    e.id
                );
                assert!(
                    !o.summary.is_empty(),
                    "{} {c:?}: give_up without summary",
                    e.id
                );
            }
            let mut over = false;
            for i in 0..100_000 {
                assert!(!g.tick_rate().is_zero(), "{} {c:?}: zero tick rate", e.id);
                if i % 7 == 0 {
                    g.key(KEYS[i / 7 % KEYS.len()]);
                }
                if i % 97 == 0 {
                    for dim in [false, true] {
                        render(w, h, |f| g.draw(f, &DrawCtx { best: None, dim }));
                    }
                }
                if let Status::Over(o) = g.tick(&input) {
                    assert!(!o.variant.is_empty(), "{} {c:?}: empty variant", e.id);
                    if e.stages {
                        assert!(
                            g.reached() >= stage,
                            "{} {c:?}: reached {} below start stage {stage}",
                            e.id,
                            g.reached()
                        );
                    }
                    over = true;
                    break;
                }
            }
            assert!(over, "{} {c:?} at stage {stage}: never ended", e.id);
        }
    }
}
