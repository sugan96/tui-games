#[allow(dead_code)] // wired into the UI in a later task
mod db;
mod game;

use std::io;

use game::{Game, Mode, H, W};
use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    style::Color,
    widgets::{
        canvas::{Canvas, Points},
        Block,
    },
};

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut game = Game::new(Mode::Fixed(5));

    loop {
        terminal.draw(|f| {
            let status = format!(
                "{}  level {}  apples {}  score {}",
                game.mode.label(),
                game.level(),
                game.apples,
                game.score
            );
            let title = if game.over {
                format!(" GAME OVER  {status}  q quit ")
            } else {
                format!(" snake  {status}  q quit ")
            };
            let canvas = Canvas::default()
                .block(Block::bordered().title(title))
                .x_bounds([0.0, W as f64])
                .y_bounds([0.0, H as f64])
                .marker(ratatui::symbols::Marker::Block)
                .paint(|ctx| {
                    let body: Vec<(f64, f64)> =
                        game.snake.iter().map(|&(x, y)| (x as f64, y as f64)).collect();
                    ctx.draw(&Points { coords: &body, color: Color::Green });
                    let food = [(game.food.0 as f64, game.food.1 as f64)];
                    ctx.draw(&Points { coords: &food, color: Color::Red });
                });
            f.render_widget(canvas, f.area());
        })?;

        if event::poll(game.tick())?
            && let Event::Key(k) = event::read()?
        {
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up => game.turn((0, 1)),
                KeyCode::Down => game.turn((0, -1)),
                KeyCode::Left => game.turn((-1, 0)),
                KeyCode::Right => game.turn((1, 0)),
                _ => {}
            }
        }
        if !game.over {
            game.step();
        }
    }

    ratatui::restore();
    Ok(())
}
