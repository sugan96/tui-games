use std::time::Duration;

use ratatui::crossterm::event::KeyCode;

use crate::{
    db::{Db, GameRecord, ScoreRow, TOP_N},
    game::{Game, Mode},
};

pub enum Screen {
    Menu,
    Playing,
    /// The snake blinks `left` more half-periods before the result is recorded.
    Dying {
        left: u8,
    },
    NameEntry {
        game_id: i64,
        name: String,
    },
    /// `ranked` is true when the score made the table and a name was entered.
    GameOver {
        ranked: bool,
    },
}

pub struct App {
    pub screen: Screen,
    pub game: Game,
    pub top: Vec<ScoreRow>,
    pub quit: bool,
    /// Timer ticks since start, on every screen. Drives blinking.
    pub ticks: u64,
    /// False for the first half tick after a step, so the renderer draws the
    /// snake halfway to its new cells. True once the half tick has passed.
    pub settled: bool,
    db: Db,
}

pub const NAME_LEN: usize = 3;
pub const IDLE_TICK: Duration = Duration::from_millis(250);
/// Half-periods of the death blink and the length of each.
pub const BLINKS: u8 = 6;
pub const BLINK_TICK: Duration = Duration::from_millis(200);

impl App {
    /// Loads the top table. Starts on Menu with a placeholder Game::new(Mode::Variable).
    pub fn new(db: Db) -> anyhow::Result<App> {
        Ok(App {
            screen: Screen::Menu,
            game: Game::new(Mode::Variable),
            top: db.top_scores(TOP_N)?,
            quit: false,
            ticks: 0,
            settled: true,
            db,
        })
    }

