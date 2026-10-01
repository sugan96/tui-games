//! Pixel-art thumbnails for the launcher's game cards. A thumbnail is THUMB_H
//! rows of THUMB_W palette letters, one letter per pixel. Each terminal cell
//! shows two pixels stacked with a half block, so pixels come out square.

use ratatui::{
    buffer::Buffer,
    style::{Color, Style},
};

use super::{font, theme};

pub const THUMB_W: usize = 24;
pub const THUMB_H: usize = 12;
/// Terminal rows a thumbnail takes.
pub const THUMB_ROWS: u16 = THUMB_H as u16 / 2;

/// The color of a palette letter. `.` and space are transparent, and so is
/// any other letter (testkit::check rejects those in a game's own art).
pub fn color(c: char) -> Option<Color> {
    let i = match c {
        'k' => 16,  // black
        'w' => 255, // white
        'a' => 244, // grey
        'd' => 238, // dark grey
        'r' => 196, // red
        'R' => 124, // dark red
        'o' => 208, // orange
        'y' => 226, // yellow
        'Y' => 220, // gold
        'l' => 118, // lime
        'g' => 40,  // green
        'G' => 28,  // dark green
        'c' => 51,  // cyan
        'b' => 33,  // blue
        'B' => 19,  // navy
        'p' => 93,  // purple
        'm' => 201, // magenta
        'n' => 130, // brown
        's' => 173, // terracotta
        'S' => 209, // light terracotta
        _ => return None,
    };
    Some(Color::Indexed(i))
}

/// True for the letters `color` knows, plus `.` for transparent.
pub fn is_palette(c: char) -> bool {
    c == '.' || color(c).is_some()
}

/// Draw `rows` with its top left cell at (x, y) over `bg`. Missing rows and
/// columns are transparent, extra ones are ignored. With no rows at all,
/// draws `initial` in block letters instead.
pub fn draw(buf: &mut Buffer, x: u16, y: u16, rows: &[String], bg: Color, initial: char) {
    if rows.is_empty() {
        return placeholder(buf, x, y, bg, initial);
    }
    let px = |col: usize, row: usize| {
        rows.get(row)
            .and_then(|r| r.chars().nth(col))
            .and_then(color)
            .unwrap_or(bg)
    };
    for cy in 0..THUMB_ROWS as usize {
        for cx in 0..THUMB_W {
            let style = Style::new().fg(px(cx, cy * 2)).bg(px(cx, cy * 2 + 1));
            buf.set_string(x + cx as u16, y + cy as u16, "▀", style);
        }
    }
}

/// A blank card with the game's initial in block digits, for a game without art.
fn placeholder(buf: &mut Buffer, x: u16, y: u16, bg: Color, initial: char) {
    let blank = " ".repeat(THUMB_W);
    for cy in 0..THUMB_ROWS {
        buf.set_string(x, y + cy, &blank, Style::new().bg(bg));
    }
    let glyph = font::render_wide(&initial.to_ascii_uppercase().to_string());
    let gx = x + (THUMB_W as u16).saturating_sub(glyph[0].chars().count() as u16) / 2;
    for (i, row) in glyph.iter().enumerate() {
        buf.set_string(gx, y + i as u16, row, theme::MUTED.bg(bg));
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::*;

    fn cell(buf: &Buffer, x: u16, y: u16) -> (String, Option<Color>, Option<Color>) {
        let c = &buf[(x, y)];
        (c.symbol().to_string(), c.style().fg, c.style().bg)
    }

    #[test]
    fn two_pixels_per_cell_with_transparent_as_background() {
        let bg = Color::Indexed(233);
        let rows: Vec<String> = ["r.", "gk"].map(String::from).to_vec();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 8));
        draw(&mut buf, 1, 1, &rows, bg, 's');
        let red = Some(Color::Indexed(196));
        assert_eq!(
            cell(&buf, 1, 1),
            ("▀".into(), red, Some(Color::Indexed(40)))
        );
        assert_eq!(
            cell(&buf, 2, 1),
            ("▀".into(), Some(bg), Some(Color::Indexed(16)))
        );
        assert_eq!(
            cell(&buf, 3, 1),
            ("▀".into(), Some(bg), Some(bg)),
            "past the art"
        );
        assert_eq!(
            cell(&buf, 1, 2),
            ("▀".into(), Some(bg), Some(bg)),
            "missing rows"
        );
        assert_eq!(cell(&buf, 25, 1).0, " ", "nothing drawn past THUMB_W");
    }

    #[test]
    fn no_art_draws_the_initial() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 6));
        draw(&mut buf, 0, 0, &[], Color::Indexed(233), 's');
        let row0: String = (0..24).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert_eq!(row0.trim(), "█".repeat(10), "the 5 wide S glyph, doubled");
    }

    #[test]
    fn palette_has_transparent_dot() {
        assert!(is_palette('.') && is_palette('g') && !is_palette('z') && !is_palette(' '));
        assert_eq!(color('.'), None);
    }
}
