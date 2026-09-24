use std::time::Duration;

use ratatui::crossterm::event::KeyCode;

use crate::{
    db::{Db, GameRecord, ScoreRow, TOP_N},
    game::{Game, Mode},
};

pub enum Screen {
    Menu,
    Playing,
    NameEntry { game_id: i64, name: String },
    GameOver,
}

pub struct App {
    pub screen: Screen,
    pub game: Game,
    pub top: Vec<ScoreRow>,
    pub quit: bool,
    db: Db,
}

pub const NAME_LEN: usize = 3;
pub const IDLE_TICK: Duration = Duration::from_millis(250);

impl App {
    /// Loads the top table. Starts on Menu with a placeholder Game::new(Mode::Variable).
    pub fn new(db: Db) -> anyhow::Result<App> {
        Ok(App {
            screen: Screen::Menu,
            game: Game::new(Mode::Variable),
            top: db.top_scores(TOP_N)?,
            quit: false,
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
                    self.screen = Screen::GameOver;
                }
                _ => {}
            },
            Screen::GameOver => match key {
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

    /// Playing: game.step(); if the game just ended, record it and move to NameEntry or GameOver.
    /// Other screens: no-op.
    pub fn on_tick(&mut self) -> anyhow::Result<()> {
        if !matches!(self.screen, Screen::Playing) {
            return Ok(());
        }
        self.game.step();
        if self.game.over {
            let g = &self.game;
            let game_id = self.db.record_game(&GameRecord {
                score: g.score,
                apples: g.apples,
                mode: g.mode,
            })?;
            self.screen = if self.db.qualifies(g.score)? {
                Screen::NameEntry { game_id, name: String::new() }
            } else {
                Screen::GameOver
            };
        }
        Ok(())
    }

    /// game.tick() while Playing, IDLE_TICK otherwise.
    pub fn tick_rate(&self) -> Duration {
        match self.screen {
            Screen::Playing => self.game.tick(),
            _ => IDLE_TICK,
        }
    }

    fn start(&mut self, mode: Mode) {
        self.game = Game::new(mode);
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
            if !matches!(app.screen, Screen::Playing) {
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
        assert_eq!(a.tick_rate(), a.game.tick());
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
        assert!(matches!(a.screen, Screen::GameOver));
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
        assert!(matches!(a.screen, Screen::GameOver));
        assert_eq!(
            a.top,
            vec![ScoreRow { name: "ABQ".into(), score: 7, mode: Mode::Fixed(5) }]
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
