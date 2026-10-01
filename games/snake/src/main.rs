mod board;
mod draw;

use std::time::Duration;

use ratatui::{Frame, crossterm::event::KeyCode};

use arcade_sdk::{DrawCtx, Entry, Game, Input, Outcome, Status};
use board::{Board, Mode};

const ENTRY: Entry = Entry {
    id: "snake",
    title: "snake",
    about: "an apple is worth the current level in points",
    starts: &[("1-9", "fixed speed"), ("v", "variable")],
    start,
    min_size: (draw::TOTAL_W, draw::BOARD_H),
    // Lime near the head fading to dark green at the tail, an apple ahead.
    thumb: &[
        "........................",
        "....................G...",
        "..llllllllllllyk...wrr..",
        "..llllllllllllyy...rrr..",
        "..ll...............rrr..",
        "..ll....................",
        "..gggggggggggggggggg....",
        "..gggggggggggggggggg....",
        "..................gg....",
        "..................gg....",
        "......GGGGGGGGGGGGGG....",
        "......GGGGGGGGGGGGGG....",
    ],
};

fn main() -> anyhow::Result<()> {
    arcade_sdk::run(&ENTRY)
}

/// Half-periods of the death blink and the length of each.
const BLINKS: u8 = 6;
const BLINK_TICK: Duration = Duration::from_millis(200);

fn start(c: char) -> Option<Box<dyn Game>> {
    let mode = match c {
        '1'..='9' => Mode::Fixed(c as u8 - b'0'),
        'v' => Mode::Variable,
        _ => return None,
    };
    Some(Box::new(Snake::new(mode)))
}

struct Snake {
    board: Board,
    /// False for the first half tick after a step, so the renderer draws the
    /// snake halfway to its new cells. True once the half tick has passed.
    settled: bool,
    /// True while the arrow for the current direction is held. Halves the step time.
    boost: bool,
    /// Some while the dead snake blinks: half-periods left before Over.
    dying: Option<u8>,
    /// Steps taken. Drives the food blink.
    steps: u64,
}

impl Snake {
    fn new(mode: Mode) -> Snake {
        Snake {
            board: Board::new(mode),
            settled: true,
            boost: false,
            dying: None,
            steps: 0,
        }
    }

    /// Time per cell, halved while boosting.
    fn step_time(&self) -> Duration {
        if self.boost {
            self.board.tick() / 2
        } else {
            self.board.tick()
        }
    }
}

impl Game for Snake {
    fn key(&mut self, key: KeyCode) {
        if self.dying.is_none()
            && let Some(d) = arrow_dir(key)
        {
            self.board.turn(d);
        }
    }

    /// Alternates a step with a render-only half tick that settles the
    /// interpolated frame. A collision starts the death blink, which ends in Over.
    fn tick(&mut self, input: &Input) -> Status {
        self.boost = self.dying.is_none() && input.held(dir_key(self.board.dir));
        match &mut self.dying {
            Some(left) if *left > 1 => *left -= 1,
            Some(_) => {
                return Status::Over(Outcome {
                    score: self.board.score,
                    variant: self.board.mode.label(),
                    summary: format!("▟▙ {} apples", self.board.apples),
                });
            }
            None if !self.settled => self.settled = true,
            None => {
                self.board.step();
                self.steps += 1;
                self.settled = false;
                if self.board.over {
                    self.dying = Some(BLINKS);
                }
            }
        }
        Status::Running
    }

    /// Half the step time while alive, BLINK_TICK while dying.
    fn tick_rate(&self) -> Duration {
        match self.dying {
            Some(_) => BLINK_TICK,
            None => self.step_time() / 2,
        }
    }

    fn draw(&self, frame: &mut Frame, ctx: &DrawCtx) {
        draw::draw(frame, self, ctx);
    }
}

/// Grid direction of an arrow key, y up.
fn arrow_dir(key: KeyCode) -> Option<(i32, i32)> {
    match key {
        KeyCode::Up => Some((0, 1)),
        KeyCode::Down => Some((0, -1)),
        KeyCode::Left => Some((-1, 0)),
        KeyCode::Right => Some((1, 0)),
        _ => None,
    }
}

