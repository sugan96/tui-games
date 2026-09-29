mod font;
mod game_over;
mod menu;
mod name_entry;
mod play;
mod theme;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};

use crate::app::{App, Screen};

/// Smallest terminal that fits the board with its HUD, and the menu panel.
pub const MIN_WIDTH: u16 = 108;
pub const MIN_HEIGHT: u16 = 28;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = format!(
            "terminal too small\nneed {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
            area.width, area.height
        );
        let rect = centered(area, area.width.min(40), 2);
        frame.render_widget(
            Paragraph::new(msg)
                .style(theme::DANGER)
                .alignment(Alignment::Center),
            rect,
        );
        return;
    }
    match app.screen {
        Screen::Menu => menu::draw(frame, app),
        Screen::Playing | Screen::Dying { .. } => play::draw(frame, app),
        Screen::NameEntry { .. } => name_entry::draw(frame, app),
        Screen::GameOver { .. } => game_over::draw(frame, app),
    }
}

/// A `width` by `height` rectangle centered in `area`, clipped to it.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// Double-line panel. Key hints, if any, sit as chips in the bottom border.
pub fn panel<'a>(border: Style, hints: &[(&'a str, &'a str)]) -> Block<'a> {
    let mut block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(border);
    if !hints.is_empty() {
        block = block.title_bottom(chips(hints).centered());
    }
    block
}

/// Modal panel: accent border, filled background, centered title.
pub fn modal<'a>(title: &'a str, title_style: Style, hints: &[(&'a str, &'a str)]) -> Block<'a> {
    panel(theme::MODAL_BORDER, hints)
        .style(Style::new().bg(theme::MODAL_BG))
        .title_top(Line::styled(format!(" {title} "), title_style).centered())
}

