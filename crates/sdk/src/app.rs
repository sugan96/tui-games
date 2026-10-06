use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEventKind};

use crate::{
    db::{Db, ScoreRow, TOP_N},
    game::{Entry, Game, Outcome, Status},
    input::Input,
};

pub enum Screen {
    /// The game menu: logo, top table, start keys.
    Menu,
    Playing(Box<dyn Game>),
    /// The finished game stays so the modals can draw it dimmed underneath.
    NameEntry {
        game: Box<dyn Game>,
        outcome: Outcome,
        row: i64,
        name: String,
    },
    /// `ranked` is true when the score made the table and a name was entered.
    GameOver {
        game: Box<dyn Game>,
        outcome: Outcome,
        ranked: bool,
    },
}

pub struct App {
    pub screen: Screen,
    pub entry: &'static Entry,
    /// Top table of the game.
    pub top: Vec<ScoreRow>,
    pub quit: bool,
    /// Timer ticks outside play. Drives blinking prompts.
    pub ticks: u64,
    pub input: Input,
    /// Stage chosen on the menu picker, 1 to max_stage.
    pub stage: u32,
    /// Highest stage reached in this game, at least 1. Always 1 for a game without stages.
    pub max_stage: u32,
    db: Db,
}

pub const NAME_LEN: usize = 3;
pub const IDLE_TICK: Duration = Duration::from_millis(250);

impl App {
    /// Starts on the game menu. `release_events`: the terminal sends key release events.
    pub fn new(db: Db, entry: &'static Entry, release_events: bool) -> anyhow::Result<App> {
        let max_stage = if entry.stages {
            db.reached(entry.id)?
        } else {
            1
        };
        Ok(App {
            screen: Screen::Menu,
            entry,
            top: db.top(entry.id, TOP_N)?,
            quit: false,
            ticks: 0,
            input: Input::new(release_events, entry.hold_on_press),
            stage: 1,
            max_stage,
            db,
        })
    }

    pub fn playing(&self) -> bool {
        matches!(self.screen, Screen::Playing(_))
    }

    /// Every key event. Repeats and releases only feed the hold tracker, and only while playing.
    pub fn on_key(&mut self, key: KeyCode, kind: KeyEventKind, now: Instant) -> anyhow::Result<()> {
        if self.playing() {
            self.input.event(key, kind, now);
        }
        if kind == KeyEventKind::Press {
            self.handle_key(key)?;
        }
        Ok(())
    }

    /// A key press. q quits on every screen except name entry, where it is a letter.
    /// q during a run of a game with stages saves its progress first.
    pub fn handle_key(&mut self, key: KeyCode) -> anyhow::Result<()> {
        if key == KeyCode::Char('q') && !matches!(self.screen, Screen::NameEntry { .. }) {
            if let Screen::Playing(game) = &self.screen
                && self.entry.stages
            {
                let reached = game.reached();
                self.save_progress(reached)?;
            }
            self.quit = true;
            return Ok(());
        }
        let entry = self.entry;
        match &mut self.screen {
            Screen::Menu => {
                let stage = if entry.stages { self.stage } else { 1 };
                let game = match key {
                    KeyCode::Left if entry.stages => {
                        self.stage = self.stage.saturating_sub(1).max(1);
                        None
                    }
                    KeyCode::Right if entry.stages => {
                        self.stage = (self.stage + 1).min(self.max_stage);
                        None
                    }
                    KeyCode::Char(c) => (entry.start)(c, stage),
                    KeyCode::Enter => (entry.start)('\n', stage),
                    _ => None,
                };
                if let Some(game) = game {
                    self.input.clear();
                    self.screen = Screen::Playing(game);
                }
            }
            Screen::Playing(game) => game.key(key),
            Screen::NameEntry { row, name, .. } => match key {
                KeyCode::Char(c) if c.is_ascii_alphabetic() && name.len() < NAME_LEN => {
                    name.push(c.to_ascii_uppercase());
                }
                KeyCode::Backspace => {
                    name.pop();
                }
                KeyCode::Enter if name.len() == NAME_LEN => {
                    self.db.set_name(*row, name)?;
                    self.top = self.db.top(entry.id, TOP_N)?;
                    let Screen::NameEntry { game, outcome, .. } =
                        std::mem::replace(&mut self.screen, Screen::Menu)
                    else {
                        unreachable!()
                    };
                    self.screen = Screen::GameOver {
                        game,
                        outcome,
                        ranked: true,
                    };
                }
                _ => {}
            },
            Screen::GameOver { .. } => {
                if key == KeyCode::Char('r') {
                    self.top = self.db.top(entry.id, TOP_N)?;
                    self.screen = Screen::Menu;
                }
            }
        }
        Ok(())
    }

