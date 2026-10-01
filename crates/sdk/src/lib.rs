//! Everything an arcade game needs except its rules. A game crate is a binary
//! whose main is `arcade_sdk::run(&ENTRY)`. The SDK supplies the game menu with
//! its top 10, name entry, game over, the score table and the terminal.
//! See docs/adding-a-game.md.

mod app;
mod db;
mod game;
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
    terminal::supports_keyboard_enhancement,
};
use std::{io::stdout, time::Instant};

/// Runs one game until the player quits.
pub fn run(entry: &'static Entry) -> anyhow::Result<()> {
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
        terminal.draw(|f| ui::draw(f, &app))?;
        let now = Instant::now();
        if now >= next {
            app.on_timer(now)?;
            next = now + app.tick_rate();
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
