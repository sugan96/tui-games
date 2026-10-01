use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
};

use super::{
    Snake,
    board::{H, Mode, W},
};
use arcade_sdk::{
    DrawCtx,
    ui::{centered, font, panel, theme},
};

/// Each cell is two characters wide so it reads as square.
const CELL_W: u16 = 2;
pub const BOARD_W: u16 = W as u16 * CELL_W + 2;
pub const BOARD_H: u16 = H as u16 + 2;
const HUD_W: u16 = 24;
pub const TOTAL_W: u16 = BOARD_W + 1 + HUD_W;

const R: (i32, i32) = (1, 0);
const L: (i32, i32) = (-1, 0);
const U: (i32, i32) = (0, 1);

/// How the board is drawn: live play, the death blink, or dimmed under a modal.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Live,
    Dead { visible: bool },
    Dim,
}

/// Board and HUD, centered in the frame.
pub fn draw(frame: &mut Frame, snake: &Snake, ctx: &DrawCtx) {
    let look = match (ctx.dim, snake.dying) {
        (true, _) => Look::Dim,
        (false, Some(left)) => Look::Dead {
            visible: left % 2 == 1,
        },
        (false, None) => Look::Live,
    };
    let dim = ctx.dim;
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
    paint(buf, inner, snake, look);
    paint_hud(buf, hud_inner, snake, ctx);
}

fn paint(buf: &mut Buffer, inner: Rect, snake: &Snake, look: Look) {
    let game = &snake.board;
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
        Look::Live => theme::FOOD[(snake.steps % 2) as usize],
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
    let halfway = look == Look::Live && !snake.settled && n >= 2 && !game.over;
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

fn paint_hud(buf: &mut Buffer, inner: Rect, snake: &Snake, ctx: &DrawCtx) {
    let game = &snake.board;
    let dim = ctx.dim;
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
    match ctx.best {
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
        Mode::Fixed(_) => "fixed",
        Mode::Variable => "variable",
    };
    buf.set_string(right_x(mode), y + 15, mode, s(theme::TEXT));

    buf.set_string(x, y + 16, "SPEED", s(theme::MUTED));
    let speed = format!("{}ms", snake.step_time().as_millis());
    buf.set_string(right_x(&speed), y + 16, &speed, s(theme::TEXT));
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcade_sdk::{ScoreRow, testkit::render as render_with};

    const D: (i32, i32) = (0, -1);
    // Board is 107 wide by 22 tall centered in 120 by 32: inner origin (7, 6).
    const IX: usize = 7;
    const IY: usize = 6;

    /// Head at (22, 11) moving right, body back and down to (17, 9), tail just
    /// left (16, 9). Food at (30, 14).
    fn playing() -> Snake {
        let mut s = Snake::new(Mode::Fixed(3));
        let b = &mut s.board;
        b.apples = 4;
        b.score = 12;
        b.snake = [
            (22, 11),
            (21, 11),
            (20, 11),
            (19, 11),
            (18, 11),
            (18, 10),
            (18, 9),
            (17, 9),
        ]
        .into();
        b.last_tail = Some((16, 9));
        b.food = (30, 14);
        s
    }

    fn render(s: &Snake) -> String {
        let best = ScoreRow {
            name: "ABC".into(),
            score: 42,
            variant: "fixed 5".into(),
        };
        let ctx = DrawCtx {
            best: Some(&best),
            dim: false,
        };
        render_with(120, 32, |f| draw(f, s, &ctx))
    }

    /// Character at board cell `(gx, gy)`, column `dx` of the two.
    fn cell(s: &str, gx: usize, gy: usize, dx: usize) -> char {
        s.lines()
            .nth(IY + 19 - gy)
            .unwrap()
            .chars()
            .nth(IX + gx * 2 + dx)
            .unwrap()
    }

    #[test]
    fn head_points_where_it_moves() {
        assert_eq!(head_glyph(R), "█▶");
        assert_eq!(head_glyph(L), "◀█");
        assert_eq!(head_glyph(U), "◢◣");
        assert_eq!(head_glyph(D), "◥◤");
    }

    #[test]
    fn settled_draws_pointed_head_food_and_hud() {
        let s = render(&playing());
        assert_eq!(
            (cell(&s, 22, 11, 0), cell(&s, 22, 11, 1)),
            ('█', '▶'),
            "{s}"
        );
        assert_eq!(cell(&s, 21, 11, 1), '█');
        assert_eq!(cell(&s, 17, 9, 0), '█');
        assert_eq!(
            cell(&s, 16, 9, 1),
            ' ',
            "old tail cell is empty once settled"
        );
        assert_eq!((cell(&s, 30, 14, 0), cell(&s, 30, 14, 1)), ('▟', '▙'));
        for want in [
            "1UP",
            "    ██ ██████",
            "HI-SCORE     42  ABC",
            "LEVEL          3 / 9",
            "▮ ▮ ▮ ▮ ▮ ▮ ▮ ▮ ▮",
            "APPLES          ▟▙ 4",
            "MODE           fixed",
            "SPEED          160ms",
            "↑↓←→    q  quit",
        ] {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
    }

    #[test]
    fn halfway_shifts_head_and_tail_by_one_column() {
        let mut g = playing();
        g.settled = false;
        let s = render(&g);
        assert_eq!(cell(&s, 21, 11, 1), '█', "{s}");
        assert_eq!(cell(&s, 22, 11, 0), '▶');
        assert_eq!(cell(&s, 22, 11, 1), ' ');
        assert_eq!(cell(&s, 16, 9, 0), ' ');
        assert_eq!(cell(&s, 16, 9, 1), '█');
    }

    #[test]
    fn halfway_vertical_uses_half_blocks() {
        let mut g = playing();
        g.settled = false;
        g.board.snake = [(5, 6), (5, 5), (5, 4)].into();
        g.board.last_tail = Some((5, 3));
        let s = render(&g);
        assert_eq!((cell(&s, 5, 6, 0), cell(&s, 5, 6, 1)), ('▄', '▄'), "{s}");
        assert_eq!((cell(&s, 5, 3, 0), cell(&s, 5, 3, 1)), ('▀', '▀'));
    }

    #[test]
    fn dying_blinks_the_snake() {
        let mut g = playing();
        g.dying = Some(5);
        let s = render(&g);
        assert_eq!(cell(&s, 22, 11, 1), '▶', "{s}");
        g.dying = Some(4);
        let s = render(&g);
        assert_eq!(cell(&s, 22, 11, 1), ' ', "{s}");
        assert_eq!(cell(&s, 18, 10, 0), ' ');
        assert_eq!(cell(&s, 30, 14, 0), '▟', "food stays");
    }
}
