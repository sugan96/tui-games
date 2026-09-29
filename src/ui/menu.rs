use ratatui::{
    Frame,
    buffer::Buffer,
    style::{Color, Style},
};

use super::{centered, font, panel, theme};
use crate::{app::App, db::TOP_N};

const WIDTH: u16 = 58;
const HEIGHT: u16 = 28;
const RAMP: [&str; 9] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█", "█"];

pub fn draw(frame: &mut Frame, app: &App) {
    let area = centered(frame.area(), WIDTH, HEIGHT);
    frame.render_widget(
        panel(
            theme::CHROME,
            &[("1-9", "fixed speed"), ("v", "variable"), ("q", "quit")],
        ),
        area,
    );
    let buf = frame.buffer_mut();
    let (x, y) = (area.x, area.y);

    let logo = font::render("SNAKE");
    let lx = x + (WIDTH - logo[0].chars().count() as u16) / 2;
    for (i, row) in logo.iter().enumerate() {
        blit(buf, lx + 1, y + 3 + i as u16, row, theme::SHADOW);
    }
    for (i, row) in logo.iter().enumerate() {
        blit(buf, lx, y + 2 + i as u16, row, theme::LOGO[i]);
    }
    buf.set_string(
        x + 4,
        y + 8,
        "▀▄".repeat((WIDTH as usize - 8) / 2),
        theme::MARQUEE,
    );

    let tx = x + 8;
    buf.set_string(tx, y + 10, "RANK  NAME       SCORE   MODE", theme::MUTED);
    for i in 0..TOP_N {
        let row = y + 11 + i as u16;
        let Some(r) = app.top.get(i) else {
            buf.set_string(
                tx + 2,
                row,
                format!("{:>2}    ·  ·  ·", i + 1),
                theme::CHROME_DIM,
            );
            continue;
        };
        let (mark, style) = if i < 3 {
            ("★", theme::PODIUM[i])
        } else {
            (" ", theme::DIM)
        };
        buf.set_string(tx, row, format!("{mark} {:>2}", i + 1), style);
        buf.set_string(
            tx + 6,
            row,
            &r.name,
            if i < 3 { theme::TEXT_BOLD } else { theme::TEXT },
        );
        buf.set_string(
            tx + 15,
            row,
            format!("{:>7}", r.score),
            if i < 3 { style } else { theme::TEXT },
        );
        buf.set_string(tx + 25, row, &r.variant, theme::MUTED);
    }

    if app.ticks % 4 != 3 {
        let press = "PRESS 1-9 OR V TO START";
        buf.set_string(
            x + (WIDTH - press.len() as u16) / 2,
            y + 22,
            press,
            theme::AMBER,
        );
    }

    let sy = y + 24;
    buf.set_string(tx, sy, "SPEED", theme::MUTED);
    let mut cx = tx + 6;
    for (i, bar) in RAMP.iter().enumerate() {
        buf.set_string(cx, sy, format!(" {} ", i + 1), theme::CHIP);
        buf.set_string(cx + 1, sy + 1, bar, Style::new().fg(theme::SPEED_RAMP[i]));
        cx += 4;
    }
    buf.set_string(cx, sy, " v ", theme::CHIP);
    buf.set_string(cx + 3, sy, "auto", theme::MUTED);
    buf.set_string(tx + 6, sy + 2, "200ms", theme::CHROME_DIM);
    buf.set_string(tx + 38, sy + 2, "40ms", theme::CHROME_DIM);
}

/// Write the filled cells of a block-font row, leaving its gaps untouched.
fn blit(buf: &mut Buffer, x: u16, y: u16, row: &str, color: Color) {
    for (i, ch) in row.chars().enumerate() {
        if ch != ' ' {
            buf.set_string(x + i as u16, y, "█", Style::new().fg(color));
        }
    }
}