/// ` key  label   key  label ` with each key drawn as a chip.
pub fn chips<'a>(hints: &[(&'a str, &'a str)]) -> Line<'a> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(format!(" {key} "), theme::CHIP));
        if !label.is_empty() {
            spans.push(Span::styled(format!(" {label}"), theme::MUTED));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Write `spans` on one row, centered in `area`.
pub fn center_line(frame: &mut Frame, area: Rect, y: u16, spans: Vec<Span>) {
    let width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let x = area.x + (area.width.saturating_sub(width as u16)) / 2;
    frame
        .buffer_mut()
        .set_line(x, y, &Line::from(spans), area.width);
}
#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        db::{Db, ScoreRow},
        game::{Game, Mode},
    };

    const COLS: u16 = 120;
    const ROWS: u16 = 32;
    // Board is 107 wide by 22 tall centered in COLS by ROWS: inner origin (7, 6).
    const IX: usize = 7;
    const IY: usize = 6;

    fn render(app: &App) -> String {
        render_at(app, COLS, ROWS)
    }

    fn render_at(app: &App, cols: u16, rows: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(cols, rows)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        t.backend()
            .buffer()
            .content()
            .chunks(cols as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app() -> App {
        App::new(Db::open_in_memory().unwrap()).unwrap()
    }

    fn top() -> Vec<ScoreRow> {
        vec![
            ScoreRow {
                name: "ABC".into(),
                score: 42,
                mode: Mode::Fixed(5),
            },
            ScoreRow {
                name: "XYZ".into(),
                score: 7,
                mode: Mode::Variable,
            },
        ]
    }

    /// Head at (22, 11) moving right, body back and down to (17, 9), tail just
    /// left (16, 9). Food at (30, 14).
    fn playing() -> App {
        let mut a = app();
        a.top = top();
        a.game = Game::new(Mode::Fixed(3));
        a.game.apples = 4;
        a.game.score = 12;
        a.game.snake = [
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
        a.game.last_tail = Some((16, 9));
        a.game.food = (30, 14);
        a.screen = Screen::Playing;
        a
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

    fn has(s: &str, wants: &[&str]) {
        for want in wants {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
    }

    #[test]
    fn small_terminal_shows_size_message() {
        let s = render_at(&app(), 100, 30);
        has(&s, &["terminal too small", "need 108x28, have 100x30"]);
        assert!(!s.contains("PRESS"));
    }

    #[test]
    fn centered_rect_is_centered_and_clipped() {
        let r = centered(Rect::new(0, 0, 100, 30), 40, 10);
        assert_eq!(r, Rect::new(30, 10, 40, 10));
        let r = centered(Rect::new(0, 0, 20, 5), 40, 10);
        assert_eq!(r, Rect::new(0, 0, 20, 5));
    }

    #[test]
    fn menu_empty_shows_placeholders_prompt_and_speed_picker() {
        let s = render(&app());
        has(
            &s,
            &[
                "█████ █   █  ███  █   █ █████",
                "▀▄▀▄▀▄",
                "RANK  NAME       SCORE   MODE",
                "  1    ·  ·  ·",
                " 10    ·  ·  ·",
                "PRESS 1-9 OR V TO START",
                "SPEED  1   2   3   4   5   6   7   8   9   v auto",
                "▁   ▂   ▃   ▄   ▅   ▆   ▇   █   █",
                "200ms",
                "40ms",
                "1-9  fixed speed   v  variable   q  quit",
            ],
        );
        let speed = s.lines().find(|l| l.contains("SPEED")).unwrap();
        assert!(
            speed.trim_end().ends_with('║'),
            "speed row broke the border:\n{speed}"
        );
    }

    #[test]
    fn menu_lists_top_rows_and_blinks_prompt() {
        let mut a = app();
        a.top = top();
        let s = render(&a);
        has(
            &s,
            &[
                "★  1  ABC           42   fixed 5",
                "★  2  XYZ            7   variable",
                "  3    ·  ·  ·",
            ],
        );
        a.ticks = 3;
        assert!(!render(&a).contains("PRESS"));
    }

    #[test]
    fn play_settled_draws_pointed_head_food_and_hud() {
        let mut a = playing();
        a.settled = true;
        let s = render(&a);
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
        has(
            &s,
            &[
                "1UP",
                "HI-SCORE     42  ABC",
                "LEVEL          3 / 9",
                "▮ ▮ ▮ ▮ ▮ ▮ ▮ ▮ ▮",
                "APPLES          ▟▙ 4",
                "MODE           fixed",
                "SPEED          160ms",
                "↑↓←→    q  quit",
            ],
        );
    }

    #[test]
    fn play_halfway_shifts_head_and_tail_by_one_column() {
        let mut a = playing();
        a.settled = false;
        let s = render(&a);
        assert_eq!(cell(&s, 21, 11, 1), '█', "{s}");
        assert_eq!(cell(&s, 22, 11, 0), '▶');
        assert_eq!(cell(&s, 22, 11, 1), ' ');
        assert_eq!(cell(&s, 16, 9, 0), ' ');
        assert_eq!(cell(&s, 16, 9, 1), '█');
    }

    #[test]
    fn play_halfway_vertical_uses_half_blocks() {
        let mut a = playing();
        a.settled = false;
        a.game.snake = [(5, 6), (5, 5), (5, 4)].into();
        a.game.last_tail = Some((5, 3));
        let s = render(&a);
        assert_eq!((cell(&s, 5, 6, 0), cell(&s, 5, 6, 1)), ('▄', '▄'), "{s}");
        assert_eq!((cell(&s, 5, 3, 0), cell(&s, 5, 3, 1)), ('▀', '▀'));
    }

    #[test]
    fn dying_blinks_the_snake() {
        let mut a = playing();
        a.screen = Screen::Dying { left: 5 };
        let s = render(&a);
        assert_eq!(cell(&s, 22, 11, 1), '▶', "{s}");
        a.screen = Screen::Dying { left: 4 };
        let s = render(&a);
        assert_eq!(cell(&s, 22, 11, 1), ' ', "{s}");
        assert_eq!(cell(&s, 18, 10, 0), ' ');
        assert_eq!(cell(&s, 30, 14, 0), '▟', "food stays");
    }

    #[test]
    fn name_entry_modal_over_dimmed_board() {
        let mut a = playing();
        a.screen = Screen::NameEntry {
            game_id: 1,
            name: "AB".into(),
        };
        let s = render(&a);
        has(
            &s,
            &[
                "NEW HIGH SCORE",
                "score 12   ·   rank 2",
                "│ A │ │ B │ │ _ │",
                "A-Z  type   Bksp    Enter  confirm",
                "HI-SCORE     42  ABC",
            ],
        );
        assert!(!s.contains("q  quit"), "board hints hide under a modal");
    }

    #[test]
    fn game_over_modal_shows_block_score_and_rank_tag() {
        let mut a = playing();
        a.screen = Screen::GameOver { ranked: true };
        let s = render(&a);
        has(
            &s,
            &[
                "GAME OVER",
                "  █ ███",
                "★ new best   ·   ▟▙ 4 apples   ·   fixed 3",
                "r  menu   q  quit",
            ],
        );
        a.screen = Screen::GameOver { ranked: false };
        let s = render(&a);
        has(&s, &["▟▙ 4 apples   ·   fixed 3"]);
        assert!(!s.contains("new best"));
    }
}
