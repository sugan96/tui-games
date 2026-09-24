use std::{collections::VecDeque, time::Duration};

pub const W: i32 = 40;
pub const H: i32 = 20;
pub const MAX_LEVEL: u8 = 9;
pub const APPLES_PER_LEVEL: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Fixed(u8),
    Variable,
}

impl Mode {
    /// 0 for Variable, 1 to 9 for Fixed.
    pub fn as_db(self) -> i64 {
        match self {
            Mode::Variable => 0,
            Mode::Fixed(n) => n as i64,
        }
    }

    /// Inverse of as_db. None for anything else.
    pub fn from_db(v: i64) -> Option<Mode> {
        match v {
            0 => Some(Mode::Variable),
            1..=9 => Some(Mode::Fixed(v as u8)),
            _ => None,
        }
    }

    /// "variable" or "fixed 5".
    pub fn label(self) -> String {
        match self {
            Mode::Variable => "variable".to_string(),
            Mode::Fixed(n) => format!("fixed {n}"),
        }
    }
}

/// Duration::from_millis(220 - level * 20). Level is clamped to 1..=9.
pub fn tick_for_level(level: u8) -> Duration {
    let level = level.clamp(1, MAX_LEVEL) as u64;
    Duration::from_millis(220 - level * 20)
}

pub struct Game {
    pub snake: VecDeque<(i32, i32)>,
    pub dir: (i32, i32),
    pub food: (i32, i32),
    pub over: bool,
    pub mode: Mode,
    pub apples: u32,
    pub score: u32,
    /// Direction of the last step. turn() checks reversal against this.
    moved: (i32, i32),
}

impl Game {
    pub fn new(mode: Mode) -> Game {
        let mut g = Game {
            snake: VecDeque::from([(W / 2, H / 2)]),
            dir: (1, 0),
            food: (0, 0),
            over: false,
            mode,
            apples: 0,
            score: 0,
            moved: (1, 0),
        };
        g.spawn_food();
        g
    }

    /// Fixed(n) returns n. Variable returns min(9, 1 + apples / 3).
    pub fn level(&self) -> u8 {
        match self.mode {
            Mode::Fixed(n) => n,
            Mode::Variable => (1 + self.apples / APPLES_PER_LEVEL).min(MAX_LEVEL as u32) as u8,
        }
    }

    pub fn tick(&self) -> Duration {
        tick_for_level(self.level())
    }

    fn spawn_food(&mut self) {
        loop {
            let p = (rand::random_range(0..W), rand::random_range(0..H));
            if !self.snake.contains(&p) {
                self.food = p;
                return;
            }
        }
    }

    /// Advance one cell. On wall or self collision set over = true and return.
    /// On eating: apples += 1, score += level() as computed BEFORE the increment, spawn new food.
    pub fn step(&mut self) {
        let (hx, hy) = self.snake[0];
        let head = (hx + self.dir.0, hy + self.dir.1);
        let hit_wall = head.0 < 0 || head.0 >= W || head.1 < 0 || head.1 >= H;
        if hit_wall || self.snake.contains(&head) {
            self.over = true;
            return;
        }
        self.moved = self.dir;
        self.snake.push_front(head);
        if head == self.food {
            self.score += self.level() as u32;
            self.apples += 1;
            self.spawn_food();
        } else {
            self.snake.pop_back();
        }
    }

    /// Ignore a reversal of the last step, even across several turns in one tick.
    pub fn turn(&mut self, d: (i32, i32)) {
        if d.0 != -self.moved.0 || d.1 != -self.moved.1 {
            self.dir = d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(g: &mut Game) {
        let (hx, hy) = g.snake[0];
        g.food = (hx + g.dir.0, hy + g.dir.1);
        g.step();
    }

    #[test]
    fn eats_grows_and_dies_on_wall() {
        let mut g = Game::new(Mode::Fixed(5));
        g.food = (W / 2 + 1, H / 2);
        g.step();
        assert_eq!(g.snake.len(), 2);
        g.turn((-1, 0)); // reverse ignored
        assert_eq!(g.dir, (1, 0));
        for _ in 0..W {
            g.step();
        }
        assert!(g.over);
    }

    #[test]
    fn two_turns_in_one_tick_cannot_reverse() {
        let mut g = Game::new(Mode::Fixed(1));
        g.snake = VecDeque::from([(W / 2, H / 2), (W / 2 - 1, H / 2)]);
        g.turn((0, -1));
        g.turn((-1, 0));
        g.step();
        assert!(!g.over);
    }

    #[test]
    fn tick_bounds() {
        assert_eq!(tick_for_level(1), Duration::from_millis(200));
        assert_eq!(tick_for_level(9), Duration::from_millis(40));
    }

    #[test]
    fn mode_db_round_trip() {
        assert_eq!(Mode::from_db(Mode::Fixed(7).as_db()), Some(Mode::Fixed(7)));
        assert_eq!(Mode::from_db(0), Some(Mode::Variable));
        assert_eq!(Mode::from_db(10), None);
    }

    #[test]
    fn variable_level_rises_every_three_apples_capped_at_nine() {
        let mut g = Game::new(Mode::Variable);
        // Start at the left edge so the snake has room for 30 apples in a row.
        g.snake = VecDeque::from([(0, H / 2)]);
        assert_eq!(g.level(), 1);
        feed(&mut g);
        feed(&mut g);
        assert_eq!(g.level(), 1);
        feed(&mut g);
        assert_eq!(g.level(), 2);
        for _ in 0..27 {
            feed(&mut g);
            assert!(g.level() <= MAX_LEVEL);
        }
        assert!(!g.over);
        assert_eq!(g.apples, 30);
        assert_eq!(g.level(), MAX_LEVEL);
    }

    #[test]
    fn apple_at_level_four_adds_four() {
        let mut g = Game::new(Mode::Fixed(4));
        feed(&mut g);
        assert_eq!(g.apples, 1);
        assert_eq!(g.score, 4);
    }
}
