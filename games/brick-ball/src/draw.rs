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
/// Grey index of the field, theme::FIELD[0]. A ball is shaded from it toward
/// BALL by coverage.
const FIELD_GREY: u8 = 233;

const fn c(i: u8) -> Color {
    Color::Indexed(i)
}

/// Grey of a pixel a ball covers by c: FIELD_GREY + round(c² × (BALL - FIELD_GREY)),
/// c clamped to 0..1. Squared so the neighbours of the main pixel fall off fast.
fn shade(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    FIELD_GREY + (c * c * f64::from(BALL - FIELD_GREY)).round() as u8
}

/// Color of a capsule and of its HUD timer chip.
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
    if let Phase::Clear { left } = b.phase {
        clear_modal(frame, b, left);
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

/// Pixels a 1 wide, h tall block at (x, y) overlaps, as (x, y, coverage):
/// the overlap on x times the overlap on y, each in 0..1. For a 1 by 1 block
/// the overlap with pixel p is 1 - |x - p| clamped to 0..1.
fn coverage(x: f64, y: f64, h: f64) -> impl Iterator<Item = (usize, usize, f64)> {
    let span = |a: f64, len: f64| {
        (a.floor() as usize..=(a + len).floor() as usize).map(move |p| {
            (
                p,
                ((a + len).min(p as f64 + 1.0) - a.max(p as f64)).clamp(0.0, 1.0),
            )
        })
    };
    span(y, h).flat_map(move |(py, cy)| span(x, 1.0).map(move |(px, cx)| (px, py, cx * cy)))
}

/// Pixel colors, None for the field. Later layers win: bricks, paddle,
/// capsule, balls.
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

    // A ball or capsule is drawn as a block centered on its position, so a
    // ball at a pixel center covers that pixel alone.
    let at = |v: f64| v - 0.5;

    // The capsule: a 1 by 2 block, in its power color on each pixel it covers
    // by half or more. It falls straight down, so it keeps the pixel its x
    // floors to.
    if let Some(cap) = b.capsule {
        for (x, y, cover) in coverage(cap.x.floor(), at(cap.y), 2.0) {
            if cover >= 0.5
                && let Some(p) = px.get_mut(y).and_then(|row| row.get_mut(x))
            {
                *p = Some(power_color(cap.kind));
            }
        }
    }

    // Ball coverage per pixel, summed over the balls, is shaded by shade(),
    // which caps it at 1. A brick, the paddle or the capsule shows the shade
    // only at coverage 0.5 or more.
    let mut light = [[0.0; PW]; PH];
    for ball in &b.balls {
        for (x, y, cover) in coverage(at(ball.x), at(ball.y), 1.0) {
            if let Some(l) = light.get_mut(y).and_then(|row| row.get_mut(x)) {
                *l += cover;
            }
        }
    }
    for (row, lights) in px.iter_mut().zip(&light) {
        for (p, &l) in row.iter_mut().zip(lights) {
            let shade = shade(l);
            if shade > FIELD_GREY && (p.is_none() || l >= 0.5) {
                *p = Some(c(shade));
            }
        }
    }
    px
}

