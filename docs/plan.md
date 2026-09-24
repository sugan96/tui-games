# Snake arcade implementation plan

Spec: docs/spec.md

## Global constraints

- Commit messages: exactly one line, conventional commits, no body, no trailers.
- Atomic commits, one logical change each. Build must pass at every commit.
- `cargo build` and `cargo test` must pass with no warnings.
- No em dashes anywhere in code, comments, or docs.
- No new dependencies beyond those already in Cargo.toml: ratatui 0.30, rand 0.10, rusqlite 0.40 bundled, anyhow 1.
- Exact values from the spec: grid 40 by 20, tick ms = 220 - level * 20, 3 apples per level in variable mode, max level 9, top 10 table, 3 letter names, mode integer 0 = variable and 1 to 9 = fixed, database at `~/.config/snake/snake.db`, corrupt file renamed to `snake.db.broken-<unix seconds>`.
- `game.rs` imports nothing from ratatui or rusqlite.
- Tests are plain `#[cfg(test)]` modules, no test frameworks.
- Do not dispatch subagents.

## Task 1: game logic module

Create `src/game.rs` from the current `src/main.rs` game code and extend it with modes, levels, and scoring. Leave `src/main.rs` compiling by adding `mod game;` and updating it to use `game::Game` and `game::Mode::Fixed(5)` with the existing fixed 120ms poll replaced by `game.tick()`. Do not build any new screens in this task.

Public interface, exact:

```rust
pub const W: i32 = 40;
pub const H: i32 = 20;
pub const MAX_LEVEL: u8 = 9;
pub const APPLES_PER_LEVEL: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Fixed(u8), Variable }

impl Mode {
    /// 0 for Variable, 1 to 9 for Fixed.
    pub fn as_db(self) -> i64;
    /// Inverse of as_db. None for anything else.
    pub fn from_db(v: i64) -> Option<Mode>;
    /// "variable" or "fixed 5".
    pub fn label(self) -> String;
}

/// Duration::from_millis(220 - level * 20). Level is clamped to 1..=9.
pub fn tick_for_level(level: u8) -> std::time::Duration;

pub struct Game {
    pub snake: std::collections::VecDeque<(i32, i32)>,
    pub dir: (i32, i32),
    pub food: (i32, i32),
    pub over: bool,
    pub mode: Mode,
    pub apples: u32,
    pub score: u32,
}

impl Game {
    pub fn new(mode: Mode) -> Game;
    /// Fixed(n) returns n. Variable returns min(9, 1 + apples / 3).
    pub fn level(&self) -> u8;
    pub fn tick(&self) -> std::time::Duration;
    /// Advance one cell. On wall or self collision set over = true and return.
    /// On eating: apples += 1, score += level() as computed BEFORE the increment, spawn new food.
    pub fn step(&mut self);
    /// Ignore a direct reversal.
    pub fn turn(&mut self, d: (i32, i32));
}
```

Move the existing test into `game.rs` and add tests for: `tick_for_level(1) == 200ms`, `tick_for_level(9) == 40ms`, `Mode::from_db(Mode::Fixed(7).as_db()) == Some(Fixed(7))`, `Mode::from_db(0) == Some(Variable)`, `Mode::from_db(10) == None`, variable mode level rises from 1 to 2 after the third apple and never exceeds 9, an apple eaten at level 4 adds 4 to score. Tests set `food` directly in front of the head to force eating.

Commit as `refactor(snake): extract game module with modes and scoring`.

## Task 2: database module

Create `src/db.rs`. Add `mod db;` to `src/main.rs` but do not use it yet beyond the declaration. All SQLite code lives here. Uses `game::Mode` for the mode column.

Public interface, exact:

```rust
pub struct Db { conn: rusqlite::Connection }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreRow { pub name: String, pub score: u32, pub mode: crate::game::Mode }

pub struct GameRecord { pub score: u32, pub apples: u32, pub mode: crate::game::Mode }

pub const TOP_N: usize = 10;

impl Db {
    /// Opens ~/.config/snake/snake.db, creating the directory. Uses std::env::home_dir().
    /// Errors if home is unknown or the directory cannot be created.
    pub fn open_default() -> anyhow::Result<Db>;
    /// Opens the file at path. If it exists but is not a valid SQLite database
    /// (rusqlite reports NotADatabase, or PRAGMA integrity_check fails),
    /// rename it to "<path>.broken-<unix seconds>" and create a fresh database.
    /// Never deletes the old file.
    pub fn open(path: &std::path::Path) -> anyhow::Result<Db>;
    pub fn open_in_memory() -> anyhow::Result<Db>;
    /// Inserts a row with name NULL. Returns the row id.
    pub fn record_game(&self, rec: &GameRecord) -> anyhow::Result<i64>;
    /// Sets the name on a row.
    pub fn set_name(&self, id: i64, name: &str) -> anyhow::Result<()>;
    /// Named rows only, ordered by score desc then id asc, limited to n.
    pub fn top_scores(&self, n: usize) -> anyhow::Result<Vec<ScoreRow>>;
    /// score > 0 and (fewer than TOP_N named rows or score > the TOP_N-th named score).
    pub fn qualifies(&self, score: u32) -> anyhow::Result<bool>;
}
```

Schema, created with `CREATE TABLE IF NOT EXISTS` on every open:

```sql
CREATE TABLE IF NOT EXISTS games (
  id INTEGER PRIMARY KEY,
  name TEXT,
  score INTEGER NOT NULL,
  apples INTEGER NOT NULL,
  mode INTEGER NOT NULL,
  played_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
```

