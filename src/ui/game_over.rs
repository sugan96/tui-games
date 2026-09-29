use ratatui::{Frame, text::Span, widgets::Clear};

use super::{center_line, centered, font, modal, play, theme};
use crate::app::{App, Screen};

const WIDTH: u16 = 46;
const HEIGHT: u16 = 12;

pub fn draw(frame: &mut Frame, app: &App) {
    play::draw_board(frame, app, play::Look::Dim);
    let Screen::GameOver { ranked } = app.screen else {
        return;
    };
    let g = &app.game;
    let area = centered(frame.area(), WIDTH, HEIGHT);
    let block = modal(
        "GAME OVER",
        theme::ACCENT_HI,
        &[("r", "menu"), ("q", "quit")],
    );
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let digits = font::render_wide(&g.score.to_string());
    let dx = inner.x + (inner.width - digits[0].chars().count() as u16) / 2;
    for (i, row) in digits.iter().enumerate() {
        frame
            .buffer_mut()
            .set_string(dx, inner.y + 1 + i as u16, row, theme::ACCENT_HI);
    }

    let sep = Span::styled("   ·   ", theme::CHROME_DIM);
    let mut spans = Vec::new();
    if ranked {
        spans.push(Span::styled("★ new best", theme::GOLD));
        spans.push(sep.clone());
    }
    spans.push(Span::styled(format!("▟▙ {} apples", g.apples), theme::TEXT));
    spans.push(sep);
    spans.push(Span::styled(g.mode.label(), theme::MUTED));
    center_line(frame, inner, inner.y + 8, spans);
}
