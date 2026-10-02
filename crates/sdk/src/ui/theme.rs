//! One color or style per visual role. Screens import from here and nowhere else.
//! xterm 256 indexed colors. Indexes 16 to 255 render the same in every
//! terminal, so the game looks identical everywhere without truecolor.

use ratatui::style::{Color, Modifier, Style};

const fn c(i: u8) -> Color {
    Color::Indexed(i)
}
const fn fg(i: u8) -> Style {
    Style::new().fg(c(i))
}
const fn bold(i: u8) -> Style {
    Style::new().fg(c(i)).add_modifier(Modifier::BOLD)
}

// Text roles.
pub const TEXT: Style = fg(250);
pub const TEXT_BOLD: Style = bold(250);
pub const MUTED: Style = fg(244);
pub const DIM: Style = fg(238);
pub const CHROME: Style = fg(240);
pub const CHROME_DIM: Style = fg(237);
pub const CHIP: Style = Style::new()
    .fg(c(254))
    .bg(c(236))
    .add_modifier(Modifier::BOLD);
pub const ACCENT: Style = bold(173);
pub const ACCENT_HI: Style = bold(209);
pub const MARQUEE: Style = fg(173);
pub const MODAL_BORDER: Style = fg(173);
pub const AMBER: Style = bold(214);
/// The `STAGE n` chip of the menu stage picker.
pub const PICKER: Style = Style::new()
    .fg(c(233))
    .bg(c(214))
    .add_modifier(Modifier::BOLD);
pub const GOLD: Style = bold(220);
pub const PODIUM: [Style; 3] = [bold(220), bold(250), bold(172)];
pub const DANGER: Style = bold(196);
pub const METER_ON: Style = fg(82);
pub const METER_OFF: Style = fg(238);
pub const FOOD_TEXT: Style = fg(196);

// Cell roles, painted straight into the buffer.
pub const FIELD: [Color; 2] = [c(233), c(234)];
pub const MODAL_BG: Color = c(234);
pub const SHADOW: Color = c(236);
pub const DIM_C: Color = c(237);
pub const LOGO: [Color; 5] = [c(209), c(209), c(173), c(180), c(223)];
pub const HEAD: Color = c(226);
/// Body fades from the neck to the tail.
pub const BODY: [Color; 4] = [c(118), c(82), c(40), c(28)];
/// Food alternates between these every tick.
pub const FOOD: [Color; 2] = [c(196), c(203)];
pub const DEAD: Color = c(131);
pub const DEAD_HEAD: Color = c(167);