    /// The main loop's timer. Ticks the game while playing. When it ends, records
    /// the outcome and moves to name entry if it made the table, else game over.
    pub fn on_timer(&mut self, now: Instant) -> anyhow::Result<()> {
        self.input.set_now(now);
        let Screen::Playing(game) = &mut self.screen else {
            self.ticks += 1;
            return Ok(());
        };
        let Status::Over(outcome) = game.tick(&self.input) else {
            return Ok(());
        };
        let Screen::Playing(game) = std::mem::replace(&mut self.screen, Screen::Menu) else {
            unreachable!()
        };
        let id = self.entry.id;
        let row = self.db.record(id, &outcome.variant, outcome.score)?;
        if self.entry.stages {
            self.save_progress(game.reached())?;
        }
        self.screen = if self.db.qualifies(id, outcome.score)? {
            Screen::NameEntry {
                game,
                outcome,
                row,
                name: String::new(),
            }
        } else {
            Screen::GameOver {
                game,
                outcome,
                ranked: false,
            }
        };
        Ok(())
    }

    /// Raises the saved progress and the picker's limit to `reached`. Only for a game with stages.
    fn save_progress(&mut self, reached: u32) -> anyhow::Result<()> {
        self.db.raise(self.entry.id, reached)?;
        self.max_stage = self.max_stage.max(reached);
        Ok(())
    }

