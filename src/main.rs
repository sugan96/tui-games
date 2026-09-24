use std::{collections::VecDeque, io, time::Duration};

use ratatui::{
    crossterm::event::{self, Event, KeyCode},
    style::Color,
    widgets::{
        canvas::{Canvas, Points},
        Block,
    },
};

const W: i32 = 40;
const H: i32 = 20;

struct Game {
    snake: VecDeque<(i32, i32)>,
    dir: (i32, i32),
    food: (i32, i32),
    over: bool,
}

impl Game {
    fn new() -> Self {
        let mut g = Game {
            snake: VecDeque::from([(W / 2, H / 2)]),
            dir: (1, 0),
            food: (0, 0),
            over: false,
        };
        g.spawn_food();
        g
    }

    fn spawn_food(&mut self) {
        loop {
            let p = (rand::random_range(0..W), rand::random_range(0..H));
            if !self.snake.contains(&p) {
                self.food = p;
                return;
            }
        }
    }

    fn step(&mut self) {
        let (hx, hy) = self.snake[0];
        let head = (hx + self.dir.0, hy + self.dir.1);
        let hit_wall = head.0 < 0 || head.0 >= W || head.1 < 0 || head.1 >= H;
        if hit_wall || self.snake.contains(&head) {
            self.over = true;
            return;
        }
        self.snake.push_front(head);
        if head == self.food {
            self.spawn_food();
        } else {
            self.snake.pop_back();
        }
    }

    fn turn(&mut self, d: (i32, i32)) {
        if d.0 != -self.dir.0 || d.1 != -self.dir.1 {
            self.dir = d;
        }
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut game = Game::new();

    loop {
        terminal.draw(|f| {
            let title = if game.over {
                format!(" GAME OVER  score {}  q quit ", game.snake.len() - 1)
            } else {
                format!(" snake  score {}  q quit ", game.snake.len() - 1)
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

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up => game.turn((0, 1)),
                    KeyCode::Down => game.turn((0, -1)),
                    KeyCode::Left => game.turn((-1, 0)),
                    KeyCode::Right => game.turn((1, 0)),
                    _ => {}
                }
            }
        }
        if !game.over {
            game.step();
        }
    }

    ratatui::restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eats_grows_and_dies_on_wall() {
        let mut g = Game::new();
        g.food = (W / 2 + 1, H / 2);
        g.step();
        assert_eq!(g.snake.len(), 2);
        g.turn((-1, 0)); // reverse ignored
        assert_eq!(g.dir, (1, 0));
        for _ in 0..W {
            g.step();
        }
        assert!(g.over);
    }
}
