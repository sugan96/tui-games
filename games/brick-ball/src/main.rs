mod board;
mod draw;
mod stage;

use std::time::Duration;

use ratatui::{Frame, crossterm::event::KeyCode};

use arcade_sdk::{DrawCtx, Entry, Game, Input, Outcome, Status};
use board::{Board, Phase};

const ENTRY: Entry = Entry {
    id: "brick-ball",
    title: "brick ball",
    about: "break every brick · reach a stage to start there",
    starts: &[("enter", "start")],
    stages: true,
    start,
    // The play screen fits 80 by 24. The SDK takes the larger of this and its
    // 64 by 28 game menu on each axis, so the program needs 80 by 28.
    min_size: (80, 24),
    // Four brick rows in red, orange, gold and lime with a gap in the lower two,
    // a 2 by 2 white ball, a 7 pixel terracotta paddle. From the mockup.
    thumb: &[
        "........................",
        ".rrrrr.rrrrr.rrrrr.rrrrr",
        ".ooooo.ooooo.ooooo.ooooo",
        ".YYYYY.YYYYY.......YYYYY",
        ".lllll.............lllll",
        "........................",
        "..............ww........",
        "..............ww........",
        "........................",
        "........................",
        "..........sssssss.......",
        "........................",
    ],
};

fn main() -> anyhow::Result<()> {
    arcade_sdk::run(&ENTRY)
}

/// Enter starts a run at the chosen stage with a fresh random run seed.
fn start(c: char, stage: u32) -> Option<Box<dyn Game>> {
    (c == '\n').then(|| Box::new(BrickBall::new(rand::random(), stage)) as Box<dyn Game>)
}

struct BrickBall {
    board: Board,
    paused: bool,
}

impl BrickBall {
    fn new(run_seed: u64, stage: u32) -> BrickBall {
        BrickBall {
            board: Board::new(run_seed, stage),
            paused: false,
        }
    }
}

impl Game for BrickBall {
    /// Keys are ignored while dying. While paused only p, which resumes, applies.
    fn key(&mut self, key: KeyCode) {
        if matches!(self.board.phase, Phase::Dying { .. } | Phase::Over) {
            return;
        }
        if key == KeyCode::Char('p') {
            self.paused = !self.paused;
            return;
        }
        if self.paused {
            return;
        }
        match key {
            KeyCode::Char(' ') => self.board.launch(),
            KeyCode::Left | KeyCode::Char('a') => self.board.press(-1),
            KeyCode::Right | KeyCode::Char('d') => self.board.press(1),
            _ => {}
        }
    }

    /// Nothing changes while paused, so the stage clear countdown and the
    /// power-up timers stop too.
    fn tick(&mut self, input: &Input) -> Status {
        if self.paused {
            return Status::Running;
        }
        let held = |a, b| input.held(a) || input.held(KeyCode::Char(b));
        let dir = if held(KeyCode::Left, 'a') {
            -1
        } else if held(KeyCode::Right, 'd') {
            1
        } else {
            0
        };
        self.board.tick(dir);
        if self.board.over() {
            return Status::Over(Outcome {
                score: self.board.score,
                variant: self.board.variant(),
                summary: self.board.summary(),
            });
        }
        Status::Running
    }

    /// board::BLINK while dying, board::TICK otherwise.
    fn tick_rate(&self) -> Duration {
        match self.board.phase {
            Phase::Dying { .. } => board::BLINK,
            _ => board::TICK,
        }
    }

    fn draw(&self, frame: &mut Frame, ctx: &DrawCtx) {
        draw::draw(frame, self, ctx);
    }

    fn reached(&self) -> u32 {
        self.board.reached
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::crossterm::event::KeyEventKind;

    use super::*;

    fn held_right() -> Input {
        let mut i = Input::new(true);
        i.event(KeyCode::Right, KeyEventKind::Press, Instant::now());
        i
    }

    #[test]
    fn meets_the_arcade_contract() {
        arcade_sdk::testkit::check(&ENTRY);
    }

    #[test]
    fn starts_only_on_enter_at_the_chosen_stage() {
        assert!(start('\n', 3).is_some());
        assert!(start(' ', 1).is_none());
        assert!(start('1', 1).is_none());
        assert_eq!(BrickBall::new(5, 3).board.stage, 3);
    }

    #[test]
    fn pause_freezes_the_board() {
        let mut g = BrickBall::new(5, 1);
        g.key(KeyCode::Char(' '));
        g.tick(&Input::new(true));
        let (cx, balls) = (g.board.paddle_cx, g.board.balls.clone());
        g.key(KeyCode::Char('p'));
        let input = held_right();
        for _ in 0..5 {
            assert!(matches!(g.tick(&input), Status::Running));
        }
        assert_eq!((g.board.paddle_cx, &g.board.balls), (cx, &balls));
        g.key(KeyCode::Char('p'));
        g.tick(&input);
        assert!(g.board.paddle_cx > cx);
    }

    #[test]
    fn keys_are_ignored_while_dying() {
        let mut g = BrickBall::new(5, 1);
        g.board.phase = Phase::Dying { left: 6 };
        let cx = g.board.paddle_cx;
        g.key(KeyCode::Char('p'));
        assert!(!g.paused);
        g.key(KeyCode::Right);
        g.tick(&Input::new(true));
        assert_eq!(g.board.paddle_cx, cx);
    }

    #[test]
    fn over_carries_score_variant_summary_and_reached() {
        let mut g = BrickBall::new(5, 2);
        let b = &mut g.board;
        (b.phase, b.score, b.reached, b.bricks) = (Phase::Dying { left: 1 }, 40, 4, 3);
        let Status::Over(o) = g.tick(&Input::new(true)) else {
            panic!("not over");
        };
        assert_eq!(
            (o.score, o.variant.as_str(), o.summary.as_str()),
            (40, "stage 2-4", "▬ 3 bricks")
        );
        assert_eq!(g.reached(), 4);
    }

    #[test]
    fn reached_reports_the_board_during_a_run() {
        let mut g = BrickBall::new(5, 3);
        assert_eq!(g.reached(), 3);
        g.board.reached = 6;
        assert_eq!(g.reached(), 6);
    }

    #[test]
    fn tap_moves_the_paddle_one_pixel() {
        let mut g = BrickBall::new(5, 1);
        let cx = g.board.paddle_cx;
        g.key(KeyCode::Right);
        g.tick(&Input::new(true));
        assert_eq!(g.board.paddle_cx - cx, 1.0);
    }
}
