//! Rules and physics of brick ball. No terminal code here.

use std::time::Duration;

use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::stage::{BASE_SPEED, Cell, Grid, Levers, ROWS, build, effective, levers};

pub const W: f64 = 60.0; // pixels
pub const H: f64 = 44.0;
pub const BRICK_W: f64 = 4.0;
pub const BRICK_TOP: f64 = 4.0; // y of brick row 0
pub const PADDLE_Y: f64 = 42.0;
pub const TICK: Duration = Duration::from_millis(33);
pub const DT: f64 = TICK.as_secs_f64(); // seconds per tick
pub const BLINK: Duration = Duration::from_millis(200);
pub const BLINKS: u8 = 6;
/// No sub-step moves a ball further than this, in pixels.
pub const MAX_STEP: f64 = 0.5;
pub const MAX_ANGLE_DEG: f64 = 60.0;
/// Launch direction: straight up tilted by a uniform angle in -LAUNCH_TILT_DEG..LAUNCH_TILT_DEG. Plan decision.
pub const LAUNCH_TILT_DEG: f64 = 10.0;
/// Multi ball copies leave at this angle from vertical. Plan decision.
pub const SPLIT_DEG: f64 = 45.0;
pub const START_LIVES: u32 = 3;
pub const MAX_LIVES: u32 = 5;
/// Clearing a stage whose number is a multiple of this adds a life.
pub const LIFE_EVERY: u32 = 5;
pub const MAX_BALLS: usize = 9;
pub const CAPSULE_CHANCE: f64 = 0.08;
pub const CAPSULE_SPEED: f64 = 20.0; // px/s
pub const WIDE_PX: u32 = 3;
pub const SLOW_FACTOR: f64 = 0.75;
/// Timers in ticks: whole ticks in the duration, rounded down.
pub const WIDE_TICKS: u32 = ticks(Duration::from_secs(15)); // 454
pub const SLOW_TICKS: u32 = ticks(Duration::from_secs(10)); // 303
pub const CLEAR_TICKS: u32 = ticks(Duration::from_secs(2)); // 60
/// Hold ticks for the paddle step to rise from 1 to 3 pixels.
pub const HOLD_RAMP: u32 = 6;
/// A ball that hits neither the paddle nor a breakable brick for this long
/// turns by a random angle in -STALL_TURN_DEG..STALL_TURN_DEG, so it cannot
/// loop forever between walls and unbreakable bricks.
pub const STALL_TICKS: u32 = ticks(Duration::from_secs(10)); // 303
pub const STALL_TURN_DEG: f64 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball {
    pub x: f64,
    pub y: f64,
    /// (dx, dy) is a unit vector.
    pub dx: f64,
    pub dy: f64,
    /// Ticks since the last paddle or breakable brick hit.
    stall: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Wide,
    Slow,
    Multi,
}

