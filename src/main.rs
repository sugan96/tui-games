mod app;
mod db;
mod game;
mod ui;

use app::App;
use db::Db;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

fn main() -> anyhow::Result<()> {
    let db = Db::open_default()?; // before the terminal is touched
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, db);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, db: Db) -> anyhow::Result<()> {
    let mut app = App::new(db)?;
    while !app.quit {
        terminal.draw(|f| ui::draw(f, &app))?;
        if event::poll(app.tick_rate())? {
            if let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
            {
                app.handle_key(k.code)?;
            }
        } else {
            app.on_tick()?;
        }
    }
    Ok(())
}
