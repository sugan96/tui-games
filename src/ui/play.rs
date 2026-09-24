use ratatui::{
    Frame,
    style::Color,
    widgets::{
        Block,
        canvas::{Canvas, Points},
    },
};

use crate::{
    app::App,
    game::{H, W},
};

pub fn draw(frame: &mut Frame, app: &App) {
    let game = &app.game;
    let title = format!(
        " {}  level {}  apples {}  score {}  q quit ",
        game.mode.label(),
        game.level(),
        game.apples,
        game.score
    );
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
    frame.render_widget(canvas, frame.area());
}
