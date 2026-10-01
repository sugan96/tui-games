use ratatui::{Frame, text::Span, widgets::Clear};

use super::{center_line, centered, modal, theme};
use crate::app::{App, NAME_LEN};

const WIDTH: u16 = 46;
const HEIGHT: u16 = 12;

/// Modal over the dimmed game. `app.top` gives the rank the score will take.
pub fn draw(frame: &mut Frame, app: &App, score: u32, name: &str) {
    let area = centered(frame.area(), WIDTH, HEIGHT);
    let block = modal(
        "NEW HIGH SCORE",
        theme::GOLD,
        &[("A-Z", "type"), ("Bksp", ""), ("Enter", "confirm")],
    );
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let rank = app.top.iter().filter(|r| r.score >= score).count() + 1;
    center_line(
        frame,
        inner,
        inner.y + 1,
        vec![
            Span::styled(format!("score {score}"), theme::MUTED),
            Span::styled("   ·   ", theme::CHROME_DIM),
            Span::styled(format!("rank {rank}"), theme::MUTED),
        ],
    );

    let letters: Vec<char> = name.chars().collect();
    let bx = inner.x + (inner.width - (NAME_LEN as u16 * 6 - 1)) / 2;
    let buf = frame.buffer_mut();
    for i in 0..NAME_LEN {
        let x = bx + i as u16 * 6;
        let y = inner.y + 3;
        let border = if i == letters.len() {
            theme::ACCENT
        } else {
            theme::CHROME
        };
        buf.set_string(x, y, "┌───┐", border);
        buf.set_string(x, y + 1, "│", border);
        buf.set_string(x + 4, y + 1, "│", border);
        buf.set_string(x, y + 2, "└───┘", border);
        match letters.get(i) {
            Some(c) => buf.set_string(x + 2, y + 1, c.to_string(), theme::TEXT_BOLD),
            None => buf.set_string(x + 2, y + 1, "_", theme::ACCENT_HI),
        }
    }
    center_line(
        frame,
        inner,
        inner.y + 7,
        vec![Span::styled(
            "three letters, stored upper case",
            theme::CHROME_DIM,
        )],
    );
}
