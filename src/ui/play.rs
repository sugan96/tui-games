use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
};

use super::{centered, font, panel, theme};
use crate::{
    app::{App, Screen},
    game::{H, W},
};

/// Each cell is two characters wide so it reads as square.
const CELL_W: u16 = 2;
const BOARD_W: u16 = W as u16 * CELL_W + 2;
const BOARD_H: u16 = H as u16 + 2;
const HUD_W: u16 = 24;
const TOTAL_W: u16 = BOARD_W + 1 + HUD_W;

const R: (i32, i32) = (1, 0);
const L: (i32, i32) = (-1, 0);
const U: (i32, i32) = (0, 1);

/// How the board is drawn: live play, the death blink, or dimmed under a modal.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Live,
    Dead { visible: bool },
    Dim,
}

pub fn draw(frame: &mut Frame, app: &App) {
    let look = match app.screen {
        Screen::Dying { left } => Look::Dead {
            visible: left % 2 == 1,
        },
        _ => Look::Live,
    };
    draw_board(frame, app, look);
}

/// Board and HUD. Modals call this with `Look::Dim` and draw on top.
pub fn draw_board(frame: &mut Frame, app: &App, look: Look) {
    let dim = look == Look::Dim;
    let area = centered(frame.area(), TOTAL_W, BOARD_H);
    let board = Rect::new(area.x, area.y, BOARD_W, BOARD_H);
    let hud = Rect::new(area.x + BOARD_W + 1, area.y, HUD_W, BOARD_H);
    let chrome = if dim {
        theme::CHROME_DIM
    } else {
        theme::CHROME
    };

    let block = panel(chrome, &[]).title_top(Line::styled(
        " snake ",
        if dim { theme::CHROME_DIM } else { theme::MUTED },
    ));
    let inner = block.inner(board);
    frame.render_widget(block, board);

    let hints: &[(&str, &str)] = if dim {
        &[]
    } else {
        &[("↑↓←→", ""), ("q", "quit")]
    };
    let hud_block = panel(chrome, hints);
    let hud_inner = hud_block.inner(hud);
    frame.render_widget(hud_block, hud);

    let buf = frame.buffer_mut();
    paint(buf, inner, app, look);
    paint_hud(buf, hud_inner, app, dim);
}

fn paint(buf: &mut Buffer, inner: Rect, app: &App, look: Look) {
    let game = &app.game;
    let dim = look == Look::Dim;
    let pos = |cell: (i32, i32)| {
        (
            inner.x + cell.0 as u16 * CELL_W,
            inner.y + (H - 1 - cell.1) as u16,
        )
    };
    let fg = |c: Color| Style::new().fg(c);

    for gy in 0..H {
        for gx in 0..W {
            let (x, y) = pos((gx, gy));
            let bg = if dim {
                theme::FIELD[0]
            } else {
                theme::FIELD[((gx + gy) % 2) as usize]
            };
            for dx in 0..CELL_W {
                if let Some(c) = buf.cell_mut((x + dx, y)) {
                    c.set_symbol(" ");
                    c.set_bg(bg);
                }
            }
        }
    }

    let food = match look {
        Look::Live => theme::FOOD[(app.ticks % 2) as usize],
        Look::Dead { .. } => theme::FOOD[0],
        Look::Dim => theme::DIM_C,
    };
    let (fx, fy) = pos(game.food);
    buf.set_string(fx, fy, "▟▙", fg(food));

    if matches!(look, Look::Dead { visible: false }) {
        return;
    }
    let n = game.snake.len();
    let color = |i: usize| match look {
        Look::Live if i == 0 => theme::HEAD,
        Look::Live => theme::BODY[((i - 1) * 4 / (n - 1).max(1)).min(3)],
        _ if i == 0 => theme::DEAD_HEAD,
        _ => theme::DEAD,
    };
    for i in 1..n {
        let (x, y) = pos(game.snake[i]);
        buf.set_string(x, y, "██", fg(color(i)));
    }

    let head = game.snake[0];
    let moving = if n >= 2 {
        dir(game.snake[1], head)
    } else {
        game.dir
    };
    let halfway = look == Look::Live && !app.settled && n >= 2 && !game.over;
    if !halfway {
        let (x, y) = pos(head);
        buf.set_string(x, y, head_glyph(moving), fg(color(0)));
        return;
    }
    // Halfway frame: the head has left the neck by half a cell and the tail
    // has left its old cell by half a cell.
    let (nx, ny) = pos(game.snake[1]);
    let (hx, hy) = pos(head);
    match moving {
        R => buf.set_string(nx + 1, ny, "█▶", fg(theme::HEAD)),
        L => buf.set_string(hx + 1, hy, "◀█", fg(theme::HEAD)),
        U => buf.set_string(hx, hy, "▄▄", fg(theme::HEAD)),
        _ => buf.set_string(hx, hy, "▀▀", fg(theme::HEAD)),
    }
    if let Some(old) = game.last_tail {
        let (ox, oy) = pos(old);
        let tail = fg(color(n - 1));
        match dir(old, game.snake[n - 1]) {
            R => buf.set_string(ox + 1, oy, "█", tail),
            L => buf.set_string(ox, oy, "█", tail),
            U => buf.set_string(ox, oy, "▀▀", tail),
            _ => buf.set_string(ox, oy, "▄▄", tail),
        }
    }
}