Tests use `open_in_memory` except the corruption test, which writes garbage bytes to a file in `std::env::temp_dir()` under a unique name, calls `Db::open`, asserts the database works, asserts a sibling file whose name starts with `snake.db.broken-` exists, and cleans up. Cover: record then top_scores returns nothing until set_name, ordering by score desc with tie broken by id, `qualifies(0)` is false, qualifies is true when fewer than 10 named rows, false when equal to the 10th score, true when greater than the 10th score, mode round trips through the column.

Commit as `feat(snake): add sqlite score storage`.

## Task 3: app state machine

Create `src/app.rs`. Add `mod app;` to `src/main.rs` but leave main's loop unchanged for now. No drawing code. Uses `ratatui::crossterm::event::KeyCode` as its input type so tests construct keys directly.

Public interface, exact:

```rust
use ratatui::crossterm::event::KeyCode;
use crate::{db::{Db, ScoreRow}, game::{Game, Mode}};

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
pub const IDLE_TICK: std::time::Duration = std::time::Duration::from_millis(250);

impl App {
    /// Loads the top table. Starts on Menu with a placeholder Game::new(Mode::Variable).
    pub fn new(db: Db) -> anyhow::Result<App>;
    pub fn handle_key(&mut self, key: KeyCode) -> anyhow::Result<()>;
    /// Playing: game.step(); if the game just ended, record it and move to NameEntry or GameOver.
    /// Other screens: no-op.
    pub fn on_tick(&mut self) -> anyhow::Result<()>;
    /// game.tick() while Playing, IDLE_TICK otherwise.
    pub fn tick_rate(&self) -> std::time::Duration;
}
```

Key rules per screen:
- Menu: `Char('1')` to `Char('9')` start `Mode::Fixed(n)`, `Char('v')` starts `Mode::Variable`, `Char('q')` sets quit. Others ignored.
- Playing: Up, Down, Left, Right call `game.turn` with (0,1), (0,-1), (-1,0), (1,0). `Char('q')` sets quit.
- NameEntry: `Char(c)` where `c.is_ascii_alphabetic()` and name length is below NAME_LEN appends `c.to_ascii_uppercase()`. Backspace pops. Enter with length exactly NAME_LEN calls `db.set_name`, reloads `top`, moves to GameOver. Enter otherwise ignored. `Char('q')` does not quit here because it is a letter.
- GameOver: `Char('r')` reloads `top` and moves to Menu. `Char('q')` sets quit.

On death inside `on_tick`: call `db.record_game`. Then if `db.qualifies(score)` move to `NameEntry { game_id, name: String::new() }`, else `GameOver`.

Tests use `Db::open_in_memory()`. Cover: menu `5` starts Fixed(5) and Playing, `v` starts Variable, death with score 0 goes to GameOver not NameEntry, death with a qualifying score goes to NameEntry and a row exists, name entry rejects Enter at 2 letters, accepts at 3 and the top table then contains the name, lowercase letters are stored upper case, a 4th letter is ignored, `r` on GameOver returns to Menu, `q` on Menu sets quit. Force a death by setting `game.dir` toward the nearest wall and ticking.

Commit as `feat(snake): add screen state machine`.

## Task 4: screens and terminal loop

Create `src/ui/mod.rs`, `src/ui/menu.rs`, `src/ui/play.rs`, `src/ui/game_over.rs`, `src/ui/name_entry.rs`. Rewrite `src/main.rs` to the loop below. Remove all game code that remains in `main.rs` from Task 1.

`src/ui/mod.rs`:

```rust
pub fn draw(frame: &mut ratatui::Frame, app: &crate::app::App)
```
Matches on `app.screen` and calls the screen module's `draw(frame, app)`.

Screen contents:
- `menu.rs`: bordered block titled " snake ". Body lists the top 10 as `"{rank:>2}. {name}  {score:>5}  {mode label}"`, or `"no scores yet"` if empty. Below it the lines `"1-9  fixed speed"`, `"v    variable speed"`, `"q    quit"`, and `"apple = current level in points"`. Use a `Paragraph`.
- `play.rs`: the existing canvas drawing moved here. Title `format!(" {}  level {}  apples {}  score {}  q quit ", mode label, level, apples, score)`.
- `game_over.rs`: bordered block titled " game over ". Body shows `score`, `apples`, mode label, then `"r  menu"` and `"q  quit"`.
- `name_entry.rs`: bordered block titled " high score ". Body shows `"enter your name"` and the name padded with `_` to 3 characters, for example `"A__"`, then `"Enter to confirm"`.

`src/main.rs`:

```rust
fn main() -> anyhow::Result<()> {
    let db = Db::open_default()?;          // before the terminal is touched
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, db);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, db: Db) -> anyhow::Result<()> {
    let mut app = App::new(db)?;
    while !app.quit {
        terminal.draw(|f| ui::draw(f, &app))?;
        if event::poll(app.tick_rate())? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press { app.handle_key(k.code)?; }
            }
        } else {
            app.on_tick()?;
        }
    }
    Ok(())
}
```

Note: `on_tick` runs only when poll times out, so a key press does not also advance the snake. That is intended.

Verify by running `cargo build`, `cargo test`, and `cargo clippy` if available. Manual play is not possible for the implementer; note that in the report.

Commit as `feat(snake): add menu, name entry, and game over screens`.