/// Arrow key for a grid direction, the inverse of arrow_dir.
fn dir_key(dir: (i32, i32)) -> KeyCode {
    match dir {
        (0, 1) => KeyCode::Up,
        (0, -1) => KeyCode::Down,
        (-1, 0) => KeyCode::Left,
        _ => KeyCode::Right,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::crossterm::event::KeyEventKind;

    use super::*;
    use board::H;

    fn snake(level: u8) -> Snake {
        Snake::new(Mode::Fixed(level))
    }

    fn input() -> Input {
        Input::new(true)
    }

    fn outcome(s: &mut Snake) -> Option<Outcome> {
        match s.tick(&input()) {
            Status::Over(o) => Some(o),
            Status::Running => None,
        }
    }

    #[test]
    fn meets_the_arcade_contract() {
        arcade_sdk::testkit::check(&ENTRY);
    }

    #[test]
    fn start_keys_pick_the_mode() {
        assert!(start('5').is_some());
        assert!(start('v').is_some());
        assert!(start('0').is_none());
        assert!(start('x').is_none());
    }

    #[test]
    fn arrows_turn_but_not_into_a_reversal() {
        let mut s = snake(5);
        s.key(KeyCode::Up);
        assert_eq!(s.board.dir, (0, 1));
        // Left would reverse the start direction (1, 0), so step once first.
        s.tick(&input());
        s.key(KeyCode::Left);
        assert_eq!(s.board.dir, (-1, 0));
        // The last step was (0, 1), so Down is a reversal and is ignored.
        s.key(KeyCode::Down);
        assert_eq!(s.board.dir, (-1, 0));
    }

    #[test]
    fn ticks_alternate_step_and_half_tick() {
        let mut s = snake(5);
        assert_eq!(s.tick_rate(), s.board.tick() / 2);
        let head = s.board.snake[0];
        s.tick(&input());
        assert!(!s.settled);
        assert_ne!(s.board.snake[0], head);
        let head = s.board.snake[0];
        s.tick(&input());
        assert!(s.settled);
        assert_eq!(s.board.snake[0], head, "half tick does not move the snake");
    }

    #[test]
    fn death_blinks_then_ends_with_the_outcome() {
        let mut s = snake(5);
        s.board.score = 7;
        s.board.apples = 2;
        s.board.food = (0, 0); // off the path of the head column
        s.board.dir = (0, 1); // head starts at y = H / 2, the top wall at y = H is nearest
        for _ in 0..H * 2 {
            assert!(outcome(&mut s).is_none());
            if s.dying.is_some() {
                break;
            }
        }
        assert_eq!(s.dying, Some(BLINKS));
        assert_eq!(s.tick_rate(), BLINK_TICK);
        let head = s.board.snake[0];
        s.key(KeyCode::Left);
        for _ in 0..BLINKS - 1 {
            assert!(outcome(&mut s).is_none());
        }
        assert_eq!(s.board.snake[0], head, "steering is ignored while dying");
        let o = outcome(&mut s).expect("over after the blink");
        assert_eq!((o.score, o.variant.as_str()), (7, "fixed 5"));
        assert_eq!(o.summary, "▟▙ 2 apples");
    }

    #[test]
    fn holding_the_current_direction_halves_the_tick() {
        let mut s = snake(5);
        let normal = s.tick_rate();
        let mut i = input();
        let t = Instant::now();
        i.event(KeyCode::Up, KeyEventKind::Press, t);
        s.tick(&i);
        assert!(!s.boost, "held arrow is not the current direction");
        i.event(KeyCode::Right, KeyEventKind::Press, t);
        s.tick(&i);
        assert_eq!(s.tick_rate(), normal / 2);
        i.event(KeyCode::Right, KeyEventKind::Release, t);
        s.tick(&i);
        assert_eq!(s.tick_rate(), normal);
    }
}
