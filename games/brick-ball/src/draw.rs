use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Clear,
};

use super::{
    BrickBall,
    board::{self, Board, Phase, Power},
    stage::{self, Cell},
};
use arcade_sdk::{
    DrawCtx,
    ui::{self, theme},
};

pub const BOARD_W: u16 = 62; // 60 cells + border
pub const BOARD_H: u16 = 24; // 22 cells + border
pub const HUD_W: u16 = 16; // the spec's 16 columns, border included
pub const TOTAL_W: u16 = BOARD_W + 1 + HUD_W; // 79

/// Board size in pixels.
const PW: usize = board::W as usize;
const PH: usize = board::H as usize;

/// Light and dark color of a brick for HP 1 to 5.
const HP_COLORS: [(u8, u8); 5] = [(223, 180), (215, 173), (209, 167), (167, 131), (124, 88)];
const UNBREAKABLE: (u8, u8) = (245, 239);
const PADDLE: u8 = 173;
const BALL: u8 = 255;

const fn c(i: u8) -> Color {
    Color::Indexed(i)
}

/// Chip color of a capsule and of its HUD timer.
fn power_color(p: Power) -> Color {
    match p {
        Power::Wide => c(45),
        Power::Slow => c(82),
        Power::Multi => c(201),
    }
}

/// Board, HUD and the pause or stage clear modal, centered in the frame.
pub fn draw(frame: &mut Frame, game: &BrickBall, ctx: &DrawCtx) {
    let b = &game.board;
    let dim = ctx.dim || game.paused || matches!(b.phase, Phase::Clear { .. });
    let area = ui::centered(frame.area(), TOTAL_W, BOARD_H);
    let board_area = Rect::new(area.x, area.y, BOARD_W, BOARD_H);
    let hud = Rect::new(area.x + BOARD_W + 1, area.y, HUD_W, BOARD_H);
    let chrome = if dim {
        theme::CHROME_DIM
    } else {
        theme::CHROME
    };

    let block = ui::panel(chrome, &[]).title_top(Line::styled(
        " brick ball ",
        if dim { theme::CHROME_DIM } else { theme::MUTED },
    ));
    let inner = block.inner(board_area);
    frame.render_widget(block, board_area);

    let mut hud_block = ui::panel(chrome, &[]);
    if !dim {
        hud_block = hud_block.title_bottom(keys().centered());
    }
    let hud_inner = hud_block.inner(hud);
    frame.render_widget(hud_block, hud);

    let buf = frame.buffer_mut();
    paint(buf, inner, b, dim);
    paint_hud(buf, hud_inner, b, ctx, dim);

    // On the field between the bricks (rows 2 to 6) and the ball (row 20),
    // on rows 11 and 13 as in the mockup.
    if b.phase == Phase::Serve && !dim {
        let stage = Span::styled(format!("STAGE {}", b.stage), theme::AMBER);
        ui::center_line(frame, inner, inner.y + 11, vec![stage]);
        let launch = Span::styled("SPACE TO LAUNCH", theme::MUTED);
        ui::center_line(frame, inner, inner.y + 13, vec![launch]);
    }
    if let Phase::Clear { ticks } = b.phase {
        clear_modal(frame, b, ticks);
    }
    // Drawn last: it holds the keys that apply and fits inside the clear modal.
    if game.paused {
        pause_modal(frame, b);
    }
}

/// `←→ ␣ p q`, each key a chip with no padding. ui::chips needs 21 columns
/// and the HUD's inner width is 14.
fn keys() -> Line<'static> {
    let mut spans = Vec::new();
    for (i, key) in ["←→", "␣", "p", "q"].into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(key, theme::CHIP));
    }
    Line::from(spans)
}