/// Two pixels per cell with half blocks.
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
        (Power::Wide, b.wide, board::WIDE_SECS),
        (Power::Slow, b.slow, board::SLOW_SECS),
    ];
    let mut row = 14;
    for (p, left, max) in timers.into_iter().filter(|t| t.1 > 0.0) {
        // Marks lit out of 5: ceil(5 × left / max).
        let on = (5.0 * left / max).ceil() as usize;
        let color = power_color(p);
        put(
            row,
            &[
                (&format!(" {} ", p.letter()), chip(color)),
                (" ", theme::TEXT),
                (&"▰".repeat(on), Style::new().fg(color)),
                (&"▱".repeat(5 - on), theme::METER_OFF),
                (&format!(" {}s", board::secs(left)), theme::MUTED),
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
    let block = ui::modal("PAUSED", theme::ACCENT_HI, &[("p", "resume"), ("q", "end")]);
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

fn clear_modal(frame: &mut Frame, b: &Board, left: f64) {
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
    // At the life cap the row stays blank so the modal keeps its size.
    if b.lives < board::MAX_LIVES {
        let next = format!("next life at stage {}", b.next_life_stage());
        ui::center_line(
            frame,
            inner,
            inner.y + 5,
            vec![Span::styled(next, theme::MUTED)],
        );
    }
    let countdown = format!("STAGE {} IN {}", b.stage + 1, board::secs(left));
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
        // Ball at (29.5, 41.5) on the paddle's middle pixel 29: the lower
        // pixel of inner row 20.
        assert_eq!(at(&s, 30, 21), '▄', "{s}");
    }

    /// Lit pixels above the paddle row as (x, y, color), for one ball at (x, y).
    fn lit(x: f64, y: f64) -> Vec<(usize, usize, Color)> {
        let mut b = Board::new(1, 1);
        (b.cells, b.phase) = (empty_cells(), Phase::Play);
        (b.balls[0].x, b.balls[0].y) = (x, y);
        let px = pixels(&b);
        let mut out = Vec::new();
        for (py, row) in px.iter().enumerate().take(board::PADDLE_Y as usize) {
            for (pxx, p) in row.iter().enumerate() {
                if let Some(color) = *p {
                    out.push((pxx, py, color));
                }
            }
        }
        out
    }

    #[test]
    fn shade_is_squared() {
        assert_eq!(
            (shade(1.0), shade(0.5), shade(0.25), shade(0.0)),
            (255, 239, 234, 233)
        );
    }

    #[test]
    fn smooth_ball_shades_two_pixels_by_coverage() {
        // The square is centered on the ball, so a ball at a pixel center
        // lights that pixel alone.
        assert_eq!(lit(30.5, 21.0), [(30, 20, c(239)), (30, 21, c(239))]);
        assert_eq!(lit(30.5, 20.5), [(30, 20, c(255))]);
        let quarter = c(234);
        assert_eq!(
            lit(31.0, 21.0),
            [
                (30, 20, quarter),
                (31, 20, quarter),
                (30, 21, quarter),
                (31, 21, quarter)
            ]
        );
    }

    #[test]
    fn smooth_colored_pixels_change_only_at_half_coverage() {
        let mut b = Board::new(1, 1);
        (b.cells, b.phase) = (empty_cells(), Phase::Play);
        // A ball in paddle row 42, 0.4 over pixel 30 and 0.6 over pixel 31.
        let paddle_x = 30;
        (b.balls[0].x, b.balls[0].y) = (paddle_x as f64 + 1.1, 42.5);
        b.capsule = Some(Capsule {
            kind: Power::Slow,
            x: 10.0,
            y: 20.9,
        });
        let px = pixels(&b);
        // Covered 0.4 by the ball: the paddle stays; 0.6: the ball shade.
        assert_eq!(px[42][paddle_x], Some(c(PADDLE)));
        assert_eq!(px[42][paddle_x + 1], Some(c(233 + 8)));
        // Capsule rows 20, 21, 22 covered 0.6, 1 and 0.4.
        let col: Vec<_> = (19..24).map(|y| px[y][10]).collect();
        let slow = Some(power_color(Power::Slow));
        assert_eq!(col, [None, slow, slow, None, None]);

        // Two balls on one pixel add up, capped at 1.
        b.balls = vec![b.balls[0]; 2];
        (b.balls[0].x, b.balls[0].y) = (20.5, 21.0);
        (b.balls[1].x, b.balls[1].y) = (20.5, 20.5);
        let px = pixels(&b);
        assert_eq!((px[20][20], px[21][20]), (Some(c(255)), Some(c(239))));
        // Two balls each covering a pixel by 0.5 light it fully.
        (b.balls[1].x, b.balls[1].y) = (20.5, 20.0);
        let col: Vec<_> = (19..22).map(|y| pixels(&b)[y][20]).collect();
        assert_eq!(col, [Some(c(239)), Some(c(255)), Some(c(239))]);

        // A capsule dropped by a brick, at a pixel center, covers its two
        // rows only.
        b.capsule.as_mut().unwrap().y = 20.5;
        let col: Vec<_> = (19..23).map(|y| pixels(&b)[y][10]).collect();
        assert_eq!(col, [None, slow, slow, None]);
    }

    #[test]
    fn serve_ball_is_drawn_like_a_moving_ball() {
        // The ball rests on the paddle's middle pixel in row 41: that pixel
        // alone, at full light, nothing in the rows above or below but the
        // paddle. The paddle starts at 30.0, on a pixel edge.
        for (cx, want) in [(30.0, 29), (30.2, 30), (30.5, 30), (30.7, 30)] {
            let mut b = Board::new(1, 1);
            b.cells = empty_cells();
            b.paddle_cx = cx;
            b.tick(0, false);
            assert_eq!(b.phase, Phase::Serve);
            let px = pixels(&b);
            let ball = |y: usize| {
                px[y]
                    .iter()
                    .enumerate()
                    .filter(|&(_, &p)| p.is_some() && p != Some(c(PADDLE)))
                    .map(|(x, &p)| (x, p))
                    .collect::<Vec<_>>()
            };
            assert_eq!(ball(41), [(want, Some(c(BALL)))], "paddle_cx {cx}");
            assert_eq!((ball(40), ball(42)), (vec![], vec![]), "paddle_cx {cx}");
            let paddle: Vec<_> = (0..PW).filter(|&x| px[42][x] == Some(c(PADDLE))).collect();
            assert_eq!((paddle.len(), paddle[4]), (9, want), "paddle_cx {cx}");
        }
    }

    #[test]
    fn serve_ball_keeps_its_paddle_pixel_at_every_width() {
        // Widths 8, 9 and, with W, 12. Moving the paddle 0.3 px at a time
        // across two pixels keeps the ball on the paddle's middle drawn pixel:
        // the leftmost drawn pixel plus (w - 1) / 2 rounded down.
        for (paddle, wide) in [(8, 0.0), (9, 0.0), (9, 5.0)] {
            let mut b = Board::new(1, 1);
            (b.levers.paddle, b.wide) = (paddle, wide);
            let w = b.paddle_w() as usize;
            for k in 0..8 {
                b.paddle_cx = 30.0 + 0.3 * k as f64;
                b.tick(0, false);
                let px = pixels(&b);
                let first = (0..PW).find(|&x| px[42][x] == Some(c(PADDLE))).unwrap();
                let ball: Vec<_> = (0..PW)
                    .filter(|&x| px[41][x].is_some())
                    .map(|x| (x, px[41][x]))
                    .collect();
                assert_eq!(
                    ball,
                    [(first + (w - 1) / 2, Some(c(BALL)))],
                    "w {w}, paddle_cx {}",
                    b.paddle_cx
                );
            }
        }
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
    fn capsule_draws_two_colored_pixels() {
        let mut g = BrickBall::new(1, 1);
        g.board.capsule = Some(Capsule {
            kind: Power::Wide,
            x: 10.0,
            y: 20.0,
        });
        g.board.wide = 5.0;
        // Pixels (10, 20) and (10, 21) fill inner cell (10, 10).
        let buf = buffer(&g);
        let cell = &buf[(1 + 10, 1 + 10)];
        assert_eq!((cell.symbol(), cell.fg, cell.bg), ("▀", c(45), c(45)));
        // No letter on the board, the HUD still has the W chip.
        let s = render(&g, false);
        for line in s.lines().skip(1).take(PH / 2) {
            let board: String = line.chars().skip(1).take(PW).collect();
            assert!(!board.contains('W'), "{s}");
        }
        has(&s, &[" W "]);
    }

    #[test]
    fn paused_and_clear_draw_their_modals() {
        let mut g = BrickBall::new(1, 1);
        g.paused = true;
        has(&render(&g, false), &["PAUSED", "stage 1  ·  0", "q  end"]);

        g.paused = false;
        g.board.phase = Phase::Clear { left: 2.0 };
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
    fn clear_modal_hides_the_next_life_line_at_the_cap() {
        let mut g = BrickBall::new(1, 1);
        g.board.phase = Phase::Clear { left: 2.0 };
        g.board.lives = board::MAX_LIVES;
        let s = render(&g, false);
        assert!(
            s.contains("STAGE 1 CLEAR") && !s.contains("next life"),
            "{s}"
        );
        for lives in [3, board::MAX_LIVES - 1] {
            g.board.lives = lives;
            has(&render(&g, false), &["next life at stage 5"]);
        }
    }

    #[test]
    fn power_bar_and_countdown_use_seconds() {
        let mut g = BrickBall::new(1, 1);
        g.board.wide = 9.4;
        has(&render(&g, false), &[" W  ▰▰▰▰▱ 10s"]);

        g.board.phase = Phase::Clear { left: 1.2 };
        has(&render(&g, false), &["STAGE 2 IN 2"]);
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