    pub fn handle_key(&mut self, key: KeyCode) -> anyhow::Result<()> {
        match &mut self.screen {
            Screen::Menu => match key {
                KeyCode::Char(c @ '1'..='9') => self.start(Mode::Fixed(c as u8 - b'0')),
                KeyCode::Char('v') => self.start(Mode::Variable),
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Screen::Playing => match key {
                KeyCode::Up => self.game.turn((0, 1)),
                KeyCode::Down => self.game.turn((0, -1)),
                KeyCode::Left => self.game.turn((-1, 0)),
                KeyCode::Right => self.game.turn((1, 0)),
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
            Screen::Dying { .. } => {
                if key == KeyCode::Char('q') {
                    self.quit = true;
                }
            }
            Screen::NameEntry { game_id, name } => match key {
                KeyCode::Char(c) if c.is_ascii_alphabetic() && name.len() < NAME_LEN => {
                    name.push(c.to_ascii_uppercase());
                }
                KeyCode::Backspace => {
                    name.pop();
                }
                KeyCode::Enter if name.len() == NAME_LEN => {
                    self.db.set_name(*game_id, name)?;
                    self.top = self.db.top_scores(TOP_N)?;
                    self.screen = Screen::GameOver { ranked: true };
                }
                _ => {}
            },
            Screen::GameOver { .. } => match key {
                KeyCode::Char('r') => {
                    self.top = self.db.top_scores(TOP_N)?;
                    self.screen = Screen::Menu;
                }
                KeyCode::Char('q') => self.quit = true,
                _ => {}
            },
        }
        Ok(())
    }

    /// Playing: game.step(); a collision starts the death blink.
    /// Dying: count the blink down; at zero record the game and move to
    /// NameEntry or GameOver. Other screens: only the tick counter moves.
    pub fn on_tick(&mut self) -> anyhow::Result<()> {
        self.ticks += 1;
        match &mut self.screen {
            Screen::Playing => {
                self.game.step();
                self.settled = false;
                if self.game.over {
                    self.screen = Screen::Dying { left: BLINKS };
                }
            }
            Screen::Dying { left } if *left > 1 => *left -= 1,
            Screen::Dying { .. } => {
                let g = &self.game;
                let game_id = self.db.record_game(&GameRecord {
                    score: g.score,
                    apples: g.apples,
                    mode: g.mode,
                })?;
                self.screen = if self.db.qualifies(g.score)? {
                    Screen::NameEntry {
                        game_id,
                        name: String::new(),
                    }
                } else {
                    Screen::GameOver { ranked: false }
                };
            }
            _ => {}
        }
        Ok(())
    }

    /// The main loop's timer. While playing, every other firing is a render-only
    /// half tick that settles the interpolated frame; the rest step the game.
    pub fn on_timer(&mut self) -> anyhow::Result<()> {
        if matches!(self.screen, Screen::Playing) && !self.settled {
            self.settled = true;
            return Ok(());
        }
        self.on_tick()
    }

    /// Half the game tick while Playing, BLINK_TICK while Dying, IDLE_TICK otherwise.
    pub fn tick_rate(&self) -> Duration {
        match self.screen {
            Screen::Playing => self.game.tick() / 2,
            Screen::Dying { .. } => BLINK_TICK,
            _ => IDLE_TICK,
        }
    }

    fn start(&mut self, mode: Mode) {
        self.game = Game::new(mode);
        self.settled = true;
        self.screen = Screen::Playing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::H;

    fn app() -> App {
        App::new(Db::open_in_memory().unwrap()).unwrap()
    }

    fn press(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.handle_key(KeyCode::Char(c)).unwrap();
        }
    }

    /// Starts a fixed level 5 game with the given score and ticks it into the top wall.
    fn die_with_score(app: &mut App, score: u32) {
        press(app, "5");
        app.game.score = score;
        app.game.food = (0, 0); // off the path of the head column
        app.game.dir = (0, 1); // head starts at y = H / 2, the top wall at y = H is nearest
        for _ in 0..H {
            app.on_tick().unwrap();
            if let Screen::Dying { left } = app.screen {
                assert_eq!(left, BLINKS);
                for _ in 0..BLINKS {
                    app.on_tick().unwrap();
                }
                return;
            }
        }
        panic!("snake did not die");
    }

    fn name(app: &App) -> &str {
        match &app.screen {
            Screen::NameEntry { name, .. } => name,
            _ => panic!("not on name entry"),
        }
    }

    #[test]
    fn menu_digit_starts_fixed_game() {
        let mut a = app();
        press(&mut a, "5");
        assert!(matches!(a.screen, Screen::Playing));
        assert_eq!(a.game.mode, Mode::Fixed(5));
        assert_eq!(a.tick_rate(), a.game.tick() / 2);
    }

    #[test]
    fn menu_v_starts_variable_game() {
        let mut a = app();
        assert_eq!(a.tick_rate(), IDLE_TICK);
        press(&mut a, "v");
        assert!(matches!(a.screen, Screen::Playing));
        assert_eq!(a.game.mode, Mode::Variable);
    }

    #[test]
    fn menu_q_quits() {
        let mut a = app();
        press(&mut a, "q");
        assert!(a.quit);
    }

    #[test]
    fn arrows_turn_and_q_quits_while_playing() {
        let mut a = app();
        press(&mut a, "5");
        a.handle_key(KeyCode::Up).unwrap();
        assert_eq!(a.game.dir, (0, 1));
        // Left would reverse the start direction (1, 0), so step once first.
        a.on_tick().unwrap();
        a.handle_key(KeyCode::Left).unwrap();
        assert_eq!(a.game.dir, (-1, 0));
        // The last step was (0, 1), so Down is a reversal and is ignored.
        a.handle_key(KeyCode::Down).unwrap();
        assert_eq!(a.game.dir, (-1, 0));
        press(&mut a, "q");
        assert!(a.quit);
    }

    #[test]
    fn zero_score_death_goes_to_game_over() {
        let mut a = app();
        die_with_score(&mut a, 0);
        assert!(matches!(a.screen, Screen::GameOver { ranked: false }));
    }

    #[test]
    fn death_blinks_before_recording_and_ignores_steering() {
        let mut a = app();
        press(&mut a, "5");
        a.game.dir = (0, 1);
        a.game.food = (0, 0);
        while matches!(a.screen, Screen::Playing) {
            a.on_tick().unwrap();
        }
        assert!(matches!(a.screen, Screen::Dying { left: BLINKS }));
        assert_eq!(a.tick_rate(), BLINK_TICK);
        let head = a.game.snake[0];
        a.handle_key(KeyCode::Left).unwrap();
        for _ in 0..BLINKS - 1 {
            a.on_tick().unwrap();
            assert!(matches!(a.screen, Screen::Dying { .. }));
        }
        assert_eq!(a.game.snake[0], head);
        assert!(a.db.top_scores(TOP_N).unwrap().is_empty());
        a.on_tick().unwrap();
        assert!(matches!(a.screen, Screen::GameOver { .. }));
        press(&mut a, "q");
        assert!(a.quit);
    }

    #[test]
    fn timer_alternates_half_tick_and_step_while_playing() {
        let mut a = app();
        a.on_timer().unwrap();
        assert_eq!(a.ticks, 1, "idle screens step every timer");
        press(&mut a, "5");
        assert!(a.settled);
        let head = a.game.snake[0];
        a.on_timer().unwrap();
        assert!(!a.settled);
        assert_ne!(a.game.snake[0], head);
        let head = a.game.snake[0];
        a.on_timer().unwrap();
        assert!(a.settled);
        assert_eq!(a.game.snake[0], head, "half tick does not move the snake");
    }

    #[test]
    fn qualifying_death_goes_to_name_entry_with_a_row() {
        let mut a = app();
        die_with_score(&mut a, 7);
        let Screen::NameEntry { game_id, ref name } = a.screen else {
            panic!("not on name entry");
        };
        assert!(name.is_empty());
        // The row exists: naming it succeeds only when the id is present.
        a.db.set_name(game_id, "ROW").unwrap();
        assert_eq!(a.db.top_scores(TOP_N).unwrap()[0].score, 7);
    }

    #[test]
    fn name_entry_needs_exactly_three_letters() {
        let mut a = app();
        die_with_score(&mut a, 7);
        press(&mut a, "ab");
        a.handle_key(KeyCode::Enter).unwrap();
        assert_eq!(name(&a), "AB");
        press(&mut a, "cd");
        assert_eq!(name(&a), "ABC");
        a.handle_key(KeyCode::Backspace).unwrap();
        press(&mut a, "q");
        assert_eq!(name(&a), "ABQ");
        assert!(!a.quit);
        a.handle_key(KeyCode::Enter).unwrap();
        assert!(matches!(a.screen, Screen::GameOver { ranked: true }));
        assert_eq!(
            a.top,
            vec![ScoreRow {
                name: "ABQ".into(),
                score: 7,
                mode: Mode::Fixed(5)
            }]
        );
    }

    #[test]
    fn game_over_r_returns_to_menu() {
        let mut a = app();
        die_with_score(&mut a, 0);
        press(&mut a, "r");
        assert!(matches!(a.screen, Screen::Menu));
    }
}
