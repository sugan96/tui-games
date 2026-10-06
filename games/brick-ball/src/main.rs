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
    hold_on_press: true,
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
        if key == KeyCode::Char(' ') && !self.paused {
            self.board.launch();
        }
    }

    /// Nothing changes while paused, so the stage clear countdown and the
    /// power-up timers stop too.
    fn tick(&mut self, input: &Input) -> Status {
        if self.paused {
            return Status::Running;
        }
        let keys = [
            (KeyCode::Left, -1),
            (KeyCode::Char('a'), -1),
            (KeyCode::Right, 1),
            (KeyCode::Char('d'), 1),
        ];
        let (dir, provisional) = keys
            .into_iter()
            .find(|&(k, _)| input.held(k))
            .map_or((0, false), |(k, dir)| (dir, input.provisional(k)));
        self.board.tick(dir, provisional);
        if self.board.over() {
            return Status::Over(Outcome {
                score: self.board.score,
                variant: self.board.variant(),
                summary: self.board.summary(),
            });
        }
        Status::Running
    }

    /// board::BLINK while dying, board::IDLE_TICK while paused, the board's
    /// tick_len otherwise.
    fn tick_rate(&self) -> Duration {
        match self.board.phase {
            Phase::Dying { .. } => board::BLINK,
            _ if self.paused => board::IDLE_TICK,
            _ => self.board.tick_len(),
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
        let mut i = Input::new(true, false);
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
        g.tick(&Input::new(true, false));
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
    fn provisional_hold_moves_at_the_start_speed() {
        let mut g = BrickBall::new(5, 1);
        let mut input = Input::new(false, true);
        input.event(KeyCode::Right, KeyEventKind::Press, Instant::now());
        for _ in 0..3 {
            let (cx, dt) = (g.board.paddle_cx, g.board.tick_len().as_secs_f64());
            g.tick(&input);
            let moved = g.board.paddle_cx - cx;
            assert!(
                (moved - board::PADDLE_SPEED_MIN * dt).abs() < 1e-9,
                "{moved}"
            );
        }
    }

    #[test]
    fn keys_are_ignored_while_dying() {
        let mut g = BrickBall::new(5, 1);
        g.board.phase = Phase::Dying { left: 6 };
        g.key(KeyCode::Char('p'));
        assert!(!g.paused);
        g.key(KeyCode::Char(' '));
        assert_eq!(g.board.phase, Phase::Dying { left: 6 });
    }

    #[test]
    fn over_carries_score_variant_summary_and_reached() {
        let mut g = BrickBall::new(5, 2);
        let b = &mut g.board;
        (b.phase, b.score, b.reached, b.bricks) = (Phase::Dying { left: 1 }, 40, 4, 3);
        let Status::Over(o) = g.tick(&Input::new(true, false)) else {
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
    fn tick_rate_follows_the_ball() {
        let mut g = BrickBall::new(5, 1);
        g.key(KeyCode::Char(' '));
        assert_eq!(g.board.phase, Phase::Play);
        assert_eq!(g.tick_rate(), g.board.tick_len());
        assert_ne!(g.tick_rate(), board::IDLE_TICK);
        g.key(KeyCode::Char('p'));
        assert_eq!(g.tick_rate(), board::IDLE_TICK);
        g.key(KeyCode::Char('p'));
        g.board.phase = Phase::Dying { left: 6 };
        assert_eq!(g.tick_rate(), board::BLINK);
    }
}