/// Pixel colors, None for the field. Later layers win: bricks, paddle, balls.
fn pixels(b: &Board) -> [[Option<Color>; PW]; PH] {
    let mut px = [[None; PW]; PH];
    let (bw, top) = (board::BRICK_W as usize, board::BRICK_TOP as usize);
    for (r, cells) in b.cells.iter().enumerate() {
        for (x, p) in px[top + r].iter_mut().enumerate() {
            let (light, dark) = match cells[x / bw] {
                Cell::Empty => continue,
                Cell::Brick(h) => HP_COLORS[h as usize - 1],
                Cell::Unbreakable => UNBREAKABLE,
            };
            *p = Some(c(if x % bw < bw - 1 { light } else { dark }));
        }
    }

    let paddle = match b.phase {
        Phase::Dying { left } if left % 2 == 1 => Some(theme::DEAD),
        Phase::Dying { .. } => None,
        _ => Some(c(PADDLE)),
    };
    if let Some(color) = paddle {
        let (left, w) = (b.paddle_left(), b.paddle_w());
        for (x, p) in px[board::PADDLE_Y as usize].iter_mut().enumerate() {
            let mid = x as f64 + 0.5;
            if left <= mid && mid < left + w {
                *p = Some(color);
            }
        }
    }

    for ball in &b.balls {
        if let Some(p) = px
            .get_mut(ball.y.floor() as usize)
            .and_then(|row| row.get_mut(ball.x.floor() as usize))
        {
            *p = Some(c(BALL));
        }
    }
    px
}

/// Two pixels per cell with half blocks, then the capsule chip.
fn paint(buf: &mut Buffer, inner: Rect, b: &Board, dim: bool) {
    let field = theme::FIELD[0];
    let lit = |p: Option<Color>| p.map(|color| if dim { theme::DIM_C } else { color });
    let px = pixels(b);
    // Cell row cy shows pixel rows 2cy on top and 2cy + 1 below.
    for (cy, rows) in px.chunks_exact(2).enumerate() {
        for (cx, (&top, &bottom)) in rows[0].iter().zip(&rows[1]).enumerate() {
            let (sym, fg, bg) = match (lit(top), lit(bottom)) {
                (None, None) => (" ", field, field),
                (Some(top), None) => ("▀", top, field),
                (None, Some(bottom)) => ("▄", bottom, field),
                (Some(top), Some(bottom)) => ("▀", top, bottom),
            };
            if let Some(cell) = buf.cell_mut((inner.x + cx as u16, inner.y + cy as u16)) {
                cell.set_symbol(sym).set_fg(fg).set_bg(bg);
            }
        }
    }

    if let Some(cap) = b.capsule {
        let bg = if dim {
            theme::DIM_C
        } else {
            power_color(cap.kind)
        };
        let (x, y) = (cap.x.floor() as u16, cap.y.floor() as u16 / 2);
        buf.set_string(
            inner.x + x,
            inner.y + y,
            cap.kind.letter().to_string(),
            chip(bg),
        );
    }
}

/// Letter chip style: dark bold letter on `bg`.
fn chip(bg: Color) -> Style {
    Style::new().fg(c(233)).bg(bg).add_modifier(Modifier::BOLD)
}

fn paint_hud(buf: &mut Buffer, inner: Rect, b: &Board, ctx: &DrawCtx, dim: bool) {
    let s = |st: Style| if dim { theme::DIM } else { st };
    // One row of styled segments, starting one column in.
    let mut put = |r: u16, segs: &[(&str, Style)]| {
        let spans: Vec<Span> = segs.iter().map(|&(t, st)| Span::styled(t, s(st))).collect();
        buf.set_line(
            inner.x + 1,
            inner.y + r,
            &Line::from(spans),
            inner.width - 1,
        );
    };

    put(1, &[("SCORE", theme::AMBER)]);
    put(2, &[(&b.score.to_string(), theme::ACCENT_HI)]);

    put(4, &[("STAGE", theme::AMBER)]);
    let on = ((b.stage - 1) % stage::BLOCK + 1) as usize;
    put(
        5,
        &[
            (&format!("{:<4}", b.stage), theme::TEXT_BOLD),
            (&"▮".repeat(on), theme::AMBER),
            (&"▮".repeat(stage::BLOCK as usize - on), theme::METER_OFF),
        ],
    );

    put(7, &[("LIVES", theme::AMBER)]);
    let lives = b.lives;
    put(
        8,
        &[
            (&"● ".repeat(lives as usize), theme::ACCENT_HI),
            (
                &"○ ".repeat((board::MAX_LIVES - lives) as usize),
                theme::METER_OFF,
            ),
        ],
    );

    put(10, &[("BEST", theme::AMBER)]);
    match ctx.best {
        Some(r) => put(
            11,
            &[
                (&r.name, theme::GOLD),
                (" ", theme::TEXT),
                (&r.score.to_string(), theme::TEXT),
            ],
        ),
        None => put(11, &[("none yet", theme::MUTED)]),
    }

    put(13, &[("POWER", theme::AMBER)]);
    let timers = [
        (Power::Wide, b.wide, board::WIDE_TICKS),
        (Power::Slow, b.slow, board::SLOW_TICKS),
    ];
    let mut row = 14;
    for (p, ticks, max) in timers.into_iter().filter(|t| t.1 > 0) {
        let on = (ticks * 5).div_ceil(max) as usize;
        let color = power_color(p);
        put(
            row,
            &[
                (&format!(" {} ", p.letter()), chip(color)),
                (" ", theme::TEXT),
                (&"▰".repeat(on), Style::new().fg(color)),
                (&"▱".repeat(5 - on), theme::METER_OFF),
                (&format!(" {}s", board::secs(ticks)), theme::MUTED),
            ],
        );
        row += 1;
    }
    if row == 14 {
        put(14, &[("none", theme::DIM)]);
    }

    put(17, &[("SPEED", theme::AMBER)]);
    put(
        18,
        &[(&format!("{:.2}×", b.speed_multiplier()), theme::TEXT)],
    );
}

