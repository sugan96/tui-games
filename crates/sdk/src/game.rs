//! The contract between the SDK shell and a game. A game is a binary crate
//! under games/ whose main is `arcade_sdk::run(&ENTRY)`.
//! The shell owns the game menu with its top 10, name entry, game over, the
//! score table and the terminal. The game owns its rules, its keys, its tick
//! speed, its animations and its drawing.

use std::time::Duration;

use ratatui::{Frame, crossterm::event::KeyCode};

use crate::{db::ScoreRow, input::Input};

/// One game in the launcher.
pub struct Entry {
    /// Key for the score table. Never change it after release or old scores disappear.
    pub id: &'static str,
    /// Drawn as the block letter logo on the game menu. Letters and spaces only.
    pub title: &'static str,
    /// One line on the game menu: how to play or how scoring works.
    pub about: &'static str,
    /// Start keys and what they start, as key chips on the game menu.
    pub starts: &'static [(&'static str, &'static str)],
    /// True: the game menu shows a stage picker and `start` gets the chosen
    /// stage. False: no picker, `start` gets 1 and `Game::reached` is never read.
    pub stages: bool,
    /// Without release events a press starts a hold for 400 ms. For continuous
    /// controls such as a paddle. Leave false for tap controls.
    pub hold_on_press: bool,
    /// Starts a game for a key pressed on the game menu and a stage. Enter
    /// arrives as '\n'. None for keys the game does not use. q is never passed.
    pub start: fn(char, u32) -> Option<Box<dyn Game>>,
    /// Smallest terminal, in columns and rows, that `Game::draw` fits in.
    pub min_size: (u16, u16),
    /// Pixel art for the launcher card: `ui::thumb::THUMB_H` rows of
    /// `ui::thumb::THUMB_W` palette letters, `.` for transparent. See `ui::thumb::color`.
    pub thumb: &'static [&'static str],
}

pub enum Status {
    Running,
    Over(Outcome),
}

pub struct Outcome {
    pub score: u32,
    /// Stored with the score and shown in the top table, for example "fixed 5".
    pub variant: String,
    /// One line on the game over screen, for example "▟▙ 4 apples".
    pub summary: String,
}

pub struct DrawCtx<'a> {
    /// Best named score of this game, for a HUD.
    pub best: Option<&'a ScoreRow>,
    /// True under the name entry and game over modals: draw muted and without key hints.
    pub dim: bool,
}

pub trait Game {
    /// A key press. Repeats and releases do not arrive here, use `Input::held`.
    /// q never arrives here.
    fn key(&mut self, key: KeyCode);
    /// Called once per `tick_rate`. Return `Over` once, after any death animation.
    /// The shell then records the outcome and never ticks this game again.
    fn tick(&mut self, input: &Input) -> Status;
    /// Time until the next tick. The shell asks again after every tick.
    fn tick_rate(&self) -> Duration;
    /// Draw into the whole frame, which is at least `Entry::min_size`.
    fn draw(&self, frame: &mut Frame, ctx: &DrawCtx);
    /// Highest stage this run has got to. The SDK raises the saved progress to
    /// it when the run ends and when the player quits with q during the run.
    /// The default 0 is for games without stages, which leave it out.
    fn reached(&self) -> u32 {
        0
    }
    /// The outcome of the run so far, for `q` during a run. Return it to have the
    /// run end with a game over and the score recorded; the default quits at once.
    fn give_up(&self) -> Option<Outcome> {
        None
    }
}
