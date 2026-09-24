use ratatui::{
    Frame,
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::app::{App, NAME_LEN, Screen};

pub fn draw(frame: &mut Frame, app: &App) {
    let Screen::NameEntry { name, .. } = &app.screen else {
        return;
    };
    let lines: Vec<Line> = vec![
        "enter your name".into(),
        format!("{name:_<NAME_LEN$}").into(),
        "".into(),
        "Enter to confirm".into(),
    ];
    let p = Paragraph::new(lines).block(Block::bordered().title(" high score "));
    frame.render_widget(p, frame.area());
}
