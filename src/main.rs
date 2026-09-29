mod app;
mod db;
mod game;
mod input;
mod ui;

use app::{App, Screen};
use db::Db;
use ratatui::crossterm::{
    event::{
        self, Event, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::supports_keyboard_enhancement,
};
use std::{io::stdout, time::Instant};

fn main() -> anyhow::Result<()> {
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
    let result = run(&mut terminal, db, release_events);
    if release_events {
        execute!(stdout(), PopKeyboardEnhancementFlags)?;
    }
    ratatui::restore();
    result
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    db: Db,
    release_events: bool,
) -> anyhow::Result<()> {
    let mut app = App::new(db)?;
    app.input = input::Input::new(release_events);
    // Deadline for the next step, so input events do not push the step back.
    let mut next = Instant::now() + app.tick_rate();
    while !app.quit {
        app.refresh_boost(Instant::now());
        terminal.draw(|f| ui::draw(f, &app))?;
        let now = Instant::now();
        if now >= next {
            app.on_timer()?;
            next = now + app.tick_rate();
        } else if event::poll(next - now)?
            && let Event::Key(k) = event::read()?
        {
            app.track_hold(k.code, k.kind, Instant::now());
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let was_playing = matches!(app.screen, Screen::Playing);
            app.handle_key(k.code)?;
            if !was_playing && matches!(app.screen, Screen::Playing) {
                next = Instant::now() + app.tick_rate();
            }
        }
    }
    Ok(())
}
