mod app;
mod db;
mod game;
mod ui;

use app::{App, Screen};
use db::Db;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let db = Db::open_default()?; // before the terminal is touched
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, db);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, db: Db) -> anyhow::Result<()> {
    let mut app = App::new(db)?;
    // Deadline for the next step, so input events do not push the step back.
    let mut next = Instant::now() + app.tick_rate();
    while !app.quit {
        terminal.draw(|f| ui::draw(f, &app))?;
        let now = Instant::now();
        if now >= next {
            app.on_tick()?;
            next = now + app.tick_rate();
        } else if event::poll(next - now)?
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
        {
            let was_playing = matches!(app.screen, Screen::Playing);
            app.handle_key(k.code)?;
            if !was_playing && matches!(app.screen, Screen::Playing) {
                next = Instant::now() + app.tick_rate();
            }
        }
    }
    Ok(())
}
