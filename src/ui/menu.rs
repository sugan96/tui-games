use ratatui::{
    Frame,
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let mut lines: Vec<Line> = if app.top.is_empty() {
        vec!["no scores yet".into()]
    } else {
        app.top
            .iter()
            .enumerate()
            .map(|(i, r)| {
                format!("{:>2}. {}  {:>5}  {}", i + 1, r.name, r.score, r.mode.label()).into()
            })
            .collect()
    };
    lines.extend([
        "".into(),
        "1-9  fixed speed".into(),
        "v    variable speed".into(),
        "q    quit".into(),
        "".into(),
        "apple = current level in points".into(),
    ]);
    let p = Paragraph::new(lines).block(Block::bordered().title(" snake "));
    frame.render_widget(p, frame.area());
}
