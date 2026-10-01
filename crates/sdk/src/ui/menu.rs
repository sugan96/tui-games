use ratatui::{
    Frame,
    buffer::Buffer,
    style::{Color, Style},
    text::Span,
};

use super::{center_line, centered, font, panel, theme};
use crate::{app::App, db::TOP_N};

pub const WIDTH: u16 = 64;
pub const HEIGHT: u16 = 28;

/// The game menu: logo, top table, start prompt and about line.
pub fn draw(frame: &mut Frame, app: &App) {
    let entry = app.entry;
    let area = centered(frame.area(), WIDTH, HEIGHT);
    let mut hints = entry.starts.to_vec();
    hints.push(("q", "quit"));
    let block = panel(theme::CHROME, &hints);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (x, y) = (area.x, area.y);

    if app.ticks % 4 != 3 {
        center_line(
            frame,
            inner,
            y + 22,
            vec![Span::styled(prompt(entry.starts), theme::AMBER)],
        );
    }
    center_line(
        frame,
        inner,
        y + 24,
        vec![Span::styled(entry.about, theme::MUTED)],
    );

    let buf = frame.buffer_mut();
    let logo = font::render(&entry.title.to_ascii_uppercase());
    let lx = x + WIDTH.saturating_sub(logo[0].chars().count() as u16) / 2;
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

    let tx = x + 11;
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
}

/// "PRESS 1-9 OR V TO START" from the start keys.
fn prompt(starts: &[(&str, &str)]) -> String {
    let keys: Vec<String> = starts.iter().map(|(k, _)| k.to_uppercase()).collect();
    let keys = match keys.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} OR {last}", rest.join(", ")),
        None => String::new(),
    };
    format!("PRESS {keys} TO START")
}

/// Write the filled cells of a block-font row, leaving its gaps untouched.
fn blit(buf: &mut Buffer, x: u16, y: u16, row: &str, color: Color) {
    for (i, ch) in row.chars().enumerate() {
        if ch != ' ' {
            buf.set_string(x + i as u16, y, "█", Style::new().fg(color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_joins_keys() {
        assert_eq!(prompt(&[("v", "")]), "PRESS V TO START");
        assert_eq!(prompt(&[("1-9", ""), ("v", "")]), "PRESS 1-9 OR V TO START");
        assert_eq!(
            prompt(&[("a", ""), ("b", ""), ("c", "")]),
            "PRESS A, B OR C TO START"
        );
    }
}
