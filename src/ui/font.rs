//! Block letter font, 5 rows tall. Digits are 3 wide, letters 5 wide.
//! Glyphs: 0 to 9, A to Z and space (3 wide). Anything else renders as ?.

pub const ROWS: usize = 5;

fn glyph(c: char) -> [&'static str; ROWS] {
    match c {
        '0' => ["███", "█ █", "█ █", "█ █", "███"],
        '1' => ["  █", "  █", "  █", "  █", "  █"],
        '2' => ["███", "  █", "███", "█  ", "███"],
        '3' => ["███", "  █", "███", "  █", "███"],
        '4' => ["█ █", "█ █", "███", "  █", "  █"],
        '5' => ["███", "█  ", "███", "  █", "███"],
        '6' => ["███", "█  ", "███", "█ █", "███"],
        '7' => ["███", "  █", "  █", "  █", "  █"],
        '8' => ["███", "█ █", "███", "█ █", "███"],
        '9' => ["███", "█ █", "███", "  █", "███"],
        'A' => [" ███ ", "█   █", "█████", "█   █", "█   █"],
        'B' => ["████ ", "█   █", "████ ", "█   █", "████ "],
        'C' => [" ████", "█    ", "█    ", "█    ", " ████"],
        'D' => ["████ ", "█   █", "█   █", "█   █", "████ "],
        'E' => ["█████", "█    ", "████ ", "█    ", "█████"],
        'F' => ["█████", "█    ", "████ ", "█    ", "█    "],
        'G' => [" ████", "█    ", "█  ██", "█   █", " ████"],
        'H' => ["█   █", "█   █", "█████", "█   █", "█   █"],
        'I' => ["█████", "  █  ", "  █  ", "  █  ", "█████"],
        'J' => ["█████", "   █ ", "   █ ", "█  █ ", " ██  "],
        'K' => ["█   █", "█  █ ", "███  ", "█  █ ", "█   █"],
        'L' => ["█    ", "█    ", "█    ", "█    ", "█████"],
        'M' => ["█   █", "██ ██", "█ █ █", "█   █", "█   █"],
        'N' => ["█   █", "██  █", "█ █ █", "█  ██", "█   █"],
        'O' => [" ███ ", "█   █", "█   █", "█   █", " ███ "],
        'P' => ["████ ", "█   █", "████ ", "█    ", "█    "],
        'Q' => [" ███ ", "█   █", "█ █ █", "█  █ ", " ██ █"],
        'R' => ["████ ", "█   █", "████ ", "█  █ ", "█   █"],
        'S' => ["█████", "█    ", "█████", "    █", "█████"],
        'T' => ["█████", "  █  ", "  █  ", "  █  ", "  █  "],
        'U' => ["█   █", "█   █", "█   █", "█   █", " ███ "],
        'V' => ["█   █", "█   █", "█   █", " █ █ ", "  █  "],
        'W' => ["█   █", "█   █", "█ █ █", "██ ██", "█   █"],
        'X' => ["█   █", " █ █ ", "  █  ", " █ █ ", "█   █"],
        'Y' => ["█   █", " █ █ ", "  █  ", "  █  ", "  █  "],
        'Z' => ["█████", "   █ ", "  █  ", " █   ", "█████"],
        ' ' => ["   ", "   ", "   ", "   ", "   "],
        _ => ["   ", "   ", " ? ", "   ", "   "],
    }
}

/// Render text with square pixels: each pixel is two characters wide, one
/// space between glyphs. A terminal cell is half as wide as it is tall, so
/// this is what makes digits read as blocks instead of slivers.
pub fn render_wide(text: &str) -> [String; ROWS] {
    let mut out: [String; ROWS] = Default::default();
    for (i, c) in text.chars().enumerate() {
        let g = glyph(c);
        for (row, line) in out.iter_mut().enumerate() {
            if i > 0 {
                line.push(' ');
            }
            for px in g[row].chars() {
                line.push(px);
                line.push(px);
            }
        }
    }
    out
}

/// Render text as ROWS lines, one space between glyphs.
pub fn render(text: &str) -> [String; ROWS] {
    let mut out: [String; ROWS] = Default::default();
    for (i, c) in text.chars().enumerate() {
        let g = glyph(c);
        for (row, line) in out.iter_mut().enumerate() {
            if i > 0 {
                line.push(' ');
            }
            line.push_str(g[row]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_render_to_fixed_shapes() {
        assert_eq!(render("0"), ["███", "█ █", "█ █", "█ █", "███"]);
        assert_eq!(
            render("10"),
            ["  █ ███", "  █ █ █", "  █ █ █", "  █ █ █", "  █ ███"]
        );
        for d in '0'..='9' {
            let g = render(&d.to_string());
            assert!(
                g.iter().all(|l| l.chars().count() == 3),
                "{d} is not 3 wide"
            );
            assert!(g.iter().any(|l| l.contains('█')), "{d} is blank");
        }
    }

    #[test]
    fn wide_doubles_every_pixel() {
        assert_eq!(
            render_wide("14"),
            [
                "    ██ ██  ██",
                "    ██ ██  ██",
                "    ██ ██████",
                "    ██     ██",
                "    ██     ██"
            ]
        );
    }

    #[test]
    fn letters_are_five_wide_and_distinct() {
        let all: Vec<_> = ('A'..='Z').map(|c| render(&c.to_string())).collect();
        for (i, g) in all.iter().enumerate() {
            assert!(
                g.iter().all(|l| l.chars().count() == 5),
                "{i} is not 5 wide"
            );
            assert!(all[i + 1..].iter().all(|o| o != g), "{i} is a duplicate");
        }
        assert_eq!(render("A B")[0], " ███      ████ ");
    }

    #[test]
    fn digits_are_distinct() {
        let all: Vec<_> = ('0'..='9').map(|d| render(&d.to_string())).collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
