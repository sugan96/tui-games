//! Block letter font, 5 rows tall. Digits are 3 wide, letters 5 wide.
//! Only the glyphs the screens use exist: 0 to 9 and the letters of SNAKE.

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
        'S' => ["█████", "█    ", "█████", "    █", "█████"],
        'N' => ["█   █", "██  █", "█ █ █", "█  ██", "█   █"],
        'A' => [" ███ ", "█   █", "█████", "█   █", "█   █"],
        'K' => ["█   █", "█  █ ", "███  ", "█  █ ", "█   █"],
        'E' => ["█████", "█    ", "████ ", "█    ", "█████"],
        _ => ["   ", "   ", " ? ", "   ", "   "],
    }
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
    fn digits_are_distinct() {
        let all: Vec<_> = ('0'..='9').map(|d| render(&d.to_string())).collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
