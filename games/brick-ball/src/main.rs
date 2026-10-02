#[allow(dead_code)] // used by the game from Task 4 on, which removes this allow
mod board;
#[allow(dead_code)] // used by the game from Task 4 on, which removes this allow
mod stage;

use std::time::Duration;

use ratatui::{Frame, crossterm::event::KeyCode};

use arcade_sdk::{DrawCtx, Entry, Game, Input, Outcome, Status};

const ENTRY: Entry = Entry {
    id: "brick-ball",
    title: "brick ball",
    about: "break every brick · reach a stage to start there",
    starts: &[("enter", "start")],
    stages: true,
    start,
    min_size: (80, 24),
    thumb: &[], // Task 4 adds the art
};

fn main() -> anyhow::Result<()> {
    arcade_sdk::run(&ENTRY)
}

fn start(c: char, stage: u32) -> Option<Box<dyn Game>> {
    (c == '\n').then(|| Box::new(Stub { stage }) as Box<dyn Game>)
}

/// Ends at once with score 0 and draws nothing. Task 4 replaces it with the game.
struct Stub {
    stage: u32,
}

impl Game for Stub {
    fn key(&mut self, _key: KeyCode) {}

    fn tick(&mut self, _input: &Input) -> Status {
        Status::Over(Outcome {
            score: 0,
            variant: format!("stage {0}-{0}", self.stage),
            summary: String::new(),
        })
    }

    fn tick_rate(&self) -> Duration {
        Duration::from_millis(33)
    }

    fn draw(&self, _frame: &mut Frame, _ctx: &DrawCtx) {}

    fn reached(&self) -> u32 {
        self.stage
    }
}
