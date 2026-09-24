use ratatui::{
    Frame,
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let g = &app.game;
    let lines: Vec<Line> = vec![
        format!("score {}", g.score).into(),
        format!("apples {}", g.apples).into(),
        g.mode.label().into(),
        "".into(),
        "r  menu".into(),
        "q  quit".into(),
    ];
    let p = Paragraph::new(lines).block(Block::bordered().title(" game over "));
    frame.render_widget(p, frame.area());
}
