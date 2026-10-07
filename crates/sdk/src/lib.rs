//! Everything an arcade game needs except its rules. A game crate is a binary
//! whose main is `arcade_sdk::run(&ENTRY)`. The SDK supplies the game menu with
//! its top 10, name entry, game over, the score table and the terminal.
//! See docs/adding-a-game.md.

mod app;
mod db;
mod game;
pub mod info;
mod input;
pub mod testkit;
pub mod ui;

pub use db::ScoreRow;
pub use game::{DrawCtx, Entry, Game, Outcome, Status};
pub use input::Input;

use app::App;
use db::Db;
use ratatui::crossterm::{
    event::{
        self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate, supports_keyboard_enhancement},
};
use std::{
    io::stdout,
    time::{Duration, Instant},
};

/// Runs one game until the player quits. With `--info` as the first argument,
/// prints the game's info for the launcher and exits instead.
pub fn run(entry: &'static Entry) -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--info") {
        print!("{}", info::Info::of(entry).to_text());
        return Ok(());
    }
    let db = Db::open_default()?; // before the terminal is touched
    let mut terminal = ratatui::init();
    // Key release events (kitty keyboard protocol) where the terminal has them.
    // Without them, held keys are detected from OS key repeat.
    let release_events = supports_keyboard_enhancement().unwrap_or(false);
    if release_events {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
        )?;
    }
    let result = run_loop(
        &mut terminal,
        App::new(db, entry, Input::new(release_events)),
    );
    if release_events {
        execute!(stdout(), PopKeyboardEnhancementFlags)?;
    }
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: anyhow::Result<App>,
) -> anyhow::Result<()> {
    let mut app = app?;
    // Deadline for the next tick, so input events do not push the tick back.
    let mut next = Instant::now() + app.tick_rate();
    while !app.quit {
        // Synchronized output: the terminal shows the frame only when complete.
        execute!(stdout(), BeginSynchronizedUpdate)?;
        terminal.draw(|f| ui::draw(f, &app))?;
        execute!(stdout(), EndSynchronizedUpdate)?;
        let now = Instant::now();
        if now >= next {
            app.on_timer(now)?;
            next = next_deadline(next, now, app.tick_rate());
        } else if event::poll(next - now)?
            && let Event::Key(k) = event::read()?
        {
            let was_playing = app.playing();
            app.on_key(k.code, k.kind, Instant::now())?;
            if !was_playing && app.playing() {
                next = Instant::now() + app.tick_rate();
            }
        }
    }
    Ok(())
}

/// The deadline after a tick that was due at prev and ran at now: prev + rate,
/// so lateness does not add to the period. When that has already passed, now +
/// rate instead, so two ticks never run without a draw between them.
pub fn next_deadline(prev: Instant, now: Instant, rate: Duration) -> Instant {
    let next = prev + rate;
    if next <= now { now + rate } else { next }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: Duration = Duration::from_millis(30);

    #[test]
    fn a_late_tick_advances_from_the_previous_deadline() {
        let prev = Instant::now();
        let now = prev + Duration::from_millis(7);
        assert_eq!(next_deadline(prev, now, RATE), prev + RATE);
    }

    #[test]
    fn a_passed_deadline_restarts_from_now() {
        let prev = Instant::now();
        let now = prev + Duration::from_millis(45);
        assert_eq!(next_deadline(prev, now, RATE), now + RATE);
        let now = prev + RATE; // exactly at the deadline
        assert_eq!(next_deadline(prev, now, RATE), now + RATE);
    }

    #[test]
    fn late_ticks_never_give_a_deadline_before_now() {
        let start = Instant::now();
        let mut next = start;
        for late in [20, 40, 5, 70, 29] {
            let now = next + Duration::from_millis(late);
            next = next_deadline(next, now, RATE);
            assert!(next > now, "{late} ms late");
        }
    }
}