/// Unit direction from `a` to `b` in game coordinates, y up.
fn dir(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    (b.0 - a.0, b.1 - a.1)
}

/// Pointed head for the direction of travel.
fn head_glyph(moving: (i32, i32)) -> &'static str {
    match moving {
        R => "█▶",
        L => "◀█",
        U => "◢◣",
        _ => "◥◤",
    }
}

fn paint_hud(buf: &mut Buffer, inner: Rect, app: &App, dim: bool) {
    let game = &app.game;
    let s = |st: Style| if dim { theme::DIM } else { st };
    let (x, y) = (inner.x + 1, inner.y);
    let right_x = |text: &str| inner.x + inner.width - 1 - text.chars().count() as u16;

    buf.set_string(x, y, "1UP", s(theme::AMBER));
    let score = game.score.to_string();
    // ponytail: square-pixel digits fit up to 3 digits in the panel, plain text beyond.
    if score.len() <= 3 {
        for (i, row) in font::render_wide(&score).iter().enumerate() {
            buf.set_string(x + 1, y + 2 + i as u16, row, s(theme::ACCENT_HI));
        }
    } else {
        buf.set_string(x + 1, y + 2, &score, s(theme::ACCENT_HI));
    }

    buf.set_string(x, y + 8, "HI-SCORE", s(theme::AMBER));
    match app.top.first() {
        Some(r) => {
            let text = format!("{}  {}", r.score, r.name);
            buf.set_string(right_x(&text), y + 8, &text, s(theme::TEXT_BOLD));
            buf.set_string(right_x(&r.name), y + 8, &r.name, s(theme::MUTED));
        }
        None => buf.set_string(right_x("none yet"), y + 8, "none yet", s(theme::MUTED)),
    }

    buf.set_string(x, y + 11, "LEVEL", s(theme::MUTED));
    let level = format!("{} / 9", game.level());
    buf.set_string(right_x(&level), y + 11, &level, s(theme::TEXT_BOLD));
    for i in 0..9u16 {
        let on = i < game.level() as u16;
        buf.set_string(
            x + i * 2,
            y + 12,
            "▮",
            s(if on {
                theme::METER_ON
            } else {
                theme::METER_OFF
            }),
        );
    }

    buf.set_string(x, y + 14, "APPLES", s(theme::MUTED));
    let apples = format!("▟▙ {}", game.apples);
    buf.set_string(right_x(&apples), y + 14, &apples, s(theme::TEXT_BOLD));
    buf.set_string(right_x(&apples), y + 14, "▟▙", s(theme::FOOD_TEXT));

    buf.set_string(x, y + 15, "MODE", s(theme::MUTED));
    let mode = match game.mode {
        crate::game::Mode::Fixed(_) => "fixed",
        crate::game::Mode::Variable => "variable",
    };
    buf.set_string(right_x(mode), y + 15, mode, s(theme::TEXT));

    buf.set_string(x, y + 16, "SPEED", s(theme::MUTED));
    let speed = format!("{}ms", game.tick().as_millis());
    buf.set_string(right_x(&speed), y + 16, &speed, s(theme::TEXT));
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: (i32, i32) = (0, -1);

    #[test]
    fn head_points_where_it_moves() {
        assert_eq!(head_glyph(R), "█▶");
        assert_eq!(head_glyph(L), "◀█");
        assert_eq!(head_glyph(U), "◢◣");
        assert_eq!(head_glyph(D), "◥◤");
    }
}