fn pause_modal(frame: &mut Frame, b: &Board) {
    let area = ui::centered(frame.area(), 30, 7);
    frame.render_widget(Clear, area);
    let block = ui::modal(
        "PAUSED",
        theme::ACCENT_HI,
        &[("p", "resume"), ("q", "quit")],
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = format!("stage {}  ·  {}", b.stage, b.score);
    ui::center_line(
        frame,
        inner,
        inner.y + 1,
        vec![Span::styled(text, theme::MUTED)],
    );
}

fn clear_modal(frame: &mut Frame, b: &Board, ticks: u32) {
    let area = ui::centered(frame.area(), 36, 10);
    frame.render_widget(Clear, area);
    let title = format!("STAGE {} CLEAR", b.stage);
    let block = ui::modal(&title, theme::GOLD, &[]);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Label padded to 12, value right-aligned so the row is inner width - 2.
    let w = inner.width as usize - 2 - 12;
    let rows = [
        ("bricks", format!("+{}", b.stage_points), theme::TEXT),
        ("clear bonus", format!("+{}", 100 * b.stage), theme::TEXT),
        ("score", b.score.to_string(), theme::ACCENT_HI),
    ];
    for (i, (label, value, style)) in rows.into_iter().enumerate() {
        let line = Line::from(vec![
            Span::styled(format!("{label:<12}"), theme::MUTED),
            Span::styled(format!("{value:>w$}"), style),
        ]);
        frame
            .buffer_mut()
            .set_line(inner.x + 1, inner.y + 1 + i as u16, &line, inner.width - 2);
    }
    let next = format!("next life at stage {}", b.next_life_stage());
    ui::center_line(
        frame,
        inner,
        inner.y + 5,
        vec![Span::styled(next, theme::MUTED)],
    );
    let countdown = format!("STAGE {} IN {}", b.stage + 1, board::secs(ticks));
    ui::center_line(
        frame,
        inner,
        inner.y + 7,
        vec![Span::styled(countdown, theme::AMBER)],
    );
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        board::Capsule,
        stage::{COLS, Grid, ROWS},
    };
    use arcade_sdk::{ScoreRow, testkit::render as render_with};

    fn best() -> ScoreRow {
        ScoreRow {
            name: "ABC".into(),
            score: 42,
            variant: "stage 1-3".into(),
        }
    }

    fn render(g: &BrickBall, dim: bool) -> String {
        let best = best();
        let ctx = DrawCtx {
            best: Some(&best),
            dim,
        };
        render_with(80, 24, |f| draw(f, g, &ctx))
    }

    /// The live frame at 80 by 24, for colors.
    fn buffer(g: &BrickBall) -> Buffer {
        let best = best();
        let ctx = DrawCtx {
            best: Some(&best),
            dim: false,
        };
        let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
        t.draw(|f| draw(f, g, &ctx)).unwrap();
        t.backend().buffer().clone()
    }

    fn has(s: &str, wants: &[&str]) {
        for want in wants {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
    }

    /// Character at screen column x of row y.
    fn at(s: &str, x: usize, y: usize) -> char {
        s.lines().nth(y).unwrap().chars().nth(x).unwrap()
    }

    fn empty_cells() -> Grid {
        [[Cell::Empty; COLS]; ROWS]
    }

    #[test]
    fn renders_hud_labels_and_paddle_at_80_by_24() {
        let s = render(&BrickBall::new(1, 1), false);
        has(
            &s,
            &[
                "SCORE",
                "STAGE",
                "LIVES",
                "● ● ● ○ ○",
                "BEST",
                "ABC 42",
                "POWER",
                "none",
                "SPEED",
                "1.03×",
                "←→ ␣ p q",
                "brick ball",
            ],
        );
        // The 79 column screen starts at x 0, so board pixel x is screen column x + 1.
        // Paddle: 9 pixels centered on x 30, pixels 25 to 33, pixel row 42.
        let paddle: String = (25..=33).map(|x| at(&s, x + 1, 22)).collect();
        assert_eq!(paddle, "▀".repeat(9), "{s}");
        assert_eq!((at(&s, 25, 22), at(&s, 35, 22)), (' ', ' '));
        // Ball at (30, 41.5): the lower pixel of inner row 20.
        assert_eq!(at(&s, 31, 21), '▄', "{s}");
    }

    #[test]
    fn dimmed_draw_has_no_key_chips() {
        let s = render(&BrickBall::new(1, 1), true);
        assert!(!s.contains("←→") && !s.contains('␣'), "{s}");
        has(&s, &["SCORE", "LIVES", "SPEED"]);
    }

    #[test]
    fn brick_pixels_are_light_then_dark() {
        let fg = |cell: Cell| {
            let mut g = BrickBall::new(1, 1);
            g.board.cells = empty_cells();
            g.board.cells[0][0] = cell;
            let buf = buffer(&g);
            let fg = |x| buf[(x, 3)].fg;
            assert_eq!((fg(1), fg(2)), (fg(3), fg(3)), "the light pixels match");
            (fg(3), fg(4))
        };
        assert_eq!(fg(Cell::Brick(1)), (c(223), c(180)));
        assert_eq!(fg(Cell::Brick(5)), (c(124), c(88)));
        assert_eq!(fg(Cell::Unbreakable), (c(245), c(239)));
    }

    #[test]
    fn capsule_draws_a_letter_chip() {
        let mut g = BrickBall::new(1, 1);
        g.board.capsule = Some(Capsule {
            kind: Power::Wide,
            x: 10.0,
            y: 20.0,
        });
        let buf = buffer(&g);
        let cell = &buf[(1 + 10, 1 + 10)];
        assert_eq!((cell.symbol(), cell.bg), ("W", c(45)));
    }

    #[test]
    fn paused_and_clear_draw_their_modals() {
        let mut g = BrickBall::new(1, 1);
        g.paused = true;
        has(&render(&g, false), &["PAUSED", "stage 1  ·  0"]);

        g.paused = false;
        g.board.phase = Phase::Clear { ticks: 60 };
        g.board.score = 300;
        has(
            &render(&g, false),
            &[
                "STAGE 1 CLEAR",
                "clear bonus",
                "next life at stage 5",
                "STAGE 2 IN 2",
            ],
        );

        g.paused = true;
        has(&render(&g, false), &["PAUSED", "stage 1  ·  300"]);
    }

    #[test]
    fn serve_shows_the_stage_and_space_to_launch() {
        // Screen rows 12 and 14 are board rows 11 and 13.
        let rows = |s: &str| {
            let row = |y: usize| s.lines().nth(y).unwrap().to_string();
            (row(12), row(14))
        };
        let mut g = BrickBall::new(1, 3);
        let s = render(&g, false);
        let (stage, launch) = rows(&s);
        assert!(
            stage.contains("STAGE 3") && launch.contains("SPACE TO LAUNCH"),
            "{s}"
        );

        let dimmed = render(&g, true);
        g.paused = true;
        let paused = render(&g, false);
        g.paused = false;
        g.board.phase = Phase::Play;
        let play = render(&g, false);
        // The pause modal covers the prompt rows except the SP of SPACE.
        for s in [dimmed, paused, play] {
            let (stage, launch) = rows(&s);
            assert!(!stage.contains("STAGE") && !launch.contains("SP"), "{s}");
        }
    }
}