    /// The game's tick rate while playing, IDLE_TICK otherwise.
    pub fn tick_rate(&self) -> Duration {
        match &self.screen {
            Screen::Playing(game) => game.tick_rate(),
            _ => IDLE_TICK,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::cell::RefCell;

    use ratatui::Frame;

    use super::*;
    use crate::game::DrawCtx;

    pub const FAKE_RATE: Duration = Duration::from_millis(10);

    thread_local! {
        /// Keys the Fake got, and whether Right was held on each tick. Tests run
        /// on their own threads, so each test sees only its own log.
        static LOG: RefCell<(Vec<KeyCode>, Vec<bool>)> = RefCell::default();
    }

    /// Ends after `left` ticks with the score given by its start digit.
    pub struct Fake {
        left: u32,
        score: u32,
        reached: u32,
    }

    impl Game for Fake {
        fn key(&mut self, key: KeyCode) {
            LOG.with_borrow_mut(|l| l.0.push(key));
        }
        fn tick(&mut self, input: &Input) -> Status {
            LOG.with_borrow_mut(|l| l.1.push(input.held(KeyCode::Right)));
            if self.left == 0 {
                return Status::Over(Outcome {
                    score: self.score,
                    variant: format!("level {}", self.score),
                    summary: "fake summary".into(),
                });
            }
            self.left -= 1;
            Status::Running
        }
        fn tick_rate(&self) -> Duration {
            FAKE_RATE
        }
        fn draw(&self, frame: &mut Frame, ctx: &DrawCtx) {
            let text = if ctx.dim { "fake dim" } else { "fake live" };
            frame
                .buffer_mut()
                .set_string(0, 0, text, ratatui::style::Style::new());
        }
        fn reached(&self) -> u32 {
            self.reached
        }
    }

    /// Reports a reached stage past its start, so a test fails if the SDK
    /// saves progress for this game without stages.
    fn fake_start(c: char, stage: u32) -> Option<Box<dyn Game>> {
        let score = c.to_digit(10)?;
        Some(Box::new(Fake {
            left: 2,
            score,
            reached: stage + 1,
        }))
    }

    /// Starts only on Enter. Score 0 so the run ends on game over, and it has
    /// got two stages past its start from the first tick.
    fn staged_start(c: char, stage: u32) -> Option<Box<dyn Game>> {
        (c == '\n').then(|| {
            Box::new(Fake {
                left: 2,
                score: 0,
                reached: stage + 2,
            }) as Box<dyn Game>
        })
    }

    pub static FAKE: Entry = Entry {
        id: "fake",
        title: "fake",
        about: "fake about line",
        starts: &[("0-9", "score")],
        stages: false,
        hold_on_press: false,
        start: fake_start,
        min_size: (20, 10),
        thumb: &[
            "........................",
            "..rrrr..........gggg....",
            "..rrrr..........gggg....",
            "........................",
            "........yyyyyyyy........",
            "........yyyyyyyy........",
            "........................",
            "..bbbb..........pppp....",
            "..bbbb..........pppp....",
            "........................",
            "wwwwwwwwwwwwwwwwwwwwwwww",
            "........................",
        ],
    };

    pub static STAGED: Entry = Entry {
        id: "staged",
        title: "staged",
        about: "staged about line",
        starts: &[("enter", "start")],
        stages: true,
        hold_on_press: false,
        start: staged_start,
        min_size: FAKE.min_size,
        thumb: FAKE.thumb,
    };

    pub fn app() -> App {
        App::new(Db::open_in_memory().unwrap(), &FAKE, true).unwrap()
    }

    /// The staged game with progress saved up to stage `max`.
    pub fn staged_app(max: u32) -> App {
        let db = Db::open_in_memory().unwrap();
        db.raise("staged", max).unwrap();
        App::new(db, &STAGED, true).unwrap()
    }

    pub fn start(c: char) -> Box<dyn Game> {
        fake_start(c, 1).unwrap()
    }

    #[test]
    fn fake_meets_the_contract() {
        crate::testkit::check(&FAKE);
    }

    #[test]
    fn staged_fake_meets_the_contract() {
        crate::testkit::check(&STAGED);
    }

    fn press(app: &mut App, keys: &str) {
        for c in keys.chars() {
            app.handle_key(KeyCode::Char(c)).unwrap();
        }
    }

    /// Ticks until the game is over.
    fn finish(app: &mut App) {
        while app.playing() {
            app.on_timer(Instant::now()).unwrap();
        }
    }

    fn name(app: &App) -> &str {
        match &app.screen {
            Screen::NameEntry { name, .. } => name,
            _ => panic!("not on name entry"),
        }
    }

    #[test]
    fn start_key_starts_the_game_and_others_do_nothing() {
        let mut a = app();
        assert_eq!(a.tick_rate(), IDLE_TICK);
        press(&mut a, "x");
        assert!(matches!(a.screen, Screen::Menu));
        press(&mut a, "5");
        assert!(a.playing());
        assert_eq!(a.tick_rate(), FAKE_RATE);
    }

    #[test]
    fn q_quits_from_menu_play_and_game_over() {
        for setup in ["", "5", "0"] {
            let mut a = app();
            press(&mut a, setup);
            if setup == "0" {
                finish(&mut a);
                assert!(matches!(a.screen, Screen::GameOver { ranked: false, .. }));
            }
            press(&mut a, "q");
            assert!(a.quit, "after {setup:?}");
        }
    }

    #[test]
    fn keys_and_holds_reach_the_game() {
        let mut a = app();
        press(&mut a, "5");
        let t = Instant::now();
        a.on_key(KeyCode::Right, KeyEventKind::Press, t).unwrap();
        a.on_key(KeyCode::Left, KeyEventKind::Release, t).unwrap();
        a.on_timer(t).unwrap();
        a.on_key(KeyCode::Right, KeyEventKind::Release, t).unwrap();
        a.on_timer(t).unwrap();
        let (keys, held) = LOG.take();
        assert_eq!(keys, [KeyCode::Right], "releases are not key presses");
        assert_eq!(held, [true, false]);
    }

    #[test]
    fn hold_on_press_reaches_the_input() {
        static HOLD: Entry = Entry {
            hold_on_press: true,
            ..FAKE
        };
        let mut a = App::new(Db::open_in_memory().unwrap(), &HOLD, false).unwrap();
        press(&mut a, "5");
        let t = Instant::now();
        a.on_key(KeyCode::Right, KeyEventKind::Press, t).unwrap();
        a.on_timer(t).unwrap();
        let (_, held) = LOG.take();
        assert_eq!(
            held,
            [true],
            "held right after a press without release events"
        );
    }

    #[test]
    fn zero_score_goes_to_game_over_unranked() {
        let mut a = app();
        press(&mut a, "0");
        finish(&mut a);
        assert!(matches!(a.screen, Screen::GameOver { ranked: false, .. }));
        assert!(a.db.top("fake", TOP_N).unwrap().is_empty());
    }

    #[test]
    fn name_entry_needs_exactly_three_letters_and_saves_the_row() {
        let mut a = app();
        press(&mut a, "7");
        finish(&mut a);
        assert!(name(&a).is_empty());
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
        assert!(matches!(a.screen, Screen::GameOver { ranked: true, .. }));
        assert_eq!(
            a.top,
            vec![ScoreRow {
                name: "ABQ".into(),
                score: 7,
                variant: "level 7".into()
            }]
        );
    }

    #[test]
    fn game_over_r_returns_to_menu() {
        let mut a = app();
        press(&mut a, "0");
        finish(&mut a);
        press(&mut a, "r");
        assert!(matches!(a.screen, Screen::Menu));
    }

    #[test]
    fn idle_timer_counts_ticks() {
        let mut a = app();
        a.on_timer(Instant::now()).unwrap();
        assert_eq!(a.ticks, 1);
    }

    fn keys(app: &mut App, keys: &[KeyCode]) {
        for &k in keys {
            app.handle_key(k).unwrap();
        }
    }

    #[test]
    fn stage_picker_moves_within_one_to_max() {
        let mut a = staged_app(3);
        keys(&mut a, &[KeyCode::Left]);
        assert_eq!(a.stage, 1);
        keys(&mut a, &[KeyCode::Right; 3]);
        assert_eq!(a.stage, 3);
        keys(&mut a, &[KeyCode::Left]);
        assert_eq!(a.stage, 2);
    }

    #[test]
    fn enter_starts_the_chosen_stage_and_raises_progress() {
        let mut a = staged_app(3);
        keys(&mut a, &[KeyCode::Right, KeyCode::Enter]);
        assert!(a.playing());
        finish(&mut a);
        assert_eq!(a.db.reached("staged").unwrap(), 4);
        assert_eq!(a.max_stage, 4);
    }

    #[test]
    fn picker_keeps_its_value_between_runs() {
        let mut a = staged_app(3);
        keys(&mut a, &[KeyCode::Right, KeyCode::Enter]);
        finish(&mut a);
        assert!(matches!(a.screen, Screen::GameOver { .. }));
        press(&mut a, "r");
        assert!(matches!(a.screen, Screen::Menu));
        assert_eq!(a.stage, 2);
        assert_eq!(a.max_stage, 4);
    }

    #[test]
    fn quitting_mid_run_saves_progress() {
        let mut a = staged_app(3);
        keys(&mut a, &[KeyCode::Right, KeyCode::Right, KeyCode::Enter]);
        press(&mut a, "q");
        assert!(a.quit);
        assert_eq!(a.db.reached("staged").unwrap(), 5);
        assert_eq!(a.max_stage, 5);

        let mut a = app();
        press(&mut a, "5q");
        assert!(a.quit);
        assert_eq!(a.db.progress_rows(), 0);
    }

    #[test]
    fn game_without_stages_ignores_arrows_and_saves_no_progress() {
        let mut a = app();
        keys(&mut a, &[KeyCode::Right]);
        assert_eq!(a.stage, 1);
        assert_eq!(a.max_stage, 1);
        press(&mut a, "5");
        finish(&mut a);
        assert_eq!(a.db.reached("fake").unwrap(), 1);
        assert_eq!(a.db.progress_rows(), 0);
    }
}
