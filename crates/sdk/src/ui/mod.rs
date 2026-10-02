pub mod font;
mod game_over;
mod menu;
mod name_entry;
pub mod theme;
pub mod thumb;

use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};

use crate::{
    app::{App, Screen},
    game::DrawCtx,
};

pub(crate) fn draw(frame: &mut Frame, app: &App) {
    let (min_w, min_h) = min_size(app);
    if too_small(frame, min_w, min_h) {
        return;
    }
    let ctx = |dim| DrawCtx {
        best: app.top.first(),
        dim,
    };
    match &app.screen {
        Screen::Menu => menu::draw(frame, app),
        Screen::Playing(game) => game.draw(frame, &ctx(false)),
        Screen::NameEntry {
            game,
            outcome,
            name,
            ..
        } => {
            game.draw(frame, &ctx(true));
            name_entry::draw(frame, app, outcome.score, name);
        }
        Screen::GameOver {
            game,
            outcome,
            ranked,
        } => {
            game.draw(frame, &ctx(true));
            game_over::draw(frame, outcome, *ranked);
        }
    }
}

/// The menu panel or the game, whichever is larger on each axis.
fn min_size(app: &App) -> (u16, u16) {
    let (w, h) = app.entry.min_size;
    (w.max(menu::WIDTH), h.max(menu::HEIGHT))
}

/// If the frame is smaller than `width` by `height`, draws a message saying so
/// and returns true. The caller then draws nothing else.
pub fn too_small(frame: &mut Frame, width: u16, height: u16) -> bool {
    let area = frame.area();
    if area.width >= width && area.height >= height {
        return false;
    }
    let msg = format!(
        "terminal too small\nneed {width}x{height}, have {}x{}",
        area.width, area.height
    );
    let rect = centered(area, area.width.min(40), 2);
    frame.render_widget(
        Paragraph::new(msg)
            .style(theme::DANGER)
            .alignment(Alignment::Center),
        rect,
    );
    true
}

/// Block letter logo with a drop shadow, centered across `area`, top row at `y`.
pub fn logo(buf: &mut Buffer, area: Rect, y: u16, text: &str) {
    let rows = font::render(&text.to_ascii_uppercase());
    let x = area.x + area.width.saturating_sub(rows[0].chars().count() as u16) / 2;
    let blit = |buf: &mut Buffer, x: u16, y: u16, row: &str, color| {
        for (i, ch) in row.chars().enumerate() {
            if ch != ' ' {
                buf.set_string(x + i as u16, y, "█", Style::new().fg(color));
            }
        }
    };
    for (i, row) in rows.iter().enumerate() {
        blit(buf, x + 1, y + 1 + i as u16, row, theme::SHADOW);
    }
    for (i, row) in rows.iter().enumerate() {
        blit(buf, x, y + i as u16, row, theme::LOGO[i]);
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
    use super::*;
    use crate::{
        app::tests::{app, staged_app, start},
        db::ScoreRow,
        game::Outcome,
        testkit,
    };

    fn render(app: &App) -> String {
        testkit::render(80, 30, |f| draw(f, app))
    }

    fn top() -> Vec<ScoreRow> {
        vec![
            ScoreRow {
                name: "ABC".into(),
                score: 42,
                variant: "fixed 5".into(),
            },
            ScoreRow {
                name: "XYZ".into(),
                score: 7,
                variant: "variable".into(),
            },
        ]
    }

    fn outcome() -> Outcome {
        Outcome {
            score: 12,
            variant: "fixed 3".into(),
            summary: "▟▙ 4 apples".into(),
        }
    }

    fn has(s: &str, wants: &[&str]) {
        for want in wants {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
    }

    #[test]
    fn small_terminal_shows_size_message() {
        let s = testkit::render(60, 20, |f| draw(f, &app()));
        has(&s, &["terminal too small", "need 64x28, have 60x20"]);
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
    fn menu_empty_shows_logo_placeholders_prompt_and_about() {
        let s = render(&app());
        has(
            &s,
            &[
                "█████  ███  █   █ █████",
                "▀▄▀▄▀▄",
                "RANK  NAME       SCORE   MODE",
                "  1    ·  ·  ·",
                " 10    ·  ·  ·",
                "PRESS 0-9 TO START",
                "fake about line",
                "0-9  score   q  quit",
            ],
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
    fn playing_draws_the_game_live() {
        let mut a = app();
        a.screen = Screen::Playing(start('3'));
        has(&render(&a), &["fake live"]);
    }

    #[test]
    fn name_entry_modal_over_dimmed_game() {
        let mut a = app();
        a.top = top();
        a.screen = Screen::NameEntry {
            game: start('3'),
            outcome: outcome(),
            row: 1,
            name: "AB".into(),
        };
        has(
            &render(&a),
            &[
                "fake dim",
                "NEW HIGH SCORE",
                "score 12   ·   rank 2",
                "│ A │ │ B │ │ _ │",
                "A-Z  type   Bksp    Enter  confirm",
            ],
        );
    }

    #[test]
    fn game_over_modal_shows_block_score_summary_and_rank_tag() {
        let mut a = app();
        a.screen = Screen::GameOver {
            game: start('3'),
            outcome: outcome(),
            ranked: true,
        };
        has(
            &render(&a),
            &[
                "fake dim",
                "GAME OVER",
                "    ██ ██████",
                "★ new best   ·   ▟▙ 4 apples   ·   fixed 3",
                "r  menu   q  quit",
            ],
        );
        let Screen::GameOver { ranked, .. } = &mut a.screen else {
            unreachable!()
        };
        *ranked = false;
        let s = render(&a);
        has(&s, &["▟▙ 4 apples   ·   fixed 3"]);
        assert!(!s.contains("new best"));
    }

    #[test]
    fn menu_stage_picker_dims_arrows_at_the_limits() {
        use ratatui::{Terminal, backend::TestBackend, style::Color};
        // Text and the fg of the two arrow cells.
        fn picker(app: &App) -> (String, Color, Color) {
            let mut t = Terminal::new(TestBackend::new(80, 30)).unwrap();
            t.draw(|f| draw(f, app)).unwrap();
            let buf = t.backend().buffer();
            let cell = |sym| buf.content().iter().find(|c| c.symbol() == sym).unwrap().fg;
            let text = buf.content().iter().map(|c| c.symbol()).collect();
            (text, cell("◀"), cell("▶"))
        }
        let (dim, live) = (Color::Indexed(237), Color::Indexed(214));
        let mut a = staged_app(3);
        let (s, left, right) = picker(&a);
        has(&s, &["◀ STAGE 1 ▶ of 3", "←→  stage"]);
        assert_eq!((left, right), (dim, live));
        a.stage = 3;
        let (s, left, right) = picker(&a);
        has(&s, &["◀ STAGE 3 ▶ of 3"]);
        assert_eq!((left, right), (live, dim));
        let (_, left, right) = picker(&staged_app(1));
        assert_eq!((left, right), (dim, dim));
    }

    #[test]
    fn menu_without_stages_has_no_picker() {
        assert!(!render(&app()).contains("STAGE"));
    }
}