impl Power {
    /// 'W', 'S' or 'M'.
    pub fn letter(self) -> char {
        match self {
            Power::Wide => 'W',
            Power::Slow => 'S',
            Power::Multi => 'M',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub kind: Power,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Ball waits on the paddle for space.
    Serve,
    Play,
    /// Stage cleared, counting down to the next stage.
    Clear {
        ticks: u32,
    },
    /// Last life lost: half-periods of the paddle blink left.
    Dying {
        left: u8,
    },
    /// The blink finished. The adapter returns Status::Over.
    Over,
}

pub struct Board {
    pub run_seed: u64,
    pub start: u32,
    pub stage: u32,
    pub reached: u32,
    pub levers: Levers,
    pub cells: Grid,
    pub balls: Vec<Ball>,
    /// Paddle center x in pixels.
    pub paddle_cx: f64,
    pub capsule: Option<Capsule>,
    /// Ticks left on each timed power-up, 0 when off.
    pub wide: u32,
    pub slow: u32,
    pub lives: u32,
    pub score: u32,
    /// Bricks broken in the run.
    pub bricks: u32,
    /// Brick points scored in the current stage.
    pub stage_points: u32,
    pub phase: Phase,
    hold: u32, // ticks of the current paddle hold
    hold_dir: i32,
    pressed: i32, // direction of a single press since the last tick, -1, 0 or 1
    rng: StdRng,
}

/// d.as_millis() / TICK.as_millis(), as u32.
const fn ticks(d: Duration) -> u32 {
    (d.as_millis() / TICK.as_millis()) as u32
}

/// Unit direction leaving the paddle for t in -1..=1, with a = MAX_ANGLE_DEG.to_radians() × t:
/// (sin a, -cos a).
pub fn bounce_dir(t: f64) -> (f64, f64) {
    let a = MAX_ANGLE_DEG.to_radians() * t;
    (a.sin(), -a.cos())
}

/// Sub-steps for a move of `dist` pixels: max(1, ceil(dist / MAX_STEP)).
pub fn substeps(dist: f64) -> u32 {
    ((dist / MAX_STEP).ceil() as u32).max(1)
}

/// Paddle pixels moved on hold tick k (1 based):
/// 1 + 2 × (min(k, HOLD_RAMP) - 1) / (HOLD_RAMP - 1).
pub fn paddle_step(k: u32) -> f64 {
    1.0 + 2.0 * (k.min(HOLD_RAMP) - 1) as f64 / (HOLD_RAMP - 1) as f64
}

/// Whole seconds shown for `ticks` left: ceil(ticks × TICK.as_millis() / 1000).
pub fn secs(ticks: u32) -> u32 {
    (ticks as u128 * TICK.as_millis()).div_ceil(1000) as u32
}

impl Board {
    /// Stage `start` of a run: layout from stage::build(run_seed, start), lives
    /// START_LIVES, paddle centered, ball on the paddle, reached = start,
    /// rng = StdRng::seed_from_u64(run_seed).
    pub fn new(run_seed: u64, start: u32) -> Board {
        let mut board = Board {
            run_seed,
            start,
            stage: start,
            reached: start,
            levers: levers(effective(start)),
            cells: build(run_seed, start),
            balls: Vec::new(),
            paddle_cx: W / 2.0,
            capsule: None,
            wide: 0,
            slow: 0,
            lives: START_LIVES,
            score: 0,
            bricks: 0,
            stage_points: 0,
            phase: Phase::Serve,
            hold: 0,
            hold_dir: 0,
            pressed: 0,
            rng: StdRng::seed_from_u64(run_seed),
        };
        board.serve();
        board
    }

    /// Paddle width in pixels: levers.paddle, plus WIDE_PX while wide > 0.
    pub fn paddle_w(&self) -> f64 {
        let wide = if self.wide > 0 { WIDE_PX } else { 0 };
        (self.levers.paddle + wide) as f64
    }

    /// Paddle left edge: paddle_cx - paddle_w / 2.
    pub fn paddle_left(&self) -> f64 {
        self.paddle_cx - self.paddle_w() / 2.0
    }

    /// Ball speed in px/s: levers.speed, times SLOW_FACTOR while slow > 0.
    pub fn ball_speed(&self) -> f64 {
        let slow = if self.slow > 0 { SLOW_FACTOR } else { 1.0 };
        self.levers.speed * slow
    }

    /// ball_speed / stage::BASE_SPEED, for the HUD.
    pub fn speed_multiplier(&self) -> f64 {
        self.ball_speed() / BASE_SPEED
    }

    /// Space: Serve becomes Play with the launch tilt. No-op in other phases.
    pub fn launch(&mut self) {
        if self.phase != Phase::Serve {
            return;
        }
        let a = self
            .rng
            .random_range(-LAUNCH_TILT_DEG..LAUNCH_TILT_DEG)
            .to_radians();
        let ball = &mut self.balls[0];
        (ball.dx, ball.dy) = (a.sin(), -a.cos());
        self.phase = Phase::Play;
    }

    /// A single key press of direction dir (-1 or 1). The next tick moves the
    /// paddle 1 pixel even when no hold is reported. Ignored while dying.
    pub fn press(&mut self, dir: i32) {
        if !matches!(self.phase, Phase::Dying { .. } | Phase::Over) {
            self.pressed = dir;
        }
    }

    /// One tick. dir is -1, 0 or 1 for the paddle. Ignored while dying.
    pub fn tick(&mut self, dir: i32) {
        match self.phase {
            Phase::Over => {}
            Phase::Dying { left } if left > 1 => self.phase = Phase::Dying { left: left - 1 },
            Phase::Dying { .. } => self.phase = Phase::Over,
            Phase::Clear { ticks } if ticks > 1 => self.phase = Phase::Clear { ticks: ticks - 1 },
            Phase::Clear { .. } => self.next_stage(),
            Phase::Serve => {
                self.move_paddle(dir);
                self.balls[0] = self.rest_ball();
            }
            Phase::Play => self.play(dir),
        }
    }

    /// phase == Over.
    pub fn over(&self) -> bool {
        self.phase == Phase::Over
    }

    pub fn breakable_left(&self) -> usize {
        self.cells
            .iter()
            .flatten()
            .filter(|cell| matches!(cell, Cell::Brick(_)))
            .count()
    }

    /// First multiple of LIFE_EVERY above the current stage:
    /// (stage / LIFE_EVERY + 1) × LIFE_EVERY.
    pub fn next_life_stage(&self) -> u32 {
        (self.stage / LIFE_EVERY + 1) * LIFE_EVERY
    }

    /// "stage {start}-{reached}".
    pub fn variant(&self) -> String {
        format!("stage {}-{}", self.start, self.reached)
    }

    /// "▬ {bricks} bricks".
    pub fn summary(&self) -> String {
        format!("▬ {} bricks", self.bricks)
    }

    fn play(&mut self, dir: i32) {
        self.move_paddle(dir);

        self.slow = self.slow.saturating_sub(1);
        if self.wide > 0 {
            self.wide -= 1;
            if self.wide == 0 {
                self.clamp_paddle();
            }
        }

        if let Some(mut cap) = self.capsule {
            let before = cap.y;
            cap.y += CAPSULE_SPEED * DT;
            // Caught only on the tick y first reaches PADDLE_Y: y < PADDLE_Y
            // before the move and y >= PADDLE_Y after it, over the paddle.
            if before < PADDLE_Y && cap.y >= PADDLE_Y && self.over_paddle(cap.x) {
                self.capsule = None;
                self.apply(cap.kind);
            } else if cap.y >= H {
                self.capsule = None;
            } else {
                self.capsule = Some(cap);
            }
        }

        for i in 0..self.balls.len() {
            if self.phase != Phase::Play {
                break;
            }
            self.move_ball(i);
        }
        // A clear ends the tick, so it never costs a life.
        if self.phase != Phase::Play {
            return;
        }

        self.balls.retain(|b| b.y < H);
        if self.balls.is_empty() {
            self.lose_life();
        }
    }

    fn move_paddle(&mut self, mut dir: i32) {
        if dir == 0 {
            dir = self.pressed;
        }
        self.pressed = 0;
        if dir == 0 {
            self.hold = 0;
            self.hold_dir = 0;
            return;
        }
        if dir == self.hold_dir {
            self.hold += 1;
        } else {
            self.hold = 1;
            self.hold_dir = dir;
        }
        self.paddle_cx += dir as f64 * paddle_step(self.hold);
        self.clamp_paddle();
    }

    /// Keeps the paddle inside the walls. Called after every move and width change.
    fn clamp_paddle(&mut self) {
        let half = self.paddle_w() / 2.0;
        self.paddle_cx = self.paddle_cx.clamp(half, W - half);
    }

    /// True when x is over the paddle: paddle_left <= x < paddle_left + paddle_w.
    fn over_paddle(&self, x: f64) -> bool {
        let left = self.paddle_left();
        left <= x && x < left + self.paddle_w()
    }

    /// The ball waiting on the paddle.
    fn rest_ball(&self) -> Ball {
        Ball {
            x: self.paddle_cx,
            y: PADDLE_Y - 0.5,
            dx: 0.0,
            dy: -1.0,
            stall: 0,
        }
    }

    /// Ball on the paddle, falling capsule removed. The W and S timers carry on.
    fn serve(&mut self) {
        self.phase = Phase::Serve;
        self.balls = vec![self.rest_ball()];
        self.capsule = None;
    }

    fn next_stage(&mut self) {
        self.stage += 1;
        self.reached = self.reached.max(self.stage);
        self.levers = levers(effective(self.stage));
        self.cells = build(self.run_seed, self.stage);
        self.stage_points = 0;
        self.clamp_paddle();
        self.serve();
    }

    fn lose_life(&mut self) {
        self.lives -= 1;
        if self.lives > 0 {
            self.serve();
        } else {
            self.capsule = None;
            self.phase = Phase::Dying { left: BLINKS };
        }
    }

    /// Moves ball i one tick in sub-steps of at most MAX_STEP pixels.
    fn move_ball(&mut self, i: usize) {
        let dist = self.ball_speed() * DT;
        let n = substeps(dist);
        let step = dist / n as f64;
        let mut b = self.balls[i];
        b.stall += 1;
        for _ in 0..n {
            let (ox, oy) = (b.x, b.y);
            b.x += b.dx * step;
            b.y += b.dy * step;

            if b.x < 0.0 {
                b.x = -b.x;
                b.dx = b.dx.abs();
            } else if b.x >= W {
                b.x = 2.0 * W - b.x - 1e-9;
                b.dx = -b.dx.abs();
            }
            if b.y < 0.0 {
                b.y = -b.y;
                b.dy = b.dy.abs();
            }

            let row = b.y.floor() - BRICK_TOP;
            if (0.0..ROWS as f64).contains(&row) {
                let (c, r) = ((b.x / BRICK_W) as usize, row as usize);
                let cell = self.cells[r][c];
                if cell != Cell::Empty {
                    // Bricks are 1 pixel tall, so a change of pixel row means
                    // the ball entered through the top or bottom.
                    if oy.floor() != b.y.floor() {
                        b.dy = -b.dy;
                    } else {
                        b.dx = -b.dx;
                    }
                    (b.x, b.y) = (ox, oy);
                    if matches!(cell, Cell::Brick(_)) {
                        b.stall = 0;
                        self.hit(c, r);
                        if self.phase != Phase::Play {
                            return;
                        }
                    }
                }
            }

            if b.dy > 0.0
                && oy.floor() < PADDLE_Y
                && b.y.floor() == PADDLE_Y
                && self.over_paddle(b.x)
            {
                let t = (2.0 * (b.x - self.paddle_left()) / self.paddle_w() - 1.0).clamp(-1.0, 1.0);
                (b.x, b.y) = (ox, oy);
                (b.dx, b.dy) = bounce_dir(t);
                b.stall = 0;
            }
        }
        // The turn keeps the angle within MAX_ANGLE_DEG of vertical, so |dy|
        // stays at 0.5 or more, and keeps the vertical direction.
        if b.stall >= STALL_TICKS {
            let turn = self.rng.random_range(-STALL_TURN_DEG..STALL_TURN_DEG);
            let a = (b.dx.atan2(b.dy.abs()).to_degrees() + turn)
                .clamp(-MAX_ANGLE_DEG, MAX_ANGLE_DEG)
                .to_radians();
            (b.dx, b.dy) = (a.sin(), b.dy.signum() * a.cos());
            b.stall = 0;
        }
        self.balls[i] = b;
    }

    /// One hit on cell (c, r): score, break, capsule roll, clear check.
    fn hit(&mut self, c: usize, r: usize) {
        let Cell::Brick(h) = self.cells[r][c] else {
            return;
        };
        let points = 10 + self.stage;
        self.score += points;
        self.stage_points += points;
        if h > 1 {
            self.cells[r][c] = Cell::Brick(h - 1);
            return;
        }
        self.cells[r][c] = Cell::Empty;
        self.bricks += 1;
        if self.capsule.is_none() && self.rng.random_bool(CAPSULE_CHANCE) {
            let kinds = [Power::Wide, Power::Slow, Power::Multi];
            self.capsule = Some(Capsule {
                kind: kinds[self.rng.random_range(0..kinds.len())],
                x: BRICK_W * c as f64 + BRICK_W / 2.0,
                y: BRICK_TOP + r as f64 + 0.5,
            });
        }
        if self.breakable_left() == 0 {
            self.score += 100 * self.stage;
            if self.stage.is_multiple_of(LIFE_EVERY) {
                self.lives = (self.lives + 1).min(MAX_LIVES);
            }
            self.balls.clear();
            self.capsule = None;
            self.phase = Phase::Clear { ticks: CLEAR_TICKS };
        }
    }

    /// The effect of a caught capsule.
    fn apply(&mut self, p: Power) {
        match p {
            Power::Wide => {
                self.wide = WIDE_TICKS;
                self.clamp_paddle();
            }
            Power::Slow => self.slow = SLOW_TICKS,
            Power::Multi => {
                let (s, c) = SPLIT_DEG.to_radians().sin_cos();
                for i in 0..self.balls.len() {
                    let b = self.balls[i];
                    for dx in [-s, s] {
                        if self.balls.len() < MAX_BALLS {
                            self.balls.push(Ball {
                                dx,
                                dy: b.dy.signum() * c,
                                ..b
                            });
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::COLS;

    fn ball(x: f64, y: f64, dx: f64, dy: f64) -> Ball {
        Ball {
            x,
            y,
            dx,
            dy,
            stall: 0,
        }
    }

    /// Board::new(1, stage) with every cell Empty, in Play, with one ball at
    /// (30, 20) moving straight up.
    fn empty(stage: u32) -> Board {
        let mut b = Board::new(1, stage);
        b.cells = [[Cell::Empty; COLS]; ROWS];
        b.phase = Phase::Play;
        b.balls = vec![ball(30.0, 20.0, 0.0, -1.0)];
        b
    }

    /// Ticks once and returns how far the paddle moved.
    fn moved(b: &mut Board, dir: i32) -> f64 {
        let before = b.paddle_cx;
        b.tick(dir);
        b.paddle_cx - before
    }

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn paddle_angle_is_60_degrees_times_t() {
        for (t, want) in [
            (0.0, (0.0, -1.0)),
            (1.0, (0.866, -0.5)),
            (-1.0, (-0.866, -0.5)),
        ] {
            let (dx, dy) = bounce_dir(t);
            assert!(
                close(dx, want.0, 1e-3) && close(dy, want.1, 1e-3),
                "t {t}: {dx} {dy}"
            );
        }
    }

    #[test]
    fn ball_at_the_left_edge_leaves_at_minus_60() {
        let mut b = empty(1);
        b.balls = vec![ball(b.paddle_left() + 0.01, 41.9, 0.0, 1.0)];
        b.tick(0);
        let (dx, dy) = bounce_dir(-1.0);
        let got = b.balls[0];
        assert!(
            close(got.dx, dx, 0.02) && close(got.dy, dy, 0.02),
            "{got:?}"
        );
    }

    #[test]
    fn paddle_center_sends_the_ball_up_and_the_right_edge_at_60() {
        let mut b = empty(1);
        let (left, w) = (b.paddle_left(), b.paddle_w());
        b.balls = vec![
            ball(left + w / 2.0, 41.9, 0.0, 1.0),
            ball(left + w - 0.01, 41.9, 0.0, 1.0),
        ];
        b.tick(0);
        for (got, t) in b.balls.iter().zip([0.0, 1.0]) {
            let (dx, dy) = bounce_dir(t);
            assert!(
                close(got.dx, dx, 0.02) && close(got.dy, dy, 0.02),
                "t {t}: {got:?}"
            );
        }
    }

    #[test]
    fn launch_tilts_at_most_ten_degrees() {
        let mut most: f64 = 0.0;
        for seed in 0..200 {
            let mut b = Board::new(seed, 1);
            b.launch();
            let Ball { dx, dy, .. } = b.balls[0];
            let deg = dx.atan2(-dy).to_degrees().abs();
            assert!(deg <= 10.0 + 1e-9, "seed {seed}: {deg}");
            most = most.max(deg);
        }
        assert!(most > 5.0, "no launch tilted more than {most}");
    }

    #[test]
    fn substeps_never_exceed_half_a_pixel() {
        for i in 0..=10_000 {
            let dist = i as f64 * 0.01;
            let n = substeps(dist);
            assert!(n >= 1 && dist / n as f64 <= 0.5, "dist {dist}: {n}");
        }
    }

    #[test]
    fn cap_speed_ball_never_tunnels_a_one_pixel_brick() {
        for deg in [-50.0_f64, -30.0, -10.0, 0.0, 10.0, 30.0, 50.0] {
            for k in 0..=15 {
                let a = deg.to_radians();
                let y0 = 30.0 + 0.1 * k as f64;
                let mut b = empty(1);
                b.levers.speed = levers(1000).speed;
                b.cells[5][7] = Cell::Brick(1);
                b.balls = vec![ball(30.0 - (y0 - 9.5) * a.tan(), y0, a.sin(), -a.cos())];
                let mut hit = false;
                for _ in 0..60 {
                    b.tick(0);
                    if b.cells[5][7] == Cell::Empty {
                        hit = true;
                        break;
                    }
                    assert!(b.balls[0].y >= 9.0, "{deg} deg, y0 {y0}: passed row 9");
                }
                assert!(hit, "{deg} deg, y0 {y0}: no hit in 60 ticks");
            }
        }
    }

    #[test]
    fn walls_and_ceiling_reflect() {
        let mut b = empty(1);
        b.balls = vec![ball(0.2, 20.0, -0.6, -0.8)];
        b.tick(0);
        assert!(b.balls[0].dx > 0.0, "{:?}", b.balls[0]);

        let mut b = empty(1);
        b.balls = vec![ball(30.0, 0.2, 0.0, -1.0)];
        b.tick(0);
        assert!(b.balls[0].dy > 0.0, "{:?}", b.balls[0]);
    }

    #[test]
    fn right_wall_reflects_and_keeps_the_ball_inside() {
        let mut b = empty(1);
        b.balls = vec![ball(59.8, 20.0, 0.6, -0.8)];
        b.tick(0);
        assert!(b.balls[0].dx < 0.0 && b.balls[0].x < W, "{:?}", b.balls[0]);

        // A step can land exactly on x = W. The fold puts the ball back in
        // column 14, so the brick lookup stays inside the grid.
        let mut b = empty(1);
        b.balls = vec![ball(W, 10.5, 0.0, -1.0)];
        b.tick(0);
        assert!(b.balls[0].x < W, "{:?}", b.balls[0]);
    }

    #[test]
    fn brick_reflects_on_the_entry_axis() {
        // Each case keeps a Brick(1) at (0, 0) that is never hit, so no case clears the stage.
        let board = |cell: Cell, ball: Ball| {
            let mut b = empty(1);
            b.cells[0][0] = Cell::Brick(1);
            b.cells[5][7] = cell;
            b.balls = vec![ball];
            b.tick(0);
            b
        };

        let b = board(Cell::Brick(2), ball(30.0, 10.2, 0.6, -0.8));
        assert_eq!((b.balls[0].dx, b.balls[0].dy), (0.6, 0.8));
        assert_eq!(b.cells[5][7], Cell::Brick(1));

        let b = board(Cell::Brick(2), ball(27.9, 9.5, 0.866, 0.5));
        assert_eq!((b.balls[0].dx, b.balls[0].dy), (-0.866, 0.5));
        assert_eq!(b.cells[5][7], Cell::Brick(1));

        let b = board(Cell::Unbreakable, ball(30.0, 10.2, 0.0, -1.0));
        assert!(b.balls[0].dy > 0.0, "{:?}", b.balls[0]);
        assert_eq!(b.cells[5][7], Cell::Unbreakable);
        assert_eq!(b.phase, Phase::Play);

        // Diagonal: the pixel row and the brick column change in one step.
        // Bricks are 1 pixel tall, so the axis is y.
        let b = board(Cell::Brick(2), ball(27.9, 10.2, 0.6, -0.8));
        assert_eq!((b.balls[0].dx, b.balls[0].dy), (0.6, 0.8));
        assert_eq!(b.cells[5][7], Cell::Brick(1));
    }

    #[test]
    fn a_hit_puts_the_ball_back_outside_the_brick() {
        // Start heights over two sub-steps, so some hits fall on the last
        // sub-step of the tick.
        for k in 0..20 {
            let mut b = empty(1);
            b.cells[0][0] = Cell::Brick(1);
            b.cells[5][7] = Cell::Brick(2);
            b.balls = vec![ball(30.0, 10.0 + 0.05 * k as f64, 0.0, -1.0)];
            b.tick(0);
            assert_eq!(b.cells[5][7], Cell::Brick(1), "k {k}");
            assert!(b.balls[0].y >= 10.0, "k {k}: {:?}", b.balls[0]);
        }
    }

    #[test]
    fn last_ball_lost_costs_a_life_and_serves_again() {
        let mut b = empty(1);
        b.balls = vec![ball(30.0, 43.9, 0.0, 1.0)];
        b.tick(0);
        assert_eq!((b.lives, b.phase, b.balls.len()), (2, Phase::Serve, 1));
        assert_eq!(b.balls[0].y, PADDLE_Y - 0.5);
    }

    #[test]
    fn losing_one_of_several_balls_costs_nothing() {
        let mut b = empty(1);
        b.balls.push(ball(30.0, 43.9, 0.0, 1.0));
        b.tick(0);
        assert_eq!((b.lives, b.balls.len(), b.phase), (3, 1, Phase::Play));
    }

    #[test]
    fn last_life_blinks_six_half_periods_then_over() {
        let mut b = empty(1);
        b.lives = 1;
        b.balls = vec![ball(30.0, 43.9, 0.0, 1.0)];
        b.tick(0);
        assert_eq!(b.phase, Phase::Dying { left: 6 });
        let cx = b.paddle_cx;
        for _ in 0..5 {
            b.tick(1);
            assert!(!b.over());
            assert_eq!(b.paddle_cx, cx);
        }
        b.tick(1);
        assert!(b.over());
    }

    #[test]
    fn hits_score_ten_plus_stage_and_clear_scores_hundred_times_stage() {
        let mut b = empty(3);
        b.cells[5][7] = Cell::Brick(2);
        b.hit(7, 5);
        assert_eq!(b.score, 13);
        b.hit(7, 5);
        assert_eq!((b.score, b.stage_points, b.bricks), (26 + 300, 26, 1));
        assert_eq!(b.phase, Phase::Clear { ticks: 60 });
    }

    #[test]
    fn clearing_through_tick_costs_no_life() {
        let mut b = empty(3);
        b.cells[5][7] = Cell::Brick(1);
        b.balls = vec![ball(30.0, 10.2, 0.0, -1.0)];
        b.tick(0);
        assert_eq!(b.cells[5][7], Cell::Empty);
        assert_eq!((b.lives, b.balls.len()), (3, 0));
        assert_eq!(b.phase, Phase::Clear { ticks: 60 });
    }

    #[test]
    fn clearing_a_multiple_of_five_adds_a_life_up_to_five() {
        let clear = |stage: u32, lives: u32| {
            let mut b = empty(stage);
            b.lives = lives;
            b.cells[5][7] = Cell::Brick(1);
            b.hit(7, 5);
            b.lives
        };
        assert_eq!(clear(LIFE_EVERY, 3), 4);
        assert_eq!(clear(LIFE_EVERY, MAX_LIVES), MAX_LIVES);
        assert_eq!(clear(LIFE_EVERY - 1, 3), 3);
        assert_eq!(empty(1).next_life_stage(), 5);
        assert_eq!(empty(4).next_life_stage(), 5);
        assert_eq!(empty(5).next_life_stage(), 10);
    }

    #[test]
    fn clear_counts_down_then_starts_the_next_stage() {
        let mut b = empty(3);
        b.cells[5][7] = Cell::Brick(1);
        b.hit(7, 5);
        for _ in 0..59 {
            b.tick(0);
            assert!(matches!(b.phase, Phase::Clear { .. }), "{:?}", b.phase);
        }
        b.tick(0);
        assert_eq!((b.stage, b.reached, b.phase), (4, 4, Phase::Serve));
        let filled = b
            .cells
            .iter()
            .flatten()
            .filter(|&&c| c != Cell::Empty)
            .count();
        assert_eq!(filled, levers(effective(4)).count);
        assert_eq!((secs(60), secs(31), secs(30)), (2, 2, 1));
    }

    #[test]
    fn new_board_has_reached_the_start_stage() {
        let mut b = Board::new(9, 7);
        assert_eq!((b.reached, b.lives, b.phase), (7, 3, Phase::Serve));
        assert_eq!(b.variant(), "stage 7-7");
        b.bricks = 12;
        assert_eq!(b.summary(), "▬ 12 bricks");
    }

    #[test]
    fn wide_and_slow_timers_refresh_on_a_second_catch() {
        let mut b = empty(1);
        b.apply(Power::Wide);
        assert_eq!(b.wide, 454);
        assert_eq!(b.paddle_w(), (b.levers.paddle + 3) as f64);
        for _ in 0..100 {
            b.tick(0);
        }
        assert_eq!(b.wide, 354);
        b.apply(Power::Wide);
        assert_eq!(b.wide, 454);

        let mut b = empty(1);
        b.apply(Power::Slow);
        assert_eq!(b.slow, 303);
        assert_eq!(b.ball_speed(), b.levers.speed * 0.75);
        for _ in 0..100 {
            b.tick(0);
        }
        assert_eq!(b.slow, 203);
        b.apply(Power::Slow);
        assert_eq!(b.slow, 303);
        assert_eq!(b.phase, Phase::Play);
    }

    #[test]
    fn serve_keeps_power_up_timers() {
        let mut b = empty(1);
        (b.wide, b.slow) = (100, 50);
        b.capsule = Some(Capsule {
            kind: Power::Wide,
            x: 10.0,
            y: 20.0,
        });
        b.balls = vec![ball(30.0, 43.9, 0.0, 1.0)];
        b.tick(0);
        assert_eq!(
            (b.phase, b.capsule, b.wide, b.slow),
            (Phase::Serve, None, 99, 49)
        );
        for _ in 0..10 {
            b.tick(0);
        }
        assert_eq!((b.phase, b.wide, b.slow), (Phase::Serve, 99, 49));
    }

    #[test]
    fn multi_splits_every_ball_into_three_up_to_nine() {
        for (before, after) in [(1, 3), (3, 9), (5, 9), (9, 9)] {
            let mut b = empty(1);
            b.balls = vec![ball(30.0, 20.0, 0.0, -1.0); before];
            b.apply(Power::Multi);
            assert_eq!(b.balls.len(), after, "{before} balls");
        }
    }

    #[test]
    fn capsules_drop_about_eight_percent_and_one_at_a_time() {
        // The Brick(1) at (0, 0) is never hit, so hit never clears the stage.
        let mut b = empty(1);
        b.cells[0][0] = Cell::Brick(1);
        let mut drops = 0;
        for _ in 0..10_000 {
            b.cells[5][7] = Cell::Brick(1);
            b.hit(7, 5);
            if b.capsule.take().is_some() {
                drops += 1;
            }
        }
        assert!((650..=950).contains(&drops), "{drops} drops");

        while b.capsule.is_none() {
            b.cells[5][7] = Cell::Brick(1);
            b.hit(7, 5);
        }
        let first = b.capsule;
        for _ in 0..1_000 {
            b.cells[5][7] = Cell::Brick(1);
            b.hit(7, 5);
        }
        assert_eq!(b.capsule, first);
    }

    #[test]
    fn capsule_caught_over_the_paddle_or_lost_past_the_bottom() {
        let mut b = empty(1);
        b.capsule = Some(Capsule {
            kind: Power::Wide,
            x: b.paddle_cx,
            y: 41.9,
        });
        b.tick(0);
        assert_eq!((b.capsule, b.wide), (None, WIDE_TICKS));

        let mut b = empty(1);
        b.capsule = Some(Capsule {
            kind: Power::Wide,
            x: 2.0,
            y: 41.9,
        });
        b.tick(0);
        assert!(b.capsule.is_some());
        for _ in 0..10 {
            b.tick(0);
            if let Some(cap) = b.capsule {
                assert!(cap.y < H, "{cap:?}");
            }
        }
        assert_eq!((b.capsule, b.wide, b.slow, b.balls.len()), (None, 0, 0, 1));
    }

    #[test]
    fn capsule_that_passed_the_paddle_row_beside_the_paddle_is_not_caught() {
        // The capsule falls 0.66 px per tick, so it is in the paddle row after
        // two ticks, at y 42.06 and 42.72. The paddle is beside it on the first
        // and under it on the second. A capsule is caught only on the tick it
        // reaches the row, so this one is not.
        let mut b = empty(1);
        b.capsule = Some(Capsule {
            kind: Power::Wide,
            x: 10.0,
            y: 41.4,
        });
        let in_paddle_row = |b: &Board| b.capsule.is_some_and(|cap| cap.y.floor() == PADDLE_Y);
        b.tick(0);
        assert!(in_paddle_row(&b), "{:?}", b.capsule);
        b.paddle_cx = 10.0;
        b.tick(0);
        assert!(in_paddle_row(&b), "{:?}", b.capsule);
        assert_eq!(b.wide, 0);
    }

    #[test]
    fn ball_that_passed_the_paddle_row_beside_the_paddle_is_not_bounced() {
        let mut b = empty(1);
        b.balls = vec![ball(10.0, 41.5, 0.0, 1.0)];
        b.tick(0);
        assert_eq!(b.balls[0].y.floor(), PADDLE_Y, "{:?}", b.balls[0]);
        b.paddle_cx = 10.0;
        b.tick(0);
        assert!(b.balls[0].dy > 0.0, "{:?}", b.balls[0]);
    }

    #[test]
    fn paddle_accelerates_from_one_to_three_and_stops_at_walls() {
        let mut b = Board::new(1, 1);
        for want in [1.0, 1.4, 1.8, 2.2, 2.6, 3.0, 3.0] {
            assert!(close(moved(&mut b, 1), want, 1e-9), "want {want}");
        }
        assert_eq!(moved(&mut b, 0), 0.0);
        assert!(close(moved(&mut b, 1), 1.0, 1e-9));
        for _ in 0..100 {
            b.tick(1);
        }
        assert_eq!(b.paddle_cx, W - b.paddle_w() / 2.0);
        assert_eq!(b.balls, vec![ball(b.paddle_cx, PADDLE_Y - 0.5, 0.0, -1.0)]);
    }

    #[test]
    fn single_press_moves_one_pixel_on_the_next_tick() {
        let mut b = Board::new(1, 1);
        b.press(1);
        assert_eq!(moved(&mut b, 0), 1.0);
        assert_eq!(moved(&mut b, 0), 0.0);
        b.press(-1);
        assert_eq!(moved(&mut b, 0), -1.0);
        b.press(1);
        assert_eq!(moved(&mut b, 1), 1.0);

        b.phase = Phase::Dying { left: 6 };
        b.press(1);
        assert_eq!(moved(&mut b, 0), 0.0);
    }

    #[test]
    fn stall_guard_turns_a_ball_out_of_a_closed_loop() {
        // A loop found in play at stage 100. The ball bounces between the
        // ceiling, the right wall and four unbreakable bricks and repeats its
        // path every 103 ticks. Without the guard it never drops below y 20.
        let mut b = empty(100);
        for (c, r) in [(11, 0), (10, 1), (11, 6), (12, 7)] {
            b.cells[r][c] = Cell::Unbreakable;
        }
        let d = std::f64::consts::FRAC_1_SQRT_2; // 45 degrees, up and right
        b.balls = vec![ball(51.1, 10.4, d, -d)];
        let mut t = 0;
        while b.balls[0].y <= 20.0 {
            b.tick(0);
            t += 1;
            assert!(b.balls[0].dy.abs() >= 0.5, "tick {t}: {:?}", b.balls[0]);
            assert!(t < 10 * STALL_TICKS, "still looping after {t} ticks");
        }
        assert!(t > STALL_TICKS, "left after {t} ticks, before the guard");
    }

    #[test]
    fn stall_turn_stays_within_60_degrees_of_vertical() {
        // A stalled ball at the 60 degree limit, turned with 20 rng seeds.
        let (dx, dy) = bounce_dir(1.0);
        for seed in 0..20 {
            let mut b = empty(1);
            b.rng = StdRng::seed_from_u64(seed);
            b.balls = vec![Ball {
                stall: STALL_TICKS - 1,
                ..ball(30.0, 20.0, dx, dy)
            }];
            b.tick(0);
            let got = b.balls[0];
            let turn = got.dx.atan2(-got.dy).to_degrees() - MAX_ANGLE_DEG;
            assert!(
                got.dy <= -0.5 && turn.abs() <= STALL_TURN_DEG,
                "seed {seed}: {got:?}"
            );
        }
    }

    #[test]
    fn paddle_and_breakable_hits_reset_the_stall_count() {
        // Both balls are one tick from a turn. A hit in that tick resets the
        // count, so neither turns.
        let mut b = empty(1);
        b.cells[0][0] = Cell::Brick(1);
        b.cells[5][7] = Cell::Brick(2);
        let stalled = |ball| Ball {
            stall: STALL_TICKS - 1,
            ..ball
        };
        b.balls = vec![
            stalled(ball(30.0, 10.2, 0.6, -0.8)),
            stalled(ball(b.paddle_cx, 41.9, 0.0, 1.0)),
        ];
        b.tick(0);
        assert_eq!((b.balls[0].dx, b.balls[0].dy), (0.6, 0.8));
        assert_eq!((b.balls[1].dx, b.balls[1].dy), bounce_dir(0.0));
    }
}
